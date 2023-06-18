use std::{collections::HashMap, error::Error, fmt::Display, net::IpAddr, sync::Arc};

use rune::{Any, Module};
use specht_core::resolver::Resolver;
use specht_core::Result;

#[derive(Any, Debug, PartialEq)]
pub struct IpSet {
    ips: Vec<IpAddr>,
}

impl From<Vec<IpAddr>> for IpSet {
    fn from(ips: Vec<IpAddr>) -> Self {
        Self { ips }
    }
}

impl IpSet {
    pub fn module() -> Result<Module> {
        let mut module = Module::default();

        module.ty::<IpSet>()?;

        Ok(module)
    }
}

#[derive(Debug, PartialEq, Any)]
pub struct ResolverNotFound {
    pub name: String,
}

impl Display for ResolverNotFound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "resolver {} not found", self.name)
    }
}

impl Error for ResolverNotFound {}

pub enum DnsType {
    A,
    AAAA,
    Any,
}

// TODO: Cache the response
#[derive(Default, Debug)]
pub struct ResolverGroup {
    resolvers: HashMap<String, Arc<dyn Resolver + Sync>>,
}

impl ResolverGroup {
    pub fn new() -> Self {
        Self {
            resolvers: HashMap::new(),
        }
    }

    pub fn add_resolver(&mut self, name: &str, resolver: Arc<dyn Resolver + Sync>) {
        self.resolvers.insert(name.to_owned(), resolver);
    }

    pub fn get_resolver(&self, name: &str) -> Result<Arc<dyn Resolver + Sync>> {
        match self.resolvers.get(name) {
            Some(resolver) => Ok(resolver.clone()),
            None => Err(ResolverNotFound {
                name: name.to_owned(),
            })?,
        }
    }
}

impl ResolverGroup {
    pub async fn resolve(&self, name: &str, hostname: &str) -> Result<IpSet> {
        Ok(self.get_resolver(name)?.lookup_ip(hostname).await?.into())
    }
}
