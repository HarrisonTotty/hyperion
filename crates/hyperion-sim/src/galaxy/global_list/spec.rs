//! What the global list knows of each stream and dwarf core before any tracer is sprayed (plan 10,
//! P10.T3): the progenitor's state at the epoch, its orbit, how long its debris has been stripping
//! and how much it lost in that time.
//!
//! # The orbit
//!
//! Every progenitor's orbit is characterised in the galaxy's mid-plane potential taken as
//! spherical, the approximation plan 09 finds a globular's peri- and apocentre in (P09.T13,
//! Design note 11): [`SphericalOrbit`] holds the turning points, the energy and angular momentum
//! they imply, and by a fixed quadrature of `dr ÷ v_r` the radial period and the angle swept in
//! one, whose ratio is the mean angular frequency Ω̄. No orbit is integrated for a specification:
//! the quadrature costs some hundreds of potential lookups, needs only the in-plane tables, and
//! so gives every figure from the galaxy's parameters alone, which plan 10's Design note 1 needs
//! of the dwarfs' cold masses (P10.T3.d). The leapfrog of [`orbit`](super::orbit) integrates the
//! specification's epoch state later (P10.T4's shell orbits, P10.T5's spray), at the step
//! [`SphericalOrbit::fixed_step`] gives it.
//!
//! **The step rule's inputs (ruling 146.1).** `τ_p = r_p ÷ v_p` takes `r_p` from
//! [`SphericalOrbit::pericentre`] and `v_p` from [`SphericalOrbit::pericentre_speed`]: `|L| ÷
//! r_p`, which at a turning point of the spherical potential equals `√(2 [E − Φ(r_p)])` with `E`
//! the epoch state's energy. Both are the spherical approximation's, as plan 09's pericentre is;
//! the integrated orbit's own pericentre differs by the flattening of the disc's potential.
//!
//! # The stripping time
//!
//! Plan 10's Design note 5: `T_s = min(π ÷ (η Ω̄), time since the last major merger)`, and for a
//! dwarf its accretion time too, with `η = C r_t ÷ r_p` the fractional spread in orbital frequency
//! between an arm's debris and the progenitor. A star released at `r_p + r_t` moving with the
//! progenitor's angular speed differs from it in energy by `ΔE = (dΦ ÷ dr + Ω² r) r_t = 2 v_c² r_t
//! ÷ r_p` in a flat rotation curve, and there `T_Ψ ∝ exp(E ÷ v_c²)`, so `ΔΩ ÷ Ω = ΔE ÷ v_c² = 2 r_t
//! ÷ r_p` and `C` = [`FREQUENCY_SPREAD`] = 2. Johnston (1998, ApJ 495, 297, §2.2.1 eqs. 4, 8 and 11,
//! §2.2.2 eq. 13) finds the debris of each passage spread over `0.6–3.1` of the energy scale `ε =
//! r_tide dΦ ÷ dR`, a median near 1.5, and Johnston, Sackett and Bullock (2001, ApJ 557, 137, eqs. 3
//! and 5) take each arm at about `2ε`; with their `r_tide = (m ÷ M)^⅓ R_p` about 1.26 of the
//! Jacobi radius `r_t` used here, that is `C` of about 1.9–2.5. Each arm then gains `η Ω̄ t` on the
//! progenitor and has wrapped half the orbit at `π ÷ (η Ω̄)`; older debris is the smooth field
//! (brainstorm, "Streams and accreted structure": "stripping is counted back to the wrap time or
//! to the last major merger, whichever is shorter"). `T_s` also stops at a cluster's age, since no
//! cluster lost stars before it formed (P10.T3, as built).

use crate::coords::{GalacticDisplacement, GalacticVelocity};
use crate::galaxy::consts::LIGHT_YEARS_PER_YEAR_PER_KM_S;
use crate::galaxy::potential::PotentialTables;
use crate::galaxy::quad::gl32;
use crate::math;
use crate::units::{
    Dex, KilometresPerSecond, LightYears, Metres, MetresPerSecond, PerYear, Radians, Seconds,
    SolarMasses, SolarMassesPerYear, Years,
};

use super::orbit::{BuildLeapfrogError, FixedStep};
use super::{DwarfCoreNumber, StreamNumber, StreamOrigin};

/// `C` in `η = C r_t ÷ r_p`, the fractional spread in orbital frequency between an arm's debris
/// and its progenitor (module documentation, "The stripping time"): 2, a flat rotation curve's
/// value for stars released at the tidal radius with the progenitor's angular speed. A parameter
/// of the generator version.
pub const FREQUENCY_SPREAD: f64 = 2.0;

/// Panels of the radial quadrature in the angle θ of `r = r̄ + ½ (r_a − r_p) sin θ`, each by the
/// 32-point Gauss–Legendre rule: 512 potential lookups an orbit.
const RADIAL_PANELS: u32 = 16;

/// A turning point's relative separation below which an orbit is taken as circular and given no
/// radial period.
const CIRCULAR: f64 = 1e-9;

/// A progenitor's orbit in the galaxy's mid-plane potential taken as spherical (module
/// documentation, "The orbit").
///
/// # Examples
///
/// The step the leapfrog integrates a stream's progenitor at, over its stripping time backwards:
///
/// ```no_run
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::global_list::Debris;
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::units::Seconds;
///
/// let galaxy = Galaxy::from_params(Seed::new(7), GalaxyParams::milky_way_like())?
///     .with_full_potential();
/// let debris = Debris::generate(&galaxy);
/// let stream = &debris.streams()[0];
/// let step = stream
///     .orbit()
///     .fixed_step(-Seconds::from(stream.stripping_time()))?;
/// assert!(step.count() > 0);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SphericalOrbit {
    /// Turning points, ly.
    pericentre: f64,
    apocentre: f64,
    /// `E`, (km/s)², zero at infinity.
    energy: f64,
    /// `|L|`, ly km/s.
    angular_momentum: f64,
    /// The radial period, years.
    radial_period: f64,
    /// The angle swept about the centre in one radial period, radians.
    azimuthal_advance: f64,
}

impl SphericalOrbit {
    /// The orbit between turning points `pericentre` and `apocentre` in `tables`' mid-plane
    /// potential, or `None` if they do not bound one: not finite and positive, the apocentre not
    /// above the pericentre by a relative 10⁻⁹ (a circular orbit, which has no radial period), or
    /// no positive angular momentum between them.
    #[must_use]
    pub fn between(
        tables: &PotentialTables,
        pericentre: LightYears,
        apocentre: LightYears,
    ) -> Option<Self> {
        Self::from_turning_points(
            |r| tables.potential_in_plane(LightYears::new(r)),
            pericentre.value(),
            apocentre.value(),
        )
    }

    /// [`between`](Self::between) in a spherical potential `phi` ((km/s)² at radius r, ly, zero
    /// at infinity).
    #[must_use]
    pub(crate) fn from_turning_points(
        phi: impl Fn(f64) -> f64,
        pericentre: f64,
        apocentre: f64,
    ) -> Option<Self> {
        if !(pericentre.is_finite()
            && apocentre.is_finite()
            && pericentre > 0.0
            && apocentre > pericentre * (1.0 + CIRCULAR))
        {
            return None;
        }
        let (phi_p, phi_a) = (phi(pericentre), phi(apocentre));
        // At both turning points E = Φ(r) + L² ÷ (2r²).
        let l_sq = 2.0 * (phi_a - phi_p)
            / (1.0 / (pericentre * pericentre) - 1.0 / (apocentre * apocentre));
        if !(l_sq.is_finite() && l_sq > 0.0) {
            return None;
        }
        let energy = phi_p + 0.5 * l_sq / (pericentre * pericentre);
        let mut orbit = Self {
            pericentre,
            apocentre,
            energy,
            angular_momentum: l_sq.sqrt(),
            radial_period: 0.0,
            azimuthal_advance: 0.0,
        };
        let (time, angle) = orbit.radial_integrals(&phi, core::f64::consts::FRAC_PI_2);
        orbit.radial_period = 2.0 * time;
        orbit.azimuthal_advance = 2.0 * angle;
        (orbit.radial_period > 0.0 && orbit.radial_period.is_finite()).then_some(orbit)
    }

    /// The time, years, and the angle swept, radians, from pericentre out to the radius at angle
    /// `theta` of `r = r̄ + ½ (r_a − r_p) sin θ`, θ in `[−π ÷ 2, π ÷ 2]`, by the quadrature of `dr ÷
    /// v_r` and `L dr ÷ (r² v_r)`: in θ both integrands stay finite at the turning points, where
    /// `v_r` vanishes as `cos θ` does.
    #[must_use]
    pub(crate) fn radial_integrals(&self, phi: &impl Fn(f64) -> f64, theta: f64) -> (f64, f64) {
        let start = -core::f64::consts::FRAC_PI_2;
        let width = core::f64::consts::PI / f64::from(RADIAL_PANELS);
        let (mut time, mut swept) = (0.0, 0.0);
        for k in 0..RADIAL_PANELS {
            let a = start + width * f64::from(k);
            if a >= theta {
                break;
            }
            let (t, angle) = self.panel_integrals(phi, a, (a + width).min(theta));
            time += t;
            swept += angle;
        }
        (time, swept)
    }

    /// Where the orbit is `t` years after a pericentre, `t` in `[0, radial period]`: the radius
    /// (ly), the radial speed (km/s, negative on the way in) and the angle swept since the
    /// pericentre (radians). The time from pericentre is inverted by 60 bisections in θ, each
    /// integrating only the panel it falls in, whose start is summed once.
    #[must_use]
    pub(crate) fn state_after_pericentre(&self, phi: &impl Fn(f64) -> f64, t: f64) -> [f64; 3] {
        let period = self.radial_period;
        let t = t.clamp(0.0, period);
        let (outbound, t_half) = if t <= 0.5 * period {
            (true, t)
        } else {
            (false, period - t)
        };
        let start = -core::f64::consts::FRAC_PI_2;
        let width = core::f64::consts::PI / f64::from(RADIAL_PANELS);
        // The panel the time falls in, and the time and angle at its start.
        let (mut before, mut swept, mut panel) = (0.0, 0.0, 0);
        for k in 0..RADIAL_PANELS {
            let a = start + width * f64::from(k);
            let (time, angle) = self.panel_integrals(phi, a, a + width);
            if before + time >= t_half || k + 1 == RADIAL_PANELS {
                panel = k;
                break;
            }
            before += time;
            swept += angle;
        }
        let a = start + width * f64::from(panel);
        let (mut lo, mut hi) = (a, a + width);
        for _ in 0..60 {
            let mid = f64::midpoint(lo, hi);
            if before + self.panel_integrals(phi, a, mid).0 < t_half {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let theta = f64::midpoint(lo, hi);
        let angle = swept + self.panel_integrals(phi, a, theta).1;
        let r = self.radius_at(theta);
        let v_r = self.radial_speed(phi, r);
        if outbound {
            [r, v_r, angle]
        } else {
            [r, -v_r, self.azimuthal_advance - angle]
        }
    }

    /// [`radial_integrals`](Self::radial_integrals) over `[a, b]` of θ alone, by one 32-point
    /// rule: the time (years) and the angle.
    #[must_use]
    fn panel_integrals(&self, phi: &impl Fn(f64) -> f64, from: f64, to: f64) -> (f64, f64) {
        let (mid, half) = (
            f64::midpoint(self.apocentre, self.pericentre),
            0.5 * (self.apocentre - self.pericentre),
        );
        let momentum = self.angular_momentum;
        // dt ÷ dθ in ly ÷ (km/s), and the radius.
        let rate = |theta: f64| {
            let (sin, cos) = math::sin_cos(theta);
            let radius = mid + half * sin;
            let v_r_sq =
                2.0 * (self.energy - phi(radius)) - momentum * momentum / (radius * radius);
            // Rounding can leave v_r² at or below zero only within a hair of a turning point,
            // where cos θ is as small; such a node adds nothing.
            if v_r_sq <= 0.0 {
                (0.0, radius)
            } else {
                (half * cos / v_r_sq.sqrt(), radius)
            }
        };
        let time = gl32(|theta| rate(theta).0, from, to);
        let angle = gl32(
            |theta| {
                let (dt, radius) = rate(theta);
                dt * momentum / (radius * radius)
            },
            from,
            to,
        );
        (time / LIGHT_YEARS_PER_YEAR_PER_KM_S, angle)
    }

    /// The radius at angle `theta` of the quadrature's substitution, ly.
    #[must_use]
    pub(crate) fn radius_at(&self, theta: f64) -> f64 {
        f64::midpoint(self.apocentre, self.pericentre)
            + 0.5 * (self.apocentre - self.pericentre) * math::sin(theta)
    }

    /// The radial speed at radius `r` (ly), km/s, zero outside the turning points.
    #[must_use]
    pub(crate) fn radial_speed(&self, phi: &impl Fn(f64) -> f64, r: f64) -> f64 {
        let l = self.angular_momentum;
        (2.0 * (self.energy - phi(r)) - l * l / (r * r))
            .max(0.0)
            .sqrt()
    }

    /// The pericentre.
    #[must_use]
    pub fn pericentre(&self) -> LightYears {
        LightYears::new(self.pericentre)
    }

    /// The apocentre.
    #[must_use]
    pub fn apocentre(&self) -> LightYears {
        LightYears::new(self.apocentre)
    }

    /// `(r_a − r_p) ÷ (r_a + r_p)`.
    #[must_use]
    pub fn eccentricity(&self) -> f64 {
        (self.apocentre - self.pericentre) / (self.apocentre + self.pericentre)
    }

    /// The speed at pericentre, `|L| ÷ r_p`: the `v_p` of ruling 146.1's step (module
    /// documentation, "The step rule's inputs").
    #[must_use]
    pub fn pericentre_speed(&self) -> KilometresPerSecond {
        KilometresPerSecond::new(self.angular_momentum / self.pericentre)
    }

    /// The energy per unit mass, (km/s)², zero at infinity: negative for a bound orbit.
    #[must_use]
    pub fn energy_km2_s2(&self) -> f64 {
        self.energy
    }

    /// The magnitude of the angular momentum per unit mass, ly km/s.
    #[must_use]
    pub fn angular_momentum_ly_km_s(&self) -> f64 {
        self.angular_momentum
    }

    /// The radial period, pericentre to pericentre.
    #[must_use]
    pub fn radial_period(&self) -> Years {
        Years::new(self.radial_period)
    }

    /// The angle swept about the centre in one radial period, radians: 2π for a Kepler orbit, π
    /// for a harmonic one.
    #[must_use]
    pub fn azimuthal_advance(&self) -> Radians {
        Radians::new(self.azimuthal_advance)
    }

    /// The mean angular frequency Ω̄: the angle swept in a radial period over the period.
    #[must_use]
    pub fn mean_angular_frequency(&self) -> PerYear {
        PerYear::new(self.azimuthal_advance / self.radial_period)
    }

    /// Ruling 146.1's fixed step over `span` (negative to integrate backwards), from the
    /// pericentre and the speed there (module documentation, "The step rule's inputs").
    ///
    /// # Errors
    ///
    /// As [`FixedStep::new`]: a span that is not finite or needs more than `u32::MAX` steps.
    pub fn fixed_step(&self, span: Seconds) -> Result<FixedStep, BuildLeapfrogError> {
        FixedStep::new(
            Metres::from(self.pericentre()),
            MetresPerSecond::from(self.pericentre_speed()),
            span,
        )
    }
}

/// Design note 5's stripping time for an orbit whose progenitor has tidal radius `tidal_radius`
/// at its pericentre: the wrap time `π ÷ (η Ω̄)`, `η = C r_t ÷ r_p`, or the least of `limits` if
/// shorter (module documentation, "The stripping time").
#[must_use]
pub(crate) fn stripping_time(
    orbit: &SphericalOrbit,
    tidal_radius: LightYears,
    limits: impl IntoIterator<Item = Years>,
) -> Years {
    let eta = FREQUENCY_SPREAD * tidal_radius.value() / orbit.pericentre;
    let wrap = core::f64::consts::PI / (eta * orbit.mean_angular_frequency().value());
    let wrap = if wrap.is_nan() { f64::INFINITY } else { wrap };
    Years::new(
        limits
            .into_iter()
            .fold(wrap, |least, limit| least.min(limit.value())),
    )
}

/// How a stream's progenitor lost the mass in its tube over the stripping time (plan 10, P10.T5.a
/// releases its tracers by it).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MassLoss {
    /// A living globular: at a constant rate, its history's (P09.T13), over the whole stripping
    /// time.
    Steady(SolarMassesPerYear),
    /// An orphan: at a constant rate until the cluster dissolved, `ago` before the epoch, and
    /// nothing since, which leaves the gap.
    Dissolved {
        /// When the cluster dissolved, before the epoch, inside the stripping time.
        ago: Years,
    },
    /// A dwarf: a share `fraction` of its bound mass at each pericentre (Design note 11).
    Pulsed {
        /// The share of the bound mass lost at each pericentre, 0.1–0.5.
        fraction: f64,
        /// The time since the last pericentre, below one radial period.
        last_pericentre: Years,
        /// The pericentres inside the stripping time, each of which fed the tube.
        pericentres: u32,
    },
}

/// A globular cluster's stars, which a globular's or an orphan's stream shares: its [Fe/H] and its
/// age. A dwarf's stream takes its core's instead (plan 10, P10.T7).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClusterStars {
    pub(super) fe_h: Dex,
    pub(super) age: Years,
}

impl ClusterStars {
    /// The cluster's [Fe/H].
    #[must_use]
    pub const fn fe_h(&self) -> Dex {
        self.fe_h
    }

    /// The cluster's age at the epoch.
    #[must_use]
    pub const fn age(&self) -> Years {
        self.age
    }
}

/// One stream of the global list, as P10.T3 decides it (plan 10's Provides): which progenitor, its
/// state and orbit at the epoch, its stripping time and the mass of its tube.
#[derive(Debug, Clone, PartialEq)]
pub struct StreamSpec {
    pub(super) number: StreamNumber,
    pub(super) origin: StreamOrigin,
    pub(super) position: GalacticDisplacement,
    pub(super) velocity: GalacticVelocity,
    pub(super) orbit: SphericalOrbit,
    pub(super) stripping_time: Years,
    pub(super) mass: SolarMasses,
    pub(super) progenitor_mass: SolarMasses,
    pub(super) tidal_radius: LightYears,
    pub(super) mass_loss: MassLoss,
    pub(super) stars: Option<ClusterStars>,
}

impl StreamSpec {
    /// The stream's number on the global list (Design note 12).
    #[must_use]
    pub const fn number(&self) -> StreamNumber {
        self.number
    }

    /// Where its debris came from.
    #[must_use]
    pub const fn origin(&self) -> StreamOrigin {
        self.origin
    }

    /// The progenitor's position about the galactic centre at the epoch, m; for an orphan, where
    /// its cluster would be on the drawn orbit, the tube's middle.
    #[must_use]
    pub const fn position(&self) -> GalacticDisplacement {
        self.position
    }

    /// The progenitor's velocity in the galactic frame at the epoch.
    #[must_use]
    pub const fn velocity(&self) -> GalacticVelocity {
        self.velocity
    }

    /// The progenitor's orbit, in the spherical approximation.
    #[must_use]
    pub const fn orbit(&self) -> &SphericalOrbit {
        &self.orbit
    }

    /// How far back the tube's debris was stripped (Design note 5).
    #[must_use]
    pub const fn stripping_time(&self) -> Years {
        self.stripping_time
    }

    /// The tube's stellar mass: what the progenitor lost within the stripping time.
    #[must_use]
    pub const fn mass(&self) -> SolarMasses {
        self.mass
    }

    /// The progenitor's bound mass at the epoch: a living cluster's present mass, a dwarf's
    /// core's, zero for an orphan.
    #[must_use]
    pub const fn progenitor_mass(&self) -> SolarMasses {
        self.progenitor_mass
    }

    /// The progenitor's tidal radius at its pericentre, which sets η (Design note 5): for a
    /// living cluster at its present mass, for an orphan at its tube's, for a dwarf at its stellar
    /// mass at accretion (the [`dwarfs`](super::dwarfs) module's documentation says why that is
    /// short of its whole mass's).
    #[must_use]
    pub const fn tidal_radius(&self) -> LightYears {
        self.tidal_radius
    }

    /// How the progenitor lost the tube's mass.
    #[must_use]
    pub const fn mass_loss(&self) -> MassLoss {
        self.mass_loss
    }

    /// A cluster's stars, for a globular's or an orphan's stream; `None` for a dwarf's.
    #[must_use]
    pub const fn stars(&self) -> Option<ClusterStars> {
        self.stars
    }
}

/// One dwarf core of the global list (Design note 11): what is still bound of a recent
/// progenitor that lies inside the root cube at the epoch.
#[derive(Debug, Clone, PartialEq)]
pub struct DwarfCoreSpec {
    pub(super) number: DwarfCoreNumber,
    pub(super) progenitor: u32,
    pub(super) mass: SolarMasses,
    pub(super) systems: f64,
    pub(super) accreted: Years,
    pub(super) accreted_mass: SolarMasses,
    pub(super) position: GalacticDisplacement,
    pub(super) velocity: GalacticVelocity,
    pub(super) orbit: SphericalOrbit,
    pub(super) stripping_fraction: f64,
    pub(super) pericentres: u32,
}

impl DwarfCoreSpec {
    /// The core's number on the global list.
    #[must_use]
    pub const fn number(&self) -> DwarfCoreNumber {
        self.number
    }

    /// `j` of its progenitor, plan 02's
    /// [`ProgenitorKind::Recent(j)`](crate::galaxy::params::ProgenitorKind::Recent).
    #[must_use]
    pub const fn progenitor(&self) -> u32 {
        self.progenitor
    }

    /// The stellar mass still bound.
    #[must_use]
    pub const fn mass(&self) -> SolarMasses {
        self.mass
    }

    /// The expected number of systems it holds: its mass over the halo's mean system mass, at
    /// least 10³.
    #[must_use]
    pub const fn systems(&self) -> f64 {
        self.systems
    }

    /// How long before the epoch the progenitor was accreted, 0–6 Gyr.
    #[must_use]
    pub const fn accreted(&self) -> Years {
        self.accreted
    }

    /// The progenitor's stellar mass when it was accreted (plan 02).
    #[must_use]
    pub const fn accreted_mass(&self) -> SolarMasses {
        self.accreted_mass
    }

    /// Its position about the galactic centre at the epoch, m, inside the root cube.
    #[must_use]
    pub const fn position(&self) -> GalacticDisplacement {
        self.position
    }

    /// Its bulk velocity in the galactic frame at the epoch.
    #[must_use]
    pub const fn velocity(&self) -> GalacticVelocity {
        self.velocity
    }

    /// Its orbit, in the spherical approximation.
    #[must_use]
    pub const fn orbit(&self) -> &SphericalOrbit {
        &self.orbit
    }

    /// The share of its bound mass it lost at each pericentre.
    #[must_use]
    pub const fn stripping_fraction(&self) -> f64 {
        self.stripping_fraction
    }

    /// The pericentres it has passed since it was accreted.
    #[must_use]
    pub const fn pericentres(&self) -> u32 {
        self.pericentres
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// G M in (km/s)² ly for 10¹¹ M☉: G = 4.3009 × 10⁻³ pc (km/s)² ÷ M☉, times 3.2616 ly ÷ pc.
    const GM: f64 = 4.300_9e-3 * 3.261_563_777 * 1e11;

    fn kepler(r: f64) -> f64 {
        -GM / r
    }

    #[test]
    fn a_kepler_orbit_closes_on_its_period() {
        for (peri, apo) in [
            (10_000.0, 30_000.0),
            (2_000.0, 78_000.0),
            (50_000.0, 51_000.0),
        ] {
            let orbit = SphericalOrbit::from_turning_points(kepler, peri, apo).unwrap();
            let semi_major = f64::midpoint(peri, apo);
            // T = 2π √(a³ ÷ GM), in ly ÷ (km/s), then years.
            let period =
                2.0 * core::f64::consts::PI * (semi_major * semi_major * semi_major / GM).sqrt()
                    / LIGHT_YEARS_PER_YEAR_PER_KM_S;
            // The energy and angular momentum come from differences at the two turning points,
            // which lose digits as e²: 2 × 10⁻⁹ at e = 0.01.
            let e = orbit.eccentricity();
            assert!(
                (orbit.radial_period().value() / period - 1.0).abs() < 1e-8,
                "e {e}: {} against {period}",
                orbit.radial_period().value()
            );
            assert!(
                (orbit.azimuthal_advance().value() / (2.0 * core::f64::consts::PI) - 1.0).abs()
                    < 1e-8,
                "e {e}: {}",
                orbit.azimuthal_advance().value()
            );
            // Bound, and v_p = L ÷ r_p = √(2 (E − Φ(r_p))).
            assert!(orbit.energy_km2_s2() < 0.0);
            let v_p = orbit.pericentre_speed().value();
            let from_energy = (2.0 * (orbit.energy_km2_s2() - kepler(peri))).sqrt();
            assert!(
                (v_p / from_energy - 1.0).abs() < 1e-12,
                "{v_p} {from_energy}"
            );
        }
    }

    #[test]
    fn a_harmonic_orbit_sweeps_half_a_turn_per_radial_period() {
        // Φ = ½ ω² r²: the radial period is π ÷ ω, the azimuthal period 2π ÷ ω.
        let omega = 0.01; // km/s per ly
        let harmonic = |r: f64| 0.5 * omega * omega * r * r;
        let orbit = SphericalOrbit::from_turning_points(harmonic, 1_000.0, 9_000.0).unwrap();
        let period = core::f64::consts::PI / omega / LIGHT_YEARS_PER_YEAR_PER_KM_S;
        assert!((orbit.radial_period().value() / period - 1.0).abs() < 1e-9);
        assert!((orbit.azimuthal_advance().value() / core::f64::consts::PI - 1.0).abs() < 1e-9);
    }

    #[test]
    fn half_the_quadrature_is_half_the_orbit() {
        let orbit = SphericalOrbit::from_turning_points(kepler, 8_000.0, 60_000.0).unwrap();
        let (time, angle) = orbit.radial_integrals(&kepler, 0.0);
        let (full_time, full_angle) = orbit.radial_integrals(&kepler, core::f64::consts::FRAC_PI_2);
        assert!(time > 0.0 && time < full_time);
        assert!(angle > 0.0 && angle < full_angle);
        assert!((orbit.radius_at(core::f64::consts::FRAC_PI_2) - 60_000.0).abs() < 1e-9);
        assert!((orbit.radius_at(-core::f64::consts::FRAC_PI_2) - 8_000.0).abs() < 1e-9);
        // Kepler's equation at the semi-major axis's radius, E = π ÷ 2: M = E − e.
        let e = orbit.eccentricity();
        let at_a = orbit.radial_integrals(&kepler, 0.0).0;
        let mean_anomaly = core::f64::consts::FRAC_PI_2 - e;
        let want = mean_anomaly / (2.0 * core::f64::consts::PI) * orbit.radial_period().value();
        assert!((at_a / want - 1.0).abs() < 1e-9, "{at_a} {want}");
    }

    #[test]
    fn circular_and_reversed_turning_points_bound_no_orbit() {
        assert_eq!(
            SphericalOrbit::from_turning_points(kepler, 10_000.0, 10_000.0),
            None
        );
        assert_eq!(
            SphericalOrbit::from_turning_points(kepler, 10_000.0, 9_000.0),
            None
        );
        assert_eq!(
            SphericalOrbit::from_turning_points(kepler, 0.0, 9_000.0),
            None
        );
        assert_eq!(
            SphericalOrbit::from_turning_points(kepler, f64::NAN, 9_000.0),
            None
        );
    }

    #[test]
    fn the_stripping_time_is_the_wrap_time_or_the_least_limit() {
        let orbit = SphericalOrbit::from_turning_points(kepler, 20_000.0, 60_000.0).unwrap();
        let omega = orbit.mean_angular_frequency().value();
        // r_t ÷ r_p = 0.01: the wrap time is π ÷ (0.01 C Ω̄).
        let wrap = core::f64::consts::PI / (0.01 * FREQUENCY_SPREAD * omega);
        let tidal = LightYears::new(200.0);
        let none = stripping_time(&orbit, tidal, []);
        assert!((none.value() / wrap - 1.0).abs() < 1e-12);
        let capped = stripping_time(&orbit, tidal, [Years::new(1e9), Years::new(5e8)]);
        assert!((capped.value() - 5e8).abs() < 1e-3);
        assert!(
            stripping_time(&orbit, LightYears::new(0.0), [])
                .value()
                .is_infinite()
        );
    }
}
