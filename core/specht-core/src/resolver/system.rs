use super::Resolver;
use crate::Result;
use dns_lookup::{getaddrinfo, lookup_host, AddrFamily, AddrInfoHints, SockType};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

#[derive(Debug, Default)]
pub struct SystemResolver {}

#[async_trait::async_trait]
impl Resolver for SystemResolver {
    async fn lookup_ip(&self, name: &str) -> Result<Vec<IpAddr>> {
        let name = name.to_owned();
        Ok(tokio::task::spawn_blocking(move || lookup_host(&name)).await??)
    }

    async fn lookup_ipv4(&self, name: &str) -> Result<Vec<Ipv4Addr>> {
        // We won't error out if we see an ipv6 address.
        Ok(self
            .lookup(name, AddrFamily::Inet)
            .await?
            .into_iter()
            .filter_map(|ip| match ip {
                IpAddr::V4(ip_) => Some(ip_),
                IpAddr::V6(_) => None,
            })
            .collect::<Vec<_>>())
    }

    async fn lookup_ipv6(&self, name: &str) -> Result<Vec<Ipv6Addr>> {
        Ok(self
            .lookup(name, AddrFamily::Inet6)
            .await?
            .into_iter()
            .filter_map(|ip| match ip {
                IpAddr::V4(_) => None,
                IpAddr::V6(ip_) => Some(ip_),
            })
            .collect::<Vec<_>>())
    }
}

impl SystemResolver {
    pub fn new() -> Self {
        Self {}
    }

    async fn lookup(&self, name: &str, family: AddrFamily) -> Result<Vec<IpAddr>> {
        let hints = AddrInfoHints {
            socktype: SockType::Stream.into(),
            address: family.into(),
            ..AddrInfoHints::default()
        };

        let name = name.to_owned();
        Ok(
            tokio::task::spawn_blocking(move || getaddrinfo(Some(&name), None, Some(hints)))
                .await?
                .map_err(Into::<std::io::Error>::into)?
                .filter_map(|r| r.ok())
                .map(|r| r.sockaddr.ip())
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;

    #[rstest]
    #[case("localhost")]
    #[case("google.com")]
    #[tokio::test]
    async fn test_look_up_existing_domain(#[case] host: &str) {
        let resolver = SystemResolver::new();

        let result = resolver.lookup_ip(host).await.unwrap();
        assert!(!result.is_empty());
    }

    #[rstest]
    #[case("t.test")]
    #[case("t.invalid")]
    #[tokio::test]
    async fn test_look_up_nonexisting_domain(#[case] host: &str) {
        let resolver = SystemResolver::new();

        assert!(resolver.lookup_ip(host).await.is_err());
    }

    #[rstest]
    #[case("localhost", Some("127.0.0.1"))]
    #[case("google.com", None)]
    #[tokio::test]
    async fn test_look_up_a_record(#[case] host: &str, #[case] expected: Option<&str>) {
        let resolver = SystemResolver::new();

        let result = resolver.lookup_ipv4(host).await.unwrap();
        assert!(!result.is_empty());

        if let Some(expect) = expected {
            assert!(result
                .into_iter()
                .any(|x| x == expect.parse::<Ipv4Addr>().unwrap()));
        }
    }

    #[rstest]
    #[case("t.test")]
    #[case("t.invalid")]
    #[tokio::test]
    async fn test_look_up_nonexisting_domain_for_a_record(#[case] host: &str) {
        let resolver = SystemResolver::new();

        assert!(resolver.lookup_ipv4(host).await.is_err());
    }

    // Surprisingly, we cannot use getaddrinfo to do AAAA query on Windows! And
    // it's not documented. But we can find similar issue here
    // https://stackoverflow.com/questions/66755681/getaddrinfo-c-on-windows-not-handling-ipv6-correctly-returning-error-code-1
    // and it seems there is no fix. An article suggests setting up Teredo would
    // fix this on Windows 7.
    // http://netscantools.blogspot.co.uk/2011/06/ipv6-teredo-problems-and-solutions-on.html
    // Obviously it not a solution but a bug Microsoft probably not going to
    // fix.
    //
    // But it should still be ok as long as IPv4 is still available, just we
    // won't be able to use IPv6 on Windows.
    //
    // But I'm not using Windows, so it's fine for me and I won't try to fix it
    // anymore.
    //
    // Anyway, PR is always welcomed.
    #[cfg(not(target_os = "windows"))]
    #[rstest]
    #[case("localhost", Some("::1"))]
    #[case("google.com", None)]
    #[tokio::test]
    async fn test_look_up_aaaa_record(#[case] host: &str, #[case] expected: Option<&str>) {
        let resolver = SystemResolver::new();

        let result = resolver.lookup_ipv6(host).await.unwrap();
        assert!(!result.is_empty());

        if let Some(expect) = expected {
            assert!(result
                .into_iter()
                .any(|x| x == expect.parse::<Ipv6Addr>().unwrap()));
        }
    }

    #[rstest]
    #[case("t.test")]
    #[case("t.invalid")]
    #[tokio::test]
    async fn test_look_up_nonexisting_domain_for_aaaa_record(#[case] host: &str) {
        let resolver = SystemResolver::new();

        assert!(resolver.lookup_ipv6(host).await.is_err());
    }
}
