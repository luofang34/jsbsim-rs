// SPDX-License-Identifier: LGPL-2.1-or-later
//! A small evaluator of ANSI/AIAA S-119 (DAVE-ML) models, independent of
//! JSBSim, and the models' checkData static shots. It covers only what
//! F16_control.dml, F16_gnc.dml and F16_inertia.dml use: variableDef
//! calculations in a MathML subset (plus, minus, times, divide, power,
//! abs, sin, cos, the atan2 csymbol, comparisons and piecewise), initial
//! values and minValue/maxValue clips.

use std::collections::BTreeMap;

use jsbsim::oracle_support::xml::Element;

use crate::OracleError;

/// A variableDef.
#[derive(Debug, Clone)]
struct Variable {
    initial: Option<f64>,
    min: Option<f64>,
    max: Option<f64>,
    /// The first child of the calculation's `math` element.
    calculation: Option<Element>,
}

/// The trigonometric functions a model is evaluated with: the maths
/// library of the JSBSim run it is compared with, so that a difference is
/// the translation's and not the last digit of a sine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Maths {
    /// The platform's, as C++ JSBSim calls.
    Platform,
    /// The `libm` crate's, as the port calls.
    LibmCrate,
}

impl Maths {
    fn sin(self, x: f64) -> f64 {
        match self {
            Self::Platform => x.sin(),
            Self::LibmCrate => libm::sin(x),
        }
    }

    fn cos(self, x: f64) -> f64 {
        match self {
            Self::Platform => x.cos(),
            Self::LibmCrate => libm::cos(x),
        }
    }

    fn atan2(self, y: f64, x: f64) -> f64 {
        match self {
            Self::Platform => y.atan2(x),
            Self::LibmCrate => libm::atan2(y, x),
        }
    }
}

/// The variables of one DAVE-ML model.
#[derive(Debug, Clone)]
pub struct Model {
    name: String,
    variables: BTreeMap<String, Variable>,
    maths: Maths,
}

/// Nesting deeper than any acyclic model of this size reaches.
const MAX_DEPTH: usize = 512;

fn number(source: &str, text: &str) -> Result<f64, OracleError> {
    text.trim().parse::<f64>().map_err(|e| OracleError::DaveMl {
        source_name: source.to_owned(),
        message: format!("{text:?}: {e}"),
    })
}

fn optional_number(source: &str, e: &Element, key: &str) -> Result<Option<f64>, OracleError> {
    if e.has_attribute(key) {
        number(source, e.attribute(key)).map(Some)
    } else {
        Ok(None)
    }
}

impl Model {
    /// Reads the variableDefs of the DAVEfunc `root`; `name` labels errors.
    pub fn from_element(name: &str, root: &Element) -> Result<Self, OracleError> {
        let mut variables = BTreeMap::new();
        for v in root.find_elements("variableDef") {
            let calculation = v
                .find_element("calculation")
                .and_then(|c| c.find_element("math"))
                .and_then(|m| m.children().first())
                .cloned();
            variables.insert(
                v.attribute("varID").to_owned(),
                Variable {
                    initial: optional_number(name, v, "initialValue")?,
                    min: optional_number(name, v, "minValue")?,
                    max: optional_number(name, v, "maxValue")?,
                    calculation,
                },
            );
        }
        Ok(Self {
            name: name.to_owned(),
            variables,
            maths: Maths::Platform,
        })
    }

    /// The model evaluated with `maths`.
    #[must_use]
    pub fn with_maths(mut self, maths: Maths) -> Self {
        self.maths = maths;
        self
    }

    /// Whether the model defines `id`.
    #[must_use]
    pub fn has(&self, id: &str) -> bool {
        self.variables.contains_key(id)
    }

    /// Whether `id` takes a given value: no calculation.
    #[must_use]
    pub fn is_settable(&self, id: &str) -> bool {
        self.variables
            .get(id)
            .is_some_and(|v| v.calculation.is_none())
    }

    /// Every variable, with `given` overriding inputs and initial values.
    pub fn evaluate(
        &self,
        given: &BTreeMap<String, f64>,
    ) -> Result<BTreeMap<String, f64>, OracleError> {
        let mut values = given.clone();
        for id in self.variables.keys() {
            self.get(id, &mut values, 0)?;
        }
        Ok(values)
    }

    fn error(&self, message: String) -> OracleError {
        OracleError::DaveMl {
            source_name: self.name.clone(),
            message,
        }
    }

    fn get(
        &self,
        id: &str,
        values: &mut BTreeMap<String, f64>,
        depth: usize,
    ) -> Result<f64, OracleError> {
        if let Some(&v) = values.get(id) {
            return Ok(v);
        }
        if depth > MAX_DEPTH {
            return Err(self.error(format!("{id}: definitions nest too deep (a cycle?)")));
        }
        let var = self
            .variables
            .get(id)
            .ok_or_else(|| self.error(format!("{id} is not defined")))?;
        let mut x = match (&var.calculation, var.initial) {
            (Some(c), _) => self.eval(c, values, depth + 1)?,
            (None, Some(v)) => v,
            (None, None) => return Err(self.error(format!("input {id} not given"))),
        };
        if let Some(min) = var.min {
            x = x.max(min);
        }
        if let Some(max) = var.max {
            x = x.min(max);
        }
        values.insert(id.to_owned(), x);
        Ok(x)
    }

    fn eval(
        &self,
        e: &Element,
        values: &mut BTreeMap<String, f64>,
        depth: usize,
    ) -> Result<f64, OracleError> {
        match e.name() {
            "ci" => self.get(e.data_line(0), values, depth),
            "cn" => number(&self.name, e.data_line(0)),
            "piecewise" => self.piecewise(e, values, depth),
            "apply" => self.apply(e, values, depth),
            other => Err(self.error(format!("element <{other}> is not supported"))),
        }
    }

    fn piecewise(
        &self,
        e: &Element,
        values: &mut BTreeMap<String, f64>,
        depth: usize,
    ) -> Result<f64, OracleError> {
        for p in e.children() {
            let parts = p.children();
            match (p.name(), parts) {
                ("piece", [value, condition]) => {
                    if self.eval(condition, values, depth)? != 0.0 {
                        return self.eval(value, values, depth);
                    }
                }
                ("otherwise", [value]) => return self.eval(value, values, depth),
                (name, _) => return Err(self.error(format!("malformed <{name}> in piecewise"))),
            }
        }
        Err(self.error("piecewise without a matching piece".to_owned()))
    }

    fn apply(
        &self,
        e: &Element,
        values: &mut BTreeMap<String, f64>,
        depth: usize,
    ) -> Result<f64, OracleError> {
        let Some((op, args)) = e.children().split_first() else {
            return Err(self.error("empty <apply>".to_owned()));
        };
        // An <apply> wrapping a single expression, as the F-16 models
        // wrap their piecewise switches, is that expression.
        if args.is_empty() && matches!(op.name(), "piecewise" | "apply" | "ci" | "cn") {
            return self.eval(op, values, depth);
        }
        let mut a = Vec::with_capacity(args.len());
        for arg in args {
            a.push(self.eval(arg, values, depth)?);
        }
        let truth = |b: bool| if b { 1.0 } else { 0.0 };
        let name = if op.name() == "csymbol" {
            op.data_line(0)
        } else {
            op.name()
        };
        Ok(match (name, a.as_slice()) {
            ("plus", _) => a.iter().sum(),
            ("minus", [x]) => -x,
            ("minus", [x, y]) => x - y,
            ("times", _) => a.iter().product(),
            ("divide", [x, y]) => x / y,
            ("power", [x, y]) => x.powf(*y),
            ("abs", [x]) => x.abs(),
            ("sin", [x]) => self.maths.sin(*x),
            ("cos", [x]) => self.maths.cos(*x),
            ("atan2", [y, x]) => self.maths.atan2(*y, *x),
            ("gt", [x, y]) => truth(x > y),
            ("lt", [x, y]) => truth(x < y),
            ("geq", [x, y]) => truth(x >= y),
            ("leq", [x, y]) => truth(x <= y),
            ("eq", [x, y]) => truth(x == y),
            (other, _) => {
                return Err(self.error(format!(
                    "operator {other} with {} arguments is not supported",
                    a.len()
                )));
            }
        })
    }
}

/// One checkData staticShot.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Shot {
    /// The shot's name.
    pub name: String,
    /// checkInputs by signal name.
    pub inputs: BTreeMap<String, f64>,
    /// checkOutputs by signal name: value and tolerance.
    pub outputs: BTreeMap<String, (f64, f64)>,
    /// internalValues by varID.
    pub internal: BTreeMap<String, f64>,
}

fn signals<'a>(shot: &'a Element, group: &str) -> impl Iterator<Item = &'a Element> + 'a {
    shot.find_element(group)
        .into_iter()
        .flat_map(|g| g.find_elements("signal"))
}

/// The staticShots of the DAVEfunc `root`; `name` labels errors.
pub fn shots(name: &str, root: &Element) -> Result<Vec<Shot>, OracleError> {
    let Some(check) = root.find_element("checkData") else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for s in check.find_elements("staticShot") {
        let mut shot = Shot {
            name: s.attribute("name").to_owned(),
            ..Shot::default()
        };
        for sig in signals(s, "checkInputs") {
            let value = number(name, sig.find_element_value("signalValue"))?;
            shot.inputs
                .insert(sig.find_element_value("signalName").to_owned(), value);
        }
        for sig in signals(s, "checkOutputs") {
            let value = number(name, sig.find_element_value("signalValue"))?;
            let tol = match sig.find_element("tol") {
                Some(t) => number(name, t.data_line(0))?,
                None => 0.0,
            };
            shot.outputs.insert(
                sig.find_element_value("signalName").to_owned(),
                (value, tol),
            );
        }
        for sig in signals(s, "internalValues") {
            let value = number(name, sig.find_element_value("signalValue"))?;
            shot.internal
                .insert(sig.find_element_value("varID").to_owned(), value);
        }
        out.push(shot);
    }
    Ok(out)
}

#[cfg(test)]
mod tests;
