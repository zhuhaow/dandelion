pub mod http;
pub mod socks5;

use crate::{
    core::{endpoint::Endpoint, io::Io},
    Result,
};
use futures::{Future, Stream, TryFutureExt, TryStreamExt};

pub fn handle_connection_stream<
    Input: Io,
    F: Future<Output = Result<(Endpoint, impl Future<Output = Result<impl Io>>)>>,
>(
    s: impl Stream<Item = Result<Input>>,
    handshake: impl Fn(Input) -> F + 'static,
) -> impl Stream<Item = Result<Result<(Endpoint, impl Future<Output = Result<impl Io>>)>>> {
    s.and_then(move |io| handshake(io).map_ok(Ok))
}

pub fn handle_connection_stream_with_config<
    Input: Io,
    C,
    F: Future<Output = Result<(Endpoint, impl Future<Output = Result<impl Io>>)>>,
>(
    s: impl Stream<Item = Result<Input>>,
    handshake: impl Fn(Input, &C) -> F,
    config: C,
) -> impl Stream<Item = Result<Result<(Endpoint, impl Future<Output = Result<impl Io>>)>>> {
    s.and_then(move |io| handshake(io, &config).map_ok(Ok))
}
