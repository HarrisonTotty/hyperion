//! The classifier: the authoritative material at a point, a form of one of the body's substances,
//! decided from the same inputs as the height (plan R10, T1.b; Design note 1).
//!
//! [`material_at`] reads the finest level's height and gradient ([`Synthesiser::height_at`]), the
//! coarse cell's record and climate, and the header's palette, and takes the first of an ordered
//! list of rules whose condition holds. Nothing is drawn from a stream, so the classifier opens no
//! domain tag; it keeps no cache of its own, and the answer is the same in any order of queries.
//!
//! [`material_at`]'s documentation gives the rules, their order and the regimes they read.

use std::collections::BTreeMap;

use hyperion_base::math;
use hyperion_base::units::consts::SECONDS_PER_JULIAN_YEAR;
use hyperion_base::units::{Kelvin, Pascals};

use super::{CLIFF_SLOPE, Material, MaterialClass, SNOW_SHED_SLOPE};
use crate::cube::{Edge, FaceUv, PatchKey, face_uv_to_xyz, unit_dir};
use crate::field::{
    ClimateCell, Crust, FieldHeader, FieldView, FlowDirection, LogArea, MaterialPalette,
    MechanicsFamily, PaletteRole, SynthesisCell, month_share,
};
use crate::geometry::finest_level;
use crate::height::HeightSample;
use crate::num;
use crate::patch::normals::surface_normal;
use crate::synth::channels::NetworkId;
use crate::synth::{BandLevel, QueryHeightError, SynthCache, Synthesiser, interp};

/// The share of a cell its ice must cover to be massive ice, a glacier or an ice cap, rather than
/// a thin condensate: a half.
///
/// The field carries an ice share a cell and no thickness, so the share is what tells a cap from a
/// frost (R10.T1.a's record). A half is a choice, not a measured figure: a cell mostly under ice is
/// taken as part of a cap, and one mostly bare as holding patches of frost.
pub const MASSIVE_ICE_SHARE: f64 = 0.5;

/// The surface pressure below which an atmosphere is thin: 30 kPa, the sim's threshold below which
/// the gas cannot hide the ground (`planetary::derive::photometry::THIN_ATMOSPHERE_PRESSURE`,
/// decision-phase-curves item 4), which splits a Mars from an Earth.
///
/// The classifier reads it for `Dust`, which settles on an arid thin-aired world, as on Mars
/// (Design note 1). It is a photometric threshold reused, not an aeolian one: no source read
/// splits dust from sand by pressure. This crate cannot see the sim, so the figure is repeated
/// here; the two change together, which the sim's `surface_material_worlds` test checks.
pub const THIN_ATMOSPHERE_PRESSURE: Pascals = Pascals::new(30_000.0);

/// Köppen–Geiger's arid threshold's offset where neither half-year holds 70% of the precipitation,
/// millimetres a year (Peel, Finlayson and McMahon 2007, Table 1).
const KOPPEN_EVEN_OFFSET_MM: f64 = 14.0;

/// Köppen–Geiger's arid threshold's offset where the summer half-year holds 70% of the
/// precipitation, millimetres a year (Peel, Finlayson and McMahon 2007, Table 1).
const KOPPEN_SUMMER_OFFSET_MM: f64 = 28.0;

/// The share of the year's precipitation in one half-year at which Köppen–Geiger's arid threshold
/// takes that half's offset: 70% (Peel, Finlayson and McMahon 2007, Table 1).
const KOPPEN_SEASONAL_SHARE: f64 = 0.7;

/// The arid climates' bound on the precipitation, as a multiple of the threshold: "MAP <
/// 10 × `MAP_threshold`" (Peel, Finlayson and McMahon 2007, Table 1).
const KOPPEN_ARID_MULTIPLE: f64 = 10.0;

/// The kelvin of 0 °C, which Köppen–Geiger's temperatures are in.
const ZERO_CELSIUS_K: f64 = 273.15;

/// The authoritative material at `at`: the form the ground takes there, and the palette entry it is
/// made of where the body's palette names one ([`GroundMaterial`]).
///
/// It is the first of the rules below whose condition holds at the point, from the height and
/// gradient at the body's finest level ([`finest_level`], R05's, which the collision ground reads),
/// with the gradient's tangential part at the spheroid point for the slope ([`surface_normal`],
/// R09's "Deviations in T4, as built"), and from the coarse cell's record, its climate and the
/// header. `cache` is the caller's, and the answer does not depend on what it holds.
///
/// # The rules, in order
///
/// | Rule | Holds where | Material |
/// | --- | --- | --- |
/// | Sea ice | the point lies below the cell's water surface, in the cell's ice | `SeaIce` of the ice entry |
/// | Seabed | the point lies below the cell's water surface | `Seabed` of the floor's entry |
/// | Land ice | the point lies in the cell's ice | `Glacier`, `Frost` or, airless, `Regolith` of the ice entry |
/// | Melt | the point's annual mean temperature is above the crust entry's transition | `Melt` of the crust's entry |
/// | Snow | the point is frozen all year where it snows, no steeper than [`SNOW_SHED_SLOPE`] | `Snow` of the snow's entry |
/// | Cliff | the slope is above [`CLIFF_SLOPE`] | `Bedrock` of the crust's entry |
/// | Evaporite | an arid closed basin's floor, on a body whose palette has a salt | `Evaporite` of the salt's entry |
/// | Sediment | a closed basin's floor, or a channel's bed | `Sediment` of the crust's entry |
/// | Regolith | an airless body | `Regolith` of the crust's entry, else a `Deposit`'s, else a `Cover`'s |
/// | Organic sediment | an organic cover under an atmosphere where it is not arid | `OrganicSediment` of the cover's entry |
/// | Dust | an arid, thin-aired body | `Dust` of the loose ground's entry |
/// | Volcanic rock | a province's crust | `VolcanicRock` of the province's entry |
/// | Sand | arid ground | `Sand` of the loose ground's entry |
/// | Soil | everywhere else | `Soil` of the crust's entry |
///
/// The last holds everywhere, so the order is total. Where the palette does not name the deciding
/// rule's substance, as on a body whose composition is not modelled (an empty palette), the form
/// is still classed and the entry is `None`, which a console reads as the substance Missing
/// (`MATERIAL REGOLITH · —`; the guide, signed off in signoff-9 §17.5): never a stand-in entry.
/// Every substance is a palette entry (decision-composition §1.8): the forms of ice are the cell's
/// ice entry's, whatever its species; the seabed is the floor's own, the crust's, the bulk's or a
/// `Deposit`'s, never its liquid's (signoff-9); the loose ground is the crust's, a `Deposit`'s or a
/// `Cover`'s, a salt `Deposit` last, since it is an evaporite's source.
///
/// # Where Design note 1's order moves, and why
///
/// - **Sea ice comes before the seabed.** The ice of a cell under water lies on its liquid, over
///   the point below the water surface, so a seabed first would leave sea ice unreachable.
/// - **Melt and snow come before the cliff.** Melt above its transition holds no slope, and
///   [`SNOW_SHED_SLOPE`] (60°) is steeper than [`CLIFF_SLOPE`] (40°): after the cliff, snow would
///   end at 40° and its shed slope could never decide.
/// - **Organic sediment, dust and volcanic rock come before sand and soil**, which hold on every
///   arid and every other ground: after them the three could never hold.
///
/// # Glacier or frost, and gardened ice
///
/// The cell's ice share is massive where it is at least [`MASSIVE_ICE_SHARE`] of the cell, and a
/// thin condensate below it: by the ice field's share, as R10.T1.a's record has the rule owed, the
/// field carrying no thickness. Thin ice is `Frost`, on any body: a seasonal frost or a cold trap's
/// exposed ice. Massive ice is `Glacier` under an atmosphere (Earth's ice sheets, Mars's residual
/// caps, Pluto's nitrogen glacier), and `Regolith` of the ice entry on an airless body, whose
/// exposed ice impacts garden into a particulate regolith, lunar-like and not Lambertian, as
/// Ganymede's, Rhea's and Dione's (decision-r10-limb.md §2, "What T1.b owes"). A body is airless
/// where its header's surface pressure is zero, as the header defines one.
///
/// # Where in a cell its ice lies
///
/// Where a cell's share is whole, its ice is everywhere on it. Where it is partial, the ice lies on
/// the cell's shadowed ground, its hollows and pole-facing slopes, not evenly over it: a cold
/// trap's exposed ice in a crater's floor and under its pole-facing wall (R09's "Cold traps"
/// ruling), and a partial cap's in its cirques.
///
/// - **The index.** A point's index is z = t ÷ `σ_t` + d ÷ `σ_d`: t its pole-facing tilt, the fall of
///   the ground finer than the coarse cells towards the nearer pole along the surface, per metre;
///   d its depth below the coarse interpolant ([`interp::base_elevation`]); each over its standard
///   deviation over the cell. The two terms' equal weights are a choice, not a measured figure:
///   R09's ruling names both kinds of ground and weighs neither.
/// - **The cut.** The cell's cut is the 1 − s quantile of z over a 16 × 16 grid of the cell's
///   points, at the centres of its equal steps in face coordinates, by linear interpolation between
///   order statistics. A point is in the ice where its z is above the cut, so that the ice covers
///   the share s of the cell to the grid's sampling, whatever the relief's distribution. A cell's
///   smallest share, 1 ÷ 255, is above the grid's 1 ÷ 256. A Gaussian quantile would not do:
///   craters and steep walls give z heavy tails, and on a cratered airless pole it covered a sixth
///   of the share.
/// - **The degenerate cell.** A term whose deviation is zero is left out. With neither, the ice
///   covers the cell where the share is above a half. Sea ice is placed so too, since a sea's ice
///   does not follow its floor.
///
/// The grid is the cell's, the same for every point in it, so the caller's [`SynthCache`] keeps
/// each cell's scales and cut, and no answer depends on whether it holds them. A grid point reads
/// the cells within the synthesis's margin of the cell, as the point does, so a surveyed cell's
/// points answer wherever their height does.
///
/// # The regimes
///
/// - **Airless:** the header's surface pressure is zero.
/// - **Thin-aired:** below [`THIN_ATMOSPHERE_PRESSURE`], 30 kPa.
/// - **Arid:** the point's mean precipitation, in millimetres of water a Julian year, is zero, or
///   below Köppen–Geiger's threshold for the arid climates, ten times 2 T, 2 T + 14 or 2 T + 28 by
///   season (T the point's annual mean in °C; Peel, Finlayson and McMahon 2007, Hydrol. Earth Syst.
///   Sci. 11, 1633, Table 1). The summer is the year's six warmest months: Peel's is the warmer of
///   October to March and April to September, which a generated calendar does not have. The
///   threshold is water's and Earth's: it is negative below −7 °C (−14 °C where 70% falls in
///   summer, 0 °C where it falls in winter), so colder ground is arid only where nothing falls.
/// - **A closed basin** is a dry cell whose water has nowhere to go and into which a neighbour's
///   flows, the coarse pass's terminal outlet of an inland basin: a flow direction of
///   [`FlowDirection::Terminal`], a drainage area, and an edge neighbour whose flow names it. The
///   whole cell is the basin's floor.
/// - **An organic cover** is a palette's `Cover` entry of the organic family: a Titan-like haze's
///   deposit.
///
/// The temperatures are the climate record's, at the point's height by the header's lapse rate:
/// the annual mean for melt, and the warmest month's for snow. R09's `SurfaceClass` is not read:
/// until R09.T15 assigns its codes every cell is unclassified.
///
/// # Errors
///
/// [`QueryHeightError::NotSurveyed`] naming the first cell read that the view does not hold.
///
/// # Panics
///
/// If `at` names no direction (a (u, v) that is not finite), or a height is not finite (a bug).
pub fn material_at<F: FieldView>(
    synth: &Synthesiser<'_, F>,
    cache: &mut SynthCache,
    at: FaceUv,
) -> Result<GroundMaterial, QueryHeightError> {
    let facts = Facts::at(synth, cache, unit_dir(face_uv_to_xyz(at)))?;
    Ok(facts.decide().1)
}

/// What the classifier decides at a point ([`material_at`]): the form the ground takes, always, and
/// the palette entry it is made of, where the palette names one.
///
/// The form is classed even where the substance is not computed, so that only the substance is
/// Missing (signoff-9 §17.5). [`material`](Self::material) is the pair as T1.a's [`Material`],
/// which [`properties`](super::properties()) reads, where the entry is named.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GroundMaterial {
    class: MaterialClass,
    entry: Option<u8>,
}

impl GroundMaterial {
    /// The form `class` of the entry `entry`, for the tests.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn of(class: MaterialClass, entry: Option<u8>) -> Self {
        Self { class, entry }
    }

    /// The form.
    #[must_use]
    pub const fn class(self) -> MaterialClass {
        self.class
    }

    /// The palette entry's index, or `None` where the palette names no substance for the ground.
    #[must_use]
    pub const fn entry(self) -> Option<u8> {
        self.entry
    }

    /// The material, a form of a palette entry, or `None` where the entry is not named.
    #[must_use]
    pub fn material(self) -> Option<Material> {
        self.entry
            .and_then(|entry| Material::new(self.class, entry))
    }
}

/// The classifier's rules, in the order they are tried: the first whose condition holds decides
/// ([`material_at`]'s table).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum Rule {
    /// The cell's ice over its sea.
    SeaIce,
    /// The ground under the water surface.
    Seabed,
    /// The cell's ice on land.
    LandIce,
    /// Lithology above its transition.
    Melt,
    /// Snow lying all year, below the shed slope.
    Snow,
    /// Rock too steep for loose material.
    Cliff,
    /// An arid closed basin's salts.
    Evaporite,
    /// A closed basin's or a channel's sediment.
    Sediment,
    /// An airless body's regolith.
    Regolith,
    /// An organic cover's sediment where it is not arid.
    OrganicSediment,
    /// An arid thin-aired body's dust.
    Dust,
    /// A province's rock.
    VolcanicRock,
    /// Arid ground's sand.
    Sand,
    /// Every other ground's soil.
    Soil,
}

impl Rule {
    /// Every rule, in the order they are tried.
    pub(crate) const ALL: [Self; 14] = [
        Self::SeaIce,
        Self::Seabed,
        Self::LandIce,
        Self::Melt,
        Self::Snow,
        Self::Cliff,
        Self::Evaporite,
        Self::Sediment,
        Self::Regolith,
        Self::OrganicSediment,
        Self::Dust,
        Self::VolcanicRock,
        Self::Sand,
        Self::Soil,
    ];

    /// Whether the rule's condition holds for `facts`.
    #[must_use]
    pub(crate) fn holds(self, facts: &Facts) -> bool {
        match self {
            Self::SeaIce => facts.under_water && facts.ice.is_some(),
            Self::Seabed => facts.under_water,
            Self::LandIce => facts.ice.is_some(),
            Self::Melt => facts.molten,
            Self::Snow => facts.snow.is_some() && !facts.above_snow_shed,
            Self::Cliff => facts.above_cliff,
            Self::Evaporite => facts.closed_basin && facts.arid && facts.entries.salt.is_some(),
            Self::Sediment => facts.closed_basin || facts.channel_floor,
            Self::Regolith => facts.airless,
            Self::OrganicSediment => facts.entries.organic_cover.is_some() && !facts.arid,
            Self::Dust => facts.arid && facts.thin_aired,
            Self::VolcanicRock => facts.province,
            Self::Sand => facts.arid,
            Self::Soil => true,
        }
    }

    /// The material the rule gives for `facts`, where its condition holds: its form, and the entry
    /// it names where the palette has one.
    #[must_use]
    pub(crate) fn ground(self, facts: &Facts) -> GroundMaterial {
        let e = &facts.entries;
        let (class, entry) = match self {
            Self::SeaIce => (MaterialClass::SeaIce, facts.ice.and_then(|ice| ice.entry)),
            Self::Seabed => (MaterialClass::Seabed, e.floor),
            Self::LandIce => {
                let (massive, entry) = facts.ice.map_or((false, None), |i| (i.massive, i.entry));
                let class = match (massive, facts.airless) {
                    (false, _) => MaterialClass::Frost,
                    (true, false) => MaterialClass::Glacier,
                    (true, true) => MaterialClass::Regolith,
                };
                (class, entry)
            }
            Self::Melt => (MaterialClass::Melt, e.crust),
            Self::Snow => (MaterialClass::Snow, facts.snow),
            Self::Cliff => (MaterialClass::Bedrock, e.crust),
            Self::Evaporite => (MaterialClass::Evaporite, e.salt),
            Self::Sediment => (MaterialClass::Sediment, e.crust),
            Self::Regolith => (MaterialClass::Regolith, e.regolith),
            Self::OrganicSediment => (MaterialClass::OrganicSediment, e.organic_cover),
            Self::Dust => (MaterialClass::Dust, e.loose),
            Self::VolcanicRock => (MaterialClass::VolcanicRock, e.crust),
            Self::Sand => (MaterialClass::Sand, e.loose),
            Self::Soil => (MaterialClass::Soil, e.crust),
        };
        GroundMaterial { class, entry }
    }
}

/// The cell's ice where the point lies in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Ice {
    /// The cell's ice entry, which a valid field names wherever a cell has ice.
    entry: Option<u8>,
    /// Whether the cell's share is at least [`MASSIVE_ICE_SHARE`].
    massive: bool,
}

/// The palette entries each rule's substance can be, for one cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(crate) struct Entries {
    /// The cell's crust's entry (the header's `crust_palette`).
    crust: Option<u8>,
    /// The seabed's: the crust's, else the body's `Bulk` entry, else its first `Deposit`.
    floor: Option<u8>,
    /// The evaporite's: the first entry of the salt family that is a `Deposit`, else the first of
    /// that family in a crust's, a `Cover`'s or a `Deposit`'s role.
    salt: Option<u8>,
    /// The regolith's: the crust's, else the first `Deposit` not of the salt family, else the first
    /// `Cover`, else the first `Deposit`.
    regolith: Option<u8>,
    /// Sand's and dust's: the first `Deposit` not of the salt family (the role's "source of a
    /// body's sand and dust where it is not the crust's"), else the first `Cover`, else the
    /// crust's, else the first `Deposit`, a salt.
    loose: Option<u8>,
    /// The first `Cover` of the organic family.
    organic_cover: Option<u8>,
}

impl Entries {
    /// The entries of a cell of crust `crust` on a body of header `header`.
    #[must_use]
    fn of(header: &FieldHeader, crust: Crust) -> Self {
        let palette = header.palette();
        let crust = header.crust_palette()[usize::from(u8::from(crust))];
        let deposit = first(palette, |e| e.role == PaletteRole::Deposit);
        // A salt deposit is an evaporite's source, not loose ground's, where another deposit is.
        let loose_deposit = first(palette, |e| {
            e.role == PaletteRole::Deposit && e.mechanics != MechanicsFamily::Salt
        });
        let cover = first(palette, |e| e.role == PaletteRole::Cover);
        let bulk = first(palette, |e| e.role == PaletteRole::Bulk);
        let salt = first(palette, |e| {
            e.mechanics == MechanicsFamily::Salt && e.role == PaletteRole::Deposit
        })
        .or_else(|| {
            first(palette, |e| {
                e.mechanics == MechanicsFamily::Salt
                    && (e.role.is_crust()
                        || e.role == PaletteRole::Cover
                        || e.role == PaletteRole::Deposit)
            })
        });
        Self {
            crust,
            floor: crust.or(bulk).or(deposit),
            salt,
            regolith: crust.or(loose_deposit).or(cover).or(deposit),
            loose: loose_deposit.or(cover).or(crust).or(deposit),
            organic_cover: first(palette, |e| {
                e.role == PaletteRole::Cover && e.mechanics == MechanicsFamily::Organic
            }),
        }
    }
}

/// The index of the first entry of `palette` for which `which` holds.
#[must_use]
fn first(
    palette: &MaterialPalette,
    which: impl Fn(&crate::field::PaletteEntry) -> bool,
) -> Option<u8> {
    (0_u8..)
        .zip(palette.entries())
        .find_map(|(k, e)| which(e).then_some(k))
}

/// What the classifier knows at a point: each rule's condition and the entries their substances
/// can be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is one rule's condition, read apart by its rule; a state machine or enum \
              would hide which conditions the order of rules tests"
)]
pub(crate) struct Facts {
    /// The point lies below its cell's water surface.
    under_water: bool,
    /// The cell's ice, where the point lies in it.
    ice: Option<Ice>,
    /// The body is airless: its surface pressure is zero.
    airless: bool,
    /// The body has an atmosphere thinner than [`THIN_ATMOSPHERE_PRESSURE`].
    thin_aired: bool,
    /// The point's annual mean temperature is above its crust's entry's transition.
    molten: bool,
    /// The snow's entry, where the point's warmest month is below its transition and the cell
    /// precipitates.
    snow: Option<u8>,
    /// The slope is above [`CLIFF_SLOPE`].
    above_cliff: bool,
    /// The slope is above [`SNOW_SHED_SLOPE`].
    above_snow_shed: bool,
    /// The cell is a closed basin's floor.
    closed_basin: bool,
    /// The point is arid.
    arid: bool,
    /// The point lies on a channel's bed.
    channel_floor: bool,
    /// The cell's crust is a province's.
    province: bool,
    /// The entries the rules name.
    entries: Entries,
}

impl Facts {
    /// The rule that decides `self`, and its material.
    #[must_use]
    pub(crate) fn decide(&self) -> (Rule, GroundMaterial) {
        let rule = Rule::ALL
            .into_iter()
            .find(|rule| rule.holds(self))
            .expect("the last rule holds everywhere");
        (rule, rule.ground(self))
    }

    /// The facts at the unit direction `dir`.
    pub(crate) fn at<F: FieldView>(
        synth: &Synthesiser<'_, F>,
        cache: &mut SynthCache,
        dir: [f64; 3],
    ) -> Result<Self, QueryHeightError> {
        Ok(Point::at(synth, cache, dir)?.facts())
    }
}

/// The band level of R05's finest level on the body of `header`, at which the classifier reads
/// the ground ([`finest_level`]).
#[must_use]
pub(crate) fn finest_band(header: &FieldHeader) -> BandLevel {
    BandLevel::new(finest_level(header.figure().equatorial_radius_m))
        .expect("R05's finest level is a band level")
}

/// How steep the ground is against the classifier's two slope thresholds: what the cliff and snow
/// rules read of the slope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum Steepness {
    /// At most [`CLIFF_SLOPE`]: loose material rests on it.
    Loose,
    /// Above [`CLIFF_SLOPE`], at most [`SNOW_SHED_SLOPE`]: rock, where snow still lies.
    Cliff,
    /// Above [`SNOW_SHED_SLOPE`]: snow sheds from it too.
    Shed,
}

impl Steepness {
    /// The steepness of a ground whose normal makes an angle of cosine `cos_slope` with the datum's
    /// normal. The snow's threshold is the steeper, so ground above it is above the cliff's too.
    #[must_use]
    pub(crate) fn of(cos_slope: f64) -> Self {
        if cos_slope < math::cos(SNOW_SHED_SLOPE.value()) {
            Self::Shed
        } else if cos_slope < math::cos(CLIFF_SLOPE.value()) {
            Self::Cliff
        } else {
            Self::Loose
        }
    }
}

/// How a cell's ice is placed at a point ([`material_at`]'s "Where in a cell its ice lies").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum IcePlacement {
    /// The point lies in the cell's ice, or does not, whatever its ground: a cell with no ice, a
    /// whole share, or a sea's ice by the majority.
    Settled(bool),
    /// The point lies in the ice where its shadowed-ground index is above the cell's cut
    /// ([`ice_cut`]).
    ByCut,
}

/// What the classifier reads of a point's coarse cell, the same for every point of it: every
/// rule's input but the point's height, steepness, ice and channel bed, which [`Site::facts`]
/// takes, so that R10.T2's weights can ask what the rules decide at other heights and slopes.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Site<'a> {
    header: &'a FieldHeader,
    cell: &'a SynthesisCell,
    climate: &'a ClimateCell,
    entries: Entries,
    airless: bool,
    thin_aired: bool,
    closed_basin: bool,
    /// The climate's annual mean temperature at sea level, kelvin.
    annual_sea_k: f64,
    /// Its warmest month's, kelvin.
    warmest_sea_k: f64,
    /// Its mean precipitation, millimetres of water a Julian year.
    precipitation_mm: f64,
    /// The snow's entry, before the precipitation and the temperature are asked.
    snow_entry: Option<u8>,
}

impl<'a> Site<'a> {
    /// The site of `cell`, the record of `key` in `field`: its climate's and its neighbours' reads.
    ///
    /// # Errors
    ///
    /// [`QueryHeightError::NotSurveyed`] for the cell's climate or a neighbour the view does not
    /// hold.
    pub(crate) fn of<F: FieldView>(
        field: &'a F,
        key: PatchKey,
        cell: &'a SynthesisCell,
    ) -> Result<Self, QueryHeightError> {
        let header = field.header();
        let climate = field
            .climate(key)
            .ok_or(QueryHeightError::NotSurveyed(key))?;
        let pressure = header.surface_pressure();
        let airless = pressure <= Pascals::ZERO;
        Ok(Self {
            header,
            cell,
            climate,
            entries: Entries::of(header, cell.crust),
            airless,
            thin_aired: !airless && pressure < THIN_ATMOSPHERE_PRESSURE,
            closed_basin: is_closed_basin(field, key, cell)?,
            annual_sea_k: header.sea_level_temperature(climate).value(),
            warmest_sea_k: warmest_month_k(header, climate),
            precipitation_mm: annual_precipitation_mm(header, climate),
            snow_entry: snow_entry(header, cell),
        })
    }

    /// Whether ground of height `h_m` metres lies below the cell's water surface.
    #[must_use]
    pub(crate) fn under_water(&self, h_m: f64) -> bool {
        self.cell.is_under_water() && h_m < self.cell.water_surface().value()
    }

    /// How the cell's ice is placed at a point that lies under water, or does not.
    #[must_use]
    pub(crate) fn ice_placement(&self, under_water: bool) -> IcePlacement {
        match self.cell.ice {
            0 => IcePlacement::Settled(false),
            u8::MAX => IcePlacement::Settled(true),
            _ if under_water => IcePlacement::Settled(self.cell.ice_fraction() > 0.5),
            _ => IcePlacement::ByCut,
        }
    }

    /// The heights, metres, at which a rule's condition on the height may change: the water
    /// surface, and where the temperature by the lapse rate crosses the crust's transition, the
    /// snow's and Köppen–Geiger's arid bound, each where the cell has one. Those of the temperature
    /// are where the rules' own comparisons change to rounding: a caller asks the rules on either
    /// side of one, not at it.
    #[must_use]
    pub(crate) fn height_breaks(&self) -> Vec<f64> {
        let header = self.header;
        let palette = header.palette();
        let lapse = header.lapse_rate_k_per_m();
        let sea = header.sea_level().value();
        let mut breaks = Vec::with_capacity(4);
        if self.cell.is_under_water() {
            breaks.push(self.cell.water_surface().value());
        }
        let mut at_temperature = |sea_k: f64, k: f64| {
            // sea_k − lapse (h − sea) = k.
            let h = sea + (sea_k - k) / lapse;
            if h.is_finite() {
                breaks.push(h);
            }
        };
        if let Some(e) = self.entries.crust.and_then(|e| palette.get(e)) {
            at_temperature(self.annual_sea_k, e.transition.value());
        }
        if self.precipitation_mm > 0.0 {
            if let Some(e) = self.snow_entry.and_then(|e| palette.get(e)) {
                at_temperature(self.warmest_sea_k, e.transition.value());
            }
            let offset = koppen_offset_mm(header, self.climate, self.precipitation_mm);
            let arid_k =
                ZERO_CELSIUS_K + 0.5 * (self.precipitation_mm / KOPPEN_ARID_MULTIPLE - offset);
            at_temperature(self.annual_sea_k, arid_k);
        }
        breaks.sort_by(f64::total_cmp);
        breaks.dedup_by(|a, b| a.total_cmp(b).is_eq());
        breaks
    }

    /// The rules' conditions at a point of the cell whose ground is `h_m` metres high and of
    /// steepness `steepness`, lying in the cell's ice or not and on a channel's bed or not.
    #[must_use]
    pub(crate) fn facts(
        &self,
        h_m: f64,
        steepness: Steepness,
        in_ice: bool,
        channel_floor: bool,
    ) -> Facts {
        let header = self.header;
        let palette = header.palette();
        let cell = self.cell;
        let lapse_k = header.lapse_rate_k_per_m() * (h_m - header.sea_level().value());
        let annual_k = self.annual_sea_k - lapse_k;
        let warmest_k = self.warmest_sea_k - lapse_k;
        let molten = self
            .entries
            .crust
            .and_then(|e| palette.get(e))
            .is_some_and(|e| annual_k > e.transition.value());
        let precipitation_mm = self.precipitation_mm;
        let snow = self.snow_entry.filter(|&e| {
            precipitation_mm > 0.0
                && palette
                    .get(e)
                    .is_some_and(|e| warmest_k < e.transition.value())
        });
        let ice = (in_ice && cell.ice > 0).then(|| Ice {
            entry: cell.ice_entry(),
            massive: cell.ice_fraction() >= MASSIVE_ICE_SHARE,
        });
        Facts {
            under_water: self.under_water(h_m),
            ice,
            airless: self.airless,
            thin_aired: self.thin_aired,
            molten,
            snow,
            above_cliff: steepness != Steepness::Loose,
            above_snow_shed: steepness == Steepness::Shed,
            closed_basin: self.closed_basin,
            arid: is_arid(header, self.climate, annual_k, precipitation_mm),
            channel_floor,
            province: cell.crust == Crust::Province,
            entries: self.entries,
        }
    }
}

/// What the classifier reads at a point of the finest level: its cell's site, and the point's
/// height, slope, ice and channel bed.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Point<'a> {
    site: Site<'a>,
    height_m: f64,
    cos_slope: f64,
    in_ice: bool,
    channel_floor: bool,
}

impl<'a> Point<'a> {
    /// The point at the unit direction `dir`, at the body's finest level.
    ///
    /// # Errors
    ///
    /// [`QueryHeightError::NotSurveyed`] naming the first cell read that the view does not hold:
    /// the height's, the cell's site's, then the ice cut's grid and the channels'.
    pub(crate) fn at<F: FieldView>(
        synth: &Synthesiser<'a, F>,
        cache: &mut SynthCache,
        dir: [f64; 3],
    ) -> Result<Self, QueryHeightError> {
        let field = synth.field();
        let level = finest_band(field.header());
        let sample = synth.height_at(cache, dir, level)?;
        let (key, cell) = synth.cell_at(dir)?;
        let site = Site::of(field, key, cell)?;
        let h = sample.height_m;
        let in_ice = match site.ice_placement(site.under_water(h)) {
            IcePlacement::Settled(lies) => lies,
            IcePlacement::ByCut => {
                let cut = ice_cut(synth, cache, key, cell, level)?;
                cut.holds(shadow_terms(field, dir, &sample)?, cell.ice_fraction())
            }
        };
        Ok(Self {
            site,
            height_m: h,
            cos_slope: cos_slope(field.header(), dir, &sample),
            in_ice,
            channel_floor: on_channel_floor(synth, cache, dir, cell, level)?,
        })
    }

    /// The cosine of the point's slope, the angle between its ground's normal and the datum's.
    #[must_use]
    pub(crate) const fn cos_slope(&self) -> f64 {
        self.cos_slope
    }

    /// The rules' conditions at the point, of steepness `steepness`: its own, or another the
    /// weights ask of.
    #[must_use]
    pub(crate) fn facts_at(&self, steepness: Steepness) -> Facts {
        self.site
            .facts(self.height_m, steepness, self.in_ice, self.channel_floor)
    }

    /// The rules' conditions at the point.
    #[must_use]
    pub(crate) fn facts(&self) -> Facts {
        self.facts_at(Steepness::of(self.cos_slope))
    }
}

/// The cosine of the slope of the ground whose height and gradient at the unit direction `dir`
/// are `sample`: R05's [`surface_normal`] against the datum's normal.
#[must_use]
pub(crate) fn cos_slope(header: &FieldHeader, dir: [f64; 3], sample: &HeightSample) -> f64 {
    let figure = header.figure();
    let normal = surface_normal(&figure, dir, sample.height_m, sample.gradient);
    dot(normal, figure.normal(dir))
}

/// Whether the unit direction `dir`, in the coarse cell `cell`, lies on the bed of a channel that
/// band level `level` evaluates: never under water.
///
/// # Errors
///
/// [`QueryHeightError::NotSurveyed`] for a cell the channels read that the view does not hold.
pub(crate) fn on_channel_floor<F: FieldView>(
    synth: &Synthesiser<'_, F>,
    cache: &mut SynthCache,
    dir: [f64; 3],
    cell: &SynthesisCell,
    level: BandLevel,
) -> Result<bool, QueryHeightError> {
    Ok(match synth.level_set(level).channels_through() {
        Some(through) if !cell.is_under_water() => {
            synth
                .channels()
                .on_channel_floor(synth.field(), dir, through, cache)?
        }
        _ => false,
    })
}

/// Whether `cell`, the record of `key`, is a closed basin's floor: dry, its water going nowhere
/// ([`FlowDirection::Terminal`]) with a drainage area, and an edge neighbour's water flowing into
/// it, so that a flat cell with no inflow, which the builder's routing leaves terminal, is not one.
///
/// # Errors
///
/// [`QueryHeightError::NotSurveyed`] for a neighbour the view does not hold.
fn is_closed_basin<F: FieldView>(
    field: &F,
    key: PatchKey,
    cell: &SynthesisCell,
) -> Result<bool, QueryHeightError> {
    if cell.is_under_water()
        || cell.flow != FlowDirection::Terminal
        || cell.drainage == LogArea::ZERO
    {
        return Ok(false);
    }
    for edge in Edge::ALL {
        let (neighbour, back) = key.edge_neighbour_and_back(edge);
        let record = field
            .cell(neighbour)
            .ok_or(QueryHeightError::NotSurveyed(neighbour))?;
        if record.flow.edge() == Some(back) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The cells of a cut's grid along each side: 16, so that the grid's 256 points resolve a cell's
/// smallest ice share, 1 ÷ 255.
const CUT_GRID: u8 = 16;

/// The most cells whose cuts a cache keeps: past it the cache starts afresh, as its craters' part
/// does at the same count.
const CUT_CACHE_CELLS: usize = 16_384;

/// A point's shadowed-ground terms ([`material_at`]), of the ground finer than the coarse
/// cells: its fall towards the nearer pole per metre along the surface and its depth below the
/// coarse interpolant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ShadowTerms {
    /// The pole-facing tilt, metres a metre: positive where the ground falls towards the pole.
    tilt: f64,
    /// The depth below the coarse interpolant, metres: positive in a hollow.
    depth_m: f64,
}

/// A cell's cut of the shadowed-ground index for its ice share ([`material_at`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct IceCut {
    /// The tilt's weight in the index: one over its standard deviation over the cell's grid, or 0
    /// where it is zero.
    tilt_scale: f64,
    /// The depth's weight in the index, per metre, as the tilt's.
    depth_scale_per_m: f64,
    /// The index above which a point lies in the ice, or `None` where neither term varies.
    cut: Option<f64>,
}

impl IceCut {
    /// The index of a point of terms `terms`.
    #[must_use]
    pub(crate) fn index(&self, terms: ShadowTerms) -> f64 {
        terms.tilt * self.tilt_scale + terms.depth_m * self.depth_scale_per_m
    }

    /// Whether a point of terms `terms` lies in the cell's ice of share `share`: above the cut, or,
    /// where nothing varies, everywhere if the share is above a half.
    #[must_use]
    pub(crate) fn holds(&self, terms: ShadowTerms, share: f64) -> bool {
        match self.cut {
            Some(cut) => self.index(terms) > cut,
            None => share > 0.5,
        }
    }
}

/// The cuts a [`SynthCache`] keeps, for one field at a time: by packed cell key, level and ice
/// code, the three a cut depends on beside the field's heights.
///
/// Beside them, for R10.T2's weights, each cell's omitted index at a coarser band level
/// ([`ice_share_at`]): by packed cell key, that level and the ice code; and each cell's closed
/// forms of what a band level leaves out (`weights::Omitted`, R09's level-keyed forms, 50 to 150 µs
/// a call since R09.T7.h): by packed cell key and level. Each is a pure function of the field's
/// header and the cell's record, so no answer depends on what is kept.
#[derive(Debug, Clone, Default)]
pub(crate) struct IceCuts {
    /// The field the cuts are of, by what tells one coarse pass's field of a body and seed from
    /// another's, as the channels' memo is kept ([`NetworkId`]).
    of: Option<NetworkId>,
    cuts: BTreeMap<(u64, u8, u8), IceCut>,
    omitted: BTreeMap<(u64, u8, u8), Vec<f64>>,
    forms: BTreeMap<(u64, u8), [f64; 2]>,
}

impl IceCuts {
    /// No cuts.
    #[must_use]
    pub(crate) const fn new() -> Self {
        Self {
            of: None,
            cuts: BTreeMap::new(),
            omitted: BTreeMap::new(),
            forms: BTreeMap::new(),
        }
    }

    /// Keeps the cuts of the field `of` only: starts afresh for another.
    fn keep(&mut self, of: Option<NetworkId>) {
        if self.of != of {
            self.of = of;
            self.cuts.clear();
            self.omitted.clear();
            self.forms.clear();
        }
    }
}

/// The closed forms `compute` gives for the coarse cell `cell` at band level `level` on the field
/// of `synth`, kept in `cache` (R10.T2's weights: [`IceCuts`]).
///
/// # Errors
///
/// `compute`'s own.
pub(crate) fn kept_forms<F: FieldView, E>(
    synth: &Synthesiser<'_, F>,
    cache: &mut SynthCache,
    cell: PatchKey,
    level: BandLevel,
    compute: impl FnOnce() -> Result<[f64; 2], E>,
) -> Result<[f64; 2], E> {
    let key = (cell.to_u64(), level.get());
    let kept = cache.ice_cuts_mut();
    kept.keep(Some(synth.channels().id()));
    if let Some(&forms) = kept.forms.get(&key) {
        return Ok(forms);
    }
    let forms = compute()?;
    let kept = cache.ice_cuts_mut();
    if kept.forms.len() >= CUT_CACHE_CELLS {
        kept.forms.clear();
    }
    kept.forms.insert(key, forms);
    Ok(forms)
}

/// The terms of the point at the unit direction `dir`, whose height and gradient at the finest
/// level are `sample`.
///
/// # Errors
///
/// [`QueryHeightError::NotSurveyed`] for a cell the coarse interpolant reads that the view does not
/// hold.
pub(crate) fn shadow_terms<F: FieldView>(
    field: &F,
    dir: [f64; 3],
    sample: &HeightSample,
) -> Result<ShadowTerms, QueryHeightError> {
    let figure = field.header().figure();
    let base = interp::base_elevation(field, dir)?;
    let gradient: [f64; 3] = [0, 1, 2].map(|k| sample.gradient[k] - base.gradient[k]);
    let nu = figure.normal(dir);
    let along = dot(gradient, nu);
    let tangential = [0, 1, 2].map(|k| gradient[k] - along * nu[k]);
    // Towards the nearer pole along the surface; the north at the equator. At a pole itself there
    // is no such direction, and no tilt.
    let pole = [0.0, 0.0, if dir[2] >= 0.0 { 1.0 } else { -1.0 }];
    let up = dot(pole, nu);
    let towards = [0, 1, 2].map(|k| pole[k] - up * nu[k]);
    let towards_size = dot(towards, towards).sqrt();
    let tilt = if towards_size > 0.0 {
        -dot(tangential, towards) / towards_size
    } else {
        0.0
    };
    Ok(ShadowTerms {
        tilt: num::assert_finite(tilt),
        depth_m: num::assert_finite(base.height_m - sample.height_m),
    })
}

/// The cut of the ice of `record`, the record of `cell`, from the terms at its grid's points at
/// `level`, through `cache`.
///
/// # Errors
///
/// [`QueryHeightError::NotSurveyed`] for a cell a grid point reads that the view does not hold.
pub(crate) fn ice_cut<F: FieldView>(
    synth: &Synthesiser<'_, F>,
    cache: &mut SynthCache,
    cell: PatchKey,
    record: &SynthesisCell,
    level: BandLevel,
) -> Result<IceCut, QueryHeightError> {
    let field = synth.field();
    let of = Some(synth.channels().id());
    let key = (cell.to_u64(), level.get(), record.ice);
    let share = record.ice_fraction();
    {
        let cuts = cache.ice_cuts_mut();
        cuts.keep(of);
        if let Some(&cut) = cuts.cuts.get(&key) {
            return Ok(cut);
        }
    }
    let mut terms = Vec::with_capacity(usize::from(CUT_GRID) * usize::from(CUT_GRID));
    for b in 0..CUT_GRID {
        for a in 0..CUT_GRID {
            // The centre of step (a, b) of the cell's 16 × 16 equal steps: vertex 4a + 2 of 64.
            let dir = cell.vertex_dir(4 * a + 2, 4 * b + 2);
            let sample = synth.height_at(cache, dir, level)?;
            terms.push(shadow_terms(field, dir, &sample)?);
        }
    }
    let tilts: Vec<f64> = terms.iter().map(|t| t.tilt).collect();
    let depths: Vec<f64> = terms.iter().map(|t| t.depth_m).collect();
    let mut cut = IceCut {
        tilt_scale: inverse_deviation(&tilts),
        depth_scale_per_m: inverse_deviation(&depths),
        cut: None,
    };
    if cut.tilt_scale > 0.0 || cut.depth_scale_per_m > 0.0 {
        let mut z: Vec<f64> = terms.iter().map(|&t| cut.index(t)).collect();
        z.sort_by(f64::total_cmp);
        cut.cut = Some(quantile(&z, 1.0 - share));
    }
    let cuts = cache.ice_cuts_mut();
    if cuts.cuts.len() >= CUT_CACHE_CELLS {
        cuts.cuts.clear();
    }
    cuts.cuts.insert(key, cut);
    Ok(cut)
}

/// The share of the partial ice of `record`, the record of `cell`, that a point at the unit
/// direction `dir` of the band level `level` is expected to lie in, where `sample` is the level's
/// height and gradient there (R10.T2's weights at a level coarser than the finest).
///
/// The finest index is the level's plus what the level leaves out of it, and the part left out is
/// taken from the cell itself: at each of the cut's 256 grid points, the finest index less the
/// level's, each by the cut's scales ([`ice_cut`] at the finest level). The share is the fraction
/// of those whose sum with the point's index at the level is above the cut, as [`IceCut::holds`]
/// compares it. An empirical distribution, not a normal one: the index is heavy-tailed, craters
/// and steep walls carrying few of its points far out, so a normal deviate of its variance puts
/// too much of a small share's ice far from the cut (T1.b's record, "Where a partial share lies").
/// At a level that leaves nothing out each difference is zero and the share is the point's own
/// answer; at one that resolves nothing below the cells the point's index is zero and the share
/// is the grid's, the cell's share to its sampling. Where neither term varies, it is the cut's
/// majority. The differences are kept in `cache` as the cuts are.
///
/// # Errors
///
/// [`QueryHeightError::NotSurveyed`] for a cell the grid's points or the coarse interpolant read
/// that the view does not hold.
pub(crate) fn ice_share_at<F: FieldView>(
    synth: &Synthesiser<'_, F>,
    cache: &mut SynthCache,
    (cell, record): (PatchKey, &SynthesisCell),
    dir: [f64; 3],
    level: BandLevel,
    sample: &HeightSample,
) -> Result<f64, QueryHeightError> {
    let field = synth.field();
    let finest = finest_band(field.header());
    let cut = ice_cut(synth, cache, cell, record, finest)?;
    let Some(threshold) = cut.cut else {
        return Ok(f64::from(u8::from(record.ice_fraction() > 0.5)));
    };
    let index = cut.index(shadow_terms(field, dir, sample)?);
    let key = (cell.to_u64(), level.get(), record.ice);
    let held = {
        let cuts = cache.ice_cuts_mut();
        cuts.keep(Some(synth.channels().id()));
        cuts.omitted.contains_key(&key)
    };
    if !held {
        let mut omitted = Vec::with_capacity(usize::from(CUT_GRID) * usize::from(CUT_GRID));
        for b in 0..CUT_GRID {
            for a in 0..CUT_GRID {
                // The cut's own grid points: vertex 4a + 2 of 64.
                let at = cell.vertex_dir(4 * a + 2, 4 * b + 2);
                let fine = synth.height_at(cache, at, finest)?;
                let coarse = synth.height_at(cache, at, level)?;
                let fine = cut.index(shadow_terms(field, at, &fine)?);
                let coarse = cut.index(shadow_terms(field, at, &coarse)?);
                omitted.push(num::assert_finite(fine - coarse));
            }
        }
        omitted.sort_by(f64::total_cmp);
        let cuts = cache.ice_cuts_mut();
        if cuts.omitted.len() >= CUT_CACHE_CELLS {
            cuts.omitted.clear();
        }
        cuts.omitted.insert(key, omitted);
    }
    let omitted = &cache.ice_cuts_mut().omitted[&key];
    // Ascending differences: the sum is above the cut from some point on.
    let below = omitted.partition_point(|&d| index + d <= threshold);
    #[expect(clippy::cast_precision_loss, reason = "256 points, exact in f64")]
    let share = (omitted.len() - below) as f64 / omitted.len() as f64;
    Ok(share)
}

/// One over the standard deviation of `values` about their mean, or 0 where it is zero: each sum
/// in the values' order.
#[must_use]
fn inverse_deviation(values: &[f64]) -> f64 {
    #[expect(clippy::cast_precision_loss, reason = "256 values, exact in f64")]
    let count = values.len() as f64;
    let mean = values.iter().sum::<f64>() / count;
    let variance = values.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / count;
    if variance > 0.0 {
        1.0 / variance.sqrt()
    } else {
        0.0
    }
}

/// The `p` quantile (0 to 1) of the ascending values `sorted`, by linear interpolation between
/// order statistics at (n − 1) p (Hyndman and Fan 1996, Am. Stat. 50, 361, definition 7).
#[must_use]
fn quantile(sorted: &[f64], p: f64) -> f64 {
    #[expect(clippy::cast_precision_loss, reason = "256 values, exact in f64")]
    let at = (sorted.len() - 1) as f64 * p.clamp(0.0, 1.0);
    let low = at.floor();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "at is between 0 and n − 1: a whole number of order statistics"
    )]
    let k = low as usize;
    match sorted.get(k + 1) {
        Some(&next) => sorted[k] + (at - low) * (next - sorted[k]),
        None => sorted[k],
    }
}

/// The entry of the snow on `cell`: its own ice entry, else the `Ice` entry of the body's main
/// liquid's substance, the condensable that precipitates; `None` where the body names neither.
#[must_use]
fn snow_entry(header: &FieldHeader, cell: &SynthesisCell) -> Option<u8> {
    cell.ice_entry().or_else(|| {
        let liquid = header.palette().get(header.main_liquid()?)?;
        header.palette().find(PaletteRole::Ice, liquid.substance)
    })
}

/// The warmest month's mean temperature of `climate`, reduced to sea level, kelvin: the annual mean
/// in a one-month year.
#[must_use]
fn warmest_month_k(header: &FieldHeader, climate: &ClimateCell) -> f64 {
    (0..header.months())
        .filter_map(|m| header.month_temperature(climate, m))
        .map(Kelvin::value)
        .fold(header.sea_level_temperature(climate).value(), num::max)
}

/// The mean precipitation of `climate` over the year, millimetres of water a Julian year (a
/// kilogram a square metre is a millimetre of water): each month's rate weighted by its share of
/// the year ([`month_share`]).
#[must_use]
fn annual_precipitation_mm(header: &FieldHeader, climate: &ClimateCell) -> f64 {
    (0..header.months())
        .map(|m| {
            let share = month_share(header, m).expect("the month is in the year");
            share * climate.month_precipitation[usize::from(m)].rate_kg_per_m2_s()
        })
        .sum::<f64>()
        * SECONDS_PER_JULIAN_YEAR
}

/// Whether a point of `climate` whose annual mean temperature is `annual_k` and whose mean
/// precipitation is `precipitation_mm` (a Julian year) is arid: it does not precipitate, or it is
/// below Köppen–Geiger's arid threshold ([`material_at`]).
#[must_use]
fn is_arid(
    header: &FieldHeader,
    climate: &ClimateCell,
    annual_k: f64,
    precipitation_mm: f64,
) -> bool {
    if precipitation_mm <= 0.0 {
        return true;
    }
    let celsius = annual_k - ZERO_CELSIUS_K;
    let offset = koppen_offset_mm(header, climate, precipitation_mm);
    precipitation_mm < KOPPEN_ARID_MULTIPLE * (2.0 * celsius + offset)
}

/// Köppen–Geiger's seasonal offset of the arid threshold for `climate`, whose mean precipitation is
/// `precipitation_mm` (positive, a Julian year), millimetres a year: the summer's where 70% falls
/// in the summer half, none where 70% falls in the winter half, the even offset otherwise and in a
/// year without seasons (Peel, Finlayson and McMahon 2007, Table 1).
#[must_use]
fn koppen_offset_mm(header: &FieldHeader, climate: &ClimateCell, precipitation_mm: f64) -> f64 {
    match summer_share(header, climate, precipitation_mm) {
        Some(summer) if summer >= KOPPEN_SEASONAL_SHARE => KOPPEN_SUMMER_OFFSET_MM,
        Some(summer) if 1.0 - summer >= KOPPEN_SEASONAL_SHARE => 0.0,
        _ => KOPPEN_EVEN_OFFSET_MM,
    }
}

/// The share of the year's precipitation that falls in its warmer half, the six warmest months
/// (ties to the earlier month), or `None` in a one-month year, which has no seasons.
#[must_use]
fn summer_share(header: &FieldHeader, climate: &ClimateCell, precipitation_mm: f64) -> Option<f64> {
    if header.months() < 12 {
        return None;
    }
    let mut months: [(f64, u8); 12] = [(0.0, 0); 12];
    for (slot, m) in months.iter_mut().zip(0_u8..) {
        let t = header
            .month_temperature(climate, m)
            .expect("the month is in the year");
        *slot = (t.value(), m);
    }
    months.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    let summer: f64 = months[..6]
        .iter()
        .map(|&(_, m)| {
            let share = month_share(header, m).expect("the month is in the year");
            share * climate.month_precipitation[usize::from(m)].rate_kg_per_m2_s()
        })
        .sum::<f64>()
        * SECONDS_PER_JULIAN_YEAR;
    Some(summer / precipitation_mm)
}

/// The dot product, in index order.
#[must_use]
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[cfg(test)]
mod tests;
