pub mod cork;
mod kvdatabase;
mod kvengine;
mod kvtree;

#[cfg(feature = "rocksdb")]
pub(crate) mod rocksdb;

#[cfg(feature = "sqlite")]
pub(crate) mod sqlite;

#[cfg(any(feature = "sqlite", feature = "rocksdb"))]
pub(crate) mod watchers;

pub use cork::Cork;
pub use kvdatabase::KeyValueDatabase;
pub use kvengine::KeyValueDatabaseEngine;
pub use kvtree::KvTree;

extern crate conduit_core as conduit;
pub(crate) use conduit::{utils, Config, Result};

conduit::mod_ctor! {}
conduit::mod_dtor! {}
