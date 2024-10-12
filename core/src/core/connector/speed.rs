use crate::core::{endpoint::Endpoint, io::Io};
use anyhow::Result;
use futures::{
    future::{select_ok, FutureExt},
    Future,
};
use std::time::Duration;
use tokio::time::sleep;

pub async fn connect<
    F: Future<Output = Result<impl Io>> + Send,
    C: (FnOnce(&Endpoint) -> F) + Send,
>(
    connectors: Vec<(Duration, C)>,
    endpoint: &Endpoint,
) -> Result<impl Io> {
    select_ok(connectors.into_iter().map(|c| {
        async move {
            sleep(c.0).await;

            c.1(endpoint).await
        }
        .boxed()
    }))
    .await
    .map(|r| r.0)
}
