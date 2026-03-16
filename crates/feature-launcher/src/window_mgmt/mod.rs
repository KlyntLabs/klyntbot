#[cfg(target_os = "macos")]
pub mod accessibility;
pub mod actions;

pub use actions::WindowManager;
