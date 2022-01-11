use specht2_core::Result;

#[cfg(target_os = "macos")]
#[tokio::main]
async fn main() -> Result<()> {
    use std::{sync::Arc, time::Duration};

    use ipnetwork::Ipv4Network;
    use specht2_core::tun::{device::Device, dns::FakeDns, stack::run_stack, TranslatorConfig};

    flexi_logger::Logger::try_with_env()
        .unwrap()
        .start()
        .unwrap();

    let device = Device::new("10.128.0.1/12".parse().unwrap())?;

    let ip_block: Ipv4Network = "10.128.0.1/12".parse().unwrap();
    let mut ip_iter = ip_block.into_iter();

    let fake_ips = (&mut ip_iter)
        .take(10)
        .map(|ip| match ip {
            std::net::IpAddr::V4(ip) => ip,
            std::net::IpAddr::V6(_) => unreachable!(),
        })
        .collect();
    let fake_ports = 1024..65535;
    let translator_config = TranslatorConfig::new(
        "127.0.0.1:8091".parse().unwrap(),
        fake_ips,
        fake_ports,
        Duration::from_secs(180),
    );

    run_stack(
        device,
        Arc::new(FakeDns::new("8.8.8.8:53".parse().unwrap(), ip_iter).await?),
        "10.128.0.1:53".parse().unwrap(),
        translator_config,
        1500,
    )
    .await?;

    Ok(())
}

#[cfg(not(target_os = "macos"))]
#[tokio::main]
async fn main() -> Result<()> {
    use anyhow::bail;

    bail!("Not supported platform");
}
