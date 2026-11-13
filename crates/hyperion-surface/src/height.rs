//! What a height function returns: the height at a point and its gradient (plan R05, Design note
//! 5; plan R09, T4).
//!
//! R05 defined [`HeightSample`] in its provisional `test_planet` module, which R10 retires once the
//! client draws generated worlds. Every height source returns it (R05's test planet, R09's
//! synthesis), and the patch bake reads it, so it lives here, and `test_planet` re-exports it.

/// The height at a point and its gradient.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeightSample {
    /// The height above the spheroid along its normal, metres.
    pub height_m: f64,
    /// The height field's gradient in body-fixed space at the spheroid point, metres per metre.
    pub gradient: [f64; 3],
}
