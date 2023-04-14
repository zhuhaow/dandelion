use crate::io::Io;
use async_compat::Compat;
use futures::{stream::TryStreamExt, task::AtomicWaker, SinkExt, Stream};
use std::task::Poll;
use tokio_tungstenite::{
    tungstenite::{error::Error as WsError, Message},
    WebSocketStream,
};

lazy_static::lazy_static! {
    static ref EOF_MESSAGE: Message = Message::Text("EOF".to_string());
}

pub fn into_io<C: Io>(stream: WebSocketStream<C>) -> impl Io {
    let stream = WebSocketStreamToAsyncWrite::new(stream).into_async_read();
    Compat::new(stream)
}

#[pin_project::pin_project]
pub struct WebSocketStreamToAsyncWrite<C: Io> {
    stream: WebSocketStream<C>,
    read_closed: bool,
    write_closed: bool,
    waker: AtomicWaker,
}

impl<C: Io> WebSocketStreamToAsyncWrite<C> {
    pub fn new(stream: WebSocketStream<C>) -> Self {
        Self {
            stream,
            read_closed: false,
            write_closed: false,
            waker: AtomicWaker::default(),
        }
    }
}

fn ws_to_io_error(error: WsError) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other, error)
}

fn is_eof(message: &Message) -> bool {
    message == &*EOF_MESSAGE
}

// Here we implement futures AsyncWrite and then use Compat to support tokio's.
impl<C: Io> futures::io::AsyncWrite for WebSocketStreamToAsyncWrite<C> {
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
                        .start_send_unpin(Message::Binary(buf.to_owned()))
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

    fn poll_close(
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

pub struct MessageWrapper(Message);

static EMPTY_BUF: [u8; 0] = [];

impl AsRef<[u8]> for MessageWrapper {
    fn as_ref(&self) -> &[u8] {
        match self.0 {
            Message::Binary(ref bytes) => bytes.as_ref(),
            _ => EMPTY_BUF.as_ref(),
        }
    }
}

impl<C: Io> Stream for WebSocketStreamToAsyncWrite<C> {
    type Item = std::io::Result<MessageWrapper>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let this = self.project();
        let stream = this.stream;
        let message = futures::ready!(stream.try_poll_next_unpin(cx));

        match message {
            Some(m) => match m {
                Ok(m) => {
                    if is_eof(&m) {
                        *this.read_closed = true;
                        if *this.write_closed {
                            this.waker.wake();
                        }
                        Poll::Ready(None)
                    } else {
                        Poll::Ready(Some(Ok(MessageWrapper(m))))
                    }
                }
                Err(err) => Poll::Ready(Some(Err(ws_to_io_error(err)))),
            },
            None => Poll::Ready(None),
        }
    }
}
