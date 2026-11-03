//! The binary classes' prior members (plan 11's test helpers; P11.T8.a and P11.T8.b): every
//! first-attempt binary of the grid that [`carved_class`] puts in a class, drawn with no carve-out,
//! against which a class's sampler is checked (P11.T8) and plan 15's P15.T10.b fits it.

use crate::galaxy::imf::MassBand;
use crate::galaxy::placement::{CellKey, SystemRecord, generate_cell};
use crate::galaxy::{Galaxy, Population};
use crate::id::Layer;
use crate::math;
use crate::orbit::{Eccentricity, KeplerElements};
use crate::rng::Threshold;
use crate::stellar::Phase;
use crate::stellar::binary::classify::{
    MAGNETIC_CV_SHARE, nova_recurrence_under, white_dwarf_feeding, xray_binary_reading, xrb_line,
};
use crate::stellar::binary::{
    BinaryClass, BinaryTimeline, CarvedClass, HmxbKind, XrbKind, carved_class,
};
use crate::stellar::multiplicity::{MultiplicityContext, RedrawAttempt};
use crate::stellar::system::{StarModel, SystemStars, draw_metallicity, primary_draws};
use crate::tables::binary::{AwdDonorKind, CLASS_SHARES, XrbHostKind};
use crate::units::{MetresPerSecond, Seconds, SolarMasses, SolarMassesPerYear, Years};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use super::Mix;

/// What an accreting white dwarf member is where [`carved_class`] reads it: the marks its class's
/// sampler draws.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AccretingMarks {
    /// The age read, Julian years: the epoch's, or the first of the horizon at which it accretes.
    pub age: Years,
    /// The white dwarf's mass, M☉.
    pub white_dwarf_mass: SolarMasses,
    /// The donor's mass, M☉.
    pub donor_mass: SolarMasses,
    /// The donor's phase.
    pub donor_phase: Phase,
    /// The orbital period, or `None` for a pair with no orbit then (none of the classes').
    pub period: Option<Seconds>,
    /// The rate onto the white dwarf, M☉ yr⁻¹.
    pub transfer_rate: SolarMassesPerYear,
    /// The nova recurrence period, Julian years, as [`carved_class`] reads it under the pair's
    /// rules: infinite for none (from 27 a white dwarf fed helium has none).
    pub recurrence: Years,
    /// Whether the white dwarf is magnetic, by its own draw.
    pub magnetic: bool,
    /// The white dwarf's progenitor's initial mass, M☉.
    pub white_dwarf_initial_mass: SolarMasses,
    /// The donor's initial mass, M☉.
    pub donor_initial_mass: SolarMasses,
}

impl AccretingMarks {
    /// The catalogue's kind of the donor ([`AwdDonorKind`]): a helium star's or a helium white
    /// dwarf's [`Helium`](AwdDonorKind::Helium), a giant's [`Giant`](AwdDonorKind::Giant), and
    /// any other's, a subgiant's or a brown dwarf's among them,
    /// [`MainSequence`](AwdDonorKind::MainSequence).
    #[must_use]
    pub const fn donor_kind(&self) -> AwdDonorKind {
        match self.donor_phase {
            Phase::HeliumWhiteDwarf
            | Phase::HeliumMainSequence
            | Phase::HeliumHertzsprungGap
            | Phase::HeliumGiantBranch => AwdDonorKind::Helium,
            Phase::FirstGiantBranch
            | Phase::CoreHeliumBurning
            | Phase::EarlyAgb
            | Phase::ThermallyPulsingAgb
            | Phase::PostAgb => AwdDonorKind::Giant,
            Phase::Protostar
            | Phase::PreMainSequence
            | Phase::MainSequence
            | Phase::Substellar
            | Phase::HertzsprungGap
            | Phase::CarbonOxygenWhiteDwarf
            | Phase::OxygenNeonWhiteDwarf
            | Phase::NeutronStar
            | Phase::BlackHole
            | Phase::NoRemnant => AwdDonorKind::MainSequence,
        }
    }
}

/// What an X-ray binary member is where [`carved_class`] reads it (P11.T8.b): the marks its class's
/// sampler draws.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct XrayMarks {
    /// The age read, Julian years: the epoch's, or the first of the horizon at which it is one.
    pub age: Years,
    /// Its class then: a low-mass X-ray binary's kind or a high-mass one's.
    pub class: BinaryClass,
    /// The compact star's phase: a neutron star or a black hole.
    pub accretor_phase: Phase,
    /// The compact star's mass, M☉.
    pub accretor_mass: SolarMasses,
    /// The donor's mass, M☉.
    pub donor_mass: SolarMasses,
    /// The donor's phase.
    pub donor_phase: Phase,
    /// The orbital period, or `None` for a pair with no orbit then.
    pub period: Option<Seconds>,
    /// The orbit's eccentricity, or `None` for a pair with no orbit then.
    pub eccentricity: Option<Eccentricity>,
    /// The rate of transfer in stable transfer then, M☉ yr⁻¹.
    pub transfer_rate: Option<SolarMassesPerYear>,
    /// The pair's speed of recoil from its supernovae, if it is still bound.
    pub recoil_speed: Option<MetresPerSecond>,
    /// The years from the pair's last supernova to the age read.
    pub since_supernova: Option<Years>,
    /// The compact star's progenitor's initial mass, M☉.
    pub accretor_initial_mass: SolarMasses,
    /// The donor's initial mass, M☉.
    pub donor_initial_mass: SolarMasses,
    /// The transfer rate over the irradiated disc's instability line that `classify` splits the
    /// low-mass kinds on, dex: below 0 a transient; `None` with no rate or no orbit.
    pub rate_excess_dex: Option<f64>,
}

impl XrayMarks {
    /// The member's kind among the six P15.T10.b's first fit fitted ([`XrbHostKind::LOW_MASS_HELD`]
    /// and [`XrbHostKind::HIGH_MASS_HELD`]), or `None` for one they do not hold: a symbiotic X-ray
    /// binary, a black hole beside a Be star or a supergiant, or a low-mass X-ray binary whose
    /// donor is a helium star or a white dwarf, an ultracompact one ([`member_kind`] names them).
    ///
    /// [`member_kind`]: Self::member_kind
    #[must_use]
    pub const fn host_kind(&self) -> Option<XrbHostKind> {
        let black_hole = matches!(self.accretor_phase, Phase::BlackHole);
        let hydrogen_donor = matches!(
            self.donor_phase,
            Phase::Protostar
                | Phase::PreMainSequence
                | Phase::MainSequence
                | Phase::Substellar
                | Phase::HertzsprungGap
                | Phase::FirstGiantBranch
                | Phase::CoreHeliumBurning
                | Phase::EarlyAgb
                | Phase::ThermallyPulsingAgb
                | Phase::PostAgb
        );
        match self.class {
            BinaryClass::LowMassXrayBinary(kind) => match (kind, black_hole, hydrogen_donor) {
                (XrbKind::Persistent, false, true) => Some(XrbHostKind::NeutronStarPersistent),
                (XrbKind::Transient, false, true) => Some(XrbHostKind::NeutronStarTransient),
                (XrbKind::Persistent, true, true) => Some(XrbHostKind::BlackHolePersistent),
                (XrbKind::Transient, true, true) => Some(XrbHostKind::BlackHoleTransient),
                (XrbKind::Persistent | XrbKind::Transient, _, false)
                | (XrbKind::Symbiotic, _, _) => None,
            },
            BinaryClass::HighMassXrayBinary(kind) => match (kind, black_hole) {
                (HmxbKind::BeX, false) => Some(XrbHostKind::BeX),
                (HmxbKind::Supergiant, false) => Some(XrbHostKind::Supergiant),
                (HmxbKind::BeX | HmxbKind::Supergiant, true) => None,
            },
            BinaryClass::None
            | BinaryClass::Algol
            | BinaryClass::Contact
            | BinaryClass::BlueStraggler
            | BinaryClass::HotSubdwarf
            | BinaryClass::RCoronaeBorealis
            | BinaryClass::Symbiotic
            | BinaryClass::CataclysmicVariable(_)
            | BinaryClass::MillisecondPulsar
            | BinaryClass::DoubleNeutronStar
            | BinaryClass::DoubleWhiteDwarf
            | BinaryClass::TypeIaProgenitor => None,
        }
    }
}

impl XrayMarks {
    /// The member's kind among all nine of [`XrbHostKind`] (P11.T8.b's fix): its held kind
    /// ([`host_kind`](Self::host_kind)), or else an ultracompact, a symbiotic or a black hole
    /// beside a massive star; `None` only for marks whose class is not an X-ray binary's, which no
    /// member's is.
    #[must_use]
    pub const fn member_kind(&self) -> Option<XrbHostKind> {
        if let Some(kind) = self.host_kind() {
            return Some(kind);
        }
        match self.class {
            BinaryClass::LowMassXrayBinary(XrbKind::Symbiotic) => Some(XrbHostKind::Symbiotic),
            BinaryClass::LowMassXrayBinary(XrbKind::Persistent | XrbKind::Transient) => {
                Some(XrbHostKind::Ultracompact)
            }
            BinaryClass::HighMassXrayBinary(HmxbKind::BeX | HmxbKind::Supergiant) => {
                Some(XrbHostKind::BlackHoleHighMass)
            }
            BinaryClass::None
            | BinaryClass::Algol
            | BinaryClass::Contact
            | BinaryClass::BlueStraggler
            | BinaryClass::HotSubdwarf
            | BinaryClass::RCoronaeBorealis
            | BinaryClass::Symbiotic
            | BinaryClass::CataclysmicVariable(_)
            | BinaryClass::MillisecondPulsar
            | BinaryClass::DoubleNeutronStar
            | BinaryClass::DoubleWhiteDwarf
            | BinaryClass::TypeIaProgenitor => None,
        }
    }
}

/// One prior member of a carved class: the grid record it came from, its class, its pair's
/// timeline and, for an accreting white dwarf or an X-ray binary, its marks.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassMember {
    record: SystemRecord,
    class: CarvedClass,
    timeline: BinaryTimeline,
    accreting: Option<AccretingMarks>,
    xray: Option<XrayMarks>,
}

impl ClassMember {
    /// The grid record.
    #[must_use]
    pub const fn record(&self) -> &SystemRecord {
        &self.record
    }

    /// The carved class [`carved_class`] puts it in.
    #[must_use]
    pub const fn class(&self) -> CarvedClass {
        self.class
    }

    /// The pair's timeline, from the engine.
    #[must_use]
    pub const fn timeline(&self) -> &BinaryTimeline {
        &self.timeline
    }

    /// An accreting white dwarf's marks where its class reads them.
    #[must_use]
    pub const fn accreting(&self) -> Option<&AccretingMarks> {
        self.accreting.as_ref()
    }

    /// An X-ray binary's marks where its class reads them.
    #[must_use]
    pub const fn xray(&self) -> Option<&XrayMarks> {
        self.xray.as_ref()
    }
}

/// `n` prior members of `class` near the solar circle (plan 11's test helpers): the first
/// attempt of every two-star pair of grid records, run through the engine with no carve-out, kept
/// where [`carved_class`] is `class`, from the layers in the shares [`CLASS_SHARES`] gives the
/// class's hosts. [`brute_force_class_members_in`] of stream 0.
///
/// It runs the binary engine on every pair that can interact: some milliseconds a pair, so a test
/// of thousands of members is a slow one.
#[must_use]
pub fn brute_force_class_members(
    galaxy: &Galaxy,
    class: CarvedClass,
    n: usize,
) -> Vec<ClassMember> {
    brute_force_class_members_in(galaxy, class, n, 0)
}

/// [`brute_force_class_members`] on the cells of stream `stream`, so that a test can split the
/// sampling between threads, each with its own stream, and join the members in stream order:
/// [`MemberCells::members_in`] of a cache of its own.
///
/// Each layer with a share s of the class's hosts contributes `round(n s)` members, from random
/// cells within 4,000 ly along x and 2,000 ly along y of the point 26,000 ly out along +y, up to
/// eight records a cell.
///
/// # Panics
///
/// Never: the layers' cells are a few light-years to 128 ly, and the cells it opens lie inside the
/// root cube.
#[must_use]
pub fn brute_force_class_members_in(
    galaxy: &Galaxy,
    class: CarvedClass,
    n: usize,
    stream: u64,
) -> Vec<ClassMember> {
    MemberCells::new(galaxy).members_in(class, n, stream)
}

/// The carved classes a cell's members are kept of ([`MemberCells`]): every one, each at its
/// [`kept_index`].
const KEPT_CLASSES: [CarvedClass; 5] = [
    CarvedClass::AccretingWdFast,
    CarvedClass::AccretingWdSlow,
    CarvedClass::XrayBinary,
    CarvedClass::StellarMerger,
    CarvedClass::NeutronStarMerger,
];

/// Where `class` sits in [`KEPT_CLASSES`]. The `match` is exhaustive, so a class added to
/// [`CarvedClass`] fails to compile here until it is placed, and a place [`KEPT_CLASSES`] lacks
/// fails [`MemberCells::members_in`] at once, where a draw from cells that keep none of the class
/// would never end.
const fn kept_index(class: CarvedClass) -> usize {
    match class {
        CarvedClass::AccretingWdFast => 0,
        CarvedClass::AccretingWdSlow => 1,
        CarvedClass::XrayBinary => 2,
        CarvedClass::StellarMerger => 3,
        CarvedClass::NeutronStarMerger => 4,
    }
}

/// The prior members of the cells that the draws of [`brute_force_class_members_in`] have opened
/// in one galaxy, each cell's members of every carved class found and kept, so that draws sharing
/// it, of any class and stream and on any threads, run each cell through the engine once (twice
/// at most where two draws first open it together).
///
/// A draw takes its cells at random from a box of each layer: 251 by 125 cells of layer C, 125 by
/// 63 of D and 63 by 31 of E. So a draw of a rare class opens each cell many times: the fast
/// accreting white dwarfs, about one in 2 × 10⁵ of layer C's first eight records under every 27
/// hold (2026-11-01), open about 4 × 10⁵ cells a stream for its 14 of 300, each of the box's
/// 31,375 some twelve times, and the sixteen streams of P11.T8.f's test some two hundred. A cell's members are its records' first attempts run through the engine, a function of
/// the galaxy and the cell alone, so a draw from the kept cells is the draw without them, member
/// for member, whatever else they hold and in whatever order they were filled.
///
/// It grows with the cells opened, holding the members alone: about one cell in a hundred of
/// layer C has one.
#[derive(Debug)]
pub struct MemberCells<'a> {
    galaxy: &'a Galaxy,
    kept: Mutex<BTreeMap<CellKey, Arc<[ClassMember]>>>,
}

impl<'a> MemberCells<'a> {
    /// No cells yet, of `galaxy`.
    #[must_use]
    pub const fn new(galaxy: &'a Galaxy) -> Self {
        Self {
            galaxy,
            kept: Mutex::new(BTreeMap::new()),
        }
    }

    /// [`brute_force_class_members_in`] of the galaxy, `n` members of `class` on the cells of
    /// stream `stream`, from the kept cells and those it opens, which it keeps.
    ///
    /// # Panics
    ///
    /// If the cells keep no members of `class`, a carved class added and not kept. Never else: the
    /// layers' cells are a few light-years to 128 ly, and the cells it opens lie inside the root
    /// cube.
    #[must_use]
    pub fn members_in(&self, class: CarvedClass, n: usize, stream: u64) -> Vec<ClassMember> {
        assert_eq!(
            KEPT_CLASSES.get(kept_index(class)),
            Some(&class),
            "a cell keeps the members of every carved class"
        );
        let row = CLASS_SHARES.row(class);
        let mut members = Vec::with_capacity(n + 8);
        for (index, band) in (0_u64..).zip(MassBand::ALL) {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                clippy::cast_precision_loss,
                reason = "a member count, rounded, of a test's few thousand"
            )]
            let wanted = (n as f64 * row.layer_share(band)).round() as usize;
            if wanted == 0 {
                continue;
            }
            let layer = Layer::from(band);
            let mut mix = Mix(0x0b17_08a0 ^ (stream << 8) ^ index);
            let size = i32::try_from(layer.cell_size_ly()).expect("a small cell size");
            let (along, across) = ((4_000 / size).max(1), (2_000 / size).max(1));
            let start = members.len();
            while members.len() - start < wanted {
                let key = CellKey::new(
                    layer,
                    [mix.within(along), 26_000 / size + mix.within(across), 0],
                )
                .expect("a cell of the grid");
                let cell = self.cell(key);
                members.extend(cell.iter().filter(|m| m.class == class).cloned());
            }
            members.truncate(start + wanted);
        }
        members
    }

    /// The members of every carved class among the first eight records of cell `key`, in record
    /// and pair order: kept, or found and kept. The engine runs outside the lock, so two draws
    /// opening one cell at once may both run it; the first kept is the one both return, and the
    /// two are the same.
    #[must_use]
    fn cell(&self, key: CellKey) -> Arc<[ClassMember]> {
        let kept = self.kept().get(&key).map(Arc::clone);
        if let Some(cell) = kept {
            return cell;
        }
        let mut records = Vec::new();
        generate_cell(self.galaxy, key, &mut records);
        let mut found = Vec::new();
        for record in records.iter().take(8) {
            members_of(self.galaxy, record, &KEPT_CLASSES, &mut found);
        }
        Arc::clone(self.kept().entry(key).or_insert_with(|| found.into()))
    }

    /// The kept cells, locked.
    fn kept(&self) -> MutexGuard<'_, BTreeMap<CellKey, Arc<[ClassMember]>>> {
        self.kept.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Where a census draws its grid records ([`class_census_in`]): the cells of its layer that meet a
/// box about a point, within half-widths of it along x, y and z, in the galactic frame of the
/// grid's cells, light-years (plan 01's `GalacticPosition`: the centre at the origin, the disc in
/// the x–y plane, the Sun-like point 26,000 ly out along +y).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CensusSite {
    centre_ly: [i32; 3],
    half_widths_ly: [u32; 3],
}

impl CensusSite {
    /// A slab of the disc about the solar circle: within 12,000 ly along x, 2,000 ly along y and
    /// 1,500 ly along z of the point 26,000 ly out along +y, where [`brute_force_class_members`]
    /// draws its members, wide enough that a census of 10⁷ records meets no cell twice.
    pub const SOLAR_CIRCLE: Self = Self::new([0, 26_000, 0], [12_000, 2_000, 1_500]);

    /// The cells within `half_widths_ly` of `centre_ly` along each axis.
    #[must_use]
    pub const fn new(centre_ly: [i32; 3], half_widths_ly: [u32; 3]) -> Self {
        Self {
            centre_ly,
            half_widths_ly,
        }
    }

    /// The site's cells of `layer`: the first cell's coordinates, and how many along each axis.
    ///
    /// # Panics
    ///
    /// If the site's box passes the `i32` light-years of the root cube's frame, or holds more than
    /// 2⁶⁴ cells.
    #[must_use]
    fn cells(&self, layer: Layer) -> ([i32; 3], [u64; 3]) {
        let size = i64::from(layer.cell_size_ly());
        let mut first = [0; 3];
        let mut counts = [0; 3];
        for axis in 0..3 {
            let centre = i64::from(self.centre_ly[axis]);
            let half = i64::from(self.half_widths_ly[axis]);
            let lo = (centre - half).div_euclid(size);
            let hi = (centre + half).div_euclid(size);
            first[axis] = i32::try_from(lo).expect("a site's first cell lies in the i32 frame");
            counts[axis] = u64::try_from(hi - lo + 1).expect("a site's cells are counted forward");
        }
        counts[0]
            .checked_mul(counts[1])
            .and_then(|n| n.checked_mul(counts[2]))
            .expect("a site holds under 2⁶⁴ cells");
        (first, counts)
    }
}

/// Whose records a census takes ([`class_census_in`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CensusRecords {
    /// Every population's.
    All,
    /// One population's alone.
    Of(Population),
}

impl CensusRecords {
    /// Whether a record of `population` is taken.
    #[must_use]
    pub const fn takes(self, population: Population) -> bool {
        match self {
            Self::All => true,
            Self::Of(p) => p.index() == population.index(),
        }
    }
}

/// One part of a census ([`class_census_in`]): of a site's cells taken in a fixed order that
/// spreads them over the site, those at positions `index`, `index + count`, `index + 2 count`, …,
/// so that the parts of one count share no cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CensusPart {
    index: u64,
    count: u64,
}

impl CensusPart {
    /// Part `index` of `count`.
    ///
    /// # Panics
    ///
    /// If `index` is not below `count`.
    #[must_use]
    pub const fn new(index: u64, count: u64) -> Self {
        assert!(index < count, "a part's index is below the parts' count");
        Self { index, count }
    }
}

/// What a census found ([`class_census_in`]): the records it ran, by population, and the members
/// of the accreting white dwarf and X-ray binary classes among them, in the order it met them.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassCensus {
    band: MassBand,
    scanned: [u64; 7],
    members: Vec<ClassMember>,
}

impl ClassCensus {
    /// The band whose layer's records it ran.
    #[must_use]
    pub const fn band(&self) -> MassBand {
        self.band
    }

    /// The records of `population` it ran.
    #[must_use]
    pub const fn scanned(&self, population: Population) -> u64 {
        self.scanned[population.index()]
    }

    /// The records it ran, in every population.
    #[must_use]
    pub fn scanned_total(&self) -> u64 {
        self.scanned.iter().sum()
    }

    /// The members found, in the order met.
    #[must_use]
    pub fn members(&self) -> &[ClassMember] {
        &self.members
    }

    /// The members found, in the order met, taken out of the census.
    #[must_use]
    pub fn into_members(self) -> Vec<ClassMember> {
        self.members
    }
}

/// The classes a census counts: both accreting white dwarf classes and the X-ray binaries.
const CENSUS_CLASSES: [CarvedClass; 3] = [
    CarvedClass::AccretingWdFast,
    CarvedClass::AccretingWdSlow,
    CarvedClass::XrayBinary,
];

/// A census of the carved classes' prior members among `records` grid records of `band`'s layer at
/// `site`, of the populations `whose`, from the cells of `part` (plan 15's P15.T10.b): each
/// record of each cell taken in turn, its first attempt run through the engine with no carve-out
/// as [`brute_force_class_members`] runs it, and every pair [`carved_class`] puts in an accreting
/// white dwarf class or the X-ray binaries' kept. Where [`brute_force_class_members`] takes a fixed
/// number of members a layer from random cells, a census takes a fixed number of records from
/// cells it meets once each, so that the members it finds a record are each band's and
/// population's rate, and two parts of one count are independent samples.
///
/// The site's cells are taken in the order of the stride ⌊φ⁻¹ N⌋ (lowered to the nearest number
/// prime to N) through their N linear indices, x fastest, which spreads any run of them over the
/// whole site. A part whose cells run out before it has its records stops there, so that a census
/// of a few cells takes all their records ([`ClassCensus::scanned_total`] says how many).
///
/// # Panics
///
/// If `band` is substellar or the site's cells leave the root cube.
#[must_use]
pub fn class_census_in(
    galaxy: &Galaxy,
    band: MassBand,
    site: CensusSite,
    whose: CensusRecords,
    records: u64,
    part: CensusPart,
) -> ClassCensus {
    assert!(band.is_stellar(), "a census runs a stellar layer");
    let layer = Layer::from(band);
    let (first, [nx, ny, nz]) = site.cells(layer);
    // The product cannot overflow: `cells` checks it.
    let n = nx * ny * nz;
    let stride = coprime_stride(n);
    let mut scanned = [0_u64; 7];
    let mut members = Vec::new();
    let mut cell = Vec::new();
    let mut ran = 0_u64;
    let mut position = part.index;
    while ran < records && position < n {
        let linear = u64::try_from(u128::from(position) * u128::from(stride) % u128::from(n))
            .expect("a residue below a u64 count");
        let offset = |value: u64| i32::try_from(value).expect("a site's cell offset fits an i32");
        let key = CellKey::new(
            layer,
            [
                first[0] + offset(linear % nx),
                first[1] + offset(linear / nx % ny),
                first[2] + offset(linear / (nx * ny)),
            ],
        )
        .expect("a census site's cells lie inside the root cube");
        position += part.count;
        generate_cell(galaxy, key, &mut cell);
        for record in &cell {
            if ran == records {
                break;
            }
            if !whose.takes(record.population()) {
                continue;
            }
            scanned[record.population().index()] += 1;
            ran += 1;
            members_of(galaxy, record, &CENSUS_CLASSES, &mut members);
        }
    }
    ClassCensus {
        band,
        scanned,
        members,
    }
}

/// The stride a census walks `n` cells by: ⌊φ⁻¹ n⌋, lowered until it is prime to `n`, so that it
/// meets every cell once in `n` steps.
#[must_use]
fn coprime_stride(n: u64) -> u64 {
    const INVERSE_GOLDEN: u128 = 0x9e37_79b9_7f4a_7c15;
    let mut stride = u64::try_from((u128::from(n) * INVERSE_GOLDEN) >> 64)
        .expect("a fraction of a u64 count")
        .max(1);
    while gcd(stride, n) != 1 {
        stride -= 1;
    }
    stride
}

/// The greatest common divisor of `a` and `b`.
#[must_use]
const fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// The members of `classes` among the first attempt's pairs of grid record `record`, of a stellar
/// layer.
fn members_of(
    galaxy: &Galaxy,
    record: &SystemRecord,
    classes: &[CarvedClass],
    out: &mut Vec<ClassMember>,
) {
    let composition = draw_metallicity(galaxy, record);
    let age = record.age_at_epoch();
    let primary = StarModel::new_under(
        record.primary_initial_mass(),
        composition,
        primary_draws(galaxy, record),
        age,
        galaxy.rules().stellar,
    )
    .expect("a stellar layer's primary is a star with a finite age");
    let stars = SystemStars::at_attempt(
        galaxy,
        record,
        &primary,
        MultiplicityContext::Free,
        RedrawAttempt::FIRST,
    );
    for pair in stars.pairs() {
        let timeline = pair.timeline();
        let Some(class) = carved_class(timeline, age).filter(|c| classes.contains(c)) else {
            continue;
        };
        let initial = pair
            .stars()
            .map(|star| stars.stars()[usize::from(star.get())].initial_mass());
        out.push(ClassMember {
            record: *record,
            class,
            timeline: timeline.clone(),
            accreting: accreting_marks(timeline, age, initial),
            xray: (class == CarvedClass::XrayBinary)
                .then(|| xray_marks(timeline, age, initial))
                .flatten(),
        });
    }
}

/// An accreting white dwarf's marks where [`carved_class`] reads them, the pair's stars of initial
/// masses `initial` in the engine's order, or `None` for a pair whose white dwarf no age of the
/// horizon feeds.
pub(super) fn accreting_marks(
    timeline: &BinaryTimeline,
    age_at_epoch: Years,
    initial: [SolarMasses; 2],
) -> Option<AccretingMarks> {
    let (age, accretor, rate) = white_dwarf_feeding(timeline, age_at_epoch)?;
    let state = timeline.state_at(age);
    let a = accretor.index();
    let [white_dwarf, donor] = [&state.stars()[a], &state.stars()[1 - a]];
    let magnetic = timeline
        .context()
        .draws(a)
        .magnetism()
        .is_below(Threshold::from_probability(MAGNETIC_CV_SHARE));
    Some(AccretingMarks {
        age,
        white_dwarf_mass: white_dwarf.mass(),
        donor_mass: donor.mass(),
        donor_phase: donor.phase(),
        period: state.orbit().map(KeplerElements::period),
        transfer_rate: rate,
        recurrence: nova_recurrence_under(
            white_dwarf.mass(),
            rate,
            donor.phase(),
            timeline.context().rules(),
        ),
        magnetic,
        white_dwarf_initial_mass: initial[a],
        donor_initial_mass: initial[1 - a],
    })
}

/// An X-ray binary's marks where [`carved_class`] reads them, the pair's stars of initial masses
/// `initial` in the engine's order, or `None` for a pair that no age of the horizon makes one.
pub(super) fn xray_marks(
    timeline: &BinaryTimeline,
    age_at_epoch: Years,
    initial: [SolarMasses; 2],
) -> Option<XrayMarks> {
    let (age, class) = xray_binary_reading(timeline, age_at_epoch)?;
    let state = timeline.state_at(age);
    let compact = |p: Phase| matches!(p, Phase::NeutronStar | Phase::BlackHole);
    let stars = state.stars();
    let a = usize::from(!compact(stars[0].phase()) && compact(stars[1].phase()));
    let [accretor, donor] = [&stars[a], &stars[1 - a]];
    let since_supernova = timeline
        .supernovae()
        .iter()
        .rfind(|s| s.age() <= age)
        .map(|s| Years::new(age.value() - s.age().value()));
    Some(XrayMarks {
        age,
        class,
        accretor_phase: accretor.phase(),
        accretor_mass: accretor.mass(),
        donor_mass: donor.mass(),
        donor_phase: donor.phase(),
        period: state.orbit().map(KeplerElements::period),
        eccentricity: state.orbit().map(KeplerElements::eccentricity),
        transfer_rate: state.transfer_rate(),
        recoil_speed: timeline.recoil().map(|v| v.speed()),
        since_supernova,
        accretor_initial_mass: initial[a],
        donor_initial_mass: initial[1 - a],
        rate_excess_dex: state
            .transfer_rate()
            .zip(state.orbit())
            .map(|(rate, orbit)| {
                let line = xrb_line(accretor.mass(), donor.mass(), orbit.semi_major_axis());
                math::log10(rate.value() / line.value())
            }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An X-ray binary member of `class` whose compact star and donor are of `accretor` and
    /// `donor`, the rest of its marks fixed.
    fn xray(class: BinaryClass, accretor: Phase, donor: Phase) -> XrayMarks {
        XrayMarks {
            age: Years::new(1.0e9),
            class,
            accretor_phase: accretor,
            accretor_mass: SolarMasses::new(1.4),
            donor_mass: SolarMasses::new(0.5),
            donor_phase: donor,
            period: None,
            eccentricity: None,
            transfer_rate: None,
            recoil_speed: None,
            since_supernova: None,
            accretor_initial_mass: SolarMasses::new(15.0),
            donor_initial_mass: SolarMasses::new(1.0),
            rate_excess_dex: None,
        }
    }

    /// Every X-ray binary member has a kind: a held one where those hold it, else an ultracompact
    /// (a white dwarf's or a helium star's matter), a symbiotic one, or a black hole beside a
    /// massive star; marks of another class have none.
    #[test]
    fn every_x_ray_binary_member_has_a_kind() {
        use XrbHostKind::{BlackHoleHighMass, Symbiotic, Ultracompact};
        let (ns, bh) = (Phase::NeutronStar, Phase::BlackHole);
        let lmxb = |kind| BinaryClass::LowMassXrayBinary(kind);
        let hmxb = |kind| BinaryClass::HighMassXrayBinary(kind);
        let cases = [
            (
                lmxb(XrbKind::Persistent),
                ns,
                Phase::MainSequence,
                XrbHostKind::NeutronStarPersistent,
            ),
            (
                lmxb(XrbKind::Transient),
                bh,
                Phase::FirstGiantBranch,
                XrbHostKind::BlackHoleTransient,
            ),
            (
                lmxb(XrbKind::Transient),
                ns,
                Phase::HeliumWhiteDwarf,
                Ultracompact,
            ),
            (
                lmxb(XrbKind::Persistent),
                bh,
                Phase::CarbonOxygenWhiteDwarf,
                Ultracompact,
            ),
            (
                lmxb(XrbKind::Persistent),
                ns,
                Phase::HeliumMainSequence,
                Ultracompact,
            ),
            (
                lmxb(XrbKind::Symbiotic),
                ns,
                Phase::FirstGiantBranch,
                Symbiotic,
            ),
            (lmxb(XrbKind::Symbiotic), bh, Phase::EarlyAgb, Symbiotic),
            (
                hmxb(HmxbKind::BeX),
                ns,
                Phase::MainSequence,
                XrbHostKind::BeX,
            ),
            (
                hmxb(HmxbKind::Supergiant),
                ns,
                Phase::CoreHeliumBurning,
                XrbHostKind::Supergiant,
            ),
            (
                hmxb(HmxbKind::BeX),
                bh,
                Phase::MainSequence,
                BlackHoleHighMass,
            ),
            (
                hmxb(HmxbKind::Supergiant),
                bh,
                Phase::HeliumMainSequence,
                BlackHoleHighMass,
            ),
        ];
        for (class, accretor, donor, kind) in cases {
            let marks = xray(class, accretor, donor);
            assert_eq!(
                marks.member_kind(),
                Some(kind),
                "{class:?} {accretor:?} {donor:?}"
            );
            let held = XrbHostKind::LOW_MASS_HELD.contains(&kind)
                || XrbHostKind::HIGH_MASS_HELD.contains(&kind);
            assert_eq!(marks.host_kind(), held.then_some(kind));
        }
        assert_eq!(
            xray(
                BinaryClass::CataclysmicVariable(crate::stellar::binary::CvKind::DwarfNova),
                ns,
                Phase::MainSequence
            )
            .member_kind(),
            None
        );
    }

    /// The stride is prime to the cells' count, so that a census's walk meets every cell once in
    /// as many steps, and the parts of one count share no cell and together hold them all.
    #[test]
    fn a_census_walk_meets_every_cell_once_and_its_parts_share_none() {
        for n in [1_u64, 2, 3, 16, 97, 1_000, 4_096, 123_456] {
            let stride = coprime_stride(n);
            assert_eq!(gcd(stride, n), 1, "{n}");
            let mut seen = vec![false; usize::try_from(n).unwrap()];
            for position in 0..n {
                let cell =
                    usize::try_from(u128::from(position) * u128::from(stride) % u128::from(n))
                        .unwrap();
                assert!(!seen[cell], "{n}: cell {cell} met twice");
                seen[cell] = true;
            }
            assert!(seen.iter().all(|&s| s), "{n}");
        }
        let count = 8;
        let mut owner = [None; 100];
        for index in 0..count {
            let part = CensusPart::new(index, count);
            let mut position = part.index;
            while position < 100 {
                let slot = &mut owner[usize::try_from(position).unwrap()];
                assert!(slot.is_none(), "position {position} in two parts");
                *slot = Some(index);
                position += part.count;
            }
        }
        assert!(owner.iter().all(Option::is_some));
    }

    /// The walk of [`brute_force_class_members_in`] as it was before [`MemberCells`]: each cell it
    /// meets opened and its records run through the engine, every time.
    fn opening_every_cell(
        galaxy: &Galaxy,
        class: CarvedClass,
        n: usize,
        stream: u64,
    ) -> Vec<ClassMember> {
        let row = CLASS_SHARES.row(class);
        let mut members = Vec::with_capacity(n + 8);
        let mut cell = Vec::new();
        for (index, band) in (0_u64..).zip(MassBand::ALL) {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                clippy::cast_precision_loss,
                reason = "a member count, rounded, of a test's few"
            )]
            let wanted = (n as f64 * row.layer_share(band)).round() as usize;
            if wanted == 0 {
                continue;
            }
            let layer = Layer::from(band);
            let mut mix = Mix(0x0b17_08a0 ^ (stream << 8) ^ index);
            let size = i32::try_from(layer.cell_size_ly()).unwrap();
            let (along, across) = ((4_000 / size).max(1), (2_000 / size).max(1));
            let start = members.len();
            while members.len() - start < wanted {
                let key = CellKey::new(
                    layer,
                    [mix.within(along), 26_000 / size + mix.within(across), 0],
                )
                .unwrap();
                generate_cell(galaxy, key, &mut cell);
                for record in cell.iter().take(8) {
                    members_of(galaxy, record, &[class], &mut members);
                }
            }
            members.truncate(start + wanted);
        }
        members
    }

    /// A draw from kept cells is the draw that opens every cell it meets, member for member: a
    /// draw of one slow accreting white dwarf fills some cells, and draws of two on the same
    /// stream, which meets those cells again first, and of one on another stream read them; and a
    /// kept cell's members of each carved class are that class's alone.
    #[test]
    #[ignore = "slow: some thousands of grid pairs run through the binary engine, twice"]
    fn a_draw_from_kept_cells_is_the_draw_that_opens_every_cell() {
        let galaxy = Galaxy::from_params(
            crate::rng::Seed::new(0x0b17_08a0),
            crate::galaxy::params::GalaxyParams::milky_way_like(),
        )
        .unwrap();
        let class = CarvedClass::AccretingWdSlow;
        let cells = MemberCells::new(&galaxy);
        let first = cells.members_in(class, 1, 0);
        let kept = cells.kept().len();
        for (n, stream) in [(2, 0), (1, 1)] {
            let members = cells.members_in(class, n, stream);
            assert!(!members.is_empty(), "{n} on stream {stream}");
            assert_eq!(
                members,
                opening_every_cell(&galaxy, class, n, stream),
                "{n} on stream {stream}"
            );
        }
        assert_eq!(first, opening_every_cell(&galaxy, class, 1, 0));
        assert!(
            cells.kept().len() > kept,
            "the later draws open cells of their own"
        );
        // A kept cell serves a draw of any class: its members of each are those a draw of that
        // class alone finds, in their order, in the cells that hold one and in some that hold none.
        let keys: Vec<CellKey> = {
            let kept = cells.kept();
            let holding = kept.iter().filter(|(_, m)| !m.is_empty()).map(|(k, _)| *k);
            holding.chain(kept.keys().take(8).copied()).collect()
        };
        assert!(keys.len() > 8, "some kept cell holds a member");
        let mut records = Vec::new();
        for key in keys {
            let cell = cells.cell(key);
            generate_cell(&galaxy, key, &mut records);
            for class in KEPT_CLASSES {
                let mut alone = Vec::new();
                for record in records.iter().take(8) {
                    members_of(&galaxy, record, &[class], &mut alone);
                }
                let kept: Vec<ClassMember> =
                    cell.iter().filter(|m| m.class == class).cloned().collect();
                assert_eq!(kept, alone, "{key:?} {class:?}");
            }
        }
    }

    /// A site's cells cover its box in each layer, its first cell the one that holds its low
    /// corner, rounding down below zero.
    #[test]
    fn a_site_s_cells_cover_its_box() {
        let site = CensusSite::new([-100, 26_000, 0], [100, 0, 64]);
        let (first, counts) = site.cells(Layer::D);
        assert_eq!(first, [-4, 406, -1]);
        assert_eq!(counts, [5, 1, 3]);
        assert!(CensusRecords::All.takes(Population::Halo));
        assert!(!CensusRecords::Of(Population::Bulge).takes(Population::Halo));
    }
}
