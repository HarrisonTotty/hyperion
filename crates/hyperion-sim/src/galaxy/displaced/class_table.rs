//! The once-per-galaxy quadrature that splits each birth population's layer budget into alive or
//! retained, displaced classes and gone (plan 08, P08.T9 and Design notes 13–19 and 23–24).
//!
//! # The classes
//!
//! Remnants of the thin disc take 56 classes, eight speed bins by seven age bins; the thick disc,
//! the halo, the bulge, the bar and the nuclear disc eight speed bins each; and the hypervelocity
//! survivors one more, registered at zero weight (Design note 25): 97 in all, in that order
//! ([`ClassKey::id`]). Runaways and walkaways join the thin disc's and the nuclear disc's classes of
//! their speed and time since ejection as further kinds ([`runaway`](super::runaway)).
//!
//! # The quadrature
//!
//! For each source its reference metallicity is the count-weighted mean of its components'
//! \[Fe/H\] at a reference site (3 `R_d` in the plane for the discs and the halo, the centre for the
//! bulge, bar and nuclear disc) at their mean ages. At each of 33 log-spaced masses of layers D
//! and E ([`MassNodes`]), weighted by the galaxy's mass function, the class table reads:
//!
//! - the star's lifetime `L`, plan 06's [`lifetime`] at the reference metallicity and the median
//!   star's draws;
//! - for each field component, the share of its born systems alive (age below `L`) and dead in
//!   each age bin (age in `L` plus the bin's edges in `R_d ÷ v_c`), exactly, through the age
//!   distribution's CDF;
//! - for the dead of layer E, plan 06's kick law in each speed bin by remnant kind and mode,
//!   through [`kick_bins::speed_bin_shares`], and the share leaving nothing, which is gone;
//! - for the living of the thin disc and the nuclear disc, the runaway and walkaway shares by
//!   speed bin and time since ejection ([`RunawayModel`]), which leave the field.
//!
//! A dead remnant of the thin disc, thick disc or halo in the lowest speed bin is retained: it
//! stays in the field component that placed it. Every other remnant meets plan 15's form table
//! ([`FormTable`]): the share `in_cube` of its class, at the galaxy's escape ratio, is bound and
//! inside the root cube; `unbound_in_cube` is unbound and still inside, and joins the fastest
//! class of its age bin with its origin bin as a mark (Design note 23); the rest is gone. For the
//! bulge, bar and nuclear disc the bound part splits again by the own-form share at the galaxy's
//! corotation ratio: that share keeps the population's own form and is retained, and the rest is
//! the class's spheroid (Design note 13).
//!
//! Class weights are shares of the source's whole band budget, the sum of its components' counts
//! times their band shares (Design note 15), and the stay share of a component is a share of its
//! own. Per source and band, the budget closes: the budget-weighted stay shares, the class weights
//! and the gone share sum to 1 to rounding ([`ClassTable::closure`]).

use std::error::Error;
use std::fmt;

use super::forms::{
    COROTATION_RATIO_NODES, CoredPowerLawParams, DiscBornRow, ESCAPE_RATIO_NODES, OldBornRow,
    at_nodes,
};
use super::kick_bins::{KINDS, KickBinShares, speed_bin_shares};
use super::marks::{
    ConditionalMarks, MARK_MASS_NODES, MassNodes, StayCategory, StayMarks, age_bin_years,
};
use super::runaway::{
    ENCOUNTER_MAX_AGE, Ejected, RELEASE_Q_RANGE, RELEASING_MIN_MASS, RunawayModel, gl16,
};
use super::{
    AGE_BINS, AgeBin, BirthSource, DisplacedClassId, DisplacedKind, GalaxyScales, SPEED_BINS,
    SPEED_EDGES, SpeedBin,
};
use crate::galaxy::fields::{Component, ComponentId};
use crate::galaxy::imf::{MASS_BAND_EDGES, MassBand};
use crate::galaxy::{Galaxy, PointLy, Population};
use crate::math;
use crate::stellar::draws::StarDraws;
use crate::stellar::remnant::{RemnantKind, StandardKickLaw};
use crate::stellar::sse::MAX_INITIAL_MASS;
use crate::stellar::{Composition, lifetime};
use crate::units::{Dex, HeliumExcess, KilometresPerSecond, SolarMasses, Years};

/// The birth sources, in the class table's order.
pub const SOURCES: [BirthSource; 6] = [
    BirthSource::ThinDisc,
    BirthSource::ThickDisc,
    BirthSource::Halo,
    BirthSource::Bulge,
    BirthSource::LongBar,
    BirthSource::NuclearDisc,
];

/// The number of classes: 56 thin-disc classes, eight for each of the five old sources, and the
/// hypervelocity class.
pub const CLASS_COUNT: usize = SPEED_BINS * AGE_BINS + 5 * SPEED_BINS + 1;

/// The kick speeds the bins span together, in `v_c`, for the classes' mean `u`: plan 15's 0.02 to
/// 6 (P15.T6.b), the range its orbits were kicked over.
pub(crate) const KICK_SPAN: (f64, f64) = (0.02, 6.0);

/// The source's place in [`SOURCES`].
#[must_use]
pub(crate) fn source_index(source: BirthSource) -> usize {
    match source {
        BirthSource::ThinDisc => 0,
        BirthSource::ThickDisc => 1,
        BirthSource::Halo => 2,
        BirthSource::Bulge => 3,
        BirthSource::LongBar => 4,
        BirthSource::NuclearDisc => 5,
    }
}

/// The source a population's remnants belong to (Design note 14: one thin-disc source).
#[must_use]
pub fn source_of(population: Population) -> BirthSource {
    match population {
        Population::YoungThinDisc | Population::OldThinDisc => BirthSource::ThinDisc,
        Population::ThickDisc => BirthSource::ThickDisc,
        Population::Halo => BirthSource::Halo,
        Population::Bulge => BirthSource::Bulge,
        Population::LongBar => BirthSource::LongBar,
        Population::NuclearDisc => BirthSource::NuclearDisc,
    }
}

/// Whether `source`'s remnants keep their own form in part (bulge, bar and nuclear disc; Design
/// note 13), rather than being retained below a quarter of the circular speed.
#[must_use]
pub(crate) fn keeps_own_form(source: BirthSource) -> bool {
    matches!(
        source,
        BirthSource::Bulge | BirthSource::LongBar | BirthSource::NuclearDisc
    )
}

/// Whether `source` holds runaways and walkaways: the thin disc and the nuclear disc's young part
/// (Design note 24).
#[must_use]
pub(crate) fn ejects(source: BirthSource) -> bool {
    matches!(source, BirthSource::ThinDisc | BirthSource::NuclearDisc)
}

/// The hypervelocity survivors' class, registered at zero weight (Design note 25).
///
/// # Panics
///
/// Never: the table has 97 classes.
#[must_use]
pub fn hypervelocity_class() -> DisplacedClassId {
    ClassKey::Hypervelocity.id()
}

/// The rows of a form table, before [`FormTable::new`] checks them: `tables::displaced_forms` in
/// one value, so that a test can pass another.
#[derive(Debug, Clone, PartialEq)]
pub struct FormRows {
    /// The thin disc's classes, `[speed bin][age bin]`.
    pub disc_born: [[DiscBornRow; AGE_BINS]; SPEED_BINS],
    /// The thick disc's classes by speed bin.
    pub thick_disc: [OldBornRow; SPEED_BINS],
    /// The halo's.
    pub halo: [OldBornRow; SPEED_BINS],
    /// The bulge's.
    pub bulge: [OldBornRow; SPEED_BINS],
    /// The long bar's.
    pub bar: [OldBornRow; SPEED_BINS],
    /// The nuclear disc's, its speeds against its own circular speed.
    pub nuclear_disc: [OldBornRow; SPEED_BINS],
    /// The hypervelocity survivors' form.
    pub hypervelocity: CoredPowerLawParams,
}

/// Plan 15's form table as the class table reads it, its shares checked once
/// ([`FormTable::new`]).
#[derive(Debug, Clone, PartialEq)]
pub struct FormTable {
    rows: FormRows,
}

impl FormTable {
    /// The table of `rows`.
    ///
    /// # Errors
    ///
    /// - [`BuildFormTableError::ShareOutOfRange`] if an in-cube, unbound or own-form share lies
    ///   outside 0–1 or is not a number;
    /// - [`BuildFormTableError::SharesExceedOne`] if a row's bound and unbound shares inside the
    ///   cube sum past 1 at a node.
    pub fn new(rows: FormRows) -> Result<Self, BuildFormTableError> {
        rows.validate()?;
        Ok(Self { rows })
    }

    /// The committed table, `tables::displaced_forms` (plan 15, P15.T6).
    ///
    /// While the table is provisional (`tables::MANIFEST`) its values are fitted to the orbit
    /// run's smoke histograms and are not physical; nothing generated reads the class table
    /// until P08.T12 switches it on with the production table.
    ///
    /// # Panics
    ///
    /// If the committed table holds a share outside 0–1, which its emitter and this crate's
    /// tests rule out.
    #[must_use]
    pub fn committed() -> Self {
        use crate::tables::displaced_forms as t;
        Self::new(FormRows {
            disc_born: t::DISC_BORN,
            thick_disc: t::THICK_DISC_BORN,
            halo: t::HALO_BORN,
            bulge: t::BULGE_BORN,
            bar: t::BAR_BORN,
            nuclear_disc: t::NUCLEAR_DISC_BORN,
            hypervelocity: t::HYPERVELOCITY,
        })
        .expect("the committed form table's shares are fractions")
    }

    /// The rows.
    #[must_use]
    pub fn rows(&self) -> &FormRows {
        &self.rows
    }

    /// `(in_cube, unbound_in_cube, own_share)` of the class of `source` in `speed` and `age` (the
    /// thin disc's), the node arrays read at `scales`' escape and corotation ratios.
    fn shares(
        &self,
        source: BirthSource,
        speed: usize,
        age: usize,
        scales: &GalaxyScales,
    ) -> (f64, f64, f64) {
        self.rows.shares(source, speed, age, scales)
    }
}

impl FormRows {
    /// The old source's rows; `None` for the thin disc.
    #[must_use]
    pub fn old(&self, source: BirthSource) -> Option<&[OldBornRow; SPEED_BINS]> {
        match source {
            BirthSource::ThinDisc => None,
            BirthSource::ThickDisc => Some(&self.thick_disc),
            BirthSource::Halo => Some(&self.halo),
            BirthSource::Bulge => Some(&self.bulge),
            BirthSource::LongBar => Some(&self.bar),
            BirthSource::NuclearDisc => Some(&self.nuclear_disc),
        }
    }

    /// `(in_cube, unbound_in_cube, own_share)` of the class of `source` in `speed` and `age` (the
    /// thin disc's), the node arrays read at `scales`' escape and corotation ratios.
    fn shares(
        &self,
        source: BirthSource,
        speed: usize,
        age: usize,
        scales: &GalaxyScales,
    ) -> (f64, f64, f64) {
        let escape = scales.escape_ratio();
        match self.old(source) {
            None => {
                let row = &self.disc_born[speed][age];
                (
                    at_nodes(&ESCAPE_RATIO_NODES, &row.in_cube, escape),
                    at_nodes(&ESCAPE_RATIO_NODES, &row.unbound_in_cube, escape),
                    0.0,
                )
            }
            Some(rows) => {
                let row = &rows[speed];
                let own = if keeps_own_form(source) {
                    at_nodes(
                        &COROTATION_RATIO_NODES,
                        &row.own_share,
                        scales.corotation_ratio(),
                    )
                } else {
                    0.0
                };
                (
                    at_nodes(&ESCAPE_RATIO_NODES, &row.in_cube, escape),
                    at_nodes(&ESCAPE_RATIO_NODES, &row.unbound_in_cube, escape),
                    own,
                )
            }
        }
    }

    /// Checks that every share lies in 0–1 and the bound and unbound shares inside the cube sum to
    /// at most 1.
    fn validate(&self) -> Result<(), BuildFormTableError> {
        let check =
            |source: BirthSource, in_cube: &[f64; 3], unbound: &[f64; 3], own: &[f64; 3]| {
                let fraction = |v: f64| (0.0..=1.0).contains(&v);
                if !in_cube
                    .iter()
                    .chain(unbound)
                    .chain(own)
                    .all(|&v| fraction(v))
                {
                    return Err(BuildFormTableError::ShareOutOfRange { source });
                }
                if in_cube
                    .iter()
                    .zip(unbound)
                    .any(|(a, b)| a + b > 1.0 + 1e-12)
                {
                    return Err(BuildFormTableError::SharesExceedOne { source });
                }
                Ok(())
            };
        for row in self.disc_born.iter().flatten() {
            check(
                BirthSource::ThinDisc,
                &row.in_cube,
                &row.unbound_in_cube,
                &[0.0; 3],
            )?;
        }
        for &source in &SOURCES[1..] {
            for row in self.old(source).into_iter().flatten() {
                check(source, &row.in_cube, &row.unbound_in_cube, &row.own_share)?;
            }
        }
        Ok(())
    }
}

/// A form table's rows cannot be used: a share is not a fraction, or a row's shares inside the cube
/// sum past 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildFormTableError {
    /// An in-cube, unbound or own-form share of one of `source`'s rows lies outside 0–1.
    ShareOutOfRange {
        /// The source whose row is wrong.
        source: BirthSource,
    },
    /// A row of `source` holds bound and unbound shares inside the cube summing past 1.
    SharesExceedOne {
        /// The source whose row is wrong.
        source: BirthSource,
    },
}

impl BuildFormTableError {
    /// The source whose row is wrong.
    #[must_use]
    pub fn source(&self) -> BirthSource {
        match *self {
            Self::ShareOutOfRange { source } | Self::SharesExceedOne { source } => source,
        }
    }
}

impl fmt::Display for BuildFormTableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ShareOutOfRange { source } => {
                write!(
                    f,
                    "a {source:?} row of the form table holds a share outside 0–1"
                )
            }
            Self::SharesExceedOne { source } => write!(
                f,
                "a {source:?} row of the form table holds shares inside the cube summing past 1"
            ),
        }
    }
}

impl Error for BuildFormTableError {}

/// One of the five old sources, whose classes have speed bins only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum OldSource {
    /// The thick disc.
    ThickDisc,
    /// The stellar halo.
    Halo,
    /// The boxy bulge.
    Bulge,
    /// The long bar.
    LongBar,
    /// The nuclear disc.
    NuclearDisc,
}

impl OldSource {
    /// The birth source.
    #[must_use]
    pub fn source(self) -> BirthSource {
        match self {
            Self::ThickDisc => BirthSource::ThickDisc,
            Self::Halo => BirthSource::Halo,
            Self::Bulge => BirthSource::Bulge,
            Self::LongBar => BirthSource::LongBar,
            Self::NuclearDisc => BirthSource::NuclearDisc,
        }
    }

    /// The old source of `source`, or `None` for the thin disc.
    #[must_use]
    pub fn of(source: BirthSource) -> Option<Self> {
        match source {
            BirthSource::ThinDisc => None,
            BirthSource::ThickDisc => Some(Self::ThickDisc),
            BirthSource::Halo => Some(Self::Halo),
            BirthSource::Bulge => Some(Self::Bulge),
            BirthSource::LongBar => Some(Self::LongBar),
            BirthSource::NuclearDisc => Some(Self::NuclearDisc),
        }
    }
}

/// What a class is: a thin-disc class by speed and age bin, an old source's by speed bin, or the
/// hypervelocity survivors. Every class of the table's remnants (and, for the thin disc and the
/// nuclear disc, its runaways and walkaways) is one of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ClassKey {
    /// A thin-disc class.
    Thin {
        /// The speed bin of the kick or ejection.
        speed: SpeedBin,
        /// The age bin of the time since death or ejection.
        age: AgeBin,
    },
    /// An old source's class.
    Old {
        /// The source.
        source: OldSource,
        /// The speed bin of the kick or ejection.
        speed: SpeedBin,
    },
    /// The ancient Type Ia survivors, at zero weight (Design note 25).
    Hypervelocity,
}

impl ClassKey {
    /// The class's place in the table: the thin disc's 56 by speed then age, the old sources' eight
    /// each, then the hypervelocity survivors.
    ///
    /// # Panics
    ///
    /// Never: the table has 97 classes.
    #[must_use]
    pub fn id(self) -> DisplacedClassId {
        let index = match self {
            Self::Thin { speed, age } => speed.index() * AGE_BINS + age.index(),
            Self::Old { source, speed } => {
                SPEED_BINS * AGE_BINS
                    + (source_index(source.source()) - 1) * SPEED_BINS
                    + speed.index()
            }
            Self::Hypervelocity => CLASS_COUNT - 1,
        };
        DisplacedClassId::new(u16::try_from(index).expect("under 97 classes"))
    }

    /// The birth source, or `None` for the hypervelocity survivors.
    #[must_use]
    pub fn source(self) -> Option<BirthSource> {
        match self {
            Self::Thin { .. } => Some(BirthSource::ThinDisc),
            Self::Old { source, .. } => Some(source.source()),
            Self::Hypervelocity => None,
        }
    }

    /// The key of `source`'s class in `speed` and, for the thin disc, `age` (ignored for the old
    /// sources).
    #[must_use]
    pub fn of(source: BirthSource, speed: SpeedBin, age: AgeBin) -> Self {
        match OldSource::of(source) {
            None => Self::Thin { speed, age },
            Some(source) => Self::Old { source, speed },
        }
    }
}

/// One class of the table: its key, its weights in layers D and E, its `⟨uτ⟩` and its marks.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassEntry {
    key: ClassKey,
    /// `[D, E]`, shares of the source's band budget.
    weight: [f64; 2],
    mean_ut: f64,
    marks: [ConditionalMarks; 2],
    /// `Σ weight × u × τ` and `Σ weight` over layer E's members and D's runaways, for `⟨uτ⟩`.
    ut_sums: (f64, f64),
}

impl ClassEntry {
    /// Sets `⟨uτ⟩` from the sums.
    fn finish(&mut self) {
        let (ut, weight) = self.ut_sums;
        self.mean_ut = if weight > 0.0 { ut / weight } else { 0.0 };
    }

    /// What the class is.
    #[must_use]
    pub fn key(&self) -> ClassKey {
        self.key
    }

    /// The class's weight in layer D (`band` D) or E, a share of its source's band budget; 0 in
    /// the other bands.
    #[must_use]
    pub fn weight(&self, band: MassBand) -> f64 {
        band_slot(band).map_or(0.0, |b| self.weight[b])
    }

    /// The members' mean kick speed times time since death, `⟨uτ⟩` in `R_d` (Design note 21):
    /// the speed bin's log-mean speed for remnants (their origin bin's for the unbound), the
    /// mean speed in the bin for runaways, times the mean time since death or ejection in `R_d ÷
    /// v_c`.
    #[must_use]
    pub fn mean_ut(&self) -> f64 {
        self.mean_ut
    }

    /// The class's marks in `band` (D or E).
    ///
    /// # Panics
    ///
    /// If `band` is not D or E.
    #[must_use]
    pub fn marks(&self, band: MassBand) -> &ConditionalMarks {
        &self.marks[band_slot(band).expect("displaced classes live in layers D and E")]
    }
}

/// The slot of band D (0) or E (1).
fn band_slot(band: MassBand) -> Option<usize> {
    match band {
        MassBand::D => Some(0),
        MassBand::E => Some(1),
        MassBand::A | MassBand::B | MassBand::C | MassBand::BrownDwarf | MassBand::RoguePlanet => {
            None
        }
    }
}

/// One field component's part of the table.
#[derive(Debug, Clone, PartialEq)]
struct ComponentEntry {
    source: BirthSource,
    /// `[D, E]`, systems.
    budget: [f64; 2],
    /// `[D, E]`, shares of the component's band budget.
    stay: [f64; 2],
    /// `[D, E]`: the share alive, runaways and walkaways included.
    alive: [f64; 2],
    stay_marks: StayMarks,
}

/// One source's totals, shares of its band-E budget unless named otherwise.
#[derive(Debug, Clone, PartialEq)]
struct SourceEntry {
    composition: Composition,
    /// `[D, E]`, systems.
    budget: [f64; 2],
    /// `[D, E]`.
    gone: [f64; 2],
    /// By remnant kind in [`KINDS`] order: every remnant, the retained, the bound inside the cube
    /// (retained included), the unbound inside the cube.
    remnants: [[f64; 4]; 3],
    /// The lifetimes at the band's mass nodes at the reference composition, `[D, E]`, years.
    lifetimes: [[f64; MARK_MASS_NODES]; 2],
    /// The companion-stripped share the kick bins used at band E's nodes.
    stripped: [f64; MARK_MASS_NODES],
}

/// The class table of one galaxy (module documentation): stay shares per field component, class
/// weights, gone shares and the conditional marks.
///
/// # Examples
///
/// ```no_run
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::displaced::BirthSource;
/// use hyperion_sim::galaxy::displaced::class_table::{ClassTable, FormTable};
/// use hyperion_sim::galaxy::imf::MassBand;
///
/// // Tens of seconds, nearly all of it the kick law's quadrature.
/// let galaxy = Galaxy::new(Seed::new(1));
/// let table = ClassTable::build(&galaxy, &FormTable::committed());
/// // Every source's layer-E budget is accounted for.
/// for source in [BirthSource::ThinDisc, BirthSource::Bulge] {
///     assert!((table.closure(MassBand::E, source) - 1.0).abs() < 1e-12);
/// }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ClassTable {
    scales: GalaxyScales,
    nodes: [MassNodes; 2],
    sources: Vec<SourceEntry>,
    components: Vec<ComponentEntry>,
    classes: Vec<ClassEntry>,
}

/// The log-mean speed of kicks in bin `s`, in units of the speed scale.
fn bin_mean_speed(s: usize) -> f64 {
    let lo = if s == 0 {
        KICK_SPAN.0
    } else {
        SPEED_EDGES[s - 1]
    };
    let hi = SPEED_EDGES.get(s).copied().unwrap_or(KICK_SPAN.1);
    (hi - lo) / math::ln(hi / lo)
}

/// The speed scale of `source`'s kicks and ejections, km/s: `v_c`, or the nuclear disc's own
/// circular speed for its classes (Design note 13).
fn source_speed(source: BirthSource, scales: &GalaxyScales) -> f64 {
    match source {
        BirthSource::NuclearDisc => scales.nuclear_v_c().value(),
        BirthSource::ThinDisc
        | BirthSource::ThickDisc
        | BirthSource::Halo
        | BirthSource::Bulge
        | BirthSource::LongBar => scales.v_c().value(),
    }
}

/// The reference site of `source`'s metallicity: 3 `R_d` in the plane, or the centre.
fn reference_site(source: BirthSource, scales: &GalaxyScales) -> PointLy {
    if keeps_own_form(source) {
        PointLy::new(0.0, 0.0, 0.0)
    } else {
        PointLy::new(3.0 * scales.r_d().value(), 0.0, 0.0)
    }
}

/// A lifetime at the median star's draws, masses above the tracks' 100 M☉ taken at it.
fn reference_lifetime_of(m: f64, comp: &Composition) -> f64 {
    let m = SolarMasses::new(m.min(MAX_INITIAL_MASS.value()));
    lifetime(m, comp, &StarDraws::median()).value()
}

/// Linear interpolation of `values` at the log-spaced `masses`, in log mass, clamped.
fn interpolate_log(
    masses: &[f64; MARK_MASS_NODES],
    values: &[f64; MARK_MASS_NODES],
    m: f64,
) -> f64 {
    let m = m.clamp(masses[0], masses[MARK_MASS_NODES - 1]);
    let i = masses
        .partition_point(|&x| x <= m)
        .saturating_sub(1)
        .min(MARK_MASS_NODES - 2);
    let t = math::ln(m / masses[i]) / math::ln(masses[i + 1] / masses[i]);
    values[i] + t * (values[i + 1] - values[i])
}

/// The born systems of `ages` younger than `age` years: 0 below 0, 1 at infinity.
fn born_below(component: &Component, age: f64) -> f64 {
    if age.is_infinite() {
        1.0
    } else {
        component.ages().born_cdf(Years::new(age))
    }
}

/// The mean time since death in `[lo, hi]` years of a star of lifetime `life` in `component`, by a
/// 16-node rule on the ages' density; the interval's middle if it holds none.
fn mean_time_since_death(component: &Component, life: f64, lo: f64, hi: f64) -> f64 {
    let hi = hi.min(component.ages().max().value() - life);
    if hi <= lo {
        return lo;
    }
    let (mut m0, mut m1) = (0.0, 0.0);
    for (s, w) in gl16(lo, hi) {
        let p = component.ages().pdf(Years::new(life + s));
        m0 += w * p;
        m1 += w * p * s;
    }
    if m0 > 0.0 {
        m1 / m0
    } else {
        f64::midpoint(lo, hi)
    }
}

/// The share of a living star of lifetime `life` (years) in `component` that was ejected at the
/// ages `ejection` (years, with their weights summing to 1) and whose time since ejection lies in
/// `[lo, hi)` years.
fn ejected_in(component: &Component, life: f64, ejection: &[(f64, f64)], lo: f64, hi: f64) -> f64 {
    ejection
        .iter()
        .filter(|&&(t, _)| t < life)
        .map(|&(t, w)| {
            let a = (t + lo).min(life);
            let b = (t + hi).min(life);
            w * (born_below(component, b) - born_below(component, a)).max(0.0)
        })
        .sum()
}

/// One mass node of one field component in one band: what the quadrature reads there.
struct Node<'a> {
    component: &'a Component,
    source: BirthSource,
    /// The component's place among its source's.
    within: usize,
    /// 0 for layer D, 1 for E.
    band: usize,
    /// The node's index.
    index: usize,
    /// The node's mass, M☉.
    m: f64,
    /// Its quadrature weight.
    weight: f64,
    /// The mass function's density there, per M☉.
    pdf: f64,
    /// The lifetime at the source's reference composition, years.
    life: f64,
    /// The component's band budget over its source's.
    relative: f64,
    /// The source's speed scale.
    v_ref: KilometresPerSecond,
}

/// What the quadrature adds up: the classes for the whole galaxy, one component's stay terms and
/// one source's remnant terms at a time.
struct Accumulator<'a> {
    forms: &'a FormTable,
    scales: &'a GalaxyScales,
    model: RunawayModel,
    classes: Vec<ClassEntry>,
    /// The component's stay shares, `[D, E]`.
    stay: [f64; 2],
    /// The component's alive shares, `[D, E]`.
    alive: [f64; 2],
    stay_marks: StayMarks,
    /// The source's gone share of layer E.
    gone: f64,
    /// The source's remnants by kind: every one, retained, bound inside, unbound inside.
    remnants: [[f64; 4]; 3],
}

impl Accumulator<'_> {
    /// Adds `part` of a star at `at` to class `class` as `kind`, its kick or ejection in speed bin
    /// `origin` at a mean `u τ` of `ut`.
    fn place(
        &mut self,
        at: &Node<'_>,
        class: usize,
        kind: DisplacedKind,
        origin: usize,
        part: f64,
        ut: f64,
    ) {
        if part <= 0.0 {
            return;
        }
        let entry = &mut self.classes[class];
        let weight = at.relative * at.weight * part;
        entry.weight[at.band] += weight;
        entry.marks[at.band].add(
            kind,
            at.within,
            at.index,
            origin,
            weight,
            at.relative * at.pdf * part,
        );
        entry.ut_sums.0 += weight * ut;
        entry.ut_sums.1 += weight;
    }

    /// The runaways and walkaways among the living at `at`, into their classes: the share of the
    /// component's stars there that they take. `lifetimes_e` are band E's node lifetimes at
    /// `masses_e`, from which a releasing companion's is read.
    fn eject(
        &mut self,
        at: &Node<'_>,
        lifetimes_e: &[f64; MARK_MASS_NODES],
        masses_e: &[f64; MARK_MASS_NODES],
    ) -> f64 {
        let tau_unit = self.scales.tau_unit();
        let mut total = 0.0;
        for kind in [Ejected::Runaway, Ejected::Walkaway] {
            let share = match kind {
                Ejected::Runaway => self.model.runaway_share(SolarMasses::new(at.m)),
                Ejected::Walkaway => self.model.walkaway_share(SolarMasses::new(at.m)),
            };
            if share <= 0.0 {
                continue;
            }
            let ejection = ejection_ages(self.model, kind, at.m, lifetimes_e, masses_e);
            let speeds = self.model.speed_bins(kind, at.v_ref);
            let marked = match kind {
                Ejected::Runaway => DisplacedKind::Runaway,
                Ejected::Walkaway => DisplacedKind::Walkaway,
            };
            for age in 0..AGE_BINS {
                let (lo, hi) = age_bin_years(age_bin(age), tau_unit);
                let hi = hi.map_or(f64::INFINITY, Years::value);
                let ejected = ejected_in(at.component, at.life, &ejection, lo.value(), hi);
                if ejected <= 0.0 {
                    continue;
                }
                let tau_mid =
                    f64::midpoint(lo.value(), hi.min(at.life).max(lo.value())) / tau_unit.value();
                for (bin, &p) in speeds.iter().enumerate() {
                    let part = share * p * ejected;
                    if part <= 0.0 {
                        continue;
                    }
                    total += part;
                    let u = self.model.mean_speed_in_bin(kind, speed_bin(bin), at.v_ref);
                    let class = remnant_class(at.source, bin, age);
                    self.place(at, class, marked, bin, part, u * tau_mid);
                }
            }
        }
        total
    }

    /// The dead of layer E at `at`, whose alive share is `alive`, under the kick shares `kick`:
    /// retained, into their classes, or gone (module documentation).
    fn dead(&mut self, at: &Node<'_>, alive: f64, kick: &KickBinShares) {
        let tau_unit = self.scales.tau_unit();
        self.gone += at.relative * at.weight * (1.0 - alive) * kick.no_remnant();
        for age in 0..AGE_BINS {
            let (lo, hi) = age_bin_years(age_bin(age), tau_unit);
            let hi = hi.map_or(f64::INFINITY, Years::value);
            let in_bin = born_below(at.component, at.life + hi)
                - born_below(at.component, at.life + lo.value());
            if in_bin <= 0.0 {
                continue;
            }
            let tau =
                mean_time_since_death(at.component, at.life, lo.value(), hi) / tau_unit.value();
            for (k, &kind) in KINDS.iter().enumerate() {
                for (bin, &p) in kick.bins(kind).iter().enumerate() {
                    self.remnant(at, k, bin, age, in_bin * p, tau);
                }
            }
        }
    }

    /// One remnant share `part` of kind number `k` in speed bin `bin` and age bin `age`, at a mean
    /// time since death of `tau`.
    fn remnant(&mut self, at: &Node<'_>, k: usize, bin: usize, age: usize, part: f64, tau: f64) {
        if part <= 0.0 {
            return;
        }
        let of_source = at.relative * at.weight;
        self.remnants[k][0] += of_source * part;
        if !keeps_own_form(at.source) && bin == 0 {
            self.stay[1] += at.weight * part;
            self.stay_marks.add(
                StayCategory::Retained(speed_bin(0)),
                at.index,
                at.weight * part,
                at.pdf * part,
            );
            self.remnants[k][1] += of_source * part;
            self.remnants[k][2] += of_source * part;
            return;
        }
        let row_age = if at.source == BirthSource::ThinDisc {
            age
        } else {
            0
        };
        let (in_cube, unbound, own) = self.forms.shares(at.source, bin, row_age, self.scales);
        let retained = part * in_cube * own;
        self.stay[1] += at.weight * retained;
        self.stay_marks.add(
            StayCategory::Retained(speed_bin(bin)),
            at.index,
            at.weight * retained,
            at.pdf * retained,
        );
        self.remnants[k][1] += of_source * retained;
        self.remnants[k][2] += of_source * part * in_cube;
        self.remnants[k][3] += of_source * part * unbound;
        let ut = bin_mean_speed(bin) * tau;
        let own_class = remnant_class(at.source, bin, age);
        let fastest = remnant_class(at.source, SPEED_BINS - 1, age);
        self.place(
            at,
            own_class,
            DisplacedKind::Remnant,
            bin,
            part * in_cube * (1.0 - own),
            ut,
        );
        self.place(at, fastest, DisplacedKind::Remnant, bin, part * unbound, ut);
        self.gone += of_source * part * (1.0 - in_cube - unbound);
    }
}

/// Each source's components, in the fields' order.
fn members_of(galaxy: &Galaxy) -> Vec<Vec<ComponentId>> {
    let fields = galaxy.fields();
    let mut members: Vec<Vec<ComponentId>> = vec![Vec::new(); SOURCES.len()];
    for id in fields.component_ids() {
        members[source_index(source_of(fields.component(id).population()))].push(id);
    }
    members
}

/// Component `c`'s budgets in layers D and E, systems.
fn budget_of(galaxy: &Galaxy, c: &Component) -> [f64; 2] {
    [MassBand::D, MassBand::E].map(|band| c.count() * galaxy.shares().component_share(band, c))
}

/// Each source's reference composition, budgets and node lifetimes, and the kick shares at band
/// E's nodes: one quadrature per distinct composition and speed scale, since the bulge and the bar
/// share both and the kick law's quadrature is most of the build's cost.
fn source_entries(
    galaxy: &Galaxy,
    members: &[Vec<ComponentId>],
    scales: &GalaxyScales,
    nodes: &[MassNodes; 2],
) -> (Vec<SourceEntry>, Vec<Vec<KickBinShares>>, Vec<usize>) {
    let fields = galaxy.fields();
    let law = StandardKickLaw::default();
    let mut sources = Vec::with_capacity(SOURCES.len());
    let mut kicks: Vec<(f64, f64, Vec<KickBinShares>)> = Vec::with_capacity(SOURCES.len());
    let mut kick_of = Vec::with_capacity(SOURCES.len());
    for (s, &source) in SOURCES.iter().enumerate() {
        let site = reference_site(source, scales);
        let (mut iron, mut count) = (0.0, 0.0);
        let mut budget = [0.0; 2];
        for &id in &members[s] {
            let c = fields.component(id);
            iron += c.count() * c.metallicity(&site, c.ages().mean()).mean().value();
            count += c.count();
            let b = budget_of(galaxy, c);
            budget[0] += b[0];
            budget[1] += b[1];
        }
        let fe_h = if count > 0.0 { iron / count } else { 0.0 };
        let composition = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
        let lifetimes = [0, 1].map(|b| {
            if b == 1 || ejects(source) {
                nodes[b]
                    .masses()
                    .map(|m| reference_lifetime_of(m, &composition))
            } else {
                [0.0; MARK_MASS_NODES]
            }
        });
        let v_ref = source_speed(source, scales);
        let known = kicks
            .iter()
            .position(|(fe, v, _)| fe.total_cmp(&fe_h).is_eq() && v.total_cmp(&v_ref).is_eq());
        let index = known.unwrap_or_else(|| {
            let k = nodes[1]
                .masses()
                .iter()
                .map(|&m| speed_bin_shares(&law, SolarMasses::new(m), &composition, scales, source))
                .collect();
            kicks.push((fe_h, v_ref, k));
            kicks.len() - 1
        });
        kick_of.push(index);
        sources.push(SourceEntry {
            composition,
            budget,
            gone: [0.0; 2],
            remnants: [[0.0; 4]; 3],
            lifetimes,
            stripped: core::array::from_fn(|i| kicks[index].2[i].stripped_share()),
        });
    }
    (
        sources,
        kicks.into_iter().map(|(_, _, k)| k).collect(),
        kick_of,
    )
}

/// Every class, empty, its marks over `nodes` and its source's components.
fn empty_classes(members: &[Vec<ComponentId>], nodes: &[MassNodes; 2]) -> Vec<ClassEntry> {
    (0..CLASS_COUNT)
        .map(|index| {
            let key = key_of(index);
            let components = key
                .source()
                .map_or_else(Vec::new, |source| members[source_index(source)].clone());
            ClassEntry {
                key,
                weight: [0.0; 2],
                mean_ut: 0.0,
                marks: [0, 1]
                    .map(|b| ConditionalMarks::empty(nodes[b].masses(), components.clone())),
                ut_sums: (0.0, 0.0),
            }
        })
        .collect()
}

impl ClassTable {
    /// The class table of `galaxy` under plan 15's form table `forms`: the once-per-galaxy
    /// quadrature of the module documentation.
    ///
    #[must_use]
    pub fn build(galaxy: &Galaxy, forms: &FormTable) -> Self {
        let scales = GalaxyScales::new(galaxy.params(), galaxy.potential());
        let mf = galaxy.mass_function();
        let nodes = [
            MassNodes::of(MassBand::D, mf),
            MassNodes::of(MassBand::E, mf),
        ];
        let fields = galaxy.fields();
        let members = members_of(galaxy);
        let (mut sources, kicks, kick_of) = source_entries(galaxy, &members, &scales, &nodes);
        let mut acc = Accumulator {
            forms,
            scales: &scales,
            model: RunawayModel,
            classes: empty_classes(&members, &nodes),
            stay: [0.0; 2],
            alive: [0.0; 2],
            stay_marks: StayMarks::empty(nodes[1].masses()),
            gone: 0.0,
            remnants: [[0.0; 4]; 3],
        };
        let mut components = Vec::with_capacity(fields.components().len());
        for (id, component) in fields.component_ids().zip(fields.components()) {
            let source = source_of(component.population());
            let s_index = source_index(source);
            let within = members[s_index].iter().take_while(|&&m| m != id).count();
            let budget = budget_of(galaxy, component);
            let src = &sources[s_index];
            let relative = [0, 1].map(|b| budget[b] / src.budget[b].max(f64::MIN_POSITIVE));
            acc.stay = [0.0; 2];
            acc.alive = [0.0; 2];
            acc.stay_marks = StayMarks::empty(nodes[1].masses());
            acc.gone = src.gone[1];
            acc.remnants = src.remnants;
            for (band, band_nodes) in nodes.iter().enumerate() {
                let source_kicks = &kicks[kick_of[s_index]];
                for (node, kick) in source_kicks.iter().enumerate() {
                    let at = Node {
                        component,
                        source,
                        within,
                        band,
                        index: node,
                        m: band_nodes.masses()[node],
                        weight: band_nodes.weights()[node],
                        pdf: band_nodes.pdf()[node],
                        life: src.lifetimes[band][node],
                        relative: relative[band],
                        v_ref: KilometresPerSecond::new(source_speed(source, &scales)),
                    };
                    // Layer D outside the ejecting sources: every star stays.
                    let alive = if band == 1 || ejects(source) {
                        born_below(component, at.life)
                    } else {
                        1.0
                    };
                    let ejected = if ejects(source) {
                        acc.eject(&at, &src.lifetimes[1], nodes[1].masses())
                    } else {
                        0.0
                    };
                    let staying = alive - ejected;
                    acc.stay[band] += at.weight * staying;
                    acc.alive[band] += at.weight * alive;
                    if band == 0 {
                        // Layer D's dead are white dwarfs, which stay where they are.
                        acc.stay[0] += at.weight * (1.0 - alive);
                    } else {
                        acc.stay_marks.add(
                            StayCategory::Alive,
                            node,
                            at.weight * staying,
                            at.pdf * staying,
                        );
                        acc.dead(&at, alive, kick);
                    }
                }
            }
            let src = &mut sources[s_index];
            src.gone[1] = acc.gone;
            src.remnants = acc.remnants;
            components.push(ComponentEntry {
                source,
                budget,
                stay: acc.stay,
                alive: acc.alive,
                stay_marks: std::mem::replace(
                    &mut acc.stay_marks,
                    StayMarks::empty(nodes[1].masses()),
                ),
            });
        }
        let mut classes = acc.classes;
        classes.iter_mut().for_each(ClassEntry::finish);
        Self {
            scales,
            nodes,
            sources,
            components,
            classes,
        }
    }

    /// The galaxy's scales the table was built at.
    #[must_use]
    pub fn scales(&self) -> &GalaxyScales {
        &self.scales
    }

    /// The mass nodes of `band` (D or E).
    ///
    /// # Panics
    ///
    /// If `band` is not D or E.
    #[must_use]
    pub fn nodes(&self, band: MassBand) -> &MassNodes {
        &self.nodes[band_slot(band).expect("the class table has bands D and E")]
    }

    /// The classes, in [`ClassKey::id`]'s order.
    #[must_use]
    pub fn classes(&self) -> &[ClassEntry] {
        &self.classes
    }

    /// The share of component `c`'s `band` budget that stays in the field: alive (less the
    /// runaways and walkaways) and retained. 1 outside bands D and E.
    ///
    /// # Panics
    ///
    /// If `c` is not a component of the galaxy the table was built for.
    #[must_use]
    pub fn stay_share(&self, band: MassBand, c: ComponentId) -> f64 {
        band_slot(band).map_or(1.0, |b| self.components[c.index()].stay[b])
    }

    /// The share of component `c`'s `band` budget alive at the epoch, runaways and walkaways
    /// included; in layer D for the ejecting sources only (1 for the others). 1 outside bands D
    /// and E.
    ///
    /// # Panics
    ///
    /// If `c` is not a component of the galaxy the table was built for.
    #[must_use]
    pub fn alive_share(&self, band: MassBand, c: ComponentId) -> f64 {
        band_slot(band).map_or(1.0, |b| self.components[c.index()].alive[b])
    }

    /// Class `id`'s weight in `band`, a share of its source's `band` budget (Design note 15).
    #[must_use]
    pub fn class_weight(&self, band: MassBand, id: DisplacedClassId) -> f64 {
        self.classes[id.index()].weight(band)
    }

    /// Class `id`'s expected systems in `band` over the whole galaxy: its weight times its
    /// source's band budget.
    #[must_use]
    pub fn class_count(&self, band: MassBand, id: DisplacedClassId) -> f64 {
        let class = &self.classes[id.index()];
        match (class.key.source(), band_slot(band)) {
            (Some(source), Some(b)) => {
                class.weight[b] * self.sources[source_index(source)].budget[b]
            }
            (None, _) | (_, None) => 0.0,
        }
    }

    /// `source`'s band budget, systems: its components' counts times their band shares.
    #[must_use]
    pub fn source_budget(&self, band: MassBand, source: BirthSource) -> f64 {
        band_slot(band).map_or(0.0, |b| self.sources[source_index(source)].budget[b])
    }

    /// The share of `source`'s layer-E budget that is gone: unbound or bound beyond the cube, or
    /// destroyed with no remnant.
    #[must_use]
    pub fn gone_share(&self, source: BirthSource) -> f64 {
        self.sources[source_index(source)].gone[1]
    }

    /// The share of `source`'s layer-E remnants of `kind` that is unbound and still inside the
    /// cube (plan 15's `unbound_in_cube`). 0 for [`RemnantKind::None`].
    ///
    /// There is no `unbound_share` of all the unbound yet (P08.T14.4's 13–14% of neutron stars):
    /// plan 15's table gives only the unbound still inside the cube, and those beyond it sit in
    /// the gone share with the bound beyond it. The total needs the kick law against the local
    /// escape speed at the birth sites, which P08.T14 adds.
    #[must_use]
    pub fn unbound_in_cube_share(&self, source: BirthSource, kind: RemnantKind) -> f64 {
        self.remnant_share(source, kind, 3)
    }

    /// The share of `source`'s layer-E remnants of `kind` inside the root cube: retained, bound
    /// and unbound. 0 for [`RemnantKind::None`].
    #[must_use]
    pub fn in_cube_share(&self, source: BirthSource, kind: RemnantKind) -> f64 {
        self.remnant_share(source, kind, 2) + self.remnant_share(source, kind, 3)
    }

    /// The share of `source`'s layer-E remnants of `kind` that is retained in the field.
    #[must_use]
    pub fn retained_share(&self, source: BirthSource, kind: RemnantKind) -> f64 {
        self.remnant_share(source, kind, 1)
    }

    /// Column `column` of `source`'s remnants of `kind` over the column of every remnant.
    fn remnant_share(&self, source: BirthSource, kind: RemnantKind, column: usize) -> f64 {
        let Some(k) = KINDS.iter().position(|&x| x == kind) else {
            return 0.0;
        };
        let row = &self.sources[source_index(source)].remnants[k];
        if row[0] > 0.0 {
            row[column] / row[0]
        } else {
            0.0
        }
    }

    /// The companion-stripped share the kick bins used for a star of `source` of initial mass `m`
    /// (M☉, layer E's band), linear in log mass between the nodes: plan 11's agreement (P08.T8.a).
    #[must_use]
    pub fn stripped_share_used(&self, source: BirthSource, m: SolarMasses) -> f64 {
        interpolate_log(
            self.nodes[1].masses(),
            &self.sources[source_index(source)].stripped,
            m.value(),
        )
    }

    /// `source`'s reference composition, at which its lifetimes and kicks are taken.
    #[must_use]
    pub fn reference_composition(&self, source: BirthSource) -> Composition {
        self.sources[source_index(source)].composition
    }

    /// The lifetime of a star of `source` of initial mass `m` in `band` (D or E) at its reference
    /// composition and the median draws, linear in log mass between the nodes, years; for layer
    /// D, only for the sources that eject runaways (0 otherwise).
    ///
    /// # Panics
    ///
    /// If `band` is not D or E.
    #[must_use]
    pub fn reference_lifetime(&self, source: BirthSource, band: MassBand, m: SolarMasses) -> Years {
        let b = band_slot(band).expect("the class table has bands D and E");
        Years::new(interpolate_log(
            self.nodes[b].masses(),
            &self.sources[source_index(source)].lifetimes[b],
            m.value(),
        ))
    }

    /// Class `id`'s marks in `band` (D or E).
    ///
    /// # Panics
    ///
    /// If `band` is not D or E.
    #[must_use]
    pub fn marks(&self, band: MassBand, id: DisplacedClassId) -> &ConditionalMarks {
        self.classes[id.index()].marks(band)
    }

    /// Field component `c`'s layer-E marks: alive and retained by speed bin.
    #[must_use]
    pub fn stay_marks(&self, c: ComponentId) -> &StayMarks {
        &self.components[c.index()].stay_marks
    }

    /// The source of field component `c`.
    #[must_use]
    pub fn component_source(&self, c: ComponentId) -> BirthSource {
        self.components[c.index()].source
    }

    /// The budget of `source` in `band` accounted for: its components' stay shares weighted by
    /// their budgets, its classes' weights and its gone share, over its budget. 1 to rounding
    /// (P08.T9's `budget_closes`); 1 for a source with no budget.
    #[must_use]
    pub fn closure(&self, band: MassBand, source: BirthSource) -> f64 {
        let Some(b) = band_slot(band) else {
            return 1.0;
        };
        let src = &self.sources[source_index(source)];
        if src.budget[b] <= 0.0 {
            return 1.0;
        }
        let stay: f64 = self
            .components
            .iter()
            .filter(|c| c.source == source)
            .map(|c| c.stay[b] * c.budget[b] / src.budget[b])
            .sum();
        let classes: f64 = self
            .classes
            .iter()
            .filter(|c| c.key.source() == Some(source))
            .map(|c| c.weight[b])
            .sum();
        stay + classes + src.gone[b]
    }
}

/// The class of `index` in [`ClassKey::id`]'s order.
fn key_of(index: usize) -> ClassKey {
    let thin = SPEED_BINS * AGE_BINS;
    let bin = |i: usize| SpeedBin::new(u8::try_from(i).expect("under 8")).expect("under 8");
    if index < thin {
        ClassKey::Thin {
            speed: bin(index / AGE_BINS),
            age: age_bin(index % AGE_BINS),
        }
    } else if index < CLASS_COUNT - 1 {
        let old = index - thin;
        ClassKey::Old {
            source: OldSource::of(SOURCES[1 + old / SPEED_BINS]).expect("an old source"),
            speed: bin(old % SPEED_BINS),
        }
    } else {
        ClassKey::Hypervelocity
    }
}

/// The index of `source`'s remnant class in speed bin `speed` and age bin `age` (ignored for the
/// old sources).
fn remnant_class(source: BirthSource, speed: usize, age: usize) -> usize {
    ClassKey::of(source, speed_bin(speed), age_bin(age))
        .id()
        .index()
}

/// Speed bin `i`, below 8.
fn speed_bin(i: usize) -> SpeedBin {
    SpeedBin::new(u8::try_from(i).expect("under 8")).expect("under 8")
}

/// Age bin `i`, below 7.
fn age_bin(i: usize) -> AgeBin {
    AgeBin::new(u8::try_from(i).expect("under 7")).expect("under 7")
}

/// The ages at which a star of mass `m` (M☉) of `kind` is ejected, with their weights summing to
/// 1: the encounters' uniform 0–3 Myr and the supernova release at the lifetime of a companion of
/// max(8 M☉, m ÷ q), q uniform on 0.3–1, each by a 16-node rule. The companion's lifetime is
/// interpolated in log mass on band E's `lifetimes` at `masses`.
fn ejection_ages(
    model: RunawayModel,
    kind: Ejected,
    m: f64,
    lifetimes: &[f64; MARK_MASS_NODES],
    masses: &[f64; MARK_MASS_NODES],
) -> Vec<(f64, f64)> {
    let (encounter, supernova) = model.channels(kind);
    let mut out = Vec::with_capacity(32);
    if encounter > 0.0 {
        out.extend(
            gl16(0.0, ENCOUNTER_MAX_AGE.value())
                .map(|(t, w)| (t, encounter * w / ENCOUNTER_MAX_AGE.value())),
        );
    }
    if supernova > 0.0 {
        let (q_lo, q_hi) = RELEASE_Q_RANGE;
        out.extend(gl16(q_lo, q_hi).map(|(q, w)| {
            let companion = (m / q).clamp(RELEASING_MIN_MASS.value(), MASS_BAND_EDGES[5]);
            (
                interpolate_log(masses, lifetimes, companion),
                supernova * w / (q_hi - q_lo),
            )
        }));
    }
    out
}
