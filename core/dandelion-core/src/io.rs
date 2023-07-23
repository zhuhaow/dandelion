use std::fmt::Debug;
use tokio::io::{AsyncRead, AsyncWrite};

pub trait Io: AsyncRead + AsyncWrite + Unpin + Send + 'static + Debug {}

impl<T> Io for T where T: AsyncRead + AsyncWrite + Unpin + Send + 'static + Debug {}
