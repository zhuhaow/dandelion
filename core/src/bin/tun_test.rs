use specht2_core::Result;

#[cfg(target_os = "macos")]
#[tokio::main]
async fn main() -> Result<()> {
    use log::info;
    use specht2_core::tun::device::Device;
    use tokio::signal::ctrl_c;

    flexi_logger::Logger::try_with_env()
        .unwrap()
        .start()
        .unwrap();

    let _device = Device::create("10.128.0.1/12".parse().unwrap())?;

    info!("Device created, waiting for ^+C...");

    ctrl_c().await?;

    Ok(())
}

#[cfg(not(target_os = "macos"))]
#[tokio::main]
async fn main() -> Result<()> {
    use anyhow::bail;

    bail!("Not supported platform");
}
