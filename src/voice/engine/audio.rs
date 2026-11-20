#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::*;

#[cfg(target_arch = "wasm32")]
mod unsupported;
#[cfg(target_arch = "wasm32")]
pub use unsupported::*;
