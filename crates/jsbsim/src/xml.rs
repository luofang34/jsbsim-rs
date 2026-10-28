// SPDX-License-Identifier: LGPL-2.1-or-later
//! Loader for JSBSim's aircraft, engine, system, script and
//! initial-conditions XML, with JSBSim's unit conversions. Constructs that are
//! not supported are rejected with [`Error::Unsupported`](crate::Error::Unsupported).
