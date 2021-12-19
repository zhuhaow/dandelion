use crate::{
    acceptor::{http::HttpAcceptor, simplex::SimplexAcceptor, socks5::Socks5Acceptor, Acceptor},
    connector::{
        boxed::BoxedConnectorFactory,
        http::HttpConnectorFactory,
        rule::{
            all::AllRule,
            dns_fail::DnsFailRule,
            domain::{DomainRule, Mode},
            geoip::GeoRule,
            ip::IpRule,
            Rule, RuleConnectorFactory,
        },
        simplex::SimplexConnectorFactory,
        socks5::Socks5ConnectorFactory,
        speed::SpeedConnectorFactory,
        tcp::TcpConnectorFactory,
        tls::TlsConnectorFactory,
    },
    endpoint::Endpoint,
    geoip::Source,
    simplex::Config,
    Result,
};
use anyhow::Error;
use futures::{StreamExt, TryStreamExt};
use ipnetwork::IpNetwork;
use iso3166_1::CountryCode;
use serde::Deserialize;
use serde_with::{serde_as, DisplayFromStr, DurationMilliSeconds};
use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};
use tokio::net::TcpStream;

use super::geoip::GeoIpBuilder;

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub acceptors: Vec<AcceptorConfig>,
    pub connector: ConnectorConfig,
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
}

impl AcceptorConfig {
    pub fn server_addr(&self) -> &SocketAddr {
        match self {
            AcceptorConfig::Socks5 { addr } => addr,
            AcceptorConfig::Simplex {
                addr,
                path: _,
                secret_key: _,
                secret_value: _,
            } => addr,
            AcceptorConfig::Http { addr } => addr,
        }
    }

    pub fn get_acceptor(&self) -> Box<dyn Acceptor<TcpStream>> {
        match self {
            AcceptorConfig::Socks5 { addr: _ } => Box::new(Socks5Acceptor {}),
            AcceptorConfig::Simplex {
                addr: _,
                ref path,
                ref secret_key,
                ref secret_value,
            } => {
                let config = Config::new(
                    path.to_string(),
                    (secret_key.to_string(), secret_value.to_string()),
                );
                Box::new(SimplexAcceptor::new(config))
            }
            AcceptorConfig::Http { addr: _ } => Box::new(HttpAcceptor {}),
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

async fn get_rule_factory(
    geoip_config: &Option<Source>,
    connectors: &HashMap<String, Box<ConnectorConfig>>,
    rules: &[RuleEntry],
) -> Result<BoxedConnectorFactory> {
    async fn get_connector(
        connectors: &HashMap<String, Box<ConnectorConfig>>,
        ind: &str,
    ) -> Result<BoxedConnectorFactory> {
        let config = connectors
            .get(ind)
            .ok_or_else(|| anyhow::anyhow!("Failed to find connector named {}", ind))?;

        config.get_factory().await
    }

    let mut geo_ip_builder = geoip_config
        .as_ref()
        .map(|s| GeoIpBuilder::new(s.to_owned()));
    let mut connector_rules: Vec<Box<dyn Rule>> = Vec::new();
    for entry in rules.iter() {
        let rule: Box<dyn Rule> = match entry {
            RuleEntry::All(ind) => Box::new(AllRule::new(get_connector(connectors, ind).await?)),
            RuleEntry::DnsFail(ind) => {
                Box::new(DnsFailRule::new(get_connector(connectors, ind).await?))
            }
            RuleEntry::Domain { modes, index } => Box::new(DomainRule::new(
                modes.clone(),
                get_connector(connectors, index).await?,
            )),
            RuleEntry::GeoIp {
                country,
                equal,
                index,
            } => Box::new(GeoRule::new(
                get_connector(connectors, index).await?,
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
                get_connector(connectors, index).await?,
            )),
        };

        connector_rules.push(rule);
    }

    Ok(BoxedConnectorFactory::new(RuleConnectorFactory::new(
        Arc::new(connector_rules),
    )))
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
}

impl ConnectorConfig {
    #[async_recursion::async_recursion]
    pub async fn get_factory(&self) -> Result<BoxedConnectorFactory> {
        match self {
            ConnectorConfig::Direct => Ok(BoxedConnectorFactory::new(TcpConnectorFactory {})),
            ConnectorConfig::Simplex {
                endpoint,
                path,
                secret_key,
                secret_value,
                next,
            } => Ok(BoxedConnectorFactory::new(SimplexConnectorFactory::new(
                next.get_factory().await?,
                endpoint.clone(),
                Config::new(
                    path.to_owned(),
                    (secret_key.to_owned(), secret_value.to_owned()),
                ),
            ))),
            ConnectorConfig::Tls(c) => Ok(BoxedConnectorFactory::new(TlsConnectorFactory::new(
                c.get_factory().await?,
            ))),
            ConnectorConfig::Rule {
                geoip,
                connectors,
                rules,
            } => get_rule_factory(geoip, connectors, rules).await,
            ConnectorConfig::Speed(c) => {
                Ok(BoxedConnectorFactory::new(SpeedConnectorFactory::new(
                    futures::stream::iter(c.iter())
                        .then(|c| async move { Ok::<_, Error>((c.0, c.1.get_factory().await?)) })
                        .try_collect()
                        .await?,
                )))
            }
            ConnectorConfig::Http { endpoint, next } => Ok(BoxedConnectorFactory::new(
                HttpConnectorFactory::new(next.get_factory().await?, endpoint.clone()),
            )),
            ConnectorConfig::Socks5 { endpoint, next } => Ok(BoxedConnectorFactory::new(
                Socks5ConnectorFactory::new(next.get_factory().await?, endpoint.clone()),
            )),
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

        let factory_result = config.connector.get_factory().await;

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

        let license = env::var("MAXMINDDB_LICENSE")?;
        let content = content.replace("$$LICENSE$$", &license);

        test_config_file(&content, success).await
    }
}
