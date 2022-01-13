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
use tun::{configure, create, Layer};

pub type RawDeviceHandle = RawFd;
pub static INVALID_DEVICE_HANDLE: RawDeviceHandle = -1;

/// Creating a tun device and then returns the fd.
///
/// We will send this fd with XPC so the unprivileged app can create tun
/// interface.
#[cfg(target_os = "macos")]
pub fn create_tun_as_raw_handle(subnet: Ipv4Network) -> Result<RawDeviceHandle> {
    use tun::Device as TunDevice;

    let mut config = configure();
    config
        .layer(Layer::L3)
        .address(subnet.ip())
        .netmask(subnet.mask())
        .up();

    let mut device = create(&config)?;
    // This is a bug, the netmask is not applied. We need to apply again.
    device.set_alias(subnet.ip(), subnet.broadcast(), subnet.mask())?;

    self::route::add_route_for_device(device.name(), &subnet)?;

    Ok(device.into_raw_fd())
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn create_tun_as_raw_handle(subnet: Ipv4Network) -> Result<RawDeviceHandle> {
    let mut config = configure();
    config
        .layer(Layer::L3)
        .address(subnet.ip())
        // This is a bug, the netmask is not applied.
        // But this won't prevent us from setting up routes.
        .netmask(subnet.mask())
        .up();

    let device = create(&config)?;

    Ok(device.into_raw_fd())
}

pub struct Device {
    inner: AsyncFd<Tun>,
}

impl Device {
    pub fn new(subnet: Ipv4Network) -> Result<Self> {
        let fd = create_tun_as_raw_handle(subnet)?;

        Self::from_raw_device_handle(fd)
    }

    pub fn from_raw_device_handle(fd: RawDeviceHandle) -> Result<Self> {
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

#[cfg(target_os = "macos")]
mod route {
    use crate::Result;
    use anyhow::{bail, Context};
    use ipnetwork::Ipv4Network;
    use nix::unistd::{close, write};
    use os_socketaddr::OsSocketAddr;
    use scopeguard::guard;
    use std::{
        ffi::CStr,
        mem,
        net::{SocketAddr, SocketAddrV4},
        os::unix::prelude::AsRawFd,
        slice::from_raw_parts,
    };

    // From https://github.com/shadowsocks/shadowsocks-rust/blob/master/crates/shadowsocks-service/src/local/tun/sys/unix/apple/macos.rs

    /// These numbers are used by reliable protocols for determining
    /// retransmission behavior and are included in the routing structure.
    #[repr(C)]
    #[allow(non_camel_case_types)]
    #[derive(Debug, Clone, Copy)]
    struct rt_metrics {
        rmx_locks: u32,       //< Kernel must leave these values alone
        rmx_mtu: u32,         //< MTU for this path
        rmx_hopcount: u32,    //< max hops expected
        rmx_expire: i32,      //< lifetime for route, e.g. redirect
        rmx_recvpipe: u32,    //< inbound delay-bandwidth product
        rmx_sendpipe: u32,    //< outbound delay-bandwidth product
        rmx_ssthresh: u32,    //< outbound gateway buffer limit
        rmx_rtt: u32,         //< estimated round trip time
        rmx_rttvar: u32,      //< estimated rtt variance
        rmx_pksent: u32,      //< packets sent using this route
        rmx_state: u32,       //< route state
        rmx_filler: [u32; 3], //< will be used for T/TCP later
    }

    /// Structures for routing messages.
    #[repr(C)]
    #[allow(non_camel_case_types)]
    #[derive(Debug, Clone, Copy)]
    struct rt_msghdr {
        rtm_msglen: libc::c_ushort, //< to skip over non-understood messages
        rtm_version: libc::c_uchar, //< future binary compatibility
        rtm_type: libc::c_uchar,    //< message type
        rtm_index: libc::c_ushort,  //< index for associated ifp
        rtm_flags: libc::c_int,     //< flags, incl. kern & message, e.g. DONE
        rtm_addrs: libc::c_int,     //< bitmask identifying sockaddrs in msg
        rtm_pid: libc::pid_t,       //< identify sender
        rtm_seq: libc::c_int,       //< for sender to identify action
        rtm_errno: libc::c_int,     //< why failed
        rtm_use: libc::c_int,       //< from rtentry
        rtm_inits: u32,             //< which metrics we are initializing
        rtm_rmx: rt_metrics,        //< metrics themselves
    }

    // The content needs to be aligned to 4, which should be true anyway since we
    // are not packing. Do nothing explicitly.
    //
    // Only support IPv4.
    //
    // TODO: Add support for IPv6
    #[repr(C)]
    #[allow(non_camel_case_types)]
    #[derive(Debug, Clone, Copy)]
    struct rt_msg {
        rtm: rt_msghdr,
        dst: libc::sockaddr_in,
        gateway: libc::sockaddr_dl,
        netmask: libc::sockaddr_in,
    }

    pub fn add_route_for_device(name: &str, route: &Ipv4Network) -> Result<()> {
        let mut rtmsg: rt_msg = unsafe { mem::zeroed() };
        rtmsg.rtm.rtm_type = libc::RTM_ADD as libc::c_uchar;
        rtmsg.rtm.rtm_flags = libc::RTF_UP | libc::RTF_STATIC;
        rtmsg.rtm.rtm_version = libc::RTM_VERSION as libc::c_uchar;
        rtmsg.rtm.rtm_seq = rand::random();
        rtmsg.rtm.rtm_addrs = libc::RTA_DST | libc::RTA_GATEWAY | libc::RTA_NETMASK;
        rtmsg.rtm.rtm_msglen = mem::size_of_val(&rtmsg) as libc::c_ushort;
        unsafe {
            rtmsg.rtm.rtm_pid = libc::getpid();
        }

        let addr_sockaddr: OsSocketAddr = SocketAddr::V4(SocketAddrV4::new(route.ip(), 0)).into();
        unsafe {
            std::ptr::copy_nonoverlapping(
                addr_sockaddr.as_ptr() as *mut libc::sockaddr_in,
                &mut rtmsg.dst as *mut _,
                1,
            );
        }
        rtmsg.dst.sin_len = std::mem::size_of_val(&rtmsg.dst) as u8;

        let netmask_sockaddr: OsSocketAddr =
            SocketAddr::V4(SocketAddrV4::new(route.mask(), 0)).into();
        unsafe {
            std::ptr::copy_nonoverlapping(
                netmask_sockaddr.as_ptr() as *mut libc::sockaddr_in,
                &mut rtmsg.netmask as *mut _,
                1,
            );
        }
        rtmsg.netmask.sin_len = std::mem::size_of_val(&rtmsg.netmask) as u8;

        unsafe {
            let mut ifap: *mut libc::ifaddrs = std::ptr::null_mut();
            if libc::getifaddrs(&mut ifap) != 0 {
                return Err(std::io::Error::last_os_error().into());
            }

            let _guard = guard(ifap, |ifap| libc::freeifaddrs(ifap));

            let mut ifa = ifap;
            let mut found = false;
            while !ifa.is_null() {
                if !(*ifa).ifa_addr.is_null()
                    && (*(*ifa).ifa_addr).sa_family as i32 == libc::AF_LINK
                {
                    let ifa_name = CStr::from_ptr((*ifa).ifa_name);
                    if ifa_name.to_bytes() == name.as_bytes() {
                        let sdl: *mut libc::sockaddr_dl = (*ifa).ifa_addr as *mut _;
                        std::ptr::copy_nonoverlapping(sdl, &mut rtmsg.gateway as *mut _, 1);
                        found = true;

                        break;
                    }
                }

                ifa = (*ifa).ifa_next;
            }

            if !found {
                bail!("Failed to get AF_LINK address for interface {}", name)
            }
        }

        // Ideally we would use nix here, but nix doesn't has PF_ROUTE/AF_ROUTE defined.
        let s = unsafe { libc::socket(libc::PF_ROUTE, libc::SOCK_RAW, 0) };

        if s < 0 {
            bail!(
                "Failed to create route control socket: {}",
                std::io::Error::last_os_error()
            )
        }

        let fd = s;

        let _guard = guard(fd, |fd| {
            let _ = close(fd.as_raw_fd());
        });

        let buf: &[u8] =
            unsafe { from_raw_parts(&rtmsg as *const _ as *const u8, mem::size_of_val(&rtmsg)) };

        write(fd, buf).context("Failed to send route control message")?;

        Ok(())
    }
}
