use super::geoip::GeoIpBuilder;
use crate::{
    connector::{
        block::BlockConnector,
        http::HttpConnector,
        rule::{
            all::AllRule,
            dns_fail::DnsFailRule,
            domain::{DomainRule, Mode},
            geoip::GeoRule,
            ip::IpRule,
            Rule, RuleConnector,
        },
        simplex::SimplexConnector,
        socks5::Socks5Connector,
        speed::SpeedConnector,
        tcp::TcpConnector,
        tls::TlsConnector,
        BoxedConnector, Connector,
    },
    endpoint::Endpoint,
    geoip::Source,
    resolver::{system::SystemResolver, udp::UdpResolver, Resolver},
    simplex::Config,
    Result,
};
use anyhow::Error;
use futures::{Future, StreamExt, TryStreamExt};
use ipnetwork::{IpNetwork, Ipv4Network};
use iso3166_1::CountryCode;
use serde::Deserialize;
use serde_with::{serde_as, DisplayFromStr, DurationMilliSeconds};
use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub acceptors: Vec<AcceptorConfig>,
    pub connector: ConnectorConfig,
    pub resolver: ResolverConfig,
}

impl ServerConfig {
    pub fn tun_enabled(&self) -> bool {
        self.acceptors
            .iter()
            .any(|a| matches!(a, &AcceptorConfig::Tun { .. }))
    }
}

#[derive(Debug, Deserialize)]
pub enum ResolverConfig {
    System,
    Udp(SocketAddr),
}

impl ResolverConfig {
    pub fn is_system(&self) -> bool {
        matches!(self, &ResolverConfig::System)
    }

    pub async fn get_resolver(&self) -> Result<Arc<dyn Resolver>> {
        match self {
            ResolverConfig::System => Ok(Arc::new(SystemResolver::new())),
            ResolverConfig::Udp(addr) => Ok(Arc::new(UdpResolver::new(*addr).await?)),
        }
    }
}

#[derive(Debug, Deserialize)]
pub enum AcceptorConfig {
    Socks5 {
        addr: SocketAddr,
    },
    Simplex {
        addr: SocketAddr,
        path: String,
        secret_key: String,
        secret_value: String,
    },
    Http {
        addr: SocketAddr,
    },
    Tun {
        listen_addr: SocketAddr,
        subnet: Ipv4Network,
    },
}

impl AcceptorConfig {
    pub fn server_addr(&self) -> &SocketAddr {
        match self {
            AcceptorConfig::Socks5 { addr }
            | AcceptorConfig::Simplex { addr, .. }
            | AcceptorConfig::Http { addr } => addr,
            AcceptorConfig::Tun {
                listen_addr: addr, ..
            } => addr,
        }
    }
}

type ConnectorIndex = String;

serde_with::serde_conv!(
    Iso31661,
    CountryCode<'static>,
    |code: &CountryCode| code.alpha2.to_owned(),
    |value: String| -> Result<_, Error> {
        iso3166_1::alpha2(value.as_str())
            .ok_or_else(|| anyhow::anyhow!("{} is not a valid ISO3166-1 name.", value))
    }
);

#[serde_as]
#[derive(Debug, Deserialize)]
pub enum RuleEntry {
    All(ConnectorIndex),
    DnsFail(ConnectorIndex),
    Domain {
        modes: Vec<Mode>,
        index: ConnectorIndex,
    },
    GeoIp {
        #[serde_as(as = "Option<Iso31661>")]
        country: Option<CountryCode<'static>>,
        equal: bool,
        index: ConnectorIndex,
    },
    Ip {
        subnets: Vec<IpNetwork>,
        index: ConnectorIndex,
    },
}

// Workaround issue https://github.com/rust-lang/rust/issues/63033
#[allow(clippy::manual_async_fn)]
fn get_rule_connector<'a>(
    geoip_config: &'a Option<Source>,
    connectors: &'a HashMap<String, Box<ConnectorConfig>>,
    rules: &'a [RuleEntry],
    resolver: Arc<dyn Resolver>,
) -> impl Future<Output = Result<BoxedConnector>> + 'a {
    #[allow(clippy::manual_async_fn)]
    fn get_connector<'a>(
        connectors: &'a HashMap<String, Box<ConnectorConfig>>,
        ind: &'a str,
        resolver: Arc<dyn Resolver>,
    ) -> impl Future<Output = Result<BoxedConnector>> + 'a {
        async move {
            let config = connectors
                .get(ind)
                .ok_or_else(|| anyhow::anyhow!("Failed to find connector named {}", ind))?;

            config.get_connector(resolver).await
        }
    }

    async move {
        let mut geo_ip_builder = geoip_config
            .as_ref()
            .map(|s| GeoIpBuilder::new(s.to_owned()));
        let mut connector_rules: Vec<Box<dyn Rule>> = Vec::new();
        for entry in rules.iter() {
            let rule: Box<dyn Rule> = match entry {
                RuleEntry::All(ind) => Box::new(AllRule::new(
                    get_connector(connectors, ind, resolver.clone()).await?,
                )),
                RuleEntry::DnsFail(ind) => Box::new(DnsFailRule::new(
                    get_connector(connectors, ind, resolver.clone()).await?,
                )),
                RuleEntry::Domain { modes, index } => Box::new(DomainRule::new(
                    modes.clone(),
                    get_connector(connectors, index, resolver.clone()).await?,
                )),
                RuleEntry::GeoIp {
                    country,
                    equal,
                    index,
                } => Box::new(GeoRule::new(
                    get_connector(connectors, index, resolver.clone()).await?,
                    geo_ip_builder
                        .as_mut()
                        .ok_or_else(|| {
                            anyhow::anyhow!("Must provide geoip config to enable geo based rule.")
                        })?
                        .get()
                        .await?,
                    country.clone(),
                    *equal,
                )),
                RuleEntry::Ip { subnets, index } => Box::new(IpRule::new(
                    subnets.clone(),
                    get_connector(connectors, index, resolver.clone()).await?,
                )),
            };

            connector_rules.push(rule);
        }

        Ok(RuleConnector::new(connector_rules).boxed())
    }
}

#[serde_as]
#[derive(Debug, Deserialize)]
pub enum ConnectorConfig {
    Direct,
    Simplex {
        #[serde_as(as = "DisplayFromStr")]
        endpoint: Endpoint,
        path: String,
        secret_key: String,
        secret_value: String,
        next: Box<ConnectorConfig>,
    },
    Tls(Box<ConnectorConfig>),
    Rule {
        geoip: Option<Source>,
        connectors: HashMap<String, Box<ConnectorConfig>>,
        rules: Vec<RuleEntry>,
    },
    Speed(#[serde_as(as = "Vec<(DurationMilliSeconds, _)>")] Vec<(Duration, Box<ConnectorConfig>)>),
    Http {
        #[serde_as(as = "DisplayFromStr")]
        endpoint: Endpoint,
        next: Box<ConnectorConfig>,
    },
    Socks5 {
        #[serde_as(as = "DisplayFromStr")]
        endpoint: Endpoint,
        next: Box<ConnectorConfig>,
    },
    Block,
}

impl ConnectorConfig {
    #[async_recursion::async_recursion]
    pub async fn get_connector(&self, resolver: Arc<dyn Resolver>) -> Result<BoxedConnector> {
        match self {
            ConnectorConfig::Direct => Ok(TcpConnector::new(resolver).boxed()),
            ConnectorConfig::Simplex {
                endpoint,
                path,
                secret_key,
                secret_value,
                next,
            } => Ok(SimplexConnector::new(
                endpoint.clone(),
                Config::new(
                    path.to_owned(),
                    (secret_key.to_owned(), secret_value.to_owned()),
                ),
                next.get_connector(resolver).await?,
            )
            .boxed()),
            ConnectorConfig::Tls(c) => {
                Ok(TlsConnector::new(c.get_connector(resolver).await?).boxed())
            }
            ConnectorConfig::Rule {
                geoip,
                connectors,
                rules,
            } => get_rule_connector(geoip, connectors, rules, resolver).await,
            ConnectorConfig::Speed(c) => Ok(SpeedConnector::new(
                futures::stream::iter(c.iter())
                    .then(|c| async {
                        Ok::<_, Error>((c.0, c.1.get_connector(resolver.clone()).await?))
                    })
                    .try_collect()
                    .await?,
            )
            .boxed()),
            ConnectorConfig::Http { endpoint, next } => Ok(HttpConnector::new(
                next.get_connector(resolver).await?,
                endpoint.clone(),
            )
            .boxed()),
            ConnectorConfig::Socks5 { endpoint, next } => Ok(Socks5Connector::new(
                next.get_connector(resolver).await?,
                endpoint.clone(),
            )
            .boxed()),
            ConnectorConfig::Block => Ok(BlockConnector {}.boxed()),
        }
    }
}

#[cfg(test)]
mod test {
    use super::ServerConfig;
    use crate::Result;
    use rstest::rstest;
    use std::{env, fs::read_to_string, path::Path};

    async fn test_config_file(content: &str, success: bool) -> Result<()> {
        let config: ServerConfig = ron::de::from_str(content)?;

        let factory_result = config
            .connector
            .get_connector(config.resolver.get_resolver().await?)
            .await;

        if success {
            factory_result?;
        } else {
            assert!(factory_result.is_err());
        }

        Ok(())
    }

    #[rstest]
    #[case("local.ron", true)]
    #[case("remote.ron", true)]
    #[case("rule_without_geo.ron", true)]
    #[case("wrong_rule.ron", false)]
    #[case("multiple_acceptors.ron", true)]
    #[trace]
    #[tokio::test]
    async fn config_file_without_geo(#[case] filename: &str, #[case] success: bool) -> Result<()> {
        let path = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("config")
            .join(filename);
        let content = read_to_string(path)?;
        test_config_file(&content, success).await
    }

    #[rstest]
    #[case("rule_with_geo.ron", true)]
    #[ignore]
    #[trace]
    #[tokio::test]
    async fn config_file_with_geo(#[case] filename: &str, #[case] success: bool) -> Result<()> {
        let path = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("config")
            .join(filename);
        let content = read_to_string(path)?;

        // We skip test when we explicitly disable it.
        if env::var_os("SKIP_MAXMINDDB_TESTS").is_some() {
            return Ok(());
        }

        let license = env::var("MAXMINDDB_LICENSE")?;
        let content = content.replace("$$LICENSE$$", &license);

        test_config_file(&content, success).await
    }
}
