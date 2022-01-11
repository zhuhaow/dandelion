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

    let netmask_sockaddr: OsSocketAddr = SocketAddr::V4(SocketAddrV4::new(route.mask(), 0)).into();
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
            if !(*ifa).ifa_addr.is_null() && (*(*ifa).ifa_addr).sa_family as i32 == libc::AF_LINK {
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
