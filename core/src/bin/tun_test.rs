use log::info;
use specht2_core::{tun::device::Device, Result};
use tokio::signal::ctrl_c;

#[tokio::main]
async fn main() -> Result<()> {
    flexi_logger::Logger::try_with_env()
        .unwrap()
        .start()
        .unwrap();

    let _device = Device::create("10.128.0.1/12".parse().unwrap())?;

    info!("Device created, waiting for ^+C...");

    ctrl_c().await?;

    Ok(())
}
