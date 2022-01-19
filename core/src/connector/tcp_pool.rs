use super::tcp::TcpConnector;
use super::Connector;
use crate::endpoint::Endpoint;
use crate::resolver::Resolver;
use crate::Result;
use anyhow::ensure;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::Mutex;

pub struct TcpPoolConnector<R: Resolver + Clone + 'static> {
    resolver: R,
    endpoint: Endpoint,
    // Here we use Result instead of using the stream directly. This helps us to
    // provide a natural backpressure if the there is issue with network that
    // all connections will fail.
    pool: Arc<Mutex<VecDeque<Result<TcpStream>>>>,
}

impl<R: Resolver + Clone + 'static> TcpPoolConnector<R> {
    pub fn new(resolver: R, endpoint: Endpoint, size: usize) -> Self {
        let connector = Self {
            resolver,
            endpoint,
            pool: Arc::new(Mutex::new(VecDeque::with_capacity(size))),
        };

        for _ in 0..size {
            connector.fill();
        }

        connector
    }

    fn fill(&self) {
        let connector = TcpConnector::new(self.resolver.clone());

        let endpoint = self.endpoint.clone();
        let pool = self.pool.clone();

        tokio::spawn(async move {
            pool.lock()
                .await
                .push_back(connector.connect(&endpoint).await);
        });
    }
}

#[async_trait::async_trait]
impl<R: Resolver + Clone + 'static> Connector for TcpPoolConnector<R> {
    type Stream = TcpStream;

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

        let connector = TcpConnector::new(self.resolver.clone());

        connector.connect(endpoint).await
    }
}
