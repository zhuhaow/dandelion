use std::sync::Arc;

use crate::{
    core::{endpoint::Endpoint, resolver::Resolver},
    Result,
};
use anyhow::bail;
use futures::{future::select_ok, FutureExt};
use quinn::{crypto::rustls::QuicClientConfig, ClientConfig, Connection, Endpoint as QuicEndpoint};
use rustls_platform_verifier::ConfigVerifierExt;

pub async fn create_quic_connection<R: Resolver>(
    server: Endpoint,
    resolver: R,
    alpn_protocols: Vec<Vec<u8>>,
) -> Result<Connection> {
    match server {
        // It's too unlikely there will be a ip cert, plus we need to verify if
        // ring will correctly validate this.
        Endpoint::Addr(addr) => bail!(
            "Cannot connect to remote with ip {}, domain is required for certificate validation",
            addr
        ),
        Endpoint::Domain(host, port) => {
            let addrs = resolver.lookup_ip(&host).await?;
            let host_ref = &host;

            let crypto_config = {
                let mut config = rustls::ClientConfig::with_platform_verifier();
                config.alpn_protocols = alpn_protocols;
                Arc::new(QuicClientConfig::try_from(config)?)
            };

            let connection = select_ok(addrs.into_iter().map(|addr| {
                let config = ClientConfig::new(crypto_config.clone());

                async move {
                    match addr {
                        std::net::IpAddr::V4(addr_v4) => Ok::<_, anyhow::Error>(
                            QuicEndpoint::client((std::net::Ipv4Addr::UNSPECIFIED, 0).into())?
                                .connect_with(config, (addr_v4, port).into(), host_ref)?
                                .await?,
                        ),
                        std::net::IpAddr::V6(addr_v6) => Ok(QuicEndpoint::client(
                            (std::net::Ipv6Addr::UNSPECIFIED, 0).into(),
                        )?
                        .connect_with(config, (addr_v6, port).into(), host_ref)?
                        .await?),
                    }
                }
                .boxed()
            }))
            .await?
            .0;

            Ok(connection)
        }
    }
}
