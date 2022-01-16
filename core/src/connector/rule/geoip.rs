use super::Rule;
use crate::{connector::BoxedConnector, endpoint::Endpoint, resolver::Resolver};
use iso3166_1::CountryCode;
use maxminddb::{geoip2::Country, MaxMindDBError, Reader};
use memmap2::Mmap;
use std::{net::IpAddr, sync::Arc};

/// GeoRule matches the geo location of the endpoint based on IP address.
///
/// It only matches when the IP addresses can be obtained either they are
/// provided directly or we can resolve the host name successfully.
pub struct GeoRule<R: Resolver> {
    connector: BoxedConnector,
    reader: Arc<Reader<Mmap>>,
    country: Option<CountryCode<'static>>,
    equal: bool,
    resolver: R,
}

impl<R: Resolver> GeoRule<R> {
    pub fn new(
        connector: BoxedConnector,
        reader: Arc<Reader<Mmap>>,
        country: Option<CountryCode<'static>>,
        equal: bool,
        resolver: R,
    ) -> Self {
        Self {
            connector,
            reader,
            country,
            equal,
            resolver,
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
impl<R: Resolver> Rule for GeoRule<R> {
    async fn check(&self, endpoint: &Endpoint) -> Option<&BoxedConnector> {
        match endpoint {
            Endpoint::Addr(addr) => {
                if self.match_ip(&addr.ip()) == Some(self.equal) {
                    return Some(&self.connector);
                }
            }
            Endpoint::Domain(host, _) => {
                let ips = self.resolver.lookup_ip(host.as_str()).await.ok()?;
                for ip in ips {
                    if self.match_ip(&ip) == Some(self.equal) {
                        return Some(&self.connector);
                    }
                }
            }
        };

        None
    }
}
