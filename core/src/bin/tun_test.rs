use ipnetwork::Ipv4Network;
use specht2_core::tun::{device::Device, stack::create_stack};
use specht2_core::Result;

#[tokio::main]
async fn main() -> Result<()> {
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
