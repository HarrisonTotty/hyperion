//! The X-ray binaries' class (plan 11, P11.T8.b; design notes 10 and 13): every neutron star or
//! black hole fed by a companion, `XRAY_BINARY` (5), placed on 512 ly cells of its own.
//!
//! The grid redraws every binary that falls into a carved class (P11.T7), so these hosts are
//! nowhere in the cells, and the share matrix gives up what the class holds: its row of
//! [`CLASS_SHARES`](crate::tables::binary::CLASS_SHARES), hosts per solar mass formed, by
//! population. The class is a Poisson process placed by thinning on the class grid of plan 09
//! ([`ClassProcess`]), as P11.T8.a places the accreting white dwarfs. It reads plan 15's fitted
//! share row and sampler (P15.T10.b's refit at 27).
//!
//! # The two sides
//!
//! The low-mass kinds (a neutron star or a black hole, persistent or transient, and from 27 an
//! ultracompact or a symbiotic one) are the old populations' and the high-mass kinds (Be/X and
//! supergiant, and from 27 a black hole beside a supergiant) the young disc's. The class draws all
//! nine kinds the carving counts from 27 ([`XRB_SAMPLER`](crate::tables::binary::XRB_SAMPLER),
//! fitted at 27), each in its share of the engine's members but the black-hole transients, pinned
//! at the observed share, so that the hosts its row counts are the hosts it draws (P11.T8.b's
//! fix). The class's total
//! is its row's, `Σ_c r_c n_c` over every component c, with `r_c` the row's hosts per solar mass
//! formed in c's population × the mass formed per system × its born fraction; the high-mass side
//! takes its sampler's high-mass share of it over the young thin disc's components,
//! and the low-mass side the rest over every other component, each side's components in
//! proportion to `r_c n_c` (lean, 2026-10-31). A row equal in every population would give the
//! young disc, a few tenths of a per cent of the mass formed, about 25 high-mass hosts, fewer
//! than are catalogued; so the budget closes over the galaxy and not population by population (a
//! finding, the row not retuned). From generator version 27 each component keeps all but its
//! features' share (P11.T8.f, `feature::catalogue_share`), which the features' lists hold at the
//! same hosts per solar mass formed ([`XrayBinaries::hosts_per_formed_mass`]). The intensity at x is `Σ_c w_c n_c(x)`, `w_c` the hosts per
//! system its side's share gives component c, and a cell's bound `Σ_c w_c × bound_c`.
//!
//! # A candidate
//!
//! On its own streams (plan 09's design note 23), a candidate:
//!
//! 1. draws its scan marks first (`stellar::binary::scan`): its site uniformly in the cell, thinned
//!    on the intensity there ÷ the bound, the same mark picking its component and so its side
//!    (`class.xrb.site`); its kind on that side and a low-mass transient's outburst recurrence
//!    (`class.xrb.kind`); its compact star's mass, then by kind its period, its donor's mass and its
//!    transfer rate over the irradiated disc's instability line on its kind's side of it (a
//!    low-mass host), its period, eccentricity and Be star (a Be/X host), its period, its
//!    supergiant's mass and its rate (a supergiant or black-hole high-mass host), its period, from
//!    which its helium white dwarf's mass and its rate follow (an ultracompact host, a transient
//!    by its rate taking its recurrence on `class.xrb.kind`), or its period, its giant's mass and
//!    its giant's share of its Roche lobe (a symbiotic host) (`class.xrb.binary`); a high-mass
//!    host's recoil from its supernova and the years since (`class.xrb.recoil`), which carried it
//!    from its site; and its velocity at the site, plan 08's draw of its component's law, plus the
//!    recoil, the one velocity its entry moves at (rule 6; `class.xrb.velocity`);
//! 2. draws its orbit's orientation and phase (`class.xrb.orbit`);
//! 3. draws its history given the present: its compact star's progenitor's initial mass, and for a
//!    low-mass host its donor's initial mass (an ultracompact host's white dwarf's progenitor's)
//!    and its age in its component's age law
//!    (`class.xrb.history`), raised to at least [`LIFETIME_MARGIN`] × the progenitor's lifetime; a
//!    high-mass host's age is the progenitor's lifetime and the years since its supernova;
//! 4. draws its \[Fe/H\] from its component's law at its site and that age, before the lifetime
//!    raises it (`class.xrb.metallicity`).
//!
//! A feature's list entry (P11.T8.f) draws the same marks after its site, keyed by its own ID, on
//! the side its members' population gives, but takes no recoil, its feature's velocity and
//! composition and an age drawn in its members' ages, and is thinned where its compact star's
//! progenitor cannot have died by then.
//!
//! # The recoil (lean, 2026-10-31)
//!
//! A high-mass host's recoil is the systemic velocity its compact star's supernova gave the pair,
//! plan 11's own kick (P11.T4.e), drawn here from the table rather than the engine. Its site is
//! where the explosion was relative to its component's field, and the pair has moved from it on a
//! straight line at the recoil over the years since, as plan 08's `explosion_site` carries a record
//! from its epoch position at its velocity. No displaced field of plan 08 (P08.T12) is read: the
//! young disc's field at the site stands in for the pair's birth place, which it outlives by under
//! 10 Myr.
//!
//! # The test
//!
//! The candidate's marks define its [`BinaryTimeline`] without the engine
//! ([`BinaryTimeline::from_marks`]): a detached history, then its phase across the whole source
//! horizon, each star held at its marked state: stable transfer onto the compact star for a
//! low-mass, ultracompact, supergiant or black-hole high-mass host, a detached Be star beside its
//! neutron star for a Be/X host, and a detached giant whose wind feeds its neutron star for a
//! symbiotic host. The candidate is an entry if and only if [`carved_class`] of that timeline is
//! [`CarvedClass::XrayBinary`], the one test the grid applies to an evolved pair (design note 10),
//! and the class `classify` gives the pair at the epoch is its kind's. A Be/X donor is plan 06's
//! Be star by its own rotation and field draws: it takes the first of [`BE_DONOR_ATTEMPTS`]
//! attempts of them that makes it one, about one in 36 doing so, so the test refuses a candidate
//! only if none does, a chance under 10⁻⁴⁹.
//!
//! # The entry
//!
//! An entry has one member, its system, at its position at the epoch and moving at its velocity on
//! a straight line. Its recoil takes it at most 3,000 ly from its site, its years since the
//! supernova held where it would pass that (`MAX_RECOIL_TRAVEL_LY`; the table refitted at 27
//! draws Be/X recoils to 59.3 km/s over 3.0 × 10⁷ years), so that with its motion over the source
//! horizon it stays within plan 09's 4,096 ly reach of its cell. Its record is a catalogue
//! system's, counted in its primary's layer, D or E: its primary is the compact star's
//! progenitor. Its stars are the pair: each star's own model at its initial mass and the system's
//! age, and the pair's timeline, which gives both stars' states ([`XrbEntry::stars`]).

use super::super::grid::{CatalogueMember, ClassProcess};
use super::super::ids::CatalogueCellKey;
use super::super::registry::ClassId;
use super::super::system::{CatalogueMotion, CatalogueSystem};
use super::awd::{
    GIANT_DONOR_CORE_MASS, GIANT_DONOR_TEMPERATURE_K, LIFETIME_MARGIN, donor_state, luminosity_of,
};
use super::feature::{HostPlace, catalogue_share};
use crate::coords::GalacticPosition;
use crate::galaxy::bounds::CellBox;
use crate::galaxy::fields::{ComponentId, MAX_COMPONENTS};
use crate::galaxy::placement::SystemRecord;
use crate::galaxy::{Galaxy, PointLy, Population};
use crate::id::{BodyId, CatalogueSystemId, SystemId};
use crate::math;
use crate::orbit::{Eccentricity, KeplerElements, Orientation, roche_lobe_radius};
use crate::rng::{ObjectKey, Stream, tags};
use crate::stellar::binary::is_be_donor;
use crate::stellar::binary::light_curves::XrayAccretor;
use crate::stellar::binary::sampler::ClassSampler;
use crate::stellar::binary::scan::{XrbScanMarks, XrbShape, scan_marks};
use crate::stellar::binary::{
    BinaryMarks, BinaryTimeline, CarvedClass, Component, MarkedPhase, SegmentKind, carved_class,
};
use crate::stellar::classify::{SpectralLetter, subtype_from_teff_under};
use crate::stellar::draws::StarDraws;
use crate::stellar::multiplicity::{SlotKind, SystemHierarchy};
use crate::stellar::remnant::RemnantRecipe;
use crate::stellar::remnant::neutron_star::hpt_luminosity;
use crate::stellar::remnant::structure::{black_hole_radius, neutron_star_radius};
use crate::stellar::rules::StellarRules;
use crate::stellar::sse::wind::REIMERS;
use crate::stellar::sse::{Track, ZCoeffs, turn_off_mass, zams};
use crate::stellar::system::{StarModel, SystemStars, track_mass_of};
use crate::stellar::{Composition, Phase, StarState, StarStateParts, lifetime};
use crate::tables::binary::{
    AwdDonorKind, CLASS_SHARES, ClassSamplerTable, XRB_SAMPLER, XrbHostKind, XrbMark, XrbPick,
};
use crate::time::{CLOCK_WINDOW_H, LIGHT_CROSSING_L};
use crate::units::consts::SOLAR_RADIUS_M;
use crate::units::{
    GravitationalParameter, PerCubicLightYear, Radians, SolarLuminosities, SolarMasses,
    SolarMassesPerYear, SolarRadii, Years,
};
use crate::{GENERATOR_VERSION, Seed};

/// The attempts of a Be/X donor's own draws (`StarDraws::for_attempt`) tried for one that makes it
/// plan 06's Be star: 4,096. About one attempt in 36 does, at 8.5–14 M☉ (plan 06's fast rotators
/// above 0.7 of critical, less its fossil fields; measured), so that all 4,096 fail with a chance
/// near e⁻¹¹⁴. An attempt reads its draws from word 64 × n of each star stream, as a redraw does.
pub const BE_DONOR_ATTEMPTS: u32 = 4_096;

/// The shares of its lifetime (plan 06's [`lifetime`]) at which a Be/X donor's state is read from
/// its own track, the first at which it is a B star on the main sequence: halfway through its main
/// sequence, Be stars being found mostly in its second half (Fabregat and Torrejón 2000, A&A 357,
/// 451; recalled, not read at source), then later and cooler, for a metal-poor star hot enough at half
/// its life to be an O star. Plan 06's main sequence ends near 0.89 of the lifetime at 8.5–30 M☉.
const BE_DONOR_LIFE_SHARES: [f64; 3] = [0.5, 0.7, 0.85];

/// The least radius of a low-mass donor's Roche lobe, as a share of its zero-age main-sequence
/// radius at its mass, at which the donor is an evolved star rather than a main-sequence one: 1.3,
/// ours, provisional, about the radius the main sequence of a star of 0.3–1 M☉ reaches by its end.
pub const EVOLVED_DONOR_LOBE_SHARE: f64 = 1.3;

/// The least core of an evolved low-mass donor, M☉: 0.15, about the least core Rappaport et al.'s
/// (1995) giant-branch relations cover (recalled, not read at source).
pub const EVOLVED_DONOR_MIN_CORE_MASS: SolarMasses = SolarMasses::new(0.15);

/// The share of its lifetime (plan 06's [`lifetime`]) at which a supergiant donor's luminosity is
/// read from its own track: 0.88, near the end of its main sequence, which plan 06 ends near 0.89 of
/// the lifetime at 8.5–30 M☉ (measured for P11.T8.b), so that its luminosity follows its mass.
pub const SUPERGIANT_DONOR_LIFE_SHARE: f64 = 0.88;

/// A supergiant donor's helium core as a share of its mass: 0.27, the share plan 06's tracks give
/// a star of 14–25 M☉ at the start of its Hertzsprung gap at solar metallicity (0.24–0.29,
/// measured for P11.T8.b).
pub const SUPERGIANT_DONOR_CORE_SHARE: f64 = 0.27;

/// The share of the turn-off mass at the system's age (plan 06's `turn_off_mass`) that a donor's
/// initial mass is held under: 0.95, ours, so that its own model is on its main sequence at the
/// epoch, as the timeline's main-sequence donors are. A high-mass donor gained mass from its
/// primary, so its initial mass lies below its mass now, and the history does not draw how much;
/// a low-mass donor's drawn initial mass can lie above the turn-off of an old system (P15.T10.b).
pub const DONOR_TURN_OFF_SHARE: f64 = 0.95;

/// The least initial mass of a donor: 0.08 M☉, the stellar floor of plan 11's companions.
const DONOR_INITIAL_MIN: SolarMasses = SolarMasses::new(0.08);

/// Which side of the class a component's hosts are on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum XrbSide {
    /// The low-mass kinds, of every component but the young thin disc's.
    LowMass,
    /// The high-mass kinds, of the young thin disc's components.
    HighMass,
}

impl XrbSide {
    /// The side of a component of `population`.
    #[must_use]
    pub const fn of(population: Population) -> Self {
        match population {
            Population::YoungThinDisc => Self::HighMass,
            Population::OldThinDisc
            | Population::ThickDisc
            | Population::Bulge
            | Population::LongBar
            | Population::NuclearDisc
            | Population::Halo => Self::LowMass,
        }
    }
}

/// One density component as the X-ray binaries' class thins on it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct XrbComponent {
    id: ComponentId,
    side: XrbSide,
    hosts_per_system: f64,
}

impl XrbComponent {
    /// The component.
    #[must_use]
    pub const fn id(&self) -> ComponentId {
        self.id
    }

    /// The side its hosts are on.
    #[must_use]
    pub const fn side(&self) -> XrbSide {
        self.side
    }

    /// Its hosts per system of its density (module documentation).
    #[must_use]
    pub const fn hosts_per_system(&self) -> f64 {
        self.hosts_per_system
    }
}

/// The X-ray binaries' class (plan 11's Provides; P11.T8.b): [`ClassId::XRAY_BINARY`], every
/// neutron star and black hole fed by a companion.
///
/// # Examples
///
/// The class's cells about the centre hold hosts, each resolving by its ID to itself and its pair
/// in the class its kind names:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::catalogue_classes::binary::XrayBinaries;
/// use hyperion_sim::galaxy::catalogue_classes::{
///     CatalogueCellKey, CatalogueClassCell, ClassId, resolve_entry,
/// };
/// use hyperion_sim::time::UniverseTime;
///
/// let galaxy = Galaxy::new(Seed::new(9));
/// let process = XrayBinaries::new(&galaxy);
/// let mut found = 0;
/// for cell in (-3..3).flat_map(|x| (-3..3).map(move |y| [x, y, 0])) {
///     let key = CatalogueCellKey::new(ClassId::XRAY_BINARY, cell)?;
///     let cell = CatalogueClassCell::generate(&galaxy, &process, key);
///     for entry in cell.entries() {
///         assert_eq!(resolve_entry(&galaxy, &process, entry.id())?.entry(), entry);
///         let now = entry.value().stars(&galaxy).state_at(UniverseTime::EPOCH);
///         let class = now.ok_or("a host at the epoch")?.pairs()[0].class();
///         assert_eq!(class, entry.value().scan().class());
///         found += 1;
///     }
/// }
/// assert!(found > 0);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct XrayBinaries {
    components: Vec<XrbComponent>,
    sampler: ClassSampler<XrbMark, XrbPick>,
    rate_split: f64,
    /// Each side's hosts per solar mass formed over the row's: the low-mass side's, then the
    /// high-mass side's.
    side_scales: (f64, f64),
}

impl XrayBinaries {
    /// The class in `galaxy`: its components, from its row of [`CLASS_SHARES`] and
    /// [`XRB_SAMPLER`]'s high-mass share, plan 15's fitted tables, and the rank of the instability
    /// line in the low-mass rate's law.
    #[must_use]
    pub fn new(galaxy: &Galaxy) -> Self {
        Self::of_table(galaxy, &XRB_SAMPLER)
    }

    /// The class in `galaxy` drawing from `table`, its row [`CLASS_SHARES`]'.
    #[must_use]
    pub(crate) fn of_table(
        galaxy: &Galaxy,
        table: &'static ClassSamplerTable<XrbMark, XrbPick>,
    ) -> Self {
        let row = CLASS_SHARES.row(CarvedClass::XrayBinary);
        let sampler = ClassSampler::new(table);
        let formed = galaxy.mean_formed_mass().value();
        let fields = galaxy.fields();
        let rows: Vec<(ComponentId, XrbSide, f64, f64)> = fields
            .component_ids()
            .filter_map(|id| {
                let component = fields.component(id);
                let hosts = row.hosts_per_formed_mass(component.population())
                    * formed
                    * component.ages().born_fraction();
                (hosts > 0.0).then(|| {
                    let side = XrbSide::of(component.population());
                    (id, side, hosts, hosts * component.count_with_unborn())
                })
            })
            .collect();
        let side_hosts = |side: XrbSide| {
            rows.iter()
                .filter(|r| r.1 == side)
                .fold(0.0, |sum, r| sum + r.3)
        };
        let (low, high) = (side_hosts(XrbSide::LowMass), side_hosts(XrbSide::HighMass));
        let total = low + high;
        let share = high_mass_share(sampler.table());
        // Each side's hosts are its share of the row's total; a galaxy without one side's
        // components gives the other side the whole.
        let scale = |side: XrbSide| match (side, low > 0.0, high > 0.0) {
            (XrbSide::LowMass, true, true) => (1.0 - share) * total / low,
            (XrbSide::HighMass, true, true) => share * total / high,
            (XrbSide::LowMass, true, false) => total / low,
            (XrbSide::HighMass, false, true) => total / high,
            (XrbSide::LowMass, false, _) | (XrbSide::HighMass, _, false) => 0.0,
        };
        let (low_scale, high_scale) = (scale(XrbSide::LowMass), scale(XrbSide::HighMass));
        let components = rows
            .into_iter()
            .map(|(id, side, hosts, _)| XrbComponent {
                id,
                side,
                hosts_per_system: hosts
                    * match side {
                        XrbSide::LowMass => low_scale,
                        XrbSide::HighMass => high_scale,
                    }
                    * catalogue_share(galaxy, row, fields.component(id)),
            })
            .filter(|c| c.hosts_per_system > 0.0)
            .collect();
        let rate_split = sampler.rank_of(XrbMark::LowMassRateExcess, 0.0, 0.0);
        Self {
            components,
            sampler,
            rate_split,
            side_scales: (low_scale, high_scale),
        }
    }

    /// The class's hosts per solar mass formed among systems of `population`, before its test: its
    /// row's, times the share of its side's (module documentation). Over every component it gives
    /// the row's total; the young disc's high-mass hosts are about 65 times the row's at Milky Way
    /// parameters (P11.T8.b's record), which is what a young feature's list counts (P11.T8.f).
    #[must_use]
    pub fn hosts_per_formed_mass(&self, population: Population) -> f64 {
        let scale = match XrbSide::of(population) {
            XrbSide::LowMass => self.side_scales.0,
            XrbSide::HighMass => self.side_scales.1,
        };
        CLASS_SHARES
            .row(CarvedClass::XrayBinary)
            .hosts_per_formed_mass(population)
            * scale
    }

    /// The components it thins on, in component order.
    #[must_use]
    pub fn components(&self) -> &[XrbComponent] {
        &self.components
    }

    /// The sampler the class draws from: the fitted [`XRB_SAMPLER`].
    #[must_use]
    pub const fn sampler(&self) -> ClassSampler<XrbMark, XrbPick> {
        self.sampler
    }

    /// The rank of the sampler's low-mass rate excess at the instability line: a transient
    /// draws below it and a persistent host above.
    #[must_use]
    pub const fn rate_split(&self) -> f64 {
        self.rate_split
    }

    /// The class's expected hosts at `point`, per cubic light-year: the intensity `Σ_c w_c n_c`,
    /// before the class's test.
    #[must_use]
    pub fn hosts_density(&self, galaxy: &Galaxy, point: &PointLy) -> PerCubicLightYear {
        let mut weights = [0.0; MAX_COMPONENTS];
        let n = self.weights(galaxy, point, &mut weights);
        PerCubicLightYear::new(weights[..n].iter().fold(0.0, |sum, w| sum + w))
    }

    /// The class's expected hosts on `side` in the whole galaxy, before its test: each of its
    /// components' hosts per system × the integral of its density.
    #[must_use]
    pub fn expected_hosts_on(&self, galaxy: &Galaxy, side: XrbSide) -> f64 {
        let fields = galaxy.fields();
        self.components
            .iter()
            .filter(|c| c.side == side)
            .fold(0.0, |sum, c| {
                sum + c.hosts_per_system * fields.component(c.id).count_with_unborn()
            })
    }

    /// The class's expected hosts in the whole galaxy, before its test.
    #[must_use]
    pub fn expected_hosts(&self, galaxy: &Galaxy) -> f64 {
        self.expected_hosts_on(galaxy, XrbSide::LowMass)
            + self.expected_hosts_on(galaxy, XrbSide::HighMass)
    }

    /// The class's expected hosts of `kind` in the whole galaxy, before its test: its side's
    /// expected hosts × its weight in its side's choice over the choice's sum, zero for a kind past
    /// the choice's weights (below 27 the carving's three).
    ///
    /// # Panics
    ///
    /// Never: every kind is one of its side's.
    #[must_use]
    pub fn expected_hosts_of(&self, galaxy: &Galaxy, kind: XrbHostKind) -> f64 {
        let (side, pick, kinds): (_, _, &[XrbHostKind]) = if kind.is_high_mass() {
            (
                XrbSide::HighMass,
                XrbPick::HighMassKind,
                &XrbHostKind::HIGH_MASS,
            )
        } else {
            (
                XrbSide::LowMass,
                XrbPick::LowMassKind,
                &XrbHostKind::LOW_MASS,
            )
        };
        let weights = self.sampler.table().pick(pick).weights();
        let place = kinds
            .iter()
            .position(|&k| k == kind)
            .expect("every kind is one of its side's");
        let sum = weights.iter().fold(0.0, |s, w| s + w);
        let hosts = self.expected_hosts_on(galaxy, side);
        weights.get(place).map_or(0.0, |w| hosts * w / sum)
    }

    /// [`expected_hosts_of`](Self::expected_hosts_of) every kind, in [`XrbHostKind::ALL`]'s order.
    #[must_use]
    pub fn expected_hosts_by_kind(&self, galaxy: &Galaxy) -> [f64; XrbHostKind::ALL.len()] {
        XrbHostKind::ALL.map(|kind| self.expected_hosts_of(galaxy, kind))
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
    /// component the same mark picks and its side (`class.xrb.site`), or `None` if the thinning
    /// rejects it.
    #[must_use]
    pub(crate) fn site(
        &self,
        galaxy: &Galaxy,
        cell: &CatalogueCellKey,
        id: CatalogueSystemId,
        bound: PerCubicLightYear,
    ) -> Option<(GalacticPosition, ComponentId, XrbSide)> {
        assert_eq!(
            cell.class(),
            ClassId::XRAY_BINARY,
            "an X-ray binary candidate is of its own class's cell"
        );
        if bound.value().is_nan() || bound.value() <= 0.0 {
            return None;
        }
        let key = ObjectKey::from(SystemId::from(id));
        let mut stream = Stream::open(galaxy.seed(), tags::CLASS_XRB_SITE, key);
        let site =
            cell.position_from_words([stream.next_u64(), stream.next_u64(), stream.next_u64()]);
        let mut weights = [0.0; MAX_COMPONENTS];
        let n = self.weights(galaxy, &PointLy::from(&site), &mut weights);
        let picked = stream.mark().pick_weighted(&weights[..n], bound.value())?;
        let component = self.components[picked];
        Some((site, component.id, component.side))
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
    #[must_use]
    fn draw_candidate(
        &self,
        galaxy: &Galaxy,
        cell: &CatalogueCellKey,
        id: CatalogueSystemId,
        bound: PerCubicLightYear,
    ) -> Option<XrbCandidate> {
        let scan = scan_marks(galaxy, self, cell, id, bound)?;
        self.candidate_of(galaxy, &scan)
    }

    /// The candidate whose scan marks are `scan`, a catalogue candidate's or a feature's list
    /// entry's (P11.T8.f): its orbit, history and composition after them. A list entry's age is
    /// drawn in its members' ages and its composition is theirs, and the candidate is `None` if
    /// its compact star's progenitor cannot have died by then.
    #[must_use]
    pub(crate) fn candidate_of(
        &self,
        galaxy: &Galaxy,
        scan: &XrbScanMarks,
    ) -> Option<XrbCandidate> {
        let seed = galaxy.seed();
        let system = scan.id();
        let key = scan.host().key();
        let sampler = self.sampler;
        let kind = scan.kind();
        // 2. The binary now: its scan marks hold its masses, period and rate, so its orbit's
        // orientation and phase remain.
        let mut stream = Stream::open(seed, tags::CLASS_XRB_ORBIT, key);
        let orientation = Orientation::new(
            Radians::new(math::acos(2.0 * stream.uniform() - 1.0)),
            Radians::new(std::f64::consts::TAU * stream.uniform()),
            Radians::new(std::f64::consts::TAU * stream.uniform()),
        )
        .expect("an inclination in [0, π] and angles in [0, 2π) are an orientation");
        let mean_anomaly = Radians::new(std::f64::consts::TAU * stream.uniform());
        // 3. The history, given the present.
        let mut stream = Stream::open(seed, tags::CLASS_XRB_HISTORY, key);
        let progenitor_mark = match kind.accretor() {
            XrayAccretor::NeutronStar => XrbMark::NeutronStarProgenitor,
            XrayAccretor::BlackHole => XrbMark::BlackHoleProgenitor,
        };
        let m0 = SolarMasses::new(math::exp10(sampler.draw(progenitor_mark, 0.0, &mut stream)));
        let host = scan.host();
        let initial_mark = donor_initial_mark(kind);
        let (initial_drawn, drawn_age) = match (scan.recoil(), host.place()) {
            (Some(recoil), _) => (None, recoil.since_supernova().value()),
            (None, HostPlace::Field(component)) => {
                let drawn = sampler.draw(initial_mark, 0.0, &mut stream);
                let ages = galaxy.fields().component(*component).ages();
                let unborn = 1.0 - ages.born_fraction();
                let age = ages.quantile(unborn + (1.0 - unborn) * stream.uniform());
                (Some(drawn), age.value())
            }
            (None, HostPlace::Listed(listed)) => {
                let drawn =
                    (!kind.is_high_mass()).then(|| sampler.draw(initial_mark, 0.0, &mut stream));
                (drawn, listed.age_at(stream.uniform()).value())
            }
        };
        // 4. The composition, at the site and the drawn age; a list entry's members'.
        let composition =
            host.composition_at(galaxy, Years::new(drawn_age), tags::CLASS_XRB_METALLICITY);
        // The age at which the compact star has formed, and the years since it did.
        let primary = StarDraws::for_star(seed, BodyId::new(system, 0));
        let life = lifetime(track_mass_of(m0), &composition, &primary).value();
        let (age, since_formation) = match (scan.recoil(), host.listed_host()) {
            (Some(recoil), _) => {
                let since = recoil.since_supernova().value();
                (life + since, since)
            }
            (None, None) => {
                let age = drawn_age.max(LIFETIME_MARGIN * life);
                (age, age - life)
            }
            (None, Some(_)) if drawn_age < LIFETIME_MARGIN * life => return None,
            (None, Some(_)) => (drawn_age, drawn_age - life),
        };
        let age = Years::new(age);
        let binary = XrbBinary::marked(
            scan,
            (orientation, mean_anomaly),
            &composition,
            (system, galaxy.seed()),
            (age, Years::new(since_formation)),
        );
        let donor_mass = binary.donor.mass();
        let held = DONOR_TURN_OFF_SHARE * turn_off_mass(age, &composition).value();
        // A white dwarf's progenitor has left its main sequence: it is not held under the
        // turn-off.
        let donor_initial = if let (XrbMark::UltracompactDonorInitialMass, Some(log_mass)) =
            (initial_mark, initial_drawn)
        {
            math::exp10(log_mass)
        } else {
            initial_drawn
                .map_or(donor_mass.value(), |excess| {
                    donor_mass.value() * math::exp10(excess)
                })
                .min(held)
        };
        let history = XrbHistory {
            primary_initial_mass: m0,
            donor_initial_mass: SolarMasses::new(
                donor_initial.max(DONOR_INITIAL_MIN.value()).min(m0.value()),
            ),
            age_at_epoch: age,
        };
        let marks = binary.marks(composition, galaxy.seed(), system, age);
        Some(XrbCandidate {
            scan: *scan,
            population: host.population(galaxy),
            composition,
            binary,
            history,
            marks,
        })
    }

    /// The class's test on `candidate`: its timeline from its marks, kept if [`carved_class`]
    /// gives the class and the pair is its kind at the epoch.
    #[must_use]
    pub(crate) fn test_candidate(candidate: XrbCandidate) -> Option<XrbEntry> {
        let XrbCandidate {
            scan,
            population,
            composition,
            binary,
            history,
            marks,
        } = candidate;
        let timeline = BinaryTimeline::from_marks(marks);
        let now = history.age_at_epoch;
        if carved_class(&timeline, now) != Some(CarvedClass::XrayBinary)
            || timeline.class_at(now) != scan.class()
        {
            return None;
        }
        let record = SystemRecord::from_parts(
            scan.id(),
            *scan.position(),
            scan.host().origin(ClassId::XRAY_BINARY),
            population,
            history.primary_initial_mass,
            history.age_at_epoch,
        );
        Some(XrbEntry {
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

/// The mark a low-mass host of `kind` draws its donor's initial mass by: an ultracompact host's
/// donor's progenitor's mass, and every other kind's excess over its mass now.
#[must_use]
const fn donor_initial_mark(kind: XrbHostKind) -> XrbMark {
    match kind {
        XrbHostKind::Ultracompact => XrbMark::UltracompactDonorInitialMass,
        XrbHostKind::NeutronStarPersistent
        | XrbHostKind::NeutronStarTransient
        | XrbHostKind::BlackHolePersistent
        | XrbHostKind::BlackHoleTransient
        | XrbHostKind::BeX
        | XrbHostKind::Supergiant
        | XrbHostKind::Symbiotic
        | XrbHostKind::BlackHoleHighMass => XrbMark::DonorInitialExcess,
    }
}

/// The high-mass side's share of the class's hosts: `table`'s high-mass weights over all
/// its kinds' weights.
#[must_use]
fn high_mass_share(table: &ClassSamplerTable<XrbMark, XrbPick>) -> f64 {
    let sum = |w: &[f64]| w.iter().fold(0.0, |s, x| s + x);
    let low = sum(table.pick(XrbPick::LowMassKind).weights());
    let high = sum(table.pick(XrbPick::HighMassKind).weights());
    high / (low + high)
}

/// An X-ray binary candidate the scan kept, with every mark its test reads.
#[derive(Debug, Clone, PartialEq)]
pub struct XrbCandidate {
    scan: XrbScanMarks,
    population: Population,
    composition: Composition,
    binary: XrbBinary,
    history: XrbHistory,
    marks: BinaryMarks,
}

/// An X-ray binary's binary now (P11.T8.b): the two stars' marked states, the orbit and, in
/// stable transfer, the rate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct XrbBinary {
    kind: XrbHostKind,
    accretor: StarState,
    donor: StarState,
    orbit: KeplerElements,
    transfer_rate: Option<SolarMassesPerYear>,
    donor_attempt: u32,
}

impl XrbBinary {
    /// The host's kind.
    #[must_use]
    pub const fn kind(&self) -> XrbHostKind {
        self.kind
    }

    /// The compact star's marked state, component 0: a neutron star or a black hole.
    #[must_use]
    pub const fn accretor(&self) -> &StarState {
        &self.accretor
    }

    /// The donor's marked state, component 1: filling its Roche lobe, a Be star, or a symbiotic
    /// host's giant inside its lobe.
    #[must_use]
    pub const fn donor(&self) -> &StarState {
        &self.donor
    }

    /// The relative orbit: circular, or a Be/X host's eccentric one.
    #[must_use]
    pub const fn orbit(&self) -> &KeplerElements {
        &self.orbit
    }

    /// The rate of transfer from the donor onto the compact star, M☉ yr⁻¹; `None` for a Be/X or a
    /// symbiotic host, which is marked detached.
    #[must_use]
    pub const fn transfer_rate(&self) -> Option<SolarMassesPerYear> {
        self.transfer_rate
    }

    /// The attempt of the donor's own draws its marks hold: 0, or for a Be/X donor the first that
    /// makes it a Be star.
    #[must_use]
    pub const fn donor_attempt(&self) -> u32 {
        self.donor_attempt
    }

    /// The binary of the host `scan` marks, its compact star's mass, period, donor and rate, its
    /// orbit oriented and phased by `angles`, its stars of `composition`, for system `system` of
    /// the universe of `seed`, `age` old with its compact star `since_formation` old.
    #[must_use]
    fn marked(
        scan: &XrbScanMarks,
        (orientation, mean_anomaly): (Orientation, Radians),
        composition: &Composition,
        (system, seed): (SystemId, Seed),
        (age, since_formation): (Years, Years),
    ) -> Self {
        let kind = scan.kind();
        let accretor_mass = scan.accretor_mass();
        let accretor = compact_state(kind.accretor(), accretor_mass, since_formation, age);
        let orbit_of = |donor: SolarMasses, eccentricity: Eccentricity| {
            KeplerElements::from_period(
                scan.period(),
                GravitationalParameter::from_solar_masses(accretor_mass + donor),
                eccentricity,
                orientation,
                mean_anomaly,
            )
            .expect("a period of hours to a year about a few to some tens of M☉ is an orbit")
        };
        let lobe_of = |donor: SolarMasses, orbit: &KeplerElements| {
            SolarRadii::new(
                roche_lobe_radius(
                    donor.value() / accretor_mass.value(),
                    orbit.semi_major_axis(),
                )
                .value()
                    / SOLAR_RADIUS_M,
            )
        };
        let (donor, orbit, donor_attempt) = match scan.shape() {
            XrbShape::LowMass { donor_mass, .. } => {
                let orbit = orbit_of(donor_mass, Eccentricity::CIRCULAR);
                let lobe = lobe_of(donor_mass, &orbit);
                let donor = low_mass_donor_state(donor_mass, lobe, composition, age);
                (donor, orbit, 0)
            }
            XrbShape::Supergiant { donor_mass, .. } => {
                let orbit = orbit_of(donor_mass, Eccentricity::CIRCULAR);
                let lobe = lobe_of(donor_mass, &orbit);
                let donor =
                    supergiant_donor_state(donor_mass, lobe, composition, (system, seed), age);
                (donor, orbit, 0)
            }
            XrbShape::BeX {
                eccentricity,
                donor_mass,
                ..
            } => {
                let (donor, attempt) = be_donor_state(donor_mass, composition, (system, seed), age);
                (donor, orbit_of(donor.mass(), eccentricity), attempt)
            }
            XrbShape::Ultracompact { donor_mass, .. } => {
                let orbit = orbit_of(donor_mass, Eccentricity::CIRCULAR);
                let lobe = lobe_of(donor_mass, &orbit);
                let donor = donor_state(AwdDonorKind::Helium, donor_mass, lobe, composition, age);
                (donor, orbit, 0)
            }
            XrbShape::Symbiotic {
                donor_mass,
                filling,
                ..
            } => {
                let orbit = orbit_of(donor_mass, Eccentricity::CIRCULAR);
                let lobe = lobe_of(donor_mass, &orbit);
                (
                    symbiotic_giant_state(donor_mass, (lobe, filling), age),
                    orbit,
                    0,
                )
            }
        };
        Self {
            kind,
            accretor,
            donor,
            orbit,
            transfer_rate: scan.transfer_rate(),
            donor_attempt,
        }
    }

    /// The stars' draws: the compact star's attempt 0 and the donor's [`donor_attempt`].
    ///
    /// [`donor_attempt`]: Self::donor_attempt
    #[must_use]
    fn draws(&self, seed: Seed, system: SystemId) -> [StarDraws; 2] {
        [
            StarDraws::for_star(seed, BodyId::new(system, 0)),
            StarDraws::for_attempt(seed, BodyId::new(system, 1), self.donor_attempt),
        ]
    }

    /// The marks of this binary for system `system` of the universe of `seed`, of `composition`,
    /// `age` old at the epoch: its phase across the source horizon about the epoch, stable
    /// transfer from the donor or a Be/X or symbiotic host's detached pair, after a detached
    /// history (design note 10).
    #[must_use]
    fn marks(
        &self,
        composition: Composition,
        seed: Seed,
        system: SystemId,
        age: Years,
    ) -> BinaryMarks {
        let ahead = CLOCK_WINDOW_H.as_julian_years_f64();
        let back = ahead + LIGHT_CROSSING_L.as_julian_years_f64();
        let kind = match self.transfer_rate {
            Some(_) => SegmentKind::StableTransfer {
                donor: Component::Secondary,
            },
            None => SegmentKind::Detached,
        };
        BinaryMarks::phase(
            composition,
            self.draws(seed, system),
            age,
            MarkedPhase {
                kind,
                start: Years::new((age.value() - back).max(0.0)),
                end: Years::new(age.value() + ahead),
                stars: [self.accretor, self.donor],
                orbit: self.orbit,
                transfer_rate: self.transfer_rate,
            },
        )
        .expect("an age past the horizon and a positive rate in transfer are a marked phase")
    }
}

/// An X-ray binary's history (P11.T8.b): what the table draws given the present.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct XrbHistory {
    primary_initial_mass: SolarMasses,
    donor_initial_mass: SolarMasses,
    age_at_epoch: Years,
}

impl XrbHistory {
    /// The compact star's progenitor's initial mass, M☉: the record's primary's.
    #[must_use]
    pub const fn primary_initial_mass(&self) -> SolarMasses {
        self.primary_initial_mass
    }

    /// The donor's initial mass, M☉: a low-mass donor's drawn, a high-mass donor's its mass now;
    /// each held under [`DONOR_TURN_OFF_SHARE`] of the turn-off mass at the system's age but an
    /// ultracompact host's white dwarf's progenitor's, and to 0.08 M☉ and the primary's initial
    /// mass.
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

/// An X-ray binary entry (plan 11, P11.T8.b): a host of the class.
#[derive(Debug, Clone, PartialEq)]
pub struct XrbEntry {
    record: SystemRecord,
    motion: CatalogueMotion,
    scan: XrbScanMarks,
    composition: Composition,
    binary: XrbBinary,
    history: XrbHistory,
    timeline: BinaryTimeline,
}

impl XrbEntry {
    /// The record of the entry's system, of origin
    /// [`SystemOrigin::CatalogueSystem`](crate::galaxy::placement::SystemOrigin::CatalogueSystem)
    /// of the class, at its position at the epoch; for a feature's list entry (P11.T8.f), of its
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
    pub const fn scan(&self) -> &XrbScanMarks {
        &self.scan
    }

    /// The composition both stars share.
    #[must_use]
    pub const fn composition(&self) -> &Composition {
        &self.composition
    }

    /// The binary now.
    #[must_use]
    pub const fn binary(&self) -> &XrbBinary {
        &self.binary
    }

    /// Its history.
    #[must_use]
    pub const fn history(&self) -> &XrbHistory {
        &self.history
    }

    /// The pair's timeline, from its marks.
    #[must_use]
    pub const fn timeline(&self) -> &BinaryTimeline {
        &self.timeline
    }

    /// The system's stars: the compact star's progenitor and the donor, each on its own model at
    /// its initial mass and the system's age with the draws its marks hold, on the marked orbit,
    /// and the pair's timeline, which gives both stars' states.
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
        let stars = self
            .binary
            .draws(galaxy.seed(), system)
            .into_iter()
            .zip(masses)
            .map(|(draws, mass)| {
                StarModel::new_under(mass, self.composition, draws, age, rules)
                    .expect("a host's stars are of 0.08–60 M☉ at a finite age")
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

/// The compact star of kind `accretor` and `mass`, `since_formation` after its birth and `age`
/// old as the system counts it: a neutron star at HPT's cooling luminosity (equation 93) and plan
/// 06's radius, or a black hole at its Schwarzschild radius, dark, as plan 06's tracks leave them.
#[must_use]
fn compact_state(
    accretor: XrayAccretor,
    mass: SolarMasses,
    since_formation: Years,
    age: Years,
) -> StarState {
    let (phase, luminosity) = match accretor {
        XrayAccretor::NeutronStar => (Phase::NeutronStar, hpt_luminosity(mass, since_formation)),
        XrayAccretor::BlackHole => (Phase::BlackHole, SolarLuminosities::ZERO),
    };
    StarState::new(StarStateParts {
        phase,
        age,
        mass,
        core_mass: mass,
        luminosity,
        radius: compact_radius(accretor, mass),
        mass_loss_rate: SolarMassesPerYear::ZERO,
        phase_fraction: 0.0,
    })
}

/// The radius of a compact star of kind `accretor` and `mass`: plan 06's neutron star's, or a black
/// hole's Schwarzschild radius, as plan 06's tracks leave them.
#[must_use]
pub(crate) fn compact_radius(accretor: XrayAccretor, mass: SolarMasses) -> SolarRadii {
    let recipe = RemnantRecipe::default();
    match accretor {
        XrayAccretor::NeutronStar => neutron_star_radius(recipe),
        XrayAccretor::BlackHole => black_hole_radius(recipe, mass),
    }
}

/// Reimers' η of a symbiotic X-ray binary's giant's wind: 0.5, Hurley, Pols and Tout's (2000,
/// MNRAS 315, 543, section 7.1), about which plan 06's stars draw theirs (`Track`'s 0.5 + 0.07 z).
const SYMBIOTIC_GIANT_REIMERS_ETA: f64 = 0.5;

/// The largest radius of a symbiotic X-ray binary's giant: 129 R☉, where its 3,600 K
/// ([`GIANT_DONOR_TEMPERATURE_K`]) gives the red giant branch's tip, about 2,500 L☉ (L ≈ 2.3 × 10⁵
/// Mc⁶ L☉ at a core of Mc ≈ 0.47 M☉; recalled, not read at source).
const SYMBIOTIC_GIANT_MAX_RADIUS: SolarRadii = SolarRadii::new(129.0);

/// A symbiotic X-ray binary's giant of `mass`, its radius `filling` of its Roche lobe of radius
/// `lobe` and no more than [`SYMBIOTIC_GIANT_MAX_RADIUS`], `age` old: on its first giant branch
/// with the accreting white dwarfs' giant donors' helium core ([`GIANT_DONOR_CORE_MASS`], held
/// under 0.9 of its mass) and effective temperature ([`GIANT_DONOR_TEMPERATURE_K`]), its luminosity
/// by Stefan–Boltzmann, and its wind Reimers' (Hurley, Pols and Tout 2000, equation 106, as plan
/// 06's winds take it), η × 4 × 10⁻¹³ L R ÷ M M☉ yr⁻¹ in solar units at
/// [`SYMBIOTIC_GIANT_REIMERS_ETA`], which feeds its neutron star as `classify` reads a symbiotic
/// X-ray binary. A host's scan marks read the same giant (`stellar::binary::scan`).
#[must_use]
pub(crate) fn symbiotic_giant_state(
    mass: SolarMasses,
    (lobe, filling): (SolarRadii, f64),
    age: Years,
) -> StarState {
    let radius = SolarRadii::new((filling * lobe.value()).min(SYMBIOTIC_GIANT_MAX_RADIUS.value()));
    let luminosity = luminosity_of(radius, GIANT_DONOR_TEMPERATURE_K);
    let wind =
        SYMBIOTIC_GIANT_REIMERS_ETA * REIMERS * luminosity.value() * radius.value() / mass.value();
    StarState::new(StarStateParts {
        phase: Phase::FirstGiantBranch,
        age,
        mass,
        core_mass: SolarMasses::new(GIANT_DONOR_CORE_MASS.value().min(0.9 * mass.value())),
        luminosity,
        radius,
        mass_loss_rate: SolarMassesPerYear::new(wind),
        phase_fraction: 0.5,
    })
}

/// A low-mass donor of `mass` filling its Roche lobe of radius `lobe`, `age` old: a main-sequence
/// star at its zero-age luminosity (plan 06's fit at `composition`) while the lobe is under
/// [`EVOLVED_DONOR_LOBE_SHARE`] of its zero-age radius, and beyond that an evolved star on the
/// giant branch, whose core and luminosity follow from its radius by Rappaport et al.'s (1995,
/// MNRAS 273, 731, equations 5 and 6) relations for a degenerate helium core, R = 5,500 Mc^4.5 ÷
/// (1 + 4 Mc⁴) + 0.5 R☉ and L = 10^5.3 Mc⁶ ÷ (1 + 10^0.4 Mc⁴ + 10^0.5 Mc⁵) L☉ (recalled, not read
/// at source in this build), the core held to [`EVOLVED_DONOR_MIN_CORE_MASS`] and 0.9 of the mass.
#[must_use]
fn low_mass_donor_state(
    mass: SolarMasses,
    lobe: SolarRadii,
    composition: &Composition,
    age: Years,
) -> StarState {
    let coeffs = ZCoeffs::new(composition.z_fit());
    let fit_mass = SolarMasses::new(mass.value().max(0.1));
    let zams_radius = zams::radius(fit_mass, &coeffs);
    let (phase, core_mass, luminosity) =
        if lobe.value() <= EVOLVED_DONOR_LOBE_SHARE * zams_radius.value() {
            (
                Phase::MainSequence,
                SolarMasses::ZERO,
                zams::luminosity(fit_mass, &coeffs),
            )
        } else {
            let core = giant_core_mass(lobe)
                .value()
                .max(EVOLVED_DONOR_MIN_CORE_MASS.value())
                .min(0.9 * mass.value());
            let c4 = core * core * core * core;
            let luminosity = math::exp10(5.3) * c4 * core * core
                / (1.0 + math::exp10(0.4) * c4 + math::exp10(0.5) * c4 * core);
            (
                Phase::FirstGiantBranch,
                SolarMasses::new(core),
                SolarLuminosities::new(luminosity),
            )
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

/// The degenerate helium core of a giant of radius `radius`: Rappaport et al.'s (1995, equation 5)
/// core–radius relation inverted by 60 bisections on 0.05–0.5 M☉, over which it rises
/// monotonically from 0.51 R☉ to 195 R☉, held at the ends.
#[must_use]
fn giant_core_mass(radius: SolarRadii) -> SolarMasses {
    let radius_of = |core: f64| {
        let c4 = core * core * core * core;
        5_500.0 * c4 * math::powf_positive(core, 0.5) / (1.0 + 4.0 * c4) + 0.5
    };
    let (mut lo, mut hi) = (0.05, 0.5);
    let r = radius.value();
    if r <= radius_of(lo) {
        return SolarMasses::new(lo);
    }
    if r >= radius_of(hi) {
        return SolarMasses::new(hi);
    }
    for _ in 0..60 {
        let mid = f64::midpoint(lo, hi);
        if radius_of(mid) < r {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    SolarMasses::new(f64::midpoint(lo, hi))
}

/// A supergiant donor of `mass` and `composition`, body 1 of system `system` in the universe of
/// `seed`, filling its Roche lobe of radius `lobe`, `age` old: in its Hertzsprung gap, as the
/// engine's supergiant X-ray binaries' donors are, with a helium core of
/// [`SUPERGIANT_DONOR_CORE_SHARE`] of its mass and the luminosity its own track has at
/// [`SUPERGIANT_DONOR_LIFE_SHARE`] of its life with attempt 0's draws, its photosphere the lobe.
#[must_use]
fn supergiant_donor_state(
    mass: SolarMasses,
    lobe: SolarRadii,
    composition: &Composition,
    (system, seed): (SystemId, Seed),
    age: Years,
) -> StarState {
    let draws = StarDraws::for_star(seed, BodyId::new(system, 1));
    let life = lifetime(track_mass_of(mass), composition, &draws).value();
    let at = Years::new(SUPERGIANT_DONOR_LIFE_SHARE * life);
    let luminosity = Track::to_age(mass, composition, &draws, at)
        .state_at(at)
        .luminosity();
    StarState::new(StarStateParts {
        phase: Phase::HertzsprungGap,
        age,
        mass,
        core_mass: SolarMasses::new(SUPERGIANT_DONOR_CORE_SHARE * mass.value()),
        luminosity,
        radius: lobe,
        mass_loss_rate: SolarMassesPerYear::ZERO,
        phase_fraction: 0.5,
    })
}

/// A Be/X host's Be star of initial mass `mass` and `composition`, body 1 of system `system` in
/// the universe of `seed`, as the system counts it `age` old, and the attempt of its draws that
/// makes it plan 06's Be star (module documentation).
///
/// Its state is its own track's at attempt 0's draws, at the first of [`BE_DONOR_LIFE_SHARES`] of
/// its lifetime at which it is a B star on the main sequence (the last if none is), its age the
/// system's. Its rotation and fossil field are its draws' at the first of [`BE_DONOR_ATTEMPTS`]
/// attempts that make that state a Be star under the rules a timeline from marks classifies by,
/// the generator version's; 0 if none does, which the class's test then refuses.
#[must_use]
fn be_donor_state(
    mass: SolarMasses,
    composition: &Composition,
    (system, seed): (SystemId, Seed),
    age: Years,
) -> (StarState, u32) {
    let body = BodyId::new(system, 1);
    let first = StarDraws::for_star(seed, body);
    let life = lifetime(track_mass_of(mass), composition, &first).value();
    let rules = StellarRules::of_version(GENERATOR_VERSION);
    let last = BE_DONOR_LIFE_SHARES[BE_DONOR_LIFE_SHARES.len() - 1];
    let track = Track::to_age(mass, composition, &first, Years::new(last * life));
    let states = BE_DONOR_LIFE_SHARES.map(|share| track.state_at(Years::new(share * life)));
    let b_star = |state: &StarState| {
        state.phase() == Phase::MainSequence
            && subtype_from_teff_under(state.effective_temperature(), rules)
                .is_some_and(|code| code.letter() == SpectralLetter::B)
    };
    let read = states
        .iter()
        .find(|state| b_star(state))
        .unwrap_or(&states[states.len() - 1]);
    let state = StarState::new(StarStateParts {
        phase: read.phase(),
        age,
        mass: read.mass(),
        core_mass: read.core_mass(),
        luminosity: read.luminosity(),
        radius: read.radius(),
        mass_loss_rate: read.mass_loss_rate(),
        phase_fraction: read.phase_fraction(),
    });
    let attempt = (0..BE_DONOR_ATTEMPTS)
        .find(|&n| {
            is_be_donor(
                &state,
                composition,
                &StarDraws::for_attempt(seed, body, n),
                rules,
            )
        })
        .unwrap_or(0);
    (state, attempt)
}

impl ClassProcess for XrayBinaries {
    type Candidate = XrbCandidate;
    type Entry = XrbEntry;

    fn class(&self) -> ClassId {
        ClassId::XRAY_BINARY
    }

    fn bound(&self, galaxy: &Galaxy, cell: &CatalogueCellKey) -> PerCubicLightYear {
        Self::bound(self, galaxy, cell)
    }

    fn candidate(
        &self,
        galaxy: &Galaxy,
        cell: &CatalogueCellKey,
        id: CatalogueSystemId,
        bound: PerCubicLightYear,
    ) -> Option<XrbCandidate> {
        self.draw_candidate(galaxy, cell, id, bound)
    }

    fn test(&self, _: &Galaxy, candidate: XrbCandidate) -> Option<XrbEntry> {
        Self::test_candidate(candidate)
    }

    fn members(&self, _: &XrbEntry) -> u8 {
        1
    }

    fn system(&self, _: &Galaxy, member: &CatalogueMember<XrbEntry>) -> CatalogueSystem {
        let entry = member.entry().value();
        CatalogueSystem::new(entry.record, entry.motion).with_class_stars()
    }

    fn stars(&self, galaxy: &Galaxy, member: &CatalogueMember<XrbEntry>) -> Option<SystemStars> {
        Some(member.entry().value().stars(galaxy))
    }
}

#[cfg(test)]
mod kinds_tests;
#[cfg(test)]
mod tests;
