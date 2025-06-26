use crate::Result;
use ipnetwork::Ipv4Network;
use tun::{create, Configuration, Device, Layer};

pub fn create_tun(subnet: Ipv4Network) -> Result<Device> {
    let mut config = Configuration::default();
    config
        .layer(Layer::L3)
        .address(subnet.ip())
        .netmask(subnet.mask())
        .up();

    let device = create(&config)?;

    Ok(device)
}
