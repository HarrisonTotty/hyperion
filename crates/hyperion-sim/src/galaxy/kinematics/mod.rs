//! Velocities: every population's closed-form kinematics in the tabulated potential, reduced once
//! per galaxy to small tables, and the velocity draw (plan 08, P08.T2–T5).
//!
//! The brainstorm's velocity table ("Orbits and time") gives each population its law:
//!
//! - the **discs** ([`discs`]): the vertical Jeans equation on each component's own profile, the
//!   radial dispersion from it by a ratio, the azimuthal by `κ² ÷ 4Ω²`, the mean lagging by the
//!   asymmetric drift; the young disc with a floor of
//!   [`YOUNG_DISC_SIGMA_FLOOR`] and streaming along its arms;
//! - the **halo** ([`halo`]): a constant-anisotropy spherical Jeans integral per component;
//! - the **bulge, bar and nuclear disc** ([`spheroid`]): one axisymmetric Jeans solver, and for
//!   bulge and bar the pattern's rotation plus streaming along the density's ellipses.
//!
//! [`KinematicTables`] holds them for one galaxy, built from its full potential tables
//! ([`Galaxy::with_full_potential`](crate::galaxy::Galaxy::with_full_potential)), and
//! [`KinematicTables::ellipsoid`] gives the mean and dispersions of any component at a point.
//! [`draw_velocity`] draws a system's velocity from it: Gaussian in the ellipsoid's axes, on the
//! `system.velocity` stream keyed by the system's ID, and cut below the local escape speed
//! (Design notes 6 and 7).
//!
//! Speeds are km/s and lengths light-years inside the module; [`draw_velocity`] returns plan 01's
//! [`GalacticVelocity`](crate::coords::GalacticVelocity) in metres per second.

pub mod discs;
mod draw;
pub mod halo;
pub mod spheroid;

pub use draw::{ESCAPE_CUT_ATTEMPTS, VelocityDraw, draw, draw_velocity};

use self::discs::{ArmStreaming, DiscKinematics, DiscLaw, RadialRatio};
use self::halo::HaloKinematics;
use self::spheroid::{
    BAR_SATOH_K, BULGE_BETA_Z, BULGE_SATOH_K, JeansTable, NUCLEAR_BETA_Z, NUCLEAR_SATOH_K,
    bar_streaming, bulge_tracer_axes, projected_sigma_from_table,
};
use crate::galaxy::consts::LIGHT_YEARS_PER_KILOPARSEC;
use crate::galaxy::fields::disc::ExponentialDisc;
use crate::galaxy::fields::{Component, ComponentId, Fields, Shape};
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::potential::PotentialTables;
use crate::galaxy::{PointLy, Population};
use crate::math;
use crate::rng::Seed;
use crate::tables::gauss_legendre::{GL4_NODES, GL4_WEIGHTS, GL8_NODES, GL8_WEIGHTS};
use crate::tables::mge::MGE_BAR;
use crate::units::KilometresPerSecond;

/// The floor on each of the young disc's dispersions, from the turbulence of the gas it formed
/// from (brainstorm, "Orbits and time": "a floor of 5 km/s"; plan 02, ruling 3 of 2026-09-22,
/// which keeps it and lets the young disc's height follow).
pub const YOUNG_DISC_SIGMA_FLOOR: KilometresPerSecond = KilometresPerSecond::new(5.0);

/// `∫ₐᵇ f` by the 8-point Gauss–Legendre rule.
pub(crate) fn gl8(mut f: impl FnMut(f64) -> f64, a: f64, b: f64) -> f64 {
    let half = 0.5 * (b - a);
    let mid = a + half;
    let mut sum = 0.0;
    for (&x, &w) in GL8_NODES.iter().zip(&GL8_WEIGHTS) {
        sum += w * f(mid + half * x);
    }
    sum * half
}

/// `∫ₐᵇ f` by the 4-point Gauss–Legendre rule.
pub(crate) fn gl4(mut f: impl FnMut(f64) -> f64, a: f64, b: f64) -> f64 {
    let half = 0.5 * (b - a);
    let mid = a + half;
    let mut sum = 0.0;
    for (&x, &w) in GL4_NODES.iter().zip(&GL4_WEIGHTS) {
        sum += w * f(mid + half * x);
    }
    sum * half
}

/// The axes an ellipsoid's components are given in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum EllipsoidAxes {
    /// Local cylindrical axes: R rimward (coreward negative), φ spinward, z north.
    Cylindrical,
    /// Local spherical axes: r outward, θ from +z (so +θ points south in the upper half), φ
    /// spinward.
    Spherical,
}

/// A component's velocity distribution at a point: its mean and its three dispersions along the
/// ellipsoid's axes, which are local cylindrical axes or, for the halo, spherical ones.
///
/// Velocities are Gaussian in these axes about the mean (plan 08, Design note 6).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VelocityEllipsoid {
    axes: EllipsoidAxes,
    mean: [KilometresPerSecond; 3],
    sigma: [KilometresPerSecond; 3],
}

impl VelocityEllipsoid {
    /// The ellipsoid with `mean` and `sigma` along `axes`.
    #[must_use]
    pub fn new(
        axes: EllipsoidAxes,
        mean: [KilometresPerSecond; 3],
        sigma: [KilometresPerSecond; 3],
    ) -> Self {
        Self { axes, mean, sigma }
    }

    /// The axes of [`mean`](Self::mean) and [`sigma`](Self::sigma).
    #[must_use]
    pub fn axes(&self) -> EllipsoidAxes {
        self.axes
    }

    /// The mean velocity along the three axes.
    #[must_use]
    pub fn mean(&self) -> [KilometresPerSecond; 3] {
        self.mean
    }

    /// The dispersions along the three axes.
    #[must_use]
    pub fn sigma(&self) -> [KilometresPerSecond; 3] {
        self.sigma
    }

    fn from_raw(axes: EllipsoidAxes, mean: [f64; 3], sigma: [f64; 3]) -> Self {
        Self {
            axes,
            mean: mean.map(KilometresPerSecond::new),
            sigma: sigma.map(KilometresPerSecond::new),
        }
    }
}

/// Which table a component's law is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Law {
    Disc(usize),
    Bulge,
    Bar,
    Nuclear,
    Halo(usize),
}

/// A spheroid's streaming: the density's axes along and across the bar, ly.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Streaming {
    axes: (f64, f64),
}

/// Every population's velocity law for one galaxy, tabulated once (plan 08, Design note 2).
///
/// Built from the parameters, the density fields and the full potential tables; about 2 MB in
/// all, and a pure function of them and the seed (which keys the lesser halo progenitors' draws).
///
/// # Examples
///
/// ```no_run
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::kinematics::EllipsoidAxes;
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::galaxy::{Galaxy, PointLy};
///
/// let galaxy = Galaxy::from_params(Seed::new(1), GalaxyParams::milky_way_like())?
///     .with_full_potential();
/// let tables = galaxy.kinematics().expect("built with the full potential");
/// // The old thin disc's oldest sub-disc near the Sun rotates a little behind the curve.
/// let oldest = galaxy.fields().component_id(5).expect("the fifth sub-disc");
/// let e = tables.ellipsoid(oldest, &PointLy::new(22_500.0, 13_000.0, 50.0));
/// assert_eq!(e.axes(), EllipsoidAxes::Cylindrical);
/// assert!(e.mean()[1].value() > 150.0 && e.sigma()[0].value() > e.sigma()[2].value());
/// # Ok::<(), hyperion_sim::galaxy::BuildGalaxyError>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct KinematicTables {
    laws: Vec<Law>,
    discs: Vec<DiscKinematics>,
    halo: HaloKinematics,
    bulge: JeansTable,
    bar: JeansTable,
    nuclear: JeansTable,
    bulge_streaming: Streaming,
    bar_streaming: Streaming,
    bulge_axes: (f64, f64),
    /// `Ω_p`, km/s per ly.
    pattern_speed: f64,
}

impl KinematicTables {
    /// The laws of the galaxy of `params`, `fields` and the full potential tables `potential`,
    /// with the lesser halo progenitors' draws keyed by `seed`.
    ///
    /// # Panics
    ///
    /// If `potential` holds no (R, z) grid ([`PotentialTables::full`]), or `fields` is not the
    /// fields of `params` (a component the parameters do not describe).
    #[must_use]
    pub fn new(
        seed: Seed,
        params: &GalaxyParams,
        fields: &Fields,
        potential: &PotentialTables,
    ) -> Self {
        assert!(
            potential.has_grid(),
            "the kinematics need the potential off the plane"
        );
        let halo = HaloKinematics::new(seed, params, potential);
        let mut build = Build {
            params,
            fields,
            potential,
            halo: &halo,
            discs: Vec::new(),
            nuclear: None,
        };
        let laws = fields
            .components()
            .iter()
            .map(|component| build.law(component))
            .collect();
        let Build { discs, nuclear, .. } = build;
        let bulge_axes = bulge_tracer_axes(params);
        let (a_r, a_z) = bulge_axes;
        let bulge = JeansTable::new(
            |r, z| -math::hypot(r / a_r, z / a_z),
            BULGE_BETA_Z,
            BULGE_SATOH_K,
            potential,
        );
        let bar_params = params.bar();
        let (half_length, bar_height) = (
            bar_params.half_length().value(),
            bar_params.height().value(),
        );
        let bar = JeansTable::new(
            |r, z| ln_bar_surface_density(r / half_length) - z / bar_height,
            BULGE_BETA_Z,
            BAR_SATOH_K,
            potential,
        );
        let corotation = potential.bar_corotation();
        let pattern_speed = potential.v_circ(corotation).value() / corotation.value();
        let bulge_params = params.bulge();
        Self {
            laws,
            discs,
            halo,
            bulge,
            bar,
            nuclear: nuclear.expect("every galaxy has a nuclear disc"),
            bulge_streaming: Streaming {
                axes: (
                    bulge_params.scale_x().value(),
                    bulge_params.scale_y().value(),
                ),
            },
            bar_streaming: Streaming {
                axes: (half_length, bar_params.width().value()),
            },
            bulge_axes,
            pattern_speed,
        }
    }

    /// The mean and dispersions of `component` at `p` (plan 08, P08.T5): cylindrical axes for the
    /// discs and spheroids, spherical for the halo.
    ///
    /// # Panics
    ///
    /// If `component` is not one of this galaxy's.
    #[must_use]
    pub fn ellipsoid(&self, component: ComponentId, p: &PointLy) -> VelocityEllipsoid {
        let r_cyl = math::hypot(p.x, p.y);
        match self.laws[component.index()] {
            Law::Disc(i) => {
                let (mean, sigma) = self.discs[i].at(p);
                VelocityEllipsoid::from_raw(EllipsoidAxes::Cylindrical, mean, sigma)
            }
            Law::Nuclear => {
                let [sr, sp, sz, mean] = self.nuclear.moments(r_cyl, p.z);
                VelocityEllipsoid::from_raw(
                    EllipsoidAxes::Cylindrical,
                    [0.0, mean, 0.0],
                    [sr.sqrt(), sp.sqrt(), sz.sqrt()],
                )
            }
            Law::Bulge => self.spheroid(&self.bulge, self.bulge_streaming, p, r_cyl),
            Law::Bar => self.spheroid(&self.bar, self.bar_streaming, p, r_cyl),
            Law::Halo(i) => {
                let r = math::hypot(r_cyl, p.z);
                let sin_theta = if r > 0.0 { r_cyl / r } else { 1.0 };
                let (mean, sigma) = self.halo.at(i, r, sin_theta);
                VelocityEllipsoid::from_raw(EllipsoidAxes::Spherical, mean, sigma)
            }
        }
    }

    /// A bulge or bar's ellipsoid: the table's dispersions, and the pattern's rotation plus the
    /// streaming along the density's ellipses as its mean (Design note 11).
    fn spheroid(
        &self,
        table: &JeansTable,
        streaming: Streaming,
        p: &PointLy,
        r_cyl: f64,
    ) -> VelocityEllipsoid {
        let [sr, sp, sz, _] = table.moments(r_cyl, p.z);
        let sigma = [sr.sqrt(), sp.sqrt(), sz.sqrt()];
        if r_cyl <= 0.0 {
            return VelocityEllipsoid::from_raw(EllipsoidAxes::Cylindrical, [0.0; 3], sigma);
        }
        let [ux, uy] = bar_streaming(table, streaming.axes, self.pattern_speed, p.x, p.y);
        let vx = -self.pattern_speed * p.y + ux.value();
        let vy = self.pattern_speed * p.x + uy.value();
        let (cos, sin) = (p.x / r_cyl, p.y / r_cyl);
        VelocityEllipsoid::from_raw(
            EllipsoidAxes::Cylindrical,
            [vx * cos + vy * sin, -vx * sin + vy * cos, 0.0],
            sigma,
        )
    }

    /// The disc laws, in component order: the young disc, the sub-discs, the thick disc.
    #[must_use]
    pub fn discs(&self) -> &[DiscKinematics] {
        &self.discs
    }

    /// The disc law of `component`, if it is a disc of the young, old thin or thick disc.
    #[must_use]
    pub fn disc(&self, component: ComponentId) -> Option<&DiscKinematics> {
        match self.laws[component.index()] {
            Law::Disc(i) => Some(&self.discs[i]),
            Law::Bulge | Law::Bar | Law::Nuclear | Law::Halo(_) => None,
        }
    }

    /// The halo's laws.
    #[must_use]
    pub fn halo(&self) -> &HaloKinematics {
        &self.halo
    }

    /// The bulge's Jeans table.
    #[must_use]
    pub fn bulge(&self) -> &JeansTable {
        &self.bulge
    }

    /// The long bar's Jeans table.
    #[must_use]
    pub fn bar(&self) -> &JeansTable {
        &self.bar
    }

    /// The nuclear disc's Jeans table.
    #[must_use]
    pub fn nuclear_disc(&self) -> &JeansTable {
        &self.nuclear
    }

    /// The bar's pattern speed `Ω_p`: the circular frequency at its corotation radius, in km/s
    /// per light-year.
    #[must_use]
    pub fn pattern_speed_km_s_per_ly(&self) -> f64 {
        self.pattern_speed
    }

    /// The bulge's line-of-sight dispersion seen face-on, mass-weighted inside its effective
    /// radius, from the final table: the quantity
    /// [`spheroid::bulge_projected_sigma`] takes from the mass model before the tables exist, for
    /// the tests that compare the two.
    #[must_use]
    pub fn bulge_projected_sigma(&self) -> KilometresPerSecond {
        KilometresPerSecond::new(projected_sigma_from_table(&self.bulge, self.bulge_axes))
    }

    /// The bytes the tables own on the heap.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        self.laws.capacity() * size_of::<Law>()
            + self.discs.capacity() * size_of::<DiscKinematics>()
            + self
                .discs
                .iter()
                .map(DiscKinematics::heap_bytes)
                .sum::<usize>()
            + self.halo.heap_bytes()
            + self.bulge.heap_bytes()
            + self.bar.heap_bytes()
            + self.nuclear.heap_bytes()
    }
}

/// What [`KinematicTables::new`] builds the components' laws from, and the tables built so far.
struct Build<'a> {
    params: &'a GalaxyParams,
    fields: &'a Fields,
    potential: &'a PotentialTables,
    halo: &'a HaloKinematics,
    discs: Vec<DiscKinematics>,
    nuclear: Option<JeansTable>,
}

impl Build<'_> {
    /// The law of `component`, building its table if it has one of its own.
    fn law(&mut self, component: &Component) -> Law {
        let potential = self.potential;
        match (component.population(), component.shape()) {
            (Population::YoungThinDisc, Shape::Disc(disc)) => {
                let streaming = ArmStreaming::new(
                    *self.fields.arms(),
                    self.params.arms().young_fraction(),
                    potential.bar_corotation(),
                );
                self.disc(
                    disc,
                    DiscLaw {
                        ratio: RadialRatio::Constant(discs::YOUNG_DISC_RATIO),
                        floor: Some(YOUNG_DISC_SIGMA_FLOOR),
                        streaming: Some(streaming),
                    },
                )
            }
            (Population::OldThinDisc, Shape::Disc(disc)) => {
                let bin = component
                    .sub_disc()
                    .expect("every old thin disc component is a sub-disc");
                let profile = disc.profile();
                let ratio = RadialRatio::Sharma {
                    age: self.fields.sub_disc_heights().mean_ages()[bin.index()],
                    vertical_gradient: profile.gradient_per_kpc() / LIGHT_YEARS_PER_KILOPARSEC,
                    reach: profile.gradient_reach().value(),
                };
                self.disc(
                    disc,
                    DiscLaw {
                        ratio,
                        floor: None,
                        streaming: None,
                    },
                )
            }
            (Population::ThickDisc, Shape::Disc(disc)) => self.disc(
                disc,
                DiscLaw {
                    ratio: RadialRatio::Constant(discs::THICK_DISC_RATIO),
                    floor: None,
                    streaming: None,
                },
            ),
            (Population::Bulge, _) => Law::Bulge,
            (Population::LongBar, _) => Law::Bar,
            (Population::NuclearDisc, Shape::Disc(disc)) => {
                let (length, profile) = (disc.length().value(), disc.profile());
                self.nuclear = Some(JeansTable::new(
                    |r, z| -r / length - profile.exponent(z),
                    NUCLEAR_BETA_Z,
                    NUCLEAR_SATOH_K,
                    potential,
                ));
                Law::Nuclear
            }
            (Population::Halo, _) => {
                let kind = component
                    .halo_component()
                    .expect("every halo component carries its kind");
                let index = self
                    .halo
                    .components()
                    .iter()
                    .position(|c| c.kind() == kind)
                    .expect("the halo's laws follow its parameters");
                Law::Halo(index)
            }
            (population, _) => {
                panic!("{population:?} has a shape its velocity law cannot read")
            }
        }
    }

    fn disc(&mut self, disc: &ExponentialDisc, law: DiscLaw) -> Law {
        self.discs
            .push(DiscKinematics::new(disc, law, self.potential));
        Law::Disc(self.discs.len() - 1)
    }
}

/// `ln` of the bar's azimuthally averaged surface density at `u` half-lengths, 0 at the centre:
/// plan 02's expansion [`MGE_BAR`], the axisymmetric bar the potential holds.
fn ln_bar_surface_density(u: f64) -> f64 {
    let terms = MGE_BAR
        .iter()
        .filter(|&&(w, _)| w > 0.0)
        .map(|&(w, s)| math::ln(w) - 0.5 * (u / s) * (u / s));
    let peak = terms.clone().fold(f64::NEG_INFINITY, f64::max);
    peak + math::ln(terms.fold(0.0, |sum, t| sum + math::exp(t - peak)))
}
