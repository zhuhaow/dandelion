pub mod client;
pub mod io;





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
