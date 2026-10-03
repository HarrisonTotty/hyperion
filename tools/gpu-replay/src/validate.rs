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

/// Parses and validates one WGSL module, with every capability naga knows allowed (the device's
/// own features decide what the browser compiled, and naga is asked only whether it can).
///
/// # Errors
///
/// naga's message when the module does not parse or does not validate.
pub fn validate_wgsl(code: &str) -> Result<(), String> {
    let module = naga::front::wgsl::parse_str(code).map_err(|error| error.emit_to_string(code))?;
    Validator::new(ValidationFlags::all(), Capabilities::all())
        .validate(&module)
        .map_err(|error| error.emit_to_string(code))?;
    Ok(())
}

/// Validates every WGSL module of a capture.
#[must_use]
pub fn validate_capture(capture: &Capture) -> ValidationReport {
    let mut report = ValidationReport::default();
    for module in capture.shader_modules() {
        report.modules += 1;
        if let Err(message) = validate_wgsl(module.code) {
            report.rejected.push(Rejection {
                id: module.id,
                label: module.label.to_owned(),
                message,
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
        let message = validate_wgsl("fn broken( {").expect_err("a syntax error");
        assert!(message.contains("error"), "{message}");
    }

    #[test]
    fn rejects_a_module_that_does_not_validate() {
        let code = "fn f() -> f32 { return 1u; }";
        assert!(validate_wgsl(code).is_err());
    }
}
