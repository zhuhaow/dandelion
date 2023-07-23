pub mod client;
pub mod io;
pub mod server;

static ENDPOINT_HEADER_KEY: &str = "Simplex-Endpoint";

#[derive(Debug, Clone)]
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
