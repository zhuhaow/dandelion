use dandelion_core::{
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
use rune::{Any, Module};
use std::{fmt::Debug, net::IpAddr};

use crate::{engine::resolver::ResolverWrapper, rune::create_wrapper};

create_wrapper!(IoWrapper, Io, Box);

#[derive(Debug, Any)]
pub struct ConnectRequest {
    endpoint: Endpoint,
}

impl ConnectRequest {
    pub fn new(endpoint: Endpoint) -> Self {
        Self { endpoint }
    }
}

pub async fn new_tcp(endpoint: &str, resolver: ResolverWrapper) -> Result<IoWrapper> {
    Ok(tcp_connect(&endpoint.parse()?, resolver.into_inner())
        .await?
        .into())
}

pub async fn new_tls(endpoint: &str, nexthop: IoWrapper) -> Result<IoWrapper> {
    Ok(tls_connect(&endpoint.parse()?, nexthop.0).await?.into())
}

pub async fn new_block(endpoint: &str) -> Result<IoWrapper> {
    match block_connect(&endpoint.parse()?).await {
        Ok(_) => unreachable!(),
        Err(e) => Err(e),
    }
}

pub async fn new_http(endpoint: &str, nexthop: IoWrapper) -> Result<IoWrapper> {
    Ok(http_connect(&endpoint.parse()?, nexthop.0).await?.into())
}

pub async fn new_simplex(
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

pub async fn new_socks5(endpoint: &str, nexthop: IoWrapper) -> Result<IoWrapper> {
    Ok(socks5_connect(&endpoint.parse()?, nexthop.0).await?.into())
}

impl ConnectRequest {
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

    fn hostname_as_ip(&self) -> Option<String> {
        match &self.endpoint {
            Endpoint::Addr(addr) => Some(addr.ip().to_string()),
            Endpoint::Domain(domain, _) => domain.parse::<IpAddr>().ok().map(|ip| ip.to_string()),
        }
    }
}

impl ConnectRequest {
    pub fn module() -> Result<Module> {
        let mut module = Module::default();

        module.ty::<Self>()?;
        module.ty::<IoWrapper>()?;

        module.async_function(["try_new_tcp_async"], new_tcp)?;
        module.async_function(["try_new_tls_async"], new_tls)?;
        module.async_function(["try_new_block_async"], new_block)?;
        module.async_function(["try_new_http_async"], new_http)?;
        module.async_function(["try_new_simplex_async"], new_simplex)?;
        module.async_function(["try_new_socks5_async"], new_socks5)?;

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

    use super::*;

    fn get_vm(sources: &mut Sources) -> Result<Vm> {
        let mut context = Context::with_default_modules()?;
        context.install(ConnectRequest::module()?)?;

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
        pub fn main(request) {{
            request.{}()
        }}
        ",
                method_name,
            ),
        ));

        let mut vm = get_vm(&mut sources)?;

        let request = ConnectRequest::new(endpoint);

        let output = T::from_value(vm.call(["main"], (request,))?)?;

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
}
