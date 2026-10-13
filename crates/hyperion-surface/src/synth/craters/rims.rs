//! The coarse craters' rims: each crater of the field's list, the boundary diameter `D_b` and
//! wider, sharpened from the coarse cells' resolution to a level's (plan R09, T7.c; Design notes
//! 10 and 13).
//!
//! # The coarse band
//!
//! The coarse pass smooths each coarse crater into the coarse elevation at the coarse cells'
//! resolution, so that climate and drainage see it (the brainstorm's step 2). That resolution is
//! [`coarse_band_half_width`], T7.b's band rule ([`band_half_width`]) with the coarse level's
//! largest cell edge in place of a patch's largest vertex spacing: ½ max(2 m, 4 × the edge), twice
//! the largest edge, which is `D_b` on a sphere (Design note 4). It is the band of patch level
//! L − 6, to the bit wherever L − 6 is a level: that level's 64 quads a side are spaced as the
//! coarse cells, a vertex at each cell's corners.
//!
//! # The sharpening
//!
//! A coarse crater of fresh shape S ([`crater_shape`]) at a level of band half-width w adds
//!
//! > g × (F(w′) − F(`w_c`)),  w′ = min(w, `w_c`),
//!
//! with F(w) S's rim filleted over w and its volume restored ([`CraterProfile::filleted`]), `w_c`
//! the coarse band and g the share of its fresh relief it keeps ([`kept_relief`]). The coarse
//! elevation holds g × F(`w_c`), which is what the coarse pass smooths in (R09.T12.d's contract,
//! [`CoarseRims::coarse_profile`]), so the crater at level n is g × F(w), its rim sharpening as the
//! level refines, as the brainstorm's level-of-detail rule asks, wherever the base elevation's
//! spline reproduces g × F(`w_c`). A level whose band is at or beyond the coarse band adds nothing,
//! to the bit, and so does a crater too small for either band to fillet less than its rim allows
//! (both held at min(1, a) rim radii) or one whose relief is gone. Main ruled each part on
//! R09.T7.c, 2026-10-13.
//!
//! Each fillet's compensating term restores the volume its fillet moved, so the sharpening
//! integrates to zero over its reach at every level, as every small crater does (T7.b). It reaches
//! no farther than the crater, (1 + a) rim radii, and is a function of the chord from the crater's
//! centre alone, so it is continuous across every coarse cell edge and face edge it spans: a query
//! finds it through the field's index of the craters reaching its own cell
//! ([`FieldView::craters_reaching`]), which holds every crater whose reach touches the cell.
//!
//! It does not integrate to zero over each coarse cell. The compensating term spreads over the
//! whole reach while the fillet's change sits on the rim, so a cell that a crater crosses takes a
//! share of the sharpening's relief: fresh bowls of depth d on planar cells of `D_b` ÷ 2, at a fine
//! band of 0.004 rim radii and the worst of three placements, put up to 0.28 d of mean height on a
//! cell at D = `D_b`, 0.35 d at 2 `D_b`, 0.15 d at 4 `D_b` and 0.085 d at 8 `D_b` (R09.T7.c's
//! probe), as a small crater moves the means of the cells it crosses. Zeroing each
//! cell's mean would take a bump in every cell of about 3.5 times that, 0.3–1.2 d, so the coarse
//! cells' means are left to the coarse values (R09.T12.d and T4) instead: main ruled the
//! brainstorm's "nothing may be added at a fine level that changes the mean at a coarse one" met
//! per crater, as its craters' "profile must integrate to the same displacement at every finer
//! level" asks.
//!
//! # Degradation
//!
//! g is the list's own record, one less the crater's degradation code over 255
//! ([`CoarseCrater::degradation_fraction`]), which the coarse pass writes to match the relief it
//! smooths in. Minton et al. 2019's relief factor ([`super::relief_factor`]), which carries a
//! small crater's diffusive age, would keep 99.95% of the relief of the narrowest coarse crater of
//! the Ceres-like world, a rim radius of 25 km at 4.5 Gyr (κt ÷ R² = 4 × 10⁻⁵), and more of every
//! wider one (eq. 8 extrapolated, a simple crater's fit with κ calibrated on rim radii of 0.4–2.5
//! km; exact diffusion of a paraboloid's centre keeps 99.98%): a coarse crater's wear is the coarse
//! pass's, by infill, erosion and later impacts.
//!
//! # Determinism
//!
//! Every value is a function of the crater, the field's header and the level: the craters are
//! summed in the field's list order, and every transcendental is `hyperion_base::math`'s.

use hyperion_base::units::Metres;

use super::{CraterProfile, ProfileSample, band_half_width, direction_gradient};
use crate::craters::{CraterParams, CraterShape, crater_shape};
use crate::cube::PatchKey;
use crate::field::{CoarseCrater, CoarseLevel, FieldHeader, FieldView};
use crate::geometry::{BAND_LIMIT_M, max_rate};
use crate::height::HeightSample;
use crate::num;
use crate::spheroid::Spheroid;
use crate::synth::{BandLevel, QueryHeightError};

/// The half-width over which the coarse elevation holds a coarse crater's rim: T7.b's band rule
/// ([`band_half_width`]) at the coarse cells, ½ max(2 m, 4 e) with e the largest cell edge of
/// `level` on the figure's equatorial radius, S2's `kMaxEdge` derivative for the quadratic warp,
/// 1.704 897 × 2⁻ᴸ radians (`s2metrics.cc`).
///
/// That is twice the largest edge, the boundary diameter `D_b` on a sphere
/// ([`boundary_diameter`](crate::field::boundary_diameter); 92.6 km on the Moon at level 6), and
/// the band of patch level L − 6 to the bit, whose vertices are spaced as the coarse cells.
#[must_use]
pub fn coarse_band_half_width(figure: &Spheroid, level: CoarseLevel) -> Metres {
    // The same operations as `vertex_spacing`'s largest at level L − 6, whose lattice step is
    // 2⁻ᴸ: a division by a power of two is exact, as its multiplication by the step is.
    let edge_m = max_rate() * (figure.equatorial_radius_m / f64::from(1_u32 << level.get()));
    Metres::new(0.5 * num::max(BAND_LIMIT_M, 4.0 * edge_m))
}

/// The share of its fresh relief a coarse crater keeps, g = 1 − degradation ÷ 255
/// ([`CoarseCrater::degradation_fraction`]), in [0, 1]: what scales its coarse profile and its
/// sharpening (the module documentation).
#[must_use]
pub fn kept_relief(crater: &CoarseCrater) -> f64 {
    1.0 - crater.degradation_fraction()
}

/// A coarse crater's sharpening at one level: g × (F(w′) − F(`w_c`)), its rim filleted over the
/// level's band less its rim filleted over the coarse band, both with their volumes restored, times
/// the relief it keeps (the module documentation).
///
/// It integrates to zero over the crater and reaches no farther. Built once per crater and level,
/// it is evaluated at any distance by [`at`](Self::at).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RimSharpening {
    /// The rim filleted over the level's band, held at the coarse band.
    fine: CraterProfile,
    /// The rim filleted over the coarse band.
    coarse: CraterProfile,
    /// The share of its fresh relief the crater keeps, g, in [0, 1].
    relief: f64,
}

impl RimSharpening {
    /// The sharpening of the fresh shape `shape` at a level of band half-width `band`
    /// ([`band_half_width`]) above the coarse elevation's.
    ///
    /// The coarse elevation's rim is filleted over `coarse_band` ([`coarse_band_half_width`]),
    /// and the crater keeps the share `relief` of its fresh relief ([`kept_relief`]). A band at or
    /// beyond the coarse band is held at it, and adds nothing.
    ///
    /// # Panics
    ///
    /// If `band` or `coarse_band` is not finite and non-negative, or `relief` is not in [0, 1].
    #[must_use]
    pub fn new(shape: CraterShape, band: Metres, coarse_band: Metres, relief: f64) -> Self {
        let (w, w_c) = (band.value(), coarse_band.value());
        assert!(
            w.is_finite() && w >= 0.0 && w_c.is_finite() && w_c >= 0.0,
            "a level's band and the coarse band must be finite and non-negative, got {w} m and \
             {w_c} m"
        );
        assert!(
            (0.0..=1.0).contains(&relief),
            "a crater's kept relief must lie in [0, 1], got {relief}"
        );
        Self {
            fine: CraterProfile::filleted(shape, Metres::new(num::min(w, w_c))),
            coarse: CraterProfile::filleted(shape, coarse_band),
            relief,
        }
    }

    /// The crater's profile at the level's band, held at the coarse band: its fillet alone, before
    /// the relief.
    #[must_use]
    pub const fn fine(&self) -> &CraterProfile {
        &self.fine
    }

    /// The crater's profile at the coarse band, before the relief: what the coarse elevation holds
    /// of it, times [`relief`](Self::relief).
    #[must_use]
    pub const fn coarse(&self) -> &CraterProfile {
        &self.coarse
    }

    /// The share of its fresh relief the crater keeps, g.
    #[must_use]
    pub const fn relief(&self) -> f64 {
        self.relief
    }

    /// Whether it adds nothing anywhere: the level's fillet is the coarse one's (its band at or
    /// beyond the coarse band, or both held at min(1, a) rim radii), or the crater keeps no relief.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        // The level's fillet is never wider than the coarse one, since its band is held at it,
        // and neither is a NaN, since both bands are finite.
        self.relief <= 0.0 || self.fine.fillet >= self.coarse.fillet
    }

    /// The sharpening's height, metres, and its slope along the radius, metres per rim radius, at
    /// `r_over_radius` rim radii from the crater's centre: zero at and beyond the reach, and
    /// everywhere when it [`is_zero`](Self::is_zero).
    ///
    /// # Panics
    ///
    /// If `r_over_radius` is not finite and non-negative.
    #[must_use]
    pub fn at(&self, r_over_radius: f64) -> ProfileSample {
        let (height_m, slope_m_per_rim_radius) = self.height_and_slope(r_over_radius);
        ProfileSample {
            height_m,
            slope_m_per_rim_radius,
        }
    }

    /// [`at`](Self::at) as a pair: the height, metres, and the slope, metres per rim radius.
    #[must_use]
    fn height_and_slope(&self, r_over_radius: f64) -> (f64, f64) {
        let x = r_over_radius;
        if self.is_zero() {
            assert!(
                x.is_finite() && x >= 0.0,
                "a distance from a crater's centre must be finite and non-negative, got {x}"
            );
            return (0.0, 0.0);
        }
        let (fine_m, fine_slope) = self.fine.height_and_slope(x);
        let (coarse_m, coarse_slope) = self.coarse.height_and_slope(x);
        (
            self.relief * (fine_m - coarse_m),
            self.relief * (fine_slope - coarse_slope),
        )
    }
}

/// A body's coarse craters' rims: the coarse band of its field's level and the crater contract
/// their shapes come from, with which each crater of the field's list is sharpened at a level (the
/// module documentation).
///
/// It is plain data, a function of the field's header alone: the craters are the field's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoarseRims {
    params: CraterParams,
    figure: Spheroid,
    level: CoarseLevel,
    coarse_band: Metres,
}

impl CoarseRims {
    /// The rims of the body of `header`: [`for_body`](Self::for_body) of its crater contract,
    /// figure and level.
    #[must_use]
    pub fn new(header: &FieldHeader) -> Self {
        Self::for_body(*header.craters(), header.figure(), header.level())
    }

    /// The rims of a body of crater contract `params` and figure `figure` whose coarse field is at
    /// `level`: what the coarse pass reads before it has a header (R09.T12.d smooths each crater
    /// in as its [`coarse_profile`](Self::coarse_profile)).
    #[must_use]
    pub fn for_body(params: CraterParams, figure: Spheroid, level: CoarseLevel) -> Self {
        Self {
            params,
            figure,
            level,
            coarse_band: coarse_band_half_width(&figure, level),
        }
    }

    /// The coarse band ([`coarse_band_half_width`]).
    #[must_use]
    pub const fn coarse_band(&self) -> Metres {
        self.coarse_band
    }

    /// The profile a coarse crater of `diameter` has in the coarse elevation, before its relief:
    /// its fresh shape on the body ([`crater_shape`]) filleted over the coarse band.
    ///
    /// The coarse pass smooths in this times the crater's [`kept_relief`], at the chord from its
    /// centre's spheroid point in rim radii, so that the coarse elevation and the sharpening are
    /// one profile (R09.T12.d's contract).
    ///
    /// # Panics
    ///
    /// If `diameter` is not finite and positive.
    #[must_use]
    pub fn coarse_profile(&self, diameter: Metres) -> CraterProfile {
        CraterProfile::filleted(crater_shape(&self.params, diameter), self.coarse_band)
    }

    /// `crater`'s sharpening at `level`: [`RimSharpening::new`] of its fresh shape on the body,
    /// the level's [`band_half_width`], the coarse band and its [`kept_relief`].
    ///
    /// # Panics
    ///
    /// If the crater's diameter is not finite and positive.
    #[must_use]
    pub fn sharpening(&self, crater: &CoarseCrater, level: BandLevel) -> RimSharpening {
        RimSharpening::new(
            crater_shape(&self.params, crater.diameter),
            band_half_width(&self.figure, level),
            self.coarse_band,
            kept_relief(crater),
        )
    }

    /// The sharpening of every coarse crater of `field` that reaches the unit direction `dir`, at
    /// `level`: its height above the coarse elevation, metres, and its gradient.
    ///
    /// The gradient is in body-fixed space at the spheroid point, metres per metre, as R09.T4's
    /// base elevation returns its own (the gradient of the height as a function of direction).
    /// `field` is the field whose header made these rims. The craters are those of the field's
    /// index for the coarse cell that holds `dir`, in list order, each at its chord from the
    /// direction's spheroid point.
    ///
    /// # Errors
    ///
    /// [`QueryHeightError::NotSurveyed`] if `field` does not hold the coarse cell, whose index of
    /// craters it would read.
    ///
    /// # Panics
    ///
    /// If `field`'s header did not make these rims, or `dir` is zero or has a component that is
    /// not finite; and if a height is not finite (a bug).
    pub fn at<F: FieldView>(
        &self,
        field: &F,
        dir: [f64; 3],
        level: BandLevel,
    ) -> Result<HeightSample, QueryHeightError> {
        assert!(
            Self::new(field.header()) == *self,
            "coarse rims read the field whose header made them"
        );
        let cell = PatchKey::containing(self.level.get(), dir)
            .expect("a coarse level is below the cube's deepest");
        if field.cell(cell).is_none() {
            return Err(QueryHeightError::NotSurveyed(cell));
        }
        Ok(self.sum_in(field, cell, dir, level))
    }

    /// The sharpening at `dir` of the craters of `field`'s index for `cell`, at `level`
    /// ([`at`](Self::at) at the cell it finds).
    #[must_use]
    fn sum_in<F: FieldView>(
        &self,
        field: &F,
        cell: PatchKey,
        dir: [f64; 3],
        level: BandLevel,
    ) -> HeightSample {
        let band = band_half_width(&self.figure, level);
        let mut height = 0.0;
        let mut gradient = [0.0; 3];
        // A level at or beyond the coarse band adds nothing, so it reads no crater.
        if band.value() < self.coarse_band.value() {
            let p = self.figure.point(dir);
            for crater in field.craters_reaching(cell) {
                let shape = crater_shape(&self.params, crater.diameter);
                let centre_m = self.figure.point(crater.centre);
                let delta = [0, 1, 2].map(|k| p[k] - centre_m[k]);
                let d2 = delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2];
                let reach = shape.reach().value();
                if d2 >= reach * reach {
                    continue;
                }
                let sharpening =
                    RimSharpening::new(shape, band, self.coarse_band, kept_relief(crater));
                if sharpening.is_zero() {
                    continue;
                }
                let distance = d2.sqrt();
                let rim_m = 0.5 * shape.diameter().value();
                let (h, slope) = sharpening.height_and_slope(distance / rim_m);
                height += h;
                if distance > 0.0 {
                    let along = slope / (rim_m * distance);
                    for (g, d) in gradient.iter_mut().zip(delta) {
                        *g += along * d;
                    }
                }
            }
        }
        let gradient = direction_gradient(&self.figure, dir, gradient);
        HeightSample {
            height_m: num::assert_finite(height),
            gradient: gradient.map(num::assert_finite),
        }
    }
}

#[cfg(test)]
mod tests;
