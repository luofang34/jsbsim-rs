// SPDX-License-Identifier: LGPL-2.1-or-later
// Copyright (C) 1999 Jon S. Berndt (jon@jsbsim.org)
// Derived from JSBSim src/models/FGAerodynamics.{h,cpp}.
//
//! Aerodynamic forces and moments as sums of functions per axis, in the
//! wind, body, axial/normal or stability axis system.

mod bind;

pub use bind::bind;

use super::base::ModelBase;
use crate::Error;
use crate::fdm::LoadContext;
use crate::functions::FnId;
use crate::math::{ColumnVector3, E_DRAG, E_LIFT, E_X, E_Y, E_Z, Matrix33};
use crate::xml::Element;

/// `eAxisType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AxisType {
    /// Not determined yet.
    #[default]
    None,
    /// Body X Y Z.
    BodyXyz,
    /// Lift, side, drag.
    Wind,
    /// Axial, side, normal.
    BodyAxialNormal,
    /// Stability axes.
    Stability,
}

/// Inputs loaded by the executive.
#[derive(Debug, Clone, Copy, Default)]
pub struct Inputs {
    /// Angle of attack, rad.
    pub alpha: f64,
    /// Sideslip, rad.
    pub beta: f64,
    /// Dynamic pressure, psf.
    pub qbar: f64,
    /// True airspeed, ft/s.
    pub vt: f64,
    /// Body to wind.
    pub tb2w: Matrix33,
    /// Wind to body.
    pub tw2b: Matrix33,
    /// Aero reference point relative to the CG, body frame, ft.
    pub rp_body: ColumnVector3,
    /// Wing area, span, chord and incidence (`LoadModelConstants`).
    pub wing_area: f64,
    /// See `wing_area`.
    pub wing_span: f64,
    /// See `wing_area`.
    pub wing_chord: f64,
    /// See `wing_area`.
    pub wing_incidence: f64,
}

/// Function values of one frame, in evaluation order.
#[derive(Debug, Clone, Default)]
pub struct Values {
    /// Sum per force axis of the functions about the reference point.
    pub native: ColumnVector3,
    /// Sum per force axis of the functions applied at the CG.
    pub native_at_cg: ColumnVector3,
    /// `aero_ref_pt_shift_x` function value.
    pub rp_shift: Option<f64>,
    /// Sum per moment axis.
    pub moments_mrc: ColumnVector3,
}

/// `FGAerodynamics`.
#[derive(Debug, Clone, Default)]
pub struct Aerodynamics {
    /// Scheduling.
    pub base: ModelBase,
    /// Inputs of the current frame.
    pub input: Inputs,
    /// Axis system of the force functions.
    pub force_axis: AxisType,
    /// Axis system of the moment functions.
    pub moment_axis: AxisType,
    /// Functions per axis (`DRAG SIDE LIFT ROLL PITCH YAW`).
    pub functions: [Vec<FnId>; 6],
    /// Functions applied at the CG per axis.
    pub functions_at_cg: [Vec<FnId>; 6],
    /// `aero_ref_pt_shift_x`.
    pub rp_shift: Option<FnId>,
    /// State.
    pub state: State,
}

/// Values the model computes and publishes.
#[derive(Debug, Clone, Copy, Default)]
pub struct State {
    /// `alphaclmin0` / `alphaclmax0` from the file.
    pub alpha_cl_min0: f64,
    /// See `alpha_cl_min0`.
    pub alpha_cl_max0: f64,
    /// Current stall limits.
    pub alpha_cl_min: f64,
    /// See `alpha_cl_min`.
    pub alpha_cl_max: f64,
    /// Hysteresis limits.
    pub alpha_hyst_min: f64,
    /// See `alpha_hyst_min`.
    pub alpha_hyst_max: f64,
    /// Stall warning.
    pub impending_stall: f64,
    /// Stall hysteresis flag.
    pub stall_hyst: f64,
    /// CL squared.
    pub clsq: f64,
    /// Lift over drag.
    pub lod: f64,
    /// Wing angle of attack.
    pub alphaw: f64,
    /// Span and chord over twice the airspeed.
    pub bi2vel: f64,
    /// See `bi2vel`.
    pub ci2vel: f64,
    /// `aero/qbar-area`.
    pub qbar_area: f64,
    /// Reference point shift.
    pub delta_rp: ColumnVector3,
    /// Forces, body frame.
    pub forces: ColumnVector3,
    /// Moments about the CG, body frame.
    pub moments: ColumnVector3,
    /// Forces in wind axes (drag and lift positive backwards and up).
    pub fw: ColumnVector3,
    /// Moments about the reference point, native axes.
    pub moments_mrc: ColumnVector3,
    /// Stability to body.
    pub ts2b: Matrix33,
    /// Body to stability.
    pub tb2s: Matrix33,
}

fn axis_index(name: &str) -> Option<usize> {
    Some(match name {
        "DRAG" | "AXIAL" | "X" => 0,
        "SIDE" | "Y" => 1,
        "LIFT" | "NORMAL" | "Z" => 2,
        "ROLL" => 3,
        "PITCH" => 4,
        "YAW" => 5,
        _ => return None,
    })
}

fn process_frame(
    axis_type: &mut AxisType,
    frame: &str,
    el: &Element,
    valid: &str,
) -> Result<(), Error> {
    let wanted = match frame {
        "BODY" | "" => AxisType::BodyXyz,
        "STABILITY" => AxisType::Stability,
        "WIND" => AxisType::Wind,
        _ => {
            return Err(Error::Model {
                message: format!("{}: Unknown axis frame type of - {frame}", el.location()),
            });
        }
    };
    if *axis_type == AxisType::None {
        *axis_type = wanted;
    } else if *axis_type != wanted {
        tracing::error!(
            "{}: Mixed aerodynamic axis types have been specified in the aircraft config file.{valid}",
            el.location()
        );
    }
    Ok(())
}

impl Aerodynamics {
    /// `InitModel`.
    pub fn init_model(&mut self) {
        self.base.init();
        let s = &mut self.state;
        s.impending_stall = 0.0;
        s.stall_hyst = 0.0;
        s.alpha_cl_min = s.alpha_cl_min0;
        s.alpha_cl_max = s.alpha_cl_max0;
        s.alpha_hyst_min = 0.0;
        s.alpha_hyst_max = 0.0;
        s.clsq = 0.0;
        s.lod = 0.0;
        s.alphaw = 0.0;
        s.bi2vel = 0.0;
        s.ci2vel = 0.0;
        self.rp_shift = None;
        s.delta_rp = ColumnVector3::zero();
        s.forces = ColumnVector3::zero();
        s.moments = ColumnVector3::zero();
    }

    /// `DetermineAxisSystem`.
    fn determine_axis_system(&mut self, document: &Element) -> Result<(), Error> {
        for axis_el in document.find_elements("axis") {
            let axis = axis_el.attribute("name");
            let frame = axis_el.attribute("frame");
            match axis {
                "X" | "Y" | "Z" => process_frame(&mut self.force_axis, frame, axis_el, "(X Y Z)")?,
                "ROLL" | "PITCH" | "YAW" => {
                    process_frame(&mut self.moment_axis, frame, axis_el, "(ROLL PITCH YAW)")?
                }
                "LIFT" | "DRAG" => match self.force_axis {
                    AxisType::None => self.force_axis = AxisType::Wind,
                    AxisType::Wind => {}
                    _ => tracing::error!(
                        "{}: Mixed aerodynamic axis types (LIFT DRAG)",
                        axis_el.location()
                    ),
                },
                "SIDE" => {
                    if !matches!(
                        self.force_axis,
                        AxisType::None | AxisType::Wind | AxisType::BodyAxialNormal
                    ) {
                        tracing::error!(
                            "{}: Mixed aerodynamic axis types (SIDE)",
                            axis_el.location()
                        );
                    }
                }
                "AXIAL" | "NORMAL" => match self.force_axis {
                    AxisType::None => self.force_axis = AxisType::BodyAxialNormal,
                    AxisType::BodyAxialNormal => {}
                    _ => tracing::error!(
                        "{}: Mixed aerodynamic axis types (NORMAL AXIAL)",
                        axis_el.location()
                    ),
                },
                _ => {
                    return Err(Error::Model {
                        message: format!(
                            "{}: An unknown axis type, {axis} has been specified in the aircraft configuration file.",
                            axis_el.location()
                        ),
                    });
                }
            }
        }
        if self.force_axis == AxisType::None {
            self.force_axis = AxisType::Wind;
        }
        if self.moment_axis == AxisType::None {
            self.moment_axis = AxisType::BodyXyz;
        }
        Ok(())
    }

    /// `Load(<aerodynamics>)`, after the executive's `Upload`.
    pub fn load(&mut self, document: &Element, ctx: &mut LoadContext<'_>) -> Result<(), Error> {
        self.determine_axis_system(document)?;
        let limits = |name: &str| -> Result<Option<(f64, f64)>, Error> {
            let Some(el) = document.find_element(name) else {
                return Ok(None);
            };
            let unit = match el.attribute("unit") {
                "" => "RAD",
                u => u,
            };
            Ok(Some((
                el.find_element_value_as_number_convert_from_to("min", unit, "RAD")?,
                el.find_element_value_as_number_convert_from_to("max", unit, "RAD")?,
            )))
        };
        if let Some((min, max)) = limits("alphalimits")? {
            let s = &mut self.state;
            (
                s.alpha_cl_min0,
                s.alpha_cl_max0,
                s.alpha_cl_min,
                s.alpha_cl_max,
            ) = (min, max, min, max);
        }
        if let Some((min, max)) = limits("hysteresis_limits")? {
            (self.state.alpha_hyst_min, self.state.alpha_hyst_max) = (min, max);
        }
        if let Some(shift) = document.find_element("aero_ref_pt_shift_x")
            && let Some(f) = shift.find_element("function")
        {
            self.rp_shift = Some(ctx.functions.load_function(ctx.binder.tree, f, "")?);
        }
        for axis_el in document.find_elements("axis") {
            let (mut ca, mut at_cg) = (Vec::new(), Vec::new());
            for f in axis_el.find_elements("function") {
                let id = ctx.functions.load_function(ctx.binder.tree, f, "")?;
                if f.attribute("apply_at_cg") == "true" {
                    at_cg.push(id);
                } else {
                    ca.push(id);
                }
            }
            let slots = axis_index(axis_el.attribute("name")).and_then(|i| {
                self.functions
                    .get_mut(i)
                    .zip(self.functions_at_cg.get_mut(i))
            });
            if let Some((ca_slot, at_cg_slot)) = slots {
                *ca_slot = ca;
                *at_cg_slot = at_cg;
            }
        }
        Ok(())
    }

    /// The part of `Run` before the functions are evaluated.
    pub fn begin_frame(&mut self) {
        let i = &self.input;
        let s = &mut self.state;
        if i.qbar > 1.0 {
            s.clsq = s.fw[E_LIFT] / (i.wing_area * i.qbar);
            s.clsq *= s.clsq;
        }
    }

    /// The part of `Run` after the pre-functions, up to the evaluation.
    pub fn prepare(&mut self) {
        let i = &self.input;
        let s = &mut self.state;
        let twovel = 2.0 * i.vt;
        if twovel != 0.0 {
            s.bi2vel = i.wing_span / twovel;
            s.ci2vel = i.wing_chord / twovel;
        }
        s.alphaw = i.alpha + i.wing_incidence;
        s.qbar_area = i.wing_area * i.qbar;
        if s.alpha_cl_max != 0.0 {
            s.impending_stall = if i.alpha > 0.85 * s.alpha_cl_max {
                10.0 * (i.alpha / s.alpha_cl_max - 0.85)
            } else {
                0.0
            };
        }
        if s.alpha_hyst_max != 0.0 && s.alpha_hyst_min != 0.0 {
            if i.alpha > s.alpha_hyst_max {
                s.stall_hyst = 1.0;
            } else if i.alpha < s.alpha_hyst_min {
                s.stall_hyst = 0.0;
            }
        }
        s.fw = ColumnVector3::zero();
        let (sa, ca) = libm::sincos(i.alpha);
        s.ts2b = Matrix33::new(ca, 0.0, -sa, 0.0, 1.0, 0.0, sa, 0.0, ca);
        s.tb2s = s.ts2b.transposed();
    }

    fn to_body(&self, axis: AxisType, mut v: ColumnVector3) -> Result<ColumnVector3, Error> {
        let i = &self.input;
        Ok(match axis {
            AxisType::BodyXyz => v,
            AxisType::Wind => {
                v[E_DRAG] *= -1.0;
                v[E_LIFT] *= -1.0;
                i.tw2b * v
            }
            AxisType::BodyAxialNormal => {
                v[E_X] *= -1.0;
                v[E_Z] *= -1.0;
                v
            }
            AxisType::Stability => {
                v[E_DRAG] *= -1.0;
                v[E_LIFT] *= -1.0;
                self.state.ts2b * v
            }
            AxisType::None => {
                return Err(Error::Model {
                    message: "A proper axis type has NOT been selected. Check your aerodynamics definition.".to_owned(),
                });
            }
        })
    }

    /// The rest of `Run`, from the function values.
    pub fn finish(&mut self, values: &Values) -> Result<(), Error> {
        let forces = self.to_body(self.force_axis, values.native)?;
        let forces_at_cg = self.to_body(self.force_axis, values.native_at_cg)?;
        if let Some(shift) = values.rp_shift {
            self.state.delta_rp[E_X] = shift * self.input.wing_chord;
        }
        let (rp, d) = (self.input.rp_body, self.state.delta_rp);
        let dxyz_cg = ColumnVector3::new(rp[E_X] - d[E_X], rp[E_Y] + d[E_Y], rp[E_Z] - d[E_Z]);
        self.state.moments_mrc = values.moments_mrc;
        let mrc_body = match self.moment_axis {
            AxisType::BodyXyz => values.moments_mrc,
            AxisType::Stability => self.state.ts2b * values.moments_mrc,
            AxisType::Wind => self.input.tw2b * values.moments_mrc,
            _ => {
                return Err(Error::Model {
                    message: "A proper axis type has NOT been selected. Check your aerodynamics definition.".to_owned(),
                });
            }
        };
        let s = &mut self.state;
        s.moments = mrc_body + dxyz_cg.cross(forces);
        s.forces = forces + forces_at_cg;
        s.fw = self.input.tb2w * s.forces;
        s.fw[E_DRAG] *= -1.0;
        s.fw[E_LIFT] *= -1.0;
        if s.fw[E_DRAG].abs() > 0.0 {
            s.lod = (s.fw[E_LIFT] / s.fw[E_DRAG]).abs();
        }
        Ok(())
    }
}
