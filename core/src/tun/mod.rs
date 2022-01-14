pub mod acceptor;
mod dns;
mod translator;

cfg_if::cfg_if! {
    if #[cfg(any(target_os = "macos", target_os = "linux"))] {
        pub mod device_unix;
        pub use device_unix as device;

        pub mod stack_unix;
        pub use stack_unix as stack;
    } else {
        pub mod device_win;
        pub use device_win as device;

        pub mod stack_win;
        pub use stack_win as stack;
    }
}
