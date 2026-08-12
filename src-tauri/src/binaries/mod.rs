pub mod process;
pub mod resolve;

pub use process::configure_hidden_process;
pub use resolve::{read_version, resolve, BinaryPaths};
