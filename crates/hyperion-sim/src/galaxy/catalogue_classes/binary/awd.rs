//! The accreting white dwarf classes (plan 11, P11.T8.a; design notes 10–13): every white dwarf
//! fed by a companion, split into the fast hosts (`ACCRETING_WHITE_DWARF`, 6), whose novae recur
//! within L, and the slow (`ACCRETING_WHITE_DWARF_SLOW`, 8), placed on 512 ly cells of their own.
//!
//! The grid redraws every binary that falls into a carved class (P11.T7), so these hosts are
//! nowhere in the cells, and the share matrix gives up what the classes hold: each class's row of
//! [`CLASS_SHARES`](crate::tables::binary::CLASS_SHARES) (hosts per solar mass formed, by
//! population). A class is a Poisson process of that intensity, placed by thinning on the class
//! grid of plan 09 ([`ClassProcess`]), as P09.T33.a places the core collapses. From generator
//! version 27 both read plan 15's fitted tables in place of the scratch ones, each class its own
//! sampler (the hold of [`BinaryTableRules`](super::BinaryTableRules)).
//!
//! # The envelope and a candidate
//!
//! The intensity at x is `Σ_c w_c n_c(x)`: component c's density of systems, times its hosts per
//! system, `w_c` = the row's hosts per solar mass formed in its population × the mass formed per
//! system × its born fraction, so that the hosts over the whole component are the row's rate × its
//! born systems' formed mass, as the share matrix counts them; from generator version 27 times the
//! share the catalogue keeps, all but the features' (P11.T8.f, `feature::catalogue_share`), which
//! the features' lists hold. A cell's bound is `Σ_c w_c × bound_c`, each component's density bound
//! over the cell's closed box. A candidate, on its own streams (plan 09's design note 23):
//!
//! 1. draws its scan marks first (`stellar::binary::scan`): its site uniformly in the cell,
//!    thinned on the intensity there ÷ the bound, the same mark picking its component
//!    (`class.awd.site`); its nova recurrence period on its class's side of L, and the white
//!    dwarf's mass given it, from which the transfer rate follows (`class.awd.recurrence`); its
//!    velocity at the site, plan 08's draw of its component's law (`class.awd.velocity`), the one
//!    velocity its entry moves at (rule 6); and its nova series, clock and, for a slow host, its
//!    eruptions in the source horizon;
//! 2. draws its binary now: its donor's kind, period and mass, and its orbit's orientation and
//!    phase (`class.awd.binary`);
//! 3. draws its history from the table given the present state: its primary's and its donor's
//!    initial masses and its age (`class.awd.history`), the age raised to at least
//!    [`LIFETIME_MARGIN`] × the primary's lifetime (plan 06's), so that its white dwarf has formed;
//! 4. draws its \[Fe/H\] from its component's law at the site and the drawn age
//!    (`class.awd.metallicity`).
//!
//! A feature's list entry (P11.T8.f) draws the same marks after its site, keyed by its own ID, but
//! takes its feature's velocity and composition and an age drawn in its members' ages, and is
//! thinned where its white dwarf's progenitor cannot have died by then.
//!
//! Every draw after the scan marks always succeeds, so the scan keeps exactly the class's
//! entries.
//!
//! # The test
//!
//! The candidate's marks define its [`BinaryTimeline`] without the engine
//! ([`BinaryTimeline::from_marks`]): a detached history, then stable transfer from the donor onto
//! the white dwarf across the whole source horizon, each star held at its marked state. The
//! candidate is an entry if and only if [`carved_class`] of that timeline is its class, the one
//! test the grid applies to an evolved pair (design note 10). It can refuse only a candidate
//! whose recurrence, recomputed from the white dwarf's mass and the rate, lands on the other side
//! of L, which the scan refuses too.
//!
//! # The entry
//!
//! An entry has one member, its system, at its site at the epoch and moving at its velocity on a
//! straight line: over the source horizon, L + 2H, a host below 4,600 km/s stays within plan 09's
//! 4,096 ly reach of its cell. Its record is a catalogue system's, counted in its primary's
//! layer, C or D; its stars are the pair: each star's own model at its initial mass and the
//! system's age, and the pair's timeline, which gives both stars' states
//! ([`AwdEntry::stars`]).

use super::super::grid::{CatalogueMember, ClassProcess, site_stream};
use super::super::ids::CatalogueCellKey;
use super::super::registry::ClassId;
use super::super::system::{CatalogueMotion, CatalogueSystem};
use super::feature::catalogue_share;
use crate::coords::GalacticPosition;
use crate::galaxy::bounds::CellBox;
use crate::galaxy::fields::{ComponentId, MAX_COMPONENTS};
use crate::galaxy::placement::SystemRecord;
use crate::galaxy::{Galaxy, PointLy, Population};
use crate::id::{BodyId, CatalogueSystemId};
use crate::math;
use crate::orbit::{Eccentricity, KeplerElements, Orientation, roche_lobe_radius};
use crate::rng::{Stream, tags};
use crate::stellar::binary::sampler::ClassSampler;
use crate::stellar::binary::scan::{AwdScanMarks, scan_marks};
use crate::stellar::binary::{
    BinaryMarks, BinaryTimeline, CarvedClass, Component, MarkedPhase, SegmentKind, carved_class,
};
use crate::stellar::draws::StarDraws;
use crate::stellar::multiplicity::{SlotKind, SystemHierarchy};
use crate::stellar::remnant::RemnantRecipe;
use crate::stellar::remnant::structure::white_dwarf_radius;
use crate::stellar::sse::{ZCoeffs, zams};
use crate::stellar::system::{StarModel, SystemStars, track_mass_of};
use crate::stellar::{Composition, Phase, StarState, StarStateParts, lifetime};
use crate::tables::binary::{AwdDonorKind, AwdMark, AwdPick};
use crate::time::{CLOCK_WINDOW_H, LIGHT_CROSSING_L};
use crate::units::consts::{SOLAR_EFFECTIVE_TEMPERATURE_K, SOLAR_RADIUS_M};
use crate::units::{
    GravitationalParameter, PerCubicLightYear, Radians, Seconds, SolarLuminosities, SolarMasses,
    SolarMassesPerYear, SolarRadii, Years,
};

/// The least a host's age is over its primary's main-sequence-and-giant lifetime (plan 06's
/// [`lifetime`]): 1.05, ours, so that its white dwarf has formed before the epoch whatever the
/// table's age drew. P15.T10.b's fitted history replaces it.
pub const LIFETIME_MARGIN: f64 = 1.05;

/// The least white dwarf mass a host's white dwarf is of oxygen and neon rather than carbon and
/// oxygen, M☉: 1.1, about where super-AGB stars' white dwarfs begin, ≈ 1.06 M☉ (Doherty et al.
/// 2015, MNRAS 446, 2599, section 5). Only the white dwarf's phase reads it, and no class test
/// distinguishes the two below the steady-burning rate.
pub const OXYGEN_NEON_MIN_MASS: SolarMasses = SolarMasses::new(1.1);

/// The effective temperature of a helium donor, K: 10⁴, ours, provisional; an AM Canum
/// Venaticorum star's donor is too faint to measure in most systems.
pub const HELIUM_DONOR_TEMPERATURE_K: f64 = 1.0e4;

/// The effective temperature of a giant donor, K: 3,600, an M3 giant's, the symbiotic stars'
/// typical class (Mikołajewska 2012, Baltic Astronomy 21, 5; recalled, not read at source).
pub const GIANT_DONOR_TEMPERATURE_K: f64 = 3_600.0;

/// The helium core of a giant donor, M☉: 0.35, ours, a first-giant-branch core of a red giant
/// as large as a symbiotic star's, held under 0.9 of the giant's mass.
pub const GIANT_DONOR_CORE_MASS: SolarMasses = SolarMasses::new(0.35);

/// The least initial mass of a donor: 0.08 M☉, the stellar floor of plan 11's companions.
const DONOR_INITIAL_MIN: SolarMasses = SolarMasses::new(0.08);

/// Which of the two accreting white dwarf classes (design note 12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AwdSide {
    /// The fast hosts, whose novae recur within L: [`ClassId::ACCRETING_WHITE_DWARF`].
    Fast,
    /// The slow hosts, whose novae recur more slowly than L:
    /// [`ClassId::ACCRETING_WHITE_DWARF_SLOW`].
    Slow,
}

impl AwdSide {
    /// The class's value in plan 09's registry.
    #[must_use]
    pub const fn class(self) -> ClassId {
        match self {
            Self::Fast => ClassId::ACCRETING_WHITE_DWARF,
            Self::Slow => ClassId::ACCRETING_WHITE_DWARF_SLOW,
        }
    }

    /// The carved class the grid and the catalogue share for it.
    #[must_use]
    pub const fn carved(self) -> CarvedClass {
        match self {
            Self::Fast => CarvedClass::AccretingWdFast,
            Self::Slow => CarvedClass::AccretingWdSlow,
        }
    }
}

/// One density component as an accreting white dwarf class thins on it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AwdComponent {
    id: ComponentId,
    hosts_per_system: f64,
}

impl AwdComponent {
    /// The component.
    #[must_use]
    pub const fn id(&self) -> ComponentId {
        self.id
    }

    /// Its hosts per system of its density: the class's hosts per solar mass formed in its
    /// population × the mass formed per system × its born fraction.
    #[must_use]
    pub const fn hosts_per_system(&self) -> f64 {
        self.hosts_per_system
    }
}

/// What the two accreting white dwarf classes share: the components they thin on, their side of
/// L, and the scan marks' draws (module documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct AwdProcess {
    side: AwdSide,
    components: Vec<AwdComponent>,
    sampler: ClassSampler<AwdMark, AwdPick>,
    recurrence_ranks: (f64, f64),
}

impl AwdProcess {
    /// The class of `side` in `galaxy`: its components, from its row of the share table, and its
    /// side's ranks of its sampler's recurrence. Below version 27 the table is
    /// [`CLASS_SHARES`](crate::tables::binary::CLASS_SHARES) and the sampler
    /// [`AWD_SAMPLER`](crate::tables::binary::AWD_SAMPLER), split at L; from it, plan 15's fitted
    /// tables, each side's sampler with its recurrence all on its side (the module's hold).
    #[must_use]
    pub fn new(galaxy: &Galaxy, side: AwdSide) -> Self {
        let tables = galaxy.rules().binary_tables;
        let row = tables.class_shares().row(side.carved());
        let formed = galaxy.mean_formed_mass().value();
        let fields = galaxy.fields();
        let components = fields
            .component_ids()
            .filter_map(|id| {
                let component = fields.component(id);
                let hosts = row.hosts_per_formed_mass(component.population())
                    * formed
                    * component.ages().born_fraction()
                    * catalogue_share(galaxy, row, component);
                (hosts > 0.0).then_some(AwdComponent {
                    id,
                    hosts_per_system: hosts,
                })
            })
            .collect();
        let sampler = ClassSampler::new(tables.awd_sampler(side));
        let split = sampler.rank_of(
            AwdMark::Recurrence,
            0.0,
            math::log10(LIGHT_CROSSING_L.as_julian_years_f64()),
        );
        let recurrence_ranks = match side {
            AwdSide::Fast => (0.0, split),
            AwdSide::Slow => (split, 1.0),
        };
        Self {
            side,
            components,
            sampler,
            recurrence_ranks,
        }
    }

    /// The class's side of L.
    #[must_use]
    pub const fn side(&self) -> AwdSide {
        self.side
    }

    /// The class in plan 09's registry.
    #[must_use]
    pub const fn class(&self) -> ClassId {
        self.side.class()
    }

    /// The components it thins on, in component order.
    #[must_use]
    pub fn components(&self) -> &[AwdComponent] {
        &self.components
    }

    /// The sampler the class draws from: [`AWD_SAMPLER`](crate::tables::binary::AWD_SAMPLER), or
    /// from version 27 its side's fitted one.
    #[must_use]
    pub const fn sampler(&self) -> ClassSampler<AwdMark, AwdPick> {
        self.sampler
    }

    /// The ranks of the sampler's recurrence on the class's side of L.
    #[must_use]
    pub const fn recurrence_ranks(&self) -> (f64, f64) {
        self.recurrence_ranks
    }

    /// The class's expected hosts at `point`, per cubic light-year: the intensity `Σ_c w_c n_c`,
    /// before the class's test.
    #[must_use]
    pub fn hosts_density(&self, galaxy: &Galaxy, point: &PointLy) -> PerCubicLightYear {
        let mut weights = [0.0; MAX_COMPONENTS];
        let n = self.weights(galaxy, point, &mut weights);
        PerCubicLightYear::new(weights[..n].iter().fold(0.0, |sum, w| sum + w))
    }

    /// The class's expected hosts in the whole galaxy, before its test: each component's hosts
    /// per system × the integral of its density.
    #[must_use]
    pub fn expected_hosts(&self, galaxy: &Galaxy) -> f64 {
        let fields = galaxy.fields();
        self.components.iter().fold(0.0, |sum, c| {
            sum + c.hosts_per_system * fields.component(c.id).count_with_unborn()
        })
    }

    /// Σ, over the components in order, its hosts per system × its density bound over the cell's
    /// closed box: each term bounds the matching weight of the site's thinning.
    ///
    /// # Panics
    ///
    /// Never: a class cell is a cell box of the root cube that straddles no plane.
    #[must_use]
    pub fn bound(&self, galaxy: &Galaxy, cell: &CatalogueCellKey) -> PerCubicLightYear {
        let cube = CellBox::new(cell.origin_ly(), cell.edge_ly())
            .expect("a class cell is a cell box of the root cube");
        let mut bounds = [0.0; MAX_COMPONENTS];
        galaxy.fields().component_bounds(&cube, &mut bounds);
        PerCubicLightYear::new(self.components.iter().fold(0.0, |sum, c| {
            sum + bounds[c.id.index()] * c.hosts_per_system
        }))
    }

    /// Candidate `id`'s site in `cell`, thinned on the intensity there ÷ `bound`, with the
    /// component the same mark picks (`class.awd.site`), or `None` if the thinning rejects it.
    #[must_use]
    pub(crate) fn site(
        &self,
        galaxy: &Galaxy,
        cell: &CatalogueCellKey,
        id: CatalogueSystemId,
        bound: PerCubicLightYear,
    ) -> Option<(GalacticPosition, ComponentId)> {
        assert_eq!(
            cell.class(),
            self.class(),
            "an accreting white dwarf candidate is of its own class's cell"
        );
        if bound.value().is_nan() || bound.value() <= 0.0 {
            return None;
        }
        let (site, mut stream) = site_stream(galaxy, cell, id, tags::CLASS_AWD_SITE);
        let mut weights = [0.0; MAX_COMPONENTS];
        let n = self.weights(galaxy, &PointLy::from(&site), &mut weights);
        let picked = stream.mark().pick_weighted(&weights[..n], bound.value())?;
        Some((site, self.components[picked].id))
    }

    /// The intensity's terms at `point` into `out`, in the components' order, hosts per cubic
    /// light-year.
    fn weights(&self, galaxy: &Galaxy, point: &PointLy, out: &mut [f64; MAX_COMPONENTS]) -> usize {
        let fields = galaxy.fields();
        for (slot, c) in out.iter_mut().zip(&self.components) {
            *slot = c.hosts_per_system * fields.component(c.id).density(point);
        }
        self.components.len()
    }

    /// Candidate `id` of `cell`: its scan marks, then its binary, history and composition.
    fn candidate(
        &self,
        galaxy: &Galaxy,
        cell: &CatalogueCellKey,
        id: CatalogueSystemId,
        bound: PerCubicLightYear,
    ) -> Option<AwdCandidate> {
        let scan = scan_marks(galaxy, self, cell, id, bound)?;
        self.candidate_of(galaxy, &scan)
    }

    /// The candidate whose scan marks are `scan`, a catalogue candidate's or a feature's list
    /// entry's (P11.T8.f): its binary, history and composition after them. A list entry's age is
    /// drawn in its members' ages and its composition is theirs, and the candidate is `None` if
    /// its white dwarf's progenitor cannot have died by then.
    #[must_use]
    pub(crate) fn candidate_of(
        &self,
        galaxy: &Galaxy,
        scan: &AwdScanMarks,
    ) -> Option<AwdCandidate> {
        let seed = galaxy.seed();
        let system = scan.id();
        let key = scan.host().key();
        let sampler = self.sampler;
        // 2. The binary now.
        let mut stream = Stream::open(seed, tags::CLASS_AWD_BINARY, key);
        let donor_kind = AwdDonorKind::ALL[sampler.pick(AwdPick::DonorKind, &mut stream)];
        let (period_mark, mass_mark) = match donor_kind {
            AwdDonorKind::MainSequence => {
                (AwdMark::MainSequencePeriod, AwdMark::MainSequenceDonorMass)
            }
            AwdDonorKind::Helium => (AwdMark::HeliumPeriod, AwdMark::HeliumDonorMass),
            AwdDonorKind::Giant => (AwdMark::GiantPeriod, AwdMark::GiantDonorMass),
        };
        let log_rate = math::log10(scan.transfer_rate().value());
        let log_period_hours = sampler.draw(period_mark, log_rate, &mut stream);
        let log_donor = sampler.draw(mass_mark, log_period_hours, &mut stream);
        let orientation = Orientation::new(
            Radians::new(math::acos(2.0 * stream.uniform() - 1.0)),
            Radians::new(std::f64::consts::TAU * stream.uniform()),
            Radians::new(std::f64::consts::TAU * stream.uniform()),
        )
        .expect("an inclination in [0, π] and angles in [0, 2π) are an orientation");
        let mean_anomaly = Radians::new(std::f64::consts::TAU * stream.uniform());
        // 3. The history, given the present.
        let mut stream = Stream::open(seed, tags::CLASS_AWD_HISTORY, key);
        let white_dwarf_mass = scan.white_dwarf_mass();
        let log_m0 = sampler.draw(
            AwdMark::PrimaryInitialMass,
            white_dwarf_mass.value(),
            &mut stream,
        );
        let log_m2_0 = sampler.draw(AwdMark::DonorInitialMass, log_donor, &mut stream);
        let log_age = sampler.draw(AwdMark::Age, log_m0, &mut stream);
        let m0 = math::exp10(log_m0);
        let donor_mass = math::exp10(log_donor);
        let m2_0 = math::exp10(log_m2_0)
            .max(donor_mass.max(DONOR_INITIAL_MIN.value()))
            .min(m0);
        // 4. The composition, at the site and the drawn age; a list entry's members' age and
        // composition.
        let host = scan.host();
        let (composition, drawn_age) = match host.listed_host() {
            None => {
                let drawn = Years::new(math::exp10(log_age));
                let composition = host.composition_at(galaxy, drawn, tags::CLASS_AWD_METALLICITY);
                (composition, drawn.value())
            }
            Some(listed) => (
                *listed.composition(),
                listed.age_at(stream.uniform()).value(),
            ),
        };
        // The stars' own draws, and an age at which the white dwarf has formed.
        let draws = [0, 1].map(|body| StarDraws::for_star(seed, BodyId::new(system, body)));
        let life = lifetime(track_mass_of(SolarMasses::new(m0)), &composition, &draws[0]).value();
        let age = match host.listed_host() {
            None => drawn_age.max(LIFETIME_MARGIN * life),
            Some(_) if drawn_age < LIFETIME_MARGIN * life => return None,
            Some(_) => drawn_age,
        };
        let history = AwdHistory {
            primary_initial_mass: SolarMasses::new(m0),
            donor_initial_mass: SolarMasses::new(m2_0),
            age_at_epoch: Years::new(age),
        };
        let binary = AwdBinary::marked(
            scan,
            donor_kind,
            SolarMasses::new(donor_mass),
            log_period_hours,
            (orientation, mean_anomaly),
            &composition,
            history.age_at_epoch,
        );
        let marks = binary.marks(composition, draws, history.age_at_epoch);
        Some(AwdCandidate {
            scan: *scan,
            population: host.population(galaxy),
            composition,
            binary,
            history,
            marks,
        })
    }

    /// The class's test on `candidate`: its timeline from its marks, kept if [`carved_class`]
    /// gives its class.
    #[must_use]
    pub(crate) fn test(&self, candidate: AwdCandidate) -> Option<AwdEntry> {
        let AwdCandidate {
            scan,
            population,
            composition,
            binary,
            history,
            marks,
        } = candidate;
        let timeline = BinaryTimeline::from_marks(marks);
        if carved_class(&timeline, history.age_at_epoch) != Some(self.side.carved()) {
            return None;
        }
        let record = SystemRecord::from_parts(
            scan.id(),
            *scan.position(),
            scan.host().origin(self.class()),
            population,
            history.primary_initial_mass,
            history.age_at_epoch,
        );
        Some(AwdEntry {
            record,
            motion: CatalogueMotion::straight(*scan.position(), scan.velocity()),
            scan,
            composition,
            binary,
            history,
            timeline,
        })
    }
}

/// An accreting white dwarf candidate the scan kept, with every mark its test reads.
#[derive(Debug, Clone, PartialEq)]
pub struct AwdCandidate {
    scan: AwdScanMarks,
    population: Population,
    composition: Composition,
    binary: AwdBinary,
    history: AwdHistory,
    marks: BinaryMarks,
}

/// An accreting white dwarf's binary now (P11.T8.a): the two stars' marked states, the orbit and
/// the transfer rate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AwdBinary {
    donor_kind: AwdDonorKind,
    white_dwarf: StarState,
    donor: StarState,
    orbit: KeplerElements,
    transfer_rate: SolarMassesPerYear,
}

impl AwdBinary {
    /// The donor's kind.
    #[must_use]
    pub const fn donor_kind(&self) -> AwdDonorKind {
        self.donor_kind
    }

    /// The white dwarf's marked state, component 0.
    #[must_use]
    pub const fn white_dwarf(&self) -> &StarState {
        &self.white_dwarf
    }

    /// The donor's marked state, component 1, filling its Roche lobe.
    #[must_use]
    pub const fn donor(&self) -> &StarState {
        &self.donor
    }

    /// The relative orbit, circular.
    #[must_use]
    pub const fn orbit(&self) -> &KeplerElements {
        &self.orbit
    }

    /// The rate of transfer from the donor onto the white dwarf, M☉ yr⁻¹.
    #[must_use]
    pub const fn transfer_rate(&self) -> SolarMassesPerYear {
        self.transfer_rate
    }

    /// The binary of `scan`'s white dwarf and rate with a donor of `donor_kind` and
    /// `donor_mass`, on a circular orbit of 10^`log_period_hours` hours oriented and phased by
    /// `angles`, its stars `age` old.
    fn marked(
        scan: &AwdScanMarks,
        donor_kind: AwdDonorKind,
        donor_mass: SolarMasses,
        log_period_hours: f64,
        (orientation, mean_anomaly): (Orientation, Radians),
        composition: &Composition,
        age: Years,
    ) -> Self {
        let wd_mass = scan.white_dwarf_mass();
        let rate = scan.transfer_rate();
        let period = Seconds::new(math::exp10(log_period_hours) * 3_600.0);
        let orbit = KeplerElements::from_period(
            period,
            GravitationalParameter::from_solar_masses(wd_mass + donor_mass),
            Eccentricity::CIRCULAR,
            orientation,
            mean_anomaly,
        )
        .expect("a period of minutes to years about a solar mass or two is an orbit");
        let lobe = SolarRadii::new(
            roche_lobe_radius(
                donor_mass.value() / wd_mass.value(),
                orbit.semi_major_axis(),
            )
            .value()
                / SOLAR_RADIUS_M,
        );
        Self {
            donor_kind,
            white_dwarf: white_dwarf_state(wd_mass, rate, age),
            donor: donor_state(donor_kind, donor_mass, lobe, composition, age),
            orbit,
            transfer_rate: rate,
        }
    }

    /// The marks of this binary for a system of `composition` and the stars' `draws`, `age` old
    /// at the epoch: stable transfer from the donor across the source horizon about the epoch,
    /// after a detached history (design note 10).
    fn marks(&self, composition: Composition, draws: [StarDraws; 2], age: Years) -> BinaryMarks {
        let ahead = CLOCK_WINDOW_H.as_julian_years_f64();
        let back = ahead + LIGHT_CROSSING_L.as_julian_years_f64();
        BinaryMarks::phase(
            composition,
            draws,
            age,
            MarkedPhase {
                kind: SegmentKind::StableTransfer {
                    donor: Component::Secondary,
                },
                start: Years::new((age.value() - back).max(0.0)),
                end: Years::new(age.value() + ahead),
                stars: [self.white_dwarf, self.donor],
                orbit: self.orbit,
                transfer_rate: Some(self.transfer_rate),
            },
        )
        .expect("an age past the horizon and a positive rate are a marked phase")
    }
}

/// An accreting white dwarf's history (P11.T8.a): what the table draws given the present.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AwdHistory {
    primary_initial_mass: SolarMasses,
    donor_initial_mass: SolarMasses,
    age_at_epoch: Years,
}

impl AwdHistory {
    /// The white dwarf's progenitor's initial mass, M☉: the record's primary's.
    #[must_use]
    pub const fn primary_initial_mass(&self) -> SolarMasses {
        self.primary_initial_mass
    }

    /// The donor's initial mass, M☉, at least its mass now and at most the primary's.
    #[must_use]
    pub const fn donor_initial_mass(&self) -> SolarMasses {
        self.donor_initial_mass
    }

    /// The system's age at the epoch, Julian years.
    #[must_use]
    pub const fn age_at_epoch(&self) -> Years {
        self.age_at_epoch
    }
}

/// An accreting white dwarf entry (plan 11, P11.T8.a): a host of its class.
#[derive(Debug, Clone, PartialEq)]
pub struct AwdEntry {
    record: SystemRecord,
    motion: CatalogueMotion,
    scan: AwdScanMarks,
    composition: Composition,
    binary: AwdBinary,
    history: AwdHistory,
    timeline: BinaryTimeline,
}

impl AwdEntry {
    /// The record of the entry's system, of origin
    /// [`SystemOrigin::CatalogueSystem`](crate::galaxy::placement::SystemOrigin::CatalogueSystem)
    /// of its class, at its site at the epoch; for a feature's list entry (P11.T8.f), of its
    /// feature's or the galactic centre's member origin, at its list position.
    #[must_use]
    pub const fn record(&self) -> &SystemRecord {
        &self.record
    }

    /// How the system moves: at its velocity, on one straight line.
    #[must_use]
    pub const fn motion(&self) -> &CatalogueMotion {
        &self.motion
    }

    /// Its scan marks, the first it drew.
    #[must_use]
    pub const fn scan(&self) -> &AwdScanMarks {
        &self.scan
    }

    /// The composition both stars share.
    #[must_use]
    pub const fn composition(&self) -> &Composition {
        &self.composition
    }

    /// The binary now.
    #[must_use]
    pub const fn binary(&self) -> &AwdBinary {
        &self.binary
    }

    /// Its history.
    #[must_use]
    pub const fn history(&self) -> &AwdHistory {
        &self.history
    }

    /// The pair's timeline, from its marks.
    #[must_use]
    pub const fn timeline(&self) -> &BinaryTimeline {
        &self.timeline
    }

    /// The system's stars: the white dwarf's progenitor and the donor, each on its own model at
    /// its initial mass and the system's age, on the marked orbit, and the pair's timeline, which
    /// gives both stars' states.
    ///
    /// # Panics
    ///
    /// Never: the history's masses are stars' and its age is finite.
    #[must_use]
    pub fn stars(&self, galaxy: &Galaxy) -> SystemStars {
        let system = self.record.id();
        let age = self.history.age_at_epoch;
        let rules = galaxy.rules().stellar;
        let masses = [
            self.history.primary_initial_mass,
            self.history.donor_initial_mass,
        ];
        let stars = [0_u16, 1].into_iter().zip(masses).map(|(body, mass)| {
            let draws = StarDraws::for_star(galaxy.seed(), BodyId::new(system, body));
            StarModel::new_under(mass, self.composition, draws, age, rules)
                .expect("a host's stars are of 0.08–7.9 M☉ at a finite age")
        });
        let hierarchy = SystemHierarchy::binary_on(
            system,
            masses[0],
            (masses[1], SlotKind::Star),
            self.binary.orbit,
        );
        SystemStars::of_marked_pair(
            galaxy.seed(),
            self.record,
            hierarchy,
            stars.collect(),
            self.timeline.clone(),
        )
    }
}

/// The white dwarf of mass `mass` fed at `rate`, `age` old: HPT's radius (equation 91) and the
/// effective temperature accretion heats it to, `T_eff` = 1.7 × 10⁴ K (⟨Ṁ⟩ ÷ 10⁻¹⁰ M☉ yr⁻¹)^¼ ×
/// (M ÷ 0.9 M☉) (Townsley and Gänsicke 2009, ApJ 693, 1007, equation 2). Their relation is for
/// hydrogen accreted at 10⁻¹¹–10⁻⁸ M☉ yr⁻¹; the rate is held to that range here, and the
/// helium donors' dwarfs take it too, an extrapolation.
fn white_dwarf_state(mass: SolarMasses, rate: SolarMassesPerYear, age: Years) -> StarState {
    let radius = white_dwarf_radius(RemnantRecipe::default(), mass);
    let heating_rate = rate.value().clamp(1.0e-11, 1.0e-8);
    let t_eff_k = 1.7e4 * math::powf_positive(heating_rate / 1.0e-10, 0.25) * (mass.value() / 0.9);
    let phase = if mass < OXYGEN_NEON_MIN_MASS {
        Phase::CarbonOxygenWhiteDwarf
    } else {
        Phase::OxygenNeonWhiteDwarf
    };
    StarState::new(StarStateParts {
        phase,
        age,
        mass,
        core_mass: mass,
        luminosity: luminosity_of(radius, t_eff_k),
        radius,
        mass_loss_rate: SolarMassesPerYear::ZERO,
        phase_fraction: 0.0,
    })
}

/// The donor of `kind` and `mass`, filling its Roche lobe of radius `lobe`, `age` old: a
/// main-sequence star at its zero-age luminosity (plan 06's fit at `composition`), a helium white
/// dwarf at [`HELIUM_DONOR_TEMPERATURE_K`], or a giant at [`GIANT_DONOR_TEMPERATURE_K`].
fn donor_state(
    kind: AwdDonorKind,
    mass: SolarMasses,
    lobe: SolarRadii,
    composition: &Composition,
    age: Years,
) -> StarState {
    let (phase, core_mass, luminosity) = match kind {
        AwdDonorKind::MainSequence => {
            let coeffs = ZCoeffs::new(composition.z_fit());
            let fit_mass = SolarMasses::new(mass.value().max(0.1));
            (
                Phase::MainSequence,
                SolarMasses::ZERO,
                zams::luminosity(fit_mass, &coeffs),
            )
        }
        AwdDonorKind::Helium => (
            Phase::HeliumWhiteDwarf,
            mass,
            luminosity_of(lobe, HELIUM_DONOR_TEMPERATURE_K),
        ),
        AwdDonorKind::Giant => (
            Phase::FirstGiantBranch,
            SolarMasses::new(GIANT_DONOR_CORE_MASS.value().min(0.9 * mass.value())),
            luminosity_of(lobe, GIANT_DONOR_TEMPERATURE_K),
        ),
    };
    StarState::new(StarStateParts {
        phase,
        age,
        mass,
        core_mass,
        luminosity,
        radius: lobe,
        mass_loss_rate: SolarMassesPerYear::ZERO,
        phase_fraction: 0.5,
    })
}

/// The luminosity of a photosphere of `radius` at `t_eff_k` K, L☉, by Stefan–Boltzmann against
/// the nominal Sun (IAU 2015 Resolution B3).
fn luminosity_of(radius: SolarRadii, t_eff_k: f64) -> SolarLuminosities {
    let t = t_eff_k / SOLAR_EFFECTIVE_TEMPERATURE_K;
    SolarLuminosities::new(radius.value() * radius.value() * t * t * t * t)
}

/// The fast hosts' class (plan 11's Provides; P11.T8.a): [`ClassId::ACCRETING_WHITE_DWARF`], the
/// accreting white dwarfs whose novae recur within L.
///
/// # Examples
///
/// The class's cells about the Sun-like point hold hosts, each resolving by its ID to itself and
/// recurring within L:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::catalogue_classes::binary::AccretingWdFast;
/// use hyperion_sim::galaxy::catalogue_classes::{
///     CatalogueCellKey, CatalogueClassCell, ClassId, resolve_entry,
/// };
///
/// use hyperion_sim::time::LIGHT_CROSSING_L;
///
/// let galaxy = Galaxy::new(Seed::new(9));
/// let process = AccretingWdFast::new(&galaxy);
/// let key = CatalogueCellKey::new(ClassId::ACCRETING_WHITE_DWARF, [0, 50, 0])?;
/// let cell = CatalogueClassCell::generate(&galaxy, &process, key);
/// for entry in cell.entries() {
///     assert_eq!(resolve_entry(&galaxy, &process, entry.id())?.entry(), entry);
///     let l = LIGHT_CROSSING_L.as_julian_years_f64();
///     assert!(entry.value().scan().recurrence().value() <= l);
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct AccretingWdFast(AwdProcess);

/// The slow hosts' class (plan 11's Provides; P11.T8.a): [`ClassId::ACCRETING_WHITE_DWARF_SLOW`],
/// the accreting white dwarfs whose novae recur more slowly than L, each carrying its eruptions in
/// the source horizon.
#[derive(Debug, Clone, PartialEq)]
pub struct AccretingWdSlow(AwdProcess);

impl AccretingWdFast {
    /// The class in `galaxy`.
    #[must_use]
    pub fn new(galaxy: &Galaxy) -> Self {
        Self(AwdProcess::new(galaxy, AwdSide::Fast))
    }

    /// What it shares with the slow hosts' class: its components, side and scan.
    #[must_use]
    pub const fn hosts(&self) -> &AwdProcess {
        &self.0
    }
}

impl AccretingWdSlow {
    /// The class in `galaxy`.
    #[must_use]
    pub fn new(galaxy: &Galaxy) -> Self {
        Self(AwdProcess::new(galaxy, AwdSide::Slow))
    }

    /// What it shares with the fast hosts' class: its components, side and scan.
    #[must_use]
    pub const fn hosts(&self) -> &AwdProcess {
        &self.0
    }
}

/// The two classes' one implementation of plan 09's [`ClassProcess`], on their shared
/// [`AwdProcess`].
macro_rules! awd_class_process {
    ($class:ident) => {
        impl ClassProcess for $class {
            type Candidate = AwdCandidate;
            type Entry = AwdEntry;

            fn class(&self) -> ClassId {
                self.0.class()
            }

            fn bound(&self, galaxy: &Galaxy, cell: &CatalogueCellKey) -> PerCubicLightYear {
                self.0.bound(galaxy, cell)
            }

            fn candidate(
                &self,
                galaxy: &Galaxy,
                cell: &CatalogueCellKey,
                id: CatalogueSystemId,
                bound: PerCubicLightYear,
            ) -> Option<AwdCandidate> {
                self.0.candidate(galaxy, cell, id, bound)
            }

            fn test(&self, _: &Galaxy, candidate: AwdCandidate) -> Option<AwdEntry> {
                self.0.test(candidate)
            }

            fn members(&self, _: &AwdEntry) -> u8 {
                1
            }

            fn system(&self, _: &Galaxy, member: &CatalogueMember<AwdEntry>) -> CatalogueSystem {
                let entry = member.entry().value();
                CatalogueSystem::new(entry.record, entry.motion).with_class_stars()
            }

            fn stars(
                &self,
                galaxy: &Galaxy,
                member: &CatalogueMember<AwdEntry>,
            ) -> Option<SystemStars> {
                Some(member.entry().value().stars(galaxy))
            }

            /// The site, where the host sits at the epoch, moving at its component's draw.
            fn epoch_place(
                &self,
                galaxy: &Galaxy,
                cell: &CatalogueCellKey,
                id: CatalogueSystemId,
            ) -> Option<GalacticPosition> {
                Some(site_stream(galaxy, cell, id, tags::CLASS_AWD_SITE).0)
            }
        }
    };
}

awd_class_process!(AccretingWdFast);
awd_class_process!(AccretingWdSlow);

#[cfg(test)]
mod tests;
