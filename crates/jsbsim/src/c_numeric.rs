// SPDX-License-Identifier: LGPL-2.1-or-later
//! Number parsing and formatting with the semantics of the C library
//! functions JSBSim and SimGear call: `strtod`, `strtol`, `atoi` and the
//! `printf` conversions `%g` and `%f`, in the "C" locale.

#[cfg(test)]
mod tests;

/// Whether `b` is a space in the "C" locale (`isspace`).
#[must_use]
pub fn is_c_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

fn skip_spaces(s: &[u8]) -> usize {
    s.iter().take_while(|&&b| is_c_space(b)).count()
}

/// `strtod(s, &end)`: the parsed value and the number of bytes consumed,
/// which is 0 when no conversion could be performed.
#[must_use]
pub fn strtod_prefix(s: &str) -> (f64, usize) {
    let bytes = s.as_bytes();
    let start = skip_spaces(bytes);
    let mut i = start;
    let negative = match bytes.get(i) {
        Some(b'-') => {
            i += 1;
            true
        }
        Some(b'+') => {
            i += 1;
            false
        }
        _ => false,
    };
    let signed = |v: f64| if negative { -v } else { v };
    let rest = tail(bytes, i);
    if let Some(len) = special_len(rest, b"infinity").or_else(|| special_len(rest, b"inf")) {
        return (signed(f64::INFINITY), i + len);
    }
    if special_len(rest, b"nan").is_some() {
        let mut end = i + 3;
        if bytes.get(end) == Some(&b'(') {
            let inner = tail(bytes, end + 1)
                .iter()
                .take_while(|b| b.is_ascii_alphanumeric() || **b == b'_')
                .count();
            if bytes.get(end + 1 + inner) == Some(&b')') {
                end += inner + 2;
            }
        }
        return (signed(f64::NAN), end);
    }
    if let Some((&[b'0', x], hex)) = rest.split_first_chunk::<2>()
        && (x | 0x20) == b'x'
        && let Some((value, len)) = parse_hex(hex)
    {
        return (signed(value), i + 2 + len);
    }
    match decimal_len(rest) {
        0 => (0.0, 0),
        len => {
            // The slice holds only ASCII digits, '.', 'e' and signs, which
            // Rust parses with the same correct rounding as strtod.
            let text = std::str::from_utf8(rest.get(..len).unwrap_or_default()).unwrap_or("0");
            (signed(text.parse::<f64>().unwrap_or(0.0)), i + len)
        }
    }
}

/// The bytes of `s` from `i`; empty past its end.
fn tail(s: &[u8], i: usize) -> &[u8] {
    s.get(i..).unwrap_or_default()
}

/// `strtod(s, 0)` / `atof(s)`.
#[must_use]
pub fn strtod(s: &str) -> f64 {
    strtod_prefix(s).0
}

fn special_len(s: &[u8], word: &[u8]) -> Option<usize> {
    s.get(..word.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(word))
        .then_some(word.len())
}

/// Length of the longest decimal floating-point prefix of `s` (no sign).
fn decimal_len(s: &[u8]) -> usize {
    let int_digits = s.iter().take_while(|b| b.is_ascii_digit()).count();
    let mut len = int_digits;
    let mut frac_digits = 0;
    if s.get(len) == Some(&b'.') {
        frac_digits = tail(s, len + 1)
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .count();
        if int_digits + frac_digits > 0 {
            len += 1 + frac_digits;
        }
    }
    if int_digits + frac_digits == 0 {
        return 0;
    }
    if matches!(s.get(len), Some(b'e' | b'E')) {
        let mut j = len + 1;
        if matches!(s.get(j), Some(b'+' | b'-')) {
            j += 1;
        }
        let exp_digits = tail(s, j).iter().take_while(|b| b.is_ascii_digit()).count();
        if exp_digits > 0 {
            len = j + exp_digits;
        }
    }
    len
}

/// Hexadecimal floating constant after the `0x` prefix: the value and the
/// bytes consumed, or `None` when no hex digit follows the prefix.
fn parse_hex(s: &[u8]) -> Option<(f64, usize)> {
    let mut mantissa: u64 = 0;
    let mut sticky = false;
    let mut exponent: i64 = 0;
    let mut digits = 0;
    let mut i = 0;
    let mut seen_point = false;
    while let Some(&b) = s.get(i) {
        if b == b'.' && !seen_point {
            seen_point = true;
        } else if let Some(d) = (b as char).to_digit(16) {
            digits += 1;
            if mantissa >> 60 == 0 {
                mantissa = (mantissa << 4) | u64::from(d);
                if seen_point {
                    exponent -= 4;
                }
            } else {
                sticky |= d != 0;
                if !seen_point {
                    exponent += 4;
                }
            }
        } else {
            break;
        }
        i += 1;
    }
    if digits == 0 {
        return None;
    }
    if matches!(s.get(i), Some(b'p' | b'P')) {
        let mut j = i + 1;
        let negative = s.get(j) == Some(&b'-');
        if matches!(s.get(j), Some(b'+' | b'-')) {
            j += 1;
        }
        let exp_digits = tail(s, j).iter().take_while(|b| b.is_ascii_digit()).count();
        if exp_digits > 0 {
            let mut e: i64 = 0;
            for &b in s.get(j..j + exp_digits).unwrap_or_default() {
                e = (e * 10 + i64::from(b - b'0')).min(100_000);
            }
            exponent += if negative { -e } else { e };
            i = j + exp_digits;
        }
    }
    if sticky {
        // Below the rounding position of a 53-bit significand, so it only
        // breaks ties the way the discarded digits would.
        mantissa |= 1;
    }
    let exponent = i32::try_from(exponent.clamp(-100_000, 100_000)).unwrap_or(0);
    Some((ldexp(mantissa as f64, exponent), i))
}

/// `x * 2^e` with a single rounding for normal results.
fn ldexp(x: f64, e: i32) -> f64 {
    libm::scalbn(x, e)
}

/// `strtol(s, 0, base)` for base 0 (C prefixes) or 10, saturating like C.
#[must_use]
pub fn strtol(s: &str, base: u32) -> i64 {
    let bytes = s.as_bytes();
    let mut i = skip_spaces(bytes);
    let negative = match bytes.get(i) {
        Some(b'-') => {
            i += 1;
            true
        }
        Some(b'+') => {
            i += 1;
            false
        }
        _ => false,
    };
    let mut radix = base;
    if base == 0 || base == 16 {
        let hex_prefix = bytes.get(i) == Some(&b'0')
            && matches!(bytes.get(i + 1), Some(b'x' | b'X'))
            && bytes.get(i + 2).is_some_and(u8::is_ascii_hexdigit);
        if hex_prefix {
            i += 2;
            radix = 16;
        } else if base == 0 {
            radix = if bytes.get(i) == Some(&b'0') { 8 } else { 10 };
        }
    }
    let mut magnitude: u64 = 0;
    let mut overflow = false;
    while let Some(d) = bytes.get(i).and_then(|&b| (b as char).to_digit(radix)) {
        match magnitude
            .checked_mul(u64::from(radix))
            .and_then(|m| m.checked_add(u64::from(d)))
        {
            Some(m) => magnitude = m,
            None => overflow = true,
        }
        i += 1;
    }
    if negative {
        if overflow || magnitude > i64::MIN.unsigned_abs() {
            i64::MIN
        } else {
            0i64.wrapping_sub_unsigned(magnitude)
        }
    } else if overflow || magnitude > i64::MAX as u64 {
        i64::MAX
    } else {
        magnitude as i64
    }
}

/// `atoi(s)`, which C libraries implement as `(int)strtol(s, 0, 10)`.
#[must_use]
pub fn atoi(s: &str) -> i32 {
    strtol(s, 10) as i32
}

fn special(v: f64) -> Option<String> {
    if v.is_nan() {
        Some("nan".to_owned())
    } else if v.is_infinite() {
        Some(if v < 0.0 { "-inf" } else { "inf" }.to_owned())
    } else {
        None
    }
}

/// `printf("%f", v)`.
#[must_use]
pub fn format_f(v: f64) -> String {
    special(v).unwrap_or_else(|| format!("{v:.6}"))
}

/// `printf("%.<precision>g", v)`, which is also what an `ostream` with
/// `setprecision(precision)` and default float formatting prints.
#[must_use]
pub fn format_g(v: f64, precision: usize) -> String {
    if let Some(text) = special(v) {
        return text;
    }
    let p = precision.max(1);
    let scientific = format!("{v:.*e}", p - 1);
    let (mantissa, exp) = scientific.split_once('e').unwrap_or((&scientific, "0"));
    let x: i64 = exp.parse().unwrap_or(0);
    if x < -4 || x >= p as i64 {
        let mantissa = strip_fraction_zeros(mantissa);
        let sign = if x < 0 { '-' } else { '+' };
        format!("{mantissa}e{sign}{:02}", x.unsigned_abs())
    } else {
        let decimals = usize::try_from(p as i64 - 1 - x).unwrap_or(0);
        strip_fraction_zeros(&format!("{v:.decimals$}")).to_owned()
    }
}

fn strip_fraction_zeros(s: &str) -> &str {
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.')
    } else {
        s
    }
}
