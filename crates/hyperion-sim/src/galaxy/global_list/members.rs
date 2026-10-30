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
//!
//! # A stream's members (P10.T8.b)
//!
//! Members are a Poisson process in the tube's own coordinates, cell by cell of a [`TubeCell`]
//! and a band (Design notes 7 and 8):
//!
//! - **The mean** of a cell is the tube's mass over the stream's mean member mass and its along
//!   cells, times the cell's gap noise, the band's share, and along each across axis the normal
//!   distribution's mass over the cell's span divided by its mass within the cut at ±4 widths
//!   ([`expected_members`]). So the means of every cell and band sum to the tube's mass over the
//!   mean member mass, its expected members, to the gap noise's rounding.
//! - **The count** is a Poisson draw of that mean on `stream.cell`, keyed by `ObjectKey::cell` of
//!   the raw ID of the cell's member 0 in its band, clamped at the 13-bit index's 8,192 with a
//!   debug assertion, as plan 03 clamps a grid cell's.
//! - **Member `i`**'s three uniforms on `stream.member.position`, keyed by its
//!   [`StreamMemberId`], put it uniformly in its along cell and in each across cell by the
//!   inverse transform of the normal distribution truncated to it, through plan 06's
//!   [`normal_quantile`](crate::math::normal_quantile); its position is the tube's
//!   [`locate`](TubeTable::locate) of the three, so [`TubeTable::cells_touching`] finds it. A
//!   member outside the root cube is dropped and its index kept. Nothing is thinned: every member
//!   counted is a member, its star drawn by [`StreamBandShares::draw_star`].
//!
//! # A stream member's motion (P10.T8.c)
//!
//! Its velocity is the tube's mean velocity at its along coordinate ([`TubeTable::mean_velocity`])
//! plus a normal draw of the tube's three dispersions there ([`TubeTable::dispersions_at`]) along
//! its local frame's tangent, normal and binormal ([`TubeTable::frame_at`]): a Box–Muller pair and
//! the first normal of a second on `stream.member.velocity`, four words. Every band takes the
//! tracers' dispersions, which carry no mass. It drifts on the straight line of its own velocity
//! ([`Drift::of_global_member`](crate::observe::Drift::of_global_member)), since plan 03's
//! `query::motion` serves grid records only.

use crate::coords::GalacticVelocity;
use crate::galaxy::Galaxy;
use crate::galaxy::Population;
use crate::galaxy::features::interior::counts::{
    WHITE_DWARF_PROGENITOR_MAX, depletion, moments, white_dwarf_mass,
};
use crate::galaxy::features::interior::{
    ClassKind, Generation, MemberClass, Multiplicity, ProfileShape,
};
use crate::galaxy::features::members::{ATTEMPT_WORDS, MAX_ATTEMPTS, remnant_fits, systemic_speed};
use crate::galaxy::imf::{MASS_BAND_EDGES, MassBand, MassFunction};
use crate::galaxy::params::ProgenitorKind;
use crate::galaxy::placement::ResolveSystemError;
use crate::galaxy::placement::{SystemOrigin, SystemRecord};
use crate::id::{MemberSlot, StreamMemberId, SystemId};
use crate::math;
use crate::rng::{ObjectKey, Stream, Threshold, tags};
use crate::stellar::Composition;
use crate::stellar::multiplicity::MultiplicityContext;
use crate::stellar::sse::turn_off_mass;
use crate::stellar::system::SystemStars;
use crate::units::{Dex, HeliumExcess, MetresPerSecond, SolarMasses, Years};

use super::StreamOrigin;
use super::dwarf_core::{
    MEAN_METALLICITY_SCATTER_DEX, StarFormationHistory, classes, mass_metallicity,
    metallicity_spread,
};
use super::spec::{MassLoss, StreamSpec};
use super::tube::{CUT_WIDTHS, TubeCell, TubeTable};

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

    /// `band`'s share of the members, its living stars' and white dwarfs'; the stellar bands'
    /// shares sum to 1, and a substellar band holds none.
    #[must_use]
    pub fn share(&self, band: MassBand) -> f64 {
        self.band(band).map_or(0.0, |b| b.living + b.white_dwarfs)
    }

    /// `band`'s living stars' share of the members.
    #[must_use]
    pub fn living(&self, band: MassBand) -> f64 {
        self.band(band).map_or(0.0, |b| b.living)
    }

    /// `band`'s white dwarfs' share of the members.
    #[must_use]
    pub fn white_dwarfs(&self, band: MassBand) -> f64 {
        self.band(band).map_or(0.0, |b| b.white_dwarfs)
    }

    /// The shares of a stellar band, or `None` for a substellar one.
    #[must_use]
    fn band(&self, band: MassBand) -> Option<&BandShare> {
        band.is_stellar().then(|| &self.bands[band.index()])
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
        let share = self.band(band)?;
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

/// The standard normal distribution's cumulative function, by the complementary error function,
/// which keeps its tail's digits.
#[must_use]
fn normal_cdf(x: f64) -> f64 {
    0.5 * math::erfc(-x / core::f64::consts::SQRT_2)
}

/// The standard normal distribution's mass within the cut, `Φ(4) − Φ(−4)`.
#[must_use]
fn cut_mass() -> f64 {
    normal_cdf(CUT_WIDTHS) - normal_cdf(-CUT_WIDTHS)
}

/// The expected members of `band` in `cell` of `tube`, whose stream's band shares are `shares`
/// (module documentation, "A stream's members").
///
/// # Panics
///
/// If `cell`'s along cell is not below the tube's [`along_cells`](TubeTable::along_cells).
#[must_use]
pub fn expected_members(
    tube: &TubeTable,
    shares: &StreamBandShares,
    cell: TubeCell,
    band: MassBand,
) -> f64 {
    let share = shares.share(band);
    if share <= 0.0 {
        return 0.0;
    }
    let per_cell =
        tube.mass().value() / (shares.mean_member_mass().value() * f64::from(tube.along_cells()));
    let cut = cut_mass();
    let across = cell
        .across_widths()
        .map(|[lo, hi]| (normal_cdf(hi) - normal_cdf(lo)) / cut);
    per_cell * tube.noise(cell.along()) * share * across[0] * across[1]
}

/// The ID of member `index` of `band` in `cell` of `tube`.
///
/// # Panics
///
/// If `band` is not stellar, `cell` is not one of the tube's or `index` is 8,192 or more.
#[must_use]
pub fn stream_member_id(
    tube: &TubeTable,
    cell: TubeCell,
    band: MassBand,
    index: u16,
) -> StreamMemberId {
    assert!(
        band.is_stellar(),
        "a stream's members are stars, not {band:?}"
    );
    let along = u16::try_from(cell.along()).expect("a tube's along cells number at most 16,384");
    StreamMemberId::new(
        tube.stream().get(),
        band.layer(),
        along,
        cell.across(),
        index,
    )
    .expect("a tube's cell, a stellar band and an index below 8,192 make an ID")
}

/// The member count of `band` in `cell` of `tube` (module documentation, "A stream's members").
///
/// # Panics
///
/// As [`stream_member_id`]; in debug builds, if the draw passes the index.
#[must_use]
pub fn stream_member_count(
    galaxy: &Galaxy,
    tube: &TubeTable,
    shares: &StreamBandShares,
    cell: TubeCell,
    band: MassBand,
) -> u16 {
    let mean = expected_members(tube, shares, cell, band);
    let word = SystemId::from(stream_member_id(tube, cell, band, 0)).raw();
    let drawn = Stream::open(galaxy.seed(), tags::STREAM_CELL, ObjectKey::cell(word)).poisson(mean);
    let limit = MemberSlot::INDEX_LIMIT;
    debug_assert!(
        drawn <= u64::from(limit),
        "a tube cell drew {drawn} members on a mean of {mean}, past its index's {limit}"
    );
    u16::try_from(drawn.min(u64::from(limit))).expect("a count clamped at 8,192 fits in u16")
}

/// The tube coordinates `(u, a, b)` of the member `system` in `cell` of a tube of `cells` along
/// cells: three uniforms of `stream.member.position` (module documentation).
#[must_use]
pub(crate) fn member_coordinates(
    galaxy: &Galaxy,
    system: SystemId,
    cell: TubeCell,
    cells: u32,
) -> [f64; 3] {
    let mut stream = Stream::open(
        galaxy.seed(),
        tags::STREAM_MEMBER_POSITION,
        ObjectKey::from(system),
    );
    let u = (f64::from(cell.along()) + stream.uniform()) / f64::from(cells);
    let [a, b] = cell.across_widths().map(|[lo, hi]| {
        let (p, q) = (normal_cdf(lo), normal_cdf(hi));
        // Inside (0, 1) by the cut's ±4 widths, which no rounding of the ranks leaves.
        math::normal_quantile(p + stream.uniform() * (q - p)).clamp(lo, hi)
    });
    [u, a, b]
}

/// Member `index` of `band` in `cell` of `tube`, whose stream's band shares are `shares`, or
/// `None` if it lies outside the root cube or its star's draw exhausts its attempts (module
/// documentation, "A stream's members"). The index is not checked against the count.
///
/// # Panics
///
/// As [`stream_member_id`].
#[must_use]
pub fn stream_member(
    galaxy: &Galaxy,
    tube: &TubeTable,
    shares: &StreamBandShares,
    cell: TubeCell,
    band: MassBand,
    index: u16,
) -> Option<GlobalMemberRecord> {
    let system = SystemId::from(stream_member_id(tube, cell, band, index));
    let [u, a, b] = member_coordinates(galaxy, system, cell, tube.along_cells());
    let position = tube.locate(u, a, b);
    if !position.in_root_cube() {
        return None;
    }
    let composition = shares.composition(galaxy, system);
    let (class, mass, attempt, age) = shares.draw_star(galaxy, band, system, &composition)?;
    let record = SystemRecord::from_parts(
        system,
        position,
        SystemOrigin::GlobalListMember {
            attempt: u16::try_from(attempt).expect("an attempt below 4,096 fits in u16"),
        },
        Population::Halo,
        mass,
        age,
    );
    Some(GlobalMemberRecord::new(
        record,
        class,
        composition,
        member_velocity(galaxy, tube, system, u),
    ))
}

/// The velocity of the member `system` at along coordinate `u` of `tube` (module documentation,
/// "A stream member's motion").
#[must_use]
fn member_velocity(
    galaxy: &Galaxy,
    tube: &TubeTable,
    system: SystemId,
    u: f64,
) -> GalacticVelocity {
    let mut stream = Stream::open(
        galaxy.seed(),
        tags::STREAM_MEMBER_VELOCITY,
        ObjectKey::from(system),
    );
    let (first, second) = stream.standard_normal_pair();
    let third = stream.standard_normal();
    let frame = tube.frame_at(u);
    let sigma = tube.dispersions_at(u).map(MetresPerSecond::value);
    let mean = tube.mean_velocity(u).metres_per_second();
    let draws = [first, second, third];
    GalacticVelocity::new(core::array::from_fn(|i| {
        mean[i]
            + sigma[0] * draws[0] * frame[0][i]
            + sigma[1] * draws[1] * frame[1][i]
            + sigma[2] * draws[2] * frame[2][i]
    }))
}

/// Every member of `band` in `cell` of `tube`, in index order, into `out` (cleared first): the
/// cell's count, each index's member if it is one (module documentation, "A stream's members").
///
/// # Panics
///
/// As [`stream_member_count`].
pub fn stream_cell_members(
    galaxy: &Galaxy,
    tube: &TubeTable,
    shares: &StreamBandShares,
    cell: TubeCell,
    band: MassBand,
    out: &mut Vec<GlobalMemberRecord>,
) {
    out.clear();
    if !band.is_stellar() {
        return;
    }
    let count = stream_member_count(galaxy, tube, shares, cell, band);
    out.extend((0..count).filter_map(|i| stream_member(galaxy, tube, shares, cell, band, i)));
}

/// The member `id` names in the stream of `tube`, whose band shares are `shares`.
///
/// # Errors
///
/// [`ResolveSystemError::NoSuchSystem`] if `id` is not of `tube`'s stream, its band is not
/// stellar, its along cell is past the tube's, its index is at or past its cell's count, or its
/// member lies outside the root cube or its draw exhausted its attempts.
pub fn resolve_stream_member(
    galaxy: &Galaxy,
    tube: &TubeTable,
    shares: &StreamBandShares,
    id: StreamMemberId,
) -> Result<GlobalMemberRecord, ResolveSystemError> {
    let band = MassBand::of_layer(id.band());
    let cell = TubeCell::from(id);
    if id.number() != tube.stream().get()
        || !band.is_stellar()
        || cell.along() >= tube.along_cells()
    {
        return Err(ResolveSystemError::NoSuchSystem);
    }
    if id.index() >= stream_member_count(galaxy, tube, shares, cell, band) {
        return Err(ResolveSystemError::NoSuchSystem);
    }
    stream_member(galaxy, tube, shares, cell, band, id.index())
        .ok_or(ResolveSystemError::NoSuchSystem)
}

#[cfg(test)]
mod tests;
