use std::{str::FromStr, sync::Arc};

use super::{Acceptor, HandshakeResult};
use crate::{endpoint::Endpoint, io::Io};
use anyhow::{bail, ensure, Result};
use futures::{Future, FutureExt};
use http::{
    header::{CONNECTION, HOST, PROXY_AUTHENTICATE, PROXY_AUTHORIZATION},
    Method, Request, Response,
};
use hyper::{client::conn::SendRequest, server::conn::Http, service::service_fn, Body};
use tokio::{
    io::duplex,
    sync::{
        oneshot::{channel, Receiver, Sender},
        Mutex,
    },
};
use tower::ServiceExt;

#[derive(Debug)]
pub struct HttpAcceptor {}

#[async_trait::async_trait]
impl<I: Io> Acceptor<I> for HttpAcceptor {
    async fn do_handshake(&self, io: I) -> HandshakeResult {
        let (endpoint, fut) = handshake(io).await?;
        Ok((
            endpoint,
            async move {
                let io: Box<dyn Io> = Box::new(fut.await?);
                Ok(io)
            }
            .boxed(),
        ))
    }
}

enum State {
    NotConnected(Option<ConnectSignal>),
    Connected((Endpoint, SendRequest<Body>)),
}

struct ConnectSignal {
    endpoint_tx: Sender<(bool, Endpoint)>,
    done_rx: Receiver<Option<SendRequest<Body>>>,
}

fn transform_proxy_request(mut request: Request<Body>) -> Option<Request<Body>> {
    if request.headers().contains_key(HOST) {
        let host = request.uri().authority()?.host().parse().ok()?;
        request.headers_mut().insert(HOST, host);
    }

    *request.uri_mut() = request.uri().path_and_query()?.as_str().parse().ok()?;

    request.headers_mut().remove(PROXY_AUTHENTICATE);
    request.headers_mut().remove(PROXY_AUTHORIZATION);

    // Map Proxy-Connection to Connection if necessary
    if let Some(c) = request.headers_mut().remove("Proxy-Connection") {
        request.headers_mut().entry(CONNECTION).or_insert(c);
    }

    Some(request)
}

async fn handler(request: Request<Body>, state: Arc<Mutex<State>>) -> Result<Response<Body>> {
    let mut state = state.lock().await;

    if matches!(request.method(), &Method::CONNECT) {
        if let State::NotConnected(signal) = &mut *state {
            if let Some(signal) = signal.take() {
                signal
                    .endpoint_tx
                    .send((true, Endpoint::from_str(&request.uri().to_string())?))
                    .expect("the other side should not be released");

                signal
                    .done_rx
                    .await
                    .expect("the done signal should be sent before polling the connection");

                return Ok(Response::new(Body::empty()));
            }
        }
        bail!("The CONNECT method can only be send in the first header")
    } else {
        match &mut *state {
            State::NotConnected(signal) => match signal.take() {
                Some(signal) => {
                    let host = request.uri().host().ok_or_else(|| {
                        anyhow::anyhow!("Invalid proxy request with no host in uri.")
                    })?;

                    let endpoint = Endpoint::from_str(host)
                        .or_else(|_| Endpoint::from_str(format!("{}:80", host).as_str()))?;

                    signal
                        .endpoint_tx
                        .send((false, endpoint.clone()))
                        .expect("the other side should not be released");

                    let mut send_request = signal
                        .done_rx
                        .await
                        .expect("the done signal should be sent before polling the connection")
                        .unwrap();

                    let request = transform_proxy_request(request)
                        .ok_or_else(|| anyhow::anyhow!("Not a valid proxy request"))?;
                    let response_fut = send_request.send_request(request);

                    *state = State::Connected((endpoint, send_request));

                    return Ok(response_fut.await?);
                }
                None => {
                    unreachable!()
                }
            },
            State::Connected((ref target_endpoint, ref mut send_request)) => {
                let host = request
                    .uri()
                    .host()
                    .ok_or_else(|| anyhow::anyhow!("Invalid proxy request with no host in uri"))?;

                let endpoint = Endpoint::from_str(host)
                    .or_else(|_| Endpoint::from_str(format!("{}:80", host).as_str()))?;

                ensure!(
                    &endpoint == target_endpoint,
                    "Do not support using same connection for requests to different hosts"
                );

                let request = transform_proxy_request(request)
                    .ok_or_else(|| anyhow::anyhow!("Not a valid proxy request"))?;

                Ok(send_request.ready().await?.send_request(request).await?)
            }
        }
    }
}

pub async fn handshake(io: impl Io) -> Result<(Endpoint, impl Future<Output = Result<impl Io>>)> {
    let (endpoint_tx, endpoint_rx) = channel();
    let (done_tx, done_rx) = channel();

    let state = Arc::new(Mutex::new(State::NotConnected(Some(ConnectSignal {
        endpoint_tx,
        done_rx,
    }))));

    let mut conn = Http::new()
        .serve_connection(
            io,
            service_fn(move |req| {
                {
                    let state = state.clone();
                    handler(req, state)
                }
                .boxed()
            }),
        )
        .without_shutdown();

    let endpoint = tokio::select! {
        _ = &mut conn=> {
            // The client is closed before we get first header. Simple close it.
            bail!("No HTTP request received.");
        }
        result = endpoint_rx => {
            match result {
                Ok(endpoint) => endpoint,
                Err(_) => unreachable!(),
            }
        }
    };

    if endpoint.0 {
        Ok((
            endpoint.1,
            async move {
                done_tx
                    .send(None)
                    .expect("bug: the done signal receiver should not be deallocated");

                let part = conn.await?;

                let io: Box<dyn Io> = Box::new(part.io);
                Ok(io)
            }
            .boxed(),
        ))
    } else {
        Ok((
            endpoint.1,
            async move {
                // 64KB
                let (s1, s2) = duplex(65536);

                let (request_sender, connection) = hyper::client::conn::handshake(s1).await?;

                done_tx
                    .send(Some(request_sender))
                    .expect("bug: the done signal receiver should not be deallocated");

                // We don't really care the error from here since it will drop the connection.
                // We will then read the EOF from the other side.
                tokio::spawn(conn);
                tokio::spawn(connection);

                let io: Box<dyn Io> = Box::new(s2);
                Ok(io)
            }
            .boxed(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use http::Uri;
    use rstest::*;

    // Make sure the Uri crate would parse the data as we expected
    #[rstest]
    #[case("google.com", None)]
    #[case("https://google.com", Some("/"))]
    #[case("https://google.com/", Some("/"))]
    #[case("https://google.com/test", Some("/test"))]
    #[case("https://google.com/test?query=1", Some("/test?query=1"))]
    #[case("/test?query=1", Some("/test?query=1"))]
    #[case("/test", Some("/test"))]
    #[case("/", Some("/"))]
    #[trace]
    fn uri_parsed_as_expected(#[case] case: Uri, #[case] expected: Option<&str>) {
        let pq = case.path_and_query().map(|p| p.as_str());
        assert_eq!(pq, expected);
    }
}
