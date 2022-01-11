// Derived from https://github.com/meh/rust-tun

use super::codec::TunPacketCodec;
use crate::Result;
use futures::ready;
use ipnetwork::Ipv4Network;
use std::{
    io::{self, IoSlice, Read, Write},
    mem,
    os::unix::prelude::{AsRawFd, IntoRawFd, RawFd},
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::{unix::AsyncFd, AsyncRead, AsyncWrite, ReadBuf};
use tokio_util::codec::Framed;

/// Creating a tun device and then returns the fd.
///
/// We will send this fd with XPC so the unprivileged app can create tun
/// interface.
#[cfg(target_os = "macos")]
pub fn create_tun_as_raw_fd(subnet: Ipv4Network) -> Result<RawFd> {
    use super::route::add_route_for_device;
    use tun::{configure, create, Device as TunDevice, Layer};

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

#[cfg(not(target_os = "macos"))]
pub fn create_tun_as_raw_fd(_subnet: Ipv4Network) -> Result<RawFd> {
    use anyhow::bail;

    bail!("Not supported")
}

pub struct Device {
    inner: AsyncFd<Tun>,
}

impl Device {
    pub fn new(subnet: Ipv4Network) -> Result<Self> {
        let fd = create_tun_as_raw_fd(subnet)?;

        Self::from_raw_fd(fd)
    }

    pub fn from_raw_fd(fd: RawFd) -> Result<Self> {
        Ok(Self {
            inner: AsyncFd::new(Tun { inner: fd })?,
        })
    }

    pub fn into_framed(self, mtu: usize) -> Framed<Self, TunPacketCodec> {
        let codec = TunPacketCodec::new(true, mtu as i32);
        Framed::new(self, codec)
    }
}

impl AsRawFd for Device {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
}

impl IntoRawFd for Device {
    fn into_raw_fd(self) -> RawFd {
        self.inner.into_inner().inner
    }
}

impl AsyncRead for Device {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf,
    ) -> Poll<io::Result<()>> {
        loop {
            let mut guard = ready!(self.inner.poll_read_ready_mut(cx))?;
            let rbuf = buf.initialize_unfilled();
            match guard.try_io(|inner| inner.get_mut().read(rbuf)) {
                Ok(res) => return Poll::Ready(res.map(|n| buf.advance(n))),
                Err(_wb) => continue,
            }
        }
    }
}

impl AsyncWrite for Device {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        loop {
            let mut guard = ready!(self.inner.poll_write_ready_mut(cx))?;
            match guard.try_io(|inner| inner.get_mut().write(buf)) {
                Ok(res) => return Poll::Ready(res),
                Err(_wb) => continue,
            }
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        loop {
            let mut guard = ready!(self.inner.poll_write_ready_mut(cx))?;
            match guard.try_io(|inner| inner.get_mut().flush()) {
                Ok(res) => return Poll::Ready(res),
                Err(_wb) => continue,
            }
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<Result<usize, io::Error>> {
        loop {
            let mut guard = ready!(self.inner.poll_write_ready_mut(cx))?;
            match guard.try_io(|inner| inner.get_mut().write_vectored(bufs)) {
                Ok(res) => return Poll::Ready(res),
                Err(_wb) => continue,
            }
        }
    }

    fn is_write_vectored(&self) -> bool {
        true
    }
}

struct Tun {
    inner: RawFd,
}

impl AsRawFd for Tun {
    fn as_raw_fd(&self) -> RawFd {
        self.inner
    }
}

impl Read for Tun {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        unsafe {
            let amount = libc::read(self.inner, buf.as_mut_ptr() as *mut _, buf.len());

            if amount < 0 {
                return Err(io::Error::last_os_error());
            }

            Ok(amount as usize)
        }
    }

    fn read_vectored(&mut self, bufs: &mut [io::IoSliceMut<'_>]) -> io::Result<usize> {
        unsafe {
            let mut msg: libc::msghdr = mem::zeroed();
            // msg.msg_name: NULL
            // msg.msg_namelen: 0
            msg.msg_iov = bufs.as_mut_ptr().cast();
            msg.msg_iovlen = bufs.len().min(libc::c_int::MAX as usize) as _;

            let n = libc::recvmsg(self.inner, &mut msg, 0);
            if n < 0 {
                return Err(io::Error::last_os_error());
            }

            Ok(n as usize)
        }
    }
}

impl Write for Tun {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        unsafe {
            let amount = libc::write(self.inner, buf.as_ptr() as *const _, buf.len());

            if amount < 0 {
                return Err(io::Error::last_os_error());
            }

            Ok(amount as usize)
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> io::Result<usize> {
        unsafe {
            let mut msg: libc::msghdr = mem::zeroed();
            // msg.msg_name = NULL
            // msg.msg_namelen = 0
            msg.msg_iov = bufs.as_ptr() as *mut _;
            msg.msg_iovlen = bufs.len().min(libc::c_int::MAX as usize) as _;

            let n = libc::sendmsg(self.inner, &msg, 0);
            if n < 0 {
                return Err(io::Error::last_os_error());
            }

            Ok(n as usize)
        }
    }
}

impl IntoRawFd for Tun {
    fn into_raw_fd(self) -> RawFd {
        self.inner
    }
}

impl Drop for Tun {
    fn drop(&mut self) {
        unsafe {
            if self.inner >= 0 {
                libc::close(self.inner);
            }
        }
    }
}
