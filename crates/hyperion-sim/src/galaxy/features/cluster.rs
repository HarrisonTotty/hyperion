//! One cluster's structure and time scales (plan 09, P09.T7), with the closed forms of its
//! remnants' retention, black holes and core collapse (P09.T9.b–d) and its recycled objects
//! (P09.T9.e).
//!
//! A [`ClusterModel`] is built once per resolved feature and cached by the caller. Everything in it
//! is a closed form of the feature's parameters ([`ClusterParameters`]) and marks
//! ([`ClusterMarks`]):
//!
//! - **Radii.** The half-mass radius is the feature's; the tidal (Jacobi) radius is plan 02's
//!   `PotentialTables::tidal_radius` at the cluster's position; the core radius is the feature's
//!   (a globular's, drawn) or, for an open cluster, `r_t ÷ 10^c` from its King concentration `c`,
//!   held below `r_h ÷ 10^0.1` so that a core never outgrows the half-mass radius (ours).
//! - **The half-mass relaxation time** `t★ = 0.138 √(M r_h³ ÷ G) ÷ (⟨m⟩ ln Λ)` with ⟨m⟩ = 0.45 M☉
//!   and ln Λ = 10 (Spitzer 1987; the prefactor is
//!   [`BH_RELAXATION_PREFACTOR`](crate::tables::cluster_dynamics::BH_RELAXATION_PREFACTOR)).
//! - **The central escape speed** `√(G M ÷ r_h) × 10^(0.1055 + 0.2550 u − 0.0769 u²)` with
//!   `u = log₁₀(r_h ÷ r_c)`, the brainstorm's fit to the Baumgardt–Hilker catalogue (Baumgardt and Hilker 2018, MNRAS 478, 1520; re-checked
//!   against it: P09.T7's test), and at birth today's times `√((M₀ ÷ M)(r_h ÷ r_h0))`.
//! - **The dispersion** of a Plummer sphere of the cluster's mass with `a = r_h ÷ 1.305`,
//!   `σ²(r) = G M ÷ (6 √(r² + a²))` (Design note 9).
//! - **The encounter rate** `Γ ∝ ρ_c^1.5 r_c²`, with `ρ_c` the central density of the cored profile
//!   of Design note 9 at `q′ = 1` holding the cluster's mass, and quoted relative to the same form
//!   at 47 Tucanae's catalogue parameters.
//!
//! # Remnants (P09.T9.b–d)
//!
//! Neutron stars, white dwarfs and black holes are retained from the kick law at the effective
//! birth escape speed ([`retention`]). Black holes then leave: their mass fraction is
//! `f(t) = [(1 + ψ₁ f₀) e^(−β ψ₁ k t ÷ t★) − 1] ÷ ψ₁`, floored at zero, with the clock factor
//! `k` = [`BH_CLOCK_FACTOR`] = 2.5 for the cluster's denser past (ruling 126.4), `f₀ = 0.06 ×` the
//! retention (Breen and Heggie 2013; Antonini and Gieles 2020), the solution at constant mass and
//! radius of `df ÷ dt = −β (1 + ψ₁ f) ÷ t★`. A cluster with no black holes left and older than 14
//! t★ is core-collapsed and takes a cusp of slope drawn on 1.6–2 (Trager, King and Djorgovski 1995, AJ 109, 218). The black
//! holes' count is their mass over [`MEAN_BLACK_HOLE_MASS`].
//!
//! # Recycled objects (P09.T9.e)
//!
//! Millisecond pulsars number `40 (Γ ÷ Γ_47Tuc)^0.7`, capped for a core-collapsed cluster at the
//! count at 47 Tucanae's Γ ([`tables::cluster_dynamics`](crate::tables::cluster_dynamics)).
//! Quiescent low-mass X-ray binaries number 5 at 47 Tucanae (Heinke et al. 2003, ApJ 598, 501) and
//! rise as `Γ^0.74` (Pooley et al. 2003, ApJ 591, L131); blue stragglers number `N = 10^1.6 ×
//! (M_core ÷ 10⁴ M☉)^0.38` (ours, the slope of Knigge, Leigh and Sills 2009, Nature 457, 288, with
//! a normalisation of our own). The last two are provisional.

use crate::galaxy::consts::{G, LIGHT_YEARS_PER_PARSEC, LIGHT_YEARS_PER_YEAR_PER_KM_S};
use crate::galaxy::fields::ComponentId;
use crate::galaxy::quad::gl_panels;
use crate::galaxy::{Galaxy, PointLy, Population};
use crate::math;
use crate::rng::{Mark, Stream};
use crate::stellar::Composition;
use crate::stellar::sse::turn_off_mass;
use crate::tables::cluster_dynamics::{
    BH_CLOCK_FACTOR, BH_LOSS_BETA, BH_LOSS_PSI_SLOPE, BH_RELAXATION_PREFACTOR,
    PULSAR_CORE_COLLAPSE_CAP, PULSAR_GAMMA_EXPONENT, PULSARS_AT_47_TUC_GAMMA,
};
use crate::units::consts::METRES_PER_LIGHT_YEAR;
use crate::units::{Dex, HeliumExcess, KilometresPerSecond, LightYears, SolarMasses, Years};

use super::catalogue::{FeatureMarks, FeatureRecord};
use super::interior::retention::{EFFECTIVE_ESCAPE_FACTOR, Retention, retention_tabulated};
use super::kinds::globular::history;
use super::kinds::open_cluster::DISRUPTION_GAMMA as DISRUPTION_GAMMA_FOR_RATE;
use crate::rng::{ObjectKey, tags};

/// The mean member mass in the relaxation time, M☉ (P09.T7).
pub const RELAXATION_MEAN_MASS: f64 = 0.45;

/// The Coulomb logarithm in the relaxation time (P09.T7).
pub const COULOMB_LOGARITHM: f64 = 10.0;

/// A Plummer sphere's half-mass radius over its scale `a`.
pub const PLUMMER_HALF_MASS_RATIO: f64 = 1.304_766_372_624_786;

/// The black holes' mass fraction at birth per unit retention, `f₀ ÷ retention` (P09.T9.c).
pub const BLACK_HOLE_BIRTH_FRACTION: f64 = 0.06;

/// The mean mass of a retained black hole, M☉, which turns their mass fraction into a count
/// (ours, provisional: a stand-in for the mass spectrum P15.T8.a fits).
pub const MEAN_BLACK_HOLE_MASS: f64 = 15.0;

/// A cluster older than this many relaxation times with no black holes is core-collapsed
/// (P09.T9.d).
pub const CORE_COLLAPSE_RELAXATION_TIMES: f64 = 14.0;

/// The range of a core-collapsed cluster's cusp slope `γ` in `ρ ∝ r^−γ` (P09.T9.d).
pub const CUSP_SLOPE_RANGE: (f64, f64) = (1.6, 2.0);

/// The least `log₁₀(r_h ÷ r_c)` an open cluster's core is held to (module documentation).
const MIN_LOG_HALF_MASS_OVER_CORE: f64 = 0.1;

/// 47 Tucanae's catalogue mass, M☉ (Baumgardt and Hilker): Γ's reference.
const TUC_47_MASS: f64 = 8.53e5;
/// 47 Tucanae's core radius, pc.
const TUC_47_CORE_PC: f64 = 0.61;
/// 47 Tucanae's tidal radius, pc.
const TUC_47_TIDAL_PC: f64 = 124.63;

/// Quiescent low-mass X-ray binaries at 47 Tucanae's Γ (Heinke et al. 2003; provisional).
pub const XRAY_BINARIES_AT_47_TUC_GAMMA: f64 = 5.0;
/// Their encounter-rate exponent (Pooley et al. 2003; provisional).
pub const XRAY_BINARY_GAMMA_EXPONENT: f64 = 0.74;
/// Blue stragglers in a cluster whose core holds 10⁴ M☉ (ours, provisional).
pub const BLUE_STRAGGLERS_AT_1E4_CORE: f64 = 39.810_717_055_349_72;
/// Their core-mass exponent (Knigge et al. 2009; provisional).
pub const BLUE_STRAGGLER_CORE_EXPONENT: f64 = 0.38;

/// Years in one light-year per km/s.
const YEARS_PER_LY_PER_KM_S: f64 = 1.0 / LIGHT_YEARS_PER_YEAR_PER_KM_S;

/// What kind of cluster a model is of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ClusterKind {
    /// A bound open cluster, young or old.
    Open,
    /// A globular cluster.
    Globular,
}

/// A cluster's own marks, drawn once on its feature's stream (P09.T9.a and T9.d).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClusterMarks {
    /// The standard normal of the dwarfs' depleted slope's per-cluster scatter.
    depletion: f64,
    /// The uniform of a core-collapsed cluster's cusp slope.
    cusp: f64,
    /// The mark that decides whether the cluster has an iron spread (P09.T9.h).
    iron_spread: Mark,
}

impl ClusterMarks {
    /// The marks at their medians: no scatter in the slope, the middle cusp, no iron spread.
    pub const MEDIAN: Self = Self {
        depletion: 0.0,
        cusp: 0.5,
        iron_spread: Mark::from_word(u64::MAX),
    };

    /// The marks drawn from `stream`: a standard normal (two words), a uniform and a mark.
    #[must_use]
    pub fn draw(stream: &mut Stream) -> Self {
        let depletion = stream.standard_normal();
        let cusp = stream.uniform();
        let iron_spread = stream.mark();
        Self {
            depletion,
            cusp,
            iron_spread,
        }
    }

    /// The standard normal of the depleted slope's scatter.
    #[must_use]
    pub const fn depletion_normal(&self) -> f64 {
        self.depletion
    }

    /// The cusp slope's uniform, in [0, 1).
    #[must_use]
    pub const fn cusp_uniform(&self) -> f64 {
        self.cusp
    }

    /// The iron-spread mark.
    #[must_use]
    pub const fn iron_spread(&self) -> Mark {
        self.iron_spread
    }
}

/// What a cluster model is built from: the feature's parameters (P09.T7).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClusterParameters {
    /// Open or globular.
    pub kind: ClusterKind,
    /// The population whose budget its members are drawn from (Design notes 3, 4 and 20).
    pub population: Population,
    /// The density component whose velocity law its bulk motion follows, if any.
    pub component: Option<ComponentId>,
    /// Its centre, ly.
    pub position: PointLy,
    /// Its age at the epoch.
    pub age: Years,
    /// Its [Fe/H].
    pub fe_h: Dex,
    /// Its present mass.
    pub mass: SolarMasses,
    /// Its initial mass.
    pub initial_mass: SolarMasses,
    /// Its present three-dimensional half-mass radius.
    pub half_mass_radius: LightYears,
    /// Its half-mass radius at birth.
    pub birth_half_mass_radius: LightYears,
    /// Its core radius, or `None` for an open cluster's, which follows from `concentration`.
    pub core_radius: Option<LightYears>,
    /// An open cluster's King concentration `log₁₀(r_t ÷ r_c)`; unused when the core is given.
    pub concentration: f64,
    /// The mass it is losing to its tails, M☉ per year.
    pub mass_loss_rate: f64,
    /// The spread of its members' ages below the cluster's: a nursery's, zero for the rest.
    pub age_spread: Years,
    /// Its own marks.
    pub marks: ClusterMarks,
}

/// One cluster's structure, time scales and remnants (module documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct ClusterModel {
    parameters: ClusterParameters,
    composition: Composition,
    tidal_radius: LightYears,
    core_radius: LightYears,
    turn_off: SolarMasses,
    relaxation_time: Years,
    escape_central: KilometresPerSecond,
    escape_birth: KilometresPerSecond,
    central_density: f64,
    retention: Retention,
    black_hole_fraction: f64,
}

impl ClusterModel {
    /// The model of the cluster `parameters` describe, in `galaxy` (module documentation).
    ///
    /// Its retention reads `tables::cluster_retention` (ruling 139.5), so it costs microseconds;
    /// the caller still caches the interior built on it.
    ///
    /// # Panics
    ///
    /// If a mass or radius is not positive and finite.
    #[must_use]
    pub fn new(galaxy: &Galaxy, parameters: &ClusterParameters) -> Self {
        let p = *parameters;
        for value in [
            p.mass.value(),
            p.initial_mass.value(),
            p.half_mass_radius.value(),
            p.birth_half_mass_radius.value(),
        ] {
            assert!(
                value > 0.0 && value.is_finite(),
                "a cluster parameter of {value}"
            );
        }
        let composition = Composition::from_fe_h(p.fe_h, HeliumExcess::ZERO);
        let tidal_radius = LightYears::new(
            galaxy.potential().tidal_radius(p.mass, &p.position).value() / METRES_PER_LIGHT_YEAR,
        );
        let r_h = p.half_mass_radius.value();
        let core_radius = LightYears::new(p.core_radius.map_or_else(
            || {
                (tidal_radius.value() / math::exp10(p.concentration))
                    .min(r_h / math::exp10(MIN_LOG_HALF_MASS_OVER_CORE))
            },
            LightYears::value,
        ));
        let turn_off = turn_off_mass(Years::new(p.age.value().max(0.0)), &composition);
        let relaxation_time = relaxation_time(p.mass, p.half_mass_radius);
        let escape_central = central_escape_speed(p.mass, p.half_mass_radius, core_radius);
        let birth_ratio =
            (p.initial_mass.value() / p.mass.value()) * (r_h / p.birth_half_mass_radius.value());
        let escape_birth = KilometresPerSecond::new(escape_central.value() * birth_ratio.sqrt());
        let central_density =
            central_density(p.mass.value(), core_radius.value(), tidal_radius.value());
        let v_eff = KilometresPerSecond::new(escape_birth.value() * EFFECTIVE_ESCAPE_FACTOR);
        // The table, not the sixteen-track quadrature it was made from (ruling 139.5).
        let kept = retention_tabulated(galaxy.mass_function(), &composition, v_eff);
        let f0 = BLACK_HOLE_BIRTH_FRACTION * kept.black_holes;
        let black_hole_fraction = black_hole_fraction(
            f0,
            BH_CLOCK_FACTOR * p.age.value() / relaxation_time.value(),
        );
        Self {
            parameters: p,
            composition,
            tidal_radius,
            core_radius,
            turn_off,
            relaxation_time,
            escape_central,
            escape_birth,
            central_density,
            retention: kept,
            black_hole_fraction,
        }
    }

    /// The model of a catalogue feature, or `None` for one that is no cluster at the epoch: a
    /// cloud, an association, an unbound embedded region or a dissolved cluster.
    ///
    /// An open cluster, old or a young bound nursery, takes Lamers et al. 2005's present mass
    /// and disruption rate ([`open_cluster`](super::kinds::open_cluster)), its birth half-mass
    /// radius equal to today's. A globular takes its history (P09.T13) on its bulk velocity, or on
    /// a circular orbit in a galaxy built without its kinematic tables. The cluster's own marks are
    /// drawn on its `feature.cluster` stream.
    #[must_use]
    #[expect(
        clippy::too_many_lines,
        reason = "one arm per kind of feature, each a short list of its parameters"
    )]
    pub fn from_record(galaxy: &Galaxy, feature: &FeatureRecord) -> Option<Self> {
        use super::kinds::nursery::NurseryStage;
        use super::kinds::open_cluster::{dissolution_time, evolution_survival, present_mass};
        let position = PointLy::from(feature.position());
        let mut stream = Stream::open(
            galaxy.seed(),
            tags::FEATURE_CLUSTER,
            ObjectKey::feature(feature.id().object_word()),
        );
        let marks = ClusterMarks::draw(&mut stream);
        let open = |age: Years,
                    m0: SolarMasses,
                    r_h: LightYears,
                    c: f64,
                    fe_h: Dex,
                    population,
                    age_spread| {
            let mass = present_mass(m0, age);
            if mass.value() <= 0.0 {
                return None;
            }
            let t_dis = dissolution_time(m0).value();
            let left = (1.0 - age.value() / t_dis).max(0.0);
            let rate = m0.value() * evolution_survival(age) / DISRUPTION_GAMMA_FOR_RATE
                * math::powf(left, 1.0 / DISRUPTION_GAMMA_FOR_RATE - 1.0)
                / t_dis;
            Some(ClusterParameters {
                kind: ClusterKind::Open,
                population,
                component: feature.component(),
                position,
                age,
                fe_h,
                mass,
                initial_mass: m0,
                half_mass_radius: r_h,
                birth_half_mass_radius: r_h,
                core_radius: None,
                concentration: c,
                mass_loss_rate: rate,
                age_spread,
                marks,
            })
        };
        let parameters = match feature.marks() {
            FeatureMarks::Cloud(_) => None,
            FeatureMarks::OpenCluster(m) => open(
                m.age_at_epoch(),
                m.initial_mass(),
                m.half_mass_radius(),
                m.concentration(),
                m.fe_h(),
                Population::OldThinDisc,
                Years::ZERO,
            ),
            FeatureMarks::Nursery(m) => {
                let bound = matches!(m.stage_at(Years::ZERO), NurseryStage::BoundCluster)
                    || (m.is_bound() && m.stage_at(Years::ZERO) == NurseryStage::Embedded);
                if bound {
                    open(
                        m.age_at_epoch(),
                        m.mass(),
                        m.half_mass_radius(),
                        m.concentration(),
                        m.fe_h(),
                        Population::YoungThinDisc,
                        m.age_spread(),
                    )
                } else {
                    None
                }
            }
            FeatureMarks::Globular(m) => {
                let velocity = super::catalogue::bulk_velocity(galaxy, feature);
                let h = history(
                    galaxy,
                    feature.position(),
                    velocity,
                    m.mass(),
                    m.half_mass_radius(),
                    m.age(),
                );
                Some(ClusterParameters {
                    kind: ClusterKind::Globular,
                    population: m.origin().population(),
                    component: feature.component(),
                    position,
                    age: m.age(),
                    fe_h: m.fe_h(),
                    mass: m.mass(),
                    initial_mass: h.initial_mass,
                    half_mass_radius: m.half_mass_radius(),
                    birth_half_mass_radius: h.birth_half_mass_radius,
                    core_radius: Some(m.core_radius()),
                    concentration: 0.0,
                    mass_loss_rate: h.mass_loss_rate,
                    age_spread: Years::ZERO,
                    marks,
                })
            }
        }?;
        Some(Self::new(galaxy, &parameters))
    }

    /// The parameters it was built from.
    #[must_use]
    pub const fn parameters(&self) -> &ClusterParameters {
        &self.parameters
    }

    /// Open or globular.
    #[must_use]
    pub const fn kind(&self) -> ClusterKind {
        self.parameters.kind
    }

    /// Its members' composition: the cluster's [Fe/H], no helium excess (the second population's
    /// is its members' own, P09.T9.h).
    #[must_use]
    pub const fn composition(&self) -> &Composition {
        &self.composition
    }

    /// Its present mass.
    #[must_use]
    pub const fn mass(&self) -> SolarMasses {
        self.parameters.mass
    }

    /// Its initial mass.
    #[must_use]
    pub const fn initial_mass(&self) -> SolarMasses {
        self.parameters.initial_mass
    }

    /// Its age at the epoch.
    #[must_use]
    pub const fn age(&self) -> Years {
        self.parameters.age
    }

    /// Its half-mass radius.
    #[must_use]
    pub const fn half_mass_radius(&self) -> LightYears {
        self.parameters.half_mass_radius
    }

    /// Its core radius.
    #[must_use]
    pub const fn core_radius(&self) -> LightYears {
        self.core_radius
    }

    /// The Plummer scale of its black holes' compact subsystem, `(0.1 + 0.2 f ÷ 0.06) r_h` held to
    /// 0.3 `r_h` (P09.T9.c; ours: the plan's 0.1–0.3 `r_h`, "correlated with f"), or `None` once
    /// they are gone.
    #[must_use]
    pub fn black_hole_scale(&self) -> Option<LightYears> {
        (self.black_hole_fraction > 0.0).then(|| {
            let f = (self.black_hole_fraction / BLACK_HOLE_BIRTH_FRACTION).min(1.0);
            LightYears::new(self.half_mass_radius().value() * (0.1 + 0.2 * f))
        })
    }

    /// The core its member classes' profiles take: its own, or while black holes remain, at
    /// least their subsystem's scale, since a cluster rich in them has a large core (the
    /// brainstorm, "What is inside a cluster today"; P09.T9.c's correlation of the core with f,
    /// ours in form). The escape speeds and the encounter rate read its own core.
    #[must_use]
    pub fn profile_core_radius(&self) -> LightYears {
        let core = self.core_radius.value();
        LightYears::new(
            self.black_hole_scale()
                .map_or(core, |s| core.max(s.value())),
        )
    }

    /// Its tidal (Jacobi) radius in the galaxy's potential.
    #[must_use]
    pub const fn tidal_radius(&self) -> LightYears {
        self.tidal_radius
    }

    /// The turn-off mass at its age and metallicity.
    #[must_use]
    pub const fn turn_off_mass(&self) -> SolarMasses {
        self.turn_off
    }

    /// Its half-mass relaxation time `t★`.
    #[must_use]
    pub const fn relaxation_time(&self) -> Years {
        self.relaxation_time
    }

    /// Its central escape speed today.
    #[must_use]
    pub const fn escape_speed_central(&self) -> KilometresPerSecond {
        self.escape_central
    }

    /// Its central escape speed at birth.
    #[must_use]
    pub const fn escape_speed_birth(&self) -> KilometresPerSecond {
        self.escape_birth
    }

    /// The effective escape speed retention reads (Design note 8).
    #[must_use]
    pub fn escape_speed_effective(&self) -> KilometresPerSecond {
        KilometresPerSecond::new(self.escape_birth.value() * EFFECTIVE_ESCAPE_FACTOR)
    }

    /// The one-dimensional velocity dispersion at radius `r` from the centre (Design note 9).
    #[must_use]
    pub fn sigma(&self, r: LightYears) -> KilometresPerSecond {
        let a = self.plummer_scale();
        let rr = r.value();
        KilometresPerSecond::new(
            (G * self.mass().value() / (6.0 * (rr * rr + a * a).sqrt())).sqrt(),
        )
    }

    /// The escape speed at radius `r` of the Plummer sphere of the cluster's mass and half-mass
    /// radius, `√(2 G M ÷ √(r² + a²))`: where a member's velocity is cut (P09.T10).
    #[must_use]
    pub fn escape_speed_at(&self, r: LightYears) -> KilometresPerSecond {
        let a = self.plummer_scale();
        let rr = r.value();
        KilometresPerSecond::new((2.0 * G * self.mass().value() / (rr * rr + a * a).sqrt()).sqrt())
    }

    /// The Plummer scale `a = r_h ÷ 1.305`, ly.
    #[must_use]
    pub fn plummer_scale(&self) -> f64 {
        self.half_mass_radius().value() / PLUMMER_HALF_MASS_RATIO
    }

    /// The Plummer sphere's mass inside `r`, `M r³ ÷ (r² + a²)^(3/2)`.
    #[must_use]
    pub fn enclosed_mass(&self, r: LightYears) -> SolarMasses {
        let a = self.plummer_scale();
        let rr = r.value();
        SolarMasses::new(self.mass().value() * rr * rr * rr / math::powf(rr * rr + a * a, 1.5))
    }

    /// The central density of its cored profile, M☉ per cubic light-year.
    #[must_use]
    pub const fn central_density(&self) -> f64 {
        self.central_density
    }

    /// Its encounter rate relative to 47 Tucanae's, `(ρ_c ÷ ρ_47)^1.5 (r_c ÷ r_c47)²`.
    #[must_use]
    pub fn encounter_rate(&self) -> f64 {
        let (rho47, rc47) = tuc_47_reference();
        math::powf(self.central_density / rho47, 1.5)
            * math::powf(self.core_radius.value() / rc47, 2.0)
    }

    /// The share of each remnant kind retained at birth.
    #[must_use]
    pub const fn retention(&self) -> &Retention {
        &self.retention
    }

    /// The black holes' mass fraction today, `f(t)` (module documentation).
    #[must_use]
    pub const fn black_hole_fraction(&self) -> f64 {
        self.black_hole_fraction
    }

    /// The black holes it holds today: their mass over [`MEAN_BLACK_HOLE_MASS`].
    #[must_use]
    pub fn black_hole_count(&self) -> f64 {
        self.black_hole_fraction * self.mass().value() / MEAN_BLACK_HOLE_MASS
    }

    /// Whether it is core-collapsed: no black holes left and older than 14 `t★` (P09.T9.d).
    #[must_use]
    pub fn is_core_collapsed(&self) -> bool {
        self.black_hole_fraction <= 0.0
            && self.age().value() > CORE_COLLAPSE_RELAXATION_TIMES * self.relaxation_time.value()
    }

    /// A core-collapsed cluster's cusp slope `γ` in `ρ ∝ r^−γ`, on 1.6–2 by its mark; `None` for
    /// a cluster with a core.
    #[must_use]
    pub fn cusp_slope(&self) -> Option<f64> {
        self.is_core_collapsed().then(|| {
            let (lo, hi) = CUSP_SLOPE_RANGE;
            lo + (hi - lo) * self.parameters.marks.cusp_uniform()
        })
    }

    /// Expected neutron stars and black holes left per primary star formed (retained or not),
    /// from the kick quadrature.
    #[must_use]
    pub const fn remnants_per_primary(&self) -> (f64, f64) {
        (
            self.retention.neutron_stars_per_primary,
            self.retention.black_holes_per_primary,
        )
    }

    /// The expected millisecond pulsars: `40 (Γ ÷ Γ_47Tuc)^0.7`, capped at 40 once core-collapsed
    /// (P09.T9.e). Open clusters hold none.
    #[must_use]
    pub fn millisecond_pulsars(&self) -> f64 {
        if self.kind() == ClusterKind::Open {
            return 0.0;
        }
        let n = PULSARS_AT_47_TUC_GAMMA * math::powf(self.encounter_rate(), PULSAR_GAMMA_EXPONENT);
        if self.is_core_collapsed() {
            n.min(PULSAR_CORE_COLLAPSE_CAP)
        } else {
            n
        }
    }

    /// The expected quiescent low-mass X-ray binaries (module documentation; provisional).
    #[must_use]
    pub fn xray_binaries(&self) -> f64 {
        if self.kind() == ClusterKind::Open {
            return 0.0;
        }
        XRAY_BINARIES_AT_47_TUC_GAMMA
            * math::powf(self.encounter_rate(), XRAY_BINARY_GAMMA_EXPONENT)
    }

    /// The expected blue stragglers (module documentation; provisional).
    #[must_use]
    pub fn blue_stragglers(&self) -> f64 {
        let core_mass = self.central_density * core_mass_volume(self.core_radius.value());
        BLUE_STRAGGLERS_AT_1E4_CORE * math::powf(core_mass / 1e4, BLUE_STRAGGLER_CORE_EXPONENT)
    }

    /// The mass it is losing to its tails, M☉ per year.
    #[must_use]
    pub const fn mass_loss_rate(&self) -> f64 {
        self.parameters.mass_loss_rate
    }

    /// The population its members' budget is.
    #[must_use]
    pub const fn population(&self) -> Population {
        self.parameters.population
    }

    /// Its centre, ly.
    #[must_use]
    pub const fn position(&self) -> PointLy {
        self.parameters.position
    }

    /// Its own marks.
    #[must_use]
    pub const fn marks(&self) -> &ClusterMarks {
        &self.parameters.marks
    }
}

/// `t★ = 0.138 √(M r_h³ ÷ G) ÷ (⟨m⟩ ln Λ)` (module documentation).
#[must_use]
pub fn relaxation_time(mass: SolarMasses, half_mass_radius: LightYears) -> Years {
    let r = half_mass_radius.value();
    let ly_per_km_s = BH_RELAXATION_PREFACTOR * (mass.value() * r * r * r / G).sqrt()
        / (RELAXATION_MEAN_MASS * COULOMB_LOGARITHM);
    Years::new(ly_per_km_s * YEARS_PER_LY_PER_KM_S)
}

/// The central escape speed `√(G M ÷ r_h) × 10^(0.1055 + 0.2550 u − 0.0769 u²)`, `u = log₁₀(r_h ÷
/// r_c)` (module documentation).
#[must_use]
pub fn central_escape_speed(
    mass: SolarMasses,
    half_mass_radius: LightYears,
    core_radius: LightYears,
) -> KilometresPerSecond {
    let u = math::log10(half_mass_radius.value() / core_radius.value());
    let fit = 0.1055 + 0.2550 * u - 0.0769 * u * u;
    KilometresPerSecond::new(
        (G * mass.value() / half_mass_radius.value()).sqrt() * math::exp10(fit),
    )
}

/// The central density, M☉ per cubic light-year, of the profile `(1 + r² ÷ r_c²)^(−3/2) (1 − r² ÷
/// r_t²)²` inside `r_t` holding `mass`, by 64 Gauss–Legendre nodes in `ln(1 + r ÷ r_c)`.
#[must_use]
pub fn central_density(mass: f64, core: f64, tidal: f64) -> f64 {
    let tidal = tidal.max(core * 1.001);
    let top = math::ln_1p(tidal / core);
    let integrand = |t: f64| {
        let r = core * math::exp_m1(t);
        let x = r / core;
        let taper = 1.0 - (r / tidal) * (r / tidal);
        4.0 * core::f64::consts::PI
            * r
            * r
            * math::powf(1.0 + x * x, -1.5)
            * taper
            * taper
            * (r + core)
    };
    let volume = gl_panels(integrand, &[0.0, 0.5 * top, top]);
    mass / volume
}

/// The volume `4π r_c³ (asinh 1 − 1 ÷ √2)` that the cored profile's core, `r < r_c`, fills at the
/// central density: the core's mass over `ρ_c`.
fn core_mass_volume(core: f64) -> f64 {
    4.0 * core::f64::consts::PI
        * core
        * core
        * core
        * (math::asinh(1.0) - core::f64::consts::FRAC_1_SQRT_2)
}

/// 47 Tucanae's central density (M☉ per cubic light-year) and core radius (ly) by this model's
/// forms, Γ's reference.
fn tuc_47_reference() -> (f64, f64) {
    let core = TUC_47_CORE_PC * LIGHT_YEARS_PER_PARSEC;
    let tidal = TUC_47_TIDAL_PC * LIGHT_YEARS_PER_PARSEC;
    (central_density(TUC_47_MASS, core, tidal), core)
}

/// `f(t)` at `t ÷ t★ = x` for birth fraction `f₀` (module documentation), floored at zero.
#[must_use]
pub fn black_hole_fraction(f0: f64, x: f64) -> f64 {
    let psi = BH_LOSS_PSI_SLOPE;
    (((1.0 + psi * f0) * math::exp(-BH_LOSS_BETA * psi * x) - 1.0) / psi).max(0.0)
}

/// The time at which `f` reaches zero for birth fraction `f₀`, on the law's own clock `k t ÷ t★`
/// (`k` = [`BH_CLOCK_FACTOR`], so today's `t★` divide it by `k`): `ln(1 + ψ₁ f₀) ÷ (β ψ₁)`.
#[must_use]
pub fn black_hole_lifetime(f0: f64) -> f64 {
    math::ln_1p(BH_LOSS_PSI_SLOPE * f0) / (BH_LOSS_BETA * BH_LOSS_PSI_SLOPE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Seed;
    use crate::galaxy::features::testing::{M4, M15, OMEGA_CEN, PAL_5, TUC_47, named_cluster};
    use crate::galaxy::params::GalaxyParams;

    fn pc(value: f64) -> LightYears {
        LightYears::new(value * LIGHT_YEARS_PER_PARSEC)
    }

    /// P09.T7: the named clusters' central escape speeds against Baumgardt and Hilker's.
    #[test]
    fn the_named_clusters_escape_speeds_match_the_catalogue() {
        for (name, want) in [
            (TUC_47, 47.4),
            (OMEGA_CEN, 62.2),
            (M4, 18.8),
            (M15, 48.9),
            (PAL_5, 2.1),
        ] {
            let g = named_cluster(name);
            assert!(
                (g.escape_speed - want).abs() < 1e-9,
                "{name}'s catalogue value"
            );
            let v = central_escape_speed(
                SolarMasses::new(g.mass),
                pc(g.half_mass_radius_pc),
                pc(g.core_radius_pc),
            )
            .value();
            assert!((v / want - 1.0).abs() < 0.05, "{name}: {v} against {want}");
        }
    }

    /// P09.T7's open clusters (ruling 126.6): central escape speeds of 0.92, 2.2 and 5.3 km/s
    /// (±10%) for 10², 10³ and 10⁴ M☉ at P09.T4.a's median radius and a concentration of 1 at the
    /// Sun.
    #[test]
    fn open_clusters_escape_at_a_few_kilometres_per_second() {
        let galaxy =
            Galaxy::from_params(Seed::new(0x0907), GalaxyParams::milky_way_like()).unwrap();
        let mut last = 0.0;
        for (mass, want) in [(1e2, 0.92), (1e3, 2.2), (1e4, 5.3)] {
            let model = open_model(&galaxy, mass);
            let v = model.escape_speed_central().value();
            eprintln!("{mass} M☉: {v} km/s");
            assert!((v / want - 1.0).abs() <= 0.10, "{mass} M☉: {v} km/s");
            assert!(v > last);
            last = v;
        }
    }

    pub(crate) fn open_model(galaxy: &Galaxy, mass: f64) -> ClusterModel {
        let r_h = super::super::kinds::open_cluster::half_mass_radius(SolarMasses::new(mass), 0.0);
        ClusterModel::new(
            galaxy,
            &ClusterParameters {
                kind: ClusterKind::Open,
                population: Population::OldThinDisc,
                component: None,
                position: PointLy::new(0.0, 26_000.0, 0.0),
                age: Years::new(3e8),
                fe_h: Dex::ZERO,
                mass: SolarMasses::new(mass),
                initial_mass: SolarMasses::new(mass),
                half_mass_radius: r_h,
                birth_half_mass_radius: r_h,
                core_radius: None,
                concentration: 1.0,
                mass_loss_rate: 0.0,
                age_spread: Years::ZERO,
                marks: ClusterMarks::MEDIAN,
            },
        )
    }

    #[test]
    fn the_relaxation_time_is_spitzer_s() {
        // 47 Tucanae: about 10^9.8 yr against the catalogue's 10^9.70 (its own definition).
        let g = named_cluster(TUC_47);
        let t = relaxation_time(SolarMasses::new(g.mass), pc(g.half_mass_radius_pc)).value();
        assert!((math::log10(t) - 9.84).abs() < 0.05, "{}", math::log10(t));
    }

    #[test]
    fn the_central_density_is_the_catalogue_s_at_47_tucanae() {
        let (rho, _) = tuc_47_reference();
        let per_pc3 = rho * math::powf(LIGHT_YEARS_PER_PARSEC, 3.0);
        assert!(
            (math::log10(per_pc3) - 4.72).abs() < 0.15,
            "{}",
            math::log10(per_pc3)
        );
    }

    /// P09.T9.c: black holes are gone after four to six relaxation times of the law's clock `k t ÷
    /// t★` for retentions of 0.6–1 (1.6–2.4 of today's `t★` at `k` = 2.5; ruling 126.4).
    #[test]
    fn black_holes_are_gone_in_four_to_six_relaxation_times() {
        for kept in [0.6, 0.8, 1.0] {
            let f0 = BLACK_HOLE_BIRTH_FRACTION * kept;
            let t = black_hole_lifetime(f0);
            assert!((4.0..=6.0).contains(&t), "{kept}: {t}");
            assert!(black_hole_fraction(f0, t * 0.999) > 0.0);
            assert!(black_hole_fraction(f0, t * 1.001).abs() < 1e-15);
            assert!((black_hole_fraction(f0, 0.0) - f0).abs() < 1e-15);
        }
        // The decay rate β ψ₁ is derived: 0.41.
        assert!((BH_LOSS_BETA * BH_LOSS_PSI_SLOPE - 0.4116).abs() < 1e-4);
    }

    #[test]
    fn the_median_marks_have_no_scatter() {
        assert!(ClusterMarks::MEDIAN.depletion_normal().abs() < 1e-15);
        assert!((ClusterMarks::MEDIAN.cusp_uniform() - 0.5).abs() < 1e-9);
    }
}
