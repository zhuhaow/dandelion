use rune::{Any, Module};
use specht_core::{endpoint::Endpoint, io::Io, Result};
use std::{collections::HashMap, error::Error, fmt::Display, net::IpAddr};

#[derive(Debug, PartialEq, Any)]
pub enum Connector {
    Tcp {
        endpoint: String,
        resolver_id: String,
    },
    Http {
        endpoint: String,
        nexthop: Box<Self>,
    },
    Socks5 {
        endpoint: String,
        nexthop: Box<Self>,
    },
    Block,
    Simplex {
        endpoint: String,
        path: String,
        secret_header: (String, String),
        nexthop: Box<Self>,
    },
    Tls {
        endpoint: String,
        nexthop: Box<Self>,
    },
    Speed {
        endpoint: String,
        nexthop_candidates: Vec<(Box<Self>, u32)>,
    },
}

impl Connector {
    pub fn tcp(endpoint: &str, resolver_id: &str) -> Connector {
        Connector::Tcp {
            endpoint: endpoint.to_owned(),
            resolver_id: resolver_id.to_owned(),
        }
    }

    pub fn http(endpoint: &str, nexthop: Connector) -> Connector {
        Connector::Http {
            endpoint: endpoint.to_owned(),
            nexthop: Box::new(nexthop),
        }
    }

    pub fn block() -> Connector {
        Connector::Block
    }

    pub fn simplex(
        endpoint: &str,
        path: &str,
        secret_header_name: &str,
        secret_header_value: &str,
        nexthop: Connector,
    ) -> Connector {
        Connector::Simplex {
            endpoint: endpoint.to_owned(),
            path: path.to_owned(),
            secret_header: (
                secret_header_name.to_owned(),
                secret_header_value.to_owned(),
            ),
            nexthop: Box::new(nexthop),
        }
    }

    pub fn socks5(endpoint: &str, nexthop: Connector) -> Connector {
        Connector::Socks5 {
            endpoint: endpoint.to_owned(),
            nexthop: Box::new(nexthop),
        }
    }

    pub fn tls(endpoint: &str, nexthop: Connector) -> Connector {
        Connector::Tls {
            endpoint: endpoint.to_owned(),
            nexthop: Box::new(nexthop),
        }
    }

    pub fn speed(endpoint: &str, nexthop_candidates: Vec<(Connector, u32)>) -> Connector {
        Connector::Speed {
            endpoint: endpoint.to_owned(),
            nexthop_candidates: nexthop_candidates
                .into_iter()
                .map(|(nexthop, weight)| (Box::new(nexthop), weight))
                .collect(),
        }
    }
}

impl Connector {
    pub async fn connect(&self) -> Result<Box<dyn Io>> {
        todo!()
    }
}

impl Connector {
    pub fn module() -> Result<Module> {
        let mut module = Module::default();

        module.ty::<Self>()?;

        module.function(["Connector", "tcp"], Self::tcp)?;
        module.function(["Connector", "http"], Self::http)?;
        module.function(["Connector", "block"], Self::block)?;
        module.function(["Connector", "simplex"], Self::simplex)?;
        module.function(["Connector", "socks5"], Self::socks5)?;
        module.function(["Connector", "tls"], Self::tls)?;
        module.function(["Connector", "speed"], Self::speed)?;

        Ok(module)
    }
}

#[derive(Debug, Any)]
pub struct ConnectRequest {
    endpoint: Endpoint,
    resolved: HashMap<String, Result<Vec<IpAddr>>>,
}

impl From<Endpoint> for ConnectRequest {
    fn from(value: Endpoint) -> Self {
        Self {
            endpoint: value,
            resolved: Default::default(),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct NotResolvedYet {
    resolver_name: String,
}

impl Display for NotResolvedYet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "`{}` is not resolved yet", self.resolver_name)
    }
}

impl Error for NotResolvedYet {}

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
        match &self.endpoint {
            Endpoint::Addr(_) => true,
            Endpoint::Domain(domain, _) => domain.parse::<IpAddr>().is_ok(),
        }
    }

    // Ensure the we have the resolve result (include error) from the given resolver
    pub fn ensure_resolved(&self, resolver_name: &str) -> Result<()> {
        match self.resolved.get(resolver_name) {
            Some(_) => Ok(()),
            None => Err(NotResolvedYet {
                resolver_name: resolver_name.to_owned(),
            })?,
        }
    }

    // No need to check this if we have already called `ensure_resolved`
    pub fn is_resolved(&self, resolver_name: &str) -> bool {
        self.resolved.contains_key(resolver_name)
    }

    pub fn add_resolve_result(&mut self, resolver_name: &str, result: Result<Vec<IpAddr>>) {
        self.resolved.insert(resolver_name.to_owned(), result);
    }
}

impl ConnectRequest {
    pub fn module() -> Result<Module> {
        let mut module = Module::new();

        module.ty::<Self>()?;
        module.inst_fn("port", Self::port)?;
        module.inst_fn("hostname", Self::hostname)?;
        module.inst_fn("endpoint", Self::endpoint)?;
        module.inst_fn("hostname_is_ip", Self::hostname_is_ip)?;
        module.inst_fn("ensure_resolved", Self::ensure_resolved)?;
        module.inst_fn("is_resolved", Self::is_resolved)?;

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
        context.install(&Connector::module()?)?;
        context.install(&ConnectRequest::module()?)?;

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

    fn test_request<T: FromValue>(
        method_name: &str,
        request: &ConnectRequest,
        params: &[&str],
    ) -> Result<T> {
        let mut sources = Sources::new();

        sources.insert(Source::new(
            "entry",
            format!(
                "
        pub fn main(request) {{
            request.{}({})
        }}
        ",
                method_name,
                params.join(", ")
            ),
        ));

        let mut vm = get_vm(&mut sources)?;

        let output = T::from_value(vm.call(["main"], (request,))?)?;

        Ok(output)
    }

    #[rstest]
    #[case("127.0.0.1:80", 80)]
    #[case("[::1]:80", 80)]
    #[case("example.com:80", 80)]
    fn test_connect_request_port(#[case] endpoint: &str, #[case] port: u16) -> Result<()> {
        let request = Endpoint::from_str(endpoint)?.into();

        assert_eq!(test_request::<u16>("port", &request, &[])?, port);

        Ok(())
    }

    #[rstest]
    #[case("127.0.0.1:80", "127.0.0.1")]
    #[case("[::1]:80", "::1")]
    #[case("example.com:80", "example.com")]
    fn test_connect_request_hostname(#[case] endpoint: &str, #[case] hostname: &str) -> Result<()> {
        let request = Endpoint::from_str(endpoint)?.into();

        assert_eq!(test_request::<String>("hostname", &request, &[])?, hostname);

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
        let request = Endpoint::from_str(endpoint)?.into();

        assert_eq!(
            test_request::<String>("endpoint", &request, &[])?,
            expect_endpoint
        );

        Ok(())
    }

    #[rstest]
    #[case("127.0.0.1:80", true)]
    #[case("[::1]:80", true)]
    #[case("example.com:80", false)]
    fn test_connect_request_host_is_ip(#[case] endpoint: &str, #[case] is_ip: bool) -> Result<()> {
        let request = Endpoint::from_str(endpoint)?.into();

        assert_eq!(
            test_request::<bool>("hostname_is_ip", &request, &[])?,
            is_ip
        );

        Ok(())
    }

    #[rstest]
    #[case("127.0.0.1:80")]
    #[case("[::1]:80")]
    #[case("example.com:80")]
    fn test_connect_request_resolve(#[case] endpoint: &str) -> Result<()> {
        let mut request = Endpoint::from_str(endpoint)?.into();

        assert!(!(test_request::<bool>("is_resolved", &request, &["\"dns\""])?));

        let result = test_request::<Result<()>>("ensure_resolved", &request, &["\"dns\""])?;

        assert_eq!(
            result.map_err(|e| e.downcast::<NotResolvedYet>().unwrap()),
            Err(NotResolvedYet {
                resolver_name: "dns".to_owned(),
            })
        );

        request.add_resolve_result("dns", Ok(vec![IpAddr::from_str("1.1.1.1").unwrap()]));

        assert!(test_request::<bool>("is_resolved", &request, &["\"dns\""])?);

        let result = test_request::<Result<()>>("ensure_resolved", &request, &["\"dns\""])?;

        assert!(result.is_ok());

        assert!(!(test_request::<bool>("is_resolved", &request, &["\"dns1\""])?));

        let result = test_request::<Result<()>>("ensure_resolved", &request, &["\"dns1\""])?;

        assert_eq!(
            result.map_err(|e| e.downcast::<NotResolvedYet>().unwrap()),
            Err(NotResolvedYet {
                resolver_name: "dns1".to_owned(),
            })
        );

        Ok(())
    }
}
