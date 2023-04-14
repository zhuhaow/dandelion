use crate::Result;
use anyhow::bail;
use ipnetwork::Ipv4Network;
use std::os::windows::io::RawSocket;

pub type RawDeviceHandle = RawSocket;
pub static INVALID_DEVICE_HANDLE: RawDeviceHandle = 0xffff;

pub fn create_tun_as_raw_handle(_subnet: Ipv4Network) -> Result<RawDeviceHandle> {
    bail!("Not supported")
}

pub struct Device {}

impl Device {
    pub fn new(_subnet: Ipv4Network) -> Result<Self> {
        bail!("Not supported");
    }

    pub fn from_raw_device_handle(_handle: RawDeviceHandle) -> Result<Self> {
        bail!("Not supported");
    }
}
