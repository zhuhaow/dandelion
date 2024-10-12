use crate::{
    core::{
        connector::{
            block::connect as block_connect,
            http::connect as http_connect,
            quic::{connect as quic_connect, create_quic_connection, QuicConnection},
            simplex::connect as simplex_connect,
            socks5::connect as socks5_connect,
            tcp::connect as tcp_connect,
            tls::connect as tls_connect,
        },
        endpoint::Endpoint,
        io::Io,
        simplex::Config,
    },
    Result,
};
use rune::{runtime::Ref, Any, Module, Value};
use std::{fmt::Debug, net::IpAddr, sync::Arc};

use crate::config::{engine::resolver::ResolverWrapper, rune::create_wrapper};

create_wrapper!(IoWrapper, Io, Box);
create_wrapper!(QuicConnectionWrapper, Arc<QuicConnection>);

#[derive(Debug, Any)]
pub struct ConnectRequest {
    endpoint: Endpoint,
}

impl ConnectRequest {
    pub fn new(endpoint: Endpoint) -> Self {
        Self { endpoint }
    }
}

#[rune::function]
pub async fn new_tcp(endpoint: Ref<str>, resolver: ResolverWrapper) -> Result<IoWrapper> {
    Ok(tcp_connect(&endpoint.parse()?, resolver.into_inner())
        .await?
        .into())
}

#[rune::function]
pub async fn new_quic_connection(
    server: Ref<str>,
    resolver: ResolverWrapper,
    alpn: Value,
) -> Result<QuicConnectionWrapper> {
    let alpn_vec: Vec<String> = rune::from_value(alpn)?;

    Ok(Arc::new(
        create_quic_connection(
            server.parse()?,
            resolver.into_inner(),
            alpn_vec.into_iter().map(|x| x.into_bytes()).collect(),
        )
        .await?,
    )
    .into())
}

#[rune::function]
pub async fn new_quic(connection: QuicConnectionWrapper) -> Result<IoWrapper> {
    Ok(quic_connect(connection.inner()).await?.into())
}

#[rune::function]
pub async fn new_tls(endpoint: Ref<str>, nexthop: IoWrapper) -> Result<IoWrapper> {
    Ok(tls_connect(&endpoint.parse()?, nexthop.0).await?.into())
}

#[rune::function]
pub async fn new_block(endpoint: Ref<str>) -> Result<IoWrapper> {
    match block_connect(&endpoint.parse()?).await {
        Ok(_) => unreachable!(),
        Err(e) => Err(e),
    }
}

#[rune::function]
pub async fn new_http(endpoint: Ref<str>, nexthop: IoWrapper) -> Result<IoWrapper> {
    Ok(http_connect(&endpoint.parse()?, nexthop.0).await?.into())
}

#[derive(Any)]
#[rune(constructor)]
pub struct SimplexConfig {
    pub host: String,
    pub path: String,
    pub header_name: String,
    pub header_value: String,
}

#[rune::function]
pub async fn new_simplex(
    endpoint: Ref<str>,
    config: SimplexConfig,
    nexthop: IoWrapper,
) -> Result<IoWrapper> {
    let config = Config::new(
        config.host,
        config.path,
        (config.header_name, config.header_value),
    );

    Ok(simplex_connect(&endpoint.parse()?, &config, nexthop.0)
        .await?
        .into())
}

#[rune::function]
pub async fn new_socks5(endpoint: Ref<str>, nexthop: IoWrapper) -> Result<IoWrapper> {
    Ok(socks5_connect(&endpoint.parse()?, nexthop.0).await?.into())
}

impl ConnectRequest {
    #[rune::function]
    pub fn port(&self) -> u16 {
        self.endpoint.port()
    }

    #[rune::function]
    pub fn hostname(&self) -> String {
        self.endpoint.hostname()
    }

    #[rune::function]
    pub fn endpoint(&self) -> String {
        self.endpoint.to_string()
    }

    #[rune::function]
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
        module.ty::<SimplexConfig>()?;

        module.function_meta(new_tcp)?;
        module.function_meta(new_tls)?;
        module.function_meta(new_block)?;
        module.function_meta(new_http)?;
        module.function_meta(new_simplex)?;
        module.function_meta(new_socks5)?;

        module.function_meta(new_quic_connection)?;
        module.function_meta(new_quic)?;

        module.function_meta(Self::port)?;
        module.function_meta(Self::hostname)?;
        module.function_meta(Self::endpoint)?;
        module.function_meta(Self::hostname_is_ip)?;

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

        Ok(Vm::new(Arc::new(context.runtime()?), Arc::new(result?)))
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
        )?)?;

        let mut vm = get_vm(&mut sources)?;

        let request = ConnectRequest::new(endpoint);

        let output = T::from_value(vm.call(["main"], (request,))?).into_result()?;

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
