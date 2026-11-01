//! The conditional marks of layers D and E in a galaxy that holds plan 08's displaced classes
//! (P08.T12.c; plan 08, Design notes 17, 19 and 28).
//!
//! A candidate the thinning accepts takes its marks from what it was picked for:
//!
//! - **Layer E, a field component.** Its kind, alive or retained by speed bin, on
//!   `displaced.kind` from the component's [`StayMarks`], then its initial mass from that kind's
//!   tabulated density on `displaced.mass`. Plan 03's `system.primary_mass` and `system.age` go
//!   unused in layer E.
//! - **Layer D or E, a displaced class.** Its kind (remnant, runaway, walkaway) on
//!   `displaced.kind` from the class's [`ConditionalMarks`], its initial mass on `displaced.mass`,
//!   and its birth component at that mass on `displaced.birth`.
//! - **Layer D, the hypervelocity survivors** (from generator version 27, plan 09's P09.T34.b): a
//!   double detonation's donor white dwarf. Its birth component by the components' Type Ia rates
//!   on `displaced.birth`, and its flight ([`Survivors::flight`]), whose speed sets its donor's
//!   mass: a helium white dwarf of that mass, or a C/O one whose progenitor is dead at `Z_ref`
//!   before its launch, as a remnant's is before its death below.
//! - **Layer D, a field component of a source that ejects runaways** (the thin disc and the
//!   nuclear disc): plan 03's mass and age, and the runaway reduction. A living star is rejected
//!   with the probability that it is a runaway or a walkaway already ejected (on `displaced.kind`),
//!   and its mass, age and draws are then taken at the next attempt: the next draws of
//!   `system.primary_mass`, `system.age` and `displaced.kind`, and
//!   [`StarDraws::for_attempt`]. Whether a star is alive is read from the
//!   [`LifetimeBracket`], and [`lifetime`](crate::stellar::lifetime) is called only for an age
//!   inside it. The attempt kept is the record's [`mark_attempt`](SystemRecord::mark_attempt).
//!   Every other layer-D component keeps plan 03's draws, at attempt 0.
//!
//! The age then follows without a fixed point (Design note 19), on `displaced.death`, with the
//! record's own draws at attempt 0 and its source's reference composition `Z_ref`:
//!
//! - a dead record, retained or displaced, keeps `Z_ref` as its composition, at which the class
//!   table weighed its class (lean of 2026-10-31: at a drawn Z its own fate could leave it no kick
//!   in its bin). It takes its fate there ([`fate_of`], at the cost of a lifetime), a time since
//!   death `s` from its component's ages restricted to its age bin after that lifetime, and
//!   `lifetime + s` as its age, so that it died exactly `s` ago. Where the fate can give no kick in
//!   its bin, it draws its mass once more (`progenitor`);
//! - a living one takes its age under `Z_ref` from its component's ages below that lifetime (a
//!   runaway's from those after its ejection, on `runaway.ejection`), draws its metallicity at
//!   that age and takes the same fraction of `lifetime(m, Z)` as its age, so that it is alive.
//!
//! From generator version 27 ([`PLACEMENT_FATES_VERSION`], the redesign research-leans-19 §3
//! sketched) a dead record's fate is the fitted fate table's wherever it can stand for the track's
//! ([`table_fate`]): its route decides the redraw, and the time since death is drawn after the
//! upper end of the table's lifetime bracket in place of the death age, so that the record is dead
//! by the bracket. Where it cannot, the exact fate is taken, as before. The record's age is then no
//! longer its lifetime plus `s` to the bit, but at most the bracket's width more.
//!
//! So placement calls `lifetime` at most twice per accepted layer-E record, and nothing else of
//! the stellar stage but the metallicity draw. A living record's metallicity age is stored with
//! it, at single precision, and the stellar stage's [`draw_metallicity`] reads it, so that it finds
//! the same composition; a dead one's composition is
//! [`progenitor_composition`](SystemRecord::progenitor_composition)'s.
//!
//! [`StayMarks`]: crate::galaxy::displaced::marks::StayMarks
//! [`ConditionalMarks`]: crate::galaxy::displaced::marks::ConditionalMarks
//! [`Survivors::flight`]: crate::galaxy::displaced::survivors::Survivors::flight
//! [`draw_metallicity`]: crate::stellar::system::draw_metallicity

#[cfg(test)]
use std::cell::Cell;

use super::candidate::Picked;
use super::{CellKey, SystemRecord};
use crate::GeneratorVersion;
use crate::coords::GalacticPosition;
use crate::galaxy::Galaxy;
use crate::galaxy::PointLy;
use crate::galaxy::ages::AgeDistribution;
use crate::galaxy::displaced::class_table::{ClassKey, ClassTable, ejects};
use crate::galaxy::displaced::marks::{
    LifetimeBracket, MARK_KINDS, MARK_MASS_NODES, StayCategory, age_between, age_bin_years,
    time_since_death,
};
use crate::galaxy::displaced::runaway::Ejected;
use crate::galaxy::displaced::site::table_bracket;
use crate::galaxy::displaced::survivors::{DonorKind, SurvivorFlight, donor_progenitor_mass};
use crate::galaxy::displaced::{
    BirthSource, DisplacedClassId, DisplacedFields, DisplacedKind, PlacementClass, SpeedBin,
};
use crate::galaxy::fields::ComponentId;
use crate::galaxy::imf::MassBand;
use crate::id::{BodyId, SystemId};
use crate::math;
use crate::rng::{Mark, ObjectKey, PiecewiseLinear, Stream, Threshold, tags};
use crate::stellar::brief::{IRON_CORE_CORNERS, STATED_FACTOR};
use crate::stellar::draws::StarDraws;
use crate::stellar::fates::{FateRoute, FittedFates, stripped_mark_matters};
use crate::stellar::remnant::RemnantKind;
use crate::stellar::remnant::collapse::RemnantDraws;
use crate::stellar::rules::StellarRules;
use crate::stellar::sse::{Fate, MAX_INITIAL_MASS, TrackOptions, fate_of, lifetime_under};
use crate::stellar::system::{draw_metallicity, metallicity_of, primary_draws};
use crate::stellar::{Composition, Phase};
use crate::units::{LightYears, SolarMasses, Years};

use super::record::KickConstraint;

/// The generator version from which a dead record's marks read its progenitor's fate from the
/// fitted fate table where it can stand for the track's (plan 08, P08.T12 at 27; the hold of
/// [`PlacementFateRules`]).
///
/// Below it every galaxy built by `Galaxy::from_params` takes the held arm,
/// [`PlacementFateRules::Exact`], so every generated value keeps its bits. The tests reach the new
/// arm through a galaxy built under `GalaxyRules::every_27_hold()`. The bump to 27 deletes this
/// hold, [`PlacementFateRules`] with its `Exact` arm, the galaxy's `placement_fates` field and its
/// tripwire (`the_placement_fates_are_held_only_until_the_27_bump`).
pub(crate) const PLACEMENT_FATES_VERSION: GeneratorVersion = GeneratorVersion::new(27);

/// How a dead record's marks read its progenitor's fate (the hold of [`PLACEMENT_FATES_VERSION`]).
///
/// It is read where a retained record's, a displaced remnant's or a C/O survivor's marks are
/// drawn, and reaches none of the galaxy's memos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum PlacementFateRules {
    /// Version 26's and before: the exact fate ([`fate_of`]) decides the redraw, and the time
    /// since death is drawn after its death age.
    Exact,
    /// Version 27's: the fate table's route decides the redraw where it can stand for the track's
    /// ([`table_fate`]), and the time since death is drawn after its lifetime bracket's upper
    /// end; elsewhere the exact fate, as under [`PlacementFateRules::Exact`].
    Fitted,
}

/// The initial masses, M☉, below which the Humphreys–Davidson arms build the same track, bit for
/// bit, by the least \[Fe/H\] of `z_fit` from which each holds, ascending: no lighter star's track
/// reaches the limit, past which the arms' winds part (`wind`'s), so that the fate table, fitted
/// under the version's arm, is the table of either arm's fates there. The hold's sweep finds the
/// first track the arms part at 31.7 M☉ at the least `z_fit`, 39.2 at \[Fe/H\] −0.55, 42.8 at
/// −0.25, 49.4 at 0, 51.5 at +0.05 and 54.4 at +0.1, rising with the metallicity between; each
/// step is about nine tenths of the first at its least \[Fe/H\] (−2.3 is `Z_FIT_MIN`'s).
pub(super) const HD_FREE_MASSES: [(f64, f64); 6] = [
    (-2.3, 30.0),
    (-0.55, 36.0),
    (-0.25, 39.0),
    (0.0, 45.0),
    (0.05, 47.0),
    (0.1, 50.0),
];

/// The initial mass below which the Humphreys–Davidson arms build the same track for a star of
/// `composition` ([`HD_FREE_MASSES`]), read at the \[Fe/H\] of its `z_fit`, as the fate table
/// reads it.
#[must_use]
pub(super) fn hd_free_mass(composition: &Composition) -> SolarMasses {
    let fe_h = math::log10(composition.z_fit().value() / 0.02);
    let step = HD_FREE_MASSES
        .iter()
        .rev()
        .find(|&&(least, _)| fe_h >= least)
        .unwrap_or(&HD_FREE_MASSES[0]);
    SolarMasses::new(step.1)
}

/// The attempts layer D's runaway reduction makes before it keeps the last: a star is rejected
/// with the probability that it is an ejected runaway or walkaway, a few per cent at 2.5–8 M☉, so
/// a record that needs more than a handful does not occur.
pub(super) const MAX_MARK_ATTEMPT: u8 = u8::MAX;

/// What the conditional marks give an accepted candidate of layer D or E (P08.T12.c).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct DrawnMarks {
    /// The component the system was born in: the field component the thinning picked, or a
    /// displaced system's birth component.
    pub(super) component: ComponentId,
    /// The primary's initial mass.
    pub(super) mass: SolarMasses,
    /// The age at the epoch.
    pub(super) age: Years,
    /// The placement class.
    pub(super) placement: PlacementClass,
    /// The attempt whose marks were kept.
    pub(super) attempt: u8,
    /// The age, years, the metallicity was read at, where the marks chose one, at the precision
    /// the record stores it.
    pub(super) metallicity_age_years: Option<f32>,
    /// The primary's lifetime at its own composition and draws, where the marks computed it:
    /// what the carve-out hook's exact step reads instead of computing it again (P08.T13). A dead
    /// record whose fate the table read ([`PlacementFateRules::Fitted`]) has none.
    pub(super) lifetime: Option<Years>,
}

#[cfg(test)]
thread_local! {
    /// The [`lifetime`] calls placement has made on this thread: the count of Design note 28's
    /// test.
    pub(super) static LIFETIME_CALLS: Cell<u64> = const { Cell::new(0) };
}

/// The lifetime of a star of `m`, `composition` and `draws` under the galaxy's stellar rules, the
/// death age of the track the stellar stage builds for it, bit for bit; counted in test builds.
#[must_use]
fn lifetime(
    galaxy: &Galaxy,
    m: SolarMasses,
    composition: &Composition,
    draws: &StarDraws,
) -> Years {
    #[cfg(test)]
    LIFETIME_CALLS.with(|calls| calls.set(calls.get() + 1));
    lifetime_under(m, composition, draws, galaxy.rules().stellar)
}

/// The conditional marks of the accepted candidate `id` of `key` at `position`, picked for
/// `picked`, in a galaxy holding `displaced`; `None` where plan 03's draws stand: outside layers D
/// and E, and in layer D for a field component of a source that ejects no runaway.
#[must_use]
pub(super) fn draw(
    galaxy: &Galaxy,
    displaced: &DisplacedFields,
    key: CellKey,
    id: SystemId,
    position: &GalacticPosition,
    picked: Picked,
) -> Option<DrawnMarks> {
    let table = displaced.class_table();
    let band = key.band();
    match (band, picked) {
        (MassBand::E, Picked::Field(pick)) => {
            Some(stay(galaxy, table, id, position, pick.component()))
        }
        (MassBand::D | MassBand::E, Picked::Displaced(class)) => {
            Some(displaced_marks(galaxy, displaced, band, id, class))
        }
        (MassBand::D, Picked::Field(pick)) if ejects(table.component_source(pick.component())) => {
            Some(runaway_reduction(
                galaxy,
                table,
                key,
                id,
                position,
                pick.component(),
            ))
        }
        (_, Picked::Field(_) | Picked::Displaced(_)) => None,
    }
}

/// The streams of one record's conditional marks, keyed by its ID.
fn stream(galaxy: &Galaxy, tag: crate::rng::DomainTag, id: SystemId) -> Stream {
    Stream::open(galaxy.seed(), tag, ObjectKey::from(id))
}

/// The pick of `mark` among `weights` by plan 01's integer thresholds over their own total, or,
/// should rounding leave the mark past every threshold, the last with weight.
///
/// # Panics
///
/// If no weight is positive, which a picked component or class never has.
fn pick(mark: Mark, weights: &[f64]) -> usize {
    let total: f64 = weights.iter().sum();
    assert!(total > 0.0, "a picked component or class has weight");
    mark.pick_weighted(weights, total).unwrap_or_else(|| {
        weights
            .iter()
            .rposition(|&w| w > 0.0)
            .expect("the total is positive")
    })
}

/// A record's metallicity age, rounded to the single precision the record stores it at: the
/// value placement and the stellar stage both read.
#[expect(
    clippy::cast_possible_truncation,
    reason = "an age of at most a few 10¹⁰ years, held to 2⁻²⁴ of itself: the stored value is the \
              one both sides read, so the rounding is part of the draw"
)]
fn stored_age_years(age_years: f64) -> f32 {
    age_years.max(0.0) as f32
}

/// One record's star, as its conditional marks read it: the record, the component it was born
/// in and where, its initial mass and draws at attempt 0, and its lifetime under its source's
/// reference composition.
struct Star<'a> {
    galaxy: &'a Galaxy,
    id: SystemId,
    component: ComponentId,
    site: PointLy,
    mass: SolarMasses,
    draws: StarDraws,
    life_ref: Years,
}

impl Star<'_> {
    /// The star's composition, read at `age_years` (single precision, as the record stores it),
    /// and its lifetime there.
    #[must_use]
    fn at(&self, age_years: f32) -> Years {
        let composition = metallicity_of(
            self.galaxy,
            self.id,
            self.galaxy.fields().component(self.component),
            &self.site,
            Years::new(f64::from(age_years)),
        );
        lifetime(self.galaxy, self.mass, &composition, &self.draws)
    }

    /// The age, metallicity age and lifetime of the star alive, its age under the reference
    /// composition in `[a, b]` of its component's ages at the rank `u` (Design note 19).
    fn living(&self, (a, b): (Years, Years), u: f64) -> (Years, f32, Years) {
        let ages = self.galaxy.fields().component(self.component).ages();
        let age_ref = age_between(ages, a, b, u);
        let fraction = age_ref.value() / self.life_ref.value();
        let metallicity_age = stored_age_years(age_ref.value());
        let life = self.at(metallicity_age);
        (Years::new(fraction * life.value()), metallicity_age, life)
    }
}

/// What a dead record's progenitor was, at its source's reference composition: its initial mass,
/// the component it was born in and its fate (death and remnant).
struct Progenitor {
    mass: SolarMasses,
    component: ComponentId,
    fate: FateRead,
}

/// What a dead record's marks read of its progenitor's fate: the track's, or the fate table's
/// where it can stand for it ([`table_fate`], from [`PLACEMENT_FATES_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum FateRead {
    /// The track's fate, bit for bit.
    Exact(Fate),
    /// The fate table's: its route, whether the star leaves a remnant, which is the same at every
    /// corner of the table's error box, and the upper end of the table's bracket on the death age.
    Table {
        route: FateRoute,
        remnant: bool,
        upper: Years,
    },
}

impl FateRead {
    /// Whether the star leaves a remnant.
    #[must_use]
    fn leaves_remnant(&self) -> bool {
        match self {
            Self::Exact(fate) => fate.remnant.kind() != RemnantKind::None,
            Self::Table { remnant, .. } => *remnant,
        }
    }

    /// Whether the star dies by an iron core's collapse.
    #[must_use]
    fn iron_core(&self) -> bool {
        match self {
            Self::Exact(fate) => fate.iron_core.is_some(),
            Self::Table { route, .. } => *route == FateRoute::IronCore,
        }
    }

    /// The age after which the time since death is drawn, the death age or the upper end of the
    /// table's bracket on it, and the death age where it is the track's.
    #[must_use]
    pub(super) fn death_bound(&self) -> (Years, Option<Years>) {
        match self {
            Self::Exact(fate) => {
                let life = fate.death.age();
                (life, Some(life))
            }
            Self::Table { upper, .. } => (*upper, None),
        }
    }
}

/// The fate of a star of `m`, `composition` and `draws` under the galaxy's stellar rules: the fate
/// table's where the galaxy's rules read it and it can stand for the track's ([`table_fate`]),
/// and otherwise the track's, bit for bit, at the cost of [`lifetime`], whose death age it holds;
/// counted in test builds as a lifetime call.
#[must_use]
fn fate(galaxy: &Galaxy, m: SolarMasses, composition: &Composition, draws: &StarDraws) -> FateRead {
    let m = if m > MAX_INITIAL_MASS {
        MAX_INITIAL_MASS
    } else {
        m
    };
    let rules = galaxy.rules();
    if rules.placement_fates == PlacementFateRules::Fitted
        && let Some(read) = table_fate(rules.stellar, m, composition, draws)
    {
        return read;
    }
    #[cfg(test)]
    LIFETIME_CALLS.with(|calls| calls.set(calls.get() + 1));
    FateRead::Exact(fate_of(
        m,
        composition,
        draws,
        TrackOptions::default().with_rules(rules.stellar),
    ))
}

/// The fate table's read of the fate of a star of `m` (at most [`MAX_INITIAL_MASS`]),
/// `composition` and `draws` under the stellar rules `rules`, or `None` where it cannot stand for
/// the track's:
///
/// - under a Humphreys–Davidson arm the table was not fitted for, from [`hd_free_mass`] up;
/// - where the table does not answer ([`FittedFates::fate_fitted`]): outside its domain, across a
///   change of route, or in a cell whose validation failed;
/// - for a white dwarf's route or no remnant's, where the companion-stripped mark can move the
///   fate ([`stripped_mark_matters`]). An iron core's and an electron capture's it cannot: the
///   stripped star's window of electron capture is wider than the single star's, but ends where
///   the iron cores begin as the single star's does, so the track reads the mark only for a star
///   that would otherwise leave a white dwarf (`Builder::by_companion_mark`);
/// - for an iron core of which some corner of its two cores' box, taken at [`STATED_FACTOR`]
///   times their bounds, leaves a remnant and another none (the range brief's guard asks the kind
///   at the bounds);
/// - where the bracket on the death age has no upper end.
///
/// The bracket is the carve-out's ([`table_bracket`]), the range brief's margin about the table's
/// death age.
#[must_use]
pub(super) fn table_fate(
    rules: StellarRules,
    m: SolarMasses,
    composition: &Composition,
    draws: &StarDraws,
) -> Option<FateRead> {
    if !(rules.hd_limit.fitted_tables_hold() || m < hd_free_mass(composition)) {
        return None;
    }
    let fate = FittedFates::generator().fate_fitted(m, composition, draws.eta())?;
    let remnant = match fate.route {
        FateRoute::HeliumWhiteDwarf
        | FateRoute::CarbonOxygenWhiteDwarf
        | FateRoute::OxygenNeonWhiteDwarf
        | FateRoute::BridgedCarbonOxygenWhiteDwarf
        | FateRoute::BridgedOxygenNeonWhiteDwarf
        | FateRoute::NoRemnant => {
            if stripped_mark_matters(m, composition, draws) {
                return None;
            }
            fate.route != FateRoute::NoRemnant
        }
        FateRoute::ElectronCapture => true,
        FateRoute::IronCore => {
            let remnant_draws = RemnantDraws::of(draws);
            let leaves = |&(_, a, b): &(f64, f64, f64)| {
                let shift = (0.0, STATED_FACTOR * a, STATED_FACTOR * b);
                fate.remnant(remnant_draws, shift).phase != Phase::NoRemnant
            };
            let centre = leaves(&(0.0, 0.0, 0.0));
            if !IRON_CORE_CORNERS
                .iter()
                .all(|corner| leaves(corner) == centre)
            {
                return None;
            }
            centre
        }
    };
    let (_, upper) = table_bracket(&fate);
    upper.value().is_finite().then_some(FateRead::Table {
        route: fate.route,
        remnant,
        upper,
    })
}

/// Whether `fate` can leave a remnant kicked into a speed bin whose least speed is above zero
/// where `moving`, or into the slowest bin otherwise: any remnant reaches the slowest, but only an
/// iron core's collapse gives the ordinary mode's kicks (a white dwarf's are about 1 km/s and an
/// electron-capture neutron star's 5 km/s, plan 06's P06.T19).
#[must_use]
fn kickable(fate: &FateRead, reach: Reach) -> bool {
    fate.leaves_remnant() && (reach == Reach::Slowest || fate.iron_core())
}

/// The speed bin a dead record's kick must reach: the slowest, which any remnant's does, or a
/// faster one, which only an iron core's collapse gives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reach {
    /// The slowest bin, under a quarter of the speed scale.
    Slowest,
    /// Any faster bin.
    Moving,
}

impl Reach {
    /// The reach of speed bin `bin`.
    #[must_use]
    fn of(bin: SpeedBin) -> Self {
        if bin.index() == 0 {
            Self::Slowest
        } else {
            Self::Moving
        }
    }
}

/// Whether [`kick_constraint`] reads a ballistic remnant's vertical component, which costs it a
/// lifetime, or leaves it out, as the kick loop, which reads only the speed range, does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Vertical {
    /// Read it.
    Read,
    /// Leave it out.
    Skipped,
}

/// A dead record's progenitor: its initial mass from its kind's density, given at the mass nodes
/// `masses` as `densities`, on `stream` (`displaced.mass`), its birth component as `born` gives it
/// at that mass, and its fate at its source's reference composition `z_ref` with its `draws`.
///
/// The class table weighed the record's class at that composition and the median star's draws,
/// with the densities linear between its nodes. Where a node and the next straddle a change of
/// fate, such as the end of the electron-capture window near 8.5 M☉, the line puts mass where no
/// iron core collapses, so a record of a moving class (`reach`) whose fate gives it no such kick
/// draws its mass once more, from the density above the node over its first mass. That costs a
/// second fate, so no record calls more than two.
fn progenitor(
    galaxy: &Galaxy,
    (z_ref, draws): (&Composition, &StarDraws),
    (masses, densities): (&[f64; MARK_MASS_NODES], &[f64; MARK_MASS_NODES]),
    born: impl Fn(SolarMasses) -> ComponentId,
    reach: Reach,
    stream: &mut Stream,
) -> Progenitor {
    let density = PiecewiseLinear::new(masses, densities)
        .expect("a picked kind or category has weight, so a mass density");
    let mass = SolarMasses::new(density.sample(stream));
    let first = fate(galaxy, mass, z_ref, draws);
    if kickable(&first, reach) {
        return Progenitor {
            mass,
            component: born(mass),
            fate: first,
        };
    }
    let above = masses.partition_point(|&node| node <= mass.value());
    let Ok(upper) = PiecewiseLinear::new(&masses[above..], &densities[above..]) else {
        return Progenitor {
            mass,
            component: born(mass),
            fate: first,
        };
    };
    let mass = SolarMasses::new(upper.sample(stream));
    Progenitor {
        mass,
        component: born(mass),
        fate: fate(galaxy, mass, z_ref, draws),
    }
}

/// A dead record's age and lifetime: the time since death on `[lo, hi]` of its birth component's
/// ages after its lifetime, at the rank `u`, and the lifetime itself, its fate's death age (Design
/// note 19 at the reference composition). A fate the table read has no lifetime: the time since
/// death is drawn after the upper end of the table's bracket on it ([`FateRead::death_bound`]).
fn dead_age(
    galaxy: &Galaxy,
    star: &Progenitor,
    (lo, hi): (Years, Option<Years>),
    u: f64,
) -> (Years, Option<Years>) {
    let (bound, life) = star.fate.death_bound();
    let ages = galaxy.fields().component(star.component).ages();
    let since = time_since_death(ages, bound, lo, hi, u);
    (Years::new(bound.value() + since.value()), life)
}

/// The primary's draws at attempt 0.
pub(super) fn draws_of(galaxy: &Galaxy, id: SystemId) -> StarDraws {
    StarDraws::for_star(galaxy.seed(), BodyId::new(id, 0))
}

/// A layer-E field record's marks: alive or retained by speed bin, from component `c`'s stay
/// marks (Design note 17).
fn stay(
    galaxy: &Galaxy,
    table: &ClassTable,
    id: SystemId,
    position: &GalacticPosition,
    c: ComponentId,
) -> DrawnMarks {
    let marks = table.stay_marks(c);
    let index = pick(
        stream(galaxy, tags::DISPLACED_KIND, id).mark(),
        marks.weights(),
    );
    let category = match index.checked_sub(1) {
        None => StayCategory::Alive,
        Some(s) => StayCategory::Retained(speed_bin(s)),
    };
    let z_ref = table.reference_composition(table.component_source(c));
    let mut masses = stream(galaxy, tags::DISPLACED_MASS, id);
    let u = stream(galaxy, tags::DISPLACED_DEATH, id).uniform();
    match category {
        StayCategory::Alive => {
            let draws = draws_of(galaxy, id);
            let mass = SolarMasses::new(
                marks
                    .mass_density(category)
                    .expect("a picked category has weight, so a mass density")
                    .sample(&mut masses),
            );
            let life_ref = lifetime(galaxy, mass, &z_ref, &draws);
            let ages = galaxy.fields().component(c).ages();
            let below = Years::new(life_ref.value().min(ages.max().value()));
            let star = Star {
                galaxy,
                id,
                component: c,
                site: PointLy::from(position),
                mass,
                draws,
                life_ref,
            };
            let (age, metallicity_age, life) = star.living((ages.min(), below), u);
            DrawnMarks {
                component: c,
                mass,
                age,
                placement: PlacementClass::Alive,
                attempt: 0,
                metallicity_age_years: Some(metallicity_age),
                lifetime: Some(life),
            }
        }
        StayCategory::Retained(speed) => {
            let star = progenitor(
                galaxy,
                (&z_ref, &draws_of(galaxy, id)),
                (marks.masses(), marks.node_density(category)),
                |_| c,
                Reach::of(speed),
                &mut masses,
            );
            let (age, life) = dead_age(galaxy, &star, (Years::ZERO, None), u);
            DrawnMarks {
                component: c,
                mass: star.mass,
                age,
                placement: PlacementClass::Retained { speed },
                attempt: 0,
                metallicity_age_years: None,
                lifetime: life,
            }
        }
    }
}

/// A displaced record's marks in `band`: its kind, initial mass and birth component from class
/// `class`'s conditional marks, then its age (Design notes 17 and 19).
fn displaced_marks(
    galaxy: &Galaxy,
    displaced: &DisplacedFields,
    band: MassBand,
    id: SystemId,
    class: DisplacedClassId,
) -> DrawnMarks {
    let table = displaced.class_table();
    let key = displaced.class(class).key();
    if key == ClassKey::Hypervelocity {
        return survivor_marks(galaxy, table, id, class);
    }
    let source = key
        .source()
        .expect("only the hypervelocity survivors have no source");
    let marks = table.marks(band, class);
    let kind = MARK_KINDS[pick(
        stream(galaxy, tags::DISPLACED_KIND, id).mark(),
        &marks.kind_odds(),
    )];
    let z_ref = table.reference_composition(source);
    let birth = stream(galaxy, tags::DISPLACED_BIRTH, id).mark();
    let mut masses = stream(galaxy, tags::DISPLACED_MASS, id);
    let u = stream(galaxy, tags::DISPLACED_DEATH, id).uniform();
    let (lo, hi) = class_ages(table, key);
    let placement = PlacementClass::Displaced { class, kind };
    if kind == DisplacedKind::Remnant {
        let origin = origin_bin(galaxy, table, band, class, id);
        let star = progenitor(
            galaxy,
            (&z_ref, &draws_of(galaxy, id)),
            (marks.masses(), &marks.node_density(kind)),
            |m| marks.component_at(kind, m, birth),
            Reach::of(origin),
            &mut masses,
        );
        let (age, life) = dead_age(galaxy, &star, (lo, hi), u);
        return DrawnMarks {
            component: star.component,
            mass: star.mass,
            age,
            placement,
            attempt: 0,
            metallicity_age_years: None,
            lifetime: life,
        };
    }
    let mass = SolarMasses::new(
        marks
            .mass_density(kind)
            .expect("a picked kind has weight, so a mass density")
            .sample(&mut masses),
    );
    let c = marks.component_at(kind, mass, birth);
    let draws = draws_of(galaxy, id);
    let life_ref = lifetime(galaxy, mass, &z_ref, &draws);
    let ages = galaxy.fields().component(c).ages();
    let span = match kind {
        DisplacedKind::Runaway | DisplacedKind::Walkaway => {
            let ejected = if kind == DisplacedKind::Walkaway {
                Ejected::Walkaway
            } else {
                Ejected::Runaway
            };
            let (a, b, _) = since_ejection(
                galaxy,
                table,
                (source, ejected),
                (ages, mass, life_ref),
                (lo, hi),
                id,
            );
            (a, b)
        }
        DisplacedKind::Remnant | DisplacedKind::HypervelocitySurvivor => (
            ages.min(),
            Years::new(life_ref.value().min(ages.max().value())),
        ),
    };
    let star = Star {
        galaxy,
        id,
        component: c,
        site: table.reference_site(source),
        mass,
        draws,
        life_ref,
    };
    let (age, metallicity_age, life) = star.living(span, u);
    DrawnMarks {
        component: c,
        mass,
        age,
        placement,
        attempt: 0,
        metallicity_age_years: Some(metallicity_age),
        lifetime: Some(life),
    }
}

/// A hypervelocity survivor's marks (module documentation): its birth component on
/// `displaced.birth`, and its star, its donor of its flight's mass ([`SurvivorFlight::donor`]):
///
/// - a helium white dwarf is the record's primary itself, of the donor's mass, its age its time
///   since launch, the stellar stage building it as one ([`is_helium_donor`]; heated by its
///   companion's detonation, it cools from its launch; lean, 2026-11-01);
/// - a C/O one's progenitor is of the donor's initial mass by the initial–final relation's inverse
///   ([`donor_progenitor_mass`]), in layer D, dead at its source's reference composition at least
///   its flight's time since launch before the epoch, its age on `displaced.death`.
#[must_use]
fn survivor_marks(
    galaxy: &Galaxy,
    table: &ClassTable,
    id: SystemId,
    class: DisplacedClassId,
) -> DrawnMarks {
    let survivors = table
        .survivors()
        .expect("a survivor is placed only where its class has weight");
    let index = pick(
        stream(galaxy, tags::DISPLACED_BIRTH, id).mark(),
        survivors.births_per_year(),
    );
    let component = galaxy
        .fields()
        .component_id(index)
        .expect("the births are the galaxy's components'");
    let flight = survivors.flight(galaxy.seed(), id);
    let placement = PlacementClass::Displaced {
        class,
        kind: DisplacedKind::HypervelocitySurvivor,
    };
    match flight.donor_kind() {
        DonorKind::Helium => DrawnMarks {
            component,
            mass: flight.donor(),
            age: flight.since_launch(),
            placement,
            attempt: 0,
            metallicity_age_years: None,
            lifetime: None,
        },
        DonorKind::CarbonOxygen => {
            let mass = donor_progenitor_mass(flight.donor());
            let z_ref = table.reference_composition(table.component_source(component));
            let star = Progenitor {
                mass,
                component,
                fate: fate(galaxy, mass, &z_ref, &draws_of(galaxy, id)),
            };
            let u = stream(galaxy, tags::DISPLACED_DEATH, id).uniform();
            let (age, life) = dead_age(galaxy, &star, (flight.since_launch(), None), u);
            DrawnMarks {
                component,
                mass,
                age,
                placement,
                attempt: 0,
                metallicity_age_years: None,
                lifetime: life,
            }
        }
    }
}

/// Whether `record` is a hypervelocity survivor whose donor is a helium white dwarf, the record's
/// primary of its own mass, which the stellar stage builds as one (plan 09, P09.T34.b;
/// `StarModel::helium_white_dwarf_under`): a survivor of a mass up to the heaviest helium white
/// dwarf, since a C/O donor's progenitor is of layer D's masses.
#[must_use]
pub(crate) fn is_helium_donor(record: &SystemRecord) -> bool {
    matches!(
        record.placement_class(),
        PlacementClass::Displaced {
            kind: DisplacedKind::HypervelocitySurvivor,
            ..
        }
    ) && DonorKind::of(record.primary_initial_mass()) == DonorKind::Helium
}

/// A hypervelocity survivor's flight (plan 09, P09.T34.b): its population, launch speed and time
/// since launch, as its marks drew them; `None` for every other record.
#[must_use]
pub(crate) fn survivor_flight(galaxy: &Galaxy, record: &SystemRecord) -> Option<SurvivorFlight> {
    let displaced = galaxy.fields().displaced()?;
    match record.placement_class() {
        PlacementClass::Displaced {
            kind: DisplacedKind::HypervelocitySurvivor,
            ..
        } => Some(
            displaced
                .class_table()
                .survivors()?
                .flight(galaxy.seed(), record.id()),
        ),
        PlacementClass::Alive
        | PlacementClass::Retained { .. }
        | PlacementClass::Displaced {
            kind: DisplacedKind::Remnant | DisplacedKind::Runaway | DisplacedKind::Walkaway,
            ..
        } => None,
    }
}

/// The speed bin a displaced remnant of class `class` in `band` was kicked in, drawn on
/// `displaced.origin_bin` from the class's odds: its own bin, but in the fastest class of an age
/// bin, which holds the unbound of every bin too (Design note 23).
fn origin_bin(
    galaxy: &Galaxy,
    table: &ClassTable,
    band: MassBand,
    class: DisplacedClassId,
    id: SystemId,
) -> SpeedBin {
    speed_bin(pick(
        stream(galaxy, tags::DISPLACED_ORIGIN_BIN, id).mark(),
        &table.marks(band, class).origin_odds(),
    ))
}

/// The time since death or ejection a member of the class `key` has: its age bin's, in years, for
/// a thin-disc class; any for an old source's, which has speed bins only.
fn class_ages(table: &ClassTable, key: ClassKey) -> (Years, Option<Years>) {
    match key {
        ClassKey::Thin { age, .. } => age_bin_years(age, table.scales().tau_unit()),
        ClassKey::Old { .. } | ClassKey::Hypervelocity => (Years::ZERO, None),
    }
}

/// The ages a living runaway or walkaway of `source` of mass `m` can have under its reference
/// composition, whose lifetime there is `life_ref`, with its time since ejection in `[lo, hi)`:
/// `[t + lo, min(t + hi, life_ref)]` for the ejection age `t` drawn on `runaway.ejection`, each
/// of the quadrature's ejection ages weighted by its share of the class at that mass, as the class
/// table weighted it (`ejected_in`). Where no ejection age leaves room before the star's death,
/// which the record's own draws can do where the median star's did not, the ages below its
/// lifetime are taken, from the bin's lower edge if it can, ejected at its formation. The ejection
/// age comes back with the span.
fn since_ejection(
    galaxy: &Galaxy,
    table: &ClassTable,
    (source, kind): (BirthSource, Ejected),
    (ages, m, life_ref): (&AgeDistribution, SolarMasses, Years),
    (lo, hi): (Years, Option<Years>),
    id: SystemId,
) -> (Years, Years, Years) {
    let life = life_ref.value();
    let hi = hi.map_or(f64::INFINITY, Years::value);
    let span = |t: f64| {
        let a = (t + lo.value()).min(life).max(0.0);
        let b = (t + hi).min(life).min(ages.max().value()).max(a);
        (a, b)
    };
    let ejections = table.ejection_ages(source, kind, m);
    let weights: Vec<f64> = ejections
        .iter()
        .map(|&(t, w)| {
            let t = t.value();
            if t >= life {
                return 0.0;
            }
            let (a, b) = span(t);
            w * (ages.cdf(Years::new(b)) - ages.cdf(Years::new(a))).max(0.0)
        })
        .collect();
    if weights.iter().any(|&w| w > 0.0) {
        let i = pick(stream(galaxy, tags::RUNAWAY_EJECTION, id).mark(), &weights);
        let (a, b) = span(ejections[i].0.value());
        (Years::new(a), Years::new(b), ejections[i].0)
    } else {
        let below = life.min(ages.max().value());
        (
            Years::new(lo.value().min(below).max(0.0)),
            Years::new(below),
            Years::ZERO,
        )
    }
}

/// A layer-D field record's marks, for a component of a source that ejects runaways: plan 03's
/// mass and age, with the runaway reduction (Design note 17).
fn runaway_reduction(
    galaxy: &Galaxy,
    table: &ClassTable,
    key: CellKey,
    id: SystemId,
    position: &GalacticPosition,
    c: ComponentId,
) -> DrawnMarks {
    let laws = galaxy.fields().component(c);
    let model = table.runaway_model();
    let source = table.component_source(c);
    let site = PointLy::from(position);
    let bracket = LifetimeBracket::shared();
    let mut masses = stream(galaxy, tags::SYSTEM_PRIMARY_MASS, id);
    let mut ages = stream(galaxy, tags::SYSTEM_AGE, id);
    let mut kinds = stream(galaxy, tags::DISPLACED_KIND, id);
    let mut attempt = 0_u8;
    loop {
        let mass = SolarMasses::new(
            galaxy
                .mass_function()
                .sample_in_band(key.band(), &mut masses),
        );
        let age = laws.ages().sample(&mut ages);
        let mark = kinds.mark();
        let kept = DrawnMarks {
            component: c,
            mass,
            age,
            placement: PlacementClass::Alive,
            attempt,
            metallicity_age_years: None,
            lifetime: None,
        };
        if attempt == MAX_MARK_ATTEMPT
            || !alive(galaxy, (id, attempt), &site, c, (mass, age), bracket)
        {
            return kept;
        }
        let ejected: f64 = [Ejected::Runaway, Ejected::Walkaway]
            .into_iter()
            .map(|kind| {
                let share = match kind {
                    Ejected::Runaway => model.runaway_share(mass),
                    Ejected::Walkaway => model.walkaway_share(mass),
                };
                share
                    * table
                        .ejection_ages(source, kind, mass)
                        .iter()
                        .filter(|&&(t, _)| t.value() < age.value())
                        .map(|&(_, w)| w)
                        .sum::<f64>()
            })
            .sum();
        if !mark.is_below(Threshold::from_probability(ejected.clamp(0.0, 1.0))) {
            return kept;
        }
        attempt += 1;
    }
}

/// Whether a layer-D star of `mass` and `age` at attempt `attempt` of record `id`, of component
/// `c` at `site`, is alive: by the lifetime bracket where it decides, and otherwise by its
/// lifetime at the metallicity the stellar stage draws for it and its draws at that attempt.
fn alive(
    galaxy: &Galaxy,
    (id, attempt): (SystemId, u8),
    site: &PointLy,
    c: ComponentId,
    (mass, age): (SolarMasses, Years),
    bracket: &LifetimeBracket,
) -> bool {
    let (least, greatest) = bracket
        .bracket(mass)
        .expect("a layer-D mass lies in band D, which the bracket covers");
    if age.value() < least.value() {
        return true;
    }
    if age.value() > greatest.value() {
        return false;
    }
    let read = Years::new(age.value().max(0.0));
    let composition = metallicity_of(galaxy, id, galaxy.fields().component(c), site, read);
    let draws = StarDraws::for_attempt(galaxy.seed(), BodyId::new(id, 0), u32::from(attempt));
    age.value() < lifetime(galaxy, mass, &composition, &draws).value()
}

/// Speed bin `i`, below 8.
fn speed_bin(i: usize) -> SpeedBin {
    SpeedBin::new(u8::try_from(i).expect("under eight bins")).expect("under eight bins")
}

/// [`SystemRecord::formation_site`]: the epoch position, or a displaced record's birth source's
/// reference site.
pub(super) fn formation_site(galaxy: &Galaxy, record: &SystemRecord) -> PointLy {
    match (record.placement_class(), galaxy.fields().displaced()) {
        (PlacementClass::Displaced { class, .. }, Some(displaced)) => {
            match displaced.class(class).key().source() {
                Some(source) => displaced.class_table().reference_site(source),
                None => PointLy::from(record.epoch_position()),
            }
        }
        (PlacementClass::Displaced { .. }, None)
        | (PlacementClass::Alive | PlacementClass::Retained { .. }, _) => {
            PointLy::from(record.epoch_position())
        }
    }
}

/// [`SystemRecord::progenitor_composition`]: a retained record's or a displaced remnant's birth
/// source's reference composition, or a hypervelocity survivor's birth component's source's.
pub(super) fn progenitor_composition(
    galaxy: &Galaxy,
    record: &SystemRecord,
) -> Option<Composition> {
    let displaced = galaxy.fields().displaced()?;
    let table = displaced.class_table();
    let source = match record.placement_class() {
        PlacementClass::Retained { .. }
        | PlacementClass::Displaced {
            kind: DisplacedKind::HypervelocitySurvivor,
            ..
        } => table.component_source(record.component()?),
        PlacementClass::Displaced {
            class,
            kind: DisplacedKind::Remnant,
        } => displaced.class(class).key().source()?,
        PlacementClass::Alive
        | PlacementClass::Displaced {
            kind: DisplacedKind::Runaway | DisplacedKind::Walkaway,
            ..
        } => return None,
    };
    Some(table.reference_composition(source))
}

/// [`SystemRecord::kick_constraint`], with its vertical component where `vertical` says.
#[must_use]
pub(super) fn kick_constraint(
    galaxy: &Galaxy,
    record: &SystemRecord,
    vertical: Vertical,
) -> Option<KickConstraint> {
    let displaced = galaxy.fields().displaced()?;
    let table = displaced.class_table();
    match record.placement_class() {
        PlacementClass::Alive
        | PlacementClass::Displaced {
            kind:
                DisplacedKind::Runaway | DisplacedKind::Walkaway | DisplacedKind::HypervelocitySurvivor,
            ..
        } => None,
        PlacementClass::Retained { speed } => {
            let source = table.component_source(record.component()?);
            Some(KickConstraint::of_bin(
                speed,
                table.speed_scale(source),
                None,
            ))
        }
        PlacementClass::Displaced {
            class,
            kind: DisplacedKind::Remnant,
        } => {
            let key = displaced.class(class).key();
            let source = key.source()?;
            let origin = origin_bin(
                galaxy,
                table,
                MassBand::from(record.layer()),
                class,
                record.id(),
            );
            let vertical = (vertical == Vertical::Read && is_ballistic(key))
                .then(|| ballistic_vertical(galaxy, record));
            Some(KickConstraint::of_bin(
                origin,
                table.speed_scale(source),
                vertical,
            ))
        }
    }
}

/// Whether the class `key` is in its ballistic layer: a thin-disc class of the two youngest age
/// bins, under 0.3 `R_d ÷ v_c` since death (Design note 22).
#[must_use]
pub(crate) fn is_ballistic(key: ClassKey) -> bool {
    matches!(key, ClassKey::Thin { age, .. } if age.index() < 2)
}

/// A ballistic record's height above the midplane at the epoch and its time since its kick
/// (Design note 22): a remnant's since its death, [`ballistic_vertical`]'s; a runaway's or a
/// walkaway's since its ejection, the ejection its marks drew on `runaway.ejection` with the
/// time between it and the age it then drew, the age rescaled from its source's reference
/// composition to its own as its age was (Design note 19); `None` for a record of no ballistic
/// class.
pub(crate) fn ballistic_motion(
    galaxy: &Galaxy,
    record: &SystemRecord,
) -> Option<(LightYears, Years)> {
    let displaced = galaxy.fields().displaced()?;
    let PlacementClass::Displaced { class, kind } = record.placement_class() else {
        return None;
    };
    let key = displaced.class(class).key();
    if !is_ballistic(key) {
        return None;
    }
    let ejected = match kind {
        DisplacedKind::Remnant => return Some(ballistic_vertical(galaxy, record)),
        DisplacedKind::Runaway => Ejected::Runaway,
        DisplacedKind::Walkaway => Ejected::Walkaway,
        DisplacedKind::HypervelocitySurvivor => return None,
    };
    let table = displaced.class_table();
    let source = key.source()?;
    let id = record.id();
    let mass = record.primary_initial_mass();
    let life_ref = lifetime(
        galaxy,
        mass,
        &table.reference_composition(source),
        &draws_of(galaxy, id),
    );
    let ages = galaxy.fields().component(record.component()?).ages();
    let (a, b, t) = since_ejection(
        galaxy,
        table,
        (source, ejected),
        (ages, mass, life_ref),
        class_ages(table, key),
        id,
    );
    let age_ref = age_between(
        ages,
        a,
        b,
        stream(galaxy, tags::DISPLACED_DEATH, id).uniform(),
    );
    let rescale = if age_ref.value() > 0.0 {
        record.age_at_epoch().value() / age_ref.value()
    } else {
        1.0
    };
    let height = record.epoch_position().to_light_years_f64()[2];
    Some((
        LightYears::new(height),
        Years::new(((age_ref.value() - t.value()) * rescale).max(0.0)),
    ))
}

/// A ballistic remnant's height above the midplane at the epoch and its time since death: its
/// age at the epoch less its primary's lifetime at its own composition and draws.
fn ballistic_vertical(galaxy: &Galaxy, record: &SystemRecord) -> (LightYears, Years) {
    let composition = draw_metallicity(galaxy, record);
    let life = lifetime(
        galaxy,
        record.primary_initial_mass(),
        &composition,
        &primary_draws(galaxy, record),
    );
    let height = record.epoch_position().to_light_years_f64()[2];
    (
        LightYears::new(height),
        Years::new(record.age_at_epoch().value() - life.value()),
    )
}
