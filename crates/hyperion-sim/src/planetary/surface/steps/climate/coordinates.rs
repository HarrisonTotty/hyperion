//! The climate models' coordinates (R09.T13.b, Design note 8): the frame a model grid's bands and
//! columns are laid in, geographic about the spin axis, or tidally locked about the substellar
//! axis of a world locked 1:1 to its seasonal host.
//!
//! The body frame is P14.T14's ([`Spin`](crate::planetary::surface::inputs::Spin)'s documentation;
//! `planetary::frames`): its third axis, the field's +z, is the pole, and its first, +x, the prime
//! meridian, which faces the primary at pericentre on a locked body, so that a 1:1 lock's
//! substellar axis is +x. A model grid's latitude is that of its frame's third axis and its
//! longitude is measured about it from the frame's first axis towards its second
//! ([`ModelGrid`](super::grid::ModelGrid)).
//!
//! # Tidally locked coordinates
//!
//! Koll and Abbot's (2015, ApJ 802, 21, arXiv:1412.8216, Appendix B, eqs. B1–B3, read): the
//! tidally locked latitude `θ_TL` is the angle from the terminator, 90° at the substellar point
//! and −90° at the antistellar one, and the tidally locked longitude `λ_TL` the angle about the
//! substellar point, 0 at the geographic north pole. With the geographic latitude θ and
//! longitude λ, the substellar point at (0, 0), a point's body-frame components are
//! (cos θ cos λ, cos θ sin λ, sin θ) (B1) and also (sin `θ_TL`, cos `θ_TL` sin `λ_TL`,
//! cos `θ_TL` cos `λ_TL`) (B2), so that
//!
//! `θ_TL` = sin⁻¹(cos θ cos λ), `λ_TL` = tan⁻¹(sin λ ÷ tan θ) (B3),
//!
//! the latter in the quadrant of (cos θ sin λ, sin θ). The locked model's frame is then the body
//! frame with its first and third axes exchanged, (z, y, x): its latitude is sin⁻¹ x = `θ_TL` and its
//! longitude atan2(y, z) = `λ_TL`. (The exchange is a reflection, so the tidally locked frame is
//! left-handed, as Koll and Abbot's is; the isotropic diffusion does not see it.)
//!
//! The angle γ of a point from the substellar point, whose P₁(cos γ) R09.T13.d sets to plan 14's
//! day–night contrast, has cos γ = x = sin `θ_TL`: on the locked grid, a band's centre sine.

/// The frame of a model grid's bands and columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Coordinates {
    /// The body frame itself: latitude about the spin axis, longitude east of the prime
    /// meridian.
    Geographic,
    /// Koll and Abbot's tidally locked coordinates about the body frame's +x, the substellar
    /// axis of a 1:1 lock ([module documentation](self)).
    TidallyLocked,
}

impl Coordinates {
    /// The components in the model frame of `direction`, a vector in the body frame.
    #[must_use]
    pub(super) fn to_model(self, direction: [f64; 3]) -> [f64; 3] {
        match self {
            Self::Geographic => direction,
            Self::TidallyLocked => [direction[2], direction[1], direction[0]],
        }
    }

    /// The components in the body frame of `direction`, a vector in the model frame:
    /// [`to_model`](Self::to_model)'s inverse.
    #[must_use]
    pub(super) fn to_body(self, direction: [f64; 3]) -> [f64; 3] {
        match self {
            // The exchange of two axes is its own inverse.
            Self::Geographic | Self::TidallyLocked => self.to_model(direction),
        }
    }
}

/// The cosine of the angle γ between the body-frame unit vector `direction` and a 1:1 lock's
/// substellar point, the body frame's +x: its first component, the sine of its tidally locked
/// latitude, which R09.T13.d's P₁(cos γ) reads ([module documentation](self)).
#[must_use]
pub(super) fn substellar_cosine(direction: [f64; 3]) -> f64 {
    direction[0]
}

#[cfg(test)]
mod tests {
    use core::f64::consts::{FRAC_PI_2, PI};

    use super::*;
    use crate::math;

    /// Koll and Abbot's eq. B3: the tidally locked latitude and longitude of the geographic
    /// latitude `latitude` and longitude `longitude`, radians, the substellar point at (0, 0).
    fn koll_abbot(latitude: f64, longitude: f64) -> (f64, f64) {
        let (s, c) = math::sin_cos(latitude);
        let (sl, cl) = math::sin_cos(longitude);
        (math::asin(c * cl), math::atan2(c * sl, s))
    }

    /// The latitude and longitude, radians, of the unit vector `v` in its own frame.
    fn angles(v: [f64; 3]) -> (f64, f64) {
        (math::asin(v[2]), math::atan2(v[1], v[0]))
    }

    /// The unit vector at latitude `latitude` and longitude `longitude`, radians (eq. B1).
    fn direction(latitude: f64, longitude: f64) -> [f64; 3] {
        let (s, c) = math::sin_cos(latitude);
        let (sl, cl) = math::sin_cos(longitude);
        [c * cl, c * sl, s]
    }

    /// The model frame's latitude and longitude of a body direction are Koll and Abbot's, and the
    /// transform round-trips: geographic to tidally locked and back, by angles and by vectors.
    /// Their landmarks: the substellar point is the tidally locked north pole, the antistellar
    /// point its south pole, the geographic north pole (0, 0) and the eastern terminator on the
    /// equator (0, 90°); cos γ is the tidally locked latitude's sine.
    #[test]
    fn the_tidally_locked_coordinates_are_koll_and_abbot_s_and_round_trip() {
        let locked = Coordinates::TidallyLocked;
        for i in 0..9 {
            for j in 0..17 {
                let latitude = -1.5 + 0.375 * f64::from(i);
                let longitude = -3.1 + 0.3875 * f64::from(j);
                let body = direction(latitude, longitude);
                let model = locked.to_model(body);
                let (tl_latitude, tl_longitude) = angles(model);
                let (expected_latitude, expected_longitude) = koll_abbot(latitude, longitude);
                assert!((tl_latitude - expected_latitude).abs() < 1e-12);
                assert!((tl_longitude - expected_longitude).abs() < 1e-12);
                // Back through eq. B2's components.
                let (s, c) = math::sin_cos(tl_latitude);
                let (sl, cl) = math::sin_cos(tl_longitude);
                let b2 = [s, c * sl, c * cl];
                for (a, b) in b2.iter().zip(body) {
                    assert!((a - b).abs() < 1e-12, "{latitude}, {longitude}");
                }
                let back = locked.to_body(model);
                let (lat, lon) = angles(back);
                assert!((lat - latitude).abs() < 1e-12 && (lon - longitude).abs() < 1e-12);
                let gamma = math::acos(substellar_cosine(body));
                assert!((gamma - (FRAC_PI_2 - tl_latitude)).abs() < 1e-7);
                for (a, b) in Coordinates::Geographic.to_model(body).into_iter().zip(body) {
                    hyperion_testkit::float::assert_same_bits(a, b);
                }
            }
        }
        let landmarks = [
            ([1.0, 0.0, 0.0], (FRAC_PI_2, None)),
            ([-1.0, 0.0, 0.0], (-FRAC_PI_2, None)),
            ([0.0, 0.0, 1.0], (0.0, Some(0.0))),
            ([0.0, 1.0, 0.0], (0.0, Some(FRAC_PI_2))),
            ([0.0, 0.0, -1.0], (0.0, Some(PI))),
        ];
        for (body, (latitude, longitude)) in landmarks {
            let (lat, lon) = angles(locked.to_model(body));
            assert!((lat - latitude).abs() < 1e-12, "{body:?}: {lat}");
            if let Some(longitude) = longitude {
                assert!((lon - longitude).abs() < 1e-12, "{body:?}: {lon}");
            }
        }
    }
}
