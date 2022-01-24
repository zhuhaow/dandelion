use super::Connector;
use crate::endpoint::Endpoint;
use crate::Result;
use anyhow::ensure;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct PoolConnector<C: Connector + Clone + 'static> {
    endpoint: Endpoint,
    connector: C,
    // Here we use Result instead of using the stream directly. This helps us to
    // provide a natural backpressure if the there is issue with network that
    // all connections will fail.
    pool: Arc<Mutex<VecDeque<Result<C::Stream>>>>,
}

impl<C: Connector + Clone + 'static> PoolConnector<C> {
    pub fn new(connector: C, endpoint: Endpoint, size: usize) -> Self {
        let c = Self {
            endpoint,
            connector,
            pool: Arc::new(Mutex::new(VecDeque::with_capacity(size))),
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
            pool.lock()
                .await
                .push_back(connector.connect(&endpoint).await);
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

        while let Some(result) = pool.pop_front() {
            // pool is already locked so we won't have the fill() fill
            // connection to the pool at the same time
            self.fill();

            if let Ok(s) = result {
                return Ok(s);
            }
        }

        self.connector.connect(endpoint).await
    }
}
