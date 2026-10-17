//! Lexo Core in Rust. The modules mirror `core/lexo` (rpy.rs for rpy.py, ...), see docs/RUST-PORT.md.
//! No UI, no HTTP: the library only reads and writes files through `std::fs`.

pub mod lang;
pub mod layout;
pub mod menus;
pub mod messages;
pub mod pickle;
pub mod rpa;
pub mod rpy;
pub mod util;
