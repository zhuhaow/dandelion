use super::Connector;
use crate::{endpoint::Endpoint, resolver::Resolver, Result};
use futures::{future::FusedFuture, Future, FutureExt, TryFutureExt};
use itertools::Itertools;
use socket2::{Socket, TcpKeepalive};
use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    ops::Add,
    pin::Pin,
    time::{Duration, Instant},
    vec::IntoIter,
};
use tokio::{
    net::TcpStream,
    time::{sleep_until, Sleep},
};

#[derive(Debug, Default)]
pub struct TcpConnector<R: Resolver> {
    resolver: R,
}

impl<R: Resolver> TcpConnector<R> {
    pub fn new(resolver: R) -> Self {
        Self { resolver }
    }
}

#[async_trait::async_trait]
impl<R: Resolver> Connector for TcpConnector<R> {
    type Stream = TcpStream;

    async fn connect(&self, endpoint: &Endpoint) -> Result<Self::Stream> {
        match endpoint {
            Endpoint::Addr(addr) => Ok(TcpStream::connect(addr).await?),
            Endpoint::Domain(host, port) => {
                Ok(HappyEyeballConnector::new(&self.resolver, host, *port)
                    .await
                    .map(|s| {
                        let s: Socket = s.into_std().unwrap().into();
                        let _ = s.set_tcp_keepalive(
                            &TcpKeepalive::new()
                                .with_time(Duration::from_secs(60))
                                .with_interval(Duration::from_secs(60)),
                        );
                        let s: std::net::TcpStream = s.into();
                        TcpStream::from_std(s).unwrap()
                    })?)
            }
        }
    }
}

// Implementing https://datatracker.ietf.org/doc/html/rfc8305
//
// This is actually super complicated to implement so it's very unfortunate that
// rust std does not provide support for this.
//
// Anyway, this implementation is implemented based on RFC8305 without
// preference for IPv6, i.e., we will start connecting when we get the first DNS
// response instead of waiting for AAAA result. Given the current status of IPv6
// connectivity, it may be better if we prefer IPv4 connection.
#[pin_project::pin_project]
struct HappyEyeballConnector<'a> {
    ipv4_future: Pin<Box<dyn FusedFuture<Output = Result<Vec<Ipv4Addr>>> + Send + 'a>>,
    ipv6_future: Pin<Box<dyn FusedFuture<Output = Result<Vec<Ipv6Addr>>> + Send + 'a>>,
    ips: IntoIter<IpAddr>,
    ip_count: usize,
    connections: Vec<Pin<Box<dyn FusedFuture<Output = Result<TcpStream>> + Send + 'static>>>,
    next_connection_timer: Pin<Box<Sleep>>,
    host: &'a str,
    port: u16,
}

impl<'a> HappyEyeballConnector<'a> {
    fn new(resolver: &'a impl Resolver, host: &'a str, port: u16) -> Self {
        Self {
            ipv4_future: Box::pin(resolver.lookup_ipv4(host).fuse()),
            ipv6_future: Box::pin(resolver.lookup_ipv6(host).fuse()),
            ips: Vec::new().into_iter(),
            ip_count: 0,
            connections: Vec::new(),
            next_connection_timer: Box::pin(sleep_until(Instant::now().into())),
            host,
            port,
        }
    }

    fn is_resolving(&self) -> bool {
        !(self.ipv4_future.is_terminated() && self.ipv6_future.is_terminated())
    }
}

const CONNECTION_ATTEMP_DELAY: Duration = Duration::from_millis(250);

impl<'a> Future for HappyEyeballConnector<'a> {
    type Output = Result<TcpStream>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        // Only need this for swapping IP iterators
        let this = self.as_mut().project();
        // First we poll the dns result. It doesn't matter in what we order we
        // poll it since we are doing it at the same time.
        if !this.ipv4_future.is_terminated() {
            if let std::task::Poll::Ready(Ok(addrs)) = this.ipv4_future.poll_unpin(cx) {
                *this.ip_count += addrs.len();
                *this.ips = this
                    .ips
                    .interleave(addrs.into_iter().map(Into::into))
                    .collect_vec()
                    .into_iter();
            };
            // Ignore error
        }

        if !this.ipv6_future.is_terminated() {
            if let std::task::Poll::Ready(Ok(addrs)) = this.ipv6_future.poll_unpin(cx) {
                *this.ip_count += addrs.len();
                *this.ips = this
                    .ips
                    .interleave(addrs.into_iter().map(Into::into))
                    .collect_vec()
                    .into_iter();
            };
            // Ignore error
        }

        drop(this);

        if !self.is_resolving() && self.ip_count == 0 {
            return std::task::Poll::Ready(Err(anyhow::anyhow!(
                "Failed to resolve domain {}",
                self.host
            )));
        }

        // Now we poll all ongoing connections
        let (has_pending, has_error, maybe_stream) =
            self.connections
                .iter_mut()
                .fold((false, false, None), |state, c| {
                    if state.2.is_some() {
                        return state;
                    }

                    if c.is_terminated() {
                        return state;
                    }

                    match c.poll_unpin(cx) {
                        std::task::Poll::Ready(result) => match result {
                            Ok(stream) => (state.0, state.1, Some(stream)),
                            Err(_) => (state.0, true, None),
                        },
                        std::task::Poll::Pending => (true, state.1, None),
                    }
                });

        if let Some(stream) = maybe_stream {
            return std::task::Poll::Ready(Ok(stream));
        }

        // Check if we should make new connection
        if !has_pending // No ongoing connection, create a new one now.
            || has_error // One connection is ended, we should start a new one now.
            || self.next_connection_timer.as_mut().poll(cx) == std::task::Poll::Ready(())
        {
            // Loop until we successfully makes a connection.
            loop {
                match self.ips.next() {
                    Some(addr) => {
                        let mut fut = Box::pin(
                            TcpStream::connect((addr, self.port))
                                .map_err(|e| e.into())
                                .fuse(),
                        );
                        match fut.poll_unpin(cx) {
                            std::task::Poll::Ready(result) => match result {
                                // This should be unreachable actually.
                                Ok(s) => return std::task::Poll::Ready(Ok(s)),
                                // Try next IP.
                                Err(_) => continue,
                            },
                            // Good, we initiated an ongoing connection.
                            std::task::Poll::Pending => {
                                self.next_connection_timer
                                    .as_mut()
                                    .reset(Instant::now().add(CONNECTION_ATTEMP_DELAY).into());
                                // The result should always be pending.
                                assert_eq!(
                                    self.next_connection_timer.poll_unpin(cx),
                                    std::task::Poll::Pending
                                );
                                self.connections.push(fut);
                                break;
                            }
                        }
                    }
                    None => {
                        if !self.is_resolving() {
                            return std::task::Poll::Ready(Err(anyhow::anyhow!(
                                "Failed to connect to domain {}",
                                self.host
                            )));
                        } else {
                            break;
                        }
                    }
                }
            }
        }

        std::task::Poll::Pending
    }
}
