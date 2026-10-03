//! Validating a capture's WGSL with naga before any replay (R05 Design note 22, step 2).
//!
//! A module naga rejects is a finding, not a point for native: Chromium's Tint accepted it, so
//! the two front ends disagree, and the replay cannot speak for that module.

use std::fmt::Write as _;

use naga::valid::{Capabilities, ValidationFlags, Validator};

use crate::capture::Capture;

/// A module naga rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rejection {
    /// Its ID in the call log.
    pub id: u64,
    /// Its label, or empty.
    pub label: String,
    /// naga's message, with the source line it points at.
    pub message: String,
}

/// What validating a capture's modules found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ValidationReport {
    /// Modules checked.
    pub modules: usize,
    /// Modules naga rejected, in the capture's order.
    pub rejected: Vec<Rejection>,
}

impl ValidationReport {
    /// The report as lines of text, one a module rejected, after a count.
    #[must_use]
    pub fn to_text(&self) -> String {
        let mut text = format!(
            "{} WGSL modules checked, {} rejected by naga\n",
            self.modules,
            self.rejected.len()
        );
        for rejection in &self.rejected {
            let label = if rejection.label.is_empty() {
                "unlabelled"
            } else {
                &rejection.label
            };
            // Writing to a `String` cannot fail.
            let _ = writeln!(
                text,
                "module {} ({label}):\n{}",
                rejection.id, rejection.message
            );
        }
        text
    }
}

/// Why naga refused a WGSL module, with its message pointing at the source.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ValidateWgslError {
    /// The module does not parse.
    #[error("{message}")]
    Parse {
        /// naga's message, with the source line it points at.
        message: String,
    },
    /// The module parses and does not validate.
    #[error("{message}")]
    Validate {
        /// naga's message, with the source line it points at.
        message: String,
    },
}

/// Parses and validates one WGSL module, with every capability naga knows allowed (the device's
/// own features decide what the browser compiled, and naga is asked only whether it can).
///
/// # Errors
///
/// [`ValidateWgslError::Parse`] when the module does not parse, and
/// [`ValidateWgslError::Validate`] when it does not validate.
pub fn validate_wgsl(code: &str) -> Result<(), ValidateWgslError> {
    let module = naga::front::wgsl::parse_str(code).map_err(|error| ValidateWgslError::Parse {
        message: error.emit_to_string(code),
    })?;
    Validator::new(ValidationFlags::all(), Capabilities::all())
        .validate(&module)
        .map_err(|error| ValidateWgslError::Validate {
            message: error.emit_to_string(code),
        })?;
    Ok(())
}

/// Validates every WGSL module of a capture.
#[must_use]
pub fn validate_capture(capture: &Capture) -> ValidationReport {
    let mut report = ValidationReport::default();
    for module in capture.shader_modules() {
        report.modules += 1;
        if let Err(error) = validate_wgsl(module.code) {
            report.rejected.push(Rejection {
                id: module.id,
                label: module.label.to_owned(),
                message: error.to_string(),
            });
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_valid_module() {
        let code = "@vertex fn vertexMain() -> @builtin(position) vec4f { return vec4f(0.0); }";
        assert_eq!(validate_wgsl(code), Ok(()));
    }

    #[test]
    fn rejects_a_module_that_does_not_parse() {
        let error = validate_wgsl("fn broken( {").expect_err("a syntax error");
        assert!(
            matches!(error, ValidateWgslError::Parse { .. }),
            "{error:?}"
        );
    }

    #[test]
    fn rejects_a_module_that_does_not_validate() {
        let code = "fn f() -> f32 { return 1u; }";
        let error = validate_wgsl(code).expect_err("a type mismatch");
        assert!(
            matches!(error, ValidateWgslError::Validate { .. }),
            "{error:?}"
        );
    }
}
