pub mod io;

#[derive(thiserror::Error, Debug)]
pub enum SimplexError {
    #[error("Failed to create simplex connection header: {0}")]
    HeaderConfigInvalid(#[from] http::Error),
}
