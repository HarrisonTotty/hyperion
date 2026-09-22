//! Conversion between the wire's types and the server's, and the validation of every request
//! field on the way in (plan 04, P04.T14).
//!
//! A request's fields are checked here, once, by a `TryFrom` from its wire type; a field that
//! cannot be used becomes a [`ConvertRequestError`], answered `bad_request` with the field named.
//! Answers are built here too, by `From` from the server's types to the wire's, as are the request
//! errors that the server's own errors become.

use std::error::Error;
use std::fmt;
use std::sync::Arc;

use hyperion_protocol::{
    CreateUniverseRequest, ErrorCode, RequestError, SeedHex, UniverseInfo, UniverseList,
};
use hyperion_sim::GENERATOR_VERSION;

use crate::universe::{
    CreateUniverseError, OpenUniverseError, ParseUniverseNameError, Universe, UniverseName,
};

/// A request field that cannot be used, answered `bad_request` with the field named.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ConvertRequestError {
    field: &'static str,
    reason: String,
}

impl ConvertRequestError {
    /// The field `field`, as the wire names it, cannot be used because of `reason`.
    #[must_use]
    pub(crate) fn new(field: &'static str, reason: impl fmt::Display) -> Self {
        Self {
            field,
            reason: reason.to_string(),
        }
    }
}

impl fmt::Display for ConvertRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid {}: {}", self.field, self.reason)
    }
}

impl Error for ConvertRequestError {}

impl From<ConvertRequestError> for RequestError {
    fn from(error: ConvertRequestError) -> Self {
        Self {
            code: ErrorCode::BadRequest,
            message: error.to_string(),
            field: Some(error.field.to_owned()),
        }
    }
}

/// A `create_universe` request, checked: a valid name and the seed, if one was given.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct NewUniverse {
    name: UniverseName,
    seed: Option<u64>,
}

impl NewUniverse {
    /// The name, trimmed and checked, and the seed asked for, or `None` for one drawn by the
    /// server.
    #[must_use]
    pub(crate) fn into_parts(self) -> (UniverseName, Option<u64>) {
        (self.name, self.seed)
    }
}

impl TryFrom<CreateUniverseRequest> for NewUniverse {
    type Error = ConvertRequestError;

    /// Checks the name by the rules of [`UniverseName`]. The seed's form was checked as it was
    /// parsed.
    fn try_from(request: CreateUniverseRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            name: request.name.parse().map_err(invalid_name)?,
            seed: request.seed.as_ref().map(SeedHex::to_u64),
        })
    }
}

/// A name that broke a rule of [`UniverseName`], as the error of the `name` field.
#[must_use]
fn invalid_name(error: ParseUniverseNameError) -> ConvertRequestError {
    ConvertRequestError::new("name", error)
}

impl From<&Universe> for UniverseInfo {
    fn from(universe: &Universe) -> Self {
        Self {
            id: universe.id().into(),
            name: universe.name().as_str().to_owned(),
            seed: SeedHex::from_u64(universe.seed()),
            generator_version: universe.generator_version().get(),
            status: universe.status(),
        }
    }
}

/// The answer to `list_universes`: `universes`, in the order given, and the server's generator
/// version, against which each one's status was judged.
#[must_use]
pub(crate) fn universe_list(universes: &[Arc<Universe>]) -> UniverseList {
    UniverseList {
        universes: universes
            .iter()
            .map(|universe| UniverseInfo::from(universe.as_ref()))
            .collect(),
        server_generator_version: GENERATOR_VERSION.get(),
    }
}

impl From<CreateUniverseError> for RequestError {
    /// A taken name names the `name` field. Failures to draw, to find a free ID or to finish are
    /// the server's own (`internal`); a failed write is `storage_failed`.
    fn from(error: CreateUniverseError) -> Self {
        let message = error.to_string();
        let (code, field) = match error {
            CreateUniverseError::NameTaken { .. } => (ErrorCode::NameTaken, Some("name")),
            CreateUniverseError::LimitReached { .. } => (ErrorCode::UniverseLimitReached, None),
            CreateUniverseError::Storage(_) => (ErrorCode::StorageFailed, None),
            CreateUniverseError::Entropy(_)
            | CreateUniverseError::NoFreeId
            | CreateUniverseError::Interrupted => (ErrorCode::Internal, None),
        };
        Self {
            code,
            message,
            field: field.map(str::to_owned),
        }
    }
}

impl From<OpenUniverseError> for RequestError {
    /// An unknown ID names the `universe` field. A universe that exists but cannot be run is not
    /// the field's fault, and its message gives both versions or both formats.
    fn from(error: OpenUniverseError) -> Self {
        let (code, field) = match error {
            OpenUniverseError::UnknownUniverse { .. } => {
                (ErrorCode::UnknownUniverse, Some("universe"))
            }
            OpenUniverseError::GeneratorVersionMismatch { .. } => {
                (ErrorCode::GeneratorVersionMismatch, None)
            }
            OpenUniverseError::UnsupportedSaveFormat { .. } => {
                (ErrorCode::UnsupportedSaveFormat, None)
            }
        };
        Self {
            code,
            message: error.to_string(),
            field: field.map(str::to_owned),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::path::PathBuf;

    use hyperion_protocol::UniverseStatus;
    use hyperion_sim::GeneratorVersion;

    use super::*;
    use crate::limits::MAX_UNIVERSE_NAME_CHARS;
    use crate::universe::{DrawEntropyError, UniverseId, WriteSaveError};

    fn request(name: &str, seed: Option<u64>) -> CreateUniverseRequest {
        CreateUniverseRequest {
            name: name.to_owned(),
            seed: seed.map(SeedHex::from_u64),
        }
    }

    #[test]
    fn a_create_request_is_trimmed_and_keeps_its_seed() {
        let (name, seed) = NewUniverse::try_from(request("  Kepler Reach ", Some(0x4d2)))
            .unwrap()
            .into_parts();
        assert_eq!(name.as_str(), "Kepler Reach");
        assert_eq!(seed, Some(0x4d2));
        let (_, seed) = NewUniverse::try_from(request("Talos", None))
            .unwrap()
            .into_parts();
        assert_eq!(seed, None);
    }

    #[test]
    fn a_bad_name_is_a_bad_request_naming_the_field() {
        let too_long = "a".repeat(MAX_UNIVERSE_NAME_CHARS + 1);
        for name in ["", "   ", &too_long, "Kepler\u{7}Reach"] {
            let error = RequestError::from(NewUniverse::try_from(request(name, None)).unwrap_err());
            assert_eq!(error.code, ErrorCode::BadRequest, "{name:?}");
            assert_eq!(error.field.as_deref(), Some("name"), "{name:?}");
            assert!(error.message.starts_with("invalid name: "), "{error:?}");
        }
    }

    #[test]
    fn universe_info_carries_the_identity_and_the_status() {
        let saved = crate::universe::SavedUniverse::new(
            UniverseId::new(0x0123_4567_89ab_cdef),
            "Talos".parse().unwrap(),
            0x4d2,
            GENERATOR_VERSION,
        );
        let info = UniverseInfo::from(&Universe::from(saved));
        assert_eq!(
            info,
            UniverseInfo {
                id: hyperion_protocol::UniverseIdHex::from_u64(0x0123_4567_89ab_cdef),
                name: "Talos".to_owned(),
                seed: SeedHex::from_u64(0x4d2),
                generator_version: GENERATOR_VERSION.get(),
                status: UniverseStatus::Compatible,
            }
        );
        let list = universe_list(&[Arc::new(Universe::from(
            crate::universe::SavedUniverse::new(
                UniverseId::new(7),
                "Vega".parse().unwrap(),
                1,
                GeneratorVersion::new(GENERATOR_VERSION.get() + 1),
            ),
        ))]);
        assert_eq!(list.server_generator_version, GENERATOR_VERSION.get());
        assert_eq!(list.universes[0].status, UniverseStatus::GeneratorMismatch);
    }

    #[test]
    fn create_errors_become_their_codes() {
        let io_error = || WriteSaveError::Io {
            operation: "write",
            path: PathBuf::from("universe.json"),
            source: io::Error::other("disk full"),
        };
        for (error, code, field) in [
            (
                CreateUniverseError::NameTaken {
                    name: "Talos".to_owned(),
                },
                ErrorCode::NameTaken,
                Some("name"),
            ),
            (
                CreateUniverseError::LimitReached { limit: 256 },
                ErrorCode::UniverseLimitReached,
                None,
            ),
            (
                CreateUniverseError::Storage(io_error()),
                ErrorCode::StorageFailed,
                None,
            ),
            (
                CreateUniverseError::Entropy(DrawEntropyError::Exhausted),
                ErrorCode::Internal,
                None,
            ),
            (CreateUniverseError::NoFreeId, ErrorCode::Internal, None),
            (CreateUniverseError::Interrupted, ErrorCode::Internal, None),
        ] {
            let message = error.to_string();
            let converted = RequestError::from(error);
            assert_eq!(converted.code, code, "{message}");
            assert_eq!(converted.field.as_deref(), field, "{message}");
            assert_eq!(converted.message, message);
        }
    }

    #[test]
    fn open_errors_become_their_codes_and_a_mismatch_names_both_versions() {
        let id = UniverseId::new(0x4d2);
        let unknown = RequestError::from(OpenUniverseError::UnknownUniverse { id });
        assert_eq!(unknown.code, ErrorCode::UnknownUniverse);
        assert_eq!(unknown.field.as_deref(), Some("universe"));

        let mismatch = RequestError::from(OpenUniverseError::GeneratorVersionMismatch {
            id,
            saved: GeneratorVersion::new(3),
            server: GeneratorVersion::new(9),
        });
        assert_eq!(mismatch.code, ErrorCode::GeneratorVersionMismatch);
        assert_eq!(mismatch.field, None);
        assert_eq!(
            mismatch.message,
            "universe 00000000000004d2 was created with generator version 3, and this server runs \
             generator version 9"
        );

        let format = RequestError::from(OpenUniverseError::UnsupportedSaveFormat { id, format: 2 });
        assert_eq!(format.code, ErrorCode::UnsupportedSaveFormat);
        assert_eq!(format.field, None);
    }
}
