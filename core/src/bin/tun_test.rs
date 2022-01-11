use specht2_core::Result;

#[cfg(target_os = "macos")]
#[tokio::main]
async fn main() -> Result<()> {
    use ipnetwork::Ipv4Network;
    use specht2_core::tun::{device::Device, stack::create_stack};

    flexi_logger::Logger::try_with_env()
        .unwrap()
        .start()
        .unwrap();

    let device = Device::new("10.128.0.1/12".parse().unwrap())?;

    let ip_block: Ipv4Network = "10.128.0.1/12".parse().unwrap();

    let stack = create_stack(device, ip_block, "127.0.0.1:8091".parse().unwrap()).await?;

    stack.0.await;

    Ok(())
}

#[cfg(not(target_os = "macos"))]
#[tokio::main]
async fn main() -> Result<()> {
    use anyhow::bail;

    bail!("Not supported platform");
}
