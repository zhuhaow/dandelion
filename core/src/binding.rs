// See https://www.nickwilcox.com/blog/recipe_swift_rust_callback/

use crate::Result;
use crate::{
    server::{config::ServerConfig, privilege::PrivilegeHandler, Server},
    tun::device::{create_tun_as_raw_fd, Device},
};
use futures::future::AbortHandle;
use ipnetwork::Ipv4Network;
use rich_phantoms::PhantomInvariantAlwaysSendSync;
use std::{
    ffi::{c_void, CStr, CString},
    fs::read_to_string,
    net::SocketAddr,
    os::{raw::c_char, unix::prelude::RawFd},
    path::PathBuf,
    ptr::{null, NonNull},
    thread,
};
use tokio::sync::oneshot::Sender;
use tokio::{runtime::Builder, sync::oneshot};

// Contains information about proxy server listening. If addr is nil, it means
// there is no server info.
#[repr(C)]
pub struct ServerInfo {
    addr: *const c_char,
    port: u16,
}

// The char ptr points to an error string if there is an error. If it's null,
// then the action completes successfully. The callback should copy the error
// string immediately if needed since it should be released after the callback.
type ErrorCallback = extern "C" fn(callback_data: NonNull<c_void>, err_str: *const c_char);
type ErrorPayloadCallback<T> =
    extern "C" fn(callback_data: NonNull<c_void>, payload: T, err_str: *const c_char);

// We do not own ServerInfo or anything inside of it. Copy the string
// immediately before te callback is returned.
//
// The callback MUST BE CALLED otherwise the callback_data will leak.
type ExternalServerHandler = extern "C" fn(
    context_data: NonNull<c_void>,
    server: NonNull<ServerInfo>,
    callback: ErrorCallback,
    callback_data: NonNull<c_void>,
);
type ExternalPayloadHandler<T> = extern "C" fn(
    context_data: NonNull<c_void>,
    info: NonNull<c_char>,
    callback: ErrorPayloadCallback<T>,
    callback_data: NonNull<c_void>,
);

#[repr(C)]
pub struct Context {
    data: NonNull<c_void>,
    // We do not own ServerInfo or anything inside of it. Copy the string
    // immediately before te callback is returned.
    set_http_proxy_handler: ExternalServerHandler,
    set_socks5_proxy_handler: ExternalServerHandler,
    set_dns_handler: ExternalServerHandler,
    create_tun_interface_handler: ExternalPayloadHandler<RawFd>,
    // The char ptr points to an error string if there is an error. If it's
    // null, then the action completes successfully. The callback should copy
    // the error string immediately if needed since it will be released after
    // the callback.
    //
    // The context data should be released by the handler.
    //
    // Even done_handler is called, the callback of any outgoing handler request
    // should still be called.
    done_handler: extern "C" fn(context_data: NonNull<c_void>, err_str: *const c_char),
}

unsafe impl Send for Context {}

impl Context {
    fn to_privilege_handler(&self) -> PrivilegeCallbackHandler<'_> {
        PrivilegeCallbackHandler {
            callback_data: self.data,
            set_http_proxy_handler: self.set_http_proxy_handler,
            set_socks5_proxy_handler: self.set_socks5_proxy_handler,
            create_tun_interface_handler: self.create_tun_interface_handler,
            set_dns_handler: self.set_dns_handler,
            _marker: Default::default(),
        }
    }

    fn done(self, error: Option<String>) {
        let err_str = error.map(|e| CString::new(e).unwrap());

        (self.done_handler)(
            self.data,
            match err_str {
                Some(ref s) => s.as_ptr(),
                None => null(),
            },
        );

        // We control the lifetime manually so we can panic if the done is not
        // called.
        std::mem::forget(self)
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        // Use this to make sure we are calling `done`, not accidentally releasing it.
        panic!("Callback must be called")
    }
}

/// # Safety
///
/// The function won't take the ownership of the passed in config path string.
/// The callback may be called from any thread. Pay attention to synchronization.
///
/// The returned stop handler must be released by calling stop with the pointer no
/// matter the server is already stopped because of an error or not.
#[no_mangle]
pub unsafe extern "C" fn specht2_start(
    config_path: NonNull<c_char>,
    route_traffic: bool,
    context: Context,
) -> NonNull<c_void> {
    let path_string = CStr::from_ptr(config_path.as_ptr())
        .to_string_lossy()
        .into_owned();
    let path = PathBuf::from(path_string);

    #[cfg(not(target_os = "windows"))]
    {
        use fdlimit::raise_fd_limit;
        raise_fd_limit();
    }

    let (handle, reg) = AbortHandle::new_pair();

    thread::spawn(move || {
        let runtime = Builder::new_multi_thread()
            .enable_io()
            .enable_time()
            .build()
            .expect("Failed to create async runtime for server");

        let context_ref = &context;

        let result = runtime.block_on(async move {
            let config: ServerConfig = ron::de::from_str(&read_to_string(path)?)?;

            let server = Server::new(config, context_ref.to_privilege_handler(), route_traffic);

            server.serve(reg).await
        });

        context.done(result.err().map(|e| e.to_string()));
    });

    let handle = Box::into_raw(Box::new(handle));
    NonNull::new_unchecked(handle as *mut c_void)
}

#[no_mangle]
pub extern "C" fn specht2_stop(handle: NonNull<c_void>) {
    let handle = unsafe { Box::from_raw(handle.as_ptr() as *mut AbortHandle) };
    handle.abort()
}

#[no_mangle]
pub extern "C" fn specht2_create_tun(subnet: NonNull<c_char>) -> RawFd {
    let subnet_string = unsafe { CStr::from_ptr(subnet.as_ptr()) }
        .to_string_lossy()
        .into_owned();

    let subnet = match subnet_string.parse() {
        Ok(subnet) => subnet,
        Err(_) => return -1,
    };

    create_tun_as_raw_fd(subnet).unwrap_or(-1)
}

extern "C" fn handler_callback(sender: NonNull<c_void>, err_ptr: *const c_char) {
    let tx = unsafe { Box::from_raw(sender.as_ptr() as *mut Sender<Result<()>>) };

    let _ = match unsafe { parse_error(err_ptr) } {
        Some(err) => tx.send(Err(err)),
        None => tx.send(Ok(())),
    };
}

extern "C" fn rawfd_handler_callback(
    sender: NonNull<c_void>,
    rawfd: RawFd,
    err_ptr: *const c_char,
) {
    let tx = unsafe { Box::from_raw(sender.as_ptr() as *mut Sender<Result<RawFd>>) };

    let _ = match unsafe { parse_error(err_ptr) } {
        Some(err) => tx.send(Err(err)),
        None => tx.send(Ok(rawfd)),
    };
}

struct PrivilegeCallbackHandler<'a> {
    callback_data: NonNull<c_void>,
    set_http_proxy_handler: ExternalServerHandler,
    set_socks5_proxy_handler: ExternalServerHandler,
    create_tun_interface_handler: ExternalPayloadHandler<RawFd>,
    set_dns_handler: ExternalServerHandler,
    _marker: PhantomInvariantAlwaysSendSync<&'a ()>,
}

unsafe impl Sync for PrivilegeCallbackHandler<'_> {}
unsafe impl Send for PrivilegeCallbackHandler<'_> {}

macro_rules! handler_impl {
    ($struct_name:ident,
        $( $fn_name:ident,$handler:ident),* ) => {
        impl $struct_name<'_> {
            $(
                async fn $fn_name(&self, addr: Option<SocketAddr>) -> Result<()> {
                    let (tx, rx) = oneshot::channel::<Result<()>>();

                    {
                        let tx = Box::into_raw(Box::new(tx));

                        let ip_str = addr
                            .as_ref()
                            .map(|s| CString::new(s.ip().to_string()).unwrap());
                        let mut info = ServerInfo {
                            addr: ip_str.as_ref().map(|ip| ip.as_ptr()).unwrap_or(null()),
                            port: addr.as_ref().map(|s| s.port()).unwrap_or_default(),
                        };

                        unsafe {
                            (self.$handler)(
                                self.callback_data,
                                NonNull::new_unchecked(&mut info as *mut ServerInfo),
                                handler_callback,
                                NonNull::new_unchecked(tx as *mut c_void),
                            )
                        };
                    }

                    rx.await.unwrap()
                }
            )*
        }
    };
}

handler_impl!(
    PrivilegeCallbackHandler,
    set_http_proxy_impl,
    set_http_proxy_handler
);

handler_impl!(
    PrivilegeCallbackHandler,
    set_socks5_proxy_impl,
    set_socks5_proxy_handler
);

handler_impl!(PrivilegeCallbackHandler, set_dns_impl, set_dns_handler);

#[async_trait::async_trait]
impl PrivilegeHandler for PrivilegeCallbackHandler<'_> {
    async fn set_http_proxy(&self, addr: Option<SocketAddr>) -> Result<()> {
        self.set_http_proxy_impl(addr).await
    }

    async fn set_socks5_proxy(&self, addr: Option<SocketAddr>) -> Result<()> {
        self.set_socks5_proxy_impl(addr).await
    }

    async fn create_tun_interface(&self, subnet: &Ipv4Network) -> Result<Device> {
        let (tx, rx) = oneshot::channel::<Result<RawFd>>();

        {
            let tx = Box::into_raw(Box::new(tx));

            let subnet_str = CString::new(subnet.to_string()).unwrap();

            unsafe {
                (self.create_tun_interface_handler)(
                    self.callback_data,
                    NonNull::new_unchecked(subnet_str.as_ptr() as *mut i8),
                    rawfd_handler_callback,
                    NonNull::new_unchecked(tx as *mut c_void),
                )
            };
        }

        rx.await.unwrap().and_then(Device::from_raw_fd)
    }

    async fn set_dns(&self, addr: Option<SocketAddr>) -> Result<()> {
        self.set_dns_impl(addr).await
    }
}

unsafe fn parse_error(error_str: *const c_char) -> Option<anyhow::Error> {
    if error_str.is_null() {
        None
    } else {
        let error_string = CStr::from_ptr(error_str).to_string_lossy().into_owned();
        Some(anyhow::anyhow!(error_string))
    }
}
