use super::route::add_route_for_device;
use crate::Result;
use ipnetwork::IpNetwork;
use std::os::unix::prelude::{AsRawFd, RawFd};
use tun::{configure, create_as_async, AsyncDevice, Device as TunDevice, Layer};

pub struct Device {
    inner: AsyncDevice,
}

impl Device {
    pub fn create(subnet: IpNetwork) -> Result<Self> {
        let mut config = configure();
        config
            .layer(Layer::L3)
            .address(subnet.ip())
            // This is a bug, the netmask is not applied.
            // But this won't prevent us from setting up routes.
            .netmask(subnet.mask())
            .up();

        let device = Self {
            inner: create_as_async(&config)?,
        };

        add_route_for_device(&device, &subnet)?;

        Ok(device)
    }

    pub fn name(&self) -> &str {
        self.inner.get_ref().name()
    }
}

impl AsRawFd for Device {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.get_ref().as_raw_fd()
    }
}
