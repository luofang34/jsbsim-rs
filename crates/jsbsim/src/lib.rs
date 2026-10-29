// SPDX-License-Identifier: LGPL-2.1-or-later
//! Unofficial pure-Rust port of the JSBSim flight dynamics model.
//!
//! The core performs no I/O, runs single-threaded and is deterministic, so
//! native and `wasm32-unknown-unknown` builds produce the same results. Model
//! and data files reach the core through a [`Resolver`].

pub mod c_numeric;
pub mod elementary;
pub mod error;
pub mod fdm;
pub mod functions;
pub mod initial_condition;
pub mod linearization;
pub mod math;
pub mod models;
pub mod options;
pub mod properties;
pub mod random;
pub mod resolver;
pub mod script;
pub mod snapshot;
pub mod trim;
pub mod units;
pub mod xml;

pub use error::Error;
pub use options::LoadOptions;
#[cfg(feature = "fs")]
pub use resolver::FileResolver;
pub use resolver::{MemoryResolver, Resolver};
