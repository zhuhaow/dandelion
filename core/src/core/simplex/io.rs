use crate::core::io::Io;
use bytes::{Buf, Bytes};
use futures::{stream::TryStreamExt, task::AtomicWaker, SinkExt};
use std::task::Poll;
use tokio::io::{AsyncBufRead, AsyncRead};
use tokio_tungstenite::{
    tungstenite::{error::Error as WsError, Message},
    WebSocketStream,
};

lazy_static::lazy_static! {
    static ref EOF_MESSAGE: Message = Message::Text("EOF".into());
}

pub fn into_io<C: Io>(stream: WebSocketStream<C>) -> impl Io {
    WebSocketStreamToAsyncWrite::new(stream)
}

#[derive(Debug)]
#[pin_project::pin_project]
pub struct WebSocketStreamToAsyncWrite<C: Io> {
    stream: WebSocketStream<C>,
    read_closed: bool,
    write_closed: bool,
    waker: AtomicWaker,
    chunk: Option<Bytes>,
}

impl<C: Io> WebSocketStreamToAsyncWrite<C> {
    pub fn new(stream: WebSocketStream<C>) -> Self {
        Self {
            stream,
            read_closed: false,
            write_closed: false,
            waker: AtomicWaker::default(),
            chunk: None,
        }
    }
}

fn ws_to_io_error(error: WsError) -> std::io::Error {
    std::io::Error::other(error)
}

fn is_eof(message: &Message) -> bool {
    message == &*EOF_MESSAGE
}

// Here we implement futures AsyncWrite and then use Compat to support tokio's.
impl<C: Io> tokio::io::AsyncWrite for WebSocketStreamToAsyncWrite<C> {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        let stream = self.project().stream;

        let result = futures::ready!(stream.poll_ready_unpin(cx));

        // TODO: There could be a better way to handle this, we are making copies.
        std::task::Poll::Ready(
            result
                .and_then(|_| {
                    stream
                        .start_send_unpin(Message::Binary(buf.to_owned().into()))
                        .map(|_| buf.len())
                })
                .map_err(ws_to_io_error),
        )
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let stream = self.project().stream;

        let result = futures::ready!(stream.poll_flush_unpin(cx));

        Poll::Ready(result.map_err(ws_to_io_error))
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        // We know now write is done.
        //
        // However, if we trigger close right now, the ws implementation would
        // send a close frame and the other side would immediately stop sending
        // any new data (other than the ones already queued).
        //
        // In order to avoid that, we need to implement our own way of sending
        // EOF.

        let this = self.project();
        let stream = this.stream;

        if !*this.write_closed {
            // Write is not closed, so we need to send EOF first.
            let result = futures::ready!(stream.poll_ready_unpin(cx));
            let result = result
                .and_then(|_| stream.start_send_unpin(EOF_MESSAGE.clone()))
                .map_err(ws_to_io_error);

            if let Err(e) = result {
                return Poll::Ready(Err(e));
            }

            // We send it successfully, mark the write as closed.
            *this.write_closed = true;
        }

        if *this.read_closed {
            // We can close the connection now.
            let result = futures::ready!(stream.poll_close_unpin(cx));

            Poll::Ready(result.map_err(ws_to_io_error))
        } else {
            // Wait for the read side to receive EOF.
            this.waker.register(cx.waker());
            Poll::Pending
        }
    }
}

impl<C: Io> AsyncBufRead for WebSocketStreamToAsyncWrite<C> {
    fn poll_fill_buf(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<std::io::Result<&[u8]>> {
        let this = self.project();
        let stream = this.stream;

        if this.chunk.is_none() {
            let message = futures::ready!(stream.try_poll_next_unpin(cx));

            match message {
                Some(m) => match m {
                    Ok(m) => {
                        if is_eof(&m) {
                            *this.read_closed = true;
                            if *this.write_closed {
                                this.waker.wake();
                            }
                            Poll::Ready(Ok(&[]))
                        } else {
                            *this.chunk = Some(m.into_data());
                            let chunk = this.chunk.as_ref().unwrap();
                            Poll::Ready(Ok(chunk))
                        }
                    }
                    Err(err) => Poll::Ready(Err(ws_to_io_error(err))),
                },
                None => {
                    if !*this.read_closed {
                        // Somehow real EOF came before EOF command
                        *this.read_closed = true;
                        if *this.write_closed {
                            this.waker.wake();
                        }
                    }

                    Poll::Ready(Ok(&[]))
                }
            }
        } else {
            let chunk = this.chunk.as_ref().unwrap();
            Poll::Ready(Ok(chunk))
        }
    }

    fn consume(self: std::pin::Pin<&mut Self>, amt: usize) {
        let chunk = self.project().chunk;

        if amt > 0 {
            chunk.as_mut().expect("No check present").advance(amt);

            if chunk.as_ref().unwrap().is_empty() {
                *chunk = None;
            }
        }
    }
}

impl<C: Io> AsyncRead for WebSocketStreamToAsyncWrite<C> {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        if buf.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }

        let inner_buf = match self.as_mut().poll_fill_buf(cx) {
            Poll::Ready(Ok(buf)) => buf,
            Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
            Poll::Pending => return Poll::Pending,
        };
        let len = std::cmp::min(inner_buf.len(), buf.remaining());
        buf.put_slice(&inner_buf[..len]);

        self.consume(len);
        Poll::Ready(Ok(()))
    }
}
