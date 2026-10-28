// SPDX-License-Identifier: LGPL-2.1-or-later
//! The Rust side, the same procedure as `tools/oracle/cpp/bench.cpp`.

use std::path::Path;
use std::time::Instant;

use jsbsim::FileResolver;
use jsbsim::fdm::Fdm;
use tracing::subscriber::NoSubscriber;

use crate::OracleError;

use super::{AIRBORNE_IC, BenchCase, Workload};

/// Times one repeat of `case` with files from `root`; returns the elapsed
/// ns of the timed frames. Log records, such as load warnings and event
/// notifications, are dropped, as the driver keeps JSBSim's off the
/// console; failures still come back as errors.
pub(super) fn time(root: &Path, case: &BenchCase) -> Result<u64, OracleError> {
    let port = |source| OracleError::BenchPort {
        case: case.name.clone(),
        source,
    };
    let workload = case.workload()?;
    let ran = tracing::subscriber::with_default(NoSubscriber::default(), || {
        let mut fdm = Fdm::new()?;
        if let Some(path) = &case.aircraft_path {
            fdm.set_aircraft_path(path);
        }
        let resolver = FileResolver::new(root);
        match workload {
            Workload::Script(script) => fdm.load_script(&resolver, script)?,
            Workload::Aircraft(name) => {
                let path = case.aircraft_path.as_deref().unwrap_or("aircraft");
                fdm.load_model(&resolver, path, name)?;
                for (property, value) in AIRBORNE_IC {
                    fdm.set_property(property, value)?;
                }
            }
        }
        fdm.run_ic()?;
        let mut ok = true;
        for _ in 0..case.warm_up {
            ok = ok && fdm.run()?;
        }
        let start = Instant::now();
        for _ in 0..case.frames {
            ok = ok && fdm.run()?;
        }
        Ok((start.elapsed(), ok))
    });
    let (elapsed, ok) = ran.map_err(port)?;
    if !ok {
        return Err(OracleError::Bench {
            case: case.name.clone(),
            message: format!(
                "Rust run ended before {} warm-up and {} timed frames",
                case.warm_up, case.frames
            ),
        });
    }
    u64::try_from(elapsed.as_nanos()).map_err(|e| OracleError::Bench {
        case: case.name.clone(),
        message: format!("elapsed time does not fit in u64 ns: {e}"),
    })
}
