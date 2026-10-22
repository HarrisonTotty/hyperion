//! The palette's resolution from plan 14's record (P14.T54.a): the entries
//! [`CoarseInputs::for_body`](super::CoarseInputs::for_body) gives a generated body, each resolved
//! from the registry's row of a substance its record's surface section names
//! (decision-composition §1.7; decision-r11-palette-liquids §1.7; decision-p14-t49b-condensed §2;
//! decision-p14-t49c-split §4).
//!
//! # The entries
//!
//! - **The crust's** (P14.T51.c): its primary crust as [`PaletteRole::PrimaryCrust`], its
//!   secondary as [`PaletteRole::SecondaryCrust`], its tertiary as [`PaletteRole::TertiaryCrust`],
//!   an undifferentiated body's bulk material as [`PaletteRole::Bulk`], and its provinces as
//!   [`PaletteRole::Province`] where they are none of those. A secondary crust that is the
//!   provinces' lithology but not the lid takes the provinces' role alone, as the reference
//!   Moon's maria do, and a lid's secondary crust that is theirs its own role alone, as the
//!   reference Mars's basalt does; the inputs' crust mapping finds either
//!   ([`CoarseInputs::crust_palette`](super::CoarseInputs::crust_palette)).
//! - **The ices**: an [`PaletteRole::Ice`] entry for each solid condensate on the record
//!   (P14.T51.a) and each ice the figures put on the ground.
//! - **The liquids**: a [`PaletteRole::Liquid`] entry, with its record, for each liquid condensate
//!   and each liquid the figures put on the ground.
//!
//! One palette holds one entry for each (role, key), so seas of one key at two compositions
//! would share one entry: a recorded limitation (decision-r11-palette-liquids §1.7), unreached
//! while P14.T51.a's entries are one a substance and phase. The record names no deposit and no
//! cover until P14.T51.d (P14.T54.b), so a generated palette holds none; nor a cold-trapped
//! [`PaletteRole::Ice`] entry on an airless body, which no condensate on the record names until
//! P14.T51.f, wired by P14.T54.b (the census, P14.T33.c, lists cold-trapped `H2O` and `CO2` as
//! unreached until then; science-composition-crusts.md §6).
//!
//! # A solid's entry
//!
//! [`resolve_palette_entry`]: P14.T49.c's and P14.T49.i's normal albedo in B, V and R at the row's
//! reference form, never scaled here (R10.T10.a's `field_albedo_scale` rescales the rendered mean),
//! its family's photometric law's code as its phase row, 1 or 2 (no palette source writes row 3,
//! Lambert's, which the form chooses: decision-r11-palette-liquids §2 item 6), its grain density
//! where it lies, its transition and its mechanics family; its `FeO_T` not modelled until P14.T54.b
//! (decision-p14-t51d-weathering §1.6). A row whose optics are a stand-in resolves the same way;
//! the registry labels it (`CondensedOptics::is_stand_in`), and the bless note lists it.
//!
//! - **An ice** lies at the lower of its coldest zone's annual-mean temperature (the climate's
//!   cold profile's coldest point, a locked world's night) and its frost point at its partial
//!   pressure in the air, held at most its first melt (science-composition-crusts.md §3.2,
//!   item 3).
//! - **A crust** lies at the record's mean surface temperature, held at most its first melt: a
//!   rock's grain density is one value at every temperature, and an ice shell's is ice Ih's there.
//!
//! A row whose solid has no grain density where it lies is not resolved: solid argon's, whose
//! density the registry has not read (P14.T49.b's `ARGON` diagram), on the coldest airless
//! members, and one phase of solid oxygen's. `for_body` answers such a body not modelled, a
//! recorded gap.
//!
//! # A liquid's record
//!
//! P14.T49.h's [`liquid_properties`] (decision-r11-palette-liquids §1.7; decision-p14-t49b-condensed
//! §2), at:
//!
//! - **the temperature:** the record's mean surface temperature, since P14.T51.a's partition gives
//!   the record no liquid's own area-mean temperature yet, clamped into the liquid's range at the
//!   surface pressure: from its first melt to its boiling point there on P14.T49.b's liquid
//!   saturation branch, and, where its row states no branch (a solution's) or the branch gives
//!   none, to a part in a million below the range's end, its critical point;
//! - **the composition:** its row's reference composition, since P14.T51.a's entry carries none
//!   yet, basis `Derived` for a solution; between a solution's first melt and its reference
//!   composition's liquidus the record is that composition's supercooled liquid, a stated
//!   stand-in, basis `Derived` ([`liquid_properties`]' own rule).
//!
//! Its entry's grain density is the record's density, P14.T49.h's at the record's temperature and
//! composition, which for a pure fluid is the row's saturated liquid's there; its normal albedo
//! the record's Fresnel reflectance ([`LiquidProperties::fresnel_normal_albedo_bvr`],
//! decision-r11-palette-liquids §2 item 7), a specular surface's; its phase row 0, no particulate
//! law; its transition and mechanics its row's condensed columns' (its frozen form's family). No
//! member is scaled by R10's albedo scale (§2 item 6), and a member without a source is absent,
//! never 0.
//!
//! # A melt's record
//!
//! A crust entry the body melts carries its melt's record (decision-r11-palette-liquids §1.3): the
//! lid's where the crust gives a melt area, and any crust entry's whose transition lies below the
//! climate's warmest temperature, but an ice's, whose liquid is its condensate's own entry
//! (decision-pre-23-review item 10). Its temperature is the record's where it gives one, a world
//! locked 1:1 to its host's dayside mean where that lies above the solidus; and otherwise this
//! rule, basis at least `Derived`: the mean surface temperature where it lies above the solidus (a
//! magma ocean's whole surface), else the warmest of the climate's range. Either way it is held
//! at or above the solidus, a melt's range having no upper end; P14.T51.c states no eruption
//! temperature. A row with no melt model (graphite's) carries none: the entry is drawn solid, a
//! recorded limitation.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use hyperion_surface::field::{
    BuildPaletteError, LiquidBasis, LiquidProperties, MaterialPalette, MechanicsFamily,
    PaletteEntry, PaletteRole,
};
use hyperion_surface::material::UNASSIGNED_PHASE_ROW;
use hyperion_surface::substance_key::SubstanceKey;

use super::inputs::LidCrust;
use crate::planetary::chem::partition::{CondensateEntry, CondensatePhase};
use crate::planetary::hooks::crust::CrustComposition;
use crate::planetary::hooks::surface::{ColdProfile, SurfaceConditions};
use crate::planetary::record::Surface;
use crate::substance::{
    Composition, ResolveLiquidError, ResolvePaletteEntryError, SubstanceId, liquid_properties,
    resolve_palette_entry,
};
use crate::units::{Kelvin, Pascals};

/// How far below its critical point a liquid's record is held, relative: a part in a million, as
/// P14.T51.b's area mix holds it (`photometry::area_mix`), inside the range.
const BELOW_CRITICAL: f64 = 1e-6;

/// Why [`resolve`] could not give a body's palette: a substance its record names whose row the
/// registry does not resolve, or entries no palette holds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ResolveBodyPaletteError {
    /// A solid's entry: its row has no condensed columns or optics, or no grain density where it
    /// lies.
    Solid {
        /// The substance.
        substance: SubstanceKey,
        /// The role its entry would take.
        role: PaletteRole,
        /// Why.
        cause: ResolvePaletteEntryError,
    },
    /// A liquid's entry: its row has no condensed columns, its frozen form's.
    LiquidNotCondensed(SubstanceKey),
    /// A liquid's record: a member it requires is not modelled, or no temperature in its range
    /// gives one.
    Liquid {
        /// The substance.
        substance: SubstanceKey,
        /// Why.
        cause: ResolveLiquidError,
    },
    /// The entries break a rule of the palette: more than it holds, or a value out of its range.
    Palette(BuildPaletteError),
}

impl fmt::Display for ResolveBodyPaletteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Solid {
                substance,
                role,
                cause,
            } => write!(
                f,
                "the {role:?} entry of {substance} is not resolved: {cause}"
            ),
            Self::LiquidNotCondensed(substance) => {
                write!(f, "the liquid {substance} has no condensed columns")
            }
            Self::Liquid { substance, cause } => {
                write!(
                    f,
                    "the liquid {substance}'s record is not resolved: {cause}"
                )
            }
            Self::Palette(error) => write!(f, "the resolved entries are no palette: {error}"),
        }
    }
}

impl Error for ResolveBodyPaletteError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Solid { cause, .. } => Some(cause),
            Self::Liquid { cause, .. } => Some(cause),
            Self::Palette(error) => Some(error),
            Self::LiquidNotCondensed(_) => None,
        }
    }
}

/// The palette of the body whose record's surface section is `surface` and whose crust, on it, is
/// `crust` (the [module](self)'s rules).
///
/// # Errors
///
/// [`ResolveBodyPaletteError`] for the first entry, in (role, key) order of the crust's, then the
/// ices', then the liquids', that the registry does not resolve, and for entries no palette holds.
pub(crate) fn resolve(
    surface: &Surface,
    crust: &CrustComposition,
) -> Result<MaterialPalette, ResolveBodyPaletteError> {
    let conditions = surface.conditions();
    let mut entries: BTreeMap<(PaletteRole, SubstanceKey), PaletteEntry> = BTreeMap::new();
    let lid = match crust.lid() {
        LidCrust::Primary => crust.primary(),
        LidCrust::Secondary => crust.secondary(),
        LidCrust::Bulk => crust.bulk(),
    };
    let melt_area = crust.melt_area_fraction();
    for (role, id) in crust_roles(crust) {
        let mut entry = solid(
            id,
            role,
            held_at_first_melt(id, mean_temperature(conditions)),
        )?;
        let melting = (lid == Some(id) && melt_area > 0.0)
            || entry.transition.value() < conditions.climate_range().1.value();
        let ice = matches!(
            entry.mechanics,
            MechanicsFamily::WaterIce | MechanicsFamily::VolatileIce
        );
        if melting && !ice {
            entry.liquid = melt_record(id, entry.transition, conditions);
        }
        entries.insert((role, entry.substance), entry);
    }
    let figures = surface.figures();
    let on_the_ground = |shares: &[(SubstanceId, f64)]| {
        shares
            .iter()
            .filter(|&&(_, area_fraction)| area_fraction > 0.0)
            .map(|&(id, _)| id)
            .collect::<Vec<_>>()
    };
    let condensates = surface.condensates().entries();
    let in_phase = |phase: CondensatePhase| {
        condensates
            .iter()
            .filter(move |entry| entry.phase() == phase)
            .map(CondensateEntry::substance)
    };
    let coldest = coldest_zone(conditions);
    let ices = in_phase(CondensatePhase::Solid).chain(on_the_ground(figures.surface_ices()));
    for id in ices {
        let key = id.substance().key();
        if entries.contains_key(&(PaletteRole::Ice, key)) {
            continue;
        }
        let lies_at = held_at_first_melt(id, ice_temperature(id, coldest, conditions));
        entries.insert(
            (PaletteRole::Ice, key),
            solid(id, PaletteRole::Ice, lies_at)?,
        );
    }
    let liquids = in_phase(CondensatePhase::Liquid).chain(on_the_ground(figures.surface_liquids()));
    for id in liquids {
        let key = id.substance().key();
        if entries.contains_key(&(PaletteRole::Liquid, key)) {
            continue;
        }
        entries.insert((PaletteRole::Liquid, key), liquid(id, conditions)?);
    }
    MaterialPalette::new(entries.into_values().collect()).map_err(ResolveBodyPaletteError::Palette)
}

/// The crust's lithologies with the roles their entries take (the [module](self)'s "The
/// entries"), each once: the primary, the secondary, the tertiary, the bulk material and the
/// provinces where they are none of the first, second and fourth.
#[must_use]
fn crust_roles(crust: &CrustComposition) -> Vec<(PaletteRole, SubstanceId)> {
    let provinces = crust.provinces();
    let mut roles = Vec::with_capacity(5);
    if let Some(primary) = crust.primary() {
        roles.push((PaletteRole::PrimaryCrust, primary));
    }
    if let Some(secondary) = crust.secondary() {
        let role = if secondary == provinces && crust.lid() != LidCrust::Secondary {
            PaletteRole::Province
        } else {
            PaletteRole::SecondaryCrust
        };
        roles.push((role, secondary));
    }
    if let Some(tertiary) = crust.tertiary() {
        roles.push((PaletteRole::TertiaryCrust, tertiary));
    }
    if let Some(bulk) = crust.bulk() {
        roles.push((PaletteRole::Bulk, bulk));
    }
    if ![crust.primary(), crust.secondary(), crust.bulk()].contains(&Some(provinces)) {
        roles.push((PaletteRole::Province, provinces));
    }
    roles
}

/// The record's mean surface temperature, K, where it is finite and positive; otherwise, on an
/// unlit body whose record states 0 K, the warmest of its climate's range, which its night floor
/// holds above zero.
#[must_use]
fn mean_temperature(conditions: &SurfaceConditions) -> Kelvin {
    let mean = conditions.mean_surface_temperature();
    if mean.value().is_finite() && mean.value() > 0.0 {
        mean
    } else {
        conditions.climate_range().1
    }
}

/// `temperature` held at most the first melt of `id`'s row, its condensed columns' transition,
/// where its solid's grain density ends; `temperature` itself for a row with none.
#[must_use]
fn held_at_first_melt(id: SubstanceId, temperature: Kelvin) -> Kelvin {
    match id.substance().condensed() {
        Some(condensed) if temperature > condensed.transition().temperature() => {
            condensed.transition().temperature()
        }
        Some(_) | None => temperature,
    }
}

/// The entry of the solid `id` in `role` where it lies at `temperature` ([`resolve_palette_entry`]).
fn solid(
    id: SubstanceId,
    role: PaletteRole,
    temperature: Kelvin,
) -> Result<PaletteEntry, ResolveBodyPaletteError> {
    resolve_palette_entry(id, role, temperature).map_err(|cause| ResolveBodyPaletteError::Solid {
        substance: id.substance().key(),
        role,
        cause,
    })
}

/// The annual-mean temperature of the coldest zone of the climate `conditions` describes, K: the
/// coldest point of its cold profile (`SurfaceConditions::cold_area`'s), a rotating world's zonal
/// profile's colder end and a locked world's night; the mean surface temperature where the profile
/// gives no finite, positive one.
#[must_use]
fn coldest_zone(conditions: &SurfaceConditions) -> Kelvin {
    let coldest = match conditions.cold_profile() {
        ColdProfile::Eyeball { night_k, .. } => night_k,
        ColdProfile::Zonal(points) => points.iter().map(|&(_, t)| t).fold(f64::INFINITY, f64::min),
    };
    if coldest.is_finite() && coldest > 0.0 {
        Kelvin::new(coldest)
    } else {
        mean_temperature(conditions)
    }
}

/// Where the ice `id` lies under the climate `conditions`, whose coldest zone is at `coldest`: the
/// lower of `coldest` and its frost point at its partial pressure in the air (its mole fraction of
/// the surface pressure, on its row's phase diagram); `coldest` where it is no gas of the air or
/// its row has no frost point there.
#[must_use]
fn ice_temperature(id: SubstanceId, coldest: Kelvin, conditions: &SurfaceConditions) -> Kelvin {
    let partial = conditions
        .gases()
        .iter()
        .find(|&&(gas, _)| gas == id)
        .map(|&(_, x)| Pascals::new(x * conditions.surface_pressure().value()));
    let frost = partial.and_then(|p| {
        id.substance()
            .phase_diagram()
            .and_then(|diagram| diagram.saturation_temperature(p))
    });
    match frost {
        Some(frost) if frost < coldest => frost,
        Some(_) | None => coldest,
    }
}

/// The [`PaletteRole::Liquid`] entry of `id` under the climate `conditions`, with its record (the
/// [module](self)'s "A liquid's record").
fn liquid(
    id: SubstanceId,
    conditions: &SurfaceConditions,
) -> Result<PaletteEntry, ResolveBodyPaletteError> {
    let row = id.substance();
    let key = row.key();
    let condensed = row
        .condensed()
        .ok_or(ResolveBodyPaletteError::LiquidNotCondensed(key))?;
    let record =
        liquid_record(id, conditions).map_err(|cause| ResolveBodyPaletteError::Liquid {
            substance: key,
            cause,
        })?;
    Ok(PaletteEntry {
        substance: key,
        role: PaletteRole::Liquid,
        normal_albedo_bvr: record.fresnel_normal_albedo_bvr(),
        phase_row: UNASSIGNED_PHASE_ROW,
        grain_density_kg_m3: record.density_kg_m3,
        transition: condensed.transition().temperature(),
        mechanics: condensed.mechanics(),
        liquid: Some(record),
        // Never on a liquid (decision-p14-t51d-weathering §1.6).
        feo_mass_fraction: None,
    })
}

/// The record of the liquid `id` at its reference composition under the climate `conditions`: at
/// the record's mean surface temperature held at most its boiling point at the surface pressure,
/// and, outside its range, at the range's bound the temperature lies beyond (the [module](self)'s
/// "A liquid's record").
///
/// # Errors
///
/// [`liquid_properties`]' refusal: the row is no liquid, a member its record needs is not modelled
/// (there or at the bound), or the range has no bound that holds the temperature.
fn liquid_record(
    id: SubstanceId,
    conditions: &SurfaceConditions,
) -> Result<LiquidProperties, ResolveLiquidError> {
    let mean = mean_temperature(conditions);
    let boiling = id
        .substance()
        .phase_diagram()
        .and_then(|diagram| diagram.saturation_temperature(conditions.surface_pressure()));
    let temperature = match boiling {
        Some(boiling) if mean > boiling => boiling,
        Some(_) | None => mean,
    };
    in_range(id, temperature)
}

/// The record of the liquid `id` at its reference composition at `temperature`, or, outside its
/// range, at the bound `temperature` lies beyond: its first melt below it, a part in a million
/// below its critical point above it.
///
/// # Errors
///
/// [`liquid_properties`]' refusal, there or at the bound.
fn in_range(id: SubstanceId, temperature: Kelvin) -> Result<LiquidProperties, ResolveLiquidError> {
    match liquid_properties(id, temperature, Composition::Reference) {
        Err(ResolveLiquidError::OutOfRange { lowest, below, .. }) => {
            let held = match below {
                Some(critical) if temperature >= critical => {
                    Kelvin::new(critical.value() * (1.0 - BELOW_CRITICAL))
                }
                // Below the first melt, or a temperature that is not a number.
                Some(_) | None => lowest,
            };
            liquid_properties(id, held, Composition::Reference)
        }
        answer => answer,
    }
}

/// The record of the melt of the crust row `id`, whose solidus is `solidus`, under the climate
/// `conditions` (the [module](self)'s "A melt's record"); `None` for a row with no melt model, or
/// a member its record needs not modelled.
#[must_use]
fn melt_record(
    id: SubstanceId,
    solidus: Kelvin,
    conditions: &SurfaceConditions,
) -> Option<LiquidProperties> {
    let (temperature, ruled) = match conditions.cold_profile() {
        ColdProfile::Eyeball { day_k, .. } if day_k > solidus.value() => {
            (Kelvin::new(day_k), false)
        }
        ColdProfile::Eyeball { .. } | ColdProfile::Zonal(_) => {
            let mean = mean_temperature(conditions);
            if mean > solidus {
                (mean, true)
            } else {
                (conditions.climate_range().1, true)
            }
        }
    };
    let temperature = if temperature > solidus {
        temperature
    } else {
        solidus
    };
    let mut record = in_range(id, temperature).ok()?;
    if ruled {
        record.basis = record.basis.max(LiquidBasis::Derived);
    }
    Some(record)
}
