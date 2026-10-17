//! Lexo Core in Rust. The modules mirror `core/lexo` (rpy.rs for rpy.py, ...), see docs/RUST-PORT.md.
//! No UI, no HTTP: the library only reads and writes files through `std::fs`.

pub mod bulk;
pub mod clock;
pub mod config;
pub mod csvio;
pub mod datadir;
pub mod editor;
pub mod events;
pub mod export;
pub mod fixes;
pub mod fontmetrics;
pub mod fsutil;
pub mod gameimages;
pub mod history;
pub mod lang;
pub mod layout;
pub mod lint;
pub mod lock;
pub mod memory;
pub mod menus;
pub mod messages;
pub mod missed;
pub mod pickle;
pub mod project;
pub mod pyfmt;
pub mod rpa;
pub mod rpy;
pub mod snapshots;
pub mod spelling;
pub mod store;
pub mod textedit;
pub mod texthistory;
pub mod util;
pub mod validator;
