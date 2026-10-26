//! A small body's taxonomic class on the record (plan 14, P14.T51.e's wiring;
//! decision-p14-t51e-contract §1.4), held off the record until the 23 → 24 bump
//! ([`SMALL_BODY_TYPES_VERSION`]), a P14.T54.b.j rider with its own hold.
//!
//! The record's builders (`primary_record` for a planet or a belt's member, `satellite_record` for
//! a moon) fill it here from the body's **built** surface, after the surface is built and outside
//! `published_surface`, `SurfaceCase` and `with_held_members` (decision-p14-t51e-contract §2.2):
//!
//! - **the kind** ([`TaxonomyBodyKind`]): a planet; a member of a [`BeltKind::Kuiper`] belt, or a
//!   moon of one, a Kuiper-belt body; another belt's member; another moon;
//! - **the lid**: the crust's `CrustComposition::lid_lithology`, a registry row, as its
//!   [`CrustLithology`] by its key;
//! - **the veneer** ([`Veneer`]): `s_t`, the modifiers' organic ground times their organic cover,
//!   and `s_i`, the published condensates' solid, non-seasonal areas summed in their order, held to
//!   1, less `s_t`, held to +0, an upper bound, labelled;
//! - **the clocks** ([`WeatheringClocks`]): the figures' surface age `t_x`, the modifiers' wind term
//!   at an unshielded share of 1 (`w₀`), the reference wind term `w_ref` ([`reference_wind_term`]),
//!   the modifiers' maturity m, and the air: above T51.d's ion floor where the conditions' surface
//!   pressure over the bulk section's gravity exceeds `ION_STOPPING_COLUMN`, as the modifiers' own
//!   exposure reads it.
//!
//! A refusal of the pure rule's (`ClassifyTaxonomyError::NotGiven`: no crust where the rule reads a
//! lid, no modifiers where it reads the veneer or the clocks) is not modelled, never none
//! (decision-p14-t51e-contract §1.5).
//!
//! # The reference wind term
//!
//! `w_ref` is the same unshielded wind term on a stated reference body: a circular orbit at
//! `Q_TYPE_ANCHOR_DISTANCE`, 2.5 au, about T51.d's reference Sun (`ModifierHost::sun()`), with
//! no field and no air (main's ruling, 2026-10-26, on decision-p14-t51e-qs §1.2). It is derived by
//! `SurfaceModifiers::derive`, the rule every record's term comes from, once a query, and only by
//! a query that builds records (`snapshot_at`, `body_at` and `hooks_at`, each opting its epoch in
//! with `Epoch::with_small_bodies`, [`SmallBodyTypes::under`]); never once a body, never by a query
//! that builds none, nothing under the hold, and never cached: 0.15998 by the ruling's table.
//!
//! # The hold
//!
//! Below [`SMALL_BODY_TYPES_VERSION`] the rules in force are [`SmallBodyRules::Held`], under which
//! nothing of the class is computed and every record's member is not modelled, so that every
//! golden keeps its bits. Every reach of the record into the pure module and into the modifiers'
//! getters for it is in this file, behind the hold (the source scans' lists:
//! `small_bodies_nothing_on_the_record_reads_the_types_yet` and
//! `modifiers_nothing_on_the_record_reads_the_modifiers_yet`). P14.T54.b.l deletes the hold with
//! its tripwire (`the_small_body_types_are_held_only_until_the_24_bump`).

use crate::planetary::chem::partition::{CondensatePhase, Condensates};
use crate::planetary::derive::atmosphere::PartialPressures;
use crate::planetary::hooks::crust::CrustLithology;
use crate::planetary::hooks::figures::MagneticField;
use crate::planetary::hooks::modifiers::{
    HeliocentricOrbit, ION_STOPPING_COLUMN, ModifierHost, ModifierInputs, Primary,
    SurfaceModifiers, WaterHistory,
};
use crate::planetary::hooks::small_bodies::{
    AirShield, BuildTaxonomyInputsError, ClassifyTaxonomyError, Q_TYPE_ANCHOR_DISTANCE,
    TaxonomicClass, TaxonomyBodyKind, TaxonomyInputs, Veneer, WeatheringClocks, WindTerms,
};
use crate::planetary::placement::OrbitHost;
use crate::planetary::record::{BeltKind, BulkProperties, Modelled, Section, Surface};
use crate::units::{
    Kelvin, Kilograms, KilogramsPerSquareMetre, Metres, MetresPerSecondSquared, Pascals,
    WattsPerSquareMetre, Years,
};
use crate::{GENERATOR_VERSION, GeneratorVersion};

use super::{Body, Part, PlanetarySystem};

/// The generator version from which a small body's taxonomic class is on the record: 24, the
/// 23 → 24 batch's bump, as a P14.T54.b.j rider with its own hold (decision-p14-t51e-contract §4).
///
/// Below it [`SmallBodyRules::in_force`] is [`SmallBodyRules::Held`], so that every record's class
/// is not modelled, as at 23, and every golden keeps its bits. The bump, P14.T54.b.l, deletes this
/// hold, [`SmallBodyRules`] with its `Held` arm, [`SmallBodyTypes::Held`], the switch
/// [`with_small_body_rules`] with its arm in `planetary::testing::with_every_24_hold`,
/// `RecordRules::small_bodies`, the source scan
/// `small_bodies_nothing_on_the_record_reads_the_types_yet` with its `HELD` list and its hold check,
/// this file's `HELD` lines in `modifiers_nothing_on_the_record_reads_the_modifiers_yet`, and its
/// tripwire (`the_small_body_types_are_held_only_until_the_24_bump`). If the rider misses
/// P14.T54.b.i's commit, the hold renumbers to 25.
pub(crate) const SMALL_BODY_TYPES_VERSION: GeneratorVersion = GeneratorVersion::new(24);

/// Whether the record's surface carries its taxonomic class (the hold,
/// [`SMALL_BODY_TYPES_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum SmallBodyRules {
    /// Versions 23's and before: the class is not modelled, and nothing of it is computed.
    Held,
    /// The 23 → 24 batch's: every planet's, member's and moon's surface carries its class
    /// ([`record_taxonomic_class`]).
    Batch,
}

impl SmallBodyRules {
    /// The rules this generator version gives: the batch's from [`SMALL_BODY_TYPES_VERSION`], 23's
    /// below it; in the tests, the rules a test's switch sets on its thread
    /// ([`with_small_body_rules`]).
    #[must_use]
    pub(crate) fn in_force() -> Self {
        #[cfg(test)]
        if let Some(rules) = SMALL_BODY_RULES.with(core::cell::Cell::get) {
            return rules;
        }
        if GENERATOR_VERSION >= SMALL_BODY_TYPES_VERSION {
            Self::Batch
        } else {
            Self::Held
        }
    }
}

#[cfg(test)]
thread_local! {
    /// The small bodies' rules a test sets on its thread, `None` for the version's.
    static SMALL_BODY_RULES: core::cell::Cell<Option<SmallBodyRules>> =
        const { core::cell::Cell::new(None) };
}

/// `run`'s result with the small bodies' rules `rules` on this thread, for the tests to reach 24's
/// record path ahead of the bump (every 24 hold in force together,
/// `planetary::testing::with_every_24_hold`); the version's rules are set back after, a panic in
/// `run` included. A system reads them as it is generated (`RecordRules::in_force`). P14.T54.b.l
/// deletes it with the hold.
#[cfg(test)]
pub(crate) fn with_small_body_rules<R>(rules: SmallBodyRules, run: impl FnOnce() -> R) -> R {
    /// Sets the thread's rules back as it drops.
    struct Restore(Option<SmallBodyRules>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SMALL_BODY_RULES.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(SMALL_BODY_RULES.with(|cell| cell.replace(Some(rules))));
    run()
}

/// What one query's records' classes share, its epoch's: the rules, and under
/// [`SmallBodyRules::Batch`] the reference wind term, derived once for the query. An epoch holds
/// [`Held`](Self::Held) until a query that builds records opts it in (`Epoch::with_small_bodies`),
/// so that a record built on an epoch that did not opt in is not modelled, never wrong.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum SmallBodyTypes {
    /// [`SmallBodyRules::Held`]: nothing of the class is computed.
    Held,
    /// [`SmallBodyRules::Batch`], with `w_ref` ([`reference_wind_term`]).
    Batch {
        /// The reference wind term `w_ref`, dimensionless, finite and positive.
        reference_wind_term: f64,
    },
}

impl SmallBodyTypes {
    /// What a query under the rules `rules` shares: under [`SmallBodyRules::Batch`] the
    /// reference wind term, derived here, once for the query; nothing under
    /// [`SmallBodyRules::Held`].
    #[must_use]
    pub(super) fn under(rules: SmallBodyRules) -> Self {
        match rules {
            SmallBodyRules::Held => Self::Held,
            SmallBodyRules::Batch => Self::Batch {
                reference_wind_term: reference_wind_term(),
            },
        }
    }
}

/// The reference wind term `w_ref`, dimensionless: T51.d's wind term at an unshielded share of 1
/// ([`SurfaceModifiers::unshielded_wind_term`]) of the reference body ([`reference_inputs`]), a
/// circular orbit at `Q_TYPE_ANCHOR_DISTANCE`, 2.5 au, about `ModifierHost::sun()`: 0.15998, the
/// Moon's eccentricity cancelling (decision-p14-t51e-qs §1.2 and its table).
///
/// # Panics
///
/// Never: the reference body's stated inputs are each in their range, as this module's tests hold.
#[must_use]
fn reference_wind_term() -> f64 {
    let none = Condensates::default();
    SurfaceModifiers::derive(&reference_inputs(&none))
        .expect("the reference body's stated inputs are each in their range")
        .unshielded_wind_term()
}

/// The modifiers' inputs of the reference body whose wind term is `w_ref`, with the condensates
/// `none` (empty): `ModifierHost::sun()`, a circular orbit at `Q_TYPE_ANCHOR_DISTANCE`, about the
/// star, with no field and no air (main's ruling, 2026-10-26).
///
/// The rest are stated valid values the wind term does not read, each unread by it: a gravity of
/// 1 m s⁻², a surface age of 0, no resurfacing, no heat flow, a surface at 100 K, no mantle
/// sulphur, an ordinary chondrite's lid, no gases and no water.
#[must_use]
fn reference_inputs(none: &Condensates) -> ModifierInputs<'_> {
    ModifierInputs {
        host: ModifierHost::sun(),
        orbit: HeliocentricOrbit {
            semi_major_axis: Metres::from(Q_TYPE_ANCHOR_DISTANCE),
            eccentricity: 0.0,
        },
        primary: Primary::Star,
        gravity: MetresPerSecondSquared::new(1.0),
        surface_pressure: Pascals::ZERO,
        field: MagneticField::None,
        surface_age: Years::ZERO,
        resurfacing_time: Years::new(f64::INFINITY),
        heat_flow: WattsPerSquareMetre::ZERO,
        surface_temperature: Kelvin::new(100.0),
        mantle_sulphur: Kilograms::ZERO,
        condensates: none,
        lid: CrustLithology::ChondriteL.substance(),
        gases: PartialPressures::default(),
        water: WaterHistory::NeverWet,
    }
}

/// The record's surface section `surface` of the body `body` of `system`, as its builder built it,
/// with its taxonomic class ([`record_taxonomic_class`], through
/// [`Surface::with_taxonomic_class`]), whose bulk section is `bulk` (`None` where it is not
/// modelled), under what its query shares `types`: the record builders' one call, after the
/// surface is built (decision-p14-t51e-contract §1.4). A section that is not `Ok` is unchanged.
#[must_use]
pub(super) fn with_class(
    types: SmallBodyTypes,
    (system, body): (&PlanetarySystem, &Body),
    bulk: Option<&BulkProperties>,
    surface: Section<Surface>,
) -> Section<Surface> {
    surface.map(|surface| {
        let gravity = bulk.map(BulkProperties::surface_gravity);
        let class = record_taxonomic_class(types, (system, body), gravity, &surface);
        surface.with_taxonomic_class(class)
    })
}

/// The taxonomic class of the record's body `body` of `system`, whose built surface is `surface`
/// and whose bulk section's surface gravity is `gravity` (`None` where its bulk section is not
/// modelled), under what its query shares `types` (the [module](self) documentation).
///
/// Under [`SmallBodyTypes::Held`] it is not modelled, and nothing of it is computed. Under
/// [`SmallBodyTypes::Batch`] it is `TaxonomicClass::of` over the surface's members, not
/// modelled where the body's rule reads a member the record does not give, never none.
#[must_use]
fn record_taxonomic_class(
    types: SmallBodyTypes,
    (system, body): (&PlanetarySystem, &Body),
    gravity: Option<MetresPerSecondSquared>,
    surface: &Surface,
) -> Modelled<Option<TaxonomicClass>> {
    match types {
        SmallBodyTypes::Held => Modelled::NotModelled,
        SmallBodyTypes::Batch {
            reference_wind_term,
        } => taxonomy_kind(system, body).map_or(Modelled::NotModelled, |kind| {
            surface_class(kind, reference_wind_term, gravity, surface)
        }),
    }
}

/// The kind of `body` of `system` as its class reads it: a planet; a member of a
/// [`BeltKind::Kuiper`] belt, or a moon of one, a Kuiper-belt body; another member; another moon.
/// `None` for a ring, which has no surface.
///
/// # Panics
///
/// Never: a member's belt and a moon's parent are of its system.
#[must_use]
fn taxonomy_kind(system: &PlanetarySystem, body: &Body) -> Option<TaxonomyBodyKind> {
    let kuiper = |belt| match system
        .belt(belt)
        .expect("a member's belt is a belt of its system")
        .kind()
    {
        BeltKind::Kuiper => true,
        BeltKind::Asteroid => false,
    };
    Some(match &body.part {
        Part::Planet(_) => TaxonomyBodyKind::Planet,
        Part::Member(member) if kuiper(member.belt) => TaxonomyBodyKind::KuiperBody,
        Part::Member(_) => TaxonomyBodyKind::AsteroidBeltMember,
        Part::Moon(_) => {
            let parent = match body.host {
                OrbitHost::Body(parent) => system.body(parent),
                OrbitHost::Star(_) | OrbitHost::Pair(_) | OrbitHost::Barycentre => None,
            }
            .expect("a moon's parent is a body of its system");
            match &parent.part {
                Part::Member(member) if kuiper(member.belt) => TaxonomyBodyKind::KuiperBody,
                Part::Planet(_) | Part::Member(_) | Part::Moon(_) | Part::Ring(_) => {
                    TaxonomyBodyKind::OtherMoon
                }
            }
        }
        Part::Ring(_) => return None,
    })
}

/// The class of a body of kind `kind` whose built surface is `surface` and whose gravity is
/// `gravity`, against the reference wind term `reference_wind_term`: not modelled where its rule
/// reads an input the surface does not give.
#[must_use]
fn surface_class(
    kind: TaxonomyBodyKind,
    reference_wind_term: f64,
    gravity: Option<MetresPerSecondSquared>,
    surface: &Surface,
) -> Modelled<Option<TaxonomicClass>> {
    let lid = match surface.crust() {
        Modelled::Ok(crust) => {
            CrustLithology::of_key(crust.lid_lithology().substance().key().as_str())
        }
        Modelled::NotModelled => None,
    };
    let modifiers = match surface.modifiers() {
        Modelled::Ok(modifiers) => Some(modifiers),
        Modelled::NotModelled => None,
    };
    // An input out of its range is one validated members never give; it is taken as not given,
    // so that a rule that reads it is not modelled, and one that does not is unaffected.
    let veneer = modifiers.and_then(|modifiers| veneer_of(modifiers, surface.condensates()).ok());
    let clocks = modifiers.zip(gravity).and_then(|(modifiers, gravity)| {
        let wind = WindTerms::new(modifiers.unshielded_wind_term(), reference_wind_term).ok()?;
        let air = air_shield(surface.conditions().surface_pressure(), gravity);
        WeatheringClocks::new(
            surface.figures().surface_age(),
            wind,
            modifiers.maturity(),
            air,
        )
        .ok()
    });
    match TaxonomicClass::of(&TaxonomyInputs::new(kind, lid, veneer, clocks)) {
        Ok(class) => Modelled::Ok(class),
        Err(ClassifyTaxonomyError::NotGiven(_)) => Modelled::NotModelled,
    }
}

/// The veneer of a surface whose modifiers are `modifiers` and whose published condensates are
/// `condensates`: `s_t`, the organic ground times the organic cover, and `s_i`, the solid,
/// non-seasonal entries' areas summed in their order, held to 1, less `s_t`, held to +0, an upper
/// bound, labelled, since the entries' areas may overlap (decision-p14-t51e-contract §1.2).
///
/// # Errors
///
/// As [`Veneer::new`]: a share that is not a number is kept, not held, so that it is refused
/// there, and a rule that reads the veneer is not modelled.
fn veneer_of(
    modifiers: &SurfaceModifiers,
    condensates: &Condensates,
) -> Result<Veneer, BuildTaxonomyInputsError> {
    let tholin = modifiers.organic_ground() * modifiers.organic_cover();
    let ice = condensates
        .entries()
        .iter()
        .filter(|entry| entry.phase() == CondensatePhase::Solid && !entry.is_seasonal())
        .fold(0.0, |sum, entry| sum + entry.area_fraction());
    let ice = if ice > 1.0 { 1.0 } else { ice };
    let uncovered = ice - tholin;
    // Held to +0 where it is at most 0, a negative zero included; a NaN reaches the veneer's check.
    let uncovered = if uncovered <= 0.0 { 0.0 } else { uncovered };
    Veneer::new(tholin, uncovered)
}

/// Whether the air of a surface at the pressure `pressure` under the gravity `gravity` stops the
/// wind above the ground: above T51.d's ion floor where its column p ÷ g exceeds
/// `ION_STOPPING_COLUMN`, the comparison the modifiers' own exposure makes, so that a column that
/// is not a number is above it too.
#[must_use]
fn air_shield(pressure: Pascals, gravity: MetresPerSecondSquared) -> AirShield {
    let column = KilogramsPerSquareMetre::new(pressure.value() / gravity.value());
    if column <= ION_STOPPING_COLUMN {
        AirShield::Open
    } else {
        AirShield::AboveIonFloor
    }
}

#[cfg(test)]
mod tests;
