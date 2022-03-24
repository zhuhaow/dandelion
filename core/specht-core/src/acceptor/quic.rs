use super::{Acceptor, HandshakeResult};
use crate::{endpoint::Endpoint, io::Io, quic::QuicMessage};
use futures::FutureExt;
use std::str::FromStr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

struct QuicAcceptor {}

// TODO: This is not necessary a Quic implementation but more like
// a protocol without authentication.
#[async_trait::async_trait]
impl<I: Io> Acceptor<I> for QuicAcceptor {
    async fn do_handshake(&self, mut io: I) -> HandshakeResult {
        let len = io.read_u8().await?;
        let mut buf = vec![0_u8; len as usize];
        io.read_exact(&mut buf).await?;

        let target = String::from_utf8(buf)?;
        let endpoint = Endpoint::from_str(target.as_str())?;

        Ok((
            endpoint,
            async move {
                io.write_u8(QuicMessage::Ok as u8).await?;
                let io: Box<dyn Io> = Box::new(io);
                Ok(io)
            }
            .boxed(),
        ))
    }
}
