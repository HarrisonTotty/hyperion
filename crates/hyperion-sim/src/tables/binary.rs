//! Plan 11's binary tables (P11.T6–T8): their shapes are plan 11's, their contents plan 15's
//! (P15.T9.b, P15.T10.b), which P11.T15 swaps in with a version bump.
//!
//! @provisional: hand-made scratch values (plan 11, Design note 13), not written by
//! `hyperion-fit`, so not in [`MANIFEST`](super::MANIFEST) until plan 15's task for them exists.
//! since-generator-version: 16 (the batch that lands P11.T7)
//! source: the brainstorm's counts at Milky Way parameters (plan 11, P11.T7): 9 × 10⁶ accreting
//!   white dwarfs (Pala et al. 2020, MNRAS 494, 3799; the brainstorm's 6–12 × 10⁶, split in
//!   halves by design note 12), 10⁴ X-ray binaries (Corral-Santana et al. 2016, A&A 587, A61),
//!   9 × 10⁴ stellar mergers in the source horizon (0.34 a year, Kochanek et al. 2014, MNRAS 443,
//!   1319, × 264,144 years) and 10 neutron-star mergers
//!
//! [`CLASS_SHARES`] (P11.T7) is the share matrix's row for each carved class: how many hosts a
//! solar mass formed makes, by population, and which layer gives each host up. Plan 09's
//! [`FeatureShares::set_class_shares`](crate::galaxy::features::shares::FeatureShares::set_class_shares)
//! folds it into the field factor, so that the grid, which redraws every binary that falls into a
//! class ([`SystemStars::generate`](crate::stellar::system::SystemStars::generate)), gives up
//! exactly what P11.T8's catalogue classes hold.
//!
//! **Scratch.** Every population makes the same hosts per solar mass formed, scaled so that the
//! Milky Way fixture's formed mass ([`MILKY_WAY_FORMED_MASS`]) gives the counts above, and each
//! class's layers are a guess from the stars that make it: an accreting white dwarf's primary is
//! its white dwarf's progenitor, 0.75–8 M☉; an X-ray binary's and a neutron-star merger's is the
//! first compact star's, 8 M☉ and up; a stellar merger's is spread over the layers with most of it
//! in C, where the contact binaries of W Ursae Majoris type lie. Plan 15's P15.T10.b fits them
//! against the engine.
//!
//! [`ClassSamplerTable`] (P11.T8) is a catalogue class's conditional laws of the marks that define
//! it now and of its history, which plan 11's `stellar::binary::sampler::ClassSampler` draws by
//! inverse transform. Each mark is a residual given by its quantiles at the 17 ranks k ÷ 16, as
//! plan 15's `PRIMARY_MASS_CDF` gives its laws, added to a median that follows a condition on a
//! 17-point grid. [`AWD_SAMPLER`] is the accreting white dwarfs' and [`XRB_SAMPLER`] the X-ray
//! binaries': hand-made scratch from the observed figures each mark's doc names (Design note 13),
//! which P15.T10.b refits against the engine's own members; [`MERGER_SAMPLER`] is the stellar
//! mergers' (P11.T8.c) and [`NSM_SAMPLER`] the neutron-star mergers' (P11.T8.d).

use std::marker::PhantomData;

use crate::galaxy::POPULATIONS;
use crate::galaxy::Population;
use crate::galaxy::imf::MassBand;
use crate::stellar::binary::light_curves::XrayAccretor;
use crate::stellar::binary::{BinaryClass, CarvedClass, HmxbKind, XrbKind};

/// The Milky Way fixture's initial mass formed, M☉, that the scratch rates are scaled to: its
/// components' systems (with those not yet born) times the mass formed per system.
pub const MILKY_WAY_FORMED_MASS: f64 = 9.376_830_704_125_888e10;

/// One carved class's row of [`ClassShareTable`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClassShareRow {
    class: CarvedClass,
    /// Hosts per solar mass formed, by population in [`POPULATIONS`] order: for the two merger
    /// classes, mergers inside the source horizon.
    hosts_per_formed_mass: [f64; 7],
    /// The share of the hosts whose primary lies in each stellar layer, A–E; they sum to 1.
    layer_shares: [f64; 5],
}

impl ClassShareRow {
    /// The class.
    #[must_use]
    pub const fn class(&self) -> CarvedClass {
        self.class
    }

    /// Hosts of the class per solar mass formed in `population`.
    #[must_use]
    pub fn hosts_per_formed_mass(&self, population: Population) -> f64 {
        self.hosts_per_formed_mass[population_index(population)]
    }

    /// The share of the class's hosts whose primary lies in `band`: 0 for a substellar band.
    #[must_use]
    pub const fn layer_share(&self, band: MassBand) -> f64 {
        match band {
            MassBand::A | MassBand::B | MassBand::C | MassBand::D | MassBand::E => {
                self.layer_shares[band.index()]
            }
            MassBand::BrownDwarf | MassBand::RoguePlanet => 0.0,
        }
    }
}

/// The share matrix's rows for the carved binary classes (plan 11's Provides): per class and
/// population, hosts per solar mass formed, and the share of them that each of layers A–E gives up.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClassShareTable {
    rows: [ClassShareRow; 5],
}

impl ClassShareTable {
    /// The rows, in [`CarvedClass`] order.
    #[must_use]
    pub const fn rows(&self) -> &[ClassShareRow; 5] {
        &self.rows
    }

    /// The row of `class`.
    ///
    /// # Panics
    ///
    /// Never: the table holds a row for every carved class.
    #[must_use]
    pub fn row(&self, class: CarvedClass) -> &ClassShareRow {
        self.rows
            .iter()
            .find(|row| row.class == class)
            .expect("the table holds a row for every carved class")
    }
}

/// A population's place in [`POPULATIONS`].
fn population_index(population: Population) -> usize {
    POPULATIONS
        .iter()
        .position(|&p| p == population)
        .expect("every population is in POPULATIONS")
}

/// A row whose hosts, `count` at Milky Way parameters, every population makes alike per solar
/// mass formed.
const fn uniform(class: CarvedClass, count: f64, layer_shares: [f64; 5]) -> ClassShareRow {
    let rate = count / MILKY_WAY_FORMED_MASS;
    ClassShareRow {
        class,
        hosts_per_formed_mass: [rate; 7],
        layer_shares,
    }
}

/// The scratch share rows (P11.T7; module documentation): 4.5 × 10⁶ fast and 4.5 × 10⁶ slow
/// accreting white dwarfs, 10⁴ X-ray binaries, 9 × 10⁴ stellar mergers and 10 neutron-star
/// mergers in the source horizon at Milky Way parameters.
pub const CLASS_SHARES: ClassShareTable = ClassShareTable {
    rows: [
        uniform(
            CarvedClass::AccretingWdFast,
            4.5e6,
            [0.0, 0.0, 0.75, 0.25, 0.0],
        ),
        uniform(
            CarvedClass::AccretingWdSlow,
            4.5e6,
            [0.0, 0.0, 0.75, 0.25, 0.0],
        ),
        uniform(CarvedClass::XrayBinary, 1.0e4, [0.0, 0.0, 0.0, 0.0, 1.0]),
        uniform(
            CarvedClass::StellarMerger,
            9.0e4,
            [0.1, 0.2, 0.5, 0.15, 0.05],
        ),
        uniform(
            CarvedClass::NeutronStarMerger,
            10.0,
            [0.0, 0.0, 0.0, 0.0, 1.0],
        ),
    ],
};

/// A mark's median as a function of its condition (P11.T8): its value at 17 evenly spaced nodes
/// from the first to the last value of the condition, linear between them and held beyond.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MedianCurve {
    first: f64,
    last: f64,
    nodes: [f64; 17],
}

impl MedianCurve {
    /// The median of `nodes` at 17 evenly spaced values of the condition from `first` to `last`.
    #[must_use]
    pub const fn new(first: f64, last: f64, nodes: [f64; 17]) -> Self {
        Self { first, last, nodes }
    }

    /// The condition's values at the first and the last node.
    #[must_use]
    pub const fn span(&self) -> (f64, f64) {
        (self.first, self.last)
    }

    /// The median at the 17 nodes.
    #[must_use]
    pub const fn nodes(&self) -> &[f64; 17] {
        &self.nodes
    }
}

/// One mark of a [`ClassSamplerTable`] (P11.T8): a residual, by its quantiles at the 17 ranks
/// k ÷ 16 and linear between them, about a median that is zero or follows a condition, the value
/// held to its limits.
///
/// The shape is narrower than one law per condition node: the residual's spread is the same at
/// every condition, which the median shifts (plan 11's Risks, "P11.T8.a as built").
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SamplerMark {
    name: &'static str,
    residual: [f64; 17],
    median: Option<MedianCurve>,
    limits: (f64, f64),
}

impl SamplerMark {
    /// The mark `name` of residual quantiles `residual` (non-decreasing, at the ranks k ÷ 16)
    /// about `median`, held to `limits`.
    #[must_use]
    pub const fn new(
        name: &'static str,
        residual: [f64; 17],
        median: Option<MedianCurve>,
        limits: (f64, f64),
    ) -> Self {
        Self {
            name,
            residual,
            median,
            limits,
        }
    }

    /// The mark's name, in `snake_case`, with its unit.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// The residual's quantiles at the ranks k ÷ 16, non-decreasing.
    #[must_use]
    pub const fn residual(&self) -> &[f64; 17] {
        &self.residual
    }

    /// The median the residual is added to: `None` for an unconditional mark, whose residual is
    /// its value.
    #[must_use]
    pub const fn median(&self) -> Option<&MedianCurve> {
        self.median.as_ref()
    }

    /// The least and the greatest value a draw takes.
    #[must_use]
    pub const fn limits(&self) -> (f64, f64) {
        self.limits
    }
}

/// One choice among a class's kinds (P11.T8): its weights, which sum to 1, in the kinds' order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SamplerPick {
    name: &'static str,
    weights: &'static [f64],
}

impl SamplerPick {
    /// The choice `name` among kinds of `weights`, in the kinds' order.
    #[must_use]
    pub const fn new(name: &'static str, weights: &'static [f64]) -> Self {
        Self { name, weights }
    }

    /// The choice's name, in `snake_case`.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// The kinds' weights, in order.
    #[must_use]
    pub const fn weights(&self) -> &'static [f64] {
        self.weights
    }
}

/// The key of a class's mark or choice in its [`ClassSamplerTable`]: a class's own enum, so that a
/// sampler is only ever asked for its own table's marks (P11.T8).
pub trait SamplerKey: Copy {
    /// The key's place in its table's marks or choices.
    #[must_use]
    fn index(self) -> usize;
}

/// A catalogue class's sampler table (plan 11's Provides; P11.T8): its marks and its choices, in
/// the order the class draws them, keyed by the class's mark enum `M` and choice enum `P`. The
/// mark list is fixed per class: [`AwdMark`] and [`AwdPick`] for [`AWD_SAMPLER`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClassSamplerTable<M, P> {
    marks: &'static [SamplerMark],
    picks: &'static [SamplerPick],
    keys: PhantomData<fn() -> (M, P)>,
}

impl<M: SamplerKey, P: SamplerKey> ClassSamplerTable<M, P> {
    /// The table of `marks` and `picks`, in the orders `M` and `P` index them.
    #[must_use]
    pub const fn new(marks: &'static [SamplerMark], picks: &'static [SamplerPick]) -> Self {
        Self {
            marks,
            picks,
            keys: PhantomData,
        }
    }

    /// The marks, in the class's order.
    #[must_use]
    pub const fn marks(&self) -> &'static [SamplerMark] {
        self.marks
    }

    /// The choices, in the class's order.
    #[must_use]
    pub const fn picks(&self) -> &'static [SamplerPick] {
        self.picks
    }

    /// Mark `mark`.
    ///
    /// # Panics
    ///
    /// If the table holds fewer marks than `M` names: a table built against another list.
    #[must_use]
    pub fn mark(&self, mark: M) -> &'static SamplerMark {
        &self.marks[mark.index()]
    }

    /// Choice `pick`.
    ///
    /// # Panics
    ///
    /// If the table holds fewer choices than `P` names.
    #[must_use]
    pub fn pick(&self, pick: P) -> &'static SamplerPick {
        &self.picks[pick.index()]
    }
}

/// The marks of [`AWD_SAMPLER`], in its order (P11.T8.a): what an accreting white dwarf is now,
/// then its history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AwdMark {
    /// log₁₀ of the nova recurrence period, years, unconditional: the mark the two classes split
    /// on at L (Design note 12).
    Recurrence,
    /// The white dwarf's mass, M☉, given the recurrence's log₁₀.
    WhiteDwarfMass,
    /// log₁₀ of a main-sequence donor's orbital period, hours, given log₁₀ of the transfer rate,
    /// M☉ yr⁻¹.
    MainSequencePeriod,
    /// log₁₀ of a helium donor's orbital period, hours, given log₁₀ of the transfer rate.
    HeliumPeriod,
    /// log₁₀ of a giant donor's orbital period, hours, given log₁₀ of the transfer rate.
    GiantPeriod,
    /// log₁₀ of a main-sequence donor's mass, M☉, given log₁₀ of the period, hours.
    MainSequenceDonorMass,
    /// log₁₀ of a helium donor's mass, M☉, given log₁₀ of the period, hours.
    HeliumDonorMass,
    /// log₁₀ of a giant donor's mass, M☉, given log₁₀ of the period, hours.
    GiantDonorMass,
    /// log₁₀ of the primary's initial mass, M☉, given the white dwarf's mass.
    PrimaryInitialMass,
    /// log₁₀ of the donor's initial mass, M☉, given log₁₀ of its mass now.
    DonorInitialMass,
    /// log₁₀ of the system's age at the epoch, years, given log₁₀ of the primary's initial mass.
    Age,
}

impl AwdMark {
    /// Every mark, in [`AWD_SAMPLER`]'s order.
    pub const ALL: [Self; 11] = [
        Self::Recurrence,
        Self::WhiteDwarfMass,
        Self::MainSequencePeriod,
        Self::HeliumPeriod,
        Self::GiantPeriod,
        Self::MainSequenceDonorMass,
        Self::HeliumDonorMass,
        Self::GiantDonorMass,
        Self::PrimaryInitialMass,
        Self::DonorInitialMass,
        Self::Age,
    ];

    /// The mark's place in [`AWD_SAMPLER`]'s marks.
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}

impl SamplerKey for AwdMark {
    fn index(self) -> usize {
        Self::index(self)
    }
}

/// The choices of [`AWD_SAMPLER`], in its order (P11.T8.a).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AwdPick {
    /// The donor's kind ([`AwdDonorKind`]).
    DonorKind,
}

impl SamplerKey for AwdPick {
    fn index(self) -> usize {
        match self {
            Self::DonorKind => 0,
        }
    }
}

/// The donor kinds of [`AWD_SAMPLER`]'s one choice, in its order (P11.T8.a).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AwdDonorKind {
    /// A hydrogen-rich main-sequence star: a cataclysmic variable.
    MainSequence,
    /// A helium white dwarf: an AM Canum Venaticorum star.
    Helium,
    /// A red giant filling its Roche lobe: a symbiotic star.
    Giant,
}

impl AwdDonorKind {
    /// Every kind, in the choice's order.
    pub const ALL: [Self; 3] = [Self::MainSequence, Self::Helium, Self::Giant];
}

/// The residual of a normal of standard deviation 1, truncated at ±2.5, at the ranks k ÷ 16.
const NORMAL_RANKS: [f64; 17] = [
    -2.5, -1.491_36, -1.128_013, -0.872_818, -0.664_751, -0.482_209, -0.314_548, -0.155_341, 0.0,
    0.155_341, 0.314_548, 0.482_209, 0.664_751, 0.872_818, 1.128_013, 1.491_36, 2.5,
];

/// [`NORMAL_RANKS`] scaled to a standard deviation of `sigma`.
const fn normal(sigma: f64) -> [f64; 17] {
    let mut out = [0.0; 17];
    let mut k = 0;
    while k < 17 {
        out[k] = NORMAL_RANKS[k] * sigma;
        k += 1;
    }
    out
}

/// A median flat at `value` over the condition's span `(first, last)`.
const fn flat(first: f64, last: f64, value: f64) -> MedianCurve {
    MedianCurve {
        first,
        last,
        nodes: [value; 17],
    }
}

/// A median rising linearly from `at_first` to `at_last` over the condition's span.
const fn linear(first: f64, last: f64, at_first: f64, at_last: f64) -> MedianCurve {
    let mut nodes = [0.0; 17];
    let mut k = 0;
    while k < 17 {
        #[expect(
            clippy::cast_precision_loss,
            reason = "a node number of 0–16, exact in f64"
        )]
        let t = k as f64 / 16.0;
        nodes[k] = at_first + (at_last - at_first) * t;
        k += 1;
    }
    MedianCurve { first, last, nodes }
}

/// The scratch sampler of the two accreting white dwarf classes (P11.T8.a; Design notes 10, 12
/// and 13): [`AwdMark`]'s eleven marks and [`AwdPick`]'s one choice, the donor's kind
/// ([`AwdDonorKind`]).
///
/// Hand-made from observed figures, each recalled and not read at source in this build unless its
/// line says otherwise, and the same in every population; P15.T10.b refits it against the
/// engine's members (plan 11's `brute_force_class_members`):
///
/// - **Recurrence**, log₁₀ years: ranks 0 to ½ from 28,000 years to L (the fast hosts), ½ to 1
///   from L to 10⁸ years (the slow), so that the median is L exactly and the two classes take
///   halves, as [`CLASS_SHARES`] gives them. Classical novae recur every 10⁴–10⁵ years; at 9 × 10⁶
///   hosts the law gives 49 eruptions a year, inside the brainstorm's 30–50. It holds no recurrent
///   nova (under a century), which a 17-rank law cannot resolve at their few hundred hosts.
/// - **White dwarf mass**: 0.70 + 0.45 exp(−(x − 4.45) ÷ 0.8) M☉ at log₁₀ recurrence x, ±0.10
///   (one standard deviation, truncated at 2.5), on 0.55–1.36 M☉. Fast novae have the heavier
///   dwarfs; cataclysmic variables' white dwarfs average 0.83 ± 0.23 M☉ (Zorotovic, Schreiber and
///   Gänsicke 2011, A&A 536, A42).
/// - **Periods** by donor kind, given the transfer rate y = log₁₀ Ṁ: main-sequence donors after the
///   standard track (Knigge, Baraffe and Patterson 2011, ApJS 194, 28): 0.12 dex in hours (80
///   minutes) up to y = −11, rising to 2.1 hours at y = −9.7, across the period gap of 2.15–3.18
///   hours by y = −9.5 and to 5 hours at y = −8.5, ±0.03 dex; helium donors from an hour at
///   y = −12 to 12 minutes at y = −8.5, ±0.05 dex, held to AM Canum Venaticorum stars' 5.4–65.6
///   minutes (Ramsay et al. 2018; Solheim 2010, PASP 122, 1133); giants flat at 600 days, ±0.15
///   dex, the peak of the symbiotic stars' periods (Mikołajewska 2012, Baltic Astronomy 21, 5).
/// - **Donor masses** given log₁₀ of the period in hours, p: main-sequence donors on Warner's
///   (1995, Cataclysmic Variable Stars, equation 2.100) M₂ = 0.065 P^(5/4), ±0.03 dex; helium
///   donors 10^(−1 − 2(p + ½)) M☉, 0.1 M☉ at 19 minutes and 0.01 at an hour, ±0.05 dex; giants
///   1.3 M☉, ±0.08 dex.
/// - **The primary's initial mass** given the white dwarf's: Cummings et al.'s (2018, ApJ 866, 21)
///   initial–final mass relation (their PARSEC fit, three lines from 0.87 to 8.2 M☉) inverted,
///   held to 0.95–7.9 M☉ (layers C and D), ±0.03 dex.
/// - **The donor's initial mass** given its mass now: through 1.4 M☉ for the helium donors' 0.005
///   M☉, 0.63 M☉ for a donor of 0.1 M☉ now, 1.15 for one of 1 M☉, ±0.06 dex.
/// - **The age**: 10^(10.3 − 2.5 log₁₀ m₀) years, about twice the main-sequence lifetime of the
///   primary's m₀, at most 10^10.1, ±0.12 dex.
/// - **The donor's kind**: 0.907 main-sequence, 0.09 helium, 0.003 giant: AM Canum Venaticorum
///   stars' space density, 5 × 10⁻⁷ pc⁻³ (Carter et al. 2013, MNRAS 429, 2143, as Ramsay et al.
///   2018, A&A 620, A141, quote it), against Pala et al.'s (2020) 4.8 × 10⁻⁶ pc⁻³ for
///   cataclysmic variables, and a few times 10⁴ symbiotic stars.
pub const AWD_SAMPLER: ClassSamplerTable<AwdMark, AwdPick> = ClassSamplerTable::new(
    &AWD_MARKS,
    &[SamplerPick::new("donor_kind", &[0.907, 0.09, 0.003])],
);

/// log₁₀ L in years: 18 log₁₀ 2.
const LOG_L_YEARS: f64 = 5.418_539_921_951_662;

/// The span of the transfer rate's log₁₀ the periods' medians follow, M☉ yr⁻¹.
const RATE_SPAN: (f64, f64) = (-12.0, -8.5);

/// [`AWD_SAMPLER`]'s marks, in [`AwdMark`]'s order.
#[rustfmt::skip]
const AWD_MARKS: [SamplerMark; 11] = [
    SamplerMark {
        name: "log_recurrence_years",
        residual: [
            4.45, 4.75, 4.92, 5.05, 5.15, 5.23, 5.30, 5.36, LOG_L_YEARS,
            5.62, 5.84, 6.08, 6.34, 6.62, 6.94, 7.34, 8.0,
        ],
        median: None,
        limits: (4.45, 8.0),
    },
    SamplerMark {
        name: "white_dwarf_mass",
        residual: normal(0.10),
        median: Some(MedianCurve {
            first: 4.45,
            last: 8.0,
            nodes: [
                1.150_000, 1.041_007, 0.958_413, 0.895_824, 0.848_394, 0.812_452, 0.785_216,
                0.764_576, 0.748_935, 0.737_083, 0.728_101, 0.721_295, 0.716_137, 0.712_229,
                0.709_267, 0.707_022, 0.705_321,
            ],
        }),
        limits: (0.55, 1.36),
    },
    SamplerMark {
        name: "log_main_sequence_period_hours",
        residual: normal(0.03),
        median: Some(MedianCurve {
            first: RATE_SPAN.0,
            last: RATE_SPAN.1,
            nodes: [
                0.120_000, 0.120_000, 0.120_000, 0.120_000, 0.120_000, 0.129_375, 0.151_250,
                0.173_125, 0.195_000, 0.243_875, 0.300_750, 0.420_312, 0.525_000, 0.568_750,
                0.612_500, 0.656_250, 0.700_000,
            ],
        }),
        limits: (0.0, 1.0),
    },
    SamplerMark {
        name: "log_helium_period_hours",
        residual: normal(0.05),
        median: Some(linear(RATE_SPAN.0, RATE_SPAN.1, 0.0, -0.7)),
        // 5.4 and 65.6 minutes.
        limits: (-1.045_757, 0.038_753),
    },
    SamplerMark {
        name: "log_giant_period_hours",
        residual: normal(0.15),
        // 600 days.
        median: Some(flat(RATE_SPAN.0, RATE_SPAN.1, 4.158_362)),
        limits: (3.5, 5.0),
    },
    SamplerMark {
        name: "log_main_sequence_donor_mass",
        residual: normal(0.03),
        // log₁₀ 0.065 + 1.25 p.
        median: Some(linear(0.0, 0.8, -1.187_087, -0.187_087)),
        limits: (-1.301_030, -0.022_276),
    },
    SamplerMark {
        name: "log_helium_donor_mass",
        residual: normal(0.05),
        median: Some(linear(-1.0, 0.1, 0.0, -2.2)),
        limits: (-2.301_030, -0.698_970),
    },
    SamplerMark {
        name: "log_giant_donor_mass",
        residual: normal(0.08),
        // 1.3 M☉.
        median: Some(flat(3.5, 5.0, 0.113_943)),
        limits: (-0.096_910, 0.477_121),
    },
    SamplerMark {
        name: "log_primary_initial_mass",
        residual: normal(0.03),
        median: Some(MedianCurve {
            first: 0.55,
            last: 1.36,
            nodes: [
                -0.022_276, 0.154_591, 0.302_644, 0.412_854, 0.476_721, 0.515_468, 0.551_039,
                0.608_993, 0.669_378, 0.722_383, 0.769_616, 0.812_213, 0.851_002, 0.886_609,
                0.897_627, 0.897_627, 0.897_627,
            ],
        }),
        limits: (-0.022_276, 0.897_627),
    },
    SamplerMark {
        name: "log_donor_initial_mass",
        residual: normal(0.06),
        median: Some(MedianCurve {
            first: -2.3,
            last: 0.5,
            nodes: [
                0.150_000, 0.117_188, 0.084_375, 0.051_562, 0.018_750, -0.030_000, -0.100_000,
                -0.170_000, -0.176_000, -0.134_000, -0.092_000, -0.045_000, 0.004_000, 0.053_000,
                0.198_000, 0.359_000, 0.520_000,
            ],
        }),
        limits: (-1.096_910, 0.897_627),
    },
    SamplerMark {
        name: "log_age_years",
        residual: normal(0.12),
        median: Some(MedianCurve {
            first: -0.022_276,
            last: 0.897_627,
            nodes: [
                10.100_000, 10.100_000, 10.068_221, 9.924_486, 9.780_751, 9.637_016, 9.493_281,
                9.349_547, 9.205_812, 9.062_077, 8.918_342, 8.774_607, 8.630_872, 8.487_137,
                8.343_402, 8.199_667, 8.055_932,
            ],
        }),
        limits: (7.5, 10.12),
    },
];

/// The marks of [`MERGER_SAMPLER`], in its order (P11.T8.c): a stellar merger's age, then its
/// pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MergerMark {
    /// The rank, in [0, 1], of the system's age at the epoch in its density component's law
    /// restricted to the born systems, unconditional.
    AgeRank,
    /// The rank, in [0, 1], of the primary's initial mass in the galaxy's mass function over its
    /// layer's band, cut at the turn-off at the merger, unconditional.
    PrimaryRank,
    /// The mass ratio q = M₂ ÷ M₁ of the pair before the merger, unconditional: version 25's and
    /// before, every merger a contact pair.
    MassRatio,
    /// The mass ratio of a contact pair before the merger, at the tidal limit, unconditional:
    /// version 26's contact channel.
    ContactMassRatio,
    /// The rank, in [0, 1], of an evolved donor's initial mass in the galaxy's mass function over
    /// the masses living past their main sequence at the merger, unconditional.
    EvolvedPrimaryRank,
    /// The mass ratio of an evolved donor's companion over the donor, initial masses,
    /// unconditional: version 26's for every evolved donor, from 27 a giant-branch donor's.
    EvolvedMassRatio,
    /// The initial mass, M☉, of a dynamical main-sequence merger's donor, the primary, a low-mass
    /// (BSE's k = 0) main-sequence star, unconditional: version 27's dynamical channel.
    DynamicalPrimaryMass,
    /// The mass ratio of a dynamical main-sequence merger's companion over its donor, initial
    /// masses, unconditional: version 27's.
    DynamicalMassRatio,
    /// The mass ratio of a Hertzsprung-gap donor's companion over the donor, initial masses,
    /// unconditional: version 27's.
    GapMassRatio,
    /// The mass ratio of a core-helium-burning donor's companion over the donor, initial masses,
    /// unconditional: version 27's.
    HeliumBurningMassRatio,
    /// The mass ratio of an asymptotic-giant-branch donor's companion over the donor, initial
    /// masses, unconditional: version 27's.
    AgbMassRatio,
}

impl MergerMark {
    /// Every mark, in [`MERGER_SAMPLER`]'s order.
    pub const ALL: [Self; 11] = [
        Self::AgeRank,
        Self::PrimaryRank,
        Self::MassRatio,
        Self::ContactMassRatio,
        Self::EvolvedPrimaryRank,
        Self::EvolvedMassRatio,
        Self::DynamicalPrimaryMass,
        Self::DynamicalMassRatio,
        Self::GapMassRatio,
        Self::HeliumBurningMassRatio,
        Self::AgbMassRatio,
    ];

    /// The mark's place in [`MERGER_SAMPLER`]'s marks.
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}

impl SamplerKey for MergerMark {
    fn index(self) -> usize {
        Self::index(self)
    }
}

/// The choices of [`MERGER_SAMPLER`], in its order (P11.T8.c).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MergerPick {
    /// The layer of the primary, A–E, in [`MassBand::ALL`]'s order.
    Layer,
    /// The channel among [`MergerChannel::HELD`]: version 26's.
    Channel,
    /// The channel among [`MergerChannel::ALL`]: version 27's, with the dynamical main sequence.
    EngineChannel,
}

impl SamplerKey for MergerPick {
    fn index(self) -> usize {
        match self {
            Self::Layer => 0,
            Self::Channel => 1,
            Self::EngineChannel => 2,
        }
    }
}

/// The channels of [`MERGER_SAMPLER`]'s channel choices (P11.T8.c's fix, versions 26 and 27): what
/// a stellar merger is before its merger time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MergerChannel {
    /// A contact pair of two main-sequence stars, at the tidal limit (Rasio 1995).
    Contact,
    /// An evolved donor, from the Hertzsprung gap to the asymptotic giant branch, whose common
    /// envelope merges it with its main-sequence companion.
    EvolvedDonor,
    /// A low-mass (BSE's k = 0) main-sequence donor at its Roche lobe past its critical mass ratio,
    /// which the engine merges dynamically with its main-sequence companion (version 27's).
    DynamicalMainSequence,
}

impl MergerChannel {
    /// Every channel, in [`MergerPick::EngineChannel`]'s order.
    pub const ALL: [Self; 3] = [
        Self::Contact,
        Self::EvolvedDonor,
        Self::DynamicalMainSequence,
    ];

    /// Version 26's channels, in [`MergerPick::Channel`]'s order. The bump to 27 deletes them with
    /// that choice.
    pub const HELD: [Self; 2] = [Self::Contact, Self::EvolvedDonor];
}

/// The ranks k ÷ 16 themselves: a residual that leaves a rank as it is drawn.
const IDENTITY_RANKS: [f64; 17] = [
    0.0, 0.0625, 0.125, 0.1875, 0.25, 0.3125, 0.375, 0.4375, 0.5, 0.5625, 0.625, 0.6875, 0.75,
    0.8125, 0.875, 0.9375, 1.0,
];

/// The scratch sampler of the stellar mergers (P11.T8.c; Design notes 10 and 13): [`MergerMark`]'s
/// three marks and [`MergerPick`]'s one choice, the primary's layer.
///
/// Hand-made, the figures recalled and not read at source in this build (the literature could not
/// be reached), and the same in every population; P15.T10.b refits it against the engine's own
/// mergers:
///
/// - **The age** and **the primary's mass** are ranks that leave the density component's own age
///   law and the galaxy's own mass function as they are: the scratch merger rate per system is the
///   same at every age and mass within a layer, so a merger's history is a grid system's. The
///   primary is on its main sequence at the merger, so its mass is cut at the turn-off then.
/// - **The mass ratio** of the contact pair before the merger: 0.06–1, median 0.25. W Ursae
///   Majoris pairs hold mass ratios from about 0.07 to 1 (Rucinski 1993, PASP 105, 1433), and the
///   pairs that merge are the low ones, below Rasio's (1995, ApJ 444, L41) tidal limit near 0.09
///   and just above it: V1309 Scorpii's progenitor, the contact pair whose merger was seen in
///   2008 (Tylenda et al. 2011, A&A 528, A114), held a mass ratio near 0.1 (Stępień, Pamyatnykh
///   and Różyczka 2011, A&A 531, A18).
/// - **The layer**: [`CLASS_SHARES`]'s row for the stellar mergers, A 0.1, B 0.2, C 0.5, D 0.15,
///   E 0.05, restricted to the layers with stars on their main sequence at the merger.
///
/// Version 26's channels (P11.T8.c's fix; research-leans-13 §2(c)) add three marks and a choice:
///
/// - **The channel**: 0.15 contact, 0.85 evolved donor, scratch. Luminous red novae whose
///   progenitors are resolved had evolved donors: M31-LRN-2015's a subgiant of 3–5.5 M☉ (MacLeod
///   et al. 2017, ApJ 835, 282) and M101 OT2015-1's a yellow supergiant of about 18 M☉
///   (Blagorodnova et al. 2017, ApJ 834, 107); and the engine's own mergers, its pre-main-sequence
///   ones left out, are mostly evolved stars beside a main-sequence companion and a quarter or so
///   two main-sequence stars, of which few are in contact
///   (`galaxy/catalogue_classes/binary/merger/channels_hold.rs`'s
///   `the_channel_sampler_against_the_engine_under_every_26_hold` prints its split).
/// - **The contact pair's mass ratio**: 0.06–0.15, median 0.09, about Rasio's (1995) tidal limit,
///   below which a contact pair is unstable and coalesces on the main sequence, as the engine's
///   contact rule merges it; V1309 Scorpii's was about 0.1 (Stępień et al. 2011). The limits are
///   scratch, ours. The contact pair's primary is at least 0.1 ÷ 0.15 M☉, so that its secondary is
///   a track's at that ratio.
/// - **The evolved donor's companion**: q = M₂ ÷ M₁ of initial masses, 0.05–1, median 0.6, broad
///   up to the twins, the engine's own evolved donors' (weighted median 0.63), scratch; the scan
///   draws it below the ratio at which the engine finds the donor's transfer unstable.
/// - **The evolved donor's mass** is a rank that leaves the galaxy's mass function over the masses
///   past their main sequence and living at the merger as it is.
///
/// Version 27's (P11.T8.c's missing channel; research-leans-16 §1.1) add a choice and five marks,
/// scratch, shaped on the engine's own mergers under every 26 hold (48,000 members, an effective
/// 1,259; `merger/engine_channels_hold.rs`'s slow test prints them) and not fitted:
///
/// - **The channel** among three: 0.15 contact, as at 26, the light curves' evidence; 0.50 evolved
///   donor and 0.35 dynamical main sequence, the rest split about as the engine's evolved donors
///   beside a main-sequence star (0.40 of its mergers) and its pairs of main-sequence stars (0.30,
///   95% of them a low-mass donor detached before the merger).
/// - **The dynamical donor's mass**: 0.1–0.7 M☉, median 0.6, mostly above 0.4 (the engine's
///   weighted median 0.61, 5–95% 0.42–0.70): a star a little under BSE's 0.7 M☉ fills its lobe
///   first.
/// - **Its companion's mass ratio**: 0.15–1, median 0.73 (the engine's 0.71, 5–95% 0.49–0.95).
/// - **An evolved donor's companion by the donor's kind**: a giant-branch donor's stays
///   [`MergerMark::EvolvedMassRatio`] (the engine's median 0.59); a Hertzsprung-gap donor's
///   0.05–0.25, median 0.16, inside the ratio 1 ÷ 4 at which its transfer is unstable (the engine's
///   gap donors there lie about 0.11–0.25); a core-helium-burning donor's 0.05–0.33, flat to the
///   ratio 1 ÷ 3, the engine merging almost none; an asymptotic-giant-branch donor's 0.05–1,
///   median 0.49 (the engine's).
pub const MERGER_SAMPLER: ClassSamplerTable<MergerMark, MergerPick> = ClassSamplerTable::new(
    &MERGER_MARKS,
    &[
        SamplerPick::new("layer", &[0.1, 0.2, 0.5, 0.15, 0.05]),
        SamplerPick::new("channel", &[0.15, 0.85]),
        SamplerPick::new("engine_channel", &[0.15, 0.50, 0.35]),
    ],
);

/// [`MERGER_SAMPLER`]'s marks, in [`MergerMark`]'s order.
#[rustfmt::skip]
const MERGER_MARKS: [SamplerMark; 11] = [
    SamplerMark {
        name: "age_rank",
        residual: IDENTITY_RANKS,
        median: None,
        limits: (0.0, 1.0),
    },
    SamplerMark {
        name: "primary_rank",
        residual: IDENTITY_RANKS,
        median: None,
        limits: (0.0, 1.0),
    },
    SamplerMark {
        name: "mass_ratio",
        residual: [
            0.06, 0.08, 0.10, 0.12, 0.14, 0.16, 0.19, 0.22, 0.25,
            0.29, 0.33, 0.38, 0.44, 0.52, 0.62, 0.76, 1.0,
        ],
        median: None,
        limits: (0.06, 1.0),
    },
    SamplerMark {
        name: "contact_mass_ratio",
        residual: [
            0.060, 0.066, 0.071, 0.075, 0.079, 0.082, 0.085, 0.088, 0.090,
            0.093, 0.096, 0.099, 0.103, 0.108, 0.115, 0.125, 0.150,
        ],
        median: None,
        limits: (0.06, 0.15),
    },
    SamplerMark {
        name: "evolved_primary_rank",
        residual: IDENTITY_RANKS,
        median: None,
        limits: (0.0, 1.0),
    },
    SamplerMark {
        name: "evolved_mass_ratio",
        residual: [
            0.05, 0.11, 0.17, 0.25, 0.33, 0.41, 0.49, 0.55, 0.60,
            0.68, 0.76, 0.83, 0.89, 0.93, 0.96, 0.98, 1.0,
        ],
        median: None,
        limits: (0.05, 1.0),
    },
    SamplerMark {
        name: "dynamical_primary_mass",
        residual: [
            0.10, 0.40, 0.42, 0.44, 0.46, 0.49, 0.52, 0.56, 0.60,
            0.61, 0.62, 0.63, 0.64, 0.66, 0.68, 0.69, 0.70,
        ],
        median: None,
        limits: (0.1, 0.7),
    },
    SamplerMark {
        name: "dynamical_mass_ratio",
        residual: [
            0.15, 0.45, 0.52, 0.58, 0.63, 0.66, 0.69, 0.71, 0.73,
            0.76, 0.79, 0.82, 0.85, 0.88, 0.91, 0.94, 1.0,
        ],
        median: None,
        limits: (0.15, 1.0),
    },
    SamplerMark {
        name: "gap_mass_ratio",
        residual: [
            0.05, 0.08, 0.10, 0.11, 0.12, 0.13, 0.14, 0.15, 0.16,
            0.17, 0.18, 0.19, 0.20, 0.21, 0.22, 0.235, 0.25,
        ],
        median: None,
        limits: (0.05, 0.25),
    },
    SamplerMark {
        name: "helium_burning_mass_ratio",
        residual: [
            0.05, 0.0675, 0.085, 0.1025, 0.12, 0.1375, 0.155, 0.1725, 0.19,
            0.2075, 0.225, 0.2425, 0.26, 0.2775, 0.295, 0.3125, 0.33,
        ],
        median: None,
        limits: (0.05, 0.33),
    },
    SamplerMark {
        name: "agb_mass_ratio",
        residual: [
            0.05, 0.09, 0.12, 0.18, 0.27, 0.33, 0.39, 0.44, 0.49,
            0.54, 0.59, 0.64, 0.69, 0.75, 0.82, 0.89, 1.0,
        ],
        median: None,
        limits: (0.05, 1.0),
    },
];

/// The marks of [`NSM_SAMPLER`], in its order (P11.T8.d): a neutron-star merger's pair, then its
/// history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NsmMark {
    /// A neutron star's mass at the merger, M☉, unconditional.
    NeutronStarMass,
    /// log₁₀ of a black hole's mass at the merger, M☉, unconditional.
    LogBlackHoleMass,
    /// The rank, in [0, 1], of a neutron star's progenitor's initial mass in the galaxy's mass
    /// function over [`NSM_NEUTRON_STAR_PROGENITORS`], unconditional.
    NeutronStarProgenitorRank,
    /// The rank, in [0, 1], of a black hole's progenitor's initial mass in the galaxy's mass
    /// function over [`NSM_BLACK_HOLE_PROGENITORS`], unconditional.
    BlackHoleProgenitorRank,
    /// The rank, in [0, 1], of the system's age at the epoch in its density component's law
    /// restricted to the born systems, unconditional.
    AgeRank,
}

impl NsmMark {
    /// Every mark, in [`NSM_SAMPLER`]'s order.
    pub const ALL: [Self; 5] = [
        Self::NeutronStarMass,
        Self::LogBlackHoleMass,
        Self::NeutronStarProgenitorRank,
        Self::BlackHoleProgenitorRank,
        Self::AgeRank,
    ];

    /// The mark's place in [`NSM_SAMPLER`]'s marks.
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}

impl SamplerKey for NsmMark {
    fn index(self) -> usize {
        Self::index(self)
    }
}

/// The choices of [`NSM_SAMPLER`], in its order (P11.T8.d).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NsmPick {
    /// The pair's kind ([`NsmPairKind`]).
    PairKind,
}

impl SamplerKey for NsmPick {
    fn index(self) -> usize {
        match self {
            Self::PairKind => 0,
        }
    }
}

/// The pair kinds of [`NSM_SAMPLER`]'s one choice, in its order (P11.T8.d): the two kinds of
/// compact pair whose merger is a neutron-star merger (P11.T7's `merger_class`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NsmPairKind {
    /// Two neutron stars.
    DoubleNeutronStar,
    /// A black hole and a neutron star; the black hole is the primary.
    NeutronStarBlackHole,
}

impl NsmPairKind {
    /// Every kind, in the choice's order.
    pub const ALL: [Self; 2] = [Self::DoubleNeutronStar, Self::NeutronStarBlackHole];
}

/// The initial masses of a neutron star's progenitor a neutron-star merger draws, M☉: 8–20, ours,
/// provisional; about where single stars leave neutron stars rather than white dwarfs or black
/// holes (Sukhbold et al. 2016, ApJ 821, 38, recalled, not read at source).
pub const NSM_NEUTRON_STAR_PROGENITORS: (f64, f64) = (8.0, 20.0);

/// The initial masses of a black hole's progenitor a neutron-star merger draws, M☉: 25–60, ours,
/// provisional, as [`NSM_NEUTRON_STAR_PROGENITORS`].
pub const NSM_BLACK_HOLE_PROGENITORS: (f64, f64) = (25.0, 60.0);

/// A normal of mean `mean` and standard deviation `sigma`, truncated at ±2.5 σ, at the ranks
/// k ÷ 16.
const fn normal_about(mean: f64, sigma: f64) -> [f64; 17] {
    let mut out = normal(sigma);
    let mut k = 0;
    while k < 17 {
        out[k] += mean;
        k += 1;
    }
    out
}

/// The scratch sampler of the neutron-star mergers (P11.T8.d; Design notes 10 and 13):
/// [`NsmMark`]'s five marks and [`NsmPick`]'s one choice, the pair's kind ([`NsmPairKind`]).
///
/// Hand-made, the figures recalled and not read at source in this build (the literature could not
/// be reached), and the same in every population; P15.T10.b refits it against the engine's own
/// mergers:
///
/// - **A neutron star's mass**: 1.33 ± 0.09 M☉, the Galactic double neutron stars' (Özel and
///   Freire 2016, ARA&A 54, 401), held to plan 06's 1.13–2.0 M☉ (Mandel and Müller 2020).
/// - **A black hole's mass**: log-normal about 7 M☉, ±0.15 dex, on 3–20 M☉: the black holes of
///   the first neutron star–black hole mergers, GW200105's 8.9 M☉ and GW200115's 5.7 M☉ (Abbott
///   et al. 2021, ApJL 915, L5).
/// - **The progenitors' masses** and **the age** are ranks that leave the galaxy's mass function
///   over [`NSM_NEUTRON_STAR_PROGENITORS`] and [`NSM_BLACK_HOLE_PROGENITORS`] and the component's
///   age law as they are: the scratch merger rate is the same at every age, as [`CLASS_SHARES`]'s
///   row makes it.
/// - **The pair's kind**: 0.8 two neutron stars, 0.2 a neutron star and a black hole, after the
///   ratio of the two rates GWTC-3 gives, about 10–1,700 and 8–140 Gpc⁻³ yr⁻¹ (Abbott et al. 2023,
///   PRX 13, 011048).
pub const NSM_SAMPLER: ClassSamplerTable<NsmMark, NsmPick> =
    ClassSamplerTable::new(&NSM_MARKS, &[SamplerPick::new("pair_kind", &[0.8, 0.2])]);

/// [`NSM_SAMPLER`]'s marks, in [`NsmMark`]'s order.
#[rustfmt::skip]
const NSM_MARKS: [SamplerMark; 5] = [
    SamplerMark {
        name: "neutron_star_mass",
        residual: normal_about(1.33, 0.09),
        median: None,
        limits: (1.13, 2.0),
    },
    SamplerMark {
        name: "log_black_hole_mass",
        // log₁₀ 7, ±0.15 dex, on 3–20 M☉.
        residual: normal_about(0.845_098, 0.15),
        median: None,
        limits: (0.477_121, 1.301_030),
    },
    SamplerMark {
        name: "neutron_star_progenitor_rank",
        residual: IDENTITY_RANKS,
        median: None,
        limits: (0.0, 1.0),
    },
    SamplerMark {
        name: "black_hole_progenitor_rank",
        residual: IDENTITY_RANKS,
        median: None,
        limits: (0.0, 1.0),
    },
    SamplerMark {
        name: "age_rank",
        residual: IDENTITY_RANKS,
        median: None,
        limits: (0.0, 1.0),
    },
];

/// The kinds of the X-ray binaries' class (P11.T8.b), in [`XRB_SAMPLER`]'s two choices' order:
/// the four low-mass kinds of the old populations, then the two high-mass kinds of the young disc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum XrbHostKind {
    /// A neutron star fed by a low-mass donor above the irradiated disc's instability line.
    NeutronStarPersistent,
    /// A neutron star fed by a low-mass donor below the line: outbursts between quiescence.
    NeutronStarTransient,
    /// A black hole fed by a low-mass donor above the line.
    BlackHolePersistent,
    /// A black hole fed by a low-mass donor below the line.
    BlackHoleTransient,
    /// A neutron star beside a Be star, plan 06's.
    BeX,
    /// A neutron star fed by a supergiant filling its Roche lobe.
    Supergiant,
}

impl XrbHostKind {
    /// Every kind, in the choices' order.
    pub const ALL: [Self; 6] = [
        Self::NeutronStarPersistent,
        Self::NeutronStarTransient,
        Self::BlackHolePersistent,
        Self::BlackHoleTransient,
        Self::BeX,
        Self::Supergiant,
    ];

    /// The low-mass kinds, in [`XrbPick::LowMassKind`]'s order.
    pub const LOW_MASS: [Self; 4] = [
        Self::NeutronStarPersistent,
        Self::NeutronStarTransient,
        Self::BlackHolePersistent,
        Self::BlackHoleTransient,
    ];

    /// The high-mass kinds, in [`XrbPick::HighMassKind`]'s order.
    pub const HIGH_MASS: [Self; 2] = [Self::BeX, Self::Supergiant];

    /// Whether the kind is a high-mass X-ray binary's, whose hosts are the young disc's.
    #[must_use]
    pub const fn is_high_mass(self) -> bool {
        match self {
            Self::BeX | Self::Supergiant => true,
            Self::NeutronStarPersistent
            | Self::NeutronStarTransient
            | Self::BlackHolePersistent
            | Self::BlackHoleTransient => false,
        }
    }

    /// Whether the kind is a low-mass transient's, whose outbursts recur.
    #[must_use]
    pub const fn is_transient(self) -> bool {
        match self {
            Self::NeutronStarTransient | Self::BlackHoleTransient => true,
            Self::NeutronStarPersistent
            | Self::BlackHolePersistent
            | Self::BeX
            | Self::Supergiant => false,
        }
    }

    /// What the kind's compact star is: a neutron star, or a black hole for the black-hole kinds.
    #[must_use]
    pub const fn accretor(self) -> XrayAccretor {
        match self {
            Self::BlackHolePersistent | Self::BlackHoleTransient => XrayAccretor::BlackHole,
            Self::NeutronStarPersistent
            | Self::NeutronStarTransient
            | Self::BeX
            | Self::Supergiant => XrayAccretor::NeutronStar,
        }
    }

    /// The binary class P11.T5's `classify` gives a pair of the kind.
    #[must_use]
    pub const fn class(self) -> BinaryClass {
        match self {
            Self::NeutronStarPersistent | Self::BlackHolePersistent => {
                BinaryClass::LowMassXrayBinary(XrbKind::Persistent)
            }
            Self::NeutronStarTransient | Self::BlackHoleTransient => {
                BinaryClass::LowMassXrayBinary(XrbKind::Transient)
            }
            Self::BeX => BinaryClass::HighMassXrayBinary(HmxbKind::BeX),
            Self::Supergiant => BinaryClass::HighMassXrayBinary(HmxbKind::Supergiant),
        }
    }
}

/// The marks of [`XRB_SAMPLER`], in its order (P11.T8.b): what an X-ray binary is now, by its
/// kind, then its history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum XrbMark {
    /// A neutron star accretor's mass, M☉.
    NeutronStarMass,
    /// A black hole accretor's mass, M☉.
    BlackHoleMass,
    /// log₁₀ of a low-mass X-ray binary's orbital period, hours.
    LowMassPeriod,
    /// log₁₀ of a low-mass donor's mass, M☉, given log₁₀ of the period, hours.
    LowMassDonorMass,
    /// The transfer rate over the irradiated disc's instability line, dex: below 0 a transient,
    /// at or above it persistent.
    LowMassRateExcess,
    /// log₁₀ of a neutron-star transient's outburst recurrence, years.
    NeutronStarRecurrence,
    /// log₁₀ of a black-hole transient's outburst recurrence, years.
    BlackHoleRecurrence,
    /// log₁₀ of a Be/X-ray binary's orbital period, days.
    BexPeriod,
    /// A Be/X-ray binary's orbital eccentricity.
    BexEccentricity,
    /// A Be/X-ray binary's Be star's initial mass on its own track, M☉.
    BexDonorMass,
    /// log₁₀ of a supergiant X-ray binary's orbital period, days.
    SupergiantPeriod,
    /// A supergiant X-ray binary's donor's mass, M☉.
    SupergiantDonorMass,
    /// log₁₀ of a supergiant X-ray binary's transfer rate, M☉ yr⁻¹.
    SupergiantRate,
    /// A Be/X-ray binary's systemic speed from its supernova, km s⁻¹.
    BexRecoil,
    /// A supergiant X-ray binary's systemic speed from its supernova, km s⁻¹.
    SupergiantRecoil,
    /// log₁₀ of the years since a Be/X-ray binary's supernova.
    BexSinceSupernova,
    /// log₁₀ of the years since a supergiant X-ray binary's supernova.
    SupergiantSinceSupernova,
    /// log₁₀ of a neutron star's progenitor's initial mass, M☉.
    NeutronStarProgenitor,
    /// log₁₀ of a black hole's progenitor's initial mass, M☉.
    BlackHoleProgenitor,
    /// log₁₀ of a low-mass donor's initial mass over its mass now.
    DonorInitialExcess,
}

impl XrbMark {
    /// Every mark, in [`XRB_SAMPLER`]'s order.
    pub const ALL: [Self; 20] = [
        Self::NeutronStarMass,
        Self::BlackHoleMass,
        Self::LowMassPeriod,
        Self::LowMassDonorMass,
        Self::LowMassRateExcess,
        Self::NeutronStarRecurrence,
        Self::BlackHoleRecurrence,
        Self::BexPeriod,
        Self::BexEccentricity,
        Self::BexDonorMass,
        Self::SupergiantPeriod,
        Self::SupergiantDonorMass,
        Self::SupergiantRate,
        Self::BexRecoil,
        Self::SupergiantRecoil,
        Self::BexSinceSupernova,
        Self::SupergiantSinceSupernova,
        Self::NeutronStarProgenitor,
        Self::BlackHoleProgenitor,
        Self::DonorInitialExcess,
    ];

    /// The mark's place in [`XRB_SAMPLER`]'s marks.
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}

impl SamplerKey for XrbMark {
    fn index(self) -> usize {
        Self::index(self)
    }
}

/// The choices of [`XRB_SAMPLER`], in its order (P11.T8.b): a host's kind on its side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum XrbPick {
    /// A low-mass host's kind, among [`XrbHostKind::LOW_MASS`].
    LowMassKind,
    /// A high-mass host's kind, among [`XrbHostKind::HIGH_MASS`].
    HighMassKind,
}

impl SamplerKey for XrbPick {
    fn index(self) -> usize {
        match self {
            Self::LowMassKind => 0,
            Self::HighMassKind => 1,
        }
    }
}

/// The scratch sampler of the X-ray binaries' class (P11.T8.b; Design notes 10 and 13):
/// [`XrbMark`]'s twenty marks and [`XrbPick`]'s two kind choices.
///
/// Hand-made from observed figures, each recalled and not read at source in this build unless its
/// line says otherwise, and the same in every population; P15.T10.b refits it against the engine's
/// members (plan 11's `brute_force_class_members`). Each unconditional mark marked "±" is a normal
/// of the given mean and standard deviation, truncated at 2.5 and held to its limits.
///
/// - **The kinds**, as shares of all hosts: a neutron star's low-mass X-ray binaries 0.01
///   persistent and 0.699 transient, a black hole's 0.001 and 0.13, so that the black-hole
///   transients are the brainstorm's 1,300 of 10⁴ (Corral-Santana et al. 2016, A&A 587, A61) and
///   the persistent ones the Galaxy's hundred or so bright sources (Grimm, Gilfanov and Sunyaev
///   2002, A&A 391, 923); Be/X-ray binaries 0.15 and supergiant ones 0.01, some hundred against
///   the forty to fifty known, most of the high-mass systems being Be/X (Reig 2011, Ap&SS 332, 1).
///   Each choice's weights are its side's shares; the high-mass side's sum, 0.16, is the young
///   disc's share of the hosts.
/// - **Accretors**: neutron stars 1.45 ± 0.15 M☉ on 1.2–2.2 (Özel and Freire 2016, ARA&A 54,
///   401); black holes 7.8 ± 1.2 on 5–15 (Özel et al. 2010, ApJ 725, 1918).
/// - **Low-mass systems**: periods by a hand-made law from 2 hours to 10.5 days, median 8.3
///   hours, with a tail of evolved donors to days (the spread of Ritter and Kolb's 2003, A&A 404,
///   301, catalogue); donors on the Roche-lobe-filling main sequence's M₂ ≈ 0.11 P (hours) M☉
///   (Frank, King and Raine 2002, Accretion Power in Astrophysics, section 4.6) up to 0.8 M☉, at
///   7.3 hours, and evolved donors of 0.8 M☉ beyond, ±0.08 dex, on 0.1–1.5 M☉; the rate over the
///   irradiated disc's line from 3 dex below it to 1 above, its median on the line, so that each
///   side takes its own kinds by rank, and the Eddington rate held (`stellar::binary::scan`);
///   transients recurring every 10^(3.0 ± 0.5) years with a neutron star and 10^(2.5 ± 0.5)
///   with a black hole, some 14 and 8 outbursts a year over the Galaxy, against the 130 or so
///   black-hole outbursts of 1996–2015 (Tetarenko et al. 2016, ApJS 222, 15) that the sky
///   monitors saw.
/// - **Be/X-ray binaries**: periods 10^(1.8 ± 0.35) days on 12.6–355 d, eccentricities 0.35 ±
///   0.15 on 0–0.85 (Reig 2011's 12.7–330 d and 0.03–0.9), Be stars of 14 ± 2.5 M☉ on 9–19,
///   B0–B2 (Negueruela 1998, A&A 338, 505); 10^(6.8 ± 0.25) years after the supernova, on 0.3–20
///   Myr, within the Be star's main sequence.
/// - **Supergiant X-ray binaries**, marked in Roche-lobe overflow: periods 10^(0.6 ± 0.3) days on
///   1.3–16 d, donors of 20 ± 4 M☉ on 10–35, transfer 10^(−9 ± 0.5) M☉ yr⁻¹, 10³⁶–10³⁸ erg s⁻¹,
///   the Eddington rate held; 10^(6.2 ± 0.2) years after the supernova on 0.3–5 Myr.
/// - **Systemic speeds**: Maxwellians whose mean projected speed is van den Heuvel et al.'s (2000,
///   A&A 364, 563) mean tangential speed, 15 ± 6 km s⁻¹ for Be/X and 42 ± 14 for supergiant
///   systems, σ the mean tangential speed ÷ √(π ÷ 2): 12.0 km s⁻¹ (plan 09's
///   `PAIR_SYSTEMIC_SIGMA`, ruling 126.3) and 33.5, mean speeds of 19.1 and 53.5 km s⁻¹, held to
///   their 99.5% points, 43 and 120.
/// - **History**: a neutron star's progenitor of 10^(1.08 ± 0.08) M☉ on 9–17 and a black hole's
///   of 10^(1.48 ± 0.1) on 21–60, the ranges in which plan 06's single stars leave that remnant
///   at [Fe/H] −0.5 to +0.3 with median draws (measured for P11.T8.b); a low-mass donor's initial
///   mass 10^(0.15 ± 0.1) times its mass now, on 1–4 times.
pub const XRB_SAMPLER: ClassSamplerTable<XrbMark, XrbPick> = ClassSamplerTable::new(
    &XRB_MARKS,
    &[
        SamplerPick::new("low_mass_kind", &[0.01, 0.699, 0.001, 0.13]),
        SamplerPick::new("high_mass_kind", &[0.15, 0.01]),
    ],
);

/// The quantiles of a Maxwellian of σ = 1 at the ranks k ÷ 16, the last its 99.5% point: speeds
/// of three normal components (the χ distribution of three degrees of freedom).
const MAXWELL_RANKS: [f64; 17] = [
    0.0, 0.642_815, 0.832_080, 0.976_476, 1.101_151, 1.215_477, 1.324_379, 1.431_091, 1.538_172,
    1.648_056, 1.763_470, 1.887_973, 2.026_905, 2.189_572, 2.395_707, 2.704_675, 3.583_037,
];

/// [`MAXWELL_RANKS`] scaled to `sigma`.
const fn maxwell(sigma: f64) -> [f64; 17] {
    let mut out = [0.0; 17];
    let mut k = 0;
    while k < 17 {
        out[k] = MAXWELL_RANKS[k] * sigma;
        k += 1;
    }
    out
}

/// The Maxwellian σ of a Be/X-ray binary's systemic speed, km s⁻¹: 12.0, van den Heuvel et al.'s
/// mean tangential 15 ÷ √(π ÷ 2), plan 09's `PAIR_SYSTEMIC_SIGMA` (ruling 126.3).
const BEX_RECOIL_SIGMA: f64 = 12.0;

/// The Maxwellian σ of a supergiant X-ray binary's systemic speed, km s⁻¹: van den Heuvel et
/// al.'s mean tangential 42 ÷ √(π ÷ 2).
const SUPERGIANT_RECOIL_SIGMA: f64 = 33.511_152;

/// [`XRB_SAMPLER`]'s marks, in [`XrbMark`]'s order.
#[rustfmt::skip]
const XRB_MARKS: [SamplerMark; 20] = [
    SamplerMark::new("neutron_star_mass", normal_about(1.45, 0.15), None, (1.2, 2.2)),
    SamplerMark::new("black_hole_mass", normal_about(7.8, 1.2), None, (5.0, 15.0)),
    SamplerMark::new(
        "log_low_mass_period_hours",
        [
            0.3, 0.45, 0.55, 0.62, 0.68, 0.74, 0.80, 0.86, 0.92,
            0.99, 1.07, 1.16, 1.27, 1.40, 1.58, 1.85, 2.4,
        ],
        None,
        (0.3, 2.4),
    ),
    SamplerMark::new(
        "log_low_mass_donor_mass",
        normal(0.08),
        // log₁₀ 0.11 + p to the fifth node, then log₁₀ 0.8, on nodes 0.13125 apart in p.
        Some(MedianCurve::new(
            0.3,
            2.4,
            [
                -0.658_607, -0.527_357, -0.396_107, -0.264_857, -0.133_607, -0.096_910,
                -0.096_910, -0.096_910, -0.096_910, -0.096_910, -0.096_910, -0.096_910,
                -0.096_910, -0.096_910, -0.096_910, -0.096_910, -0.096_910,
            ],
        )),
        (-1.0, 0.176_091),
    ),
    SamplerMark::new(
        "low_mass_rate_excess_dex",
        [
            -3.0, -2.4, -2.0, -1.7, -1.4, -1.1, -0.8, -0.4, 0.0,
            0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.8, 1.0,
        ],
        None,
        (-3.0, 1.0),
    ),
    SamplerMark::new("log_neutron_star_recurrence_years", normal_about(3.0, 0.5), None, (1.75, 4.25)),
    SamplerMark::new("log_black_hole_recurrence_years", normal_about(2.5, 0.5), None, (1.25, 3.75)),
    SamplerMark::new("log_bex_period_days", normal_about(1.8, 0.35), None, (1.1, 2.55)),
    SamplerMark::new("bex_eccentricity", normal_about(0.35, 0.15), None, (0.0, 0.85)),
    SamplerMark::new("bex_donor_mass", normal_about(14.0, 2.5), None, (9.0, 19.0)),
    SamplerMark::new("log_supergiant_period_days", normal_about(0.6, 0.3), None, (0.1, 1.2)),
    SamplerMark::new("supergiant_donor_mass", normal_about(20.0, 4.0), None, (10.0, 35.0)),
    SamplerMark::new("log_supergiant_rate", normal_about(-9.0, 0.5), None, (-10.25, -7.75)),
    SamplerMark::new("bex_recoil_km_s", maxwell(BEX_RECOIL_SIGMA), None, (0.0, 43.0)),
    SamplerMark::new(
        "supergiant_recoil_km_s",
        maxwell(SUPERGIANT_RECOIL_SIGMA),
        None,
        (0.0, 120.1),
    ),
    SamplerMark::new("log_bex_years_since_supernova", normal_about(6.8, 0.25), None, (5.5, 7.3)),
    SamplerMark::new(
        "log_supergiant_years_since_supernova",
        normal_about(6.2, 0.2),
        None,
        (5.5, 6.7),
    ),
    SamplerMark::new(
        "log_neutron_star_progenitor_mass",
        normal_about(1.08, 0.08),
        None,
        (0.954_243, 1.230_449),
    ),
    SamplerMark::new(
        "log_black_hole_progenitor_mass",
        normal_about(1.48, 0.1),
        None,
        (1.322_219, 1.778_151),
    ),
    SamplerMark::new("log_donor_initial_excess", normal_about(0.15, 0.1), None, (0.0, 0.6)),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Seed;
    use crate::galaxy::Galaxy;
    use crate::galaxy::params::GalaxyParams;

    /// The Milky Way fixture's formed mass: its components' born systems, in their fixed order,
    /// times the mass formed per system.
    fn milky_way_formed_mass() -> f64 {
        let galaxy = Galaxy::from_params(Seed::new(0x0b17_0007), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture builds");
        let systems = galaxy
            .fields()
            .components()
            .iter()
            .fold(0.0, |sum, c| sum + c.count());
        systems * galaxy.mean_formed_mass().value()
    }

    /// The scratch rates are scaled to the fixture's formed mass, so that it holds the
    /// brainstorm's counts, and each class's layer shares sum to 1.
    #[test]
    fn the_scratch_rates_give_the_milky_way_counts() {
        let formed = milky_way_formed_mass();
        assert!(
            (formed / MILKY_WAY_FORMED_MASS - 1.0).abs() < 1e-9,
            "the fixture forms {formed:e} M_sun, the table is scaled to {MILKY_WAY_FORMED_MASS:e}"
        );
        let expected = [4.5e6, 4.5e6, 1.0e4, 9.0e4, 10.0];
        for (row, count) in CLASS_SHARES.rows().iter().zip(expected) {
            let hosts = row.hosts_per_formed_mass(Population::YoungThinDisc) * formed;
            assert!(
                (hosts / count - 1.0).abs() < 1e-9,
                "{:?}: {hosts}",
                row.class()
            );
            let layers = MassBand::ALL
                .iter()
                .fold(0.0, |sum, &band| sum + row.layer_share(band));
            assert!((layers - 1.0).abs() < 1e-12, "{:?}: {layers}", row.class());
            assert!(POPULATIONS.iter().all(|&p| {
                row.hosts_per_formed_mass(p)
                    .total_cmp(&row.hosts_per_formed_mass(Population::Halo))
                    .is_eq()
            }));
        }
        assert_eq!(
            CLASS_SHARES.row(CarvedClass::XrayBinary).class(),
            CarvedClass::XrayBinary
        );
    }

    /// The stellar mergers' table follows its mark list, each residual is non-decreasing inside
    /// its limits, and its layer weights are the share row's.
    #[test]
    fn the_merger_table_follows_its_mark_list_and_the_share_row() {
        let names = [
            "age_rank",
            "primary_rank",
            "mass_ratio",
            "contact_mass_ratio",
            "evolved_primary_rank",
            "evolved_mass_ratio",
            "dynamical_primary_mass",
            "dynamical_mass_ratio",
            "gap_mass_ratio",
            "helium_burning_mass_ratio",
            "agb_mass_ratio",
        ];
        assert_eq!(MERGER_SAMPLER.marks().len(), MergerMark::ALL.len());
        for (mark, name) in MergerMark::ALL.into_iter().zip(names) {
            let law = MERGER_SAMPLER.mark(mark);
            assert_eq!(law.name(), name);
            assert!(law.residual().windows(2).all(|w| w[0] <= w[1]), "{name}");
            let (lo, hi) = law.limits();
            assert!(
                law.residual().iter().all(|r| (lo..=hi).contains(r)),
                "{name}"
            );
        }
        for (pick, kinds) in [
            (MergerPick::Channel, MergerChannel::HELD.len()),
            (MergerPick::EngineChannel, MergerChannel::ALL.len()),
        ] {
            let channels = MERGER_SAMPLER.pick(pick).weights();
            assert_eq!(channels.len(), kinds, "{pick:?}");
            assert!(
                (channels.iter().sum::<f64>() - 1.0).abs() < 1e-12,
                "{pick:?}"
            );
        }
        assert_eq!(MergerChannel::ALL[..2], MergerChannel::HELD);
        let row = CLASS_SHARES.row(CarvedClass::StellarMerger);
        let weights = MERGER_SAMPLER.pick(MergerPick::Layer).weights();
        assert_eq!(weights.len(), MassBand::ALL.len());
        for (band, &weight) in MassBand::ALL.iter().zip(weights) {
            assert!((weight - row.layer_share(*band)).abs() < 1e-15, "{band:?}");
        }
    }

    /// The neutron-star mergers' table follows its mark list, each residual is non-decreasing, its
    /// masses are inside plan 06's neutron stars' and the black holes', and its kinds' weights sum
    /// to 1.
    #[test]
    fn the_nsm_table_follows_its_mark_list() {
        use crate::stellar::remnant::collapse::{MAX_NEUTRON_STAR_MASS, MIN_NEUTRON_STAR_MASS};
        let names = [
            "neutron_star_mass",
            "log_black_hole_mass",
            "neutron_star_progenitor_rank",
            "black_hole_progenitor_rank",
            "age_rank",
        ];
        assert_eq!(NSM_SAMPLER.marks().len(), NsmMark::ALL.len());
        for (mark, name) in NsmMark::ALL.into_iter().zip(names) {
            let law = NSM_SAMPLER.mark(mark);
            assert_eq!(law.name(), name);
            assert!(law.residual().windows(2).all(|w| w[0] <= w[1]), "{name}");
            let (lo, hi) = law.limits();
            assert!(lo < hi, "{name}");
        }
        let (lo, hi) = NSM_SAMPLER.mark(NsmMark::NeutronStarMass).limits();
        assert_eq!(
            (lo, hi),
            (MIN_NEUTRON_STAR_MASS.value(), MAX_NEUTRON_STAR_MASS.value())
        );
        let (lo, _) = NSM_SAMPLER.mark(NsmMark::LogBlackHoleMass).limits();
        assert!(crate::math::exp10(lo) > MAX_NEUTRON_STAR_MASS.value());
        let kinds = NSM_SAMPLER.pick(NsmPick::PairKind).weights();
        assert_eq!(kinds.len(), NsmPairKind::ALL.len());
        assert!((kinds.iter().sum::<f64>() - 1.0).abs() < 1e-12);
    }
}
