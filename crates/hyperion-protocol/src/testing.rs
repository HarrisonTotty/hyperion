//! Assertions shared by the wire-form tests of every module.

use std::fmt::Debug;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

/// Asserts that `message` serializes to exactly `wire` and that `wire` parses back to it.
pub(crate) fn assert_wire_form<T>(message: &T, wire: Value)
where
    T: Serialize + DeserializeOwned + PartialEq + Debug,
{
    assert_eq!(serde_json::to_value(message).unwrap(), wire);
    assert_eq!(&serde_json::from_value::<T>(wire).unwrap(), message);
}

/// Asserts the wire string of each value of a string-valued enum, in both directions.
pub(crate) fn assert_wire_strings<T>(cases: &[(T, &str)])
where
    T: Serialize + DeserializeOwned + PartialEq + Debug,
{
    for (value, text) in cases {
        assert_wire_form(value, Value::String((*text).to_owned()));
    }
}
