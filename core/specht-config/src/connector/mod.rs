mod resolver;

use rune::{Any, Module};
use specht_core::{
    connector::{
        block::connect as block_connect, http::connect as http_connect,
        simplex::connect as simplex_connect, socks5::connect as socks5_connect,
        tcp::connect as tcp_connect, tls::connect as tls_connect,
    },
    endpoint::Endpoint,
    io::Io,
    simplex::Config,
    Result,
};
use std::{fmt::Debug, net::IpAddr};

pub use resolver::ResolverGroup;
use resolver::{IpSet, ResolverNotFound};

#[derive(Debug, Any)]
pub struct Connector {
    resolver_group: ResolverGroup,
    endpoint: Endpoint,
}

// We use this wrapper to derive `Any` so we can use it in the VM.
#[derive(Any)]
pub struct IoWrapper(Box<dyn Io>);

impl<T: Io> From<T> for IoWrapper {
    fn from(io: T) -> Self {
        Self(Box::new(io))
    }
}

impl IoWrapper {
    pub fn into_inner(self) -> Box<dyn Io> {
        self.0
    }
}

impl Connector {
    pub fn new(endpoint: Endpoint, resolver_group: ResolverGroup) -> Self {
        Self {
            resolver_group,
            endpoint,
        }
    }
}

impl Connector {
    pub async fn new_tcp(&self, endpoint: &str, resolver: &str) -> Result<IoWrapper> {
        Ok(tcp_connect(
            &endpoint.parse()?,
            self.resolver_group.get_resolver(resolver)?,
        )
        .await?
        .into())
    }

    pub async fn new_tls(&self, endpoint: &str, nexthop: IoWrapper) -> Result<IoWrapper> {
        Ok(tls_connect(&endpoint.parse()?, nexthop.0).await?.into())
    }

    pub async fn new_block(&self, endpoint: &str) -> Result<IoWrapper> {
        match block_connect(&endpoint.parse()?).await {
            Ok(_) => unreachable!(),
            Err(e) => Err(e),
        }
    }

    pub async fn new_http(&self, endpoint: &str, nexthop: IoWrapper) -> Result<IoWrapper> {
        Ok(http_connect(&endpoint.parse()?, nexthop.0).await?.into())
    }

    pub async fn new_simplex(
        &self,
        endpoint: &str,
        host: &str,
        path: &str,
        header_name: &str,
        header_value: &str,
        nexthop: IoWrapper,
    ) -> Result<IoWrapper> {
        let config = Config::new(
            path.to_owned(),
            (header_name.to_owned(), header_value.to_owned()),
        );

        Ok(
            simplex_connect(&endpoint.parse()?, host, &config, nexthop.0)
                .await?
                .into(),
        )
    }

    pub async fn new_socks5(&self, endpoint: &str, nexthop: IoWrapper) -> Result<IoWrapper> {
        Ok(socks5_connect(&endpoint.parse()?, nexthop.0).await?.into())
    }
}

impl Connector {
    pub fn port(&self) -> u16 {
        self.endpoint.port()
    }

    pub fn hostname(&self) -> String {
        self.endpoint.hostname()
    }

    pub fn endpoint(&self) -> String {
        self.endpoint.to_string()
    }

    pub fn hostname_is_ip(&self) -> bool {
        self.hostname_as_ip().is_some()
    }

    fn hostname_as_ip(&self) -> Option<IpAddr> {
        match &self.endpoint {
            Endpoint::Addr(addr) => Some(addr.ip()),
            Endpoint::Domain(domain, _) => domain.parse::<IpAddr>().ok(),
        }
    }

    pub async fn resolve(&self, resolver_name: &str) -> Result<IpSet> {
        match self.hostname_as_ip() {
            Some(ip) => Ok(vec![ip].into()),
            None => {
                self.resolver_group
                    .resolve(resolver_name, self.hostname().as_str())
                    .await
            }
        }
    }
}

impl Connector {
    pub fn module() -> Result<Module> {
        let mut module = Module::default();

        module.ty::<Self>()?;
        module.ty::<IoWrapper>()?;
        module.ty::<ResolverNotFound>()?;

        module.async_inst_fn("new_tcp", Self::new_tcp)?;
        module.async_inst_fn("new_tls", Self::new_tls)?;
        module.async_inst_fn("new_block", Self::new_block)?;
        module.async_inst_fn("new_http", Self::new_http)?;
        module.async_inst_fn("new_simplex", Self::new_simplex)?;
        module.async_inst_fn("new_socks5", Self::new_socks5)?;

        module.async_inst_fn("resolve", Self::resolve)?;

        module.inst_fn("port", Self::port)?;
        module.inst_fn("hostname", Self::hostname)?;
        module.inst_fn("endpoint", Self::endpoint)?;
        module.inst_fn("hostname_is_ip", Self::hostname_is_ip)?;

        Ok(module)
    }
}

#[cfg(test)]
mod tests {
    use std::{str::FromStr, sync::Arc};

    use rstest::rstest;
    use rune::{
        termcolor::{ColorChoice, StandardStream},
        Context, Diagnostics, FromValue, Source, Sources, Vm,
    };
    use specht_core::resolver::system::SystemResolver;

    use super::*;

    fn get_vm(sources: &mut Sources) -> Result<Vm> {
        let mut context = Context::with_default_modules()?;
        context.install(&Connector::module()?)?;
        context.install(&IpSet::module()?)?;

        let mut diagnostics = Diagnostics::new();
        let result = rune::prepare(sources)
            .with_context(&context)
            .with_diagnostics(&mut diagnostics)
            .build();

        if !diagnostics.is_empty() {
            let mut writer = StandardStream::stderr(ColorChoice::Always);
            diagnostics.emit(&mut writer, sources)?;
        }

        Ok(Vm::new(Arc::new(context.runtime()), Arc::new(result?)))
    }

    fn test_request<T: FromValue>(method_name: &str, endpoint: Endpoint) -> Result<T> {
        let mut sources = Sources::new();

        sources.insert(Source::new(
            "entry",
            format!(
                "
        pub fn main(connector) {{
            connector.{}()
        }}
        ",
                method_name,
            ),
        ));

        let mut vm = get_vm(&mut sources)?;

        let connector = Connector::new(endpoint, ResolverGroup::default());

        let output = T::from_value(vm.call(["main"], (connector,))?)?;

        Ok(output)
    }

    #[rstest]
    #[case("127.0.0.1:80", 80)]
    #[case("[::1]:80", 80)]
    #[case("example.com:80", 80)]
    fn test_connect_request_port(#[case] endpoint: &str, #[case] port: u16) -> Result<()> {
        let request = Endpoint::from_str(endpoint)?;

        assert_eq!(test_request::<u16>("port", request)?, port);

        Ok(())
    }

    #[rstest]
    #[case("127.0.0.1:80", "127.0.0.1")]
    #[case("[::1]:80", "::1")]
    #[case("example.com:80", "example.com")]
    fn test_connect_request_hostname(#[case] endpoint: &str, #[case] hostname: &str) -> Result<()> {
        let request = Endpoint::from_str(endpoint)?;

        assert_eq!(test_request::<String>("hostname", request)?, hostname);

        Ok(())
    }

    #[rstest]
    #[case("127.0.0.1:80", "127.0.0.1:80")]
    #[case("[::1]:80", "[::1]:80")]
    #[case("example.com:80", "example.com:80")]
    fn test_connect_request_endpoint(
        #[case] endpoint: &str,
        #[case] expect_endpoint: &str,
    ) -> Result<()> {
        let request = Endpoint::from_str(endpoint)?;

        assert_eq!(
            test_request::<String>("endpoint", request)?,
            expect_endpoint
        );

        Ok(())
    }

    #[rstest]
    #[case("127.0.0.1:80", true)]
    #[case("[::1]:80", true)]
    #[case("example.com:80", false)]
    fn test_connect_request_host_is_ip(#[case] endpoint: &str, #[case] is_ip: bool) -> Result<()> {
        let request = Endpoint::from_str(endpoint)?;

        assert_eq!(test_request::<bool>("hostname_is_ip", request)?, is_ip);

        Ok(())
    }

    #[rstest]
    #[case("127.0.0.1:80", Ok((vec!["127.0.0.1".parse().unwrap()]).into()))]
    #[case("[::1]:80", Ok((vec!["::1".parse().unwrap()]).into()))]
    #[case("example.com:80", Err(ResolverNotFound{name: "nothing".to_string()}))]
    #[tokio::test]
    async fn test_connect_request_resolve(
        #[case] endpoint: &str,
        #[case] expect: Result<IpSet, ResolverNotFound>,
    ) -> Result<()> {
        let mut sources = Sources::new();

        sources.insert(Source::new(
            "entry",
            "
            pub async fn main(connector, name) {{
                Ok(connector.resolve(name).await?)
            }}
            ",
        ));

        let mut vm = get_vm(&mut sources)?;

        let mut resolver_group = ResolverGroup::default();
        resolver_group.add_resolver("system", Arc::new(SystemResolver::default()));

        let connector = Connector::new(Endpoint::from_str(endpoint)?, resolver_group);

        let output = vm
            .async_call(["main"], (connector, "nothing"))
            .await?
            .into_result()?
            .take()
            .unwrap()
            .map(|v| IpSet::from_value(v).unwrap())
            .map_err(|v| {
                anyhow::Error::from_value(v)
                    .unwrap()
                    .downcast::<ResolverNotFound>()
                    .unwrap()
            });

        assert_eq!(output, expect);

        let mut resolver_group = ResolverGroup::default();
        resolver_group.add_resolver("system", Arc::new(SystemResolver::default()));

        let connector = Connector::new(Endpoint::from_str(endpoint)?, resolver_group);

        let output = vm
            .async_call(["main"], (connector, "system"))
            .await?
            .into_result()?
            .take()
            .unwrap()
            .map(|v| IpSet::from_value(v).unwrap())
            .map_err(|v| {
                anyhow::Error::from_value(v)
                    .unwrap()
                    .downcast::<ResolverNotFound>()
                    .unwrap()
            });

        assert!(output.is_ok());

        Ok(())
    }
}
