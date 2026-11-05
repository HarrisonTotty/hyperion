//! The exact orbit in the centre's spherical potential: the Kepler regime's term from generator
//! version 25 (plan 09, P09.T28.d.b, restated 2026-10-30 on research advice).
//!
//! The two-body law about μ = G M(<r₀) holds the stars inside the epoch radius fixed along the
//! orbit, and T28.d.a's secular step adds their orbit averages; against a direct integration both
//! miss the plan's bound, a tenth of a solar-mass system's tidal radius at pericentre after
//! 1,000 yr, on eccentric orbits by up to some 1,800 times (plan 09's Risks, "P09.T28.d.a as
//! built"). So a Kepler-regime state whose orbit is bound in the centre's potential Ψ
//! ([`CentreProfile::psi`]) follows its exact orbit there ([`SphericalOrbit`]), which holds the
//! enclosed stars' pull and so their precession, with the Schwarzschild advance (that of
//! [`KeplerOrbit::schwarzschild_rate`]) as the one secular rate on top. Ψ is the potential the
//! centre's distribution functions are inverted in (`features/centre/df.rs`), so by Jeans' theorem
//! the motion keeps the profile's density at every time.
//!
//! [`CentreProfile::psi`]: crate::galaxy::features::centre::CentreProfile::psi
//!
//! # The orbit
//!
//! From the epoch state, with `r₀`, the radial speed `v_r0` and the angular momentum `L`, the
//! radial speed at r is `v_r² = F(r) = v_r0² + 2(Ψ(r) − Ψ(r₀)) + L²(1 ÷ r₀² − 1 ÷ r²)`, which
//! avoids the energy's rounding; its roots are the turning points `r_p ≤ r₀ ≤ r_a`, found by a
//! safeguarded Newton solve of a fixed step count in brackets from the Kepler orbit's (which bound
//! them, below). The anomaly η is logarithmic, `ln r = ū − Δ cos η` with `ū = (ln r_a + ln r_p) ÷ 2`
//! and `Δ = (ln r_a − ln r_p) ÷ 2`, so that `dt ÷ dη = r Δ sin η ÷ √F` is smooth and periodic,
//! regular at the turning points, where F's simple roots meet `sin η`'s (as in Hénon's anomaly,
//! Hénon 1971, Astrophys. Space Sci., whose volume, 13 or 14, was not re-checked at source; Binney
//! and Tremaine 2008, *Galactic Dynamics*, 2nd ed., §3.1, for the radial period and the apsidal
//! angle as these integrals). It and
//! `dψ ÷ dη = L ÷ r² · dt ÷ dη` are taken at `ANOMALY_NODES` = 64 nodes
//! `η_j = 2π(j + ½) ÷ 64` and kept as 32 cosine terms each, by the trapezoid rule, which converges
//! geometrically on a smooth periodic function (here as far as the table's cubic pieces, whose
//! second derivative steps at each knot, let it). Their constant terms give the radial period and
//! the apsidal angle, and their integrals `t(η)` and `ψ(η)` the motion.
//!
//! **The anomaly is logarithmic, not Hénon's `r = r̄ − Δ cos η`** (lean, 2026-10-30; the restated
//! plan names Hénon's). Most of the grid's Kepler-regime orbits are unbound about the mass inside
//! their epoch radius and bound in Ψ far out, and on such an orbit Hénon's anomaly packs the
//! pericentre passage, where the black hole's pull gives way to the stars', into
//! `η ~ √(r_p ÷ r_a)`, inside one node; near the black hole `dψ ÷ dη` has a pole there besides, at
//! the complex anomaly where r = 0, which comes closer as e → 1. In `ln r` the passage spreads over
//! η and r never reaches zero. Measured (a scratch probe, a black hole, an r^−1.3 cusp and an
//! extended galaxy, against Hénon's anomaly at 2¹⁵ nodes): at `r_a ÷ r_p` = 3 both err by
//! 4 × 10⁻¹² of a period in t and 1.5 × 10⁻¹¹ rad in ψ; at 10³ (e = 0.998) Hénon's by 4 × 10⁻⁸ rad
//! and this one by 10⁻⁹; at 3 × 10³ (a wide orbit) Hénon's by 2 × 10⁻⁵ of a period and 0.02 rad,
//! this one by 3 × 10⁻¹² and 3 × 10⁻¹⁰; at 10⁵, 0.6 rad against 2 × 10⁻⁸; at 10⁷, 1.5 rad against
//! 1.3 × 10⁻⁵. Against the direct integration Hénon's anomaly missed the plan's bound by 16 and
//! 10⁵ times on two wide orbits from 0.1 ly, where this one meets it
//! (`the_exact_orbit_meets_the_plan_s_bound_against_a_direct_integration`).
//!
//! At a time t, η solves `t(η) = t(η₀) + t` (reduced by whole radial periods) by a safeguarded
//! Newton solve of a fixed step count. The position is `r(η)` in the orbit's plane, turned from
//! the epoch direction by the Newtonian azimuth swept, `ψ(η) − ψ(η₀)` and Δψ a radial period,
//! times `1 + δ ÷ Δψ`, δ the Schwarzschild advance a radial period, `6π (G M_bh ÷ (c L))²`: the
//! first-order relativistic orbit, `r = p ÷ (1 + e cos kφ)`, whose pericentre turns as the azimuth
//! does, nearly all of it at pericentre (lean, 2026-10-30; the restated plan turns by `ω̇ t`,
//! uniform in time, which within a radial period puts S2 some 1–3 au, 30–70 times the plan's
//! bound, off that orbit, and its ω̇ is the Kepler orbit's about μ, none for an orbit unbound about
//! μ and up to half again too large at 5 ly for a bound one; the science check, 2026-10-30). The
//! velocity is
//! `v_r = r Δ sin η ÷ (dt ÷ dη)` along it and `L ÷ r` across, the Newtonian velocity turned with
//! the position, as P09.T28.d.a's step turns its velocity, so that the speed at each radius is
//! the orbit's in Ψ, to the series' precision: some 10⁻³ of `2μ ÷ r` in the speed squared near
//! the pericentre of an orbit bound far out (`the_exact_orbit_keeps_inside_its_kepler_orbit_s_bounds`).
//!
//! # Edges
//!
//! - **A near-circular orbit**, with Δ (about `(r_a − r_p) ÷ (r_a + r_p)` there) under
//!   `NEAR_CIRCULAR`, takes the epicyclic limit, `dt ÷ dη = 1 ÷ κ` with
//!   `κ² = −Ψ″(r̄) + 3L² ÷ r̄⁴` at `r̄ = e^ū`, since F is then a difference of nearly equal terms;
//!   its amplitude is the epicycle's through the epoch state, so the orbit still passes through
//!   it. Its error is second order in Δ, but secular: the mean azimuthal rate is off by some Δ²,
//!   which the radial periods of the span multiply.
//! - **A state with no apocentre inside Ψ's table**, unbound in Ψ or bound beyond 10⁶ ly, keeps
//!   the two-body law (P09.T28.b.b's), and so does one whose radial speed rounds to no positive
//!   value at a node, which no tested orbit does.
//! - **An orbit bound only very far out**, its apocentre some 10⁵ times its pericentre, misses the
//!   plan's bound under the series alone (version 26's law): from 0.1 ly at 1.6 times the circular
//!   speed, bound at 1.7 × 10⁴ ly with a radial period of 6 × 10⁷ yr, by 22 tenths of its tidal
//!   radius (the two-body law's 345), and at 1.7 times by 3,900. From version 27 its state near
//!   the epoch is the arc's, below.
//!
//! # The arc from the epoch (P09.T28.d.c, from version 27)
//!
//! On the arc ([`SphericalOrbit::by_arc`]) an orbit of Δ at least 0.03
//! (`ARC_LEAST_RANGE`, lean) takes its state at |dt| under half its radial period (`ARC_SPAN`)
//! from the epoch itself: `t_arc(η) = ∫ dt ÷ dη′` and `ψ_arc(η) = ∫ L ÷ r² · dt ÷ dη′` from η₀ to
//! η, with `dt ÷ dη = r Δ |sin η| ÷ √F(r)` read from Ψ's table at each node, by the 16-node
//! Gauss–Legendre rule (DLMF §3.5(v); [`GL16_NODES`]) on a panel each side of a turning point the
//! arc crosses. η solves `t_arc(η) = dt` by `ARC_STEPS` = 3 quadratures, the first at the series'
//! root and each after a safeguarded Newton step, in the bracket the series' wobble gives, cut to
//! a radial period on the epoch's side; the state is placed as the series' is, the advance turned
//! in proportion to `ψ_arc`; a solve those leave off its root by some 10⁻⁶ of the span
//! (`ARC_RESIDUAL`), as on a few orbits bound only some 10⁵ ly out, takes five more
//! (`ARC_MORE_STEPS`). Longer spans and orbits nearer a circle keep the series, which is the more
//! exact there, and so does a solve still off, or left non-finite by rounding, which no tested one
//! is. The integrand is smooth through a turning point,
//! where F's simple root meets sin η's; beside one, where F's rounding outweighs the limit's
//! truncation (`ARC_FLOOR`), it is its limit there, `√(2 r Δ ÷ |F′|)`, which no node's rounding
//! can then make large. It is analytic only
//! where Ψ's table is, between its knots, so on a panel across many the rule converges
//! algebraically, as the series does. A state costs some 5 µs, against the series' 1.6 (pod,
//! 16 vCPU, `benches/centre.rs`).
//!
//! **η₀ is the arc's own** (lean, 2026-11-02): `tan(η₀ ÷ 2) = √((ln r₀ − ln r_p) ÷ (ln r_a −
//! ln r₀))` on the side of `v_r0`, where the series reads sin η₀ off its own rate at cos η₀. On a
//! wide orbit that rate errs most near pericentre, and the series' η₀ puts the whole motion out of
//! phase: a second after the epoch, the series puts the orbit from 0.1 ly at 1.6 times the
//! circular speed some 3 au from where its Kepler orbit's first second takes it, and at 1.7 times
//! some 60 au, where the arc is within 10⁻¹⁵ ly. Measured (a scratch diagnostic, the fixture's
//! layer-B Kepler-regime records within 1.5 influence radii, the plan's test's cases and
//! research's sweep, 13,308 state-times at ±100 and ±1,000 yr against a direct integration in the
//! profile's pull), the arc from the series' η₀ erred as the series does, to 3 digits, and from its
//! own it leaves none of them over the plan's bound, the worst 6.6 × 10⁻³ of it, where the series
//! leaves 526, the worst 3,939 times it. Against a direct integration in the table's own Ψ the arc
//! is within some 10⁻⁵ of the bound; one, two, four, 16 and 64 panels a side agree, and splitting
//! at the turning point leaves the widest orbits' worst a third of an unsplit panel's; at spans to
//! 0.499 of a radial period it errs as little as the series.
//!
//! # What follows for the bounds
//!
//! The Kepler orbit about μ = G M(<r₀) is at least as fast at every radius, and its radial range
//! holds the exact orbit's: outward the mass inside is at least M(<r₀), inward at most, so
//! `Ψ(r) − Ψ(r₀)` is at most the point mass's. The dark halo is in Ψ and not in μ, which leaves
//! inward the halo's mass inside r₀, some 10⁻³ of the black hole's at the Milky Way fixture's edge;
//! outward the bound is exact. So P09.T29's shells and pads, P09.T28.b.b's loss-cone verdict and
//! P09.T28.c's tidal radius, all taken on the Kepler orbit, stay as built: the verdict errs
//! towards carving out, and the radius towards the smaller.
//!
//! # The table
//!
//! [`CentrePotential`] holds Ψ as the black hole's `G M_bh ÷ r`, exact, and the rest (the
//! cluster's stars and the rest of the galaxy's monopole, its dark halo included), tabulated at
//! 32 knots a decade from 10⁻⁷ to 10⁶ ly with its exact slope `−G M(<r) ÷ r` in `ln r` and read
//! by the cubic Hermite interpolant, as the galaxy's monopole holds its own
//! ([`SphericalAverage`](crate::galaxy::potential::SphericalAverage)); inside the first knot the
//! rest is its value there, beyond the last no orbit is followed. It is built once per galaxy,
//! when first read (`Galaxy::centre_potential`), and holds the basis of the series' nodes too.
//!
//! # Determinism
//!
//! Pure: fixed knots and nodes, fixed step counts, fixed operations in a fixed order, and [`math`]
//! functions, so the table, each orbit's coefficients and every state are the same bits on every
//! platform. The knots, nodes, terms, step counts and the threshold, and the form of every
//! expression here, belong to the generator version from 25, when the hold is lifted; the arc's
//! rule, span, step count, floor and panels, and its η₀, from 27.

use std::f64::consts::{PI, TAU};
use std::fmt;
use std::sync::Arc;

use super::{CentreMotion, KeplerOrbit, cross, dot, length};
use crate::coords::{GalacticDisplacement, GalacticVelocity};
use crate::galaxy::consts::G;
use crate::galaxy::potential::spherical::SphericalMass;
use crate::math;
use crate::tables::gauss_legendre::{GL16_NODES, GL16_WEIGHTS};
use crate::units::consts::{METRES_PER_LIGHT_YEAR, SECONDS_PER_JULIAN_YEAR, SPEED_OF_LIGHT};
use crate::units::{GravitationalParameter, LightYears, PerYear, Seconds};

/// (km/s)² in m² s⁻².
const M2_S2_PER_KM2_S2: f64 = 1e6;

/// The first knot of Ψ's table, ly: inside it the stars weigh some 10⁻⁷ M☉, and a black hole's
/// loss cone, some 2 au, lies some 300 times farther out.
const RADIUS_LO_LY: f64 = 1e-7;

/// Decades of Ψ's table, 10⁻⁷ to 10⁶ ly, as [`SecularTable`](super::secular::SecularTable)'s.
const RADIUS_DECADES: u32 = 13;

/// Knots a decade of Ψ's table (the restated plan's 32).
const RADIUS_KNOTS_PER_DECADE: u32 = 32;

/// The nodes of the series in the anomaly over a radial period, N = 64 (the restated plan's),
/// `η_j = 2π(j + ½) ÷ N`; they are symmetric about π, so the 32 in (0, π) are evaluated.
pub(crate) const ANOMALY_NODES: u32 = 64;

/// The cosine terms kept of each series, N ÷ 2 = 32: at these nodes the next, `cos(32 η_j)`, is
/// zero.
const TERMS: usize = 32;

/// The nodes in (0, π), N ÷ 2.
const HALF_NODES: usize = 32;

/// The safeguarded Newton steps of each turning point, a fixed count: from the end of the bracket
/// every root of the stress set (`the_fixed_step_counts_converge`) is reached to F's rounding
/// within 44, and stays there; an epoch at a turning point, whose other bracket closes on the
/// epoch radius itself, takes the most (12 otherwise).
const TURNING_POINT_STEPS: u32 = 64;

/// The halvings or doublings that may widen a turning point's bracket from the Kepler orbit's
/// until F is negative at its end: 64 cover the table's thirteen decades.
const BRACKET_STEPS: u32 = 64;

/// The steps of the anomaly's solve at a time, a fixed count: [`ANOMALY_BISECTIONS`] of the
/// bracket, then safeguarded Newton steps; every solve of the stress set reaches the root to
/// 10⁻¹² rad within 12 and stays there (`the_fixed_step_counts_converge`).
const ANOMALY_STEPS: u32 = 20;

/// The bisections that open the anomaly's solve, narrowing its bracket to some 2π ÷ 32 before
/// Newton's steps.
const ANOMALY_BISECTIONS: u32 = 5;

/// The Δ below which an orbit takes the epicyclic limit (Edges), 10⁻⁵: measured against a
/// direct integration in the table's own Ψ over 1,000 yr at 0.1, 1 and 5 ly
/// (`the_near_circular_threshold_joins_the_two_limits`), the series' error grows as F's rounding
/// over Δ² as the orbit nears a circle and the epicycle's as Δ² times the radial
/// periods of the span as it leaves one; they cross between 10⁻⁵ and 10⁻⁴, at some 10⁻⁵ of the
/// plan's bound, and below 10⁻⁵ the epicycle's worst is under 2 × 10⁻⁴ of it.
pub(crate) const NEAR_CIRCULAR: f64 = 1e-5;

/// The span from the epoch, a share of the radial period, under which an orbit takes its state by
/// the arc ([`SphericalOrbit::by_arc`]; "The arc from the epoch"), ½ (lean): to 0.499 of a
/// period the arc errs as little as the series.
const ARC_SPAN: f64 = 0.5;

/// The least Δ of an orbit that takes the arc, 0.03 (lean, 2026-11-02): below it the series is the
/// more exact, F being too small beside its terms for the arc's nodes near a turning point, where
/// the series' are never closer than π ÷ 64. Measured against a direct integration in the table's
/// own Ψ at ±100 and ±1,000 yr from 0.1, 1 and 5 ly, the epoch midway between the turning points
/// (`the_arc_meets_the_plan_s_bound_near_a_circle`): under [`ARC_FLOOR`] the arc missed the plan's
/// bound by up to 3,000 times at Δ under 3 × 10⁻⁴; with a floor kept beside the turning point
/// instead, F ≤ 10⁻⁸ `√(s r Δ |F′|)`, it erred by up to 1.6 × 10⁻² of the bound at Δ 2 × 10⁻⁵,
/// where the series errs by 3 × 10⁻⁷, and the two crossed between 0.01 and 0.03.
const ARC_LEAST_RANGE: f64 = 0.03;

/// The share of the span, and its millionth of the radial period, by which `t_arc` at the arc's
/// root may miss the span before the solve takes [`ARC_MORE_STEPS`] more, and then the series
/// answers instead, 10⁻⁶: some 3,000 times the solve's worst over its stress set.
const ARC_RESIDUAL: f64 = 1e-6;

/// The quadratures of the arc's solve at a time, a fixed count: the first at the series' anomaly,
/// each after a safeguarded Newton step from the last. Every solve of the stress set is within
/// 10⁻⁴ of the plan's bound of what four times as many give after three
/// (`the_arc_s_fixed_step_count_converges`), and against the direct integration the arc's worst
/// is the same as after four, to 3 digits; two leave 1.5 tenths.
const ARC_STEPS: u32 = 3;

/// The further quadratures, a fixed count, of a solve that [`ARC_STEPS`] leave off its root by more
/// than [`ARC_RESIDUAL`] allows, before the series answers instead: on an orbit bound only some
/// 10⁵ ly out the series' root is a poor start, and of the census's 342,553 rogue planets' orbits
/// on the arc three needed more, the worst six in all
/// (`the_arc_solves_a_barely_bound_orbit_with_more_steps`).
const ARC_MORE_STEPS: u32 = 5;

/// F's share of the size of its terms at or below which F is taken to be rounding, so that
/// `dt ÷ dη` is its limit at the turning point beside it, `√(2 r Δ ÷ |F′|)`: about √ε, where the
/// direct form's rounding, some 10⁻¹⁶ of the terms over F, and the limit's truncation, some F over
/// the terms on a wide orbit, balance. On an orbit of Δ at least [`ARC_LEAST_RANGE`] that is
/// within some 10⁻² rad of the turning point at Δ 0.03, narrowing as 1 ÷ Δ, where the limit errs
/// by some 10⁻⁴ at most and the state by under 10⁻³ of the plan's bound (the science check's
/// emulation, 2026-11-02). Nearer a circle F's whole range falls under it,
/// and the limit at every node missed the plan's bound by up to 3,000 times at Δ under 3 × 10⁻⁴
/// (`the_arc_meets_the_plan_s_bound_near_a_circle`), which is why such an orbit keeps the series.
const ARC_FLOOR: f64 = 1e-8;

/// The centre's potential Ψ as the exact orbit reads it (module documentation): the black hole's
/// `G M_bh ÷ r` and the rest tabulated, built once from a galaxy's [`CentreMotion`].
///
/// It is a pure function of the motion's profile: the black hole, the nuclear cluster and the rest
/// of the galaxy's monopole, its dark halo included, as [`CentreProfile::psi`] holds them. It
/// holds 417 knots and the series' basis at its 32 nodes, about 15 kB, and builds in well under a
/// millisecond.
///
/// [`CentreProfile::psi`]: crate::galaxy::features::centre::CentreProfile::psi
///
/// # Examples
///
/// The potential a star 1 ly from the Milky Way's black hole moves in, against the black hole's
/// alone: the cluster and the galaxy deepen the well.
///
/// ```
/// use hyperion_sim::galaxy::motion::CentreMotion;
/// use hyperion_sim::galaxy::motion::spherical::CentrePotential;
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::galaxy::potential::MassModel;
/// use hyperion_sim::units::LightYears;
///
/// let params = GalaxyParams::milky_way_like();
/// let motion = CentreMotion::new(&params, &MassModel::new(&params))?;
/// let potential = CentrePotential::new(&motion);
/// let psi = potential.psi_j_per_kg(LightYears::new(1.0));
/// let black_hole = potential.black_hole_psi_j_per_kg(LightYears::new(1.0));
/// assert!(psi > 1.5 * black_hole, "{psi} against {black_hole} m²/s²");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct CentrePotential {
    /// G `M_bh`, m³ s⁻².
    black_hole: f64,
    /// `ln (r ÷ 1 ly)` of the first knot.
    lo: f64,
    /// `ln (r ÷ 1 m)` of the first knot, for the arc from the epoch, which knows `ln r` in metres.
    lo_metres: f64,
    /// The knots' step in `ln r`.
    step: f64,
    /// At each knot, the rest of the potential, `Ψ − G M_bh ÷ r`, and its slope in `ln r`,
    /// `−G M(<r) ÷ r` of the rest's mass, m² s⁻².
    rest: Vec<[f64; 2]>,
    /// `cos(k η_j)` for the nodes in (0, π), node-major.
    basis: Vec<[f64; TERMS]>,
}

impl CentrePotential {
    /// The potential of the centre whose motion is `motion` ([`Galaxy::centre_motion`]).
    ///
    /// [`Galaxy::centre_motion`]: crate::galaxy::Galaxy::centre_motion
    #[must_use]
    pub fn new(motion: &CentreMotion) -> Self {
        let profile = motion.profile();
        let black_hole = GravitationalParameter::from_solar_masses(profile.black_hole()).value();
        let lo = math::ln(RADIUS_LO_LY);
        let step = core::f64::consts::LN_10 / f64::from(RADIUS_KNOTS_PER_DECADE);
        let rest = (0..=RADIUS_DECADES * RADIUS_KNOTS_PER_DECADE)
            .map(|k| {
                let r = math::exp(lo + f64::from(k) * step);
                let stars = profile.stellar_mass_within(r).value();
                let galaxy = profile
                    .galaxy()
                    .map_or(0.0, |g| g.enclosed_mass(LightYears::new(r)).value());
                // `outer_psi` holds the cluster outside r and the galaxy's whole monopole, so the
                // rest is computed without `psi`'s cancellation against the black hole's term.
                let value = profile.outer_psi(r) + G * stars / r;
                let slope = -G * (stars + galaxy) / r;
                [value * M2_S2_PER_KM2_S2, slope * M2_S2_PER_KM2_S2]
            })
            .collect();
        let basis = (0..HALF_NODES)
            .map(|j| {
                let eta = node(j);
                std::array::from_fn(|k| math::cos(index_f64(k) * eta))
            })
            .collect();
        Self {
            black_hole,
            lo,
            lo_metres: lo + math::ln(METRES_PER_LIGHT_YEAR),
            step,
            rest,
            basis,
        }
    }

    /// Ψ at `r` in (0, 10⁶ ly], J kg⁻¹ (m² s⁻²): positive and falling outward. Inside the table's
    /// first knot, 10⁻⁷ ly, the rest of Ψ is held at that knot's, and beyond its last, 10⁶ ly, at
    /// the last's, where no orbit is followed.
    #[must_use]
    pub fn psi_j_per_kg(&self, r: LightYears) -> f64 {
        let r_m = r.value() * METRES_PER_LIGHT_YEAR;
        self.black_hole / r_m + self.rest_at(r_m)[0]
    }

    /// The black hole's share of Ψ at `r` > 0, `G M_bh ÷ r`, J kg⁻¹ (m² s⁻²).
    #[must_use]
    pub fn black_hole_psi_j_per_kg(&self, r: LightYears) -> f64 {
        self.black_hole / (r.value() * METRES_PER_LIGHT_YEAR)
    }

    /// The black hole's gravitational parameter, m³ s⁻².
    #[must_use]
    pub(crate) fn black_hole(&self) -> GravitationalParameter {
        GravitationalParameter::new(self.black_hole)
    }

    /// Every value it holds, in a fixed order: the black hole's μ, the knots' and the basis', for a
    /// test that holds a memo to a direct build bit for bit.
    #[cfg(test)]
    pub(crate) fn values(&self) -> Vec<f64> {
        let mut values = vec![self.black_hole, self.lo, self.lo_metres, self.step];
        values.extend(self.rest.iter().flatten());
        values.extend(self.basis.iter().flatten());
        values
    }

    /// The bytes it owns on the heap: its knots and its basis.
    #[must_use]
    pub(crate) fn heap_bytes(&self) -> usize {
        size_of_val(&*self.rest) + size_of_val(&*self.basis)
    }

    /// The farthest radius the table holds, m.
    #[must_use]
    fn reach_m(&self) -> f64 {
        math::exp(self.lo + index_f64(self.rest.len() - 1) * self.step) * METRES_PER_LIGHT_YEAR
    }

    /// The rest of Ψ at `r` m and its first two derivatives in r: m² s⁻², m s⁻², s⁻². Inside the
    /// first knot it is that knot's value, and beyond the last the last's, both with no slope.
    #[must_use]
    fn rest_at(&self, r: f64) -> [f64; 3] {
        let x = math::ln(r / METRES_PER_LIGHT_YEAR);
        self.rest_at_knots(r, (x - self.lo) / self.step)
    }

    /// [`rest_at`](Self::rest_at) at `r` m, where `log_r` is `ln (r ÷ 1 m)`, known already: on the
    /// arc from the epoch, where it saves a logarithm at each node, a third of a state's cost (10.7
    /// to 6.9 µs a wide orbit's state at four quadratures, `benches/centre.rs`, pod, 16 vCPU).
    #[must_use]
    fn rest_at_log(&self, r: f64, log_r: f64) -> [f64; 3] {
        self.rest_at_knots(r, (log_r - self.lo_metres) / self.step)
    }

    /// [`rest_at`](Self::rest_at) at `r` m, `u` knot steps past the first knot.
    #[must_use]
    #[expect(
        clippy::many_single_char_names,
        reason = "radius, knot and panel in their usual symbols, as the galaxy's monopole has them"
    )]
    fn rest_at_knots(&self, r: f64, u: f64) -> [f64; 3] {
        let last = self.rest.len() - 1;
        if u.is_nan() || u <= 0.0 {
            return [self.rest[0][0], 0.0, 0.0];
        }
        if u >= index_f64(last) {
            return [self.rest[last][0], 0.0, 0.0];
        }
        let floor = u.floor();
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "u is in (0, 416) and floored: a small non-negative integer"
        )]
        let k = (floor as usize).min(last - 1);
        let t = u - floor;
        let h = self.step;
        let ([y0, d0], [y1, d1]) = (self.rest[k], self.rest[k + 1]);
        let (t2, t3) = (t * t, t * t * t);
        let value = (2.0 * t3 - 3.0 * t2 + 1.0) * y0
            + (t3 - 2.0 * t2 + t) * h * d0
            + (-2.0 * t3 + 3.0 * t2) * y1
            + (t3 - t2) * h * d1;
        let slope = ((6.0 * t2 - 6.0 * t) * y0
            + (3.0 * t2 - 4.0 * t + 1.0) * h * d0
            + (-6.0 * t2 + 6.0 * t) * y1
            + (3.0 * t2 - 2.0 * t) * h * d1)
            / h;
        let bend = ((12.0 * t - 6.0) * y0
            + (6.0 * t - 4.0) * h * d0
            + (-12.0 * t + 6.0) * y1
            + (6.0 * t - 2.0) * h * d1)
            / (h * h);
        // d ÷ dr = (d ÷ d ln r) ÷ r, d² ÷ dr² = (d² ÷ d ln r² − d ÷ d ln r) ÷ r².
        [value, slope / r, (bend - slope) / (r * r)]
    }
}

/// Node `j`, `2π(j + ½) ÷ N`.
#[must_use]
fn node(j: usize) -> f64 {
    TAU * (index_f64(j) + 0.5) / f64::from(ANOMALY_NODES)
}

/// A small index as a float.
#[must_use]
fn index_f64(i: usize) -> f64 {
    f64::from(u32::try_from(i).expect("an index of a table of 417 knots or 32 terms"))
}

/// The radial speed squared of one orbit as a function of radius (module documentation), m² s⁻².
struct Radial<'a> {
    potential: &'a CentrePotential,
    /// The epoch radius, m.
    r0: f64,
    /// The rest of Ψ there, m² s⁻².
    rest0: f64,
    /// The radial speed at the epoch squared, `v_r0²`, m² s⁻².
    speed_sq: f64,
    /// `L²`, m⁴ s⁻².
    momentum_sq: f64,
}

impl Radial<'_> {
    /// `F(r) = v_r0² + 2(Ψ(r) − Ψ(r₀)) + L²(1 ÷ r₀² − 1 ÷ r²)`, each difference in a form without
    /// cancellation, and `F′(r) = 2Ψ′(r) + 2L² ÷ r³`.
    #[must_use]
    fn at(&self, r: f64) -> (f64, f64) {
        let (value, slope, _) = self.at_scaled(r);
        (value, slope)
    }

    /// [`at`](Self::at), with the size of F's terms, `v_r0² + 2(|G M_bh (r₀ − r) ÷ (r r₀)| +
    /// max(|rest|, |rest₀|)) + |L²(r² − r₀²) ÷ (r₀² r²)|`, m² s⁻²: what F's rounding is a share of.
    #[must_use]
    fn at_scaled(&self, r: f64) -> (f64, f64, f64) {
        self.at_scaled_with(r, self.potential.rest_at(r))
    }

    /// [`at_scaled`](Self::at_scaled) where `rest` is the rest of Ψ at `r` and its derivatives.
    #[must_use]
    fn at_scaled_with(&self, r: f64, [rest, rest_slope, _]: [f64; 3]) -> (f64, f64, f64) {
        let r0 = self.r0;
        let black_hole = self.potential.black_hole;
        let pull = black_hole * (r0 - r) / (r * r0);
        let drop = pull + (rest - self.rest0);
        let turn = self.momentum_sq * (r - r0) * (r + r0) / (r0 * r0 * r * r);
        let value = self.speed_sq + 2.0 * drop + turn;
        let slope =
            2.0 * (-black_hole / (r * r) + rest_slope) + 2.0 * self.momentum_sq / (r * r * r);
        let scale =
            self.speed_sq + 2.0 * (pull.abs() + rest.abs().max(self.rest0.abs())) + turn.abs();
        (value, slope, scale)
    }

    /// `κ² = −Ψ″(r) + 3L² ÷ r⁴`, the epicyclic frequency squared at `r`, s⁻².
    #[must_use]
    fn epicyclic_sq(&self, r: f64) -> f64 {
        let [_, _, rest_bend] = self.potential.rest_at(r);
        let psi_bend = 2.0 * self.potential.black_hole / (r * r * r) + rest_bend;
        -psi_bend + 3.0 * self.momentum_sq / (r * r * r * r)
    }
}

/// The table an exact orbit is prepared in: alone, for the series' law, or shared, so that an orbit
/// on the arc from the epoch can keep it.
#[derive(Clone, Copy)]
enum Table<'a> {
    /// Ψ's table, which the orbit does not keep.
    Alone(&'a CentrePotential),
    /// Ψ's table, shared, which an orbit on the arc keeps a clone of.
    Shared(&'a Arc<CentrePotential>),
}

impl<'a> Table<'a> {
    /// The table itself.
    #[must_use]
    fn potential(self) -> &'a CentrePotential {
        match self {
            Self::Alone(potential) => potential,
            Self::Shared(potential) => potential,
        }
    }
}

/// `dt ÷ dη` at an anomaly on the arc from the epoch, with what the state there reads.
#[derive(Debug, Clone, Copy)]
struct ArcRate {
    /// `dt ÷ dη`, s.
    rate: f64,
    /// The radius, m.
    r: f64,
    /// sin η.
    sin: f64,
}

/// The arc's solve at a time: the anomaly, `[t_arc, ψ_arc]` there (s, rad) and `dt ÷ dη`.
#[derive(Debug, Clone, Copy)]
struct ArcRoot {
    /// η, rad.
    eta: f64,
    /// `[t_arc(η), ψ_arc(η)]` from the epoch, s and rad.
    integrals: [f64; 2],
    /// `dt ÷ dη` at η, with r and sin η.
    at: ArcRate,
}

/// Which turning point a bracket holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Turning {
    /// The pericentre: F is negative below it and positive above.
    Pericentre,
    /// The apocentre: F is positive below it and negative above.
    Apocentre,
}

/// The root of `radial`'s F in `[lo, hi]` of kind `turning`, by `steps`
/// ([`TURNING_POINT_STEPS`]) safeguarded Newton steps from `start`: a step that leaves the
/// bracket, which each step's sign shrinks, is a bisection instead, and one that rounds onto its
/// end, once converged, is kept.
#[must_use]
fn turning_point(
    radial: &Radial<'_>,
    turning: Turning,
    [mut lo, mut hi]: [f64; 2],
    start: f64,
    steps: u32,
) -> f64 {
    let mut x = start.clamp(lo, hi);
    for _ in 0..steps {
        let (f, slope) = radial.at(x);
        let below = match turning {
            Turning::Pericentre => f < 0.0,
            Turning::Apocentre => f > 0.0,
        };
        let above = match turning {
            Turning::Pericentre => f > 0.0,
            Turning::Apocentre => f < 0.0,
        };
        if below {
            lo = x;
        } else if above {
            hi = x;
        } else {
            // F is zero, and x the root; a NaN, which no r inside the table gives, keeps x too.
            continue;
        }
        // On the closed bracket: once converged, rounding can put the step on the end it moved.
        let newton = x - f / slope;
        x = if newton >= lo && newton <= hi {
            newton
        } else {
            f64::midpoint(lo, hi)
        };
    }
    x
}

/// A Kepler-regime state's exact orbit in the centre's potential, prepared once from its epoch
/// state (module documentation), from which [`position_velocity`](Self::position_velocity) gives
/// its state at any time.
///
/// It holds the plane's axes, the turning points, the two series' 32 coefficients each and the
/// epoch's anomaly, about 640 bytes.
///
/// # Examples
///
/// A star 0.5 ly from the Milky Way's black hole on an eccentric orbit: in the stars' potential its
/// pericentre turns backwards from one passage to the next, where about the mass inside its
/// orbit alone it would close.
///
/// ```
/// use std::f64::consts::TAU;
///
/// use hyperion_sim::coords::GalacticVelocity;
/// use hyperion_sim::galaxy::PointLy;
/// use hyperion_sim::galaxy::motion::spherical::{CentrePotential, SphericalOrbit};
/// use hyperion_sim::galaxy::motion::{CentreMotion, KeplerOrbit};
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::galaxy::potential::MassModel;
/// use hyperion_sim::units::consts::METRES_PER_LIGHT_YEAR;
/// use hyperion_sim::units::{GravitationalParameter, LightYears};
///
/// let params = GalaxyParams::milky_way_like();
/// let motion = CentreMotion::new(&params, &MassModel::new(&params))?;
/// let potential = CentrePotential::new(&motion);
/// let r = 0.5;
/// let mu = GravitationalParameter::from_solar_masses(motion.enclosed_mass(LightYears::new(r)));
/// let speed = 0.6 * (mu.value() / (r * METRES_PER_LIGHT_YEAR)).sqrt();
/// let kepler = KeplerOrbit::from_state(
///     PointLy::new(r, 0.0, 0.0),
///     GalacticVelocity::new([0.0, speed, 0.0]),
///     mu,
/// )?;
/// let exact = SphericalOrbit::new(&potential, &kepler).expect("bound in the centre's potential");
/// assert!(exact.apsidal_angle_rad() < TAU, "the stars turn the pericentre backwards");
/// // Its radial range lies inside the Kepler orbit's.
/// assert!(exact.pericentre().value() >= kepler.pericentre().value());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct SphericalOrbit {
    /// The plane's axes: towards the epoch position, and across it along the motion.
    axes: [[f64; 3]; 2],
    /// L, m² s⁻¹.
    momentum: f64,
    /// The turning points `[r_p, r_a]`, m.
    turning: [f64; 2],
    /// ū, `ln (r ÷ 1 m)` at the anomaly's middle.
    log_mean: f64,
    /// Δ, the anomaly's half-range in `ln r`.
    log_range: f64,
    /// `t(η)`'s coefficients, s: the constant term of `dt ÷ dη` first, then each cosine term's
    /// over its order, so that `t(η) = c₀ η + Σ c_k sin kη`.
    time: [f64; TERMS],
    /// The same of `dψ ÷ dη`, rad.
    angle: [f64; TERMS],
    /// `Σ |c_k|` of the time's series, s: `t(η) − c₀ η` lies within it.
    wobble: f64,
    /// `t(η₀)`, s, and `ψ(η₀)`, rad, at the epoch's anomaly.
    epoch: [f64; 2],
    /// δ, the Schwarzschild advance a radial period, rad.
    advance: f64,
    /// What the arc from the epoch reads, for a non-circular orbit prepared by
    /// [`by_arc`](Self::by_arc); `None` under the series alone.
    arc: Option<EpochArc>,
}

/// What an exact orbit's arc from the epoch reads besides the series (P09.T28.d.c): the epoch's
/// anomaly, F's terms at the epoch and Ψ's table, shared with the galaxy that built it.
#[derive(Clone, PartialEq)]
struct EpochArc {
    /// The centre's potential the orbit is in.
    potential: Arc<CentrePotential>,
    /// η₀, the epoch's anomaly in [−π, π], from the epoch radius and the turning points.
    eta0: f64,
    /// The series' `t(η₀)` there, s, from which the arc's solve starts at the series' root.
    series_time0: f64,
    /// The epoch radius, m.
    r0: f64,
    /// The rest of Ψ there, m² s⁻².
    rest0: f64,
    /// `v_r0²`, m² s⁻².
    speed_sq: f64,
}

/// The epoch's values, without the table's knots, which the galaxy's potential prints.
impl fmt::Debug for EpochArc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EpochArc")
            .field("eta0", &self.eta0)
            .field("series_time0", &self.series_time0)
            .field("r0", &self.r0)
            .field("rest0", &self.rest0)
            .field("speed_sq", &self.speed_sq)
            .finish_non_exhaustive()
    }
}

impl EpochArc {
    /// The orbit's F, whose angular momentum is `momentum` m² s⁻¹.
    #[must_use]
    fn radial(&self, momentum: f64) -> Radial<'_> {
        Radial {
            potential: &self.potential,
            r0: self.r0,
            rest0: self.rest0,
            speed_sq: self.speed_sq,
            momentum_sq: momentum * momentum,
        }
    }
}

impl SphericalOrbit {
    /// The exact orbit in `potential` through the state `orbit` holds, its Kepler orbit about
    /// μ = G M(<r₀) ([`CentreMotion::kepler_orbit`]), with the Schwarzschild advance about the
    /// black hole; `None`, so that it keeps the two-body law (module documentation), if it has no
    /// apocentre inside the table, if a bracket of a turning point does not close inside it, or if
    /// rounding leaves F not positive at a node or κ² not positive on a near-circular orbit, which
    /// no tested orbit meets.
    ///
    /// Its state is the series' at every time, version 26's law; from version 27 an orbit takes
    /// [`by_arc`](Self::by_arc)'s.
    #[must_use]
    pub fn new(potential: &CentrePotential, orbit: &KeplerOrbit) -> Option<Self> {
        Self::prepared(potential, orbit, NEAR_CIRCULAR, TURNING_POINT_STEPS)
    }

    /// [`new`](Self::new)'s orbit, whose state within half a radial period of the epoch is taken
    /// by the arc from the epoch if its Δ is at least 0.03 (module documentation, "The arc from
    /// the epoch"; P09.T28.d.c, from version 27).
    ///
    /// It keeps a clone of `potential`, which the arc reads, only if it takes the arc, so a caller
    /// that prepares many orbits pays no reference count for the others. A galaxy's law at its
    /// version is [`Trajectory::new`](super::Trajectory::new)'s; this is the arm itself, for the
    /// benches and the tests.
    ///
    /// # Examples
    ///
    /// A star 0.1 ly from the Milky Way's black hole at 1.6 times its Kepler orbit's circular
    /// speed, bound in the centre's potential only some 10⁴ ly out. A second after the epoch the arc
    /// puts it where its Kepler orbit, whose pull at the epoch radius is the potential's, does; the
    /// series alone, whose epoch anomaly is out of phase on so wide an orbit, some 3 au from there.
    ///
    /// ```
    /// use std::sync::Arc;
    ///
    /// use hyperion_sim::coords::GalacticVelocity;
    /// use hyperion_sim::galaxy::PointLy;
    /// use hyperion_sim::galaxy::motion::spherical::{CentrePotential, SphericalOrbit};
    /// use hyperion_sim::galaxy::motion::{CentreMotion, KeplerOrbit};
    /// use hyperion_sim::galaxy::params::GalaxyParams;
    /// use hyperion_sim::galaxy::potential::MassModel;
    /// use hyperion_sim::units::consts::METRES_PER_LIGHT_YEAR;
    /// use hyperion_sim::units::{GravitationalParameter, LightYears, Seconds};
    ///
    /// let params = GalaxyParams::milky_way_like();
    /// let motion = CentreMotion::new(&params, &MassModel::new(&params))?;
    /// let potential = Arc::new(CentrePotential::new(&motion));
    /// let r = 0.1;
    /// let mu = GravitationalParameter::from_solar_masses(motion.enclosed_mass(LightYears::new(r)));
    /// let speed = 1.6 * (mu.value() / (r * METRES_PER_LIGHT_YEAR)).sqrt();
    /// let kepler = KeplerOrbit::from_state(
    ///     PointLy::new(r, 0.0, 0.0),
    ///     GalacticVelocity::new([0.3 * speed, 0.95 * speed, 0.0]),
    ///     mu,
    /// )?;
    /// let arc = SphericalOrbit::by_arc(&potential, &kepler).expect("bound in the potential");
    /// let series = SphericalOrbit::new(&potential, &kepler).expect("bound in the potential");
    /// assert!(arc.apocentre().value() > 1e4, "{} ly", arc.apocentre().value());
    /// let second = Seconds::new(1.0);
    /// let step = kepler.propagate(second).position();
    /// let off = |orbit: &SphericalOrbit| {
    ///     let [x, y, z] = orbit.position_velocity(second).0.metres();
    ///     let ly = METRES_PER_LIGHT_YEAR;
    ///     ((x / ly - step.x).powi(2) + (y / ly - step.y).powi(2) + (z / ly - step.z).powi(2)).sqrt()
    /// };
    /// assert!(off(&arc) < 1e-12, "{} ly", off(&arc));
    /// assert!(off(&series) > 1e-5, "{} ly", off(&series));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn by_arc(potential: &Arc<CentrePotential>, orbit: &KeplerOrbit) -> Option<Self> {
        Self::prepared_with(
            Table::Shared(potential),
            orbit,
            NEAR_CIRCULAR,
            TURNING_POINT_STEPS,
        )
    }

    /// [`new`](Self::new) with the near-circular threshold and the turning points' step count
    /// given, for the tests that measure them.
    #[must_use]
    fn prepared(
        potential: &CentrePotential,
        orbit: &KeplerOrbit,
        near_circular: f64,
        steps: u32,
    ) -> Option<Self> {
        Self::prepared_with(Table::Alone(potential), orbit, near_circular, steps)
    }

    /// [`prepared`](Self::prepared) in `table`, and in a shared one, if the orbit's Δ is at least
    /// [`ARC_LEAST_RANGE`], with the arc from the epoch ([`by_arc`](Self::by_arc)).
    #[must_use]
    fn prepared_with(
        table: Table<'_>,
        orbit: &KeplerOrbit,
        near_circular: f64,
        steps: u32,
    ) -> Option<Self> {
        let potential = table.potential();
        let (position, velocity) = (orbit.position, orbit.velocity);
        let r0 = orbit.radius;
        let h = cross(position, velocity);
        let momentum = length(h);
        let radial_speed = dot(position, velocity) / r0;
        let radial = Radial {
            potential,
            r0,
            rest0: potential.rest_at(r0)[0],
            speed_sq: radial_speed * radial_speed,
            momentum_sq: momentum * momentum,
        };
        let reach = potential.reach_m();
        if radial.at(reach).0 >= 0.0 {
            return None;
        }
        let [r_p, r_a] = turning_points(&radial, orbit, reach, steps)?;
        let (log_p, log_a, log0) = (math::ln(r_p), math::ln(r_a), math::ln(r0));
        let log_mean = f64::midpoint(log_p, log_a);
        let circular = 0.5 * (log_a - log_p) < near_circular;
        // Δ: the epicycle's through the epoch state, or the turning points'.
        let inverse_kappa = if circular {
            let kappa_sq = radial.epicyclic_sq(math::exp(log_mean));
            if kappa_sq.is_nan() || kappa_sq <= 0.0 {
                return None;
            }
            1.0 / kappa_sq.sqrt()
        } else {
            0.0
        };
        let log_range = if circular {
            // d(ln r) ÷ dt = v_r0 ÷ r₀ = Δ κ sin η₀ and ln r₀ = ū − Δ cos η₀.
            math::hypot(radial_speed * inverse_kappa / r0, log_mean - log0)
        } else {
            0.5 * (log_a - log_p)
        };
        let radii: [f64; HALF_NODES] =
            std::array::from_fn(|j| math::exp(log_mean - log_range * potential.basis[j][1]));
        // dt ÷ dη at the nodes: the epicycle's, or `r Δ sin η ÷ √F`.
        let mut rates = [inverse_kappa; HALF_NODES];
        if !circular {
            for ((rate, &r), j) in rates.iter_mut().zip(&radii).zip(0..) {
                let (f, _) = radial.at(r);
                if !(f.is_finite() && f > 0.0) {
                    return None;
                }
                *rate = r * log_range * math::sin(node(j)) / f.sqrt();
            }
        }
        let (mut time, mut angle) = ([0.0; TERMS], [0.0; TERMS]);
        for ((rate, basis), r) in rates.iter().zip(&potential.basis).zip(radii) {
            let turn = momentum * rate / (r * r);
            for (k, b) in basis.iter().enumerate() {
                time[k] += rate * b;
                angle[k] += turn * b;
            }
        }
        // The trapezoid rule over the N nodes, the mirror half included: 2 ÷ N for the constant
        // term, 4 ÷ N for the others, then each over its order for the integral.
        let n = f64::from(ANOMALY_NODES);
        for series in [&mut time, &mut angle] {
            series[0] *= 2.0 / n;
            for (k, c) in series.iter_mut().enumerate().skip(1) {
                *c *= 4.0 / (n * index_f64(k));
            }
        }
        let wobble = time[1..].iter().map(|c| c.abs()).sum();
        // The Schwarzschild advance a radial period about the black hole, 6π G M ÷ (c² p) with
        // `p = L² ÷ (G M)`, the semi-latus rectum of the angular momentum's Kepler orbit about it
        // (Weinberg 1972, *Gravitation and Cosmology*, §8.6): `KeplerOrbit::schwarzschild_rate`'s
        // advance an orbit, taken on the exact orbit's L (lean, 2026-10-30).
        let black_hole = potential.black_hole / (SPEED_OF_LIGHT * momentum);
        let mut exact = Self {
            axes: [
                position.map(|c| c / r0),
                cross(h.map(|c| c / momentum), position.map(|c| c / r0)),
            ],
            momentum,
            // The range r(η) spans, the turning points' or the epicycle's.
            turning: [
                math::exp(log_mean - log_range),
                math::exp(log_mean + log_range),
            ],
            log_mean,
            log_range,
            time,
            angle,
            wobble,
            epoch: [0.0; 2],
            advance: 3.0 * TAU * black_hole * black_hole,
            arc: None,
        };
        // The epoch's anomaly: cos η₀ = (ū − ln r₀) ÷ Δ and sin η₀ = v_r0 (dt ÷ dη)(η₀) ÷ (r₀ Δ),
        // the rate read at cos η₀ by the cosine series' Chebyshev form.
        let cos0 = if log_range > 0.0 {
            ((log_mean - log0) / log_range).clamp(-1.0, 1.0)
        } else {
            1.0
        };
        let eta0 = math::atan2(radial_speed * exact.rate_at_cos(cos0) / r0, log_mean - log0);
        let (sin, cos) = math::sin_cos(eta0);
        let [t0, _] = exact.time_at(eta0, sin, cos);
        exact.epoch = [t0, exact.angle_at(eta0, sin, cos)];
        exact.arc = match table {
            Table::Shared(shared) if !circular && log_range >= ARC_LEAST_RANGE => {
                Some(exact.epoch_arc(shared, &radial, [log_p, log_a, log0], radial_speed))
            }
            Table::Shared(_) | Table::Alone(_) => None,
        };
        Some(exact)
    }

    /// What the arc from the epoch reads (P09.T28.d.c), from the shared potential `shared`, the
    /// orbit's F `radial` in it, `[ln r_p, ln r_a, ln r₀]` and the epoch's radial speed, m s⁻¹.
    ///
    /// Its η₀ is the arc's own, from ln r₀ alone (module documentation): `tan(η₀ ÷ 2) =
    /// √((ln r₀ − ln r_p) ÷ (ln r_a − ln r₀))`, on the side `v_r0` gives, exact at both turning
    /// points, where the series' rate at cos η₀, which the series' η₀ reads, errs most on a wide
    /// orbit.
    #[must_use]
    fn epoch_arc(
        &self,
        shared: &Arc<CentrePotential>,
        radial: &Radial<'_>,
        [log_p, log_a, log0]: [f64; 3],
        radial_speed: f64,
    ) -> EpochArc {
        // Comparisons, not `max`, which may return either zero of a signed pair.
        let root = |x: f64| if x > 0.0 { x.sqrt() } else { 0.0 };
        let half = math::atan2(root(log0 - log_p), root(log_a - log0));
        // `0 − 2 half` is +0, not −0, at the pericentre itself.
        let eta0 = if radial_speed < 0.0 {
            0.0 - 2.0 * half
        } else {
            2.0 * half
        };
        let (sin, cos) = math::sin_cos(eta0);
        EpochArc {
            potential: Arc::clone(shared),
            eta0,
            series_time0: self.time_at(eta0, sin, cos)[0],
            r0: radial.r0,
            rest0: radial.rest0,
            speed_sq: radial.speed_sq,
        }
    }

    /// The radial period, the time from one pericentre to the next.
    #[must_use]
    pub fn radial_period(&self) -> Seconds {
        Seconds::new(TAU * self.time[0])
    }

    /// The turn in the orbit's plane from one pericentre to the next in the centre's potential
    /// alone, radians: Binney and Tremaine's Δψ a radial period (2008, §3.1), twice the classical
    /// apsidal angle from pericentre to apocentre; 2π for a Kepler orbit, π for a harmonic one.
    #[must_use]
    pub fn apsidal_angle_rad(&self) -> f64 {
        TAU * self.angle[0]
    }

    /// The Schwarzschild advance the orbit turns by on top a radial period, radians:
    /// `6π (G M_bh ÷ (c L))²`, spread over the orbit as its azimuth turns (module documentation).
    #[must_use]
    pub fn relativistic_advance_rad(&self) -> f64 {
        self.advance
    }

    /// The Schwarzschild advance's mean rate, the advance over the radial period, radians per
    /// Julian year.
    #[must_use]
    pub fn relativistic_rate(&self) -> PerYear {
        PerYear::new(self.advance / (TAU * self.time[0]) * SECONDS_PER_JULIAN_YEAR)
    }

    /// Δ, the anomaly's half-range in `ln r`, for the hold's tests.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn log_range(&self) -> f64 {
        self.log_range
    }

    /// Whether its state near the epoch is the arc's, for the hold's tests.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn takes_the_arc(&self) -> bool {
        self.arc.is_some()
    }

    /// The least distance from the black hole.
    #[must_use]
    pub fn pericentre(&self) -> LightYears {
        LightYears::new(self.turning[0] / METRES_PER_LIGHT_YEAR)
    }

    /// The greatest distance from the black hole.
    #[must_use]
    pub fn apocentre(&self) -> LightYears {
        LightYears::new(self.turning[1] / METRES_PER_LIGHT_YEAR)
    }

    /// The state `dt` after the epoch (before it for a negative `dt`): the position relative to
    /// the black hole, galactic axes, and the velocity.
    ///
    /// At `dt` = 0 it is the epoch state to the series' precision, some 10⁻¹² relative;
    /// [`Trajectory::state_at`](super::Trajectory::state_at) returns the epoch state itself there.
    #[must_use]
    pub fn position_velocity(&self, dt: Seconds) -> (GalacticDisplacement, GalacticVelocity) {
        let (position, velocity) = self.state_si(dt.value());
        (
            GalacticDisplacement::new(position),
            GalacticVelocity::new(velocity),
        )
    }

    /// [`position_velocity`](Self::position_velocity) in SI: m and m s⁻¹.
    #[must_use]
    pub(crate) fn state_si(&self, dt: f64) -> ([f64; 3], [f64; 3]) {
        if let Some(arc) = &self.arc
            && dt.abs() < ARC_SPAN * TAU * self.time[0]
            && let Some(state) = self.arc_state(arc, dt, ARC_STEPS)
        {
            return state;
        }
        self.series_state(dt)
    }

    /// The state `dt` s from the epoch by the series over the whole radial period (module
    /// documentation, "The orbit"), m and m s⁻¹.
    #[must_use]
    fn series_state(&self, dt: f64) -> ([f64; 3], [f64; 3]) {
        let period = TAU * self.time[0];
        let target = self.epoch[0] + dt;
        let turns = (target / period).floor();
        let within = (target - turns * period).clamp(0.0, period);
        let eta = self.anomaly(within, ANOMALY_STEPS);
        let (sin, cos) = math::sin_cos(eta);
        let r = math::exp(self.log_mean - self.log_range * cos);
        let [_, rate] = self.time_at(eta, sin, cos);
        let apsidal = self.apsidal_angle_rad();
        let swept = self.angle_at(eta, sin, cos) - self.epoch[1] + turns * apsidal;
        self.placed(r, sin, rate, swept)
    }

    /// The state at the anomaly whose sine is `sin`, where the radius is `r` m and `dt ÷ dη`
    /// `rate` s, the Newtonian azimuth swept since the epoch `swept` rad: m and m s⁻¹.
    #[must_use]
    fn placed(&self, r: f64, sin: f64, rate: f64, swept: f64) -> ([f64; 3], [f64; 3]) {
        // The advance in proportion to the Newtonian azimuth swept: the first-order relativistic
        // orbit, whose pericentre turns as the azimuth does.
        let apsidal = self.apsidal_angle_rad();
        let turn = swept + swept * (self.advance / apsidal);
        let (sin_turn, cos_turn) = math::sin_cos(turn);
        let [x, y] = self.axes;
        let out: [f64; 3] = [0, 1, 2].map(|i| cos_turn * x[i] + sin_turn * y[i]);
        let across: [f64; 3] = [0, 1, 2].map(|i| cos_turn * y[i] - sin_turn * x[i]);
        // The Newtonian velocity, turned with the position, as P09.T28.d.a's step turns it: the
        // advance turns the orbit, not the star's speed along it.
        let radial = r * self.log_range * sin / rate;
        let tangential = self.momentum / r;
        (
            out.map(|c| r * c),
            [0, 1, 2].map(|i| radial * out[i] + tangential * across[i]),
        )
    }

    /// The state `dt` s from the epoch by the arc from the epoch (module documentation, "The arc
    /// from the epoch"), by `steps` ([`ARC_STEPS`]) quadratures, and [`ARC_MORE_STEPS`] more if
    /// they leave its root off by more than [`ARC_RESIDUAL`] allows, m and m s⁻¹; `None`, for the
    /// series to answer instead, if those leave it off too or rounding leaves it not finite, which
    /// no tested orbit does.
    #[must_use]
    fn arc_state(&self, arc: &EpochArc, dt: f64, steps: u32) -> Option<([f64; 3], [f64; 3])> {
        // A root off by far more than the solve's measured noise, some 3 × 10⁻¹⁰ of the span
        // (`the_arc_s_fixed_step_count_converges`), and than η's own rounding, some 10⁻¹⁶ rad,
        // which outweighs it a second from the epoch, is not yet solved.
        let allowed = ARC_RESIDUAL * (dt.abs() + 1e-6 * TAU * self.time[0]);
        let solved = |root: &ArcRoot| (root.integrals[0] - dt).abs() <= allowed;
        let mut root = self.arc_root(arc, dt, steps);
        if !solved(&root) {
            // The same solve run on: its first `steps` quadratures are the same.
            root = self.arc_root(arc, dt, steps + ARC_MORE_STEPS);
            if !solved(&root) {
                return None;
            }
        }
        let state = self.placed(root.at.r, root.at.sin, root.at.rate, root.integrals[1]);
        state
            .0
            .iter()
            .chain(&state.1)
            .all(|c| c.is_finite())
            .then_some(state)
    }

    /// The anomaly where `t_arc(η) = dt`, s, by `steps` quadratures: the first at the series' root
    /// and each after a safeguarded Newton step from the last (a step that leaves the bracket,
    /// which each step's sign shrinks, is a bisection instead), with the last's integrals and rate.
    #[must_use]
    fn arc_root(&self, arc: &EpochArc, dt: f64, steps: u32) -> ArcRoot {
        let radial = arc.radial(self.momentum);
        let c0 = self.time[0];
        let period = TAU * c0;
        let target = arc.series_time0 + dt;
        let turns = (target / period).floor();
        let within = (target - turns * period).clamp(0.0, period);
        let start = self.anomaly(within, ANOMALY_STEPS) + TAU * turns;
        // The root lies on the epoch's side within a radial period of it, since `t_arc` is a whole
        // period there and |dt| under half of one, and within the wobble's bracket of the series'
        // root to the series' error.
        let (side_lo, side_hi) = if dt > 0.0 {
            (arc.eta0, arc.eta0 + TAU)
        } else {
            (arc.eta0 - TAU, arc.eta0)
        };
        // Comparisons, not `max` and `min`, which may return either zero of a signed pair.
        let (wobble_lo, wobble_hi) = ((target - self.wobble) / c0, (target + self.wobble) / c0);
        let (mut lo, mut hi) = (
            if wobble_lo > side_lo {
                wobble_lo
            } else {
                side_lo
            },
            if wobble_hi < side_hi {
                wobble_hi
            } else {
                side_hi
            },
        );
        let mut root = ArcRoot {
            eta: if start < lo {
                lo
            } else if start > hi {
                hi
            } else {
                start
            },
            integrals: [0.0; 2],
            at: ArcRate {
                rate: 0.0,
                r: 0.0,
                sin: 0.0,
            },
        };
        for step in 0..steps {
            root.integrals = self.arc_integrals(&radial, arc.eta0, root.eta);
            root.at = self.arc_rate(&radial, root.eta);
            if step + 1 == steps {
                break;
            }
            let residual = root.integrals[0] - dt;
            if residual < 0.0 {
                lo = root.eta;
            } else if residual > 0.0 {
                hi = root.eta;
            } else {
                continue;
            }
            let newton = root.eta - residual / root.at.rate;
            // On the closed bracket, as `turning_point`'s.
            root.eta = if newton >= lo && newton <= hi {
                newton
            } else {
                f64::midpoint(lo, hi)
            };
        }
        root
    }

    /// `[t_arc(to), ψ_arc(to)]` from `from`, s and rad: the 16-node Gauss–Legendre rule of
    /// `dt ÷ dη` and `L ÷ r² · dt ÷ dη` over `[from, to]`, a panel each side of a turning point it
    /// crosses (`η = kπ`), in order from `from`.
    #[must_use]
    fn arc_integrals(&self, radial: &Radial<'_>, from: f64, to: f64) -> [f64; 2] {
        let (low, high) = if from <= to { (from, to) } else { (to, from) };
        // The multiples of π strictly inside, at most two over an arc under a radial period.
        let first = (low / PI).floor() + 1.0;
        let crossed = [first, first + 1.0].map(|k| k * PI);
        let mut ends = [from, to, to, to];
        let mut count = 1;
        let order: [usize; 2] = if from <= to { [0, 1] } else { [1, 0] };
        for i in order {
            let turning = crossed[i];
            if turning > low && turning < high {
                ends[count] = turning;
                count += 1;
            }
        }
        ends[count] = to;
        let mut sums = [0.0; 2];
        for panel in ends[..=count].windows(2) {
            let (half, middle) = (
                0.5 * (panel[1] - panel[0]),
                f64::midpoint(panel[0], panel[1]),
            );
            let mut panel_sums = [0.0; 2];
            for (&x, &w) in GL16_NODES.iter().zip(&GL16_WEIGHTS) {
                let at = self.arc_rate(radial, middle + half * x);
                panel_sums[0] += w * at.rate;
                panel_sums[1] += w * (self.momentum * at.rate / (at.r * at.r));
            }
            sums[0] += half * panel_sums[0];
            sums[1] += half * panel_sums[1];
        }
        sums
    }

    /// `dt ÷ dη = r Δ |sin η| ÷ √F` at `eta`, s, with r and sin η: within F's rounding of a turning
    /// point ([`ARC_FLOOR`]), the limit there, `√(2 r Δ ÷ |F′|)`.
    #[must_use]
    fn arc_rate(&self, radial: &Radial<'_>, eta: f64) -> ArcRate {
        let (sin, cos) = math::sin_cos(eta);
        let log_r = self.log_mean - self.log_range * cos;
        let r = math::exp(log_r);
        let (f, slope, scale) = radial.at_scaled_with(r, radial.potential.rest_at_log(r, log_r));
        let rate = if f > ARC_FLOOR * scale {
            r * self.log_range * sin.abs() / f.sqrt()
        } else {
            (2.0 * r * self.log_range / slope.abs()).sqrt()
        };
        ArcRate { rate, r, sin }
    }

    /// The anomaly in `[0, 2π]` at `within` s after a pericentre, `[0, P]`: the root of
    /// `t(η) = within` by `steps` ([`ANOMALY_STEPS`]), the first from Danby's starter for
    /// Kepler's equation, `M + 0.85 e sign(sin M)` (Danby 1987, Celest. Mech. 40, 303; Danby 1988,
    /// *Fundamentals of Celestial Mechanics*, 2nd ed., §6.6; neither re-checked at source), with M
    /// and an e read off the time's first two terms, the next
    /// [`ANOMALY_BISECTIONS`] bisections of the bracket that `t(η) − c₀ η`'s bound and each step's
    /// sign keep, and the rest safeguarded Newton steps.
    #[must_use]
    fn anomaly(&self, within: f64, steps: u32) -> f64 {
        let c0 = self.time[0];
        let mean_anomaly = within / c0;
        let e = (-self.time[1] / c0).clamp(0.0, 1.0);
        let (mut lo, mut hi) = (
            ((within - self.wobble) / c0).max(0.0),
            ((within + self.wobble) / c0).min(TAU),
        );
        let lean = if mean_anomaly < PI { 0.85 } else { -0.85 };
        let mut eta = (mean_anomaly + lean * e).clamp(lo, hi);
        for step in 0..steps {
            let (sin, cos) = math::sin_cos(eta);
            let [t, rate] = self.time_at(eta, sin, cos);
            let residual = t - within;
            if residual < 0.0 {
                lo = eta;
            } else if residual > 0.0 {
                hi = eta;
            } else {
                continue;
            }
            let newton = eta - residual / rate;
            // On the closed bracket, as `turning_point`'s.
            eta = if step >= ANOMALY_BISECTIONS && newton >= lo && newton <= hi {
                newton
            } else {
                f64::midpoint(lo, hi)
            };
        }
        eta
    }

    /// `t(η)` and `dt ÷ dη` at `eta`, whose sine and cosine are `sin` and `cos`, s.
    #[must_use]
    fn time_at(&self, eta: f64, sin: f64, cos: f64) -> [f64; 2] {
        let (mut t, mut rate) = (self.time[0] * eta, self.time[0]);
        let (mut s, mut c) = ((0.0, sin), (1.0, cos));
        for (k, &coefficient) in self.time.iter().enumerate().skip(1) {
            t += coefficient * s.1;
            rate += index_f64(k) * coefficient * c.1;
            // sin and cos of (k + 1) η by Chebyshev's recurrence.
            s = (s.1, 2.0 * cos * s.1 - s.0);
            c = (c.1, 2.0 * cos * c.1 - c.0);
        }
        [t, rate]
    }

    /// `ψ(η)` at `eta`, whose sine and cosine are `sin` and `cos`, rad.
    #[must_use]
    fn angle_at(&self, eta: f64, sin: f64, cos: f64) -> f64 {
        let mut psi = self.angle[0] * eta;
        let mut s = (0.0, sin);
        for &coefficient in &self.angle[1..] {
            psi += coefficient * s.1;
            s = (s.1, 2.0 * cos * s.1 - s.0);
        }
        psi
    }

    /// `dt ÷ dη` where `cos η = x`, s: the cosine series as Chebyshev polynomials,
    /// `cos kη = T_k(x)`.
    #[must_use]
    fn rate_at_cos(&self, x: f64) -> f64 {
        let mut rate = self.time[0];
        let (mut previous, mut current) = (1.0, x);
        for (k, &coefficient) in self.time.iter().enumerate().skip(1) {
            rate += index_f64(k) * coefficient * current;
            (previous, current) = (current, 2.0 * x * current - previous);
        }
        rate
    }
}

/// Whether `x` is negative, a NaN not.
#[must_use]
fn negative(x: f64) -> bool {
    x < 0.0
}

/// The turning points `[r_p, r_a]` of `radial`, the orbit `orbit` holds in `radial`'s potential,
/// in m (module documentation): each by [`turning_point`]'s `steps` in a bracket from the Kepler
/// orbit's, widened by halving or doubling until F is negative at its end, and started from that
/// end, so that a turning point at the epoch, where F is zero, is not taken for the other: at an
/// epoch on a turning point the bracket on its other side converges onto the epoch radius, so the
/// sign of F′ there, which rounding can turn on a near-circular orbit, is never read. `None` if
/// the apocentre's would pass the table's `reach` m.
#[must_use]
fn turning_points(
    radial: &Radial<'_>,
    orbit: &KeplerOrbit,
    reach: f64,
    steps: u32,
) -> Option<[f64; 2]> {
    let r0 = radial.r0;
    let kepler_low = orbit.pericentre_m().min(r0);
    let kepler_high = if orbit.alpha > 0.0 {
        (2.0 / orbit.alpha - orbit.pericentre_m()).max(r0)
    } else {
        2.0 * r0
    };
    let mut lo = kepler_low;
    for _ in 0..BRACKET_STEPS {
        if negative(radial.at(lo).0) {
            break;
        }
        lo *= 0.5;
    }
    let mut hi = kepler_high.min(reach);
    for _ in 0..BRACKET_STEPS {
        if negative(radial.at(hi).0) {
            break;
        }
        hi = (2.0 * hi).min(reach);
    }
    if !(negative(radial.at(lo).0) && negative(radial.at(hi).0)) {
        return None;
    }
    // F(r₀) = v_r0² ≥ 0: each bracket holds a root, r₀ itself if the epoch is that turning point.
    let r_p = turning_point(radial, Turning::Pericentre, [lo, r0], lo, steps);
    let r_a = turning_point(radial, Turning::Apocentre, [r0, hi], hi, steps);
    Some([r_p, r_a])
}

#[cfg(test)]
pub(super) mod tests {
    use hyperion_testkit::float::{assert_same_bits, bits};
    use hyperion_testkit::golden::f64_digest;
    use hyperion_testkit::lcg::Lcg;
    use hyperion_testkit::order::assert_order_independent;

    use super::*;
    use crate::coords::GalacticPosition;
    use crate::galaxy::features::centre::{CentreProfile, LossCone, SlopeBreak, TracerShape};
    use crate::galaxy::motion::secular::SecularTable;
    use crate::galaxy::motion::{EpochState, Law, StateAt, Trajectory, position_velocity_at};
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::placement::{SystemOrigin, SystemRecord};
    use crate::galaxy::potential::MassModel;
    use crate::galaxy::rules::GalaxyRules;
    use crate::galaxy::{Galaxy, PointLy, Population};
    use crate::id::{CentreMemberId, Layer, MemberSlot, SystemId};
    use crate::rng::Seed;
    use crate::time::UniverseTime;
    use crate::units::consts::RADIANS_PER_DEGREE;
    use crate::units::{SolarMasses, Years};

    const YEAR: f64 = SECONDS_PER_JULIAN_YEAR;

    /// Arcminutes in one radian.
    const ARCMIN_PER_RADIAN: f64 = 60.0 / RADIANS_PER_DEGREE;

    /// The Milky Way fixture's centre motion, built from its parameters without a galaxy.
    fn milky_way() -> CentreMotion {
        let params = GalaxyParams::milky_way_like();
        CentreMotion::new(&params, &MassModel::new(&params)).expect("the fixture's centre")
    }

    /// μ = G M(<r) at `r` ly, the Kepler orbit's.
    fn mu_at(motion: &CentreMotion, r: f64) -> f64 {
        GravitationalParameter::from_solar_masses(motion.enclosed_mass(LightYears::new(r))).value()
    }

    /// The orbit from `r` ly on +x at `factor` times the Kepler orbit's circular speed there,
    /// `tilt` radians outward from the tangent and inclined by 0.3 rad out of the x-y plane: the
    /// states of P09.T28.d.a's test.
    fn orbit_at(motion: &CentreMotion, r: f64, factor: f64, tilt: f64) -> KeplerOrbit {
        let mu = mu_at(motion, r);
        let speed = factor * (mu / (r * METRES_PER_LIGHT_YEAR)).sqrt();
        let (sin, cos) = math::sin_cos(tilt);
        let (sin_i, cos_i) = math::sin_cos(0.3);
        KeplerOrbit::from_state(
            PointLy::new(r, 0.0, 0.0),
            GalacticVelocity::new([speed * sin, speed * cos * cos_i, speed * cos * sin_i]),
            GravitationalParameter::new(mu),
        )
        .expect("a valid state")
    }

    /// A black hole of `black_hole` M☉ in a cluster of uniform density holding `mass` M☉ inside
    /// the reach, out to its break at 10³ ly: a harmonic core about a point mass, to some 10⁻⁵ at
    /// 3 ly, whose depth is not so many times what an orbit spans that F rounds away.
    fn harmonic(black_hole: f64, mass: f64) -> CentreMotion {
        let shape = TracerShape::new(
            0.0,
            [SlopeBreak {
                radius: 1e3,
                sharpness: 2.0,
                rise: 4.0,
            }],
        )
        .expect("a valid shape");
        let profile =
            CentreProfile::from_shape(shape, SolarMasses::new(mass), SolarMasses::new(black_hole))
                .expect("a valid profile");
        CentreMotion {
            profile,
            influence_radius: LightYears::new(10.0),
            loss_cone: LossCone::new(SolarMasses::new(black_hole)),
        }
    }

    /// G M(<r) at `r` m of everything in Ψ: the black hole, the cluster and the rest of the
    /// galaxy's monopole with its dark halo, from the profile itself and not the table, m³ s⁻².
    pub(in crate::galaxy::motion) fn profile_pull(
        profile: &CentreProfile,
    ) -> impl Fn(f64) -> f64 + '_ {
        |r| {
            GravitationalParameter::from_solar_masses(
                profile.total_enclosed_mass(r / METRES_PER_LIGHT_YEAR),
            )
            .value()
        }
    }

    /// G M(<r) at `r` m of the table's Ψ, `−r² Ψ′(r)`, m³ s⁻².
    fn table_pull(potential: &CentrePotential) -> impl Fn(f64) -> f64 + '_ {
        |r| potential.black_hole - r * r * potential.rest_at(r)[1]
    }

    /// `−G M(<r) r̂ ÷ r²` at `x` m with `pull` giving G M(<r), m s⁻².
    fn acceleration(pull: &dyn Fn(f64) -> f64, x: [f64; 3]) -> [f64; 3] {
        let r = length(x);
        let gm = pull(r);
        x.map(|c| -gm * c / (r * r * r))
    }

    /// The position `span` s after the state `(start, speed)` (m, m s⁻¹) in the potential whose
    /// mass inside is `pull`'s, by the classical fourth-order Runge–Kutta method with steps of
    /// `eta` local dynamical times, and the azimuth it sweeps about the start's angular momentum,
    /// rad: the reference.
    pub(in crate::galaxy::motion) fn integrate(
        pull: &dyn Fn(f64) -> f64,
        start: [f64; 3],
        speed: [f64; 3],
        span: f64,
        eta: f64,
    ) -> ([f64; 3], f64) {
        let h = cross(start, speed);
        let axis = h.map(|c| c / length(h));
        let (mut position, mut velocity, mut done, mut swept) = (start, speed, 0.0, 0.0);
        let add = |a: [f64; 3], b: [f64; 3], step: f64| [0, 1, 2].map(|i| a[i] + step * b[i]);
        while (span - done).abs() > 0.0 {
            let radius = length(position);
            let dynamical = (radius * radius * radius / pull(radius)).sqrt();
            let step = (eta * dynamical).min((span - done).abs()) * span.signum();
            let k1 = (velocity, acceleration(pull, position));
            let k2 = (
                add(velocity, k1.1, step / 2.0),
                acceleration(pull, add(position, k1.0, step / 2.0)),
            );
            let k3 = (
                add(velocity, k2.1, step / 2.0),
                acceleration(pull, add(position, k2.0, step / 2.0)),
            );
            let k4 = (
                add(velocity, k3.1, step),
                acceleration(pull, add(position, k3.0, step)),
            );
            let next = [0, 1, 2].map(|i| {
                position[i] + step / 6.0 * (k1.0[i] + 2.0 * k2.0[i] + 2.0 * k3.0[i] + k4.0[i])
            });
            swept += math::atan2(dot(cross(position, next), axis), dot(position, next));
            position = next;
            velocity = [0, 1, 2].map(|i| {
                velocity[i] + step / 6.0 * (k1.1[i] + 2.0 * k2.1[i] + 2.0 * k3.1[i] + k4.1[i])
            });
            done = if (span - done).abs() <= step.abs() {
                span
            } else {
                done + step
            };
        }
        (position, swept)
    }

    pub(in crate::galaxy::motion) fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
        length([0, 1, 2].map(|i| a[i] - b[i]))
    }

    /// `x` turned by `angle` about the angular momentum of the state `(position, velocity)`:
    /// a Newtonian reference turned by the Schwarzschild advance the exact orbit adds on top.
    pub(in crate::galaxy::motion) fn turned(
        x: [f64; 3],
        (position, velocity): ([f64; 3], [f64; 3]),
        angle: f64,
    ) -> [f64; 3] {
        let h = cross(position, velocity);
        let axis = h.map(|c| c / length(h));
        let (sin, cos) = math::sin_cos(angle);
        let across = cross(axis, x);
        let along = dot(axis, x) * (1.0 - cos);
        [0, 1, 2].map(|i| x[i] * cos + across[i] * sin + axis[i] * along)
    }

    /// A centre member's record at `p`, whose velocity is its own.
    fn member_at(p: PointLy) -> SystemRecord {
        let slot = MemberSlot::InCell {
            band: Layer::A,
            level: 4,
            cell: [0, 0, 0],
            index: 0,
        };
        SystemRecord::from_parts(
            SystemId::from(CentreMemberId::new(slot).expect("a canonical slot")),
            GalacticPosition::from_light_years([p.x, p.y, p.z]).expect("near the centre"),
            SystemOrigin::CentreMember { attempt: 0 },
            Population::NuclearDisc,
            SolarMasses::new(1.0),
            Years::new(1e9),
        )
    }

    /// The table holds Ψ: against the profile's own, from 10⁻⁶ to 10⁵ ly, the whole and the rest
    /// beside the black hole's term, and the slope against the pull of everything inside.
    #[test]
    fn the_table_holds_the_centre_s_potential() {
        let motion = milky_way();
        let potential = CentrePotential::new(&motion);
        let profile = motion.profile();
        let mut lcg = Lcg::new(0x0928_d0b0_0000_0001);
        let (mut whole, mut rest, mut pull) = (0.0_f64, 0.0_f64, 0.0_f64);
        let (mut where_pull, mut inner) = (0.0, 0.0_f64);
        for _ in 0..2_000 {
            let r = 1e-6 * math::exp10(11.0 * lcg.next_f64());
            let r_m = r * METRES_PER_LIGHT_YEAR;
            let direct = profile.psi(r) * M2_S2_PER_KM2_S2;
            whole = whole.max((potential.psi_j_per_kg(LightYears::new(r)) / direct - 1.0).abs());
            let outer = (profile.outer_psi(r) + G * profile.stellar_mass_within(r).value() / r)
                * M2_S2_PER_KM2_S2;
            rest = rest.max((potential.rest_at(r_m)[0] / outer - 1.0).abs());
            let slope = -potential.black_hole / (r_m * r_m) + potential.rest_at(r_m)[1];
            let direct_slope = profile.psi_slope(r) * M2_S2_PER_KM2_S2 / METRES_PER_LIGHT_YEAR;
            let error = (slope / direct_slope - 1.0).abs();
            if error > pull {
                pull = error;
                where_pull = r;
            }
            if r < 200.0 {
                inner = inner.max(error);
            }
        }
        // The profile's own Ψ against its own pull there, by a central difference.
        let h = 1e-5;
        let own = (profile.psi(where_pull * (1.0 + h)) - profile.psi(where_pull * (1.0 - h)))
            / (2.0 * h * where_pull)
            / profile.psi_slope(where_pull)
            - 1.0;
        println!(
            "worst relative errors: Ψ {whole:.2e}, its rest {rest:.2e}, Ψ′ {pull:.2e} (at \
             {where_pull:.3e} ly, where the profile's own Ψ′ is {own:.2e} off its pull), Ψ′ \
             inside 200 ly {inner:.2e}"
        );
        // The profile's Ψ and its mass inside are tabulated apart (the cluster's and the
        // monopole's tables), and part by some 10⁻⁴ between their knots; the table follows Ψ's
        // values and the mass's pull at its own.
        assert!(
            whole < 1e-6 && rest < 1e-6 && inner < 3e-4 && pull < 1e-3,
            "{whole} {rest} {inner} {pull}"
        );
    }

    /// The closed forms: a harmonic core alone has the radial period π ÷ Ω and the apsidal angle
    /// π, a point mass alone Kepler's period and 2π, and a weak harmonic core about a point mass
    /// turns the pericentre back by `3π √(1 − e²) M★(<a) ÷ M_bh` an orbit at first order in the
    /// core's mass (P09.T28.d.a's harmonic rate, `−(3 ÷ 2) n √(1 − e²) G M★(<a) ÷ μ`), to that
    /// order's own error, some `M★(<a) ÷ M_bh`. The core is uniform to some 10⁻⁵ out to 3 ly.
    #[test]
    fn a_harmonic_core_about_a_point_mass_keeps_its_closed_forms() {
        let check = |motion: &CentreMotion,
                     expected: &dyn Fn(&KeplerOrbit, &SphericalOrbit) -> [f64; 2],
                     tolerance: [f64; 2]| {
            let potential = CentrePotential::new(motion);
            let mut worst = [0.0_f64; 2];
            for factor in [0.3, 0.6, 0.9, 1.05, 1.2] {
                for r in [0.05, 0.5, 3.0] {
                    let orbit = orbit_at(motion, r, factor, 0.2);
                    let exact = SphericalOrbit::new(&potential, &orbit).unwrap_or_else(|| {
                        panic!("{r} ly ×{factor}: not bound in the table ({orbit:?})")
                    });
                    let [period, angle] = expected(&orbit, &exact);
                    let got = [exact.radial_period().value(), exact.apsidal_angle_rad()];
                    worst[0] = worst[0].max((got[0] / period - 1.0).abs());
                    worst[1] = worst[1].max((got[1] - angle).abs() / angle.abs());
                }
            }
            println!("worst relative errors (radial period, apsidal angle): {worst:?}");
            assert!(
                worst[0] < tolerance[0] && worst[1] < tolerance[1],
                "{worst:?}"
            );
        };
        // The core alone: Ω² = G M★(<r) ÷ r³, the same at every radius inside the break.
        let core = harmonic(0.0, 1e7);
        let omega_sq = mu_at(&core, 1.0)
            / (METRES_PER_LIGHT_YEAR * METRES_PER_LIGHT_YEAR * METRES_PER_LIGHT_YEAR);
        check(&core, &|_, _| [PI / omega_sq.sqrt(), PI], [1e-4, 1e-4]);
        // The point mass alone.
        let point = harmonic(4.297e6, 1e-6);
        check(
            &point,
            &|orbit, _| {
                let black_hole =
                    GravitationalParameter::from_solar_masses(SolarMasses::new(4.297e6));
                let a = 1.0 / orbit.alpha;
                [TAU * (a * a * a / black_hole.value()).sqrt(), TAU]
            },
            [1e-9, 1e-12],
        );
        // A core of 10⁻⁵ of the black hole inside 1 ly: the first order holds to its own order.
        let unit = harmonic(4.297e6, 1.0)
            .stars_within(LightYears::new(1.0))
            .value();
        let weak = harmonic(4.297e6, 42.97 / unit);
        let stars_at =
            |motion: &CentreMotion, a: f64| motion.stars_within(LightYears::new(a)).value();
        let potential = CentrePotential::new(&weak);
        let mut worst = 0.0_f64;
        for factor in [0.3, 0.6, 0.9, 1.1] {
            for r in [0.05, 0.5, 2.0] {
                let orbit = orbit_at(&weak, r, factor, 0.2);
                let exact = SphericalOrbit::new(&potential, &orbit).expect("bound");
                let a = 1.0 / orbit.alpha / METRES_PER_LIGHT_YEAR;
                let e = orbit.eccentricity();
                let share = stars_at(&weak, a) / 4.297e6;
                let first = -3.0 * PI * ((1.0 - e) * (1.0 + e)).sqrt() * share;
                let advance = exact.apsidal_angle_rad() - TAU;
                worst = worst.max((advance / first - 1.0).abs());
                assert!(share < 2e-4, "{share}");
            }
        }
        println!("weak core: the advance against the first order, worst relative {worst:.2e}");
        assert!(worst < 2e-3, "{worst}");
    }

    /// S2 (GRAVITY Collaboration 2020, A&A 636, L5: a = 0.0163 ly, e = 0.8846) and the plan's
    /// S2-like orbit (a = 0.0158 ly, e = 0.88) about the fixture's centre: the Schwarzschild
    /// advance is 12′ a radial period, P09.T28.d.a's rate's an orbit to 10⁻³ (its μ holds the stars
    /// inside the orbit, some 2 × 10⁻⁴ of the black hole); the stars turn the
    /// pericentre back by what T28.d.a's first-order rate says, to its order; and a radial period
    /// from apocentre the star is back at apocentre, turned by the two together.
    #[test]
    fn s2_advances_12_arcminutes_an_orbit() {
        let motion = milky_way();
        let potential = CentrePotential::new(&motion);
        let table = SecularTable::new(&motion);
        for (a, e) in [(0.0163, 0.8846), (0.0158, 0.88)] {
            let apocentre = a * (1.0 + e);
            let mu = mu_at(&motion, apocentre);
            let speed = (mu * (1.0 - e) / (apocentre * METRES_PER_LIGHT_YEAR)).sqrt();
            let orbit = KeplerOrbit::from_state(
                PointLy::new(apocentre, 0.0, 0.0),
                GalacticVelocity::new([0.0, speed, 0.0]),
                GravitationalParameter::new(mu),
            )
            .expect("a valid state");
            let exact = SphericalOrbit::new(&potential, &orbit).expect("bound");
            let period = exact.radial_period().value();
            let einstein = exact.relativistic_advance_rad() * ARCMIN_PER_RADIAN;
            // The advance an orbit of P09.T28.d.a's rate, about the black hole alone.
            let kepler = orbit
                .schwarzschild_rate(potential.black_hole())
                .expect("bound")
                .value()
                * orbit.period().expect("bound").value()
                / YEAR
                * ARCMIN_PER_RADIAN;
            let stars = (exact.apsidal_angle_rad() - TAU) * ARCMIN_PER_RADIAN;
            let first = orbit.mass_precession_rate(&table).expect("bound").value()
                * orbit.period().expect("bound").value()
                / YEAR
                * ARCMIN_PER_RADIAN;
            println!(
                "a {a} ly, e {e}: radial period {:.3} yr, {einstein:.3}′ relativistic, {stars:.4}′ \
                 from the stars (first order {first:.4}′)",
                period / YEAR
            );
            assert!((11.8..12.6).contains(&einstein), "{einstein}′");
            assert!(
                (einstein / kepler - 1.0).abs() < 1e-3,
                "{einstein}′ against T28.d.a's {kepler}′"
            );
            assert!(
                stars < 0.0 && (stars / first - 1.0).abs() < 0.02,
                "{stars}′ against {first}′"
            );
            let (back, _) = exact.state_si(period);
            let r = length(back) / METRES_PER_LIGHT_YEAR;
            assert!((r / apocentre - 1.0).abs() < 1e-9, "{r} ly");
            let turned = math::atan2(back[1], back[0]) * ARCMIN_PER_RADIAN;
            assert!(
                (turned - (einstein + stars)).abs() < 1e-3,
                "{turned}′ against {}′",
                einstein + stars
            );
        }
    }

    /// The direct integration's states from each radius, as [`orbit_at`]'s speed factor and tilt:
    /// a circular orbit, P09.T28.d.a's four eccentric ones, two of e 0.98 and 0.99, and four unbound
    /// about the mass inside, as most of the grid's are.
    const BOUND_CASES: [(f64, f64); 11] = [
        (1.0, 0.0),
        (0.7, 0.0),
        (1.25, 0.0),
        (0.9, 0.9),
        (1.1, -1.0),
        (0.15, 0.0),
        (0.1, 0.05),
        (1.3, 0.5),
        (1.3, -0.5),
        (1.42, -0.2),
        (1.6, 0.3),
    ];

    /// Research's sweep (research-leans-9 §1): from 0.1 and 0.3 ly, tilt 0.3 rad, at these
    /// multiples of the Kepler orbit's circular speed, orbits bound in Ψ only 60 to 2 × 10⁵ ly out,
    /// or not at all.
    const SWEEP: [f64; 7] = [1.45, 1.5, 1.55, 1.6, 1.65, 1.7, 2.0];

    /// A centre member's error against the direct integration from `kepler`'s state `years` from
    /// the epoch, in tenths of its tidal radius at pericentre for a solar mass, the plan's bound:
    /// by the series alone ([`Trajectory::in_potential`]), under `galaxy`'s law, the arc from the
    /// epoch, and by the two-body law; with the orbit's Δ, or `None` if the state is not bound in
    /// Ψ. The reference is Newtonian, turned by the Schwarzschild advance the
    /// law adds on top (P09.T28.d.a tested its step Newtonian too), and held to a hundredth of the
    /// bound by its own convergence.
    fn errors_against_the_reference(
        galaxy: &Galaxy,
        pull: &dyn Fn(f64) -> f64,
        kepler: &KeplerOrbit,
        years: i64,
    ) -> Option<(f64, [f64; 3])> {
        let motion = galaxy.centre_motion();
        let start = EpochState::new(member_at(kepler.position()), kepler.velocity())
            .expect("a finite state");
        let orbit = motion.kepler_orbit(&start).expect("an orbit");
        let bound = 0.1
            * motion
                .tidal_radius_at_pericentre(&orbit, SolarMasses::new(1.0))
                .value();
        let series = Trajectory::in_potential(motion, galaxy.centre_potential(), start);
        let exact = series.exact_orbit()?.clone();
        let t = UniverseTime::from_julian_years(years).expect("in the clock window");
        let span = t.since_epoch().as_seconds_f64();
        let (truth, _) = integrate(pull, orbit.position, orbit.velocity, span, 2e-3);
        let (finer, swept) = integrate(pull, orbit.position, orbit.velocity, span, 1e-3);
        assert!(
            distance(finer, truth) < 1e-2 * bound,
            "{:.3} ly: the reference converges to {}",
            orbit.radius / METRES_PER_LIGHT_YEAR,
            distance(finer, truth) / bound
        );
        // The Newtonian reference turned by the one secular rate the law adds, in proportion to
        // the azimuth it sweeps.
        let reference = turned(
            finer,
            (orbit.position, orbit.velocity),
            swept * exact.advance / exact.apsidal_angle_rad(),
        );
        let error = |at: StateAt| {
            let StateAt::Exists(state) = at else {
                panic!("outside the loss cone")
            };
            let got = GalacticPosition::ORIGIN
                .displacement_to(state.position())
                .metres();
            distance(got, reference) / bound
        };
        let two_body = distance(orbit.propagate(Seconds::new(span)).position, finer) / bound;
        Some((
            exact.log_range,
            [
                error(series.state_at(t)),
                error(position_velocity_at(galaxy, &start, t)),
                two_body,
            ],
        ))
    }

    /// The plan's test (P09.T28.d.b, replacing P09.T28.d.a's pinned misses; P09.T28.d.c's arc):
    /// against a direct integration in Ψ, from the profile itself, over ±1,000 yr from 0.1, 1 and
    /// 5 ly, on a circular orbit, P09.T28.d.a's four eccentric ones, two of e 0.98 and 0.99 and
    /// four unbound about the mass inside their epoch radius, as most of the grid's are, a centre
    /// member's position by [`position_velocity_at`] is within a tenth of its tidal radius at
    /// pericentre for a solar mass (P09.T28.c), the plan's bound, where the two-body law misses it
    /// on every eccentric orbit:
    ///
    /// - by the series alone, computed directly, on every orbit but one from 0.1 ly at 1.6 times
    ///   the circular speed, bound only at 1.7 × 10⁴ ly, which misses by some 173 tenths in
    ///   `MGE_BOXY`'s potential (P15.T3.c; some 22 before it), half the two-body law's 345 (a
    ///   finding, pinned as it stands: research-leans-9);
    /// - under every 27 hold, the arc from the epoch, on every orbit, that one included, and on
    ///   research's sweep ([`SWEEP`]) at ±100 and ±1,000 yr, where the series misses by up to some
    ///   3,900 tenths.
    #[test]
    fn the_exact_orbit_meets_the_plan_s_bound_against_a_direct_integration() {
        let seed = Seed::new(0x0928_d0b0_0000_0002);
        let arc = Galaxy::from_params_with_rules(
            seed,
            GalaxyParams::milky_way_like(),
            GalaxyRules::every_27_hold(),
        )
        .expect("the Milky Way fixture builds under every 27 hold");
        let motion = arc.centre_motion();
        let pull = profile_pull(motion.profile());
        let (mut widest, mut arc_worst) = (0.0_f64, 0.0_f64);
        for r in [0.1, 1.0, 5.0] {
            let mut worst = [0.0_f64; 2];
            for (factor, tilt) in BOUND_CASES {
                for years in [-1_000, 1_000] {
                    let kepler = orbit_at(motion, r, factor, tilt);
                    let (log_range, errors) = errors_against_the_reference(
                        &arc, &pull, &kepler, years,
                    )
                    .unwrap_or_else(|| panic!("{r} ly, ×{factor} at {tilt}: not the exact orbit"));
                    println!(
                        "r {r} ly, ×{factor} at {tilt} rad (Δ {log_range:.2e}), {years} yr: \
                         series {:.2e}, arc {:.2e}, two-body {:.3e}",
                        errors[0], errors[1], errors[2]
                    );
                    arc_worst = arc_worst.max(errors[1]);
                    // Under the series an orbit whose apocentre lies some 10⁵ times its pericentre
                    // out misses (a finding, pinned as it stands): its series' η₀ is out of phase.
                    if log_range > 5.0 {
                        widest = widest.max(errors[0]);
                        assert!(errors[0] < errors[2], "{r} ly, ×{factor}: {errors:?}");
                        continue;
                    }
                    worst[0] = worst[0].max(errors[0]);
                    worst[1] = worst[1].max(errors[2]);
                }
            }
            println!(
                "r {r} ly, worst in tenths of the tidal radius: series {:.2e}, two-body {:.4e}",
                worst[0], worst[1]
            );
            assert!(worst[0] < 1.0, "{r} ly: {worst:?}");
        }
        println!("the widest orbit, apocentre 1.7 × 10⁴ ly from 0.1 ly: series {widest:.2} tenths");
        assert!((150.0..200.0).contains(&widest), "{widest}");
        let mut bound_in_psi = 0;
        for r in [0.1, 0.3] {
            for factor in SWEEP {
                for years in [-1_000, -100, 100, 1_000] {
                    let kepler = orbit_at(motion, r, factor, 0.3);
                    let Some((log_range, errors)) =
                        errors_against_the_reference(&arc, &pull, &kepler, years)
                    else {
                        continue;
                    };
                    bound_in_psi += 1;
                    println!(
                        "sweep r {r} ly, ×{factor} (Δ {log_range:.2}), {years} yr: series \
                         {:.3e}, arc {:.3e}, two-body {:.3e}",
                        errors[0], errors[1], errors[2]
                    );
                    arc_worst = arc_worst.max(errors[1]);
                }
            }
        }
        println!("under the arc, the worst of every case and the sweep: {arc_worst:.2e} tenths");
        assert!(
            bound_in_psi >= 4 * 12,
            "{bound_in_psi} state-times bound in Ψ"
        );
        assert!(arc_worst < 1.0, "{arc_worst} tenths");
    }

    /// At the epoch the trajectory is the epoch state to the bit, and the exact orbit's own state
    /// there agrees to 10⁻¹²; the same state prepared twice, from tables built twice, gives the
    /// same bits at every time; and the orbit's coefficients, the table's knots and some states
    /// are pinned by digest, as blessed natively on x86-64 (named `pinned` for the wasip1 suite).
    #[test]
    fn the_epoch_comes_back_to_the_bit_and_the_orbit_is_pinned() {
        let motion = milky_way();
        let (potential, again) = (CentrePotential::new(&motion), CentrePotential::new(&motion));
        for (one, two) in potential.rest.iter().zip(&again.rest) {
            assert_same_bits(one[0], two[0]);
            assert_same_bits(one[1], two[1]);
        }
        let mut digest = Vec::new();
        digest.extend(potential.rest.iter().flatten());
        for (r, factor, tilt) in [(0.1, 0.7, 0.2), (1.0, 1.0, 0.0), (5.0, 0.15, -0.4)] {
            let orbit = orbit_at(&motion, r, factor, tilt);
            let start = EpochState::new(member_at(orbit.position()), orbit.velocity())
                .expect("a finite state");
            let trajectory = Trajectory::in_potential(&motion, &potential, start);
            let StateAt::Exists(epoch) = trajectory.state_at(UniverseTime::EPOCH) else {
                panic!("outside the loss cone")
            };
            assert_eq!(epoch.position(), start.record().epoch_position());
            assert_eq!(epoch.velocity(), start.velocity());
            let orbit = motion.kepler_orbit(&start).expect("an orbit");
            let exact = SphericalOrbit::new(&potential, &orbit).expect("bound");
            let twice = SphericalOrbit::new(&again, &orbit).expect("bound");
            let (near, speed) = exact.state_si(0.0);
            let (off, slip) = (
                distance(near, orbit.position) / orbit.radius,
                distance(speed, orbit.velocity) / length(orbit.velocity),
            );
            println!("{r} ly ×{factor}: the series' epoch {off:.2e} and {slip:.2e} relative off");
            assert!(off < 1e-9 && slip < 1e-9, "{r} ly: {off} {slip}");
            digest.extend(exact.time);
            digest.extend(exact.angle);
            digest.extend(exact.epoch);
            for years in [-1_000.0, -3.5, 0.25, 100.0, 1_000.0] {
                let (a, b) = (exact.state_si(years * YEAR), twice.state_si(years * YEAR));
                for i in 0..3 {
                    assert_same_bits(a.0[i], b.0[i]);
                    assert_same_bits(a.1[i], b.1[i]);
                }
                digest.extend(a.0);
                digest.extend(a.1);
            }
        }
        assert_eq!(f64_digest(&digest), PINNED_DIGEST);
    }

    /// The digest of [`the_epoch_comes_back_to_the_bit_and_the_orbit_is_pinned`]'s values, blessed
    /// natively on x86-64. Re-pinned by name in `MGE_BOXY`'s potential (P15.T3.c; it was
    /// `0x0244_8142_1b85_dfb4`), and at generator version 26 (it was `0xccea_0168_ca38_9fbe`).
    const PINNED_DIGEST: u64 = 0xb037_aac9_5363_e107;

    /// The orbits [`the_arc_comes_back_to_the_epoch_and_is_pinned`] follows, as [`orbit_at`]'s
    /// radius, speed factor and tilt: [`the_epoch_comes_back_to_the_bit_and_the_orbit_is_pinned`]'s
    /// three and three of the sweep's wide ones.
    const ARC_ORBITS: [(f64, f64, f64); 6] = [
        (0.1, 0.7, 0.2),
        (1.0, 1.0, 0.0),
        (5.0, 0.15, -0.4),
        (0.1, 1.6, 0.3),
        (0.1, 1.7, 0.3),
        (0.3, 2.0, 0.3),
    ];

    /// Under the arc from the epoch (P09.T28.d.c): a second from the epoch the state is the Kepler
    /// orbit's to 10⁻¹² relative; the same state prepared twice, from tables built twice, gives the
    /// same bits at every time, whatever was asked before it; at half a radial period and beyond,
    /// and on the circular orbit, the state is the series';
    /// and states at times within half a radial period are pinned by digest, as blessed natively
    /// on x86-64 (named `pinned` for the wasip1 suite).
    #[test]
    fn the_arc_comes_back_to_the_epoch_and_is_pinned() {
        let motion = milky_way();
        let (potential, again) = (
            Arc::new(CentrePotential::new(&motion)),
            Arc::new(CentrePotential::new(&motion)),
        );
        let mut digest = Vec::new();
        for (r, factor, tilt) in ARC_ORBITS {
            let orbit = orbit_at(&motion, r, factor, tilt);
            let (exact, twice) = (
                SphericalOrbit::by_arc(&potential, &orbit).expect("bound"),
                SphericalOrbit::by_arc(&again, &orbit).expect("bound"),
            );
            let Some(arc) = &exact.arc else {
                assert!(
                    exact.log_range < 1e-3,
                    "{r} ly ×{factor}: only a circle keeps the series"
                );
                assert_eq!(exact.state_si(YEAR), exact.series_state(YEAR));
                continue;
            };
            assert_same_bits(arc.eta0, twice.arc.as_ref().expect("the same orbit").eta0);
            for second in [-1.0, 1.0] {
                // A second along the Kepler orbit, whose pull at the epoch radius is Ψ's.
                let step = orbit.propagate(Seconds::new(second));
                let (near, speed) = exact.state_si(second);
                let (off, slip) = (
                    distance(near, step.position) / orbit.radius,
                    distance(speed, step.velocity) / length(orbit.velocity),
                );
                println!("{r} ly ×{factor}: a second out, {off:.2e} and {slip:.2e} relative off");
                assert!(
                    off < 1e-12 && slip < 1e-12,
                    "{r} ly ×{factor}: {off} {slip}"
                );
            }
            let period = exact.radial_period().value();
            let times: Vec<f64> = [-1_000.0, -100.0, -3.5, 0.25, 100.0, 1_000.0]
                .into_iter()
                .map(|years| years * YEAR)
                .chain([-0.45, -0.1, 0.1, 0.45].map(|share| share * period))
                .filter(|dt| dt.abs() < ARC_SPAN * period)
                .collect();
            let state_bits = |orbit: &SphericalOrbit, dt: f64| {
                let (x, v) = orbit.state_si(dt);
                x.iter().chain(&v).map(|&c| bits(c)).collect::<Vec<u64>>()
            };
            assert_order_independent(&times, |&dt| state_bits(&exact, dt));
            // At half a radial period and beyond, the series'.
            for dt in [
                -ARC_SPAN * period,
                ARC_SPAN * period,
                0.7 * period,
                -2.5 * period,
            ] {
                assert_eq!(state_bits(&exact, dt), state_bits(&twice, dt));
                let (x, v) = exact.series_state(dt);
                let series: Vec<u64> = x.iter().chain(&v).map(|&c| bits(c)).collect();
                assert_eq!(state_bits(&exact, dt), series, "{r} ly ×{factor} at {dt} s");
            }
            for &dt in &times {
                assert_eq!(
                    state_bits(&exact, dt),
                    state_bits(&twice, dt),
                    "{r} ly ×{factor} at {dt} s"
                );
                let (x, v) = exact.state_si(dt);
                digest.extend(x);
                digest.extend(v);
            }
        }
        let mut pins = hyperion_testkit::pins!(
            "galaxy/motion/spherical",
            "the_arc_comes_back_to_the_epoch_and_is_pinned"
        );
        pins.exact("digest", f64_digest(&digest));
    }

    /// The arc's fixed step count converges: over a stress set of orbits bound in Ψ from 10⁻³ to
    /// 100 ly, eccentricities to 0.999 and orbits unbound about the mass inside
    /// ([`the_fixed_step_counts_converge`]'s), at spans to half a radial period and at ±100 and
    /// ±1,000 yr within it, every state by [`ARC_STEPS`] quadratures is what four times as many
    /// give, to 10⁻⁴ of the plan's bound and 10⁻¹² of the radius, and stays there for every count
    /// after; and the root's residual in `t_arc` is under 10⁻⁹ of the span. At four quadratures
    /// every state is there to 10⁻⁶ of the bound; past that the anomaly wanders by some 10⁻¹³ rad,
    /// the rounding of `t_arc` and, at nodes beside a turning point, [`ARC_FLOOR`]'s switch.
    #[test]
    fn the_arc_s_fixed_step_count_converges() {
        let motion = milky_way();
        let potential = Arc::new(CentrePotential::new(&motion));
        let mut lcg = Lcg::new(0x0928_d0c0_0000_0001);
        let (mut orbits, mut most, mut residual) = (0_u32, 0_u32, 0.0_f64);
        for _ in 0..300 {
            let r = 1e-3 * math::exp10(5.0 * lcg.next_f64());
            let factor = 0.03 + 1.6 * lcg.next_f64();
            let tilt = if lcg.next_f64() < 0.25 {
                0.0
            } else {
                PI * (lcg.next_f64() - 0.5)
            };
            let orbit = orbit_at(&motion, r, factor, tilt);
            let Some(exact) = SphericalOrbit::by_arc(&potential, &orbit) else {
                continue;
            };
            let Some(arc) = &exact.arc else {
                continue;
            };
            orbits += 1;
            let bound = 0.1
                * motion
                    .tidal_radius_at_pericentre(&orbit, SolarMasses::new(1.0))
                    .value();
            let period = exact.radial_period().value();
            let spans = [-1_000.0, -100.0, 100.0, 1_000.0]
                .map(|years| years * YEAR)
                .into_iter()
                .chain((0..4).map(|_| (lcg.next_f64() - 0.5) * 0.998 * period))
                .filter(|dt| dt.abs() < ARC_SPAN * period);
            for dt in spans {
                let at = |steps: u32| exact.arc_state(arc, dt, steps).map(|state| state.0);
                let converged = at(4 * ARC_STEPS).expect("a finite state");
                let root = exact.arc_root(arc, dt, 4 * ARC_STEPS);
                residual = residual.max((root.integrals[0] - dt).abs() / dt.abs());
                // The last count from which every one up to four times the fixed count is there.
                let needed = (1..=4 * ARC_STEPS)
                    .rev()
                    .find(|&steps| {
                        at(steps).is_none_or(|x| {
                            distance(x, converged) > 1e-4 * bound + 1e-12 * length(converged)
                        })
                    })
                    .map_or(1, |last_off| last_off + 1);
                most = most.max(needed);
            }
        }
        println!(
            "{orbits} orbits under the arc: states within {most} quadratures, the residual in \
             t_arc at most {residual:.2e} of the span"
        );
        assert!(orbits > 200, "{orbits}");
        assert!(most <= ARC_STEPS, "{most}");
        assert!(residual < 1e-9, "{residual}");
    }

    /// The fixed step counts converge: over a stress set from 10⁻³ to 100 ly, eccentricities to
    /// 0.999 and orbits unbound about the mass inside, every turning point is reached well within
    /// [`TURNING_POINT_STEPS`] (what four times as many give, to 10⁻¹³, the rounding of F, in which
    /// a converged root may wander by some 10⁻¹⁴) and every anomaly within
    /// [`ANOMALY_STEPS`] (what four times as many give, to 10⁻¹² rad), and each stays there for
    /// every count after, to twice and four times the fixed counts.
    #[test]
    fn the_fixed_step_counts_converge() {
        let motion = milky_way();
        let potential = CentrePotential::new(&motion);
        let mut lcg = Lcg::new(0x0928_d0b0_0000_0003);
        let (mut most_turning, mut most_anomaly, mut orbits) = (0_u32, 0_u32, 0_u32);
        for _ in 0..400 {
            let r = 1e-3 * math::exp10(5.0 * lcg.next_f64());
            let factor = 0.03 + 1.6 * lcg.next_f64();
            // A quarter start at a turning point, as the tests' tangential states do.
            let tilt = if lcg.next_f64() < 0.25 {
                0.0
            } else {
                PI * (lcg.next_f64() - 0.5)
            };
            let orbit = orbit_at(&motion, r, factor, tilt);
            let Some(converged) = SphericalOrbit::prepared(
                &potential,
                &orbit,
                NEAR_CIRCULAR,
                4 * TURNING_POINT_STEPS,
            ) else {
                assert_eq!(SphericalOrbit::new(&potential, &orbit), None);
                continue;
            };
            orbits += 1;
            let ends = |o: &SphericalOrbit| o.turning;
            let target = ends(&converged);
            let there = |steps: u32| {
                SphericalOrbit::prepared(&potential, &orbit, NEAR_CIRCULAR, steps).is_some_and(
                    |o| {
                        ends(&o)
                            .iter()
                            .zip(target)
                            .all(|(a, b)| (a - b).abs() <= 1e-13 * b)
                    },
                )
            };
            // The last count from which every one up to twice the fixed count is there.
            let needed = (1..=2 * TURNING_POINT_STEPS)
                .rev()
                .find(|&steps| !there(steps))
                .map_or(1, |last_off| last_off + 1);
            most_turning = most_turning.max(needed);
            let exact = SphericalOrbit::new(&potential, &orbit).expect("bound");
            let period = TAU * exact.time[0];
            for k in 0..16 {
                let within = period * (f64::from(k) + lcg.next_f64()) / 16.0;
                let root = exact.anomaly(within, 4 * ANOMALY_STEPS);
                // The last count from which every one up to four times the fixed count is there.
                let needed = (1..=4 * ANOMALY_STEPS)
                    .rev()
                    .find(|&steps| (exact.anomaly(within, steps) - root).abs() > 1e-12)
                    .map_or(1, |last_off| last_off + 1);
                most_anomaly = most_anomaly.max(needed);
            }
        }
        println!(
            "{orbits} orbits bound in Ψ: turning points within {most_turning} steps, anomalies \
             within {most_anomaly}"
        );
        assert!(orbits > 250, "{orbits}");
        assert!(most_turning <= TURNING_POINT_STEPS && most_anomaly <= ANOMALY_STEPS);
    }

    /// The near-circular threshold (Edges), measured: from orbits of Δ from 10⁻² down to
    /// a circle in Ψ, at 0.1, 1 and 5 ly, the series and the epicyclic limit each against a
    /// direct integration in the table's own Ψ over 1,000 yr, which leaves the table's error out,
    /// in millionths of the plan's bound. The series' error grows as Δ falls, the
    /// epicycle's as it rises; the one taken is under 4 × 10⁻⁴ of the bound everywhere (the
    /// epicycle's of a circle in Ψ, whose turning points are F's rounding at a double root, the
    /// worst: 318.5 millionths in `MGE_BOXY`'s potential, P15.T3.c, a finding pinned as it stands;
    /// research-leans-9).
    #[test]
    fn the_near_circular_threshold_joins_the_two_limits() {
        let motion = milky_way();
        let potential = CentrePotential::new(&motion);
        let profile = motion.profile();
        let pull = table_pull(&potential);
        let span = 1_000.0 * YEAR;
        let mut worst = 0.0_f64;
        for r in [0.1, 1.0, 5.0] {
            let r_m = r * METRES_PER_LIGHT_YEAR;
            // The circular speed in Ψ, which holds the halo the Kepler orbit's μ does not.
            let circular =
                (GravitationalParameter::from_solar_masses(profile.total_enclosed_mass(r)).value()
                    / r_m)
                    .sqrt();
            for offset in [
                1e-2, 3e-3, 1e-3, 3e-4, 1e-4, 3e-5, 1e-5, 3e-6, 1e-6, 3e-7, 1e-7, 1e-8, 0.0,
            ] {
                let speed = circular * (1.0 + offset);
                let orbit = KeplerOrbit::from_state(
                    PointLy::new(r, 0.0, 0.0),
                    GalacticVelocity::new([0.0, speed, 0.0]),
                    GravitationalParameter::new(mu_at(&motion, r)),
                )
                .expect("a valid state");
                let bound = 0.1
                    * motion
                        .tidal_radius_at_pericentre(&orbit, SolarMasses::new(1.0))
                        .value();
                let (truth, swept) = integrate(&pull, orbit.position, orbit.velocity, span, 1e-3);
                let error = |threshold: f64| {
                    SphericalOrbit::prepared(&potential, &orbit, threshold, TURNING_POINT_STEPS)
                        .map(|o| {
                            let reference = turned(
                                truth,
                                (orbit.position, orbit.velocity),
                                swept * o.advance / o.apsidal_angle_rad(),
                            );
                            (
                                o.log_range,
                                distance(o.state_si(span).0, reference) / bound * 1e6,
                            )
                        })
                };
                let (series, epicycle, taken) = (error(0.0), error(1.0), error(NEAR_CIRCULAR));
                println!(
                    "r {r} ly, speed ×(1 + {offset:e}): Δ {:.2e}; in millionths of the bound, \
                     series {:?}, epicycle {:?}",
                    taken.map_or(f64::NAN, |t| t.0),
                    series.map(|s| s.1),
                    epicycle.map(|e| e.1)
                );
                let (_, taken) = taken.expect("bound");
                worst = worst.max(taken);
            }
        }
        println!("the worst of the law as taken: {worst:.3} millionths of the bound");
        assert!(worst < 400.0, "{worst} millionths");
    }

    /// A barely bound orbit (P09.T28.d.c): the Milky Way fixture's rogue planet
    /// `0xd00020003fff0ae4`, 2 ly out under every 27 hold, slowed by 0.17%: bound only at 7.7 × 10⁵
    /// ly, a radial period of some 3.6 × 10⁹ yr, whose solve a century on [`ARC_STEPS`] leave
    /// 8.5 × 10⁻⁵ of the span off its root, 2.3 times what [`ARC_RESIDUAL`] allows, from a poor
    /// start in the series: the [`ARC_MORE_STEPS`] more solve it, so that its state is the arc's
    /// and not the series', out of phase by some 0.08 ly there. The rogue planet itself, bound at
    /// 8.2 × 10⁵ ly (8.3 when pinned), is off by 1.35 × 10⁻⁵ of the span after [`ARC_STEPS`], a
    /// third of what is allowed, already so at the train's base before 27's re-pin; slowed by 0.29%
    /// to 0.08% its solve needs more steps, by up to 2.26 times the allowance at 0.17%.
    #[test]
    fn the_arc_solves_a_barely_bound_orbit_with_more_steps() {
        let galaxy = Galaxy::from_params_with_rules(
            Seed::new(0x0928_b0a0_0000_0001),
            GalaxyParams::milky_way_like(),
            GalaxyRules::every_27_hold(),
        )
        .expect("the Milky Way fixture builds under every 27 hold")
        .with_full_potential();
        let id = SystemId::from_raw(0xd000_2000_3fff_0ae4).expect("a grid system's ID");
        let record =
            crate::galaxy::placement::resolve(&galaxy, id).expect("the rogue planet resolves");
        let velocity = EpochState::of_grid_record(&galaxy, &record)
            .velocity()
            .metres_per_second()
            .map(|v| v * 0.9983);
        let start = EpochState::new(record, crate::coords::GalacticVelocity::new(velocity))
            .expect("a finite state");
        let trajectory = Trajectory::new(&galaxy, start);
        let exact = trajectory.exact_orbit().expect("on an exact orbit");
        let arc = exact.arc.as_ref().expect("on the arc");
        assert!(
            exact.apocentre().value() > 1e5,
            "{} ly",
            exact.apocentre().value()
        );
        let dt = 100.0 * YEAR;
        let period = exact.radial_period().value();
        let allowed = ARC_RESIDUAL * (dt + 1e-6 * period);
        let miss = |steps: u32| (exact.arc_root(arc, dt, steps).integrals[0] - dt).abs();
        println!(
            "a century on, the root off by {:.2e} of the span after {ARC_STEPS} quadratures and \
             {:.2e} after {}",
            miss(ARC_STEPS) / dt,
            miss(ARC_STEPS + ARC_MORE_STEPS) / dt,
            ARC_STEPS + ARC_MORE_STEPS
        );
        assert!(miss(ARC_STEPS) > allowed && miss(ARC_STEPS + ARC_MORE_STEPS) < 1e-12 * dt);
        let state = exact.state_si(dt);
        assert_eq!(
            Some(state),
            exact.arc_state(arc, dt, ARC_STEPS),
            "the arc's, by more steps"
        );
        let apart = distance(state.0, exact.series_state(dt).0) / METRES_PER_LIGHT_YEAR;
        assert!(apart > 1e-2, "{apart} ly from the series'");
    }

    /// Near a circle (P09.T28.d.c): on orbits of Δ from 2 × 10⁻⁵ to 0.3 at 0.1, 1 and 5 ly, their
    /// epochs at the guiding radius and so midway between the turning points, those of Δ under
    /// [`ARC_LEAST_RANGE`] keep the series, and the law as taken is within the plan's bound at ±100
    /// and ±1,000 yr of a direct integration in the table's own Ψ, which leaves the table's error
    /// out, by a ten-thousandth of it, as it is from a turning point at Δ 0.03 to 0.3 at 1 and
    /// 5 ly, where [`ARC_FLOOR`]'s zone is widest; and a second out, an orbit on the arc is on its
    /// Kepler orbit's course.
    #[test]
    fn the_arc_meets_the_plan_s_bound_near_a_circle() {
        let motion = milky_way();
        let potential = Arc::new(CentrePotential::new(&motion));
        let pull = table_pull(&potential);
        let circular = |r: f64| {
            (GravitationalParameter::from_solar_masses(motion.profile().total_enclosed_mass(r))
                .value()
                / (r * METRES_PER_LIGHT_YEAR))
                .sqrt()
        };
        let orbit = |r: f64, velocity: [f64; 3]| {
            KeplerOrbit::from_state(
                PointLy::new(r, 0.0, 0.0),
                GalacticVelocity::new(velocity),
                GravitationalParameter::new(mu_at(&motion, r)),
            )
            .expect("a valid state")
        };
        let (mut worst, mut worst_series, mut slip, mut arcs) = (0.0_f64, 0.0_f64, 0.0_f64, 0_u32);
        for r in [0.1, 1.0, 5.0] {
            for share in [
                2e-5, 5e-5, 1e-4, 3e-4, 1e-3, 3e-3, 1e-2, 3e-2, 0.1, 0.2, 0.3,
            ] {
                let orbit = orbit(r, [share * circular(r), circular(r), 0.0]);
                let exact = SphericalOrbit::by_arc(&potential, &orbit).expect("bound");
                assert_eq!(
                    exact.arc.is_some(),
                    exact.log_range >= ARC_LEAST_RANGE,
                    "{r} ly at {share}: Δ {}",
                    exact.log_range
                );
                arcs += u32::from(exact.arc.is_some());
                if exact.arc.is_some() {
                    let step = orbit.propagate(Seconds::new(1.0));
                    let (near, speed) = exact.state_si(1.0);
                    slip = slip.max(distance(near, step.position) / orbit.radius);
                    slip = slip.max(distance(speed, step.velocity) / length(orbit.velocity));
                }
                let series = SphericalOrbit::new(&potential, &orbit).expect("bound");
                for (by_arc, by_series) in
                    near_circle_errors(&motion, &pull, &orbit, &exact, &series)
                {
                    worst = worst.max(by_arc);
                    worst_series = worst_series.max(by_series);
                }
            }
        }
        println!(
            "near a circle: the arc's worst {worst:.2e} tenths, the series' {worst_series:.2e}; a \
             second out, {slip:.2e} relative off the Kepler orbit's"
        );
        // Epochs at a turning point, where `ARC_FLOOR`'s zone is, at Δ from 0.03 up, on the arc.
        for r in [1.0, 5.0] {
            for excess in [0.026, 0.06, 0.15] {
                let orbit = orbit(r, [0.0, circular(r) * (1.0 + excess), 0.0]);
                let exact = SphericalOrbit::by_arc(&potential, &orbit).expect("bound");
                assert!(
                    exact.arc.is_some(),
                    "{r} ly ×{excess}: Δ {}",
                    exact.log_range
                );
                arcs += 1;
                for (by_arc, _) in near_circle_errors(&motion, &pull, &orbit, &exact, &exact) {
                    worst = worst.max(by_arc);
                }
            }
        }
        println!("with the epochs at a turning point too, the arc's worst {worst:.2e} tenths");
        assert!(arcs >= 15, "{arcs} on the arc");
        assert!(worst < 1e-4 && slip < 1e-12, "{worst} {slip}");
    }

    /// [`the_arc_meets_the_plan_s_bound_near_a_circle`]'s errors of `exact` and `series`, both
    /// `orbit`'s, at ±100 and ±1,000 yr against a direct integration in `pull`, in tenths of its
    /// tidal radius at pericentre for a solar mass.
    fn near_circle_errors(
        motion: &CentreMotion,
        pull: &dyn Fn(f64) -> f64,
        orbit: &KeplerOrbit,
        exact: &SphericalOrbit,
        series: &SphericalOrbit,
    ) -> Vec<(f64, f64)> {
        let bound = 0.1
            * motion
                .tidal_radius_at_pericentre(orbit, SolarMasses::new(1.0))
                .value();
        [-1_000.0, -100.0, 100.0, 1_000.0]
            .into_iter()
            .map(|years| {
                let span = years * YEAR;
                let (truth, swept) = integrate(pull, orbit.position, orbit.velocity, span, 1e-3);
                let reference = turned(
                    truth,
                    (orbit.position, orbit.velocity),
                    swept * exact.advance / exact.apsidal_angle_rad(),
                );
                let errors = (
                    distance(exact.state_si(span).0, reference) / bound,
                    distance(series.state_si(span).0, reference) / bound,
                );
                println!(
                    "r {:.1} ly, Δ {:.2e}, {years} yr: arc {:.2e}, series {:.2e} tenths",
                    orbit.radius / METRES_PER_LIGHT_YEAR,
                    exact.log_range,
                    errors.0,
                    errors.1
                );
                errors
            })
            .collect()
    }

    /// What follows for the bounds, re-checked: over states from 0.01 to 7 ly at speeds to the
    /// escape speed, every exact orbit's radial range lies inside its Kepler orbit's, and at every
    /// time sampled its speed squared is at most the Kepler orbit's at the same radius, to the
    /// dark halo's share of the potential's term `2μ ÷ r` (and the table's own pull's), which is
    /// printed.
    #[test]
    fn the_exact_orbit_keeps_inside_its_kepler_orbit_s_bounds() {
        let motion = milky_way();
        let potential = CentrePotential::new(&motion);
        let mut lcg = Lcg::new(0x0928_d0b0_0000_0004);
        let (mut inward, mut outward, mut faster, mut orbits) = (0.0_f64, 0.0_f64, 0.0_f64, 0_u32);
        for _ in 0..300 {
            let r = 0.01 * math::exp10(2.85 * lcg.next_f64());
            let factor = 1.45 * lcg.next_f64().sqrt();
            let tilt = PI * (lcg.next_f64() - 0.5);
            let orbit = orbit_at(&motion, r, factor.max(0.02), tilt);
            let Some(exact) = SphericalOrbit::new(&potential, &orbit) else {
                continue;
            };
            orbits += 1;
            let q = orbit.pericentre_m();
            inward = inward.max(1.0 - exact.turning[0] / q);
            if orbit.alpha > 0.0 {
                let apocentre = 2.0 / orbit.alpha - q;
                outward = outward.max(exact.turning[1] / apocentre - 1.0);
            }
            let energy = orbit.specific_energy_j_per_kg();
            for k in 0..24 {
                let (x, v) = exact.state_si((f64::from(k) - 11.5) * 90.0 * YEAR);
                let kepler_sq = 2.0 * energy + 2.0 * orbit.mu / length(x);
                let excess = (dot(v, v) - kepler_sq) / (2.0 * orbit.mu / length(x));
                if excess > faster {
                    faster = excess;
                    print!(
                        "  faster: r0 {r:.4} ly ×{factor:.3} at {tilt:.3}, k {k}: at {:.4e} ly \
                         [{:.4e}, {:.4e}] (Kepler q {:.4e}), v² {:.4e} against {kepler_sq:.4e}",
                        length(x) / METRES_PER_LIGHT_YEAR,
                        exact.turning[0] / METRES_PER_LIGHT_YEAR,
                        exact.turning[1] / METRES_PER_LIGHT_YEAR,
                        q / METRES_PER_LIGHT_YEAR,
                        dot(v, v)
                    );
                    println!();
                }
            }
        }
        println!(
            "{orbits} orbits: pericentre inside the Kepler one's by at most {inward:.2e} of it, \
             apocentre beyond by {outward:.2e}, speed squared over the Kepler one's by {faster:.2e} \
             of 2μ ÷ r"
        );
        assert!(orbits > 200, "{orbits}");
        assert!(outward <= 1e-12, "{outward}");
        // The speed is the series' (`r Δ sin η ÷ (dt ÷ dη)` along the radius), whose error near the
        // pericentre of an orbit bound far out is some 10⁻³ of the potential's term (a finding).
        assert!(inward < 1e-4 && faster < 3e-3, "{inward} {faster}");
    }

    /// A state unbound in Ψ has no exact orbit and keeps the two-body law.
    #[test]
    fn an_orbit_unbound_in_the_potential_keeps_the_two_body_law() {
        let motion = milky_way();
        let potential = CentrePotential::new(&motion);
        let r = 0.5;
        let escape = (2.0 * potential.psi_j_per_kg(LightYears::new(r))).sqrt();
        let circular = (mu_at(&motion, r) / (r * METRES_PER_LIGHT_YEAR)).sqrt();
        let open = orbit_at(&motion, r, 1.01 * escape / circular, 0.3);
        assert_eq!(SphericalOrbit::new(&potential, &open), None);
        let start = EpochState::new(member_at(open.position()), open.velocity()).expect("finite");
        let trajectory = Trajectory::in_potential(&motion, &potential, start);
        assert!(
            matches!(trajectory.law, Law::Kepler(_)),
            "{:?}",
            trajectory.law
        );
        assert_eq!(trajectory, Trajectory::of(&motion, start));
        let closed = orbit_at(&motion, r, 0.9 * escape / circular, 0.3);
        assert!(SphericalOrbit::new(&potential, &closed).is_some());
    }
}
