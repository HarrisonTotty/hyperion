//! What the tasks' `render()` functions share: writing a float as a Rust literal.

/// `value` as a Rust float literal: the shortest decimal that reads back to the same `f64`, with
/// its digits grouped in threes as Clippy asks. A negative value keeps its sign in front of the
/// grouped digits.
#[must_use]
pub(crate) fn literal(value: f64) -> String {
    let shortest = format!("{value:?}");
    let (sign, unsigned) = match shortest.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", shortest.as_str()),
    };
    let (mantissa, exponent) = match unsigned.split_once('e') {
        Some((m, e)) => (m, Some(e)),
        None => (unsigned, None),
    };
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, "0"));
    let mut out = String::from(sign);
    let digits: Vec<char> = whole.chars().collect();
    for (i, c) in digits.iter().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push('_');
        }
        out.push(*c);
    }
    out.push('.');
    for (i, c) in fraction.chars().enumerate() {
        if i > 0 && i.is_multiple_of(3) {
            out.push('_');
        }
        out.push(c);
    }
    if let Some(e) = exponent {
        out.push('e');
        out.push_str(e);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literals_read_back_to_the_same_value_and_are_grouped() {
        for (value, expected) in [
            (0.04, "0.04"),
            (3.5, "3.5"),
            (1.0, "1.0"),
            (0.0, "0.0"),
            (1_234.567_8, "1_234.567_8"),
            (0.001_234_567_891_234_5, "0.001_234_567_891_234_5"),
            (1.234_567e-7, "1.234_567e-7"),
            (-5.868_123_4, "-5.868_123_4"),
            (-123.5, "-123.5"),
            (-1_234.5, "-1_234.5"),
            (-2.5e-9, "-2.5e-9"),
        ] {
            let text = literal(value);
            assert_eq!(text, expected);
            let back: f64 = text.replace('_', "").parse().unwrap();
            assert!(back.total_cmp(&value).is_eq(), "{text}");
        }
    }
}
