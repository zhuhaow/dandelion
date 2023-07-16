use std::{net::IpAddr, sync::Arc};

use ipnetwork::IpNetwork;
use rune::{runtime::Vec as RuneVec, Any, FromValue, Module};
use specht_core::Result;

use crate::rune::create_wrapper;

create_wrapper!(IpNetworkSetWrapper, Arc<Vec<IpNetwork>>);

impl IpNetworkSetWrapper {
    pub fn new(ips: RuneVec) -> Result<Self> {
        Ok(Arc::new(
            ips.into_iter()
                .map(|ip| anyhow::Ok(String::from_value(ip)?.parse()?))
                .try_collect::<Vec<IpNetwork>>()?,
        )
        .into())
    }

    pub fn contains(&self, ip: &str) -> Result<bool> {
        let ip: IpAddr = ip.parse()?;

        Ok(self.inner().iter().any(|network| network.contains(ip)))
    }

    pub fn contains_any(&self, ips: RuneVec) -> Result<bool> {
        for ip in ips {
            if self.contains(&String::from_value(ip)?)? {
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub fn module() -> Result<Module> {
        let mut module = Module::new();

        module.ty::<Self>()?;
        module.function(&["try_create_iplist"], Self::new)?;
        module.inst_fn("try_contains", Self::contains)?;
        module.inst_fn("try_contains_any", Self::contains_any)?;

        Ok(module)
    }
}
