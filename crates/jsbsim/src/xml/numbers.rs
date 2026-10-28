// SPDX-License-Identifier: LGPL-2.1-or-later
// string_utilities: Copyright (C) 2022 Bertrand Coconnier, Sean McLeod.
// Derived from JSBSim src/input_output/string_utilities.cpp (atof_locale_c).
//
//! Strict decimal number parsing for XML values.

use crate::c_numeric::{is_c_space, strtod_prefix};

/// `atof_locale_c`: accepts only `\s*[+-]?(\d+(\.\d*)?|\.\d+)([eE][+-]?\d+)?\s*`
/// and rejects overflow; the error is JSBSim's message.
pub fn atof_locale_c(input: &str) -> Result<f64, String> {
    let bytes = input.as_bytes();
    if bytes.iter().all(|&b| is_c_space(b)) {
        return Err("Expecting a numeric attribute value, but only got spaces".to_owned());
    }
    if !matches_number_format(bytes) {
        return Err(format!(
            "Expecting a numeric attribute value, but got: {input}"
        ));
    }
    let value = strtod_prefix(input).0;
    if value.is_infinite() {
        return Err(format!("This number is too large: {input}"));
    }
    Ok(value)
}

fn matches_number_format(s: &[u8]) -> bool {
    let tail = |from: usize| s.get(from..).unwrap_or_default();
    let digits = |from: usize| tail(from).iter().take_while(|b| b.is_ascii_digit()).count();
    let mut i = s.iter().take_while(|&&b| is_c_space(b)).count();
    if matches!(s.get(i), Some(b'+' | b'-')) {
        i += 1;
    }
    let int_digits = digits(i);
    i += int_digits;
    if s.get(i) == Some(&b'.') {
        let frac_digits = digits(i + 1);
        if int_digits == 0 && frac_digits == 0 {
            return false;
        }
        i += 1 + frac_digits;
    } else if int_digits == 0 {
        return false;
    }
    if matches!(s.get(i), Some(b'e' | b'E')) {
        let mut j = i + 1;
        if matches!(s.get(j), Some(b'+' | b'-')) {
            j += 1;
        }
        let exp_digits = digits(j.min(s.len()));
        if exp_digits == 0 {
            return false;
        }
        i = j + exp_digits;
    }
    tail(i).iter().all(|&b| is_c_space(b))
}
