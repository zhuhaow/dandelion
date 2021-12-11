use super::Rule;
use crate::{
    connector::{
        boxed::{BoxedConnector, BoxedConnectorFactory},
        ConnectorFactory,
    },
    endpoint::Endpoint,
};
use iso3166_1::CountryCode;
use maxminddb::{geoip2::Country, MaxMindDBError, Reader};
use memmap2::Mmap;
use std::{net::IpAddr, sync::Arc};
use tokio::net::lookup_host;

/// GeoRule matches the geo location of the endpoint based on IP address.
///
/// It only matches when the IP addresses can be obtained either they are
/// provided directly or we can resolve the host name successfully.
pub struct GeoRule {
    factory: BoxedConnectorFactory,
    reader: Arc<Reader<Mmap>>,
    country: Option<CountryCode<'static>>,
    equal: bool,
}

impl GeoRule {
    pub fn new(
        factory: BoxedConnectorFactory,
        reader: Arc<Reader<Mmap>>,
        country: Option<CountryCode<'static>>,
        equal: bool,
    ) -> Self {
        Self {
            factory,
            reader,
            country,
            equal,
        }
    }

    /// Returns None if there is error when we try to find the geo location of the ip.
    fn match_ip(&self, addr: &IpAddr) -> Option<bool> {
        let result: Option<Country> = match self.reader.lookup(*addr) {
            Ok(c) => Some(c),
            Err(err) => {
                if matches!(err, MaxMindDBError::AddressNotFoundError(_)) {
                    None
                } else {
                    return None;
                }
            }
        };

        Some(
            result.and_then(|c| c.country.and_then(|c| c.iso_code))
                == self.country.as_ref().map(|c| c.alpha2),
        )
    }
}

#[async_trait::async_trait]
impl Rule for GeoRule {
    async fn check(&self, endpoint: &Endpoint) -> Option<BoxedConnector> {
        match endpoint {
            Endpoint::Addr(addr) => {
                if self.match_ip(&addr.ip()) == Some(self.equal) {
                    return Some(self.factory.build());
                }
            }
            Endpoint::Domain(host, port) => {
                let addrs = lookup_host((host.as_str(), *port)).await.ok()?;
                for addr in addrs {
                    if self.match_ip(&addr.ip()) == Some(self.equal) {
                        return Some(self.factory.build());
                    }
                }
            }
        };

        None
    }
}
