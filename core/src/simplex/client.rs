use crate::{
    endpoint::Endpoint,
    io::Io,
    simplex::{io::into_io, Config, SimplexError, ENDPOINT_HEADER_KEY},
    Result,
};
use http::Request;
use tokio_tungstenite::client_async;

pub async fn connect<I: Io>(
    io: I,
    endpoint: &Endpoint,
    config: &Config,
    host: String,
) -> Result<impl Io + 'static> {
    let uri = http::uri::Builder::new()
        .authority(host)
        .scheme("ws")
        .path_and_query(&config.path)
        .build()
        .map_err(Into::<SimplexError>::into)?;

    let request = Request::builder()
        .uri(uri)
        .header(&config.secret_header.0, &config.secret_header.1)
        .header(ENDPOINT_HEADER_KEY, endpoint.to_string())
        .body(())
        .map_err(Into::<SimplexError>::into)?;

    let (stream, _response) = client_async(request, io).await?;

    Ok(into_io(stream))
}
