//! A value that this generator version may not compute yet: absent, `null` or the value.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A single value of a record that this generator version may not compute yet, which crosses the
/// wire in one of three forms: absent, `null`, or the value.
///
/// - [`NotModelled`](Self::NotModelled): the generator does not compute the quantity at all. The
///   field is left off the wire, and the client shows the style guide's "Missing" state, the em
///   dash, never "none" (ruling 34 of 2026-09-22, for a single value inside a record the generator
///   otherwise models).
/// - [`Null`](Self::Null): it is computed and this object has none, such as the variability of a
///   star that does not vary: `null` on the wire.
/// - [`Value`](Self::Value): the value.
///
/// A field of this type is declared
///
/// ```text
/// #[serde(default, skip_serializing_if = "Modelled::is_not_modelled")]
/// #[ts(as = "Option<Option<T>>", optional)]
/// ```
///
/// with `T` written out, so that serde leaves it off the wire when it is not modelled and reads an
/// absent key as not modelled, and ts-rs writes it `name?: T | null`. The `default` is required:
/// without it serde reads an absent key through the value's own deserializer, which takes it for
/// `null`, and absent and `null` would become one. The field is then added to a
/// record without changing any wire form the record had, and filled later, by the task that
/// computes it, without changing the field's type. On its own, outside such a field, a
/// `NotModelled` value is written `null`.
///
/// # Examples
///
/// ```
/// use hyperion_protocol::Modelled;
///
/// // A star's variability before the generator computes any, once it does for a star that does
/// // not vary, and for a Cepheid with a period of 5.4 days.
/// let later: Modelled<f64> = Modelled::default();
/// assert!(later.is_not_modelled());
/// assert_eq!(serde_json::to_string(&Modelled::<f64>::Null)?, "null");
/// assert_eq!(serde_json::from_str::<Modelled<f64>>("null")?, Modelled::Null);
/// assert_eq!(serde_json::from_str::<Modelled<f64>>("5.4")?, Modelled::Value(5.4));
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Modelled<T> {
    /// This generator version does not compute the quantity: absent on the wire.
    #[default]
    NotModelled,
    /// Computed, and the object has none: `null` on the wire.
    Null,
    /// Computed: the value.
    Value(T),
}

impl<T> Modelled<T> {
    /// Whether the quantity is not computed, which is when a field of this type is left off the
    /// wire.
    #[must_use]
    pub const fn is_not_modelled(&self) -> bool {
        matches!(self, Self::NotModelled)
    }
}

impl<T: Serialize> Serialize for Modelled<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Value(value) => serializer.serialize_some(value),
            Self::Null | Self::NotModelled => serializer.serialize_none(),
        }
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Modelled<T> {
    /// Reads a key that is present: `null` as [`Null`](Self::Null) and anything else as the value.
    /// An absent key never reaches here when the field is `#[serde(default)]`, which makes it
    /// `NotModelled`.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match Option::<T>::deserialize(deserializer)? {
            Some(value) => Self::Value(value),
            None => Self::Null,
        })
    }
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};
    use serde_json::json;

    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Holder {
        #[serde(default, skip_serializing_if = "Modelled::is_not_modelled")]
        period_d: Modelled<f64>,
    }

    #[test]
    fn a_field_not_modelled_is_left_off_the_wire() {
        let holder = Holder {
            period_d: Modelled::NotModelled,
        };
        assert_eq!(serde_json::to_value(&holder).unwrap(), json!({}));
        assert_eq!(serde_json::from_value::<Holder>(json!({})).unwrap(), holder);
    }

    #[test]
    fn a_field_with_no_value_is_null() {
        let holder = Holder {
            period_d: Modelled::Null,
        };
        assert_eq!(
            serde_json::to_value(&holder).unwrap(),
            json!({ "period_d": null })
        );
        assert_eq!(
            serde_json::from_value::<Holder>(json!({ "period_d": null })).unwrap(),
            holder
        );
    }

    #[test]
    fn a_field_with_a_value_is_the_value() {
        let holder = Holder {
            period_d: Modelled::Value(5.375),
        };
        assert_eq!(
            serde_json::to_value(&holder).unwrap(),
            json!({ "period_d": 5.375 })
        );
        assert_eq!(
            serde_json::from_value::<Holder>(json!({ "period_d": 5.375 })).unwrap(),
            holder
        );
    }

    #[test]
    fn a_value_of_the_wrong_type_is_rejected() {
        let error = serde_json::from_value::<Holder>(json!({ "period_d": "long" })).unwrap_err();
        assert!(
            error.to_string().contains("invalid type"),
            "unexpected error: {error}"
        );
    }
}
