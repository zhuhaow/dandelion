use std::{net::IpAddr, sync::Arc};

use crate::Result;
use ipnetwork::IpNetwork;
use rune::{runtime::Vec as RuneVec, Any, FromValue, Module, Value};

use crate::config::rune::create_wrapper;

create_wrapper!(IpNetworkSetWrapper, Arc<Vec<IpNetwork>>);

#[rune::function]
pub fn new_iplist(ips: RuneVec) -> Result<IpNetworkSetWrapper> {
    Ok(Arc::new(
        ips.into_iter()
            .map(|ip| anyhow::Ok(String::from_value(ip).into_result()?.parse()?))
            .try_fold(Vec::new(), |mut ips, ip| {
                ips.push(ip?);
                anyhow::Ok(ips)
            })?,
    )
    .into())
}

impl IpNetworkSetWrapper {
    fn contains_impl(&self, ip: &str) -> Result<bool> {
        let ip: IpAddr = ip.parse()?;

        Ok(self.inner().iter().any(|network| network.contains(ip)))
    }

    #[rune::function]
    pub fn contains(&self, ip: &str) -> Result<bool> {
        self.contains_impl(ip)
    }

    #[rune::function]
    pub fn contains_any(&self, ips: &RuneVec) -> Result<bool> {
        for ip in ips {
            let result = match ip {
                Value::String(s) => self.contains_impl(s.borrow_ref()?.as_str()),
                _ => anyhow::bail!("not a string"),
            }?;

            if result {
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub fn module() -> Result<Module> {
        let mut module = Module::new();

        module.ty::<Self>()?;
        module.function_meta(new_iplist)?;
        module.function_meta(Self::contains)?;
        module.function_meta(Self::contains_any)?;

        Ok(module)
    }
}
