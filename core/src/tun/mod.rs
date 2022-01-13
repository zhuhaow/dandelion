pub mod acceptor;
mod dns;
mod translator;

#[cfg_attr(unix, path = "device_unix.rs")]
#[cfg_attr(windows, path = "device_win.rs")]
pub mod device;
#[cfg_attr(unix, path = "stack_unix.rs")]
#[cfg_attr(windows, path = "stack_win.rs")]
pub mod stack;
