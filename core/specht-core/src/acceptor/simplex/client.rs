use crate::{
    acceptor::simplex::{io::into_io, Config, ENDPOINT_HEADER_KEY},
    endpoint::Endpoint,
    io::Io,
    Result,
};
use anyhow::Context;
use http::Request;
use tokio_tungstenite::client_async;

pub async fn connect<I: Io>(
    io: I,
    endpoint: &Endpoint,
    config: &Config,
    host: String,
) -> Result<impl Io> {
    let uri = http::uri::Builder::new()
        .authority(host.clone())
        .scheme("ws")
        .path_and_query(&config.path)
        .build()
        .with_context(|| {
            format!(
                "Failed to create simplex request URI connecting with server: {} and path: {}",
                host, &config.path
            )
        })?;

    let request = Request::builder()
        .uri(uri)
        .header(&config.secret_header.0, &config.secret_header.1)
        .header(ENDPOINT_HEADER_KEY, endpoint.to_string())
        .body(())
        .with_context(|| {
            format!(
                "Failed to create simplex request connecting to {}",
                endpoint
            )
        })?;

    let (stream, _response) = client_async(request, io)
        .await
        .context("Websocket handshaked failed when establishing simplex connection")?;

    Ok(into_io(stream))
}
