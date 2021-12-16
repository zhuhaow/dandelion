// See https://www.nickwilcox.com/blog/recipe_swift_rust_callback/

use crate::server::{config::ServerConfig, Server};
use std::{
    ffi::{c_void, CStr, CString},
    fs::read_to_string,
    os::raw::c_char,
    path::PathBuf,
    ptr::{null, NonNull},
    thread,
};
use tokio::{runtime::Builder, select, sync::oneshot};

#[repr(C)]
pub struct CompletedCallback {
    userdata: NonNull<c_void>,
    // The char ptr points to an error string if there is an error. If it's
    // null, then the action completes successfully. The callback should copy
    // the error string immediately if needed since it will be released after
    // the callback.
    callback: extern "C" fn(NonNull<c_void>, *const c_char),
}

unsafe impl Send for CompletedCallback {}

impl CompletedCallback {
    fn done(self, error: Option<String>) {
        let err_str = error.map(|e| CString::new(e).unwrap());

        (self.callback)(
            self.userdata,
            match err_str {
                Some(ref s) => s.as_ptr(),
                None => null(),
            },
        );

        std::mem::forget(self)
    }
}

impl Drop for CompletedCallback {
    fn drop(&mut self) {
        // Use this to make sure we are calling `done`, not accidentally releasing it.
        panic!("CompletedCallback must be called")
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
    callback: CompletedCallback,
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

        let result = runtime.block_on(async move {
            let config: ServerConfig = ron::de::from_str(&read_to_string(path)?)?;
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
