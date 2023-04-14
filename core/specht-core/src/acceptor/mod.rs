pub mod http;
pub mod simplex;
pub mod socks5;

use crate::{endpoint::Endpoint, io::Io, Result};
use futures::{Future, Stream, TryFutureExt, TryStreamExt};

pub fn handle_connection_stream<
    Input: Io,
    Output: Io,
    InputStream: Stream<Item = Result<Input>>,
    F1: Future<Output = Result<(Endpoint, F2)>>,
    F2: Future<Output = Result<Output>>,
    Handshake: Fn(Input) -> F1 + 'static,
>(
    s: InputStream,
    handshake: Handshake,
) -> impl Stream<Item = Result<Result<(Endpoint, F2)>>> {
    s.and_then(move |io| handshake(io).map_ok(Ok))
}

pub fn handle_connection_stream_with_config<
    Input: Io,
    Output: Io,
    C,
    InputStream: Stream<Item = Result<Input>>,
    F1: Future<Output = Result<(Endpoint, F2)>>,
    F2: Future<Output = Result<Output>>,
    Handshake: Fn(Input, &C) -> F1,
>(
    s: InputStream,
    handshake: Handshake,
    config: C,
) -> impl Stream<Item = Result<Result<(Endpoint, F2)>>> {
    s.and_then(move |io| handshake(io, &config).map_ok(Ok))
}
