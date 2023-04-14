use super::Connector;
use crate::{endpoint::Endpoint, Result};
use anyhow::ensure;
use std::{
    collections::VecDeque,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
use tracing::info;

type PoolItem<I> = (Result<I>, Instant);

pub struct PoolConnector<C: Connector + Clone + 'static> {
    endpoint: Endpoint,
    connector: C,
    // Here we use Result instead of using the stream directly. This helps us to
    // provide a natural backpressure if the there is issue with network that
    // all connections will fail.
    pool: Arc<Mutex<VecDeque<PoolItem<C::Stream>>>>,
    timeout: Duration,
}

impl<C: Connector + Clone + 'static> PoolConnector<C> {
    pub fn new(connector: C, endpoint: Endpoint, size: usize, timeout: Duration) -> Self {
        let c = Self {
            endpoint,
            connector,
            pool: Arc::new(Mutex::new(VecDeque::with_capacity(size))),
            timeout,
        };

        for _ in 0..size {
            c.fill();
        }

        c
    }

    fn fill(&self) {
        let endpoint = self.endpoint.clone();
        let pool = self.pool.clone();
        let connector = self.connector.clone();

        tokio::spawn(async move {
            let connection = connector.connect(&endpoint).await;
            pool.lock().await.push_back((connection, Instant::now()));
        });
    }
}

#[async_trait::async_trait]
impl<C: Connector + Clone + 'static> Connector for PoolConnector<C> {
    type Stream = C::Stream;

    async fn connect(&self, endpoint: &Endpoint) -> Result<Self::Stream> {
        ensure!(
            endpoint == &self.endpoint,
            "The connection pool can only connects to {}, but got request to {}",
            self.endpoint,
            endpoint
        );

        let mut pool = self.pool.lock().await;

        while let Some((result, time)) = pool.pop_front() {
            // pool is already locked so we won't have the fill() filling
            // connection to the pool at the same time
            self.fill();

            if time.elapsed() > self.timeout {
                continue;
            }

            match result {
                Ok(s) => {
                    return Ok(s);
                }
                Err(e) => info!(
                    "Pool failed to connect to {}: {}, trying next connection",
                    endpoint, e
                ),
            }
        }

        drop(pool);

        self.connector.connect(endpoint).await
    }
}
