#![cfg(feature = "mods")]

pub(crate) use libloading::os::unix::Library; //TODO: x-platform
pub(crate) use libloading::os::unix::Symbol;

pub mod canary;
pub mod macros;
pub mod module;
pub mod new;
pub mod path;

pub use module::Module; //TODO: x-platform
