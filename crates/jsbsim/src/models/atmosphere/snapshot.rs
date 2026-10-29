// SPDX-License-Identifier: LGPL-2.1-or-later
//! The atmosphere in a snapshot.

use super::{Atmosphere, AtmosphereModel, Inputs, StandardProfile};
use crate::snapshot::{visit_fields, visit_unit_enum};

visit_unit_enum! { AtmosphereModel { Standard, Mars, Msis } }
visit_fields! { Inputs { altitude_asl, geod_latitude_deg, longitude_deg } }
// The pressure and density breakpoints are recomputed whenever the sea
// level conditions change; the tables are the standard atmosphere's.
visit_fields! {
    StandardProfile {
        std_sl_temperature, std_sl_density, std_sl_pressure, std_sl_soundspeed, temperature_bias,
        temperature_delta_gradient, gradient_fadeout_altitude,
    }
    dynamic {
        lapse_rates, pressure_breakpoints, std_pressure_breakpoints, std_density_breakpoints,
        std_lapse_rates,
    }
    skip { temperature_table, max_vapor_mass_fraction }
}
visit_fields! {
    Atmosphere {
        model, base, input, sl_temperature, sl_density, sl_pressure, sl_soundspeed, temperature,
        density, pressure, sound_speed, pressure_altitude, density_altitude, viscosity,
        kinematic_viscosity, std_day_sl_soundspeed, reng, vapor_mass_fraction,
        saturated_vapor_pressure, profile, override_nodes, orphaned_enabled, msis, sim_time,
    }
}
