use specht2_core::Result;

#[cfg(target_os = "macos")]
#[tokio::main]
async fn main() -> Result<()> {
    use specht2_core::tun::{device::create_as_device, dns::TunDns, stack::run_stack};

    flexi_logger::Logger::try_with_env()
        .unwrap()
        .start()
        .unwrap();

    let device = create_as_device("10.128.0.1/12".parse().unwrap(), None)?;

    let dns_server = TunDns::new(
        "10.128.0.1:53".parse().unwrap(),
        "10.128.0.1/12".parse().unwrap(),
    )
    .await?;

    run_stack(device, dns_server, "10.128.0.1:53".parse().unwrap()).await?;

    Ok(())
}

#[cfg(not(target_os = "macos"))]
#[tokio::main]
async fn main() -> Result<()> {
    use anyhow::bail;

    bail!("Not supported platform");
}
