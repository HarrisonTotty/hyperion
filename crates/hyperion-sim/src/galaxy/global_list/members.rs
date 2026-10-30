//! The global list's members and the record they share (plan 10, Design note 13).
//!
//! A dwarf core's members are placed in its nested grid by plan 09's machinery
//! ([`dwarf_core`](super::dwarf_core), P10.T7.b), and a stream's in its tube's own cells (P10.T8).
//! Each is a plan 03 [`SystemRecord`] of origin [`SystemOrigin::GlobalListMember`] with what a
//! member carries beyond one ([`GlobalMemberRecord`]): no density component, the halo's
//! population, and the age, composition and velocity of its core or its stream.
//!
//! # A stream's band shares (P10.T8.a)
//!
//! A stream holds what its progenitor lost ([`StreamBandShares`]; brainstorm, "Streams and
//! accreted structure": "the evolved mass function minus the cluster's present one"), as a share
//! of its members in each band, living stars and white dwarfs apart, and neutron stars and black
//! holes none (Design note 9).
//!
//! - **A globular's**, per system formed on the galaxy's mass function φ at the cluster's age and
//!   [Fe/H]: the evolved canonical population less what plan 09's model keeps of it, `K` of every
//!   class and the depleted slope's `d(m)` of the living stars below 0.8 M☉. So a living star of
//!   initial mass `m` below the turn-off weighs `φ(m) (1 − K d(m))` and a white dwarf of a
//!   progenitor of `m` up to 8 M☉ weighs `φ(m) (1 − K)`: the stream is bottom-heavy and holds white
//!   dwarfs. A living globular's `K` and slope are its model's ([`ClusterStars::kept`]). An orphan
//!   keeps nothing at the end of its life, `K = 0`, so its stream is the evolved canonical
//!   population and the end-of-life slope the plan names changes nothing (lean, 2026-10-30).
//! - **A dwarf's** takes its core's classes (P10.T7.a): its history from its stellar mass at
//!   infall and accretion time, cut at the tube's last stripping, since the tube holds no star the
//!   core formed after it (research-p10's advice §4), at the dwarf's mean [Fe/H], its core's.
//! - **Populations**: every member is of the first population, as plan 09's tails are labelled
//!   (lean, 2026-10-30: "mostly first-population stars", the brainstorm).
//!
//! A member's star is drawn attempt after attempt as a core member's: attempt `k` reads word `k`
//! of `stream.member.age` for a dwarf's age from its history (a globular's is the cluster's), and
//! six words of `member.mass` from `6k`, an initial mass in its band and a mark that keeps it with
//! its weight's odds over the band's greatest; a living star is kept below its age's turn-off, a
//! dead one only if its remnant is a white dwarf.

use crate::coords::GalacticVelocity;
use crate::galaxy::Galaxy;
use crate::galaxy::features::interior::counts::{
    WHITE_DWARF_PROGENITOR_MAX, depletion, moments, white_dwarf_mass,
};
use crate::galaxy::features::interior::{
    ClassKind, Generation, MemberClass, Multiplicity, ProfileShape,
};
use crate::galaxy::features::members::{ATTEMPT_WORDS, MAX_ATTEMPTS, remnant_fits, systemic_speed};
use crate::galaxy::imf::{MASS_BAND_EDGES, MassBand, MassFunction};
use crate::galaxy::params::ProgenitorKind;
use crate::galaxy::placement::{SystemOrigin, SystemRecord};
use crate::id::SystemId;
use crate::rng::{ObjectKey, Stream, Threshold, tags};
use crate::stellar::Composition;
use crate::stellar::multiplicity::MultiplicityContext;
use crate::stellar::sse::turn_off_mass;
use crate::stellar::system::SystemStars;
use crate::units::{Dex, HeliumExcess, SolarMasses, Years};

use super::StreamOrigin;
use super::dwarf_core::{
    MEAN_METALLICITY_SCATTER_DEX, StarFormationHistory, classes, mass_metallicity,
    metallicity_spread,
};
use super::spec::{MassLoss, StreamSpec};

/// A member of the global list: its record and what it carries beyond one (Design note 13).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlobalMemberRecord {
    record: SystemRecord,
    class: MemberClass,
    composition: Composition,
    velocity: GalacticVelocity,
}

impl GlobalMemberRecord {
    /// A member from its parts: its record, of origin [`SystemOrigin::GlobalListMember`], its
    /// class, its composition and its velocity at the epoch, galactic.
    ///
    /// # Panics
    ///
    /// In debug builds, if the record's origin is not a global-list member's.
    #[must_use]
    pub(crate) fn new(
        record: SystemRecord,
        class: MemberClass,
        composition: Composition,
        velocity: GalacticVelocity,
    ) -> Self {
        debug_assert!(
            matches!(record.origin(), SystemOrigin::GlobalListMember { .. }),
            "a global-list member's record, not {:?}",
            record.origin()
        );
        Self {
            record,
            class,
            composition,
            velocity,
        }
    }

    /// Its plan 03 record, of origin [`SystemOrigin::GlobalListMember`].
    #[must_use]
    pub const fn record(&self) -> &SystemRecord {
        &self.record
    }

    /// Its class.
    #[must_use]
    pub const fn class(&self) -> &MemberClass {
        &self.class
    }

    /// Its composition: its own \[Fe/H\], drawn about its core's or its stream's.
    #[must_use]
    pub const fn composition(&self) -> &Composition {
        &self.composition
    }

    /// Its velocity at the epoch, galactic: its core's or its tube's bulk motion plus its own.
    #[must_use]
    pub const fn velocity(&self) -> GalacticVelocity {
        self.velocity
    }

    /// The attempt its primary's draws are read at, which its class's conditional draw chose.
    #[must_use]
    pub fn attempt(&self) -> u32 {
        match self.record.origin() {
            SystemOrigin::GlobalListMember { attempt }
            | SystemOrigin::FeatureMember { attempt, .. }
            | SystemOrigin::CentreMember { attempt } => u32::from(attempt),
            SystemOrigin::Grid(_) => 0,
        }
    }

    /// Its stars: [`SystemStars::generate_with`] at its composition, its multiplicity its own to
    /// draw ([`MultiplicityContext::Free`]): a core's and a stream's classes are single
    /// (P10.T7.a, as built), and plan 11 draws a member's companions as a field star's.
    #[must_use]
    pub fn stars(&self, galaxy: &Galaxy) -> SystemStars {
        SystemStars::generate_with(
            galaxy,
            &self.record,
            &self.composition,
            MultiplicityContext::Free,
        )
    }
}

/// The youngest age a turn-off is read at, years: 10⁴, past the zero of a history still forming
/// stars, as plan 09's centre reads it.
const YOUNGEST_TURN_OFF_AGE: f64 = 1e4;

/// One band of a stream's members: the shares of its living stars and its white dwarfs in all the
/// stream's members, their classes, and the greatest weight a mass of the band has.
#[derive(Debug, Clone, Copy, PartialEq)]
struct BandShare {
    living: f64,
    white_dwarfs: f64,
    living_class: MemberClass,
    white_dwarf_class: MemberClass,
    /// The greatest weight of a mass in the band, which a mass is kept against.
    peak: f64,
}

/// How a stream's stars formed and what its progenitor kept of them (module documentation).
#[derive(Debug, Clone, Copy, PartialEq)]
enum StreamStars {
    /// A globular's or an orphan's: one age and [Fe/H], and plan 09's `K` and depleted slope.
    Cluster {
        age: Years,
        composition: Composition,
        kept: f64,
        slope: f64,
    },
    /// A dwarf's: its core's history, cut at the tube's last stripping, and its mean [Fe/H] and
    /// spread.
    Dwarf {
        history: StarFormationHistory,
        fe_h: Dex,
        spread: Dex,
    },
}

/// A stream's members by band (plan 10, P10.T8.a; module documentation, "A stream's band shares"):
/// what its progenitor lost, as shares of its members, and how a member's star is drawn.
///
/// # Examples
///
/// ```no_run
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::global_list::Debris;
/// use hyperion_sim::galaxy::global_list::members::StreamBandShares;
/// use hyperion_sim::galaxy::imf::MassBand;
/// use hyperion_sim::galaxy::params::GalaxyParams;
///
/// let galaxy = Galaxy::from_params(Seed::new(7), GalaxyParams::milky_way_like())?
///     .with_full_potential();
/// for stream in Debris::generate(&galaxy).streams() {
///     let shares = StreamBandShares::of(&galaxy, stream);
///     // Every member is in some band, and none is a neutron star or a black hole.
///     let total: f64 = MassBand::ALL.iter().map(|&b| shares.share(b)).sum();
///     assert!((total - 1.0).abs() < 1e-12);
///     assert!(shares.mean_member_mass().value() < 1.0);
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct StreamBandShares {
    bands: [BandShare; 5],
    stars: StreamStars,
    /// The members' mean present mass, M☉.
    mean_mass: f64,
}

impl StreamBandShares {
    /// The band shares of `spec`'s stream in `galaxy` (module documentation).
    ///
    /// # Panics
    ///
    /// If a globular's or an orphan's stream carries no cluster's stars, or a dwarf's names a
    /// progenitor the galaxy does not have, which no [`Debris`](super::Debris) stream does.
    #[must_use]
    pub fn of(galaxy: &Galaxy, spec: &StreamSpec) -> Self {
        let f = galaxy.mass_function();
        match spec.origin() {
            StreamOrigin::LivingGlobular(_) | StreamOrigin::Orphan => {
                let stars = spec
                    .stars()
                    .expect("a globular's or an orphan's stream carries its cluster's stars");
                let composition = Composition::from_fe_h(stars.fe_h(), HeliumExcess::ZERO);
                let turn_off = turn_off_mass(stars.age(), &composition).value();
                let (kept, slope) = (stars.kept(), stars.depleted_slope());
                let bands =
                    MassBand::ALL.map(|band| cluster_band(f, band, turn_off, (kept, slope)));
                Self::normalised(
                    bands,
                    StreamStars::Cluster {
                        age: stars.age(),
                        composition,
                        kept,
                        slope,
                    },
                )
            }
            StreamOrigin::Dwarf(j) => {
                let progenitor = galaxy
                    .params()
                    .accretion()
                    .progenitors()
                    .iter()
                    .find(|p| p.kind() == ProgenitorKind::Recent(j))
                    .expect("a dwarf's stream names one of the galaxy's recent progenitors");
                let (infall, accreted) = (progenitor.mass(), progenitor.accreted());
                let stripped = match spec.mass_loss() {
                    MassLoss::Pulsed {
                        last_pericentre, ..
                    } => last_pericentre,
                    MassLoss::Steady(_) | MassLoss::Dissolved { .. } => Years::new(0.0),
                };
                let history = StarFormationHistory::of(infall, accreted).no_younger_than(stripped);
                // The core's mean, offset on its own stream (P10.T7.a).
                let key = ObjectKey::galaxy_item(ProgenitorKind::Recent(j).number());
                let offset = Stream::open(galaxy.seed(), tags::DWARF_CORE_METALLICITY, key)
                    .normal(0.0, MEAN_METALLICITY_SCATTER_DEX);
                let fe_h = Dex::new(mass_metallicity(infall).value() + offset);
                let composition = Composition::from_fe_h(fe_h, HeliumExcess::ZERO);
                let counts = classes(
                    f,
                    &composition,
                    &history,
                    ProfileShape::Plummer { scale: 1.0 },
                    1.0,
                );
                let bands = MassBand::ALL.map(|band| {
                    let of = |kind: ClassKind| {
                        counts
                            .iter()
                            .find(|c| c.class.band == band && c.class.kind == kind)
                            .map(|c| (c.expected, c.class))
                    };
                    let (living, living_class) = of(ClassKind::Living)
                        .unwrap_or((0.0, empty_class(band, ClassKind::Living)));
                    let (white_dwarfs, white_dwarf_class) = of(ClassKind::WhiteDwarf)
                        .unwrap_or((0.0, empty_class(band, ClassKind::WhiteDwarf)));
                    BandShare {
                        living,
                        white_dwarfs,
                        living_class,
                        white_dwarf_class,
                        peak: 1.0,
                    }
                });
                Self::normalised(
                    bands,
                    StreamStars::Dwarf {
                        history,
                        fe_h,
                        spread: metallicity_spread(infall),
                    },
                )
            }
        }
    }

    /// `bands` with their shares divided by their sum.
    #[must_use]
    fn normalised(mut bands: [BandShare; 5], stars: StreamStars) -> Self {
        let total: f64 = bands.iter().map(|b| b.living + b.white_dwarfs).sum();
        assert!(
            total > 0.0 && total.is_finite(),
            "a stream's progenitor lost some stars: {total}"
        );
        let mut mass = 0.0;
        for band in &mut bands {
            band.living /= total;
            band.white_dwarfs /= total;
            mass += band.living * band.living_class.mean_mass.value()
                + band.white_dwarfs * band.white_dwarf_class.mean_mass.value();
        }
        Self {
            bands,
            stars,
            mean_mass: mass,
        }
    }

    /// `band`'s share of the members, its living stars' and white dwarfs'; the shares sum to 1.
    #[must_use]
    pub fn share(&self, band: MassBand) -> f64 {
        let b = &self.bands[band.index()];
        b.living + b.white_dwarfs
    }

    /// `band`'s living stars' share of the members.
    #[must_use]
    pub fn living(&self, band: MassBand) -> f64 {
        self.bands[band.index()].living
    }

    /// `band`'s white dwarfs' share of the members.
    #[must_use]
    pub fn white_dwarfs(&self, band: MassBand) -> f64 {
        self.bands[band.index()].white_dwarfs
    }

    /// The members' mean present mass: a living star's initial mass, a white dwarf's its
    /// Kalirai et al. 2008 mass.
    #[must_use]
    pub fn mean_member_mass(&self) -> SolarMasses {
        SolarMasses::new(self.mean_mass)
    }

    /// The composition of the member `system`: its globular's [Fe/H], or its dwarf's mean plus
    /// the dwarf's spread times one normal of `stream.member.abundance`, with no helium excess.
    #[must_use]
    pub fn composition(&self, galaxy: &Galaxy, system: SystemId) -> Composition {
        let key = ObjectKey::from(system);
        match self.stars {
            StreamStars::Cluster { composition, .. } => composition,
            StreamStars::Dwarf { fe_h, spread, .. } => {
                let offset = Stream::open(galaxy.seed(), tags::STREAM_MEMBER_ABUNDANCE, key)
                    .standard_normal();
                Composition::from_fe_h(
                    Dex::new(fe_h.value() + spread.value() * offset),
                    HeliumExcess::ZERO,
                )
            }
        }
    }

    /// The star of the member `system` in `band`, of `composition` (module documentation): its
    /// class, its primary's initial mass, the attempt its draws are read at and its age; `None` if
    /// the band holds no member or the draw exhausts its attempts.
    ///
    /// # Panics
    ///
    /// In debug builds, if a band that holds members exhausts its attempts.
    #[must_use]
    pub fn draw_star(
        &self,
        galaxy: &Galaxy,
        band: MassBand,
        system: SystemId,
        composition: &Composition,
    ) -> Option<(MemberClass, SolarMasses, u32, Years)> {
        let share = &self.bands[band.index()];
        if share.living + share.white_dwarfs <= 0.0 {
            return None;
        }
        let seed = galaxy.seed();
        let key = ObjectKey::from(system);
        let (lo, hi) = band_edges(band);
        let imf = galaxy.mass_function();
        let mut ages = Stream::open(seed, tags::STREAM_MEMBER_AGE, key);
        let mut masses = Stream::open(seed, tags::MEMBER_MASS, key);
        for attempt in 0..MAX_ATTEMPTS {
            let age = match self.stars {
                StreamStars::Cluster { age, .. } => age,
                StreamStars::Dwarf { history, .. } => {
                    ages.seek(u64::from(attempt));
                    history.age_quantile(ages.uniform())
                }
            };
            masses.seek(ATTEMPT_WORDS * u64::from(attempt));
            let mass = imf.quantile_in(lo, hi, masses.uniform());
            let mark = masses.mark();
            let systemic = systemic_speed(&mut masses);
            let turn_off = turn_off_mass(
                Years::new(age.value().max(YOUNGEST_TURN_OFF_AGE)),
                composition,
            )
            .value();
            let living = mass < turn_off;
            let weight = match self.stars {
                StreamStars::Cluster { kept, slope, .. } => {
                    if living {
                        1.0 - kept * depletion(mass, slope)
                    } else {
                        1.0 - kept
                    }
                }
                StreamStars::Dwarf { .. } => 1.0,
            };
            if !mark.is_below(Threshold::from_probability(
                (weight / share.peak).clamp(0.0, 1.0),
            )) {
                continue;
            }
            let class = if living {
                share.living_class
            } else if mass <= WHITE_DWARF_PROGENITOR_MAX
                && remnant_fits(
                    ClassKind::WhiteDwarf,
                    mass,
                    composition,
                    (seed, system, attempt),
                    age,
                    (f64::INFINITY, systemic),
                )
            {
                share.white_dwarf_class
            } else {
                continue;
            };
            return Some((class, SolarMasses::new(mass), attempt, age));
        }
        debug_assert!(
            false,
            "a stream member of band {band:?} exhausted its attempts"
        );
        None
    }
}

/// The initial-mass edges of `band`, M☉.
#[must_use]
fn band_edges(band: MassBand) -> (f64, f64) {
    (
        MASS_BAND_EDGES[band.index()],
        MASS_BAND_EDGES[band.index() + 1],
    )
}

/// A class of `band` and `kind` that holds no member, for a band a dwarf's stream has none of.
#[must_use]
fn empty_class(band: MassBand, kind: ClassKind) -> MemberClass {
    let (lo, hi) = band_edges(band);
    MemberClass {
        kind,
        band,
        generation: Generation::First,
        multiplicity: Multiplicity::Single,
        initial_mass_range: (SolarMasses::new(lo), SolarMasses::new(hi)),
        mean_mass: SolarMasses::new(f64::midpoint(lo, hi)),
    }
}

/// A globular's stream's `band` (module documentation): its living stars below `turn_off` (M☉)
/// and its white dwarfs from progenitors up to 8 M☉, each weighted by what the cluster lost with
/// `(kept, slope)`, plan 09's `K` and depleted slope, per system formed on `f`.
#[must_use]
fn cluster_band(
    f: &dyn MassFunction,
    band: MassBand,
    turn_off: f64,
    (kept, slope): (f64, f64),
) -> BandShare {
    let (lo, hi) = band_edges(band);
    let class = |kind, range: (f64, f64), mean: f64| MemberClass {
        kind,
        band,
        generation: Generation::First,
        multiplicity: Multiplicity::Single,
        initial_mass_range: (SolarMasses::new(range.0), SolarMasses::new(range.1)),
        mean_mass: SolarMasses::new(mean),
    };
    let living_range = (lo, hi.min(turn_off));
    let (living, living_mass) = if living_range.1 > living_range.0 {
        moments(f, living_range.0, living_range.1, |m| {
            1.0 - kept * depletion(m, slope)
        })
    } else {
        (0.0, 0.0)
    };
    let dead_range = (lo.max(turn_off), hi.min(WHITE_DWARF_PROGENITOR_MAX));
    let (white_dwarfs, initial) = if dead_range.1 > dead_range.0 {
        moments(f, dead_range.0, dead_range.1, |_| 1.0 - kept)
    } else {
        (0.0, 0.0)
    };
    let mean = |n: f64, m: f64, fallback: f64| if n > 0.0 { m / n } else { fallback };
    // The weights fall with mass, the depletion rising with it, so a band's greatest is at its
    // foot: its living stars' if it has any, else its white dwarfs'.
    let peak = if living_range.1 > living_range.0 {
        1.0 - kept * depletion(lo, slope)
    } else {
        1.0 - kept
    };
    BandShare {
        living,
        white_dwarfs,
        living_class: class(
            ClassKind::Living,
            living_range,
            mean(living, living_mass, lo),
        ),
        white_dwarf_class: class(
            ClassKind::WhiteDwarf,
            dead_range,
            white_dwarf_mass(mean(white_dwarfs, initial, dead_range.0)),
        ),
        peak: peak.max(1.0 - kept),
    }
}

#[cfg(test)]
mod tests;
