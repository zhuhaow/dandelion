use tokio::io::{AsyncRead, AsyncWrite};

pub trait Io: AsyncRead + AsyncWrite + Unpin + Send + 'static {}

impl<T> Io for T where T: AsyncRead + AsyncWrite + Unpin + Send + 'static {}
