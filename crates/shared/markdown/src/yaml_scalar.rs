use crate::frontmatter::PropertyValue;

#[must_use]
pub(crate) fn is_null(text: &str) -> bool {
    matches!(text, "" | "~" | "null" | "Null" | "NULL")
}

#[must_use]
pub(crate) fn bool_value(text: &str) -> Option<bool> {
    match text {
        "true" | "True" | "TRUE" => Some(true),
        "false" | "False" | "FALSE" => Some(false),
        _ => None,
    }
}

fn digits(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit())
}

fn radix_value(text: &str, radix: u32) -> Option<f64> {
    if text.is_empty() {
        return None;
    }
    let mut value = 0.0;
    for ch in text.chars() {
        let digit = ch.to_digit(radix)?;
        value = value * f64::from(radix) + f64::from(digit);
    }
    Some(value)
}

fn decimal(text: &str) -> bool {
    let body = text.strip_prefix(['-', '+']).unwrap_or(text);
    let (mantissa, exponent) = match body.find(['e', 'E']) {
        Some(at) => (
            body.get(..at).unwrap_or_default(),
            body.get(at + 1..).unwrap_or_default(),
        ),
        None => (body, ""),
    };
    if !exponent.is_empty() && !digits(exponent.strip_prefix(['-', '+']).unwrap_or(exponent)) {
        return false;
    }
    if body.find(['e', 'E']).is_some() && exponent.is_empty() {
        return false;
    }
    match mantissa.split_once('.') {
        Some((head, tail)) => {
            (head.is_empty() || digits(head))
                && (tail.is_empty() || digits(tail))
                && !(head.is_empty() && tail.is_empty())
        }
        None => digits(mantissa),
    }
}

#[must_use]
pub(crate) fn parse_number(text: &str) -> Option<f64> {
    match text {
        ".inf" | ".Inf" | ".INF" | "+.inf" | "+.Inf" | "+.INF" => return Some(f64::INFINITY),
        "-.inf" | "-.Inf" | "-.INF" => return Some(f64::NEG_INFINITY),
        ".nan" | ".NaN" | ".NAN" => return Some(f64::NAN),
        _ => {}
    }
    if let Some(rest) = text.strip_prefix("0o") {
        return radix_value(rest, 8);
    }
    if let Some(rest) = text.strip_prefix("0x") {
        return radix_value(rest, 16);
    }
    if decimal(text) {
        return text.parse::<f64>().ok();
    }
    None
}

#[must_use]
pub(crate) fn plain_scalar(text: &str) -> PropertyValue {
    if is_null(text) {
        return PropertyValue::Null;
    }
    if let Some(value) = bool_value(text) {
        return PropertyValue::Bool(value);
    }
    if let Some(value) = parse_number(text) {
        return PropertyValue::Number(value);
    }
    PropertyValue::Text(text.to_owned())
}
