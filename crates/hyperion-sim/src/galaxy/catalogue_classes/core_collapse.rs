//! The core-collapse class: the field's recent deaths of massive stars, placed on 512 ly cells of
//! their own (plan 09, P09.T33.a).
//!
//! A layer-E system whose primary dies by core collapse within the supernova test's interval is a
//! catalogue entry, not a grid star (brainstorm, "Events in time": "Rare events are found by
//! carving out their hosts, not the events"). By the marking theorem those systems are a Poisson
//! process of their own, which this module places by thinning, as plan 03 places the grid, on the
//! class grid of P09.T32 ([`ClassProcess`]). The grid side's carve-out, which drops the same
//! systems from the cells, is P09.T35's.
//!
//! # What the class follows
//!
//! The grid's layer E places a system of component c at x with initial mass m in band E and age a
//! at the epoch at the intensity `s_c n_c(x) p(m) f_c(a)`: the component's density, its layer-E
//! share (plan 02's [`ShareMatrix`](crate::galaxy::shares::ShareMatrix)), the band's mass function
//! and the component's age distribution. Its primary dies at T = lifetime − a, so in T the same
//! intensity is `s_c n_c(x) p(m) f_c(lifetime − T)`: the death rate per system at a mass is the age
//! density at its lifetime. Only the components with stars young enough to die in the interval
//! contribute, the young field: the young thin disc and the nuclear disc's last gigayear
//! ([`RECENT_DEATH_AGES`]), each with its own age distribution. From generator version 26 the
//! field gives the features their share (plan 09, P09.T2.c): the share is then the field's, the
//! share matrix's times the component's field factor, and the young disc's ages the young field's,
//! the budget's times `1 − φ(a)` renormalised, so the class follows the field's deaths alone.
//!
//! # The envelope and a candidate
//!
//! A cell's bound is the envelope of that intensity, summed over the young field's components:
//! each one's layer-E density bound over the cell's closed box
//! ([`Component::bound`](crate::galaxy::fields::Component::bound)) × the field's thinning cap,
//! W + L + 2H ([`SupernovaInterval::thinning_cap`]) × its largest death rate per system, the
//! greatest age density it has below [`RECENT_DEATH_AGES`]. Each component carries its own rate
//! because the nuclear disc's, a twentieth of its stars spread over its last gigayear, is some two
//! hundred times below the young thin disc's: thinned against the young disc's, the cells about
//! the centre drew millions of candidates for a few dozen entries. A candidate, on its own
//! streams (Design note 23):
//!
//! 1. draws its site uniformly in the cell and is thinned on the young field's envelope there,
//!    each component's density × its largest death rate, ÷ the bound, the same mark picking its
//!    component by the odds of those terms, as plan 03's candidates pick theirs
//!    (`class.core_collapse.site`);
//! 2. draws its primary's initial mass from the galaxy's mass function in band E
//!    (`class.core_collapse.mass`) and its \[Fe/H\] from its component's law at the site
//!    (`class.core_collapse.metallicity`), read at age zero, since both young components' laws
//!    are flat in age (plan 02's `fields::metallicity`);
//! 3. takes its primary's draws, plan 06's [`StarDraws::for_star`] of its body 0, and their
//!    lifetime;
//! 4. draws T uniformly over the field's interval (−(cap + L + H), +H] and sets its age at the
//!    epoch to lifetime − T (`class.core_collapse.death`, word 0);
//! 5. is thinned on its component's age density at that age ÷ the component's largest (word 1);
//! 6. draws its velocity at the site from its component's law, plan 08's draw
//!    (`class.core_collapse.velocity`), the one velocity its test and its entry read (rule 6).
//!
//! The candidates kept are a Poisson process of intensity `s_c n_c(x) p(m) f_c(lifetime − T)` and
//! the composition, draws and velocity of the grid's systems: the grid's, exactly.
//!
//! # The test
//!
//! A candidate is an entry if and only if plan 08's
//! [`death_in_interval`] on its primary's marks is
//! `Some` and [`claims`] holds on [`DeathMarks::of`] its stars in the
//! field's surroundings, which is the one shared test (P08.T13's interval predicate is part of it;
//! main's ruling, 2026-10-15). A death that makes no shell, a direct collapse to a black hole or a
//! star that ends as a white dwarf, has no marks and is thinned like any other: the class keeps the
//! deaths the grid side would carve, and no other. The entry's window is the one the test cut
//! ([`claimed_window`]).
//!
//! # Runaways and walkaways, from 27
//!
//! A massive star of the young field may die away from where it was born: plan 08's runaways and
//! walkaways (`displaced::runaway`), ejected by an encounter in its young cluster or by its
//! companion's supernova, at the shares, speeds and ejection ages of `RunawayModel`. From
//! `RUNAWAY_DEATHS_VERSION` (27, P09.T33.b) the class follows them too, in the same cells and index
//! space:
//!
//! - **The field term** takes its component's band budget taken to the field, not the share the
//!   field keeps of its systems now (`ShareMatrix::component_share`'s stay share), and keeps a
//!   death only if its ejection marks say it was not ejected before it died
//!   (`class.core_collapse.ejection`): its kind by its mass's runaway and walkaway shares, then
//!   its ejection age among its kind's (the channel with it), judged at its source's reference
//!   lifetime, at which the class table counts its classes.
//! - **One term per band-E class of plan 08's runaways and walkaways and per kind** of each young
//!   component of the thin disc and the nuclear disc (`EjectedTerm`): the class of the ejection's
//!   speed bin and of the age bin of the time from the ejection to the death, its runaways' and
//!   its walkaways' apart, since speed bin 0 holds both. Its density is the class's form (P08.T12.a's
//!   displaced fields) × the component's band-E systems, the whole budget for a runaway's and the
//!   budget taken to the field for a walkaway's × the share of a star's deaths the class can take
//!   as the kind × the death rate of those systems' ages, both bounded by mass (below); the site's
//!   mark picks a term as it picks a component. Its candidate draws its mass by those bounds,
//!   thinned on its class's share of that mass's deaths as the kind against its mass's bound,
//!   then its ejection age among the kind's, its speed inside the class's bin and an azimuth; its
//!   \[Fe/H\] at its
//!   source's reference site, where plan 08's displaced records read theirs; its velocity by its
//!   class's law at the site, plan 08's (Design note 22), as the grid's members of the class
//!   move; then the death-rate thinning on its systems' ages (the budget's for a runaway, the
//!   field's for a walkaway) and the shared test.
//! - **A feature's runaways die in the field** (lean, 2026-11-01; research-leans-21): a runaway
//!   travels some 584 pc before it dies (Renzo et al. 2019, A&A 624, A66, §5.1), about 1,900 ly,
//!   past any bubble, so a feature's list drops a member whose ejection marks put a runaway's
//!   ejection before its death (`feature_runaway`) and the runaways' terms take the whole
//!   budget. A walkaway travels some 103 pc at most (ibid.) and dies inside its feature's bubble,
//!   so the features keep listing theirs and the walkaways' terms keep the field's part.
//! - **The bounds by mass.** The field term and each class's term bound their death rates on each
//!   of band E's 32 node intervals (`MassRates`): the age density's greatest below the oldest age
//!   a death of those masses in the interval can have, the lifetime bracket's longest lifetime
//!   there widened by a quarter, plus the interval; and each term its class's share as the kind
//!   there. A candidate draws its interval by its share of the band's systems × those bounds and
//!   its mass in it by the mass function, then thins against its interval's bounds. Below 100 Myr
//!   the young disc's age density rises tenfold, while band E dies within some 50 Myr, so held to
//!   its densest the field thinned away some 85% of the candidates that had paid for a lifetime.
//! - **The screen.** A candidate the shared test cannot keep is refused before its star is built,
//!   by two bounds on the test that read no star: after the site's pick, a death T at or before
//!   the start of the interval of the longest window the field could cut at the site for either of
//!   the system's core-collapse energies (`field_window_bound`); and at step 3, whose lifetime is
//!   now its death's (`track_under`, the same build), a death that makes no supernova. Neither
//!   refuses a candidate the test keeps, so the entries are the same bit for bit; at the centre's
//!   cell they refuse 286 of the 287 the test would, which had cost a lifetime and a star each.
//! - **One track.** Step 3 builds the primary's whole track, its death and lifetime the same bits
//!   as a build that keeps no segment, and the test builds the primary's stars on it: a death in
//!   the interval is a death by the clock window's end, whose model's track is the whole track,
//!   so a kept candidate pays for one track, not two (P09.T37's bench).
//!
//! So each star's deaths split between the field term, the features and the classes at its mass
//! exactly as the model's shares do: the field drops what the classes take and the features drop
//! their runaways, mass by mass, and the class's runaways and walkaways are `RunawayModel`'s
//! closed form at their budgets. The grid side already asks every layer-E record its claim
//! (P09.T35), a runaway's and a walkaway's with the rest.
//!
//! # The entry
//!
//! An entry has one member, its system: the progenitor, then the supernova, then the bare remnant,
//! by evaluation time ([`SupernovaEntry::state_at`]). It sits at its site at T, moves at its drawn
//! velocity before T and at that plus its primary's natal kick from T ([`CatalogueMotion`]); its
//! record's epoch position is where that puts it at the epoch. Its record is a catalogue system's
//! ([`SystemOrigin::CatalogueSystem`]), its stars plan 06's at its composition with the model's own
//! multiplicity.
//!
//! [`death_in_interval`]: crate::galaxy::displaced::death_in_interval

use std::cell::LazyCell;
use std::sync::Arc;

use super::grid::{CatalogueMember, ClassProcess};
use super::ids::CatalogueCellKey;
use super::registry::ClassId;
use super::supernova::{
    DeathMarks, SupernovaEntry, SupernovaInterval, SupernovaKind, Surroundings, claimed_window,
    claims, field_window_bound, lifetime_cap,
};
use super::system::{CatalogueMotion, CatalogueSystem};
use crate::GeneratorVersion;
use crate::coords::{GalacticPosition, GalacticVelocity};
use crate::galaxy::ages::{AgeDistribution, YOUNG_AGE_LIMIT};
use crate::galaxy::bounds::CellBox;
use crate::galaxy::displaced::class_table::{ClassKey, ClassTable, ejects};
use crate::galaxy::displaced::forms::{FormPoint, young_disc};
use crate::galaxy::displaced::marks::{LifetimeBracket, MARK_MASS_NODES, age_bin_years};
use crate::galaxy::displaced::runaway::{
    Ejected, RUNAWAY_CUT_OF_V_C, RUNAWAY_MIN_SPEED, RUNAWAY_SHARE_HIGH, RunawayModel, WALKAWAY_CUT,
};
use crate::galaxy::displaced::{
    AgeBin, BirthSource, DisplacedClassId, DisplacedFields, DisplacedKind, ExplosionSite,
    SPEED_BINS, SPEED_EDGES, SpeedBin, death_in_interval_knowing,
};
use crate::galaxy::fields::{ComponentId, MAX_COMPONENTS};
use crate::galaxy::imf::{MassBand, MassFunction};
use crate::galaxy::kinematics::{BallisticKick, class_velocity, draw_on};
use crate::galaxy::placement::{SystemOrigin, SystemRecord, is_ballistic};
use crate::galaxy::query::epoch_velocity;
use crate::galaxy::snr::{ShellEnvironment, ShellWindow};
use crate::galaxy::{Galaxy, PointLy, Population};
use crate::id::{BodyId, CatalogueSystemId, Layer, SystemId};
use crate::rng::{DomainTag, Mark, ObjectKey, Stream, Threshold, tags};
use crate::stellar::Composition;
use crate::stellar::draws::StarDraws;
use crate::stellar::multiplicity::MultiplicityContext;
use crate::stellar::sse::{Track, track_under};
use crate::stellar::system::{SystemStars, primary_model_on_track, track_mass_of};
use crate::time::{CLOCK_WINDOW_H, ClockWindow, Span, UniverseTime};
use crate::units::{
    Dex, HeliumExcess, KilometresPerSecond, LightYears, PerCubicLightYear, SolarMasses, Years,
};

/// The ages at the epoch below which a component's stars can die within the supernova test's
/// interval: 100 Myr, the young thin disc's oldest (plan 09, P09.T33.a).
///
/// A recent death of band E has an age at the epoch of its lifetime less T, at most the longest
/// lifetime in the band, an 8 M☉ star's, plus the interval, at most 4.26 Myr: some 50 Myr, inside
/// this (a test holds the mass bracket's longest lifetime at 8 M☉ under it). So the class follows
/// every component with ages below it, the young field, and the largest death rate it thins
/// against is the greatest age density any of them has below it. The old thin disc's youngest
/// sub-disc starts at it, so none of its stars dies recently.
pub const RECENT_DEATH_AGES: Years = YOUNG_AGE_LIMIT;

/// The relative margin a component's largest death rate carries above the age densities it
/// bounds: 2⁻⁴⁰, as plan 02's bounds carry ([`bounds`](crate::galaxy::bounds), "Floating point"),
/// so that a candidate's age density as computed never exceeds it.
const RATE_MARGIN: f64 = 1.0 / 1_099_511_627_776.0;

/// The widening, relative, of the lifetime bracket's longest lifetime over each of band E's node
/// intervals when the class bounds the ages its deaths there can have (from 27, P09.T33.b): 1/4,
/// on top of the bracket's own tenth ([`LifetimeBracket`]), since the bracket is validated over
/// metallicities 10⁻⁴–0.03 and η within ±3, not proved, and a candidate may lie outside both.
const LIFETIME_MARGIN: f64 = 0.25;

/// Band E's node intervals, the class table's 33 mass nodes', over which the class bounds its
/// deaths by mass from 27 (P09.T33.b).
const INTERVALS: usize = MARK_MASS_NODES - 1;

/// The generator version from which the core-collapse class follows the young field's runaways and
/// walkaways (plan 09, P09.T33.b): the field term at its band budget keeps the deaths its ejection
/// marks leave it, and one term per band-E runaway and walkaway class of the thin disc and the
/// nuclear disc places the rest (module documentation). From it too the luminous blue variables'
/// claim leaves the displaced classes' systems on the grid, since that class follows the field's
/// alone (P09.T38's fault; `lbv::claims_its_placement`).
///
/// Below it the class takes the held arm, [`RunawayDeathsRules::FieldOnly`], the code of version 26
/// verbatim, so every generated value keeps its bits; the tests reach the new arm through
/// `GalaxyRules::every_27_hold()`. The bump to 27 deletes this hold: [`RunawayDeathsRules`] with
/// its `FieldOnly` arm, the galaxy rules' `runaway_deaths` field, the match on it in
/// [`CoreCollapseProcess::new`] (whose terms `candidate` and `bound` read) and the one in
/// `lbv::claims_its_placement`, and the tripwire
/// (`the_runaway_deaths_are_held_only_until_the_27_bump`).
pub(crate) const RUNAWAY_DEATHS_VERSION: GeneratorVersion = GeneratorVersion::new(27);

/// Whether the core-collapse class follows the young field's runaways and walkaways (the galaxy
/// rules' `runaway_deaths` field, the hold of [`RUNAWAY_DEATHS_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum RunawayDeathsRules {
    /// Version 26's and before: the field's deaths alone, at the share its field keeps of each
    /// component's systems (P09.T33.a).
    FieldOnly,
    /// Version 27's: the field's deaths not ejected before they die, at its band budget, and the
    /// runaways' and walkaways' deaths, by class (P09.T33.b). A galaxy without plan 08's displaced
    /// classes has no runaways to follow and takes the held arm.
    Ejected,
}

/// A death-rate bound by band E's node interval (from 27, P09.T33.b): over each, the bound on the
/// deaths per system over the supernova test's interval of stars of those masses, and the
/// interval's odds in the envelope, its share of the band's systems × that bound (× a term's share
/// bound), which sum to the term's or the component's death rate in the envelope.
///
/// A star's death in the interval puts its age at the epoch below its lifetime plus the
/// interval's length; over an interval of masses the lifetime bracket's longest lifetime, widened
/// by [`LIFETIME_MARGIN`], bounds the lifetime, and so the greatest age density below that age
/// bounds its death rate. Most of band E dies far younger than the young disc's 100 Myr, where the
/// field's age density is some tenth of its largest (`RECENT_DEATH_AGES`), so a candidate drawn
/// by these odds, its mass in its interval by the mass function, pays a lifetime only where it is
/// likely kept.
#[derive(Debug, Clone, PartialEq)]
struct MassRates {
    rates: [f64; INTERVALS],
    odds: [f64; INTERVALS],
}

impl MassRates {
    /// The odds' sum: the death rate per system in the envelope.
    #[must_use]
    fn total(&self) -> f64 {
        self.odds.iter().fold(0.0, |sum, &odds| sum + odds)
    }

    /// The interval of band E's node `masses` a mark on `stream` picks by the odds, and a mass in it
    /// drawn by `imf` from the next uniform (two words): the last interval with odds should
    /// rounding leave the mark past them all; `None` where no interval has odds.
    #[must_use]
    fn draw(
        &self,
        masses: &[f64; MARK_MASS_NODES],
        imf: &dyn MassFunction,
        stream: &mut Stream,
    ) -> Option<(usize, SolarMasses)> {
        let last = self.odds.iter().rposition(|&odds| odds > 0.0)?;
        let i = stream
            .mark()
            .pick_weighted(&self.odds, self.total())
            .unwrap_or(last);
        let m = imf.quantile_in(masses[i], masses[i + 1], stream.uniform());
        Some((i, SolarMasses::new(m)))
    }
}

/// Band E's node intervals as the bounds by mass read them (from 27, P09.T33.b; [`MassRates`]):
/// each interval's share of the band's systems by the galaxy's mass function, and the oldest age
/// at the epoch a death of its masses in the interval can have, years.
struct MassIntervals {
    shares: [f64; INTERVALS],
    oldest: [f64; INTERVALS],
}

impl MassIntervals {
    /// The intervals of `table`'s band-E nodes in `galaxy`, for an interval of `span` Julian years.
    ///
    /// # Panics
    ///
    /// Never: the nodes lie in band E, where the lifetime bracket answers.
    #[must_use]
    fn of(galaxy: &Galaxy, table: &ClassTable, span: f64) -> Self {
        let masses = *table.nodes(MassBand::E).masses();
        let imf = galaxy.mass_function();
        let band = imf.integral(masses[0], masses[INTERVALS]);
        let shares = std::array::from_fn(|i| imf.integral(masses[i], masses[i + 1]) / band);
        let oldest = std::array::from_fn(|i| {
            let (_, longest) = LifetimeBracket::shared()
                .bracket(SolarMasses::new(masses[i]))
                .expect("band E's nodes are in the bracket's range");
            longest.value() * (1.0 + LIFETIME_MARGIN) + span
        });
        Self { shares, oldest }
    }

    /// The bounds by interval of the deaths per system over the interval of `interval_years` of
    /// stars of `ages`, each scaled by `scale[i]` in the odds: the greatest age density below the
    /// interval's oldest age, raised by 2⁻⁴⁰, × the interval's length.
    #[must_use]
    fn rates(
        &self,
        ages: &AgeDistribution,
        interval_years: f64,
        scale: &[f64; INTERVALS],
    ) -> MassRates {
        let floor = Years::new(-CLOCK_WINDOW_H.as_julian_years_f64());
        let rates: [f64; INTERVALS] = std::array::from_fn(|i| {
            ages.max_pdf_between(floor, Years::new(self.oldest[i]))
                * (1.0 + RATE_MARGIN)
                * interval_years
        });
        let odds = std::array::from_fn(|i| self.shares[i] * scale[i] * rates[i]);
        MassRates { rates, odds }
    }
}

/// One of the runaway and walkaway terms the class follows from 27 (plan 09, P09.T33.b): plan 08's
/// band-E displaced class `class`, of a young component's source, whose density is its form, and
/// the deaths of that component's stars ejected into it as `kind`: every runaway, a feature's
/// with the field's, and the field's walkaways (module documentation).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct EjectedTerm {
    /// The displaced class whose form the term's deaths follow.
    class: DisplacedClassId,
    /// The kind of the term's deaths: a class of speed bin 0 has a term of each.
    kind: Ejected,
    /// The young component the term's stars were born in.
    component: ComponentId,
    /// The component's source, whose classes, reference lifetimes and speed scale it reads.
    source: BirthSource,
    /// The class's speed bin: the ejection's.
    speed: SpeedBin,
    /// The class's age bin, the time from the ejection to the death, for a thin-disc class;
    /// `None` for a nuclear-disc class, which has speed bins only.
    age: Option<AgeBin>,
    /// The component's band-E systems: its whole budget for a runaway's term, whose features drop
    /// their runaways' deaths ([`feature_runaway`]); its budget taken to the field
    /// ([`Galaxy::field_part`]) for a walkaway's, whose features list their walkaways' deaths
    /// themselves (P09.T36.a).
    systems: f64,
    /// An upper bound, over each of band E's node intervals, on the share of a band-E star's
    /// deaths that the class takes as the kind ([`ejected_share_bounds`]), raised by 2⁻⁴⁰. The
    /// candidate thins its mass against its interval's.
    shares: [f64; INTERVALS],
    /// The bounds by interval on the deaths per system of those systems' ages, the budget's for a
    /// runaway's term and the field's for a walkaway's, with odds that take [`shares`](Self::shares)
    /// in too.
    rates: MassRates,
    /// The odds' sum: the term's deaths per system in the envelope, at one system's form.
    weight: f64,
}

impl EjectedTerm {
    /// The term's weight at the point `at`, recent deaths per cubic light-year: its systems × its
    /// class's form there × its [`weight`](Self::weight).
    #[must_use]
    fn weight_at(&self, displaced: &DisplacedFields, at: &FormPoint<'_>) -> f64 {
        self.systems * displaced.density_at(self.class, at) * self.weight
    }

    /// The displaced class whose form the term's deaths follow.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn class(&self) -> DisplacedClassId {
        self.class
    }

    /// The young component the term's stars were born in.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn component(&self) -> ComponentId {
        self.component
    }
}

/// How a runaway's or a walkaway's candidate was ejected (plan 09, P09.T33.b): its class, its kind,
/// its ejection age at its source's reference lifetime and its ejection speed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ejection {
    class: DisplacedClassId,
    kind: DisplacedKind,
    age: Years,
    speed: KilometresPerSecond,
}

impl Ejection {
    /// The displaced class of the ejection's speed bin and the time from it to the death.
    #[must_use]
    pub const fn class(&self) -> DisplacedClassId {
        self.class
    }

    /// A runaway or a walkaway.
    #[must_use]
    pub const fn kind(&self) -> DisplacedKind {
        self.kind
    }

    /// The star's age at its ejection, years, as the class table weighs it.
    #[must_use]
    pub const fn age(&self) -> Years {
        self.age
    }

    /// The ejection's speed, inside the class's speed bin: plan 08's law of its kind (Design note
    /// 24), which the class's velocity reads in its ballistic bins.
    #[must_use]
    pub const fn speed(&self) -> KilometresPerSecond {
        self.speed
    }
}

/// One of the young field's components, as the class thins on it.
#[derive(Debug, Clone, PartialEq)]
pub struct YoungComponent {
    id: ComponentId,
    share: f64,
    deaths_per_system: f64,
    /// From 27 its bounds by mass interval ([`MassRates`]), whose odds sum to its deaths per
    /// system; `None` under the held arm.
    by_mass: Option<MassRates>,
}

impl YoungComponent {
    /// A field candidate's primary's initial mass in band E on `stream`, and the death rate bound
    /// its step 5 thins against: under the held arm a mass by the mass function and the
    /// component's largest rate; from 27, where the class follows `displaced`'s runaways, a mass
    /// by its interval's odds and that interval's rate (`MassRates`). `None` where no interval has
    /// odds.
    #[must_use]
    fn mass(
        &self,
        galaxy: &Galaxy,
        displaced: Option<&DisplacedFields>,
        stream: &mut Stream,
    ) -> Option<(SolarMasses, f64)> {
        let imf = galaxy.mass_function();
        match (displaced, &self.by_mass) {
            (Some(displaced), Some(by_mass)) => {
                let masses = displaced.class_table().nodes(MassBand::E).masses();
                let (i, mass) = by_mass.draw(masses, imf, stream)?;
                Some((mass, by_mass.rates[i]))
            }
            (Some(_) | None, _) => Some((
                SolarMasses::new(imf.sample_in_band(MassBand::E, stream)),
                self.deaths_per_system,
            )),
        }
    }

    /// The component.
    #[must_use]
    pub const fn id(&self) -> ComponentId {
        self.id
    }

    /// Its layer-E share: the fraction of its systems whose primaries are of band E.
    #[must_use]
    pub const fn share(&self) -> f64 {
        self.share
    }

    /// The bound on its deaths per system over the interval: its largest age density below
    /// [`RECENT_DEATH_AGES`], per year, raised by 2⁻⁴⁰, × the interval's length in Julian years;
    /// from 27 (P09.T33.b) the mean, over band E's masses, of each mass interval's bound, the
    /// greatest age density below the oldest age a death of those masses can have.
    #[must_use]
    pub const fn deaths_per_system(&self) -> f64 {
        self.deaths_per_system
    }
}

/// The core-collapse class's process (plan 09, P09.T33.a): the young field's recent deaths of
/// massive stars (module documentation).
///
/// Built from the galaxy alone each time the production lookup visits the class: it holds the
/// young field's components, with their layer-E shares and largest death rates, and the field's
/// thinning interval; from 27 (P09.T33.b) also the runaway and walkaway terms, whose bounds read
/// the class table's ejection ages at its 33 band-E mass nodes, some two hundred lookups.
///
/// # Examples
///
/// The class's cells about the Sun-like point hold a few recent deaths, each resolving by its ID to
/// itself, its window under the field's cap:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::catalogue_classes::core_collapse::CoreCollapseProcess;
/// use hyperion_sim::galaxy::catalogue_classes::{
///     CatalogueCellKey, CatalogueClassCell, ClassId, resolve_entry,
/// };
/// use hyperion_sim::galaxy::snr::ShellEnvironment;
///
/// let galaxy = Galaxy::new(Seed::new(9));
/// let process = CoreCollapseProcess::new(&galaxy);
/// let cap = galaxy.window_caps().cap(ShellEnvironment::Field);
/// for y in 48..52 {
///     let key = CatalogueCellKey::new(ClassId::CORE_COLLAPSE, [0, y, 0])?;
///     for entry in CatalogueClassCell::generate(&galaxy, &process, key).entries() {
///         assert_eq!(resolve_entry(&galaxy, &process, entry.id())?.entry(), entry);
///         assert!(entry.value().supernova().window().duration() <= cap);
///     }
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct CoreCollapseProcess {
    /// The young field's components, in component order.
    components: Vec<YoungComponent>,
    /// The field's thinning interval, at the field's cap.
    interval: SupernovaInterval,
    /// From 27 (P09.T33.b), the runaway and walkaway terms, in the components' order and then the
    /// classes'; empty under the held arm, and wherever the new arm finds no term to place them.
    /// The candidates read whether it is empty, so that the field drops a death only where a term
    /// takes it.
    ejected: Vec<EjectedTerm>,
}

/// A core-collapse candidate the thinning kept, with every mark its test reads.
#[derive(Debug, Clone, PartialEq)]
pub struct CoreCollapseCandidate {
    id: CatalogueSystemId,
    site: GalacticPosition,
    component: ComponentId,
    population: Population,
    mass: SolarMasses,
    composition: Composition,
    draws: StarDraws,
    /// The primary's lifetime under the galaxy's stellar rules, which the shared test's exact tier
    /// reads rather than computing it again.
    lifetime: Years,
    /// The primary's whole track under the galaxy's stellar rules, built for its death, which the
    /// test builds the primary's stars on rather than building it again (P09.T37's bench); `None`
    /// for a test's candidate on a grid record's marks.
    track: Option<Arc<Track>>,
    age_at_epoch: Years,
    velocity: GalacticVelocity,
    ejection: Option<Ejection>,
}

/// A core-collapse entry: a recent death of the young field (plan 09, P09.T33.a).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoreCollapseEntry {
    record: SystemRecord,
    component: ComponentId,
    composition: Composition,
    site: ExplosionSite,
    motion: CatalogueMotion,
    marks: DeathMarks,
    supernova: SupernovaEntry,
    ejection: Option<Ejection>,
}

impl CoreCollapseEntry {
    /// The record of the entry's system, of origin [`SystemOrigin::CatalogueSystem`], at its
    /// epoch position.
    #[must_use]
    pub const fn record(&self) -> &SystemRecord {
        &self.record
    }

    /// The young field's component the system was drawn from.
    #[must_use]
    pub const fn component(&self) -> ComponentId {
        self.component
    }

    /// The composition every star of the system shares.
    #[must_use]
    pub const fn composition(&self) -> &Composition {
        &self.composition
    }

    /// Where and when the primary explodes.
    #[must_use]
    pub const fn site(&self) -> &ExplosionSite {
        &self.site
    }

    /// How the system moves: at its velocity until T, at that plus the kick from T.
    #[must_use]
    pub const fn motion(&self) -> &CatalogueMotion {
        &self.motion
    }

    /// What the supernova test read of the death.
    #[must_use]
    pub const fn marks(&self) -> &DeathMarks {
        &self.marks
    }

    /// The supernova: its time, kind, window and remnant.
    #[must_use]
    pub const fn supernova(&self) -> &SupernovaEntry {
        &self.supernova
    }

    /// How the progenitor was ejected, for a runaway's or a walkaway's death (from 27, P09.T33.b);
    /// `None` for a death in the field.
    #[must_use]
    pub const fn ejection(&self) -> Option<&Ejection> {
        self.ejection.as_ref()
    }

    /// The system's stars: plan 06's at its composition, with the model's own multiplicity.
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

impl CoreCollapseProcess {
    /// The process in `galaxy`: its young field's components, the largest death rate and the
    /// field's interval; from 27 the runaway and walkaway terms, and the field's components at
    /// their band budgets (P09.T33.b).
    ///
    /// # Panics
    ///
    /// Never: the field's window cap is at most `SHELL_WINDOW_CAP`, 4 Myr, whose interval is a
    /// few 10¹⁴ s.
    #[must_use]
    pub fn new(galaxy: &Galaxy) -> Self {
        let fields = galaxy.fields();
        let interval = SupernovaInterval::of_cap(galaxy.window_caps(), ShellEnvironment::Field);
        let interval_years = interval.length().as_julian_years_f64();
        let floor = Years::new(-CLOCK_WINDOW_H.as_julian_years_f64());
        let held: Vec<YoungComponent> = fields
            .component_ids()
            .filter_map(|id| {
                // The field's share and ages: the budget's until P09.T2.c's version 27.
                let share = galaxy.field_component_share(MassBand::E, id);
                let ages = galaxy.field_ages(id);
                let rate = ages.max_pdf_between(floor, RECENT_DEATH_AGES);
                // The old thin disc's youngest sub-disc starts at the limit, which the range's
                // closed top would reach: the limit itself is no young age.
                (share > 0.0 && ages.min() < RECENT_DEATH_AGES && rate > 0.0).then_some(
                    YoungComponent {
                        id,
                        share,
                        deaths_per_system: rate * (1.0 + RATE_MARGIN) * interval_years,
                        by_mass: None,
                    },
                )
            })
            .collect();
        let displaced = match galaxy.rules().runaway_deaths {
            RunawayDeathsRules::FieldOnly => None,
            RunawayDeathsRules::Ejected => fields.displaced(),
        };
        let intervals = displaced
            .map(|displaced| MassIntervals::of(galaxy, displaced.class_table(), interval_years));
        let ejected =
            displaced
                .zip(intervals.as_ref())
                .map_or_else(Vec::new, |(displaced, intervals)| {
                    ejected_terms(galaxy, displaced, &held, intervals, interval_years)
                });
        // From 27 the field's share is its band budget's taken to the field, not what the field
        // keeps of the component's systems now: the ejection marks thin its deaths instead. Its
        // death rate is bounded by mass interval, where band E dies.
        let components = match intervals.filter(|_| !ejected.is_empty()) {
            Some(intervals) => held
                .into_iter()
                .map(|young| {
                    let rates = intervals.rates(
                        galaxy.field_ages(young.id),
                        interval_years,
                        &[1.0; INTERVALS],
                    );
                    YoungComponent {
                        share: galaxy.field_part(
                            young.id,
                            MassBand::E,
                            galaxy
                                .shares()
                                .share(MassBand::E, fields.component(young.id).population()),
                        ),
                        deaths_per_system: rates.total(),
                        by_mass: Some(rates),
                        ..young
                    }
                })
                .collect(),
            None => held,
        };
        Self {
            components,
            interval,
            ejected,
        }
    }

    /// The bytes the process owns on the heap: its components and terms.
    #[must_use]
    pub(crate) fn heap_bytes(&self) -> usize {
        self.components.capacity() * size_of::<YoungComponent>()
            + self.ejected.capacity() * size_of::<EjectedTerm>()
    }

    /// [`ClassProcess::candidate`] with no screen ([`Screening::Off`]): every candidate the
    /// thinning keeps, which the class's tests of its candidates' shares count.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn unscreened_candidate(
        &self,
        galaxy: &Galaxy,
        cell: &CatalogueCellKey,
        id: CatalogueSystemId,
        bound: PerCubicLightYear,
    ) -> Option<CoreCollapseCandidate> {
        self.screened_candidate(galaxy, cell, id, bound, Screening::Off)
    }

    /// The runaway and walkaway terms the class follows: none under the held arm (from 27,
    /// P09.T33.b; module documentation).
    #[cfg(test)]
    #[must_use]
    pub(crate) fn ejected_terms(&self) -> &[EjectedTerm] {
        &self.ejected
    }

    /// The young field's components, in component order: those with layer-E systems and ages
    /// below [`RECENT_DEATH_AGES`].
    #[must_use]
    pub fn components(&self) -> &[YoungComponent] {
        &self.components
    }

    /// The interval over which candidates draw T: the field's, at its cap.
    #[must_use]
    pub const fn interval(&self) -> &SupernovaInterval {
        &self.interval
    }

    /// The envelope's terms at `point` into `out`, in the components' order, recent deaths per
    /// cubic light-year: each component's layer-E share × its density × its deaths per system.
    fn weights(&self, galaxy: &Galaxy, point: &PointLy, out: &mut [f64; MAX_COMPONENTS]) -> usize {
        let fields = galaxy.fields();
        for (slot, young) in out.iter_mut().zip(&self.components) {
            *slot =
                young.share * fields.component(young.id).density(point) * young.deaths_per_system;
        }
        self.components.len()
    }

    /// The galaxy's displaced classes where the process follows their runaways and walkaways: where
    /// it holds a term.
    #[must_use]
    fn ejecting<'g>(&self, galaxy: &'g Galaxy) -> Option<&'g DisplacedFields> {
        if self.ejected.is_empty() {
            None
        } else {
            galaxy.fields().displaced()
        }
    }
}

/// Whether a candidate is screened by the shared test's cheaper bounds before its star is built
/// (from 27, P09.T33.b; [`CoreCollapseProcess::screened_candidate`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screening {
    /// Under the new arm, refuse where the shared test cannot keep the candidate: the cells' and
    /// every lookup's.
    Shared,
    /// Draw every candidate the thinning keeps: the class's own tests of its candidates' shares.
    #[cfg(test)]
    Off,
}

/// The margin on a candidate's drawn T, a Julian year, by which
/// [`CoreCollapseProcess::out_of_reach`] allows for its death's own T: lifetime − (lifetime − T)
/// in years, rounded off by some 10⁻⁸ yr on a lifetime of 10⁷ yr.
const SCREEN_MARGIN: Span = Span::from_seconds(31_557_600);

/// The index `mark` picks among `weights` against `bound`, as [`Mark::pick_weighted`] picks over a
/// slice: the first whose running total's ratio to the bound lies above the mark, or `None` past
/// them all. Here each weight is computed only as the pick reaches it, so a term's form is read
/// only when no earlier weight took the mark.
#[must_use]
fn pick_lazily(mark: Mark, weights: impl IntoIterator<Item = f64>, bound: f64) -> Option<usize> {
    let mut sum = 0.0;
    for (index, weight) in weights.into_iter().enumerate() {
        sum += weight;
        if mark.is_below(Threshold::from_ratio(sum, bound)) {
            return Some(index);
        }
    }
    None
}

/// A source's band-E mass nodes as the terms' bound reads them (P09.T33.b): the class table's
/// masses, their reference lifetimes, and each kind's ejection ages at each node.
struct SourceNodes {
    masses: [f64; MARK_MASS_NODES],
    lifetimes: [f64; MARK_MASS_NODES],
    ages: Vec<[Vec<(Years, f64)>; 2]>,
}

impl SourceNodes {
    /// `source`'s nodes in `table`.
    #[must_use]
    fn of(table: &ClassTable, source: BirthSource) -> Self {
        let masses = *table.nodes(MassBand::E).masses();
        let lifetimes = masses.map(|m| {
            table
                .reference_lifetime(source, MassBand::E, SolarMasses::new(m))
                .value()
        });
        let ages = masses
            .iter()
            .map(|&m| KINDS.map(|kind| table.ejection_ages(source, kind, SolarMasses::new(m))))
            .collect();
        Self {
            masses,
            lifetimes,
            ages,
        }
    }
}

/// The runaway and walkaway terms of `galaxy`'s young field `components` over an interval of
/// `interval_years` (P09.T33.b): for each young component of a source that ejects (the thin disc
/// and the nuclear disc), in the components' order, each of its source's classes with a form that
/// can take a share of some band-E star's deaths, in the classes' order, and each kind its speed
/// law puts in the class's bin, runaways first. A runaway's term is at the component's whole
/// band-E budget and its ages; a walkaway's at the budget taken to the field and the field's ages,
/// the field term's (module documentation). Each takes the class's share bounds as the kind and
/// its ages' death rates, by band E's node `intervals`.
#[must_use]
fn ejected_terms(
    galaxy: &Galaxy,
    displaced: &DisplacedFields,
    components: &[YoungComponent],
    intervals: &MassIntervals,
    interval_years: f64,
) -> Vec<EjectedTerm> {
    let table = displaced.class_table();
    let model = table.runaway_model();
    let fields = galaxy.fields();
    let mut terms = Vec::new();
    for young in components {
        let source = table.component_source(young.id);
        if !ejects(source) {
            continue;
        }
        let component = fields.component(young.id);
        let budget = component.count() * galaxy.shares().share(MassBand::E, component.population());
        // A runaway's at the whole budget and its ages, a walkaway's at the field's part and the
        // field's ages, the field term's.
        let budgets = KINDS.map(|kind| match kind {
            Ejected::Runaway => (budget, component.ages()),
            Ejected::Walkaway => (
                galaxy.field_part(young.id, MassBand::E, budget),
                galaxy.field_ages(young.id),
            ),
        });
        let nodes = SourceNodes::of(table, source);
        let speeds = KINDS.map(|kind| model.speed_bins(kind, table.speed_scale(source)));
        let tau_unit = table.scales().tau_unit();
        for (index, class) in displaced.classes().iter().enumerate() {
            let (speed, age) = match class.key() {
                ClassKey::Thin { speed, age } if source == BirthSource::ThinDisc => {
                    (speed, Some(age))
                }
                ClassKey::Old { source: old, speed } if old.source() == source => (speed, None),
                ClassKey::Thin { .. } | ClassKey::Old { .. } | ClassKey::Hypervelocity => continue,
            };
            if class.form().is_none() {
                continue;
            }
            for (k, (&kind, &(systems, ages))) in KINDS.iter().zip(&budgets).enumerate() {
                let shares = ejected_share_bounds(
                    &nodes,
                    model,
                    (kind, speed, age.map(|age| age_bin_years(age, tau_unit))),
                    &speeds[k],
                )
                .map(|share| share * (1.0 + RATE_MARGIN));
                let rates = intervals.rates(ages, interval_years, &shares);
                let weight = rates.total();
                if weight <= 0.0 || systems <= 0.0 {
                    continue;
                }
                terms.push(EjectedTerm {
                    class: DisplacedClassId::new(u16::try_from(index).expect("under 97 classes")),
                    kind,
                    component: young.id,
                    source,
                    speed,
                    age,
                    systems,
                    shares,
                    rates,
                    weight,
                });
            }
        }
    }
    terms
}

/// The two kinds of an ejected star, in the order the terms sum them.
const KINDS: [Ejected; 2] = [Ejected::Runaway, Ejected::Walkaway];

/// The most ways a star of a given mass can die in one term ([`ejected_parts`]): every ejection
/// age of its kind, a runaway's two 16-node rules or a walkaway's one.
const MAX_PARTS: usize = 32;

/// Upper bounds, over each of band E's node intervals, on the share of a star's deaths that the
/// class of speed bin `speed` and, for a thin-disc class, the age bin spanning `bin` (years) takes
/// as `kind`, [`ejected_parts`]' sum, the kind's shares in the speed bins `speeds`, at its
/// source's `nodes` (P09.T33.b).
///
/// Between two of the class table's mass nodes a star's reference lifetime is linear in log mass,
/// so its range there is its ends'. Where the nodes' lifetimes do not rise with mass, an ejection
/// age (an encounter's, fixed, or the lifetime of a companion heavier in a fixed ratio) does not
/// rise either, so its range is its ends' too, and the time from it to the death lies between
/// those ends' least and greatest. On each interval the bound is then the kind's lifetime share at
/// the heavier end (neither kind's falls with mass), times its share in the speed bin, times the
/// weights of the ejection ages that can fall before the death with that time in the age bin.
/// Should the lifetimes rise anywhere, every ejection age counts, the kind's largest share at any
/// mass, in every interval.
#[must_use]
fn ejected_share_bounds(
    nodes: &SourceNodes,
    model: RunawayModel,
    (kind, speed, bin): (Ejected, SpeedBin, Option<(Years, Option<Years>)>),
    speeds: &[f64; SPEED_BINS],
) -> [f64; INTERVALS] {
    let in_bin = speeds[speed.index()];
    if nodes.lifetimes.windows(2).any(|w| w[1] > w[0]) {
        return [largest_lifetime_share(model, kind) * in_bin; INTERVALS];
    }
    let k = kind_slot(kind);
    std::array::from_fn(|i| {
        let heavy = SolarMasses::new(nodes.masses[i + 1]);
        let (shortest, longest) = (nodes.lifetimes[i + 1], nodes.lifetimes[i]);
        let share = lifetime_share(model, kind, heavy) * in_bin;
        if share <= 0.0 {
            return 0.0;
        }
        let weight = nodes.ages[i][k]
            .iter()
            .zip(&nodes.ages[i + 1][k])
            .filter(|((early, _), (late, _))| {
                let earliest = early.value().min(late.value());
                let latest = early.value().max(late.value());
                let (soonest, slowest) = (shortest - latest, longest - earliest);
                longest > earliest
                    && bin.is_none_or(|(lo, hi)| {
                        slowest >= lo.value() && hi.is_none_or(|hi| soonest < hi.value())
                    })
            })
            .fold(0.0, |sum, ((_, w), _)| sum + w);
        share * weight
    })
}

/// `kind`'s place in [`KINDS`], and so in a [`SourceNodes`]' ages.
#[must_use]
const fn kind_slot(kind: Ejected) -> usize {
    match kind {
        Ejected::Runaway => 0,
        Ejected::Walkaway => 1,
    }
}

/// The largest lifetime share of `kind` at any mass: the runaways' [`RUNAWAY_SHARE_HIGH`], from
/// 20 M☉, and the walkaways' scale W, its ramp being at most 1.
#[must_use]
fn largest_lifetime_share(model: RunawayModel, kind: Ejected) -> f64 {
    match kind {
        Ejected::Runaway => RUNAWAY_SHARE_HIGH,
        Ejected::Walkaway => model.walkaway_scale(),
    }
}

/// The lifetime share of a star of initial mass `m` that becomes `kind`.
#[must_use]
fn lifetime_share(model: RunawayModel, kind: Ejected, m: SolarMasses) -> f64 {
    match kind {
        Ejected::Runaway => model.runaway_share(m),
        Ejected::Walkaway => model.walkaway_share(m),
    }
}

/// One way a star of a given mass dies in a runaway's or walkaway's term: its ejection age and
/// the share of the star's deaths it stands for.
#[derive(Debug, Clone, Copy, PartialEq)]
struct EjectedPart {
    age: Years,
    share: f64,
}

/// No part: a buffer's filler.
const NO_PART: EjectedPart = EjectedPart {
    age: Years::ZERO,
    share: 0.0,
};

/// The share of a band-E star of initial mass `m`'s deaths that `term`'s class takes as the
/// term's kind, by ejection age into the first parts of `parts`, with their number and sum
/// (P09.T33.b): for each of the kind's ejection ages (the class table's `ejection_ages`, both
/// channels), the kind's lifetime share × the ejection's weight × its speed law's share in the
/// class's bin, where the ejection comes before the star's death at its source's reference
/// lifetime and, for a thin-disc class, the time from it to the death lies in the class's age bin.
/// Summed over a source's classes and both kinds it is the share of the star's deaths after an
/// ejection, which the field term's marks drop.
fn ejected_parts(
    table: &ClassTable,
    term: &EjectedTerm,
    m: SolarMasses,
    parts: &mut [EjectedPart; MAX_PARTS],
) -> (usize, f64) {
    let model = table.runaway_model();
    let life = table
        .reference_lifetime(term.source, MassBand::E, m)
        .value();
    let v_ref = table.speed_scale(term.source);
    let bin = term
        .age
        .map(|age| age_bin_years(age, table.scales().tau_unit()));
    let share = lifetime_share(model, term.kind, m);
    let speed = model.speed_bins(term.kind, v_ref)[term.speed.index()];
    if share <= 0.0 || speed <= 0.0 {
        return (0, 0.0);
    }
    let (mut count, mut total) = (0, 0.0);
    for (age, weight) in table.ejection_ages(term.source, term.kind, m) {
        let since = life - age.value();
        let in_bin = bin
            .is_none_or(|(lo, hi)| lo.value() <= since && hi.is_none_or(|hi| since < hi.value()));
        if age.value() >= life || !in_bin {
            continue;
        }
        // A kind's ages number 32 at most, so every part has its slot.
        let Some(slot) = parts.get_mut(count) else {
            debug_assert!(false, "more than {MAX_PARTS} ejection ages at {m:?}");
            continue;
        };
        let part = share * weight * speed;
        total += part;
        *slot = EjectedPart { age, share: part };
        count += 1;
    }
    (count, total)
}

/// The kind a star of `source`, of initial mass `m`, was ejected as before it died, if it was, by
/// its ejection marks on `stream` (`class.core_collapse.ejection`; P09.T33.b): word 0 picks a
/// runaway, a walkaway or neither by the mass's lifetime shares; for a runaway or a walkaway, word
/// 1 picks its ejection age among its kind's by weight (an encounter's or a supernova's, the
/// channel with it); and it was ejected before its death if that age is below its source's
/// reference lifetime at the mass, as the class table weighs its classes. A star of a source that
/// ejects none draws nothing. The field term drops a death ejected as either kind, a feature's
/// list one ejected as a runaway ([`feature_runaway`]).
#[must_use]
fn ejection_before_death(
    table: &ClassTable,
    source: BirthSource,
    m: SolarMasses,
    stream: &mut Stream,
) -> Option<Ejected> {
    if !ejects(source) {
        return None;
    }
    let model = table.runaway_model();
    let shares = KINDS.map(|kind| lifetime_share(model, kind, m));
    let odds = [shares[0], shares[1], (1.0 - shares[0] - shares[1]).max(0.0)];
    let total = odds[0] + odds[1] + odds[2];
    let kind = match stream.mark().pick_weighted(&odds, total) {
        Some(0) => Ejected::Runaway,
        Some(1) => Ejected::Walkaway,
        Some(_) | None => return None,
    };
    let ages = table.ejection_ages(source, kind, m);
    let total = ages.iter().fold(0.0, |sum, &(_, w)| sum + w);
    let last = ages.iter().rposition(|&(_, w)| w > 0.0)?;
    let i = pick_lazily(stream.mark(), ages.iter().map(|&(_, w)| w), total).unwrap_or(last);
    (ages[i].0 < table.reference_lifetime(source, MassBand::E, m)).then_some(kind)
}

/// The class table of `galaxy`'s runaways and walkaways where the core-collapse class follows them
/// (from 27, P09.T33.b): where its process holds a term, so under the new arm in a galaxy with plan
/// 08's displaced classes; `None` otherwise. The features' death lists ([`feature_runaway`]) and
/// living counts (P09.T9.f's `class_counts`) read `RunawayModel` there, so that all three switch
/// together.
#[must_use]
pub(crate) fn ejecting_table(galaxy: &Galaxy) -> Option<&ClassTable> {
    galaxy
        .core_collapse()
        .ejecting(galaxy)
        .map(DisplacedFields::class_table)
}

/// Whether a feature's member of `component`, of initial mass `m` and feature-level key `key`, was
/// ejected as a runaway before it died, by the field term's ejection marks on its own key
/// ([`ejection_before_death`]; from 27, P09.T33.b; lean, 2026-11-01, research-leans-21): its death
/// is then the class's, in a runaway's term at the whole budget, and the feature's list drops it.
/// A walkaway's death stays the feature's. `false` wherever the class follows no runaway term:
/// under the held arm, and in a galaxy without plan 08's displaced classes.
///
/// A member's key is its feature-level member ID's and a class candidate's its catalogue ID's:
/// the two kinds of system ID never meet, so neither reads the other's marks.
#[must_use]
pub(crate) fn feature_runaway(
    galaxy: &Galaxy,
    component: ComponentId,
    m: SolarMasses,
    key: ObjectKey,
) -> bool {
    let Some(table) = ejecting_table(galaxy) else {
        return false;
    };
    let mut stream = Stream::open(galaxy.seed(), tags::CLASS_CORE_COLLAPSE_EJECTION, key);
    ejection_before_death(table, table.component_source(component), m, &mut stream)
        == Some(Ejected::Runaway)
}

/// The speed of `kind`'s law, km/s, at rank `u` in [0, 1) inside speed bin `bin` of a source of
/// circular speed `v_ref`: the law's inverse CDF restricted to the bin's part of its support, by
/// 60 bisections; the bin's part's lower end where the law puts nothing in it.
#[must_use]
fn speed_in_bin(
    model: RunawayModel,
    kind: Ejected,
    bin: SpeedBin,
    v_ref: KilometresPerSecond,
    u: f64,
) -> KilometresPerSecond {
    let reference = v_ref.value();
    let (law_lo, law_hi) = match kind {
        Ejected::Runaway => (RUNAWAY_MIN_SPEED.value(), RUNAWAY_CUT_OF_V_C * reference),
        Ejected::Walkaway => (0.0, WALKAWAY_CUT.value()),
    };
    let index = bin.index();
    let lo = if index == 0 {
        0.0
    } else {
        SPEED_EDGES[index - 1] * reference
    };
    let hi = SPEED_EDGES
        .get(index)
        .map_or(f64::INFINITY, |e| e * reference);
    let (a, b) = (lo.max(law_lo), hi.min(law_hi));
    if b <= a {
        return KilometresPerSecond::new(a);
    }
    let cdf = |v: f64| model.speed_cdf(kind, KilometresPerSecond::new(v), v_ref);
    let (low, high) = (cdf(a), cdf(b));
    let target = low + u.clamp(0.0, 1.0) * (high - low);
    let (mut lo, mut hi) = (a, b);
    for _ in 0..60 {
        let mid = f64::midpoint(lo, hi);
        if cdf(mid) < target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    KilometresPerSecond::new(f64::midpoint(lo, hi))
}

impl ClassProcess for CoreCollapseProcess {
    type Candidate = CoreCollapseCandidate;
    type Entry = CoreCollapseEntry;

    fn class(&self) -> ClassId {
        ClassId::CORE_COLLAPSE
    }

    /// Σ, over the young field's components in order, its layer-E share × its density bound over
    /// the cell's closed box ([`Component::bound`](crate::galaxy::fields::Component::bound)) × its
    /// deaths per system, and from 27 (P09.T33.b) then over the runaway and walkaway terms in
    /// order, each one's systems × its class's form bound over the box (plan 08's P08.T11) × its
    /// deaths per system by mass, its share and rate bounds' odds summed: each term bounds the
    /// matching weight of the site's thinning, in the same arithmetic, so their sum bounds the
    /// weights' running total.
    ///
    /// # Panics
    ///
    /// Never: a class cell is a cell box of the root cube that straddles no plane.
    fn bound(&self, galaxy: &Galaxy, cell: &CatalogueCellKey) -> PerCubicLightYear {
        let cube = CellBox::new(cell.origin_ly(), cell.edge_ly())
            .expect("a class cell is a cell box of the root cube");
        let fields = galaxy.fields();
        let field = self.components.iter().fold(0.0, |sum, young| {
            sum + young.share * fields.component_bound(young.id, &cube) * young.deaths_per_system
        });
        // From 27, the runaway and walkaway terms after the components, each its class's form
        // bound over the cell in the terms' weights' arithmetic (P09.T33.b).
        let total = self.ejecting(galaxy).map_or(field, |displaced| {
            self.ejected.iter().fold(field, |sum, term| {
                sum + term.systems * displaced.bound(term.class, &cube) * term.weight
            })
        });
        PerCubicLightYear::new(total)
    }

    fn candidate(
        &self,
        galaxy: &Galaxy,
        cell: &CatalogueCellKey,
        id: CatalogueSystemId,
        bound: PerCubicLightYear,
    ) -> Option<CoreCollapseCandidate> {
        // From 27 screened by the shared test's cheaper bounds, which refuse no entry.
        self.screened_candidate(galaxy, cell, id, bound, Screening::Shared)
    }

    /// Its death time T, read first (rule 10): word 0 of `class.core_collapse.death`, placed in
    /// the interval as the candidate's step 4 places it. Its entry's T is its primary's death at
    /// the age at the epoch that step sets, lifetime − T: the same time but for the rounding of
    /// lifetime − (lifetime − T) in years, a fraction of a second.
    fn event_time(&self, galaxy: &Galaxy, id: CatalogueSystemId) -> Option<UniverseTime> {
        let key = ObjectKey::from(SystemId::from(id));
        let mut stream = Stream::open(galaxy.seed(), tags::CLASS_CORE_COLLAPSE_DEATH, key);
        Some(self.death_at(stream.uniform()))
    }

    fn test(&self, galaxy: &Galaxy, candidate: CoreCollapseCandidate) -> Option<CoreCollapseEntry> {
        let death = RecentDeath::of(galaxy, &candidate)?;
        let window = claimed_window(galaxy, &death.site, &death.marks, Surroundings::Field)?;
        Some(death.into_entry(&candidate, window))
    }

    fn members(&self, _: &CoreCollapseEntry) -> u8 {
        1
    }

    fn system(&self, _: &Galaxy, member: &CatalogueMember<CoreCollapseEntry>) -> CatalogueSystem {
        let entry = member.entry().value();
        CatalogueSystem::new(entry.record, entry.motion)
            .with_stars(entry.composition, MultiplicityContext::Free)
    }
}

/// A candidate's death with a shell in the interval: the shared test's first half, plan 08's
/// interval predicate, and the marks its second half reads.
#[derive(Debug, Clone, PartialEq)]
struct RecentDeath {
    stars: SystemStars,
    site: ExplosionSite,
    marks: DeathMarks,
}

impl RecentDeath {
    /// The death of `candidate`'s primary, if plan 08's interval predicate
    /// ([`death_in_interval_under`](crate::galaxy::displaced::death_in_interval_under)) puts it in
    /// the interval, under the galaxy's stellar rules as its stars are, and it makes a shell
    /// ([`DeathMarks::of`]). Its exact tier reads the lifetime the candidate computed
    /// ([`death_in_interval_knowing`]), the same bits, so that a kept candidate pays for one
    /// lifetime, not two.
    #[must_use]
    fn of(galaxy: &Galaxy, candidate: &CoreCollapseCandidate) -> Option<Self> {
        // The primary, on a record at the site: it reads no position, and the record the entry
        // keeps is moved to the epoch once the kick is known. Its companions are no members of the
        // entry and are not read (`DeathMarks::of`), so none is drawn here.
        Self::of_record(galaxy, candidate, &candidate.record(candidate.site))
    }

    /// [`RecentDeath::of`] with the primary's stars drawn on `record`'s, the candidate's own at
    /// its site in production; a test passes a grid record whose marks the candidate carries.
    #[must_use]
    fn of_record(
        galaxy: &Galaxy,
        candidate: &CoreCollapseCandidate,
        record: &SystemRecord,
    ) -> Option<Self> {
        let draws = &candidate.draws;
        let death = death_in_interval_knowing(
            candidate.mass,
            &candidate.composition,
            draws.eta(),
            candidate.age_at_epoch,
            || draws.clone(),
            candidate.lifetime,
        )?;
        let stars = candidate.primary_stars(galaxy, record);
        let site = ExplosionSite::new(candidate.site, death);
        let marks = DeathMarks::of(galaxy, &stars, &site, candidate.velocity)?;
        Some(Self { stars, site, marks })
    }

    /// The entry of `candidate`, whose death this is, with the shell `window` the test cut.
    #[must_use]
    fn into_entry(
        self,
        candidate: &CoreCollapseCandidate,
        window: ShellWindow,
    ) -> CoreCollapseEntry {
        let Self { stars, site, marks } = self;
        let death = site.death();
        let kick = stars.natal_kick();
        let remnant = kick.map_or(candidate.velocity, |kick| {
            candidate.velocity + GalacticVelocity::new(kick.velocity())
        });
        let motion = CatalogueMotion::broken(*site.position(), death, candidate.velocity, remnant);
        let record = candidate.record(motion.position_at(UniverseTime::EPOCH));
        let kick_speed = kick.map_or(KilometresPerSecond::ZERO, |kick| {
            KilometresPerSecond::from(kick.speed())
        });
        let supernova = SupernovaEntry::new(record.id(), death, marks.kind(), window)
            .with_remnant(kick_speed, stars.primary().neutron_star());
        CoreCollapseEntry {
            record,
            component: candidate.component,
            composition: candidate.composition,
            site,
            motion,
            marks,
            supernova,
            ejection: candidate.ejection,
        }
    }
}

impl CoreCollapseCandidate {
    /// The stars of `record`, the candidate's primary alone:
    /// [`SystemStars::generate_with`] under [`MultiplicityContext::ForcedSingle`], bit for bit,
    /// the primary's model built on the candidate's whole track where `record` is the candidate's
    /// own ([`record`](Self::record), of its ID, initial mass and so draws) and the primary dies by
    /// the clock window's end, as every death in the interval does, so that the test builds no
    /// track again (P09.T37's bench).
    #[must_use]
    fn primary_stars(&self, galaxy: &Galaxy, record: &SystemRecord) -> SystemStars {
        let end =
            record.age_at_epoch().value() + ClockWindow::END.since_epoch().as_julian_years_f64();
        let own = |track: &&Arc<Track>| {
            record.id() == SystemId::from(self.id)
                && track.lifetime().is_some_and(|life| life.value() <= end)
        };
        match self.track.as_ref().filter(own) {
            Some(track) => {
                let (primary, _) =
                    primary_model_on_track(galaxy, record, self.composition, Arc::clone(track));
                SystemStars::generate_of_primary(
                    galaxy,
                    record,
                    primary,
                    MultiplicityContext::ForcedSingle,
                )
            }
            None => SystemStars::generate_with(
                galaxy,
                record,
                &self.composition,
                MultiplicityContext::ForcedSingle,
            ),
        }
    }

    /// The candidate's record with its system at `position` at the epoch.
    #[must_use]
    fn record(&self, position: GalacticPosition) -> SystemRecord {
        SystemRecord::from_parts(
            SystemId::from(self.id),
            position,
            SystemOrigin::CatalogueSystem(ClassId::CORE_COLLAPSE),
            self.population,
            self.mass,
            self.age_at_epoch,
        )
    }
}

impl CoreCollapseProcess {
    /// Candidate `id` of `cell` under the envelope `bound`, by the module documentation's steps;
    /// from 27 (P09.T33.b), under [`Screening::Shared`], none where the shared test cannot keep it
    /// by bounds that cost no star: its death T past every window the field could cut at its site
    /// ([`out_of_reach`](Self::out_of_reach)), read after the site's pick, and its death no
    /// supernova, read from the lifetime's own build ([`track_under`]). The test refuses either,
    /// so the entries are the same; the candidates the class's own tests count are
    /// [`Screening::Off`]'s.
    #[must_use]
    fn screened_candidate(
        &self,
        galaxy: &Galaxy,
        cell: &CatalogueCellKey,
        id: CatalogueSystemId,
        bound: PerCubicLightYear,
        screening: Screening,
    ) -> Option<CoreCollapseCandidate> {
        let seed = galaxy.seed();
        let system = SystemId::from(id);
        let key = ObjectKey::from(system);
        // 1. The site, thinned on the envelope there, the mark picking the component.
        if bound.value().is_nan() || bound.value() <= 0.0 {
            return None;
        }
        let mut stream = Stream::open(seed, tags::CLASS_CORE_COLLAPSE_SITE, key);
        let site =
            cell.position_from_words([stream.next_u64(), stream.next_u64(), stream.next_u64()]);
        let point = PointLy::from(&site);
        let displaced = self.ejecting(galaxy);
        let (picked, n) = self.pick(galaxy, displaced, &point, stream.mark(), bound)?;
        // From 27 a death no window at the site can reach is no entry: refused before any star.
        let screened = displaced.filter(|_| screening == Screening::Shared);
        if screened.is_some() && self.out_of_reach(galaxy, &site, system, key) {
            return None;
        }
        if let (Some(displaced), Some(term)) = (displaced, picked.checked_sub(n)) {
            return self.ejected_candidate(
                galaxy,
                displaced,
                (id, site),
                &self.ejected[term],
                screening,
            );
        }
        let young = &self.components[picked];
        let component = young.id;
        let laws = galaxy.fields().component(component);
        // 2. The primary's initial mass and its [Fe/H]; from 27 the mass by its interval's death
        //    rate bound (`MassRates`), whose bound step 5 thins against.
        let mut stream = Stream::open(seed, tags::CLASS_CORE_COLLAPSE_MASS, key);
        let (mass, rate) = young.mass(galaxy, displaced, &mut stream)?;
        // From 27 its ejection marks: a death after an ejection is a runaway's or a walkaway's
        // term's, not the field's (P09.T33.b).
        if let Some(displaced) = displaced {
            let table = displaced.class_table();
            let mut ejection = Stream::open(seed, tags::CLASS_CORE_COLLAPSE_EJECTION, key);
            if ejection_before_death(
                table,
                table.component_source(component),
                mass,
                &mut ejection,
            )
            .is_some()
            {
                return None;
            }
        }
        let feh = laws.metallicity(&point, Years::ZERO);
        let mut stream = Stream::open(seed, tags::CLASS_CORE_COLLAPSE_METALLICITY, key);
        let composition = Composition::from_fe_h(
            Dex::new(stream.normal(feh.mean().value(), feh.sigma().value())),
            HeliumExcess::ZERO,
        );
        // 3. The primary's draws, at attempt 0 as its record reads them, and its lifetime, as plan
        //    08's predicate's exact tier computes it, under the galaxy's stellar rules: its
        //    death's age, from 27 refused where that death makes no supernova.
        let draws = StarDraws::for_star(seed, BodyId::new(system, 0));
        let track = track_under(
            track_mass_of(mass),
            &composition,
            &draws,
            galaxy.rules().stellar,
        );
        let primary = track.death().expect("a whole track reaches its death");
        if screened.is_some() && SupernovaKind::of_death(&primary).is_none() {
            return None;
        }
        let lifetime = primary.age();
        let life = lifetime.value();
        // 4. T across the field's interval, and the age at the epoch it implies.
        let mut stream = Stream::open(seed, tags::CLASS_CORE_COLLAPSE_DEATH, key);
        let death = self.death_at(stream.uniform());
        let age_at_epoch = Years::new(life - death.since_epoch().as_julian_years_f64());
        // 5. Thinned on the component's death rate at that age: its age density, as deaths per
        //    system over the interval, against the component's largest, from 27 its mass
        //    interval's.
        let deaths = galaxy.field_ages(component).pdf(age_at_epoch)
            * self.interval.length().as_julian_years_f64();
        if !stream.decide(Threshold::from_ratio(deaths, rate)) {
            return None;
        }
        // 6. The one velocity.
        let velocity = site_velocity(galaxy, component, &site, key);
        Some(CoreCollapseCandidate {
            id,
            site,
            component,
            population: laws.population(),
            mass,
            composition,
            draws,
            lifetime,
            track: Some(Arc::new(track)),
            age_at_epoch,
            velocity,
            ejection: None,
        })
    }

    /// Candidate `id`'s marks at its `site` as a death of the runaway or walkaway term `term` of
    /// `displaced`'s classes, after the site's mark picked it (from 27, P09.T33.b; module
    /// documentation), on the field candidate's streams and `class.core_collapse.ejection`:
    ///
    /// 2. its primary's initial mass in band E, its mass interval by the term's odds (the mass
    ///    stream's word 0) and its mass in it by the mass function (word 1), thinned on the
    ///    class's share of that mass's deaths as the term's kind against the interval's bound
    ///    (word 2); then its ejection age among the kind's by their shares (ejection word 0), its
    ///    speed inside the class's bin (word 1) and an azimuth (word 2); and its \[Fe/H\] from its
    ///    component's law at its source's reference site, at age zero;
    /// 3. its primary's draws and lifetime, as a field candidate's, and under `screening` its
    ///    screen on the death's supernova;
    /// 4. T across the interval, and its age at the epoch;
    /// 5. thinned on the age density at that age of the term's systems, the component's budget's
    ///    for a runaway and its field's for a walkaway, against its interval's bound, as a field
    ///    candidate is on the field's;
    /// 6. its velocity at the site by its class's law, plan 08's (Design note 22): in a ballistic
    ///    bin the circular velocity plus the ejection, its vertical part its height over the time
    ///    since the ejection at its reference lifetime; otherwise the class's Gaussian, on
    ///    `class.core_collapse.velocity`.
    #[must_use]
    fn ejected_candidate(
        &self,
        galaxy: &Galaxy,
        displaced: &DisplacedFields,
        (id, site): (CatalogueSystemId, GalacticPosition),
        term: &EjectedTerm,
        screening: Screening,
    ) -> Option<CoreCollapseCandidate> {
        let seed = galaxy.seed();
        let system = SystemId::from(id);
        let key = ObjectKey::from(system);
        let table = displaced.class_table();
        let component = term.component;
        let laws = galaxy.fields().component(component);
        // 2. The mass by its interval's odds, thinned on the class's share of its deaths against
        //    the interval's bound, and the ejection.
        let mut stream = Stream::open(seed, tags::CLASS_CORE_COLLAPSE_MASS, key);
        let masses = table.nodes(MassBand::E).masses();
        let (interval, mass) = term
            .rates
            .draw(masses, galaxy.mass_function(), &mut stream)?;
        let mut parts = [NO_PART; MAX_PARTS];
        let (count, share) = ejected_parts(table, term, mass, &mut parts);
        if !stream.decide(Threshold::from_ratio(share, term.shares[interval])) {
            return None;
        }
        let mut stream = Stream::open(seed, tags::CLASS_CORE_COLLAPSE_EJECTION, key);
        let last = count.checked_sub(1)?;
        let picked = pick_lazily(
            stream.mark(),
            parts[..count].iter().map(|part| part.share),
            share,
        );
        let part = parts[picked.unwrap_or(last)];
        let ejection_speed = speed_in_bin(
            table.runaway_model(),
            term.kind,
            term.speed,
            table.speed_scale(term.source),
            stream.uniform(),
        );
        let azimuth = std::f64::consts::TAU * stream.uniform();
        let feh = laws.metallicity(&table.reference_site(term.source), Years::ZERO);
        let mut stream = Stream::open(seed, tags::CLASS_CORE_COLLAPSE_METALLICITY, key);
        let composition = Composition::from_fe_h(
            Dex::new(stream.normal(feh.mean().value(), feh.sigma().value())),
            HeliumExcess::ZERO,
        );
        // 3. The primary's draws and lifetime, its death's age, refused where that death makes no
        //    supernova.
        let draws = StarDraws::for_star(seed, BodyId::new(system, 0));
        let track = track_under(
            track_mass_of(mass),
            &composition,
            &draws,
            galaxy.rules().stellar,
        );
        let primary = track.death().expect("a whole track reaches its death");
        if screening == Screening::Shared && SupernovaKind::of_death(&primary).is_none() {
            return None;
        }
        let lifetime = primary.age();
        let life = lifetime.value();
        // 4–5. T, the age, and the death rate there of the term's systems: the budget's ages for
        //      a runaway, the field's for a walkaway.
        let mut stream = Stream::open(seed, tags::CLASS_CORE_COLLAPSE_DEATH, key);
        let death = self.death_at(stream.uniform());
        let age_at_epoch = Years::new(life - death.since_epoch().as_julian_years_f64());
        let ages = match term.kind {
            Ejected::Runaway => laws.ages(),
            Ejected::Walkaway => galaxy.field_ages(component),
        };
        let deaths = ages.pdf(age_at_epoch) * self.interval.length().as_julian_years_f64();
        if !stream.decide(Threshold::from_ratio(deaths, term.rates.rates[interval])) {
            return None;
        }
        // 6. The one velocity, by the class's law at the site.
        let since = table
            .reference_lifetime(term.source, MassBand::E, mass)
            .value()
            - part.age.value();
        let ballistic = is_ballistic(displaced.class(term.class).key()).then(|| BallisticKick {
            height: LightYears::new(PointLy::from(&site).z),
            since: Years::new(since),
            speed: ejection_speed.value(),
            azimuth,
        });
        let velocity = ejected_velocity(galaxy, displaced, term.class, &site, ballistic, key);
        let kind = match term.kind {
            Ejected::Runaway => DisplacedKind::Runaway,
            Ejected::Walkaway => DisplacedKind::Walkaway,
        };
        Some(CoreCollapseCandidate {
            id,
            site,
            component,
            population: laws.population(),
            mass,
            composition,
            draws,
            lifetime,
            track: Some(Arc::new(track)),
            age_at_epoch,
            velocity,
            ejection: Some(Ejection {
                class: term.class,
                kind,
                age: part.age,
                speed: ejection_speed,
            }),
        })
    }

    /// The envelope's term `mark` picks at `point` against `bound`, and the number of the
    /// components', which come first: a component's index below it, a runaway or walkaway term's
    /// from it on (from 27, `displaced`'s classes); `None` past them all (step 1).
    #[must_use]
    fn pick(
        &self,
        galaxy: &Galaxy,
        displaced: Option<&DisplacedFields>,
        point: &PointLy,
        mark: Mark,
        bound: PerCubicLightYear,
    ) -> Option<(usize, usize)> {
        let mut weights = [0.0; MAX_COMPONENTS];
        let n = self.weights(galaxy, point, &mut weights);
        let picked = match displaced {
            None => mark.pick_weighted(&weights[..n], bound.value())?,
            // From 27 the same mark picks a runaway or walkaway term after the components. The
            // terms' forms read the point's radius and arm point, found once, when the pick first
            // reaches a term, the same bits as each form's own: a rejected candidate reads every
            // term, and at the centre's cell this cut a candidate's pick from 2.2 to 1.5 µs
            // (release, pod, 16 vCPU; P09.T33.b's cost).
            Some(displaced) => {
                let at = LazyCell::new(|| {
                    FormPoint::new(point, Some(young_disc(galaxy.fields()).1.geometry()))
                });
                pick_lazily(
                    mark,
                    weights[..n].iter().copied().chain(
                        self.ejected
                            .iter()
                            .map(|term| term.weight_at(displaced, &at)),
                    ),
                    bound.value(),
                )?
            }
        };
        Some((picked, n))
    }

    /// Whether the shared test claims no death of candidate `system` at `site` whose T the death
    /// stream's word 0 draws (the candidate's step 4, read first as [`event_time`] reads it): T,
    /// plus [`SCREEN_MARGIN`] for its death's own rounding, at or before the start of the interval
    /// of [`field_window_bound`], the longest window the field could cut there for any of its
    /// core collapses (from 27, P09.T33.b's screen). At the centre's cell it refuses some seven in
    /// ten of the candidates the shared test would refuse, and the supernova screen the rest but
    /// one (2026-11-01).
    ///
    /// [`event_time`]: ClassProcess::event_time
    #[must_use]
    fn out_of_reach(
        &self,
        galaxy: &Galaxy,
        site: &GalacticPosition,
        system: SystemId,
        key: ObjectKey,
    ) -> bool {
        let mut stream = Stream::open(galaxy.seed(), tags::CLASS_CORE_COLLAPSE_DEATH, key);
        let death = self.death_at(stream.uniform());
        let start = SupernovaInterval::of_window(field_window_bound(galaxy, site, system)).start();
        death
            .checked_add(SCREEN_MARGIN)
            .is_some_and(|late| late <= start)
    }

    /// The death time a uniform `u` in [0, 1) places in the interval: +H less `u` × its length,
    /// floored to the nanosecond, so in (−(cap + L + H), +H].
    ///
    /// # Panics
    ///
    /// Never: the interval is a few 10¹⁴ s.
    #[must_use]
    fn death_at(&self, u: f64) -> UniverseTime {
        let back = Span::from_seconds_f64(u * self.interval.length().as_seconds_f64())
            .expect("a share of a 4.3 Myr interval is a span");
        self.interval
            .end()
            .checked_sub(back)
            .expect("the interval lies inside the clock's range")
    }
}

/// A candidate's velocity at its site, galactic: plan 08's draw of its component's law at the site
/// on `class.core_collapse.velocity` (rule 6), or zero in a galaxy built without its kinematic
/// tables, which has no velocities, as plan 08's `epoch_velocity` has it.
#[must_use]
fn site_velocity(
    galaxy: &Galaxy,
    component: ComponentId,
    site: &GalacticPosition,
    key: ObjectKey,
) -> GalacticVelocity {
    class_site_velocity(
        galaxy,
        component,
        site,
        tags::CLASS_CORE_COLLAPSE_VELOCITY,
        key,
    )
}

/// A runaway's or a walkaway's candidate's velocity at its death site `site`, galactic (from 27,
/// P09.T33.b): plan 08's law of its displaced class `class` (Design note 22,
/// `kinematics::class_velocity`), as the grid's members of the class move, with its `ballistic`
/// motion in the class's ballistic bins, on `class.core_collapse.velocity`; or zero in a galaxy
/// built without its kinematic tables, as a field candidate's is.
#[must_use]
fn ejected_velocity(
    galaxy: &Galaxy,
    displaced: &DisplacedFields,
    class: DisplacedClassId,
    site: &GalacticPosition,
    ballistic: Option<BallisticKick>,
    key: ObjectKey,
) -> GalacticVelocity {
    if galaxy.kinematics().is_none() {
        return GalacticVelocity::default();
    }
    let mut stream = Stream::open(galaxy.seed(), tags::CLASS_CORE_COLLAPSE_VELOCITY, key);
    class_velocity(
        galaxy,
        displaced,
        class,
        (site, Layer::E),
        ballistic,
        &mut stream,
    )
    .velocity()
}

/// A class candidate's velocity at its site, galactic: the core collapses', the Type Ia's and the
/// luminous blue variables' one velocity (rule 6).
///
/// It is plan 08's draw of its `component`'s law at `site` on its class's velocity `tag` keyed by
/// `key`, or zero in a galaxy built without its kinematic tables, as plan 08's `epoch_velocity`
/// has it.
#[must_use]
pub(super) fn class_site_velocity(
    galaxy: &Galaxy,
    component: ComponentId,
    site: &GalacticPosition,
    tag: DomainTag,
    key: ObjectKey,
) -> GalacticVelocity {
    if galaxy.kinematics().is_none() {
        return GalacticVelocity::default();
    }
    let mut stream = Stream::open(galaxy.seed(), tag, key);
    draw_on(galaxy, component, site, &mut stream).velocity()
}

/// Whether the core-collapse class claims the grid record `record`, whose primary dies at `site`
/// inside the shared test's interval (plan 09, P09.T35): the record's primary, alone as the
/// class's candidates are (`MultiplicityContext::ForcedSingle`), at its own metallicity draw, makes
/// a shell there ([`DeathMarks::of`], at the record's epoch velocity, plan 08's `epoch_velocity`),
/// and [`claims`] keeps it in the field's surroundings.
///
/// It is the class's test on the grid's side, the second half of the one shared test whose first
/// is the site's [`death_in_interval`]: a recent death is the catalogue's or the grid's and never
/// both (P09.T38's complementarity). The carve-out asks it of each layer-E record with a site under
/// its claimed arm (`displaced::site::recent_death_claim`), and P09.T43's
/// `the_class_s_core_collapse_claim_is_the_grid_s` holds the class's own claim equal to it.
///
/// # Panics
///
/// As [`SystemStars::generate_in`] and plan 08's `epoch_velocity`, for a record that is not a grid
/// record of `galaxy`; and in debug builds, as [`DeathMarks::of`], if `site`'s death is not the
/// primary's, and if the record moves fast enough to outrun the reach before its death
/// ([`lifetime_cap`]'s edge, about 4,649 km/s).
///
/// [`death_in_interval`]: crate::galaxy::displaced::death_in_interval
#[must_use]
pub(crate) fn recent_death_claims(
    galaxy: &Galaxy,
    site: &ExplosionSite,
    record: &SystemRecord,
) -> bool {
    let velocity = epoch_velocity(galaxy, record);
    // Nothing reads an entry before the source horizon, so its progenitor's line is read for at
    // most L + 2H before T, which only one above 4,096 ly ÷ (L + 2H), about 4,649 km/s, could
    // outrun the reach (`lifetime_cap`'s edge; plan 09, "P09.T33.a's leans, kept", `v_fast`).
    debug_assert!(
        lifetime_cap(velocity.speed()).value() >= 0.0,
        "a grid record of {:?} at {} m/s outruns the reach before its death",
        record.id(),
        velocity.speed().value()
    );
    let stars = SystemStars::generate_in(galaxy, record, MultiplicityContext::ForcedSingle);
    DeathMarks::of(galaxy, &stars, site, velocity)
        .is_some_and(|marks| claims(galaxy, site, &marks, Surroundings::Field))
}

/// The class's test on the marks of the grid record `record` (plan 09, P09.T38's
/// complementarity): a candidate of the record's component carrying its primary's initial mass,
/// its age at the epoch, its metallicity draw, its primary's draws and its epoch velocity, dying
/// at `site`, through the two halves [`ClassProcess::test`] takes, its stars drawn on the record's
/// own, as P09.T33.b's agreement test draws them. `site` is the record's explosion site where plan
/// 08's predicate puts its death in the interval; elsewhere the predicate refuses the candidate
/// before it is read. The entry the class keeps of those marks, or `None`.
///
/// # Panics
///
/// As [`recent_death_claims`], for a record that is not a grid record of `galaxy`.
#[cfg(test)]
#[must_use]
pub(crate) fn test_on_grid_marks(
    galaxy: &Galaxy,
    record: &SystemRecord,
    site: GalacticPosition,
) -> Option<CoreCollapseEntry> {
    let id = CatalogueCellKey::containing(ClassId::CORE_COLLAPSE, &site)
        .expect("a site in the root cube")
        .candidate_id(0, 0)
        .expect("the first candidate of a cell");
    let mass = record.primary_initial_mass();
    let composition = crate::stellar::system::draw_metallicity(galaxy, record);
    let draws = crate::stellar::system::primary_draws(galaxy, record);
    let lifetime = crate::stellar::sse::lifetime_under(
        track_mass_of(mass),
        &composition,
        &draws,
        galaxy.rules().stellar,
    );
    let candidate = CoreCollapseCandidate {
        id,
        site,
        component: record.component().expect("a grid record has its component"),
        population: record.population(),
        mass,
        composition,
        draws,
        lifetime,
        track: None,
        age_at_epoch: record.age_at_epoch(),
        velocity: epoch_velocity(galaxy, record),
        ejection: None,
    };
    let death = RecentDeath::of_record(galaxy, &candidate, record)?;
    let window = claimed_window(galaxy, &death.site, &death.marks, Surroundings::Field)?;
    Some(death.into_entry(&candidate, window))
}

#[cfg(test)]
mod runaway_hold;
#[cfg(test)]
mod tests;
