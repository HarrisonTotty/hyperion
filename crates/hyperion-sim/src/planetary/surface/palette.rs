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
//! - **The modifiers'** (P14.T51.d with T51.j, wired by P14.T54.b.c), where the record's surface
//!   models them: a [`PaletteRole::Deposit`] entry for each weathering deposit, an oxidised
//!   surface's `ferric`, the source of its sand and dust (decision-p14-t51-batch §5), and a
//!   [`PaletteRole::Cover`] entry `tholin` where the organic cover is above 0, the organic family's
//!   cover R10.T1.c's blend reads (decision-r10-t1c-maturity §3.2). An Io's volcanic deposits, SO₂
//!   frost and sulphur, take no entry here: their area on the ground is the area mix's to place
//!   (P14.T54.b.e), recorded.
//!
//! One palette holds one entry for each (role, key), so seas of one key at two compositions
//! would share one entry: a recorded limitation (decision-r11-palette-liquids §1.7), unreached
//! while P14.T51.a's entries are one a substance and phase. Below generator version 24 the record
//! holds its modifiers off it (`AIR_SECTIONS_VERSION`), so a generated palette holds no deposit and
//! no cover; nor a cold-trapped [`PaletteRole::Ice`] entry on an airless body, which no condensate
//! on the record names until P14.T51.f, wired by P14.T54.b (the census, P14.T33.c, lists
//! cold-trapped `H2O` and `CO2` as unreached until then; science-composition-crusts.md §6).
//!
//! # A solid's entry
//!
//! [`resolve_palette_entry`]: P14.T49.c's and P14.T49.i's normal albedo in B, V and R at the row's
//! reference form, never scaled here (R10.T10.a's `field_albedo_scale` rescales the rendered mean),
//! its family's photometric law's code as its phase row, 1 or 2 (no palette source writes row 3,
//! Lambert's, which the form chooses: decision-r11-palette-liquids §2 item 6), its grain density
//! where it lies, its transition and its mechanics family. A row whose optics are a stand-in
//! resolves the same way; the registry labels it (`CondensedOptics::is_stand_in`), and the bless
//! note lists it.
//!
//! Its `FeO_T` (decision-p14-t51d-weathering §1.2 and §1.6) is the registry's
//! (`palette_feo_mass_fraction`), where the record's modifiers are modelled, which R10.T1.d's
//! space-weathering law reads beside the header's maturity: the row's iron as `FeO` of its
//! formula or its melt's oxides, +0 for an ice, an iron-free solid and a metal, and not modelled
//! where the registry carries no composition (the chondrites, `phyllosilicate`, `cometary`,
//! `ca_al_residue`) or the row's albedo is an observed, weathered one (`mars_dust`). Where they are
//! not modelled, below generator version 24 among them, no entry carries one, so that the law
//! weathers nothing and every palette keeps its bytes.
//!
//! - **An ice** lies at the lower of its coldest zone's annual-mean temperature (the climate's
//!   cold profile's coldest point, a locked world's night) and its frost point at its partial
//!   pressure in the air, held at most its first melt (science-composition-crusts.md §3.2,
//!   item 3).
//! - **A crust** lies at the record's mean surface temperature, held at most its first melt: a
//!   rock's grain density is one value at every temperature, and an ice shell's is ice Ih's there.
//!   So do a deposit and a cover.
//!
//! The record's mean surface temperature is finite and positive wherever `for_body` resolves a
//! palette: it refuses an unlit body, whose record states 0 K, first. [`resolve`] itself reads
//! such a record's warmest in its place, for its tests.
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
//!   stand-in, basis `Derived` ([`liquid_properties`]' own rule);
//! - **the pressure:** the record's surface pressure, to which a pure liquid with a reference
//!   equation of state is compressed from generator version 24 (P14.T49.n's hold,
//!   `COMPRESSED_LIQUIDS_VERSION`; `Substance::compressed_liquid_density`'s law), so that its
//!   density, its index, its scattering and its viscosity read the density the partition's layer
//!   and area read at the same state (decision-p14-t54a-water-consistency's W4); until then the
//!   saturated liquid's. A solution and a melt are incompressible under either.
//!
//! Its entry's grain density is the record's density, P14.T49.h's at the record's temperature and
//! composition, which for a pure fluid is the row's saturated liquid's there until generator
//! version 24, and from it that liquid compressed to the surface pressure; its normal albedo
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
//!
//! [`liquid_properties`]: crate::substance::liquid_properties

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
use crate::planetary::record::{Modelled, Surface, SurfaceModifiers};
use crate::substance::optics::{held_at_first_melt, palette_feo_mass_fraction};
use crate::substance::{
    self, Composition, ResolveLiquidError, ResolvePaletteEntryError, SubstanceId,
    liquid_properties_at_pressure, resolve_palette_entry,
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
/// ices', then the liquids', then the modifiers' deposits and cover, that the registry does not
/// resolve, and for entries no palette holds.
///
/// # Panics
///
/// Never: the registry holds `tholin`.
pub(crate) fn resolve(
    surface: &Surface,
    crust: &CrustComposition,
) -> Result<MaterialPalette, ResolveBodyPaletteError> {
    /// The substances of `shares` that lie on the ground, with an area above zero.
    fn on_the_ground(shares: &[(SubstanceId, f64)]) -> impl Iterator<Item = SubstanceId> + '_ {
        shares
            .iter()
            .filter(|&&(_, area_fraction)| area_fraction > 0.0)
            .map(|&(id, _)| id)
    }
    let conditions = surface.conditions();
    let modifiers = surface.modifiers();
    let mut entries: BTreeMap<(PaletteRole, SubstanceKey), PaletteEntry> = BTreeMap::new();
    let lid = crust.lid_lithology();
    let melt_area = crust.melt_area_fraction();
    for (role, id) in crust_roles(crust) {
        let mut entry = solid(
            id,
            role,
            held_at_first_melt(id, mean_temperature(conditions)),
            modifiers,
        )?;
        let melting = (lid == id && melt_area > 0.0)
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
            solid(id, PaletteRole::Ice, lies_at, modifiers)?,
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
    if let Modelled::Ok(weathered) = modifiers {
        for (role, id) in modifier_entries(weathered) {
            let key = id.substance().key();
            if entries.contains_key(&(role, key)) {
                continue;
            }
            let lies_at = held_at_first_melt(id, mean_temperature(conditions));
            entries.insert((role, key), solid(id, role, lies_at, modifiers)?);
        }
    }
    MaterialPalette::new(entries.into_values().collect()).map_err(ResolveBodyPaletteError::Palette)
}

/// The entries the modifiers `modifiers` add to a palette, with their roles (the [module](self)'s
/// "The entries"): a [`PaletteRole::Deposit`] entry for each weathering deposit, an oxidised
/// surface's `ferric`, in their order, and the `tholin` [`PaletteRole::Cover`] where the organic
/// cover is above 0.
///
/// # Panics
///
/// Never: the registry holds `tholin`.
#[must_use]
fn modifier_entries(modifiers: &SurfaceModifiers) -> Vec<(PaletteRole, SubstanceId)> {
    let mut entries: Vec<(PaletteRole, SubstanceId)> = modifiers
        .weathering_deposits()
        .iter()
        .map(|deposit| (PaletteRole::Deposit, deposit.substance()))
        .collect();
    if modifiers.organic_cover() > 0.0 {
        let tholin = substance::by_key("tholin").expect("tholin is a registry row");
        entries.push((PaletteRole::Cover, tholin));
    }
    entries
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
/// holds above zero. `for_body` refuses such a body before it resolves the palette (a recorded
/// gap), so only a direct call, a test's, reads the fallback.
#[must_use]
fn mean_temperature(conditions: &SurfaceConditions) -> Kelvin {
    let mean = conditions.mean_surface_temperature();
    if mean.value().is_finite() && mean.value() > 0.0 {
        mean
    } else {
        conditions.climate_range().1
    }
}

/// The entry of the solid `id` in `role` where it lies at `temperature` ([`resolve_palette_entry`]),
/// on a body whose record's modifiers are `modifiers`: with the registry's `FeO_T`
/// ([`palette_feo_mass_fraction`]) where they are modelled, and with none where they are not (the
/// [module](self)'s "A solid's entry").
fn solid(
    id: SubstanceId,
    role: PaletteRole,
    temperature: Kelvin,
    modifiers: Modelled<&SurfaceModifiers>,
) -> Result<PaletteEntry, ResolveBodyPaletteError> {
    let mut entry = resolve_palette_entry(id, role, temperature).map_err(|cause| {
        ResolveBodyPaletteError::Solid {
            substance: id.substance().key(),
            role,
            cause,
        }
    })?;
    entry.feo_mass_fraction = match modifiers {
        Modelled::Ok(_) => palette_feo_mass_fraction(id),
        Modelled::NotModelled => None,
    };
    Ok(entry)
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
/// and, outside its range, at the range's bound the temperature lies beyond, under the surface
/// pressure (the [module](self)'s "A liquid's record").
///
/// # Errors
///
/// [`liquid_properties`]' refusal: the row is no liquid, a member its record needs is not modelled
/// (there or at the bound), or the range has no bound that holds the temperature.
///
/// [`liquid_properties`]: crate::substance::liquid_properties
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
    in_range(id, temperature, conditions.surface_pressure())
}

/// The record of the liquid `id` at its reference composition at `temperature`, or, outside its
/// range, at the bound `temperature` lies beyond: its first melt below it, a part in a million
/// below its critical point above it; under `pressure`, the body's surface pressure, to which a
/// pure liquid is compressed from generator version 24 (`liquid_properties_at_pressure`,
/// P14.T49.n's hold; its saturated record, [`liquid_properties`]', until then).
///
/// # Errors
///
/// [`liquid_properties`]' refusal, there or at the bound.
///
/// [`liquid_properties`]: crate::substance::liquid_properties
fn in_range(
    id: SubstanceId,
    temperature: Kelvin,
    pressure: Pascals,
) -> Result<LiquidProperties, ResolveLiquidError> {
    match liquid_properties_at_pressure(id, temperature, Composition::Reference, pressure) {
        Err(ResolveLiquidError::OutOfRange { lowest, below, .. }) => {
            let held = match below {
                Some(critical) if temperature >= critical => {
                    Kelvin::new(critical.value() * (1.0 - BELOW_CRITICAL))
                }
                // Below the first melt, or a temperature that is not a number.
                Some(_) | None => lowest,
            };
            liquid_properties_at_pressure(id, held, Composition::Reference, pressure)
        }
        answer => answer,
    }
}

/// The record of the melt of the crust row `id`, whose solidus is `solidus`, under the climate
/// `conditions` (the [module](self)'s "A melt's record"); `None` for a row with no melt model, or
/// a member its record needs not modelled.
///
/// # Panics
///
/// Never: the record is asked at the row's reference composition, at or above its solidus, the
/// first melt of its range, and [`in_range`] holds a temperature at or above a critical point
/// inside the range.
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
    let mut record = match in_range(id, temperature, conditions.surface_pressure()) {
        Ok(record) => record,
        // A row with no melt model (graphite's), or a member its record needs not modelled: the
        // entry is drawn solid, a recorded limitation (the module's "A melt's record").
        Err(ResolveLiquidError::NotALiquid { .. } | ResolveLiquidError::NotModelled { .. }) => {
            return None;
        }
        Err(
            refused @ (ResolveLiquidError::OutOfRange { .. }
            | ResolveLiquidError::CompositionNotModelled { .. }
            | ResolveLiquidError::ComponentNotModelled { .. }
            | ResolveLiquidError::CompositionOutOfRange { .. }),
        ) => unreachable!(
            "a melt's record is asked at its reference composition, at or above its solidus, the \
             first melt of its range, a temperature at a critical point held inside it: {refused}"
        ),
    };
    if ruled {
        record.basis = record.basis.max(LiquidBasis::Derived);
    }
    Some(record)
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::planetary::hooks::crust::tests::{mars, moon, table_conditions};
    use crate::substance::{
        CompressedLiquidsRules, CondensedPhase, by_key, liquid_properties, with_compressed_liquids,
    };

    /// The registry's row of `key`, which a test knows to be one.
    fn row(key: &str) -> SubstanceId {
        by_key(key).unwrap()
    }

    /// A liquid whose record's temperature lies above its boiling point at the surface pressure is
    /// held at it, on its liquid saturation branch: water under the table's Venus, its mean surface
    /// temperature above water's boiling point at its surface pressure, which lies below water's
    /// critical pressure.
    #[test]
    fn a_liquid_above_its_boiling_point_is_held_at_it() {
        let venus = table_conditions("Venus");
        let water = row("H2O");
        let boiling = water
            .substance()
            .phase_diagram()
            .and_then(|diagram| diagram.saturation_temperature(venus.surface_pressure()))
            .expect("water boils under Venus's air, below its critical pressure");
        assert!(
            venus.mean_surface_temperature() > boiling,
            "{:?} against {boiling:?}",
            venus.mean_surface_temperature()
        );
        let record = liquid_record(water, &venus).unwrap();
        assert_same_bits(record.temperature.value(), boiling.value());
        eprintln!(
            "water under Venus's {:.4e} Pa, {:.2} K: held at its boiling point, {:.3} K, {:.1} kg m⁻³",
            venus.surface_pressure().value(),
            venus.mean_surface_temperature().value(),
            boiling.value(),
            record.density_kg_m3
        );
    }

    /// P14.T49.n's compression, held until 24 (`COMPRESSED_LIQUIDS_VERSION`): under 23's rules a
    /// pure liquid's entry is its saturated liquid's at the record's temperature, to the bit; under
    /// 24's its grain density is the registry's law at the record's temperature and surface
    /// pressure to the bit, the density the partition's layer and area read at the same state
    /// (W4), and its record is the record at that pressure, its index, scattering and viscosity at
    /// that density: water under the table's Earth, about 3.5 × 10⁻⁵ denser under its 0.77 bar
    /// (`κ_T` Δp, as Earth's sea's 4.6 × 10⁻⁵ at one atmosphere). Water held at its
    /// boiling point under the table's Venus lies at its vapour pressure, so it is compressed by
    /// no more than rounding, and a solution's entry (Titan's seas under the table's Earth, held
    /// below their critical point) is unchanged.
    #[test]
    fn a_liquid_s_entry_reads_its_record_compressed_to_the_surface_pressure() {
        let compressed = |id: SubstanceId, conditions: &SurfaceConditions| {
            with_compressed_liquids(CompressedLiquidsRules::Compressed, || {
                liquid(id, conditions)
            })
            .unwrap()
        };
        let water = row("H2O");
        let earth = table_conditions("Earth");
        let held = liquid(water, &earth).unwrap();
        let record = held.liquid.as_ref().unwrap();
        let t = record.temperature;
        let saturated = water
            .substance()
            .grain_density(CondensedPhase::Liquid, t)
            .unwrap();
        assert_same_bits(held.grain_density_kg_m3, saturated.value());
        assert_eq!(
            record,
            &liquid_properties(water, t, Composition::Reference).unwrap()
        );
        let sea = compressed(water, &earth);
        let law = water
            .substance()
            .compressed_liquid_density(t, earth.surface_pressure())
            .unwrap();
        assert_same_bits(sea.grain_density_kg_m3, law.value());
        let sea_record = sea.liquid.as_ref().unwrap();
        assert_same_bits(sea_record.density_kg_m3, law.value());
        assert_same_bits(sea_record.temperature.value(), t.value());
        let at_pressure = with_compressed_liquids(CompressedLiquidsRules::Compressed, || {
            liquid_properties_at_pressure(
                water,
                t,
                Composition::Reference,
                earth.surface_pressure(),
            )
        })
        .unwrap();
        assert_eq!(sea_record, &at_pressure);
        assert_eq!(
            sea.normal_albedo_bvr,
            at_pressure.fresnel_normal_albedo_bvr()
        );
        let relative = sea.grain_density_kg_m3 / held.grain_density_kg_m3 - 1.0;
        assert!(
            relative > 3e-5 && relative < 6e-5,
            "{relative:e} under {:?} at {t:?}",
            earth.surface_pressure()
        );
        assert!(sea_record.refractive_index[6] > record.refractive_index[6]);
        eprintln!(
            "water under the table's Earth, {:.2} K and {:.0} Pa: ρ {:.6} → {:.6} kg m⁻³ \
             ({relative:.3e}), n(545 nm) {:.8} → {:.8}",
            t.value(),
            earth.surface_pressure().value(),
            held.grain_density_kg_m3,
            sea.grain_density_kg_m3,
            record.refractive_index[6],
            sea_record.refractive_index[6],
        );
        let venus = table_conditions("Venus");
        let boiling = liquid(water, &venus).unwrap();
        let boiling_compressed = compressed(water, &venus);
        let moved = boiling_compressed.grain_density_kg_m3 / boiling.grain_density_kg_m3 - 1.0;
        assert!(moved.abs() < 1e-12, "{moved:e}");
        let seas = row("ch4_c2h6_n2");
        assert_eq!(compressed(seas, &earth), liquid(seas, &earth).unwrap());
    }

    /// A liquid's record outside its range is held inside it: a part in a million below its
    /// critical point at or above it (methane at 250 K and at its critical point), there too where
    /// its row states no saturation branch to hold it at its boiling point first (Titan's
    /// solution under the table's Earth, at or above the lowest critical point of its components,
    /// nitrogen's); and at its first melt below it (water at 200 K).
    #[test]
    fn a_liquid_beyond_its_range_is_held_inside_it() {
        let methane = row("CH4");
        let Err(ResolveLiquidError::OutOfRange {
            below: Some(critical),
            ..
        }) = liquid_properties(methane, Kelvin::new(250.0), Composition::Reference)
        else {
            panic!("methane at 250 K lies above its critical point");
        };
        let held = critical.value() * (1.0 - BELOW_CRITICAL);
        for temperature in [Kelvin::new(250.0), critical] {
            let record = in_range(methane, temperature, Pascals::ZERO).unwrap();
            assert_same_bits(record.temperature.value(), held);
        }
        let earth = table_conditions("Earth");
        let seas = row("ch4_c2h6_n2");
        assert!(
            seas.substance().phase_diagram().is_none(),
            "a solution's row states no saturation branch"
        );
        let Err(ResolveLiquidError::OutOfRange {
            below: Some(lowest_critical),
            ..
        }) = liquid_properties(
            seas,
            earth.mean_surface_temperature(),
            Composition::Reference,
        )
        else {
            panic!("Titan's seas at Earth's temperature lie above nitrogen's critical point");
        };
        let record = liquid_record(seas, &earth).unwrap();
        assert_same_bits(
            record.temperature.value(),
            lowest_critical.value() * (1.0 - BELOW_CRITICAL),
        );
        let water = row("H2O");
        let Err(ResolveLiquidError::OutOfRange { lowest, .. }) =
            liquid_properties(water, Kelvin::new(200.0), Composition::Reference)
        else {
            panic!("water at 200 K lies below its first melt");
        };
        let record = in_range(water, Kelvin::new(200.0), Pascals::ZERO).unwrap();
        assert_same_bits(record.temperature.value(), lowest.value());
    }

    /// An ice lies at the lower of its coldest zone's temperature and its frost point at its
    /// partial pressure in the air: carbon dioxide under the table's Mars at its frost point where
    /// the coldest zone is warmer, and at the coldest zone's where it is colder; an ice that is no
    /// gas of the air lies at its coldest zone's.
    #[test]
    fn an_ice_lies_at_its_frost_point_or_its_coldest_zone() {
        let mars = table_conditions("Mars");
        let co2 = row("CO2");
        let &(_, x) = mars
            .gases()
            .iter()
            .find(|&&(gas, _)| gas == co2)
            .expect("Mars's air holds carbon dioxide");
        let partial = Pascals::new(x * mars.surface_pressure().value());
        let frost = co2
            .substance()
            .phase_diagram()
            .and_then(|diagram| diagram.saturation_temperature(partial))
            .expect("carbon dioxide has a frost point under Mars's air");
        // On its sublimation branch, below its triple point's 216.59 K (Span and Wagner 1996), at
        // the table's partial pressure of carbon dioxide, far below the triple point's 518 kPa.
        assert!(
            frost.value() > 0.0 && frost.value() < 216.59,
            "{frost:?} at {partial:?}"
        );
        let warm = Kelvin::new(frost.value() + 50.0);
        assert_same_bits(ice_temperature(co2, warm, &mars).value(), frost.value());
        let cold = Kelvin::new(frost.value() - 30.0);
        assert_same_bits(ice_temperature(co2, cold, &mars).value(), cold.value());
        let absent = ["NH3", "SO2", "CH4"]
            .map(row)
            .into_iter()
            .find(|&id| mars.gases().iter().all(|&(gas, _)| gas != id))
            .expect("one of three is no gas of Mars's air");
        assert_same_bits(ice_temperature(absent, warm, &mars).value(), warm.value());
    }

    /// A secondary crust that is the provinces' lithology takes the provinces' role alone where it
    /// is not the lid, as the Moon's maria do, and its own role alone where it is, as Mars's basalt
    /// does: one entry either way, and no second entry for the provinces.
    #[test]
    fn a_secondary_crust_of_the_provinces_takes_their_role_off_the_lid() {
        let moon_surface = table_conditions("Moon");
        let moon = CrustComposition::derive(&moon(&moon_surface)).unwrap();
        assert_eq!(moon.lid(), LidCrust::Primary);
        assert_eq!(moon.secondary(), Some(moon.provinces()));
        assert_eq!(
            crust_roles(&moon),
            [
                (PaletteRole::PrimaryCrust, moon.primary().unwrap()),
                (PaletteRole::Province, moon.provinces()),
            ]
        );
        let mars_surface = table_conditions("Mars");
        let mars = CrustComposition::derive(&mars(&mars_surface)).unwrap();
        assert_eq!(mars.lid(), LidCrust::Secondary);
        assert_eq!(mars.secondary(), Some(mars.provinces()));
        assert_eq!(
            crust_roles(&mars),
            [
                (PaletteRole::PrimaryCrust, mars.primary().unwrap()),
                (PaletteRole::SecondaryCrust, mars.provinces()),
            ]
        );
    }
}
