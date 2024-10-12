use std::str::FromStr;

use crate::{
    core::{
        endpoint::Endpoint,
        io::Io,
        simplex::{io::into_io, Config, ENDPOINT_HEADER_KEY},
    },
    Result,
};
use anyhow::Context;
use http::HeaderName;
use tokio_tungstenite::client_async;
use tungstenite::client::IntoClientRequest;

pub async fn connect<I: Io>(io: I, endpoint: &Endpoint, config: &Config) -> Result<impl Io> {
    let uri = http::uri::Builder::new()
        .authority(config.host.clone())
        .scheme("ws")
        .path_and_query(&config.path)
        .build()
        .with_context(|| {
            format!(
                "Failed to create simplex request URI connecting with server: {} and path: {}",
                &config.host, &config.path
            )
        })?;

    let mut request = uri.into_client_request()?;
    request.headers_mut().insert(
        HeaderName::from_str(&config.secret_header.0)?,
        config.secret_header.1.parse()?,
    );

    request
        .headers_mut()
        .insert(ENDPOINT_HEADER_KEY, endpoint.to_string().parse()?);

    let (stream, _response) = client_async(request, io)
        .await
        .context("Websocket handshaked failed when establishing simplex connection")?;

    Ok(into_io(stream))
}
