// SPDX-License-Identifier: LGPL-2.1-or-later
// props.cxx: Started Fall 2000 by David Megginson, david@megginson.com.
// This code is released into the Public Domain.
// Derived from JSBSim src/simgear/props/props.cxx (typed setters).
//
//! How a write of one type lands in a node of another: C casts between
//! numbers, `%d`/`%ld`/`%f` into strings, and C parsing out of them.

use super::Value;
use crate::c_numeric::{atoi, format_f, strtod, strtol};

/// What a typed write stores into a node of each type.
pub(super) trait Written: Copy {
    fn into_value(self) -> Value;
    fn to_bool(self) -> bool;
    fn to_int(self) -> i32;
    fn to_long(self) -> i64;
    fn to_float(self) -> f32;
    fn to_double(self) -> f64;
    fn to_text(self) -> String;
}

impl Written for bool {
    fn into_value(self) -> Value {
        Value::Bool(self)
    }
    fn to_bool(self) -> bool {
        self
    }
    fn to_int(self) -> i32 {
        i32::from(self)
    }
    fn to_long(self) -> i64 {
        i64::from(self)
    }
    fn to_float(self) -> f32 {
        f32::from(u8::from(self))
    }
    fn to_double(self) -> f64 {
        f64::from(u8::from(self))
    }
    fn to_text(self) -> String {
        if self { "true" } else { "false" }.to_owned()
    }
}

impl Written for i32 {
    fn into_value(self) -> Value {
        Value::Int(self)
    }
    fn to_bool(self) -> bool {
        self != 0
    }
    fn to_int(self) -> i32 {
        self
    }
    fn to_long(self) -> i64 {
        i64::from(self)
    }
    fn to_float(self) -> f32 {
        self as f32
    }
    fn to_double(self) -> f64 {
        f64::from(self)
    }
    fn to_text(self) -> String {
        self.to_string()
    }
}

impl Written for i64 {
    fn into_value(self) -> Value {
        Value::Long(self)
    }
    fn to_bool(self) -> bool {
        self != 0
    }
    fn to_int(self) -> i32 {
        self as i32
    }
    fn to_long(self) -> i64 {
        self
    }
    fn to_float(self) -> f32 {
        self as f32
    }
    fn to_double(self) -> f64 {
        self as f64
    }
    fn to_text(self) -> String {
        self.to_string()
    }
}

impl Written for f32 {
    fn into_value(self) -> Value {
        Value::Float(self)
    }
    fn to_bool(self) -> bool {
        self != 0.0
    }
    fn to_int(self) -> i32 {
        self as i32
    }
    fn to_long(self) -> i64 {
        self as i64
    }
    fn to_float(self) -> f32 {
        self
    }
    fn to_double(self) -> f64 {
        f64::from(self)
    }
    fn to_text(self) -> String {
        format_f(f64::from(self))
    }
}

impl Written for f64 {
    fn into_value(self) -> Value {
        Value::Double(self)
    }
    fn to_bool(self) -> bool {
        self != 0.0
    }
    fn to_int(self) -> i32 {
        self as i32
    }
    fn to_long(self) -> i64 {
        self as i64
    }
    fn to_float(self) -> f32 {
        self as f32
    }
    fn to_double(self) -> f64 {
        self
    }
    fn to_text(self) -> String {
        format_f(self)
    }
}

/// Text written into a typed node, parsed as `setStringValue` does.
pub(super) fn parse_into(current: &Value, text: &str) -> Value {
    match current {
        Value::Bool(_) => Value::Bool(text == "true" || atoi(text) != 0),
        Value::Int(_) => Value::Int(atoi(text)),
        Value::Long(_) => Value::Long(strtol(text, 0)),
        Value::Float(_) => Value::Float(strtod(text) as f32),
        Value::Double(_) => Value::Double(strtod(text)),
        Value::Unspecified(_) => Value::Unspecified(text.to_owned()),
        _ => Value::String(text.to_owned()),
    }
}
