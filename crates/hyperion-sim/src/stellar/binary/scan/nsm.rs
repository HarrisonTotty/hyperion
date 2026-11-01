//! The neutron-star mergers' scan marks (P11.T8.d; design note 10): what a galaxy-wide scan needs
//! of a merger of two compact stars, drawn first and on draws of its own, without the binary
//! engine.
//!
//! A neutron-star merger is a one-shot entry: one merger at its time T, uniform over the source
//! horizon ([`horizon_time`](super::horizon_time)), which P11.T9's kilonova and P12.T5.b's walk
//! read with the ejecta's mass ([`NsmScanMarks::ejecta_mass`]). The marks are drawn in this order,
//! each group on its own stream:
//!
//! 1. the site and its component, thinned on the class's intensity (`class.nsm.site`);
//! 2. T, and the system's age at the epoch from its component's own age law among its born
//!    systems (`class.nsm.time`);
//! 3. its \[Fe/H\] from its component's law at the site and that age (`class.nsm.metallicity`);
//! 4. its pair (`class.nsm.mass`): two neutron stars, or a black hole and a neutron star, by
//!    [`NSM_SAMPLER`]'s weights, each compact star's mass at the merger, and its progenitor's
//!    initial mass from the galaxy's mass function (the primary is the heavier progenitor);
//! 5. its velocity at the site, plan 08's draw of its component's law (`class.nsm.velocity`).
//!
//! What it leaves (the remnant) follows from the total mass by plan 11's rule for two compact
//! stars (P11.T4.d, the engine's `mix`): a neutron star's mass in a black hole above plan 06's
//! heaviest neutron star, [`MAX_NEUTRON_STAR_MASS`] (Mandel and Müller 2020's 2.0 M☉), so every
//! merger of two neutron stars leaves a black hole. The ejecta of two neutron stars are GW170817's
//! ([`GW170817_EJECTA_MASS`]), since the class has no model of its own for them. A black hole
//! ejects what its tides leave outside it and unbind ([`NsbhOutflow`]): nothing at the class's
//! usual pair, a hole of some 7 M☉ and q ≈ 5, which swallows its neutron star whole.
//! From `MERGER_CLASS_VERSION` (26) the remnant keeps the total less its own ejecta
//! ([`remnant_mass_under`]); its kind stays the engine's rule on the total.
//!
//! A candidate is kept exactly when both progenitors have died by T: their lifetimes (plan 06's
//! [`lifetime`]) are both under the system's age at T, which refuses only a system of a young
//! population formed less than some 30 Myr before T. The class's own test cannot refuse a
//! candidate the scan keeps: a merger in the horizon of two compact stars of which one is a
//! neutron star is a neutron-star merger.

use crate::coords::{GalacticPosition, GalacticVelocity};
use crate::galaxy::Galaxy;
use crate::galaxy::catalogue_classes::CatalogueCellKey;
use crate::galaxy::catalogue_classes::binary::{ClassHosts, MergerClassRules, NeutronStarMergers};
use crate::galaxy::fields::ComponentId;
use crate::galaxy::{PointLy, Population};
use crate::id::{BodyId, CatalogueSystemId, SystemId};
use crate::math;
use crate::rng::{ObjectKey, Stream, tags};
use crate::stellar::binary::light_curves::GW170817_EJECTA_MASS;
use crate::stellar::draws::StarDraws;
use crate::stellar::remnant::RemnantRecipe;
use crate::stellar::remnant::collapse::MAX_NEUTRON_STAR_MASS;
use crate::stellar::remnant::structure::neutron_star_radius;
use crate::stellar::system::track_mass_of;
use crate::stellar::{Composition, Phase, lifetime};
use crate::tables::binary::{
    NSM_BLACK_HOLE_PROGENITORS, NSM_NEUTRON_STAR_PROGENITORS, NSM_SAMPLER, NsmMark, NsmPairKind,
    NsmPick,
};
use crate::time::UniverseTime;
use crate::units::consts::{GM_SUN, SOLAR_RADIUS_M, SPEED_OF_LIGHT};
use crate::units::{Dex, HeliumExcess, PerCubicLightYear, SolarMasses, Years};

use super::super::sampler::ClassSampler;
use super::horizon_time;

/// What a galaxy-wide scan reads of a neutron-star merger (P11.T8.d): its place and motion, its
/// merger time T, its pair, its ejecta and its remnant.
///
/// # Examples
///
/// The neutron-star mergers of a galaxy are rare, about ten in the source horizon at Milky Way
/// parameters: the whole galaxy's, with their kilonovae's ejecta, as an alert scan reads them. A
/// black hole's merger may eject nothing.
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::catalogue_classes::binary::NeutronStarMergers;
/// use hyperion_sim::galaxy::catalogue_classes::{CatalogueCellKey, ClassId};
/// use hyperion_sim::stellar::Phase;
/// use hyperion_sim::stellar::binary::scan::scan_nsm_cell;
///
/// let galaxy = Galaxy::new(Seed::new(9));
/// let process = NeutronStarMergers::new(&galaxy);
/// let mut mergers = Vec::new();
/// for x in -4..4 {
///     for y in -4..4 {
///         for z in -1..1 {
///             let key = CatalogueCellKey::new(ClassId::NEUTRON_STAR_MERGER, [x, y, z])?;
///             scan_nsm_cell(&galaxy, &process, key, &mut mergers);
///         }
///     }
/// }
/// assert!(mergers.len() < 100);
/// for merger in &mergers {
///     assert!(merger.ejecta_mass().value() >= 0.0);
///     assert_eq!(merger.remnant_phase(), Phase::BlackHole);
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NsmScanMarks {
    id: CatalogueSystemId,
    component: ComponentId,
    population: Population,
    position: GalacticPosition,
    velocity: GalacticVelocity,
    merger_time: UniverseTime,
    age_at_epoch: Years,
    composition: Composition,
    kind: NsmPairKind,
    masses: [SolarMasses; 2],
    progenitor_masses: [SolarMasses; 2],
    collapse_ages: [Years; 2],
    merger_class: MergerClassRules,
}

impl NsmScanMarks {
    /// The merger's ID: member 0 of its catalogue entry.
    #[must_use]
    pub const fn id(&self) -> CatalogueSystemId {
        self.id
    }

    /// The density component the merger was drawn from.
    #[must_use]
    pub const fn component(&self) -> ComponentId {
        self.component
    }

    /// The component's population.
    #[must_use]
    pub const fn population(&self) -> Population {
        self.population
    }

    /// Where the system is at the epoch: its site in its cell.
    #[must_use]
    pub const fn position(&self) -> &GalacticPosition {
        &self.position
    }

    /// The system's velocity, galactic, the one its entry moves at (plan 09's rule 6).
    #[must_use]
    pub const fn velocity(&self) -> GalacticVelocity {
        self.velocity
    }

    /// The system's position at `t`, on its straight line through its site at the epoch: at
    /// [`merger_time`](Self::merger_time), where its kilonova goes off.
    ///
    /// # Panics
    ///
    /// As plan 09's `CatalogueMotion::position_at`, beyond any time the clock reaches.
    #[must_use]
    pub fn position_at(&self, t: UniverseTime) -> GalacticPosition {
        crate::galaxy::catalogue_classes::CatalogueMotion::straight(self.position, self.velocity)
            .position_at(t)
    }

    /// The merger time T, inside the source horizon, `[−(H + L), +H)`: the onset of its kilonova.
    #[must_use]
    pub const fn merger_time(&self) -> UniverseTime {
        self.merger_time
    }

    /// The system's age at the epoch, Julian years.
    #[must_use]
    pub const fn age_at_epoch(&self) -> Years {
        self.age_at_epoch
    }

    /// The system's age at T, Julian years: the pair's `merger_age`, as plan 03's record counts
    /// its age at a time.
    #[must_use]
    pub fn merger_age(&self) -> Years {
        Years::new(self.age_at_epoch.value() + self.merger_time.since_epoch().as_julian_years_f64())
    }

    /// The composition the system formed with.
    #[must_use]
    pub const fn composition(&self) -> &Composition {
        &self.composition
    }

    /// The pair's kind.
    #[must_use]
    pub const fn kind(&self) -> NsmPairKind {
        self.kind
    }

    /// Each compact star's mass at T, M☉, primary first: a neutron star of 1.13–2.0 M☉, or for a
    /// neutron star and a black hole, the black hole first.
    #[must_use]
    pub const fn masses(&self) -> [SolarMasses; 2] {
        self.masses
    }

    /// Each compact star's phase, primary first.
    #[must_use]
    pub const fn phases(&self) -> [Phase; 2] {
        match self.kind {
            NsmPairKind::DoubleNeutronStar => [Phase::NeutronStar, Phase::NeutronStar],
            NsmPairKind::NeutronStarBlackHole => [Phase::BlackHole, Phase::NeutronStar],
        }
    }

    /// The pair's total mass at T, M☉.
    #[must_use]
    pub fn total_mass(&self) -> SolarMasses {
        self.masses[0] + self.masses[1]
    }

    /// Each progenitor's initial mass, M☉, primary first, the primary the heavier: the record's
    /// primary is the first.
    #[must_use]
    pub const fn progenitor_masses(&self) -> [SolarMasses; 2] {
        self.progenitor_masses
    }

    /// The age at which each progenitor died, leaving its compact star, Julian years since the
    /// system formed: plan 06's lifetime of its initial mass.
    #[must_use]
    pub const fn collapse_ages(&self) -> [Years; 2] {
        self.collapse_ages
    }

    /// The mass the merger ejects, the kilonova's, M☉: [`GW170817_EJECTA_MASS`] for two neutron
    /// stars, since the class has no model of its own for them, and for a black hole and a neutron
    /// star the [`NsbhOutflow`]'s ejecta, zero where the hole swallows the star whole.
    #[must_use]
    pub fn ejecta_mass(&self) -> SolarMasses {
        match self.kind {
            NsmPairKind::DoubleNeutronStar => GW170817_EJECTA_MASS,
            NsmPairKind::NeutronStarBlackHole => {
                NsbhOutflow::of(self.masses[0], self.masses[1]).ejecta()
            }
        }
    }

    /// What the merger leaves, by the total mass (P11.T4.d's rule): a black hole above plan 06's
    /// heaviest neutron star, [`MAX_NEUTRON_STAR_MASS`], or below it a neutron star; a black hole
    /// always where one merges.
    #[must_use]
    pub fn remnant_phase(&self) -> Phase {
        let neutron_stars = self.kind == NsmPairKind::DoubleNeutronStar;
        if neutron_stars && self.total_mass() <= MAX_NEUTRON_STAR_MASS {
            Phase::NeutronStar
        } else {
            Phase::BlackHole
        }
    }

    /// The remnant's mass, M☉: the pair's total, as the engine's merger of two compact stars keeps
    /// it, under the version's rules; from 26 the total less its own ejecta,
    /// [`ejecta_mass`](Self::ejecta_mass) (P11.T8.d's rider).
    #[must_use]
    pub fn remnant_mass(&self) -> SolarMasses {
        remnant_mass_under(self.total_mass(), self.ejecta_mass(), self.merger_class)
    }
}

/// The mass a neutron-star merger of `total` M☉ that ejects `ejecta` M☉ leaves in its remnant
/// under `rules` (P11.T8.d's rider, at `MERGER_CLASS_VERSION`; research-leans-13 §2(b)): the
/// total, as the engine's merger of two compact stars keeps it, under the held arm; from 26 the
/// total less the merger's own ejecta, the class's published [`NsmScanMarks::ejecta_mass`], so that
/// the class conserves its mass: GW170817's 2.74 M☉ (Abbott et al. 2017, PRL 119, 161101, Table I,
/// the low-spin prior; recalled, not read at source) leaves 2.69 for two neutron stars, and a black
/// hole that swallows its neutron star whole ([`NsbhOutflow`]) keeps the pair's mass. The energy
/// the inspiral radiates in gravitational waves is not taken off: its source was not read in this
/// build, and the values recalled are lower bounds (a finding).
#[must_use]
pub(crate) fn remnant_mass_under(
    total: SolarMasses,
    ejecta: SolarMasses,
    rules: MergerClassRules,
) -> SolarMasses {
    match rules {
        MergerClassRules::ContactOnly => total,
        MergerClassRules::Channels => SolarMasses::new(total.value() - ejecta.value()),
    }
}

#[cfg(test)]
impl NsmScanMarks {
    /// The marks with their pair replaced by a black hole of `black_hole` and a neutron star of
    /// `neutron_star`, for the tests of what reads the pair.
    #[must_use]
    pub(crate) const fn with_black_hole_pair(
        mut self,
        black_hole: SolarMasses,
        neutron_star: SolarMasses,
    ) -> Self {
        self.kind = NsmPairKind::NeutronStarBlackHole;
        self.masses = [black_hole, neutron_star];
        self
    }
}

/// The share of a neutron star's baryonic mass left outside a black hole that disrupts it, by
/// Foucart, Hinderer and Nissanke's (2018, Phys. Rev. D 98, 081501(R), equation 4) fit to
/// numerical relativity: `[max(α (1 − 2C) ÷ η^⅓ − β R̂ C ÷ η + γ, 0)]^δ`, with C the neutron star's
/// compactness, η = Q ÷ (1 + Q)² the pair's symmetric mass ratio at Q = `M_BH` ÷ `M_NS`, and R̂ the
/// hole's innermost stable circular orbit over its gravitational radius. (α, β, γ, δ) = (0.406,
/// 0.139, 0.255, 1.761), fitted over Q of 1–7, hole spins of −0.5 to 0.9 and C of 0.13–0.182. The
/// paper could not be reached in this build: the constants and range are recalled, and the
/// constants are those of `PyCBC`'s `foucart18` (`pycbc/neutron_stars/eos_utils.py`) and, to the
/// third figure, of NMMA's `remnant_disk_mass_fitting` (`nmma/core/conversion.py`), both read at
/// source.
const FOUCART_2018_REMNANT: (f64, f64, f64, f64) = (0.406, 0.139, 0.255, 1.761);

/// The dynamical ejecta of a black hole's merger with a neutron star, as a share of the neutron
/// star's baryonic mass, by Krüger and Foucart's (2020, Phys. Rev. D 101, 103002, equation 9) fit:
/// `a₁ Q^n₁ (1 − 2C) ÷ C − a₂ Q^n₂ R̂ + a₄`, none where it is negative. (a₁, a₂, a₄, n₁, n₂) =
/// (7.115 951 54 × 10⁻³, 1.436 368 03 × 10⁻³, −2.762 029 90 × 10⁻², 0.863 604 211, 1.683 995 07).
/// The paper could not be reached in this build: the constants are those that gwemlightcurves'
/// `KrFo2019.FoucartEjecta` (Coughlin et al.'s kilonova code) and NMMA's `dynamic_mass_fitting`
/// carry, read at source, which agree; the range it was fitted over is unread, and every pair
/// here that ejects anything lies inside Foucart et al.'s Q and C.
const KRUGER_FOUCART_2020_DYNAMICAL: (f64, f64, f64, f64, f64) = (
    7.115_951_54e-3,
    1.436_368_03e-3,
    -2.762_029_90e-2,
    0.863_604_211,
    1.683_995_07,
);

/// The innermost stable circular orbit of a black hole that does not spin, in its gravitational
/// radius `GM ÷ c²`: 6 (Bardeen, Press and Teukolsky 1972, ApJ 178, 347).
///
/// The class's holes are taken not to spin, χ = 0 (lean, 2026-10-31): the spins of the first two
/// neutron star–black hole mergers, GW200105 and GW200115, are consistent with zero (Abbott et al.
/// 2021, ApJL 915, L5; recalled, not read at source).
const NON_SPINNING_ISCO: f64 = 6.0;

/// The share of the disc about a black hole that its outflows unbind, ejecta of the kilonova: 0.15
/// (lean, 2026-10-31), the share gwemlightcurves' `KrFo2019.calc_meje` takes of Krüger and
/// Foucart's disc (read at source). Simulations of such discs unbind some 15–40% (as Krüger and
/// Foucart 2020 survey them; recalled, not read at source), so the ejecta may err low.
const DISC_WIND_SHARE: f64 = 0.15;

/// What a black hole leaves outside itself when it merges with a neutron star (P11.T9.b), at the
/// hole's spin χ = 0 (lean), M☉: the remnant outside the hole (Foucart, Hinderer and Nissanke
/// 2018, Phys. Rev. D 98, 081501(R), equation 4), its dynamical ejecta (Krüger and Foucart 2020,
/// Phys. Rev. D 101, 103002, equation 9, at most the remnant), the bound disc they leave, and the
/// kilonova's ejecta, the dynamical ejecta and 0.15 of the disc, its outflows (lean). The module's
/// `FOUCART_2018_REMNANT`, `KRUGER_FOUCART_2020_DYNAMICAL`, `NON_SPINNING_ISCO` and
/// `DISC_WIND_SHARE` hold the figures and their sources.
///
/// The neutron star's radius is plan 06's, 12.2 km, at every mass, its compactness C = `GM_NS` ÷
/// (R c²), and both fits scale with its baryonic mass, `M_NS` (1 + 0.6 C ÷ (1 − 0.5 C)) (Krüger and
/// Foucart 2020, equation 7, after Lattimer and Prakash 2001, ApJ 550, 426). A hole heavier than
/// some four times its neutron star, as most of the class's are, leaves nothing.
///
/// # Examples
///
/// ```
/// use hyperion_sim::stellar::binary::scan::NsbhOutflow;
/// use hyperion_sim::units::SolarMasses;
///
/// let m = SolarMasses::new;
/// // The class's usual pair, a 7 M☉ hole and a 1.33 M☉ neutron star, ejects nothing.
/// assert!(NsbhOutflow::of(m(7.0), m(1.33)).ejecta().value() < f64::MIN_POSITIVE);
/// // A light hole tears its neutron star apart and ejects a share of it.
/// let light = NsbhOutflow::of(m(3.0), m(1.2));
/// assert!(light.ejecta().value() > 0.01 && light.ejecta() < light.remnant());
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NsbhOutflow {
    remnant: SolarMasses,
    dynamical: SolarMasses,
    disc: SolarMasses,
    ejecta: SolarMasses,
}

impl NsbhOutflow {
    /// The outflow of a black hole of `black_hole` merging with a neutron star of `neutron_star`.
    ///
    /// Both masses are positive and finite: the class draws holes of 3–20 M☉ and neutron stars of
    /// 1.13–2.0 M☉ (`NSM_SAMPLER`). The fits were calibrated on Q of 1–7 and C of 0.13–0.182, and
    /// leave nothing outside the hole well before either limit is passed, so a pair beyond them,
    /// as many of the class's are, ejects nothing, as a hole that swallows its neutron star whole
    /// does.
    ///
    /// # Panics
    ///
    /// In debug builds, for a mass that is not positive and finite.
    #[must_use]
    pub fn of(black_hole: SolarMasses, neutron_star: SolarMasses) -> Self {
        debug_assert!(
            [black_hole, neutron_star]
                .iter()
                .all(|m| m.value().is_finite() && m.value() > 0.0),
            "a merger's masses are positive and finite: {black_hole:?}, {neutron_star:?}"
        );
        let radius = neutron_star_radius(RemnantRecipe::default()).value() * SOLAR_RADIUS_M;
        let c = GM_SUN * neutron_star.value() / (radius * SPEED_OF_LIGHT * SPEED_OF_LIGHT);
        let baryonic = neutron_star.value() * (1.0 + 0.6 * c / (1.0 - 0.5 * c));
        let q = black_hole.value() / neutron_star.value();
        let eta = q / ((1.0 + q) * (1.0 + q));
        let (alpha, beta, gamma, delta) = FOUCART_2018_REMNANT;
        let fit =
            alpha * (1.0 - 2.0 * c) / math::cbrt(eta) - beta * NON_SPINNING_ISCO * c / eta + gamma;
        let remnant = baryonic * math::powf(fit.max(0.0), delta);
        let (a1, a2, a4, n1, n2) = KRUGER_FOUCART_2020_DYNAMICAL;
        let dynamical = baryonic
            * (a1 * math::powf(q, n1) * (1.0 - 2.0 * c) / c
                - a2 * math::powf(q, n2) * NON_SPINNING_ISCO
                + a4);
        let dynamical = dynamical.clamp(0.0, remnant);
        let disc = remnant - dynamical;
        Self {
            remnant: SolarMasses::new(remnant),
            dynamical: SolarMasses::new(dynamical),
            disc: SolarMasses::new(disc),
            ejecta: SolarMasses::new(dynamical + DISC_WIND_SHARE * disc),
        }
    }

    /// The mass left outside the hole: its dynamical ejecta and its disc, M☉.
    #[must_use]
    pub const fn remnant(&self) -> SolarMasses {
        self.remnant
    }

    /// The tidal tail the merger unbinds at once, M☉.
    #[must_use]
    pub const fn dynamical(&self) -> SolarMasses {
        self.dynamical
    }

    /// The disc bound to the hole, M☉.
    #[must_use]
    pub const fn disc(&self) -> SolarMasses {
        self.disc
    }

    /// The kilonova's ejecta, the dynamical ejecta and 0.15 of the disc, M☉: zero where nothing
    /// is left outside the hole.
    #[must_use]
    pub const fn ejecta(&self) -> SolarMasses {
        self.ejecta
    }
}

/// The scan marks of candidate `id` of the neutron-star merger class cell `cell` under its
/// `bound`, or `None` if the class keeps no entry there (P11.T8.d; module documentation). It never
/// runs the binary engine.
///
/// The marks are the entry's own, bit for bit: the class's candidate draws them through this
/// function first.
///
/// # Panics
///
/// If `cell` is not a neutron-star merger cell; in debug builds, on a density above the bound.
#[must_use]
pub fn nsm_scan_marks(
    galaxy: &Galaxy,
    process: &NeutronStarMergers,
    cell: &CatalogueCellKey,
    id: CatalogueSystemId,
    bound: PerCubicLightYear,
) -> Option<NsmScanMarks> {
    let (position, component) =
        process
            .hosts()
            .site(galaxy, cell, id, bound, tags::CLASS_NSM_SITE)?;
    let seed = galaxy.seed();
    let system = SystemId::from(id);
    let key = ObjectKey::from(system);
    let sampler = ClassSampler::new(&NSM_SAMPLER);
    let laws = galaxy.fields().component(component);
    // 2. T and the age.
    let mut stream = Stream::open(seed, tags::CLASS_NSM_TIME, key);
    let merger_time = horizon_time(stream.uniform());
    let ages = laws.ages();
    let unborn = ages.cdf(Years::ZERO);
    let rank = sampler.draw(NsmMark::AgeRank, 0.0, &mut stream);
    let age_at_epoch = ages.quantile(unborn + (1.0 - unborn) * rank);
    let merger_age = age_at_epoch.value() + merger_time.since_epoch().as_julian_years_f64();
    // 3. The composition, at the site and the age.
    let feh = laws.metallicity(&PointLy::from(&position), age_at_epoch);
    let mut stream = Stream::open(seed, tags::CLASS_NSM_METALLICITY, key);
    let composition = Composition::from_fe_h(
        Dex::new(stream.normal(feh.mean().value(), feh.sigma().value())),
        HeliumExcess::ZERO,
    );
    // 4. The pair: its kind, the compact stars' masses, their progenitors.
    let mut stream = Stream::open(seed, tags::CLASS_NSM_MASS, key);
    let kind = NsmPairKind::ALL[sampler.pick(NsmPick::PairKind, &mut stream)];
    let neutron_star =
        |stream: &mut Stream| SolarMasses::new(sampler.draw(NsmMark::NeutronStarMass, 0.0, stream));
    let first = match kind {
        NsmPairKind::DoubleNeutronStar => neutron_star(&mut stream),
        NsmPairKind::NeutronStarBlackHole => SolarMasses::new(math::exp10(sampler.draw(
            NsmMark::LogBlackHoleMass,
            0.0,
            &mut stream,
        ))),
    };
    let second = neutron_star(&mut stream);
    let progenitor = |range: (f64, f64), mark: NsmMark, stream: &mut Stream| {
        let rank = sampler.draw(mark, 0.0, stream);
        galaxy.mass_function().quantile_in(range.0, range.1, rank)
    };
    let ns_mark = NsmMark::NeutronStarProgenitorRank;
    let (a, b) = match kind {
        NsmPairKind::DoubleNeutronStar => (
            progenitor(NSM_NEUTRON_STAR_PROGENITORS, ns_mark, &mut stream),
            progenitor(NSM_NEUTRON_STAR_PROGENITORS, ns_mark, &mut stream),
        ),
        NsmPairKind::NeutronStarBlackHole => (
            progenitor(
                NSM_BLACK_HOLE_PROGENITORS,
                NsmMark::BlackHoleProgenitorRank,
                &mut stream,
            ),
            progenitor(NSM_NEUTRON_STAR_PROGENITORS, ns_mark, &mut stream),
        ),
    };
    let progenitor_masses = [SolarMasses::new(a.max(b)), SolarMasses::new(a.min(b))];
    // Both progenitors have died by T.
    let collapse_ages = [0_u16, 1].map(|body| {
        let draws = StarDraws::for_star(seed, BodyId::new(system, body));
        lifetime(
            track_mass_of(progenitor_masses[usize::from(body)]),
            &composition,
            &draws,
        )
    });
    if !(collapse_ages[0].value() < merger_age && collapse_ages[1].value() < merger_age) {
        return None;
    }
    // 5. The velocity.
    let velocity =
        ClassHosts::velocity(galaxy, component, &position, tags::CLASS_NSM_VELOCITY, key);
    Some(NsmScanMarks {
        id,
        component,
        population: laws.population(),
        position,
        velocity,
        merger_time,
        age_at_epoch,
        composition,
        kind,
        masses: [first, second],
        progenitor_masses,
        collapse_ages,
        merger_class: galaxy.rules().merger_class,
    })
}

/// Appends to `out` the scan marks of every entry of the neutron-star merger class cell `key`, in
/// index order: one bound, one count and [`nsm_scan_marks`] of each candidate, the cell's entries
/// exactly.
///
/// # Panics
///
/// As [`nsm_scan_marks`], and as plan 09's count on a bound past the Poisson sampler's limit.
pub fn scan_nsm_cell(
    galaxy: &Galaxy,
    process: &NeutronStarMergers,
    key: CatalogueCellKey,
    out: &mut Vec<NsmScanMarks>,
) {
    let bound = process.hosts().bound(galaxy, &key);
    let candidates =
        crate::galaxy::catalogue_classes::grid::candidate_count(galaxy.seed(), &key, bound);
    out.extend((0..candidates).filter_map(|index| {
        let id = key
            .candidate_id(index, 0)
            .expect("a count held under 2^24 numbers every candidate");
        nsm_scan_marks(galaxy, process, &key, id, bound)
    }));
}
