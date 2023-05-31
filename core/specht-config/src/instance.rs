
use std::sync::Arc;

use futures::future::select_all;
use futures::FutureExt;

use specht_core::{
    acceptor::{http, socks5},
    Result,
};

use crate::{
    acceptor::handle_acceptors,
    engine::{AcceptorConfig, ConfigEngine, InstanceConfig},
};

pub struct Instance {
    config: InstanceConfig,
    engine: Arc<ConfigEngine>,
}

impl Instance {
    pub async fn load_config(code: impl AsRef<str>) -> Result<Self> {
        let engine = Arc::new(ConfigEngine::compile_config(code)?);

        let config = engine.eval_config()?;

        Ok(Self { config, engine })
    }

    pub async fn run(&self) -> Result<()> {
        select_all(self.config.acceptors.iter().map(|c| {
            match c {
                AcceptorConfig::Socks5(addr, handler) => handle_acceptors(
                    addr,
                    socks5::handshake,
                    self.engine.clone(),
                    handler.to_owned(),
                )
                .boxed_local(),
                AcceptorConfig::Http(addr, handler) => handle_acceptors(
                    addr,
                    http::handshake,
                    self.engine.clone(),
                    handler.to_owned(),
                )
                .boxed_local(),
            }
        }))
        .await
        .0
    }
}
