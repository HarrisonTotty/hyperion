//! A system's sphere of influence near the black hole: in the Kepler regime its tidal radius is
//! taken at its pericentre, and is constant in time (plan 09, P09.T28.c).
//!
//! "A system on a Kepler orbit about the black hole takes its radius at pericentre, which is
//! constant in time and is also the radius to which its planetary system has been stripped: 2.7
//! au for a semi-major axis of 0.01 ly" (brainstorm, "Coordinates"). Elsewhere a system's sphere
//! of influence is plan 02's tidal (Jacobi) radius `(G m ÷ (4Ω² − κ²))^⅓` at its epoch position
//! ([`PotentialTables::tidal_radius`]), which assumes a circular orbit there. An orbit about the
//! black hole is far from circular, and is tidally stripped where the tide is strongest, at its
//! pericentre, so the radius there is the one its planets keep.
//!
//! # The radius at pericentre
//!
//! The tidal radius of a satellite of mass m at the pericentre `r_p` of its orbit in a spherical
//! potential Φ, where it turns about the centre at the angular speed `ω_p = h ÷ r_p²`, is
//!
//! `r_t = (G m ÷ (ω_p² − Φ″(r_p)))^⅓`
//!
//! (King 1962, AJ 67, 471, whose point-mass case is `r_t = r_p (m ÷ ((3 + e) M))^⅓`; in this form
//! for any spherical potential, with ω the orbit's instantaneous angular speed, as Webb et al.
//! 2013, ApJ 764, 124, §1, write it): the satellite's own pull against the tide and the
//! centrifugal force of its turning at pericentre.
//! For a circular orbit ω² is Ω² and `ω² − Φ″` is `4Ω² − κ²`, the brainstorm's formula, so the
//! two radii agree where the orbit is circular; about a point mass M it is King's
//! `r_p (m ÷ ((3 + e) M))^⅓`, which takes the brainstorm's `R (m ÷ 3M)^⅓` at e = 0. Here Φ″ is the
//! centre's spherical potential's ([`CentreProfile::psi_curvature`]: the black hole, the nuclear
//! cluster and the rest of the galaxy's monopole, `2 G M(<r) ÷ r³ − 4πG ρ(r)` with the sign of
//! `−Φ`), which carries King's `d ln M ÷ d ln r` term for the stars about the black hole, and
//! `ω_p` is the orbit's own, its angular momentum h over `r_p²`.
//!
//! - **At Milky Way values** (the black hole of 4.297 × 10⁶ M☉), a 1 M☉ system on a circular orbit
//!   at 0.01 ly keeps 2.70 au, the brainstorm's figure, and one at 3 ly some 0.0124 ly, within a
//!   few per cent of plan 02's radius there (the tests).
//! - **An eccentric orbit keeps less** than a circular one of the same semi-major axis: its
//!   pericentre is `a (1 − e)`, and its turning there adds `e` to King's 3. S2's orbit (a = 0.0163
//!   ly, e = 0.8846; GRAVITY Collaboration 2020) keeps 0.47 au for a solar mass.
//! - **The orbit is the system's own**, the [`KeplerOrbit`] its epoch state follows
//!   ([`CentreMotion::kepler_orbit`]), so the radius is fixed by the system and is the same at
//!   every time. A system the loss cone carves out still has one: it is not asked here whether the
//!   system exists.
//! - **Unbound orbits** take the rule too: a flyby is stripped at its one pericentre passage, and
//!   its planets keep what that passage leaves them, before or after the epoch (lean, 2026-10-29).
//!
//! [`tidal_radius`] is the rule a system's sphere of influence takes: the radius at pericentre in
//! the Kepler regime, and plan 02's radius at the epoch position otherwise, for plan 14's
//! `SystemContext` (P14.T29), which reads plan 02's alone today.
//!
//! # Determinism
//!
//! Pure: IEEE operations and [`math::cbrt`] on the orbit's state and the profile's closed forms.
//! Nothing generated reads it at this version; P14.T29 wires it in plan 14's train.
//!
//! [`PotentialTables::tidal_radius`]: crate::galaxy::potential::PotentialTables::tidal_radius
//! [`CentreProfile::psi_curvature`]: crate::galaxy::features::centre::CentreProfile::psi_curvature

use super::{CentreMotion, EpochState, KeplerOrbit, Regime, dot};
use crate::galaxy::{Galaxy, PointLy};
use crate::math;
use crate::units::consts::METRES_PER_LIGHT_YEAR;
use crate::units::{GravitationalParameter, Metres, SolarMasses};

/// Where the denominator `ω_p² − Φ″` is floored, as a fraction of `ω_p²`: the floor plan 02's
/// tidal radius takes in a harmonic core (`PotentialTables`' own, 0.05 Ω²; plan 02, Design note 17).
///
/// It is never reached by an orbit about the black hole: `Φ″ ≥ −G M(<r) ÷ r³` for any density that
/// falls outward, and `ω_p² ≥ μ ÷ r_p³` with μ the mass inside the epoch radius, beyond the
/// pericentre, so the denominator is positive; the floor keeps a root of a negative number out
/// should the two masses' rounding ever meet.
const TIDAL_FLOOR: f64 = 0.05;

/// Seconds⁻² in one (km/s)² ly⁻², the unit of [`CentreProfile::psi_curvature`].
///
/// [`CentreProfile::psi_curvature`]: crate::galaxy::features::centre::CentreProfile::psi_curvature
const PER_SECOND_SQ_PER_KM_S_SQ_PER_LY_SQ: f64 =
    1e6 / (METRES_PER_LIGHT_YEAR * METRES_PER_LIGHT_YEAR);

impl CentreMotion {
    /// The tidal radius of a system of mass `m` on `orbit` about the black hole, taken at the
    /// orbit's pericentre: `(G m ÷ (ω_p² − Φ″(r_p)))^⅓` in the centre's potential, with `ω_p` the
    /// orbit's angular speed at its pericentre (module documentation; King 1962).
    ///
    /// It is a property of the orbit alone, so it is the same at every point of it: the radius a
    /// Kepler-regime system's planets are stripped to (brainstorm, "Coordinates").
    ///
    /// # Examples
    ///
    /// A solar-mass system on a circular orbit a hundredth of a light-year from the Milky Way's
    /// black hole keeps 2.7 au:
    ///
    /// ```
    /// use hyperion_sim::coords::GalacticVelocity;
    /// use hyperion_sim::galaxy::PointLy;
    /// use hyperion_sim::galaxy::motion::{CentreMotion, KeplerOrbit};
    /// use hyperion_sim::galaxy::params::GalaxyParams;
    /// use hyperion_sim::galaxy::potential::MassModel;
    /// use hyperion_sim::units::consts::METRES_PER_LIGHT_YEAR;
    /// use hyperion_sim::units::{AstronomicalUnits, GravitationalParameter, LightYears, SolarMasses};
    ///
    /// let params = GalaxyParams::milky_way_like();
    /// let motion = CentreMotion::new(&params, &MassModel::new(&params))?;
    /// let r = 0.01;
    /// let mu = GravitationalParameter::from_solar_masses(motion.enclosed_mass(LightYears::new(r)));
    /// let speed = (mu.value() / (r * METRES_PER_LIGHT_YEAR)).sqrt();
    /// let orbit = KeplerOrbit::from_state(
    ///     PointLy::new(r, 0.0, 0.0),
    ///     GalacticVelocity::new([0.0, speed, 0.0]),
    ///     mu,
    /// )?;
    /// let radius = motion.tidal_radius_at_pericentre(&orbit, SolarMasses::new(1.0));
    /// let au = AstronomicalUnits::from(radius).value();
    /// assert!((au - 2.7).abs() < 0.05, "{au} au");
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn tidal_radius_at_pericentre(&self, orbit: &KeplerOrbit, m: SolarMasses) -> Metres {
        let q = orbit.pericentre_m();
        let h = orbit.angular_momentum_m2_per_s();
        // (h ÷ q²)², divided in two steps so that no intermediate leaves the range.
        let omega_sq = dot(h, h) / (q * q) / (q * q);
        let curvature = self.profile.psi_curvature(q / METRES_PER_LIGHT_YEAR)
            * PER_SECOND_SQ_PER_KM_S_SQ_PER_LY_SQ;
        let denominator = (omega_sq + curvature).max(TIDAL_FLOOR * omega_sq);
        Metres::new(math::cbrt(
            GravitationalParameter::from_solar_masses(m).value() / denominator,
        ))
    }
}

/// The tidal radius of a system of mass `m` starting from `start` in `galaxy`, by its regime's rule
/// (plan 09, P09.T28.c): the sphere of influence plan 14's `SystemContext` takes (P14.T29).
///
/// - **[`Regime::Kepler`]**, with an orbit: the radius at the pericentre of the Kepler orbit the
///   system follows ([`CentreMotion::tidal_radius_at_pericentre`] of
///   [`CentreMotion::kepler_orbit`]), constant in time.
/// - **Otherwise**, in the straight regime, or in the Kepler regime with no orbit (a state with no
///   angular momentum, which is every grid system of a galaxy built without its kinematic
///   tables): plan 02's radius at the epoch position, the same bits as
///   `galaxy.potential().tidal_radius(m, &PointLy::from(start.record().epoch_position()))`, which
///   is what `SystemContext` takes today.
///
/// It does not read the galaxy's rules: the orbit is the one the system follows under the Kepler
/// regime's law, which reaches the record with P09.T28.b.b's hold, and the radius is the planets'
/// whatever the drift hook draws. Nor does it ask the loss cone (module documentation). A grid
/// record's state is [`EpochState::of_grid_record`], a centre member's
/// [`EpochState::of_centre_member`]; a feature's member takes the smaller of this and its
/// feature's radius (`features::members::sphere_of_influence`). It reads the galaxy's
/// [`CentreMotion`], which is built on first read (about 64 ms; P09.T28.b.a as built).
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::{GalacticPosition, GalacticVelocity};
/// use hyperion_sim::galaxy::motion::EpochState;
/// use hyperion_sim::galaxy::motion::tidal::tidal_radius;
/// use hyperion_sim::galaxy::placement::{SystemOrigin, SystemRecord};
/// use hyperion_sim::galaxy::{Galaxy, PointLy, Population};
/// use hyperion_sim::id::{CentreMemberId, Layer, MemberSlot, SystemId};
/// use hyperion_sim::units::{SolarMasses, Years};
///
/// let galaxy = Galaxy::new(Seed::new(7));
/// let slot = MemberSlot::InCell { band: Layer::A, level: 4, cell: [0, 0, 0], index: 0 };
/// let at = |x: f64| {
///     let record = SystemRecord::from_parts(
///         SystemId::from(CentreMemberId::new(slot).expect("a valid slot")),
///         GalacticPosition::from_light_years([x, 0.0, 0.0]).expect("in range"),
///         SystemOrigin::CentreMember { attempt: 0 },
///         Population::NuclearDisc,
///         SolarMasses::new(1.0),
///         Years::new(1e9),
///     );
///     EpochState::new(record, GalacticVelocity::new([0.0, 2e5, 0.0])).expect("slower than light")
/// };
/// let m = SolarMasses::new(1.0);
/// // Far outside the influence radius the rule is plan 02's radius at the epoch position.
/// let far = at(500.0);
/// let today = galaxy.potential().tidal_radius(m, &PointLy::new(500.0, 0.0, 0.0));
/// assert_eq!(tidal_radius(&galaxy, &far, m).value().to_bits(), today.value().to_bits());
/// // Inside it, the radius at the orbit's pericentre, smaller than the radius at the epoch.
/// let near = at(0.5);
/// let here = galaxy.potential().tidal_radius(m, &PointLy::new(0.5, 0.0, 0.0));
/// assert!(tidal_radius(&galaxy, &near, m) < here);
/// ```
#[must_use]
pub fn tidal_radius(galaxy: &Galaxy, start: &EpochState, m: SolarMasses) -> Metres {
    let motion = galaxy.centre_motion();
    let position = start.record().epoch_position();
    if motion.regime_at(position) == Regime::Kepler
        && let Some(orbit) = motion.kepler_orbit(start)
    {
        return motion.tidal_radius_at_pericentre(&orbit, m);
    }
    galaxy.potential().tidal_radius(m, &PointLy::from(position))
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::coords::{GalacticPosition, GalacticVelocity};
    use crate::galaxy::Population;
    use crate::galaxy::features::centre::testing::milky_way_galaxy;
    use crate::galaxy::features::centre::{CentreProfile, LossCone, TracerShape};
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::placement::{SystemOrigin, SystemRecord};
    use crate::galaxy::potential::MassModel;
    use crate::id::{CentreMemberId, Layer, MemberSlot, SystemId};
    use crate::units::consts::{METRES_PER_AU, SECONDS_PER_JULIAN_YEAR};
    use crate::units::{LightYears, Seconds, Years};

    /// The Milky Way fixture's centre motion, built from its parameters without a galaxy.
    fn milky_way() -> CentreMotion {
        let params = GalaxyParams::milky_way_like();
        CentreMotion::new(&params, &MassModel::new(&params)).expect("the fixture's centre")
    }

    /// The circular speed at `r` ly about the mass inside it, m/s.
    fn circular_speed(motion: &CentreMotion, r: f64) -> f64 {
        let mu =
            GravitationalParameter::from_solar_masses(motion.enclosed_mass(LightYears::new(r)));
        (mu.value() / (r * METRES_PER_LIGHT_YEAR)).sqrt()
    }

    /// The orbit `motion` gives a state at `r` ly on +x moving at `velocity` (m/s).
    fn orbit_at(motion: &CentreMotion, r: f64, velocity: [f64; 3]) -> KeplerOrbit {
        let mu =
            GravitationalParameter::from_solar_masses(motion.enclosed_mass(LightYears::new(r)));
        KeplerOrbit::from_state(
            PointLy::new(r, 0.0, 0.0),
            GalacticVelocity::new(velocity),
            mu,
        )
        .expect("a valid state")
    }

    /// A centre member's epoch state at `position` (ly) moving at `velocity` (m/s): a record of
    /// any origin serves, since the rule reads only the epoch state.
    fn state(position: [f64; 3], velocity: [f64; 3]) -> EpochState {
        let slot = MemberSlot::InCell {
            band: Layer::A,
            level: 4,
            cell: [0, 0, 0],
            index: 0,
        };
        let record = SystemRecord::from_parts(
            SystemId::from(CentreMemberId::new(slot).expect("a valid slot")),
            GalacticPosition::from_light_years(position).expect("in range"),
            SystemOrigin::CentreMember { attempt: 0 },
            Population::NuclearDisc,
            SolarMasses::new(1.0),
            Years::new(1e9),
        );
        EpochState::new(record, GalacticVelocity::new(velocity)).expect("slower than light")
    }

    /// The plan's figure: a solar-mass system on a circular orbit of semi-major axis 0.01 ly about
    /// the Milky Way fixture's black hole keeps 2.7 au, the point-mass radius `a (m ÷ 3M)^⅓` with
    /// M the mass inside, to the cluster's King term.
    #[test]
    fn a_circular_orbit_a_hundredth_of_a_light_year_out_keeps_2_7_au() {
        let motion = milky_way();
        let r = 0.01;
        let orbit = orbit_at(&motion, r, [0.0, circular_speed(&motion, r), 0.0]);
        let m = SolarMasses::new(1.0);
        let au = motion.tidal_radius_at_pericentre(&orbit, m).value() / METRES_PER_AU;
        let inside = motion.enclosed_mass(LightYears::new(r)).value();
        let hill = r * METRES_PER_LIGHT_YEAR / METRES_PER_AU * math::cbrt(1.0 / (3.0 * inside));
        println!("circular at {r} ly: {au:.4} au (r (m ÷ 3M)^⅓ {hill:.4} au, M {inside:.6e} M☉)");
        assert!((au - 2.7).abs() < 0.05, "{au} au");
        // The stars inside 0.01 ly weigh some 10⁻⁵ of the black hole: King's term is that small.
        assert!((au / hill - 1.0).abs() < 1e-4, "{au} against {hill}");
    }

    /// The plan's second figure: a solar-mass system on a near-circular orbit at 3 ly keeps about a
    /// hundredth of a light-year, and agrees with plan 02's radius at its position, which assumes
    /// a circular orbit there in the potential tables' own model of the centre.
    #[test]
    fn a_near_circular_orbit_at_3_ly_keeps_about_a_hundredth_of_a_light_year() {
        let motion = milky_way();
        let galaxy = milky_way_galaxy();
        let r = 3.0;
        let m = SolarMasses::new(1.0);
        let today = galaxy
            .potential()
            .tidal_radius(m, &PointLy::new(r, 0.0, 0.0))
            .value()
            / METRES_PER_LIGHT_YEAR;
        for factor in [1.0, 0.99, 1.01] {
            let speed = factor * circular_speed(&motion, r);
            let orbit = orbit_at(&motion, r, [0.0, speed, 0.0]);
            let ly = motion.tidal_radius_at_pericentre(&orbit, m).value() / METRES_PER_LIGHT_YEAR;
            println!(
                "at {r} ly, {factor} × circular (e {:.4}): {ly:.5} ly; plan 02's {today:.5} ly",
                orbit.eccentricity()
            );
            assert!((0.008..0.015).contains(&ly), "{ly} ly");
            assert!((ly / today - 1.0).abs() < 0.1, "{ly} against {today}");
        }
    }

    /// About a black hole alone the rule is King's point-mass radius at pericentre,
    /// `r_p (m ÷ ((3 + e) M))^⅓`, for bound and unbound orbits alike.
    #[test]
    fn about_a_point_mass_the_radius_is_king_s() {
        let black_hole = SolarMasses::new(4.297e6);
        let profile = CentreProfile::from_shape(
            TracerShape::nuclear_cluster(),
            SolarMasses::ZERO,
            black_hole,
        )
        .expect("a valid profile");
        let motion = CentreMotion {
            profile,
            influence_radius: LightYears::new(10.0),
            loss_cone: LossCone::new(black_hole),
        };
        let mu = GravitationalParameter::from_solar_masses(black_hole);
        let m = SolarMasses::new(0.5);
        for (r, factor) in [(0.01, 1.0), (0.03, 0.4), (1.0, 0.9), (0.5, 1.2), (2.0, 1.8)] {
            let speed = factor * (mu.value() / (r * METRES_PER_LIGHT_YEAR)).sqrt();
            let orbit = KeplerOrbit::from_state(
                PointLy::new(r, 0.0, 0.0),
                GalacticVelocity::new([0.0, 0.6 * speed, 0.8 * speed]),
                mu,
            )
            .expect("a valid state");
            let e = orbit.eccentricity();
            let q = orbit.pericentre_m();
            let king = q * math::cbrt(m.value() / ((3.0 + e) * black_hole.value()));
            let radius = motion.tidal_radius_at_pericentre(&orbit, m).value();
            assert!(
                (radius / king - 1.0).abs() < 1e-12,
                "e {e}: {radius} m against {king} m"
            );
        }
    }

    /// The radius is the orbit's, so it is the same, to the orbit's own rounding, from any point
    /// of it: constant in time.
    #[test]
    fn the_radius_is_the_same_at_every_time() {
        let motion = milky_way();
        let m = SolarMasses::new(1.0);
        for (r, velocity) in [
            (0.03, [-2e4, 4.7e5, 3.5e4]),
            (1.0, [-5e4, 1.2e5, 3e4]),
            (5.0, [8e4, 1.6e5, -4e4]),
            (2.0, [3e5, 1e5, 0.0]),
        ] {
            let orbit = orbit_at(&motion, r, velocity);
            let at_epoch = motion.tidal_radius_at_pericentre(&orbit, m).value();
            for years in [-1_000.0, -100.0, 16.0, 1_000.0] {
                let later = orbit.propagate(Seconds::new(years * SECONDS_PER_JULIAN_YEAR));
                let then = motion.tidal_radius_at_pericentre(&later, m).value();
                assert!(
                    (then / at_epoch - 1.0).abs() < 1e-9,
                    "{r} ly at {years} yr: {then} m against {at_epoch} m"
                );
            }
        }
    }

    /// The rule by regime: outside the influence radius, and inside it with no orbit, plan 02's
    /// radius at the epoch position to the bit; inside with an orbit, the radius at its
    /// pericentre, below plan 02's at the epoch position for an eccentric orbit; and the same
    /// inputs give the same bits twice.
    #[test]
    fn the_rule_follows_the_regime() {
        let galaxy = milky_way_galaxy();
        let motion = galaxy.centre_motion();
        let edge = motion.influence_radius().value();
        let m = SolarMasses::new(0.8);
        let today = |p: [f64; 3]| {
            galaxy
                .potential()
                .tidal_radius(m, &PointLy::new(p[0], p[1], p[2]))
        };
        for p in [
            [1.5 * edge, 0.0, 0.0],
            [0.0, 30.0, -4.0],
            [26_000.0, 0.0, 20.0],
        ] {
            let start = state(p, [0.0, 1.5e5, 0.0]);
            assert_same_bits(tidal_radius(galaxy, &start, m).value(), today(p).value());
        }
        // At rest: no angular momentum, so no orbit.
        let still = [0.4, 0.2, 0.1];
        assert_same_bits(
            tidal_radius(galaxy, &state(still, [0.0; 3]), m).value(),
            today(still).value(),
        );
        for (p, v) in [
            ([0.4, 0.2, 0.1], [-1e5, 2e5, 5e4]),
            ([0.5 * edge, 0.0, 0.0], [0.0, 6e4, 2e4]),
            ([0.0, 0.0, 0.02], [3e5, 3e5, 0.0]),
        ] {
            let start = state(p, v);
            let orbit = motion.kepler_orbit(&start).expect("an orbit");
            let radius = tidal_radius(galaxy, &start, m);
            assert_same_bits(
                radius.value(),
                motion.tidal_radius_at_pericentre(&orbit, m).value(),
            );
            assert_same_bits(radius.value(), tidal_radius(galaxy, &start, m).value());
            assert!(
                radius < today(p),
                "{p:?}: {radius:?} against {:?}",
                today(p)
            );
        }
    }
}
