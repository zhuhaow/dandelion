pub mod client;
pub mod io;
pub mod server;

// Simplex is a lightweight protocol that based on WebSocket with only 1 extra RTT delay.
// I haven't implemented the server yet.

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
