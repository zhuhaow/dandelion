use crate::config::rune::create_wrapper;
use crate::Result;
use ipnetwork::IpNetwork;
use rune::{runtime::Vec as RuneVec, Any, FromValue, Module};
use std::{net::IpAddr, rc::Rc};

create_wrapper!(IpNetworkSetWrapper, Rc<Vec<IpNetwork>>);

#[rune::function]
pub fn new_iplist(ips: RuneVec) -> Result<IpNetworkSetWrapper> {
    Ok(Rc::new(
        ips.into_iter()
            .map(|ip| anyhow::Ok(String::from_value(ip)?.parse()?))
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
            let result = self.contains_impl(ip.borrow_string_ref()?.as_ref())?;

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

#[cfg(test)]
mod tests {
    use crate::config::engine::testing;

    use super::*;
    use rstest::*;

    #[rstest]
    #[case("10.0.0.0/8", "10.0.0.1", true)]
    #[case("10.0.0.0/8", "12.0.0.1", false)]
    #[case("2001:db8::/32", "2001:db8::1", true)]
    #[case("2001:db8::/32", "2001:db9::1", false)]
    #[case("127.0.0.1", "127.0.0.1", true)]
    #[case("127.0.0.1", "127.0.0.2", false)]
    #[case("::1/128", "::1", true)]
    #[case("::1/128", "::2", false)]
    #[tokio::test]
    async fn test_iplist(
        #[case] ipnetwork_str: &str,
        #[case] ip_str: &str,
        #[case] expected: bool,
    ) -> Result<()> {
        let result: bool = testing::run(
            vec![IpNetworkSetWrapper::module()?],
            &format!(
                r#"
                let iplist = new_iplist(["{ipnetwork_str}"])?;
                iplist.contains("{ip_str}")?
                "#,
            ),
            ((),),
        )
        .await?;

        assert_eq!(result, expected);

        Ok(())
    }
}
