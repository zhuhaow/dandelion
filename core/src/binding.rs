// See https://www.nickwilcox.com/blog/recipe_swift_rust_callback/

use crate::{
    endpoint::Endpoint,
    server::{
        config::{AcceptorConfig, ServerConfig},
        Server,
    },
};
use std::{
    ffi::{c_void, CStr, CString},
    fs::read_to_string,
    os::raw::c_char,
    path::PathBuf,
    ptr::{null, NonNull},
    thread,
};
use tokio::{runtime::Builder, select, sync::oneshot};

// Contains information about proxy server listening. The char ptr will be null
// if we are not listening on that type.
#[repr(C)]
pub struct ServerInfo {
    socks5_addr: *const c_char,
    socks5_port: u16,
    http_addr: *const c_char,
    http_port: u16,
}

#[repr(C)]
pub struct EventCallback {
    userdata: NonNull<c_void>,
    // You do not own ServerInfo or anything inside of it. Copy the string
    // immediately before te callback is returned.
    before_start_callback: extern "C" fn(NonNull<c_void>, NonNull<ServerInfo>),
    // The char ptr points to an error string if there is an error. If it's
    // null, then the action completes successfully. The callback should copy
    // the error string immediately if needed since it will be released after
    // the callback.
    done_callback: extern "C" fn(NonNull<c_void>, *const c_char),
}

unsafe impl Send for EventCallback {}

impl EventCallback {
    fn before_start(&self, socks_info: Option<Endpoint>, http_info: Option<Endpoint>) {
        #[cfg(not(target_os = "windows"))]
        {
            use fdlimit::raise_fd_limit;
            raise_fd_limit();
        }

        let socks5_addr = socks_info.map(|e| (CString::new(e.hostname()).unwrap(), e.port()));
        let http_addr = http_info.map(|e| (CString::new(e.hostname()).unwrap(), e.port()));

        let info = ServerInfo {
            socks5_addr: socks5_addr.as_ref().map(|s| s.0.as_ptr()).unwrap_or(null()),
            socks5_port: socks5_addr.as_ref().map(|s| s.1).unwrap_or_default(),
            http_addr: http_addr.as_ref().map(|s| s.0.as_ptr()).unwrap_or(null()),
            http_port: http_addr.as_ref().map(|s| s.1).unwrap_or_default(),
        };

        let info_ptr = Box::into_raw(Box::new(info));
        (self.before_start_callback)(self.userdata, unsafe { NonNull::new_unchecked(info_ptr) });
        unsafe {
            Box::from_raw(info_ptr);
        }
    }

    fn done(self, error: Option<String>) {
        let err_str = error.map(|e| CString::new(e).unwrap());

        (self.done_callback)(
            self.userdata,
            match err_str {
                Some(ref s) => s.as_ptr(),
                None => null(),
            },
        );

        std::mem::forget(self)
    }
}

impl Drop for EventCallback {
    fn drop(&mut self) {
        // Use this to make sure we are calling `done`, not accidentally releasing it.
        panic!("EventCallback must be called")
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
    callback: EventCallback,
) -> NonNull<c_void> {
    let path_string = CStr::from_ptr(config_path.as_ptr())
        .to_string_lossy()
        .into_owned();
    let path = PathBuf::from(path_string);
    let (tx, rx) = oneshot::channel::<()>();

    thread::spawn(move || {
        let runtime = Builder::new_multi_thread()
            .enable_io()
            .enable_time()
            .build()
            .expect("Failed to create async runtime for server");

        let callback_ref = &callback;

        let result = runtime.block_on(async move {
            let config: ServerConfig = ron::de::from_str(&read_to_string(path)?)?;

            let socks_info = config
                .acceptors
                .iter()
                .find(|acceptor| matches!(acceptor, AcceptorConfig::Socks5 { addr: _ }))
                .map(|acceptor| Endpoint::new_from_addr(*acceptor.server_addr()));

            let http_info = config
                .acceptors
                .iter()
                .find(|acceptor| -> bool { matches!(acceptor, AcceptorConfig::Http { addr: _ }) })
                .map(|acceptor| Endpoint::new_from_addr(*acceptor.server_addr()));

            callback_ref.before_start(socks_info, http_info);

            let server = Server::new(config);

            select! {
                result = server.serve() => result,
                _ = rx => Ok(()),
            }
        });

        callback.done(result.err().map(|e| e.to_string()));
    });

    let tx = Box::into_raw(Box::new(tx));
    NonNull::new_unchecked(tx as *mut c_void)
}

#[no_mangle]
pub extern "C" fn specht2_stop(sender: NonNull<c_void>) -> bool {
    let sender = unsafe { Box::from_raw(sender.as_ptr() as *mut oneshot::Sender<()>) };
    sender.send(()).is_ok()
}
