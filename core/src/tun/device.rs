use super::route::add_route_for_device;
use crate::Result;
use ipnetwork::IpNetwork;
use std::os::unix::prelude::{AsRawFd, IntoRawFd, RawFd};
use tun::{configure, create, create_as_async, AsyncDevice, Device as TunDevice, Layer};

pub struct Device {
    inner: AsyncDevice,
}

pub fn create_as_raw_fd(subnet: IpNetwork) -> Result<RawFd> {
    let mut config = configure();
    config
        .layer(Layer::L3)
        .address(subnet.ip())
        // This is a bug, the netmask is not applied.
        // But this won't prevent us from setting up routes.
        .netmask(subnet.mask())
        .up();

    let device = create(&config)?;

    add_route_for_device(device.name(), &subnet)?;

    Ok(device.into_raw_fd())
}

pub fn create_as_device(subnet: IpNetwork, fd: Option<RawFd>) -> Result<Device> {
    let mut config = configure();
    config
        .layer(Layer::L3)
        .address(subnet.ip())
        // This is a bug, the netmask is not applied.
        // But this won't prevent us from setting up routes.
        .netmask(subnet.mask())
        .up();

    if let Some(fd) = fd {
        config.raw_fd(fd);
    }

    let device = Device {
        inner: create_as_async(&config)?,
    };

    add_route_for_device(device.inner.get_ref().name(), &subnet)?;

    Ok(device)
}

impl Device {
    pub fn into_inner(self) -> AsyncDevice {
        self.inner
    }
}

impl AsRawFd for Device {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.get_ref().as_raw_fd()
    }
}
