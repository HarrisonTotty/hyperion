//! The helium-excess hook (plan 06, P06.T17): how a star's excess of helium ΔY
//! (`Composition::helium_excess`) enters its track, through
//! [`tables::helium`](crate::tables::helium).
//!
//! - The main sequence's timescales (its lifetime at each mass, which the effective-age rule reads)
//!   and those of the Hertzsprung gap and the first giant branch are multiplied by
//!   exp(s(m, Z) ΔY), with s from `LIFETIME_SLOPE`: a quadratic in log₁₀ m at four metallicities,
//!   interpolated linearly in log₁₀ Z and held beyond the end nodes. m is the mass the phase's
//!   formulae read: the current mass on the main sequence, the mass the gap was entered with after
//!   it.
//! - On the horizontal branch (core helium burning) log₁₀ `T_eff` is shifted by
//!   `HB_TEMPERATURE_SHIFT` (linear in the envelope mass, M − `M_c`) × ΔY, with the radius changed to
//!   keep the luminosity.
//!
//! The hook exists only for ΔY > 0 ([`HeliumHook::of`]); for every other star the track takes no
//! step of it, so the stars the grid places, all of ΔY = 0, are bit-identical whatever the table
//! holds. The committed table is the identity (P06.T17); plan 15's P15.T7.b fits it.
//!
//! Every track takes P15.T7.b's fitted table, `tables::helium_26`, through [`HeliumTable::of`]
//! (from version 26), which holds the branch's shift at the envelope mass its fit reached and
//! floors it at zero ([`HeliumHook::horizontal_branch`]; decision-p15-t7b-leans §8).

use crate::math;
use crate::stellar::Composition;
use crate::stellar::rules::StellarRules;
use crate::stellar::sse::PhasePoint;
use crate::tables::{helium, helium_26};
#[cfg(any(test, feature = "testing"))]
use crate::units::HeliumExcess;
use crate::units::{SolarMasses, SolarRadii};

/// A helium-excess table in [`tables::helium`](crate::tables::helium)'s shape.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct HeliumTable {
    /// log₁₀ Z at the four nodes, increasing.
    pub(crate) log_z_nodes: [f64; 4],
    /// (c₀, c₁, c₂) of s at each node.
    pub(crate) lifetime_slope: [[f64; 3]; 4],
    /// (a, b) of the horizontal branch's shift of log₁₀ `T_eff` per unit ΔY, a + b `M_env`.
    pub(crate) hb_temperature_shift: [f64; 2],
    /// The envelope mass, M☉, at which the shift is held, beyond which it is its value there: the
    /// fitted table's `HB_ENVELOPE_MAX`, the upper envelope of its fit. With a bound the shift is
    /// also floored at zero wherever the line is negative. `None` for the identity, whose shift is
    /// P06.T17's line, unbounded (decision-p15-t7b-leans §8).
    pub(crate) hb_envelope_max: Option<f64>,
}

impl HeliumTable {
    /// The committed table, [`tables::helium`](crate::tables::helium): P06.T17's identity, which
    /// the tests read beside the fitted one.
    #[cfg(test)]
    pub(crate) const COMMITTED: Self = Self {
        log_z_nodes: helium::LOG_Z_NODES,
        lifetime_slope: helium::LIFETIME_SLOPE,
        hb_temperature_shift: helium::HB_TEMPERATURE_SHIFT,
        hb_envelope_max: None,
    };

    /// P15.T7.b's fitted table, [`tables::helium_26`](crate::tables::helium_26), which every
    /// track reads.
    pub(crate) const FITTED: Self = Self {
        log_z_nodes: helium_26::LOG_Z_NODES,
        lifetime_slope: helium_26::LIFETIME_SLOPE,
        hb_temperature_shift: helium_26::HB_TEMPERATURE_SHIFT,
        hb_envelope_max: Some(helium_26::HB_ENVELOPE_MAX),
    };

    /// The table the stellar stage's rules read: the fitted one, [`FITTED`](Self::FITTED), at
    /// every version. Every track reads its table here, from the rules its options carry
    /// (`TrackOptions::rules`), as does `turn_off_mass` under the version's: the one choke point
    /// a later hold of the table would turn on.
    #[must_use]
    pub(crate) const fn of(_rules: StellarRules) -> Self {
        Self::FITTED
    }
}

/// One star's helium-excess correction: its ΔY, its metallicity and the table.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct HeliumHook {
    delta_y: f64,
    /// The slope's coefficients at the star's metallicity.
    slope: [f64; 3],
    shift: [f64; 2],
    /// The table's [`hb_envelope_max`](HeliumTable::hb_envelope_max).
    envelope_max: Option<f64>,
}

impl HeliumHook {
    /// The correction of a star of `composition` from `table`, or `None` unless its ΔY is
    /// positive.
    #[must_use]
    pub(crate) fn of(composition: &Composition, table: &HeliumTable) -> Option<Self> {
        let delta_y = composition.helium_excess().value();
        if delta_y <= 0.0 {
            return None;
        }
        let log_z = math::log10(composition.z_fit().value());
        let nodes = &table.log_z_nodes;
        let last = nodes.len() - 1;
        let slope = if log_z <= nodes[0] {
            table.lifetime_slope[0]
        } else if log_z >= nodes[last] {
            table.lifetime_slope[last]
        } else {
            let index = nodes[1..last]
                .iter()
                .take_while(|&&node| log_z >= node)
                .count();
            let share = (log_z - nodes[index]) / (nodes[index + 1] - nodes[index]);
            let (below, above) = (table.lifetime_slope[index], table.lifetime_slope[index + 1]);
            std::array::from_fn(|k| (1.0 - share) * below[k] + share * above[k])
        };
        Some(Self {
            delta_y,
            slope,
            shift: table.hb_temperature_shift,
            envelope_max: table.hb_envelope_max,
        })
    }

    /// exp(s(m, Z) ΔY), the factor on the timescales of a phase whose formulae read `m` M☉.
    #[must_use]
    pub(crate) fn timescale_factor(&self, m: f64) -> f64 {
        let x = math::log10(m);
        let [c0, c1, c2] = self.slope;
        math::exp((c0 + x * (c1 + x * c2)) * self.delta_y)
    }

    /// `point` of the horizontal branch, at current mass `mass`, shifted in log₁₀ `T_eff` at
    /// constant luminosity by [`branch_shift`] of the envelope mass `M_env` = M − `M_c`: (a + b
    /// `M_env`) ΔY, which the fitted table holds at its `HB_ENVELOPE_MAX` and floors at zero.
    #[must_use]
    pub(crate) fn horizontal_branch(&self, point: PhasePoint, mass: SolarMasses) -> PhasePoint {
        let envelope = (mass.value() - point.core_mass.value()).max(0.0);
        let shift = branch_shift(self.shift, self.envelope_max, envelope, self.delta_y);
        // L ∝ R² T⁴: a shift of δ in log T at fixed L is one of −2δ in log R.
        PhasePoint {
            radius: SolarRadii::new(point.radius.value() * math::exp10(-2.0 * shift)),
            ..point
        }
    }
}

/// The horizontal branch's shift of log₁₀ `T_eff`, dex, of a star with a helium excess `delta_y`
/// and an envelope of `envelope` M☉ (non-negative), under a table's (a, b) `shift` and its
/// `envelope_max`.
///
/// Without a bound it is P06.T17's line, (a + b `M_env`) ΔY, the identity's. With one it is
/// max(0, a + b min(`M_env`, `envelope_max`)) ΔY (decision-p15-t7b-leans §8): the fit's line
/// crosses zero near 0.2 M☉, and unbounded it would redden every thicker envelope, by −3.3 dex
/// for a 10 M☉ one at ΔY 0.18. Inside the bound, where the line is positive, both forms are the
/// same bits. The floor is a branch, not `f64::max`, which may return either zero's sign
/// (`f64::max`'s documentation); exp10 of either is 1 exactly.
#[must_use]
fn branch_shift(shift: [f64; 2], envelope_max: Option<f64>, envelope: f64, delta_y: f64) -> f64 {
    let [a, b] = shift;
    match envelope_max {
        None => (a + b * envelope) * delta_y,
        Some(bound) => {
            let held = if envelope > bound { bound } else { envelope };
            let raw = a + b * held;
            (if raw > 0.0 { raw } else { 0.0 }) * delta_y
        }
    }
}

/// A candidate helium-excess table in [`tables::helium`](crate::tables::helium)'s shape.
///
/// It is for building tracks under a table other than the committed one (plan 15, P15.T7.b): the
/// fit of the table to the helium-enhanced stellar models of Pietrinferni et al. (2006) measures
/// the lifetimes and horizontal branches of plan 06's tracks under each candidate. Its nodes are
/// always the committed `LOG_Z_NODES`, plan 06's, which the fit does not move. Behind the
/// `testing` feature; the generator builds none.
///
/// A candidate built by [`new`](Self::new) shifts the horizontal branch as P06.T17's identity
/// does, along its line unbounded; [`with_envelope_max`](Self::with_envelope_max) holds and floors
/// the shift as the fitted table's arm does (decision-p15-t7b-leans §8).
#[cfg(any(test, feature = "testing"))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CandidateHeliumTable {
    lifetime_slope: [[f64; 3]; 4],
    hb_temperature_shift: [f64; 2],
    hb_envelope_max: Option<f64>,
}

#[cfg(any(test, feature = "testing"))]
impl CandidateHeliumTable {
    /// The committed table, [`tables::helium`](crate::tables::helium), as a candidate.
    pub const COMMITTED: Self = Self::new(helium::LIFETIME_SLOPE, helium::HB_TEMPERATURE_SHIFT);

    /// A candidate of `lifetime_slope`, as `LIFETIME_SLOPE`, and `hb_temperature_shift`, as
    /// `HB_TEMPERATURE_SHIFT`, at the committed nodes, its branch's shift unbounded.
    ///
    /// `lifetime_slope` holds (c₀, c₁, c₂) of the slope s(m, Z) = c₀ + c₁ x + c₂ x², x = log₁₀(m ÷
    /// M☉), of the log of the main-sequence and giant-branch timescales in ΔY at each node,
    /// interpolated linearly in log₁₀ Z between the nodes and held beyond the end nodes.
    /// `hb_temperature_shift` holds (a, b) of the horizontal branch's shift of log₁₀ `T_eff` per
    /// unit ΔY at constant luminosity, a + b `M_env` with the envelope mass in M☉.
    #[must_use]
    pub const fn new(lifetime_slope: [[f64; 3]; 4], hb_temperature_shift: [f64; 2]) -> Self {
        Self {
            lifetime_slope,
            hb_temperature_shift,
            hb_envelope_max: None,
        }
    }

    /// This candidate with its branch's shift held at an envelope of `hb_envelope_max` and
    /// floored at zero, as `HB_ENVELOPE_MAX` holds the fitted table's: max(0, a + b min(`M_env`,
    /// `hb_envelope_max`)) ΔY (decision-p15-t7b-leans §8).
    #[must_use]
    pub const fn with_envelope_max(self, hb_envelope_max: SolarMasses) -> Self {
        Self {
            hb_envelope_max: Some(hb_envelope_max.value()),
            ..self
        }
    }

    /// The nodes, log₁₀ Z, increasing: the committed `LOG_Z_NODES`.
    #[must_use]
    pub const fn log_z_nodes(&self) -> [f64; 4] {
        helium::LOG_Z_NODES
    }

    /// (c₀, c₁, c₂) of the slope at each node, as `LIFETIME_SLOPE`.
    #[must_use]
    pub const fn lifetime_slope(&self) -> [[f64; 3]; 4] {
        self.lifetime_slope
    }

    /// (a, b) of the horizontal branch's shift, as `HB_TEMPERATURE_SHIFT`.
    #[must_use]
    pub const fn hb_temperature_shift(&self) -> [f64; 2] {
        self.hb_temperature_shift
    }

    /// The envelope mass at which the branch's shift is held, as `HB_ENVELOPE_MAX`, or `None` for
    /// a shift along its line unbounded.
    #[must_use]
    pub fn hb_envelope_max(&self) -> Option<SolarMasses> {
        self.hb_envelope_max.map(SolarMasses::new)
    }

    /// The horizontal branch's shift of log₁₀ `T_eff`, dex, that a track under this candidate
    /// gives a star of helium excess `delta_y` with an envelope of `envelope` (M − `M_c`, held at
    /// zero below it), by the hook's own arithmetic.
    #[must_use]
    pub fn branch_shift_dex(&self, envelope: SolarMasses, delta_y: HeliumExcess) -> f64 {
        branch_shift(
            self.hb_temperature_shift,
            self.hb_envelope_max,
            envelope.value().max(0.0),
            delta_y.value(),
        )
    }
}

/// The committed table: a table of zeros would be the identity too, but the committed one is the
/// generator's.
#[cfg(any(test, feature = "testing"))]
impl Default for CandidateHeliumTable {
    fn default() -> Self {
        Self::COMMITTED
    }
}

/// The whole track of a star of initial mass `m0`, `composition` and `draws` under the helium
/// table `table` in place of the committed one.
///
/// It is [`Track::full`](super::Track::full)'s track, the generator's options and resolution,
/// with `table`'s correction for the star's ΔY (`Composition::helium_excess`). For a star of
/// ΔY = 0 it is [`Track::full`](super::Track::full)'s bit for bit, whatever `table` holds, since
/// no correction is built for it; under [`CandidateHeliumTable::COMMITTED`] it is that track at
/// any ΔY. Plan 15's P15.T7.b fits the table with it. Behind the `testing` feature; the generator
/// never calls it.
///
/// # Panics
///
/// In debug builds, as [`Track::full`](super::Track::full).
#[cfg(any(test, feature = "testing"))]
#[must_use]
pub fn track_with_helium_table(
    m0: SolarMasses,
    composition: &Composition,
    draws: &crate::stellar::draws::StarDraws,
    table: &CandidateHeliumTable,
) -> super::Track {
    let table = HeliumTable {
        log_z_nodes: table.log_z_nodes(),
        lifetime_slope: table.lifetime_slope,
        hb_temperature_shift: table.hb_temperature_shift,
        hb_envelope_max: table.hb_envelope_max,
    };
    super::Track::build_with_helium(
        m0,
        composition,
        draws,
        super::TrackOptions::default(),
        super::Resolution::GENERATOR,
        None,
        HeliumHook::of(composition, &table),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GENERATOR_VERSION;
    use crate::units::{Dex, SolarLuminosities};
    use hyperion_testkit::float::assert_same_bits;

    /// The envelopes, M☉, at which a bounded shift is probed (decision-p15-t7b-leans §8's test).
    const ENVELOPES: [f64; 7] = [0.0, 0.05, 0.2, 0.3, 1.0, 10.0, 100.0];

    /// The bound of the tests' tables, M☉: the fit's.
    const BOUND: f64 = 0.2;

    /// §8: under a bound, at ΔY 0.18, the shift is finite and lies in [0, max(a, a + b × bound) ΔY];
    /// it is the line, (a + b `M_env`) ΔY bit for bit, where the envelope lies inside the bound and
    /// the line is positive; beyond the bound it is its value at the bound; and the radius is the
    /// point's × exp10(−2 × shift) bit for bit. One line crosses zero inside the bound, at
    /// 0.15 M☉, the other stays positive.
    #[test]
    fn a_bounded_branch_shift_is_held_beyond_its_bound_and_floored_at_zero() {
        let delta_y = 0.18;
        let rich = Composition::from_fe_h(Dex::new(-1.4), HeliumExcess::new(delta_y));
        for shift in [[0.3, -2.0], [0.2, 0.5]] {
            let [a, b] = shift;
            let table = HeliumTable {
                hb_temperature_shift: shift,
                hb_envelope_max: Some(BOUND),
                ..HeliumTable::COMMITTED
            };
            let hook = HeliumHook::of(&rich, &table).expect("a helium excess builds a hook");
            let top = (if a + b * BOUND > a { a + b * BOUND } else { a }) * delta_y;
            for envelope in ENVELOPES {
                let s = branch_shift(shift, Some(BOUND), envelope, delta_y);
                assert!(
                    s.is_finite() && (0.0..=top).contains(&s),
                    "{shift:?}: {s} at {envelope} M☉"
                );
                let line = a + b * envelope;
                if envelope <= BOUND && line > 0.0 {
                    assert_same_bits(s, line * delta_y);
                } else if envelope > BOUND {
                    assert_same_bits(s, branch_shift(shift, Some(BOUND), BOUND, delta_y));
                } else {
                    assert_same_bits(s, 0.0);
                }
                let core = SolarMasses::new(0.48);
                let point = PhasePoint {
                    luminosity: SolarLuminosities::new(50.0),
                    radius: SolarRadii::new(10.0),
                    core_mass: core,
                };
                let shifted =
                    hook.horizontal_branch(point, SolarMasses::new(core.value() + envelope));
                assert_same_bits(shifted.radius.value(), 10.0 * math::exp10(-2.0 * s));
                assert_same_bits(shifted.luminosity.value(), 50.0);
            }
        }
    }

    /// Without a bound the shift is P06.T17's line, unbounded, signs and all: the identity's
    /// expression of version 24, verbatim.
    #[test]
    fn an_unbounded_branch_shift_is_the_line() {
        let shift = [0.377, -1.867];
        for envelope in ENVELOPES {
            assert_same_bits(
                branch_shift(shift, None, envelope, 0.18),
                (shift[0] + shift[1] * envelope) * 0.18,
            );
        }
        assert!(branch_shift(shift, None, 10.0, 0.18) < -3.0);
    }

    /// The choke point: the version's rules read the fitted table, with plan 06's nodes bit for
    /// bit, c₂ zero at every node and its bound.
    #[test]
    fn the_rules_read_the_fitted_table() {
        let fitted = HeliumTable::of(StellarRules::of_version(GENERATOR_VERSION));
        assert_eq!(fitted, HeliumTable::FITTED);
        for (f, c) in fitted.log_z_nodes.iter().zip(helium::LOG_Z_NODES) {
            assert_same_bits(*f, c);
        }
        for row in fitted.lifetime_slope {
            assert_same_bits(row[2], 0.0);
        }
        assert_eq!(fitted.hb_envelope_max, Some(helium_26::HB_ENVELOPE_MAX));
    }

    /// The fitted table's values, pinned bit for bit (P15.T7.b): a 0.8 M☉ star of \[Fe/H\] −1.5
    /// and ΔY = 0.1 dies at 7.42 Gyr, and is on a blue horizontal branch, near 8,600 K, at the
    /// first 10 Myr step past 1 Gyr that finds it burning helium in its core. A later change that
    /// moves them re-pins them here by name.
    #[test]
    fn the_fitted_table_s_values_are_pinned() {
        use crate::stellar::Phase;
        use crate::stellar::draws::StarDraws;
        use crate::stellar::sse::{Track, TrackOptions};
        use crate::units::Years;
        let rich = Composition::from_fe_h(Dex::new(-1.5), HeliumExcess::new(0.1));
        let track = Track::full_with(
            SolarMasses::new(0.8),
            &rich,
            &StarDraws::median(),
            TrackOptions::default(),
        );
        assert_same_bits(
            track.lifetime().expect("a full track dies").value(),
            7_419_064_629.866_532,
        );
        let age = (0..4000)
            .map(|i| 1e9 + 1e7 * f64::from(i))
            .find(|&a| track.state_at(Years::new(a)).phase() == Phase::CoreHeliumBurning)
            .expect("a horizontal branch");
        assert_same_bits(age, 7.31e9);
        assert_same_bits(
            track
                .state_at(Years::new(age))
                .effective_temperature()
                .value(),
            8_632.309_939_944_349,
        );
    }
}
