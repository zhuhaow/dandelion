pub mod io;

use crate::{io::Io, simplex::io::into_io, Endpoint, Result};
use http::Request;
use tokio_tungstenite::client_async;

// Simplex is a lightweight protocol that based on WebSocket with only 1 extra RTT delay.
// I haven't implemented the server yet.

#[derive(thiserror::Error, Debug)]
pub enum SimplexError {
    #[error("Failed to create simplex connection header: {0}")]
    HeaderConfigInvalid(#[from] http::Error),
}

#[derive(Clone)]
pub struct Config {
    path: String,
    secret_header: (String, String),
}

impl Config {
    pub fn new(path: String, secret_header: (String, String)) -> Self {
        Self {
            path,
            secret_header,
        }
    }
}

pub async fn connect<I: Io>(
    io: I,
    endpoint: &Endpoint,
    config: &Config,
    host: String,
) -> Result<impl Io + 'static> {
    let request = Request::builder()
        .uri(format!("{}/{}", host, config.path))
        .header(&config.secret_header.0, &config.secret_header.1)
        .header("Simplex Connect", endpoint.to_string())
        .body(())
        .map_err(Into::<SimplexError>::into)?;

    let (stream, _response) = client_async(request, io).await?;

    Ok(into_io(stream))
}
