// SPDX-License-Identifier: LGPL-2.1-or-later
//! Unofficial pure-Rust port of the JSBSim flight dynamics model.
//!
//! The core performs no I/O, runs single-threaded and is deterministic, so
//! native and `wasm32-unknown-unknown` builds produce the same results. Model
//! and data files reach the core through a [`Resolver`].
//!
//! # Flow
//!
//! 1. Make an executive with [`Fdm::new`] or [`Fdm::with_options`].
//! 2. Optionally install terrain with [`Fdm::set_ground_callback`].
//! 3. Load an aircraft with [`Fdm::load_model`] or a whole script with
//!    [`Fdm::load_script`], reading files through a [`Resolver`].
//! 4. Set initial conditions (`ic/...` properties, or
//!    [`Fdm::load_initial_conditions`]), apply them with [`Fdm::run_ic`],
//!    and trim with [`Fdm::do_trim`] if wanted.
//! 5. Step with [`Fdm::run`]; read and write properties by name
//!    ([`Fdm::get_property`], [`Fdm::set_property`]) or by [`NodeId`]
//!    ([`Fdm::node`], [`Fdm::read_node`], [`Fdm::write_node`]); read the
//!    pose free of Euler singularities with [`Fdm::location`] and
//!    [`Fdm::quaternion`].
//! 6. Save and restore the dynamic state with [`snapshot::Snapshot`].
//!
//! ```
//! use jsbsim::{Fdm, MemoryResolver};
//!
//! let mut files = MemoryResolver::new();
//! files.insert(
//!     "aircraft/box/box.xml",
//!     r#"<fdm_config name="box" version="2.0">
//!       <metrics> <wingarea unit="FT2"> 1 </wingarea> <wingspan unit="FT"> 1 </wingspan>
//!         <chord unit="FT"> 1 </chord> </metrics>
//!       <mass_balance> <ixx> 10 </ixx> <iyy> 10 </iyy> <izz> 10 </izz>
//!         <emptywt> 100 </emptywt> <location name="CG"> <x>0</x> <y>0</y> <z>0</z> </location>
//!       </mass_balance>
//!       <ground_reactions> <contact type="STRUCTURE" name="BOTTOM">
//!         <location> <x>0</x> <y>0</y> <z>-10</z> </location>
//!         <spring_coeff> 1000 </spring_coeff> <damping_coeff> 100 </damping_coeff>
//!       </contact> </ground_reactions>
//!       <aerodynamics> <axis name="DRAG"/> </aerodynamics>
//!     </fdm_config>"#,
//! );
//! let mut fdm = Fdm::new()?;
//! fdm.load_model(&files, "aircraft", "box")?;
//! fdm.set_property("ic/h-sl-ft", 1000.0)?;
//! fdm.run_ic()?;
//! while fdm.get_property("simulation/sim-time-sec")? < 1.0 {
//!     fdm.run()?;
//! }
//! assert!(fdm.get_property("position/h-sl-ft")? < 1000.0);
//! # Ok::<(), jsbsim::Error>(())
//! ```

mod c_numeric;
// Without `__oracle`, the accessors and C++ mirrors that only tools/oracle
// reads are unused. Every workspace build compiles the feature in, so
// anything nothing reads is still reported there.
#[cfg_attr(not(feature = "__oracle"), allow(dead_code, unused_imports))]
mod elementary;
#[cfg_attr(not(feature = "__oracle"), allow(dead_code, unused_imports))]
mod fdm;
#[cfg_attr(not(feature = "__oracle"), allow(dead_code, unused_imports))]
mod functions;
#[cfg_attr(not(feature = "__oracle"), allow(dead_code, unused_imports))]
mod initial_condition;
#[cfg_attr(not(feature = "__oracle"), allow(dead_code, unused_imports))]
mod linearization;
#[cfg_attr(not(feature = "__oracle"), allow(dead_code, unused_imports))]
mod models;
mod options;
#[cfg_attr(not(feature = "__oracle"), allow(dead_code, unused_imports))]
mod properties;
mod random;
#[cfg(doctest)]
mod readme;
mod resolver;
mod script;
#[cfg_attr(not(feature = "__oracle"), allow(dead_code, unused_imports))]
mod units;
#[cfg_attr(not(feature = "__oracle"), allow(dead_code, unused_imports))]
mod xml;

pub mod error;
pub mod math;
pub mod snapshot;
#[cfg_attr(not(feature = "__oracle"), allow(dead_code, unused_imports))]
pub mod trim;

#[cfg(feature = "__oracle")]
pub mod oracle_support;

pub use error::Error;
pub use fdm::{Fdm, ResetMask};
pub use models::inertial::{Contact, GroundCallback};
pub use options::LoadOptions;
pub use properties::NodeId;
#[cfg(feature = "fs")]
pub use resolver::FileResolver;
pub use resolver::{MemoryResolver, Resolver};
