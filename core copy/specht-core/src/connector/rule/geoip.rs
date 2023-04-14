use super::Rule;
use crate::{connector::Connector, endpoint::Endpoint, resolver::Resolver};
use iso3166_1::CountryCode;
use maxminddb::{geoip2::Country, MaxMindDBError, Mmap, Reader};
use std::{net::IpAddr, sync::Arc};
use tracing::{debug, warn};

/// GeoRule matches the geo location of the endpoint based on IP address.
///
/// It only matches when the IP addresses can be obtained either they are
/// provided directly or we can resolve the host name successfully.
pub struct GeoRule<R: Resolver, C: Connector> {
    connector: C,
    reader: Arc<Reader<Mmap>>,
    country: Option<CountryCode<'static>>,
    equal: bool,
    resolver: R,
}

impl<R: Resolver, C: Connector> GeoRule<R, C> {
    pub fn new(
        connector: C,
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
        let result: Option<&str> = match self.look_up_country(addr) {
            Ok(c) => c,
            Err(err) => {
                if matches!(err, MaxMindDBError::AddressNotFoundError(_)) {
                    debug!(
                        "Cannot find geo information for ip {}, error: {}",
                        addr, err
                    );
                    None
                } else {
                    warn!(
                        "Failed to look up geo information for ip {}, error: {}",
                        addr, err
                    );
                    return None;
                }
            }
        };

        Some(result == self.country.as_ref().map(|c| c.alpha2))
    }

    fn look_up_country(&self, addr: &IpAddr) -> std::result::Result<Option<&str>, MaxMindDBError> {
        self.reader
            .lookup(*addr)
            .map(|c: Country| c.country.and_then(|c| c.iso_code))
            .map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl<R: Resolver, C: Connector> Rule<C> for GeoRule<R, C> {
    async fn check(&self, endpoint: &Endpoint) -> Option<&C> {
        match endpoint {
            Endpoint::Addr(addr) => {
                if self.match_ip(&addr.ip()) == Some(self.equal) {
                    debug!(
                        "Matched ip {} with geo: {:#?}, equal: {}",
                        addr,
                        self.country.as_ref().map(|c| c.name),
                        self.equal
                    );
                    return Some(&self.connector);
                }
                debug!(
                    "Didn't match ip {} with geo: {:#?}, equal: {}. The ip look up result: {:#?}",
                    addr,
                    self.country.as_ref().map(|c| c.name),
                    self.equal,
                    self.look_up_country(&addr.ip()),
                );
            }
            Endpoint::Domain(host, _) => {
                let ips = self.resolver.lookup_ip(host.as_str()).await.ok()?;
                debug!("Domain {} resolved to {:#?}", host, ips);
                for ip in ips {
                    if self.match_ip(&ip) == Some(self.equal) {
                        debug!("Matched ip {} with geo {}", ip, self.equal);
                        return Some(&self.connector);
                    }
                    debug!(
                    "Didn't match ip {} with geo: {:#?}, equal: {}. The ip look up result: {:#?}",
                    ip,
                    self.country.as_ref().map(|c| c.name),
                    self.equal,
                    self.look_up_country(&ip),
                );
                }
            }
        };

        None
    }
}
