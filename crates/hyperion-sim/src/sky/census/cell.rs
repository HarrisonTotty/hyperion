//! One cell of the census: its bright systems, each star placed and measured at its retarded time
//! (rendering plan R06, R06.T8.b; Design note 10).
//!
//! For each record the mass skip keeps
//! ([`SkyCellCache::bright_subset`](super::cache::SkyCellCache::bright_subset) above the cell's
//! floor, [`BrightnessEnvelope::mass_floor`]): the observer's own system is left out; its light is
//! bounded by [`flux_bound`] before its motion is built, at its epoch position's distance less the
//! cell's pad and offset and over the ages the light's travel then allows (R06.T8.f); a member of
//! the galactic centre, whose orbit is not built ([`TraceMotionError`]), is tallied and left out
//! before anything else is built; the system is found at its retarded time ([`retarded`] on
//! [`Drift::of_record`]); its light is bounded again at the emitted time and the apparent
//! position; and only if the bound can pass the cut is the system generated
//! ([`SystemStars::generate`]), each star read from the pair-evolved [`SystemStars::state_at`]'s
//! stars at the emitted time, placed by [`star_positions_at`] about the system's apparent position,
//! dimmed by its distance and, unless that alone already puts it past the cut, one [`sightline`] to
//! the observer, and kept if its V is brighter than the cut, plus its eye colour offset where the
//! eye is asked.
//!
//! Each skip is exact: it drops only what the census would drop after it, so the census's stars
//! are bit for bit those of the brute force, which generates every system and measures every star
//! ([`Bound::Ignored`]). The bound is the brightest single star's (Design note 10): the census
//! keeps each star alone, so a system is listable only if one of its stars is.
//!
//! The flux bound reads the envelope rather than the primary's brief: the brief is the primary's
//! single-star model, tested over the clock window only (ask A1), so for any star farther than
//! 1,000 ly it would cost a full generation; the envelope bound is cheaper and still a bound, so
//! the skip never changes an answer (R06's Risks). Plan 11's pairs can merge two stars into one
//! of up to twice the primary's mass, or feed one, and a main-sequence merger or accretor then
//! shines as a younger star of its new mass. So a grid system's bound and the cell's floor both
//! read the envelope at [`max_star_mass`] of the primary over ages from zero (R06.T16.b; see
//! [`flux_bound`]).
//!
//! A star of a multiple system is not at its system's barycentre: plan 11 keeps every apocentre
//! inside half the system's tidal radius ([`TIDAL_CUT_SHARE`]), which near the Sun is some light
//! years. Both the flux bound and the cell's floor take a star as near the observer as the cell's
//! bound allows ([`CellOffsets`], which bounds every record's [`star_offset_bound`]), read once per
//! cell (R06.T8.f).
//!
//! The census reads no luminosity table: its skips read the envelope, and each star's own state.
//! Readers of the tables (the caps, the band) pass their light ages through
//! [`LuminosityTables::age_for`](crate::sky::luminosity::LuminosityTables::age_for).

use core::f64::consts::LN_2;
use std::sync::LazyLock;

use crate::coords::{GalacticPosition, SystemPosition};
use crate::galaxy::consts::LIGHT_YEARS_PER_PARSEC;
use crate::galaxy::gas::extinction::{NoiseMode, Quality, sightline};
use crate::galaxy::gas::modifiers::GasModifier;
use crate::galaxy::imf::MassBand;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::{CellKey, SystemKind, SystemRecord};
use crate::galaxy::query::{pad_for, pad_speed};
use crate::galaxy::{Galaxy, PointLy};
use crate::id::{BodyId, Layer, SystemId};
use crate::math;
use crate::observe::{Drift, TraceMotionError, retarded};
use crate::stellar::Phase;
use crate::stellar::multiplicity::{
    MAX_COMPANIONS, MultiplicityContext, StarIndex, TIDAL_CUT_SHARE, star_positions_at,
};
use crate::stellar::system::{SystemStars, grid_multiplicity};
use crate::time::{Span, UniverseTime};
use crate::units::consts::METRES_PER_LIGHT_YEAR;
use crate::units::{LightYears, Magnitudes, SolarMasses, Years};

use super::super::colour::StarColour;
use super::super::envelope::{BrightnessEnvelope, MAX_AGE_YEARS, always_single, max_star_mass};
use super::super::eye::{SkyBackground, SpRatio, luminance, star_colour_offset};
use super::super::photometry::{absolute_v_of_state, colour_of_state};
use super::query::{SkyContext, SkyQuery};

/// The background each star's eye colour offset is taken against: μ 30 of reference light, fully
/// scotopic, where the offset is largest, as the eye's cut assumes at the darkest texel (Design
/// note 5); the limit map culls each texel's stars after.
static SCOTOPIC_SKY: LazyLock<SkyBackground> = LazyLock::new(|| {
    SkyBackground::new(
        luminance(crate::units::MagnitudesPerArcsec2::new(30.0)),
        SpRatio::REFERENCE,
    )
    .expect("a valid background")
});

/// The quality of each star's sightline (Design note 10).
const STAR_SIGHTLINE_QUALITY: Quality =
    Quality::Budget(core::num::NonZeroU32::new(64).expect("64 is not zero"));

/// A bound on any star's eye colour offset, mag: the colour table's hottest rows, its 500,000 K
/// blackbodies, give ρ 3.4850, 2.5 log₁₀(3.4850 ÷ 2.297) = 0.453 (Design note 5's largest colour
/// offset, which the eye's cut carries), held to 0.6 so that a census never skips a star its offset
/// would keep; a test holds every row of the table under it.
pub const EYE_OFFSET_BOUND_MAG: f64 = 0.6;

/// The distance modulus at `d_ly` light-years: 5 log₁₀(d ÷ 10 pc).
#[must_use]
fn distance_modulus(d_ly: f64) -> f64 {
    5.0 * math::log10(d_ly / (10.0 * LIGHT_YEARS_PER_PARSEC))
}

/// One star of the sky (Design note 17's fields before encoding).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkyStar {
    system: SystemId,
    layer: Layer,
    star: StarIndex,
    apparent: GalacticPosition,
    distance: LightYears,
    emitted: UniverseTime,
    v: Magnitudes,
    a_v: Magnitudes,
    colour: StarColour,
}

impl SkyStar {
    /// The star's system.
    #[must_use]
    pub const fn system(&self) -> SystemId {
        self.system
    }

    /// The mass layer its system was censused in: its record's ([`SystemRecord::layer`]), which
    /// for a grid system is its ID's.
    #[must_use]
    pub const fn layer(&self) -> Layer {
        self.layer
    }

    /// The star's index in its system.
    #[must_use]
    pub const fn star(&self) -> StarIndex {
        self.star
    }

    /// Where the star appears: its position at the emitted time, about its system's apparent
    /// position.
    #[must_use]
    pub const fn apparent(&self) -> &GalacticPosition {
        &self.apparent
    }

    /// The distance its light has come.
    #[must_use]
    pub const fn distance(&self) -> LightYears {
        self.distance
    }

    /// When its light left it.
    #[must_use]
    pub const fn emitted(&self) -> UniverseTime {
        self.emitted
    }

    /// Its apparent V, extinction included.
    #[must_use]
    pub const fn v(&self) -> Magnitudes {
        self.v
    }

    /// Its extinction in V along the sightline.
    #[must_use]
    pub const fn a_v(&self) -> Magnitudes {
        self.a_v
    }

    /// Its colour, from the spectral table (before reddening).
    #[must_use]
    pub const fn colour(&self) -> &StarColour {
        &self.colour
    }
}

#[cfg(test)]
impl SkyStar {
    /// Star `star` of grid system `system` at apparent V `v`, its other fields placeholders: for
    /// the merge's order tests, which need ties in V that no census gives.
    #[must_use]
    pub(super) fn placeholder(system: SystemId, star: StarIndex, v: f64) -> Self {
        Self {
            system,
            layer: system.layer().expect("a grid system"),
            star,
            apparent: GalacticPosition::from_light_years([0.0, 26_000.0, 68.0])
                .expect("in the cube"),
            distance: LightYears::new(1.0),
            emitted: UniverseTime::EPOCH,
            v: Magnitudes::new(v),
            a_v: Magnitudes::ZERO,
            colour: StarColour::default(),
        }
    }
}

/// What one layer's cells held (Design note 10's tallies), and how many of its stars a merge
/// listed (Design note 11).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct LayerTally {
    cells: u64,
    candidates: u64,
    generated: u64,
    accepted: u64,
    listed: u64,
    without_photometry: u64,
    centre_members: u64,
}

impl LayerTally {
    /// Cells opened.
    #[must_use]
    pub const fn cells(&self) -> u64 {
        self.cells
    }

    /// Records the mass skip kept.
    #[must_use]
    pub const fn candidates(&self) -> u64 {
        self.candidates
    }

    /// Systems generated, their flux bound passing.
    #[must_use]
    pub const fn generated(&self) -> u64 {
        self.generated
    }

    /// Stars kept: brighter than the cut, plus their eye colour offset where the eye is asked.
    #[must_use]
    pub const fn accepted(&self) -> u64 {
        self.accepted
    }

    /// Of the accepted, the stars [`merge_census`](super::merge_census) listed within `n_max`; the
    /// rest are its overflow. Zero in a cell's or a job's tallies, before the merge.
    #[must_use]
    pub const fn listed(&self) -> u64 {
        self.listed
    }

    /// Stars of a generated system that were white dwarfs, dark in V until ask A4.
    #[must_use]
    pub const fn without_photometry(&self) -> u64 {
        self.without_photometry
    }

    /// Records of the galactic centre's members, whose orbits are not built (until P09.T28).
    #[must_use]
    pub const fn centre_members(&self) -> u64 {
        self.centre_members
    }

    fn add(&mut self, other: &Self) {
        self.cells += other.cells;
        self.candidates += other.candidates;
        self.generated += other.generated;
        self.accepted += other.accepted;
        self.listed += other.listed;
        self.without_photometry += other.without_photometry;
        self.centre_members += other.centre_members;
    }
}

/// The census's tallies, per layer of [`Layer::ALL`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CensusTallies {
    layers: [LayerTally; Layer::ALL.len()],
    /// Plan 09's feature members are not in the census until P09.T40 (R06.T16.a).
    feature_members_absent: bool,
}

impl Default for CensusTallies {
    fn default() -> Self {
        Self {
            layers: [LayerTally::default(); Layer::ALL.len()],
            feature_members_absent: true,
        }
    }
}

impl CensusTallies {
    /// `layer`'s tally.
    #[must_use]
    pub fn layer(&self, layer: Layer) -> &LayerTally {
        &self.layers[usize::from(layer.value())]
    }

    fn layer_mut(&mut self, layer: Layer) -> &mut LayerTally {
        &mut self.layers[usize::from(layer.value())]
    }

    /// Whether plan 09's feature members are left out.
    #[must_use]
    pub const fn feature_members_absent(&self) -> bool {
        self.feature_members_absent
    }

    /// Adds `other`'s counts to these.
    pub fn add(&mut self, other: &Self) {
        for (a, b) in self.layers.iter_mut().zip(&other.layers) {
            a.add(b);
        }
        self.feature_members_absent |= other.feature_members_absent;
    }

    /// These tallies with plan 09's feature members present: for the merge's tests, until T16.a
    /// brings the members.
    #[cfg(test)]
    #[must_use]
    pub(super) const fn with_feature_members_present(mut self) -> Self {
        self.feature_members_absent = false;
        self
    }

    /// Sets each layer's [`LayerTally::listed`] to the number of `listed` in it: the merge's.
    pub(super) fn set_listed(&mut self, listed: &[SkyStar]) {
        for tally in &mut self.layers {
            tally.listed = 0;
        }
        for star in listed {
            self.layer_mut(star.layer).listed += 1;
        }
    }
}

/// Whether a record's skips are taken: its flux bound before its system is generated, and each
/// star's cut before its sightline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Bound {
    /// The census's: a system whose bound cannot pass the cut is not generated, and a star that
    /// cannot pass it with no extinction takes no sightline.
    Applied,
    /// The brute force's: every system is generated and every star measured.
    Ignored,
}

/// A star's apparent V at `d_ly` light-years with no extinction: `m_v` plus the distance modulus.
/// The census adds its extinction to this, so a star this alone puts past [`kept_to`] stays past
/// it.
#[must_use]
fn unextinguished_v(m_v: Magnitudes, d_ly: f64) -> f64 {
    m_v.value() + distance_modulus(d_ly.max(1e-6))
}

/// The faintest apparent V a star of `colour` is kept to for `query`: the cut, plus the star's eye
/// colour offset where the eye is asked.
#[must_use]
fn kept_to(query: &SkyQuery, colour: &StarColour) -> f64 {
    let offset = if query.eye().is_some() {
        SpRatio::new(colour.sp_ratio())
            .map_or(0.0, |rho| star_colour_offset(rho, &SCOTOPIC_SKY).value())
    } else {
        0.0
    };
    query.cut().value() + offset
}

/// The faintest absolute V a system's brightest possible star could have and still be listed at
/// `d_ly` light-years with no extinction, mag: the cut, the largest eye offset where the eye is
/// asked, less the distance modulus.
#[must_use]
fn faintest_listable(query: &SkyQuery, d_ly: f64) -> f64 {
    let offset = if query.eye().is_some() {
        EYE_OFFSET_BOUND_MAG
    } else {
        0.0
    };
    query.cut().value() + offset - distance_modulus(d_ly.max(1e-6))
}

/// The most bodies a grid system's hierarchy holds at the stellar level: plan 11's
/// [`MAX_COMPANIONS`] companions and the primary, which its draw caps (no subsystem is added once
/// a system has four stars), and one brown-dwarf companion beside them (P11.T2.d), which
/// [`SystemStars::state_at`] lists with the stars. A test holds every generated grid system to it.
#[expect(clippy::cast_possible_truncation, reason = "MAX_COMPANIONS is 3")]
pub const GRID_STAR_BOUND: u8 = MAX_COMPANIONS as u8 + 2;

/// The most stars a system of `record` can hold: one for a forced single (a free-floating brown
/// dwarf, which takes no companion of any kind), [`GRID_STAR_BOUND`] otherwise (a grid system's
/// redraws can change its count, so its first attempt's is not a bound). A grid record is a forced
/// single exactly when its layer is [`always_single`], which a test checks.
#[must_use]
fn star_bound(record: &SystemRecord) -> u8 {
    match grid_multiplicity(record) {
        MultiplicityContext::ForcedSingle => 1,
        MultiplicityContext::Free | MultiplicityContext::ForcedMultiple { .. } => GRID_STAR_BOUND,
    }
}

/// The farthest a star can lie from its system's barycentre, light-years, given the tidal radius
/// `tidal_radius_ly` of the system's mass: the depth of the deepest hierarchy
/// ([`GRID_STAR_BOUND`] − 1 orbits, each body no farther from its pair's barycentre than the
/// pair's separation) times the tidal cut every apocentre lies inside.
#[must_use]
fn offset_bound_at(tidal_radius_ly: f64) -> f64 {
    f64::from(GRID_STAR_BOUND - 1) * TIDAL_CUT_SHARE * tidal_radius_ly
}

/// The farthest any star of `record`'s system can lie from its barycentre, at the tidal radius of
/// [`GRID_STAR_BOUND`] × m₁ at its epoch position (no companion outweighs its primary): the depth
/// of the deepest hierarchy ([`GRID_STAR_BOUND`] − 1 orbits) times the tidal cut every apocentre
/// lies inside ([`TIDAL_CUT_SHARE`]) times that radius, and zero for a forced single.
#[must_use]
pub fn star_offset_bound(galaxy: &Galaxy, record: &SystemRecord) -> LightYears {
    if star_bound(record) == 1 {
        return LightYears::ZERO;
    }
    let mass = SolarMasses::new(f64::from(GRID_STAR_BOUND) * record.primary_initial_mass().value());
    let radius = galaxy
        .potential()
        .tidal_radius(mass, &PointLy::from(record.epoch_position()));
    LightYears::new(offset_bound_at(LightYears::from(radius).value()))
}

/// [`star_offset_bound`] for every record `key` can hold: the heaviest primary of its layer's
/// band, and the tidal radius bounded over the cell's whole extent from the galactic centre
/// ([`tidal_radius_bound_within`](crate::galaxy::potential::PotentialTables::tidal_radius_bound_within)
/// at its farthest corner); zero for a layer whose systems are all single ([`always_single`]),
/// whose one star is its barycentre. The census reads [`CellOffsets`] instead, which bounds every
/// record's offset too, and is at least this wherever the circular frequency does not rise between
/// the cell's corner and the table's node.
#[must_use]
pub fn cell_offset_bound(galaxy: &Galaxy, key: CellKey) -> LightYears {
    if always_single(key.layer()) {
        return LightYears::ZERO;
    }
    let radius = galaxy.potential().tidal_radius_bound_within(
        offset_mass(key.layer()),
        LightYears::new(farthest_from_centre_ly(key)),
    );
    LightYears::new(offset_bound_at(LightYears::from(radius).value()))
}

/// The mass whose tidal radius bounds every system of `layer`'s: [`GRID_STAR_BOUND`] times the
/// heaviest primary of its band, since no companion outweighs its primary.
#[must_use]
fn offset_mass(layer: Layer) -> SolarMasses {
    SolarMasses::new(f64::from(GRID_STAR_BOUND) * MassBand::from(layer).hi())
}

/// The distance of `key`'s farthest corner from the galactic centre, ly.
#[must_use]
fn farthest_from_centre_ly(key: CellKey) -> f64 {
    let size = f64::from(key.size_ly());
    let mut far_sq = 0.0;
    for &lo in &key.origin_ly() {
        let lo = f64::from(lo);
        let far = lo.abs().max((lo + size).abs());
        far_sq += far * far;
    }
    far_sq.sqrt()
}

/// The nodes per octave of galactocentric distance on which [`CellOffsets`] holds its bounds: some
/// 4.4% apart, so a cell's bound is at most that much farther out than its own corner.
const OFFSET_NODES_PER_OCTAVE: u32 = 16;

/// The first node, ly: 2⁻⁴, the potential tables' first grid point.
const OFFSET_FIRST_NODE_LY: f64 = 0.0625;

/// The octaves from the first node to the last, 2¹⁷ ly, beyond the root cube's farthest corner
/// from the centre (2¹⁶ √3, about 113,500 ly).
const OFFSET_OCTAVES: u32 = 21;

/// Per galaxy and layer, a bound on how far any star of a cell's systems lies from its
/// barycentre, read in O(1) a cell (R06.T8.f): [`cell_offset_bound`] costs some 40 µs, since its
/// tidal-radius bound scans the potential's grid points.
///
/// It holds, on galactocentric distance nodes [`OFFSET_NODES_PER_OCTAVE`] to the octave, the
/// running maximum of [`cell_offset_bound`]'s quantity at each node, and gives a cell the value at
/// the first node at or beyond its farthest corner from the centre. The tidal-radius bound within a
/// radius bounds the tidal radius of every point inside it, so the value bounds every record's
/// [`star_offset_bound`] in the cell; and it is never less than [`cell_offset_bound`] of the cell
/// wherever that grows with distance from the centre, as it does wherever the circular frequency
/// falls outward (a test checks it at many cells). A layer whose systems are all single holds zero.
///
/// It depends on the galaxy's potential alone, which its parameters fix, so a server builds it once
/// per galaxy, beside the luminosity tables, and every census job of that galaxy reads it through
/// its [`SkyContext`](super::SkyContext). Another galaxy's bounds could be too small for this one's
/// stars, so it keeps the parameters it was built for, and the census checks them in debug builds
/// ([`is_for`](Self::is_for)).
#[derive(Debug, Clone, PartialEq)]
pub struct CellOffsets {
    /// The parameters of the galaxy whose bounds these are.
    params: GalaxyParams,
    /// The nodes, ly, ascending.
    nodes: Vec<f64>,
    /// Per layer of [`Layer::ALL`], the bound at each node, ly; empty for a layer whose systems
    /// are all single.
    offsets: [Vec<f64>; Layer::ALL.len()],
}

impl CellOffsets {
    /// The bounds of `galaxy`'s cells: some 1,700 tidal-radius bounds, built once per galaxy.
    #[must_use]
    pub fn build(galaxy: &Galaxy) -> Self {
        let count = OFFSET_OCTAVES * OFFSET_NODES_PER_OCTAVE + 1;
        let nodes: Vec<f64> = (0..count)
            .map(|k| {
                OFFSET_FIRST_NODE_LY
                    * math::exp(LN_2 * f64::from(k) / f64::from(OFFSET_NODES_PER_OCTAVE))
            })
            .collect();
        let offsets = Layer::ALL.map(|layer| {
            if always_single(layer) {
                return Vec::new();
            }
            let mass = offset_mass(layer);
            let mut running = 0.0_f64;
            nodes
                .iter()
                .map(|&r| {
                    let radius = galaxy
                        .potential()
                        .tidal_radius_bound_within(mass, LightYears::new(r));
                    running = running.max(offset_bound_at(LightYears::from(radius).value()));
                    running
                })
                .collect()
        });
        Self {
            params: galaxy.params().clone(),
            nodes,
            offsets,
        }
    }

    /// Whether these are `galaxy`'s bounds: built for its parameters, which fix its potential.
    #[must_use]
    pub fn is_for(&self, galaxy: &Galaxy) -> bool {
        self.params == *galaxy.params()
    }

    /// The bound for `key`'s systems: how far any of their stars can lie from its barycentre.
    #[must_use]
    pub fn of(&self, key: CellKey) -> LightYears {
        let row = &self.offsets[usize::from(key.layer().value())];
        if row.is_empty() {
            return LightYears::ZERO;
        }
        let far = farthest_from_centre_ly(key);
        // Every cell of the root cube lies within the last node.
        let k = self
            .nodes
            .partition_point(|&node| node < far)
            .min(self.nodes.len() - 1);
        LightYears::new(row[k])
    }

    /// The bytes the bounds own on the heap.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        (self.nodes.capacity() + self.offsets.iter().map(Vec::capacity).sum::<usize>())
            * size_of::<f64>()
    }
}

/// The bound on the V light of any one star of a system, as an absolute magnitude, before its
/// stars are generated (decided 2026-10-02, item 2; R06.T16.b; R06.T8.f).
///
/// It is the brightest single star's, with no factor for the system's count: the census keeps each
/// star alone (Design note 10), so a system none of whose stars could pass the cut lists nothing,
/// whatever their sum. Were the census ever to list an unresolved system's blended light, the bound
/// would return to a flux sum over its stars.
///
/// - **A grid system of stars**, whose pairs may have interacted: the envelope at
///   [`max_star_mass`] of the primary (twice its mass, a merger's or an accretor's) and over
///   every age from zero to the system's at `emitted`. A main-sequence merger takes BSE's
///   fractional age at the pair's mass (eq. 80), and a main-sequence accretor keeps or lowers its
///   fractional age at its new mass (Hurley, Tout and Pols 2002, MNRAS 329, 897, §2.6.6), so each
///   shines as a younger star of its new mass. A donor keeps its fractional age at a lower mass,
///   which can put its own track's age past the system's, and an evolved accretor keeps its
///   track's luminosity with its radius from its new mass (Hurley, Pols and Tout 2000, §7.1).
///   Neither is taken to be brighter in V than a single star of at most 2 m₁ at an age within the
///   system's; the slow test `envelope_bounds_pair_states` checks it.
/// - **A single star**, as a forced single is (a free-floating brown dwarf, never in a pair): the
///   envelope's flux at the primary's own mass and age.
///
/// `None` where no star of the system can shine in V, the system is not yet born, or its record
/// has no density component (one not placed by the grid, which [`census_record`] then generates
/// unbounded).
#[must_use]
pub fn flux_bound(
    envelope: &BrightnessEnvelope,
    record: &SystemRecord,
    emitted: UniverseTime,
) -> Option<Magnitudes> {
    let age = record.age_at(emitted);
    flux_bound_over(envelope, record, (age, age))
}

/// [`flux_bound`] over every emitted time at which the system's age lies within `ages` (years,
/// inclusive): never fainter than it at any of them, and `None` only where it is `None` at each.
#[must_use]
fn flux_bound_over(
    envelope: &BrightnessEnvelope,
    record: &SystemRecord,
    (young, old): (Years, Years),
) -> Option<Magnitudes> {
    if old.value() <= 0.0 {
        return None;
    }
    let component = record.component()?;
    let m1 = record.primary_initial_mass();
    let (mass, ages) = if star_bound(record) == 1 {
        (m1, (young, old))
    } else {
        (max_star_mass(m1), (Years::ZERO, old))
    };
    envelope.brightest(record.layer(), component, mass, ages)
}

/// How far the light of a cell's stars can have come, for one query: what the cell's floor and its
/// records' bounds before and after their drift read, computed once per cell (R06.T8.f).
#[derive(Debug, Clone, Copy, PartialEq)]
struct CellReach {
    /// The least distance any star of the cell can have from the observer when its light left
    /// it, ly: the box's nearest point less the pad and the offset, at least zero.
    least: f64,
    /// How far a record can have moved from its epoch position by any time its light can have
    /// left it, ly.
    pad: f64,
    /// How far a star can lie from its system's barycentre, ly ([`CellOffsets::of`]).
    offset: f64,
}

impl CellReach {
    /// The reach of `key` for `query`.
    ///
    /// A record lies within its pad of the box and its stars within their offset of the record,
    /// so a listed star's light left it at most far + offset + pad before `t`, and the pad is β
    /// times |`t_emit` − epoch|: solved together, pad = β (|t − epoch| + far + offset) ÷ (1 − β).
    /// The same pad bounds where the record is at the retardation's first guess, from which the
    /// light's age is taken ([`retarded`]): that guess's light time, the present distance, is at
    /// most far + β |t − epoch|, within the same sum.
    ///
    /// It rests, as the range query's padding and the cells' floors do, on every grid record moving
    /// slower than its layer's [`pad_speed`]: plan 08's draw holds every speed below the least of
    /// the escape speed and 1,000 km/s (`galaxy::kinematics::draw`), and layer E pads at
    /// 3,000 km/s.
    #[must_use]
    fn of(offsets: &CellOffsets, key: CellKey, query: &SkyQuery) -> Self {
        let apex = query.observer().position().to_light_years_f64();
        let o = key.origin_ly();
        let size = f64::from(key.size_ly());
        let mut near_sq = 0.0;
        let mut far_sq = 0.0;
        for (&lo, &a) in o.iter().zip(&apex) {
            let lo = f64::from(lo);
            let hi = lo + size;
            let near = a.clamp(lo, hi) - a;
            let far = (a - lo).abs().max((hi - a).abs());
            near_sq += near * near;
            far_sq += far * far;
        }
        let t = query.observer().time();
        let speed = pad_speed(key.layer());
        let year = UniverseTime::EPOCH
            .checked_add(Span::from_julian_years(1).expect("a year is a span"))
            .expect("a year after the epoch is a time");
        let beta = pad_for(year, speed).value();
        let offset = offsets.of(key).value();
        let lead = t.since_epoch().as_julian_years_f64().abs();
        let pad = beta * (lead + far_sq.sqrt() + offset) / (1.0 - beta);
        Self {
            least: (near_sq.sqrt() - pad - offset).max(0.0),
            pad,
            offset,
        }
    }
}

/// The years by which the bound before the drift widens the light's age either way, beyond the
/// pad: far more than the rounding of the light's age (to the nanosecond) and of the ages (some
/// 10⁻⁶ years at 10¹⁰), so that the ages it reads hold every age the bound after the drift can.
const BEFORE_DRIFT_SLACK_YEARS: f64 = 1.0;

/// The share of the epoch distance by which the bound before the drift brings the system nearer,
/// beyond the pad: far more than the rounding of the distances (cell-integer arithmetic, some
/// 10⁻¹⁶ of them), so that its distance is never beyond the one the bound after the drift reads.
const BEFORE_DRIFT_SLACK_SHARE: f64 = 1e-9;

/// Whether `record`'s flux bound can pass the cut before its drift is built (R06.T8.f): the bound
/// over the system's ages across every light time its distance allows, at its epoch position's
/// distance less the cell's pad and offset. A record that fails would fail after its retardation
/// too, where [`census_record`] tests the bound at the emitted time and the apparent position:
/// its pad bounds how far the record moves by the emitted time and by the retardation's first
/// guess, so its light's age lies within the pad of its epoch distance and its apparent position
/// is no nearer than that distance less the pad.
#[must_use]
fn passes_before_drift(
    envelope: &BrightnessEnvelope,
    record: &SystemRecord,
    query: &SkyQuery,
    reach: &CellReach,
) -> bool {
    let observer = query.observer();
    let d_epoch = observer
        .position()
        .distance_to(record.epoch_position())
        .value()
        / METRES_PER_LIGHT_YEAR;
    // A light-year is a Julian year of light, so the light's age in years is its distance in ly.
    let now = record.age_at(observer.time()).value();
    let ages = (
        Years::new(now - (d_epoch + reach.pad) - BEFORE_DRIFT_SLACK_YEARS),
        Years::new(now - (d_epoch - reach.pad).max(0.0) + BEFORE_DRIFT_SLACK_YEARS),
    );
    let Some(m) = flux_bound_over(envelope, record, ages) else {
        return false;
    };
    let nearest = d_epoch * (1.0 - BEFORE_DRIFT_SLACK_SHARE) - reach.pad - reach.offset;
    m.value() <= faintest_listable(query, nearest)
}

/// The extinction in V from a star at `apparent` to the observer at `observer_at`: one
/// [`sightline`] through the realised field and the modifiers near the segment (`modifiers` is
/// the caller's buffer), never negative.
///
/// The extinction is a sum of non-negative columns, but a modifier cloud's column is a difference
/// of two values of its antiderivative, which can round below zero far from the cloud. Held at
/// zero, the cut before the sightline is exact whatever the modifiers. The brute force measures
/// the same way, and a NaN still shows.
fn star_extinction(
    galaxy: &Galaxy,
    ctx: &mut SkyContext<'_>,
    apparent: &GalacticPosition,
    observer_at: &GalacticPosition,
    modifiers: &mut Vec<GasModifier>,
) -> Magnitudes {
    modifiers.clear();
    ctx.modifiers
        .modifiers_near_segment(apparent, observer_at, modifiers);
    let a_v = sightline(
        galaxy.gas(),
        apparent,
        observer_at,
        NoiseMode::Realised,
        STAR_SIGHTLINE_QUALITY,
        modifiers,
        &mut ctx.noise,
    )
    .a_v();
    if a_v.value() < 0.0 {
        Magnitudes::ZERO
    } else {
        a_v
    }
}

/// The mass floor of `key` for `query`: the least primary mass whose system could hold a star
/// listable at the cell's least distance from the observer, its records' motion over the light's
/// age and its stars' offsets from their barycentres allowed for ([`CellOffsets`]), at the bound
/// of one star, as [`flux_bound`]'s. Like that bound, it reads the envelope at [`max_star_mass`]
/// of each primary, twice its mass, over every age (through [`BrightnessEnvelope::mass_floor`]),
/// so a primary is kept whenever a merger or an accretor it could make might be listed; a layer
/// whose systems are all single, the brown dwarfs', reads each primary's own mass. The envelope is
/// the same for every component, so the galaxy's first is asked. It reads one value a mass node
/// and a table of `ctx`'s: under a microsecond a cell, 0.15–0.51 µs on T16.b's 648 cells near the
/// Sun (R06.T8.f, as built).
///
/// # Panics
///
/// If the galaxy has no density component, which no galaxy is built without.
#[must_use]
pub fn cell_floor(
    galaxy: &Galaxy,
    ctx: &SkyContext<'_>,
    key: CellKey,
    query: &SkyQuery,
) -> SolarMasses {
    debug_assert!(ctx.offsets.is_for(galaxy), "another galaxy's offset bounds");
    floor_at(
        galaxy,
        ctx.envelope,
        key,
        query,
        &CellReach::of(ctx.offsets, key, query),
    )
}

/// [`cell_floor`] of `key` at its `reach`.
fn floor_at(
    galaxy: &Galaxy,
    envelope: &BrightnessEnvelope,
    key: CellKey,
    query: &SkyQuery,
    reach: &CellReach,
) -> SolarMasses {
    let component = galaxy
        .fields()
        .component_ids()
        .next()
        .expect("a galaxy has components");
    envelope.mass_floor(
        key.layer(),
        component,
        Magnitudes::new(faintest_listable(query, reach.least)),
        (Years::ZERO, Years::new(MAX_AGE_YEARS)),
    )
}

/// The stars of one record kept for `query` (see the [module](self) documentation), appended to
/// `out`, with its counts in `tally`. `bound` says whether the skips are taken: under
/// [`Bound::Applied`] a grid record is bounded as its cell's census bounds it.
///
/// # Panics
///
/// As [`SystemStars::generate`], for a record of another galaxy.
pub fn census_record(
    galaxy: &Galaxy,
    ctx: &mut SkyContext<'_>,
    record: &SystemRecord,
    query: &SkyQuery,
    bound: Bound,
    tally: &mut CensusTallies,
    out: &mut Vec<SkyStar>,
) {
    debug_assert!(ctx.offsets.is_for(galaxy), "another galaxy's offset bounds");
    let reach = match bound {
        Bound::Applied => CellKey::of(record.id())
            .ok()
            .map(|key| CellReach::of(ctx.offsets, key, query)),
        Bound::Ignored => None,
    };
    record_stars(
        galaxy,
        ctx,
        record,
        query,
        (bound, reach.as_ref()),
        tally,
        out,
    );
}

/// [`census_record`] with its cell's reach, which bounds the record's light when given and the
/// record has a density component.
fn record_stars(
    galaxy: &Galaxy,
    ctx: &mut SkyContext<'_>,
    record: &SystemRecord,
    query: &SkyQuery,
    (bound, reach): (Bound, Option<&CellReach>),
    tally: &mut CensusTallies,
    out: &mut Vec<SkyStar>,
) {
    if query.exclude() == Some(record.id()) || record.kind() == SystemKind::RoguePlanet {
        return;
    }
    // A record no density component placed (a feature member's, once T16.a brings them) has no
    // envelope bound here and is always generated, so both modes agree.
    let reach = reach.filter(|_| record.component().is_some());
    if let Some(reach) = reach
        && !passes_before_drift(ctx.envelope, record, query, reach)
    {
        return;
    }
    let layer = record.layer();
    let drift = match Drift::of_record(galaxy, record) {
        Ok(drift) => drift,
        Err(TraceMotionError::CentreOrbitNotBuilt(_)) => {
            tally.layer_mut(layer).centre_members += 1;
            return;
        }
    };
    let observer = query.observer();
    let r = retarded(observer, &drift);
    let emitted = r.emitted();
    // The distance to the apparent position, from which each star's own is measured, so that the
    // offset bound holds by the triangle inequality (the light age is the first guess's).
    let d_system = observer
        .position()
        .distance_to(r.apparent_position())
        .value()
        / METRES_PER_LIGHT_YEAR;
    if let Some(reach) = reach {
        let Some(m) = flux_bound(ctx.envelope, record, emitted) else {
            return;
        };
        if m.value() > faintest_listable(query, d_system - reach.offset) {
            return;
        }
    }
    let stars = SystemStars::generate(galaxy, record);
    let Some(state) = stars.state_at(emitted) else {
        return;
    };
    tally.layer_mut(layer).generated += 1;
    let mut positions: Vec<(BodyId, SystemPosition)> =
        Vec::with_capacity(usize::from(GRID_STAR_BOUND));
    star_positions_at(stars.hierarchy(), emitted, &mut positions);
    let observer_at = observer.position();
    let mut modifiers: Vec<GasModifier> = Vec::new();
    for (body, place) in &positions {
        let index = u8::try_from(body.body_index())
            .ok()
            .and_then(StarIndex::from_body)
            .expect("a hierarchy's bodies are its stars, at most five");
        let star = &state.stars()[usize::from(index.get())];
        let Some(m_v) = absolute_v_of_state(star) else {
            if matches!(
                star.phase(),
                Phase::HeliumWhiteDwarf
                    | Phase::CarbonOxygenWhiteDwarf
                    | Phase::OxygenNeonWhiteDwarf
            ) {
                tally.layer_mut(layer).without_photometry += 1;
            }
            continue;
        };
        let Some(apparent) = place.to_galactic(r.apparent_position()) else {
            continue;
        };
        let d = observer_at.distance_to(&apparent).value() / METRES_PER_LIGHT_YEAR;
        let unextinguished = unextinguished_v(m_v, d);
        let colour = colour_of_state(star);
        let limit = kept_to(query, &colour);
        // A_V is never negative (below), and adding it never lowers a float: a star past the cut
        // before its extinction stays past it, so it takes no sightline (R06.T8.f).
        if bound == Bound::Applied && unextinguished > limit {
            continue;
        }
        let a_v = star_extinction(galaxy, ctx, &apparent, observer_at, &mut modifiers);
        let v = unextinguished + a_v.value();
        if v > limit {
            continue;
        }
        out.push(SkyStar {
            system: record.id(),
            layer,
            star: index,
            apparent,
            distance: LightYears::new(d),
            emitted,
            v: Magnitudes::new(v),
            a_v,
            colour,
        });
        tally.layer_mut(layer).accepted += 1;
    }
}

/// The stars of `key` kept for `query`, appended to `out` (which is not cleared); its counts are
/// returned, for the caller to add up with [`CensusTallies::add`].
///
/// # Examples
///
/// ```no_run
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
/// use hyperion_sim::galaxy::gas::noise::NoiseCache;
/// use hyperion_sim::observe::Observer;
/// use hyperion_sim::Seed;
/// use hyperion_sim::sky::census::{
///     CellOffsets, CensusTallies, NoSkyCellCache, SkyContext, SkyQuery, census_cell, census_plan,
/// };
/// use hyperion_sim::sky::envelope::BrightnessEnvelope;
/// use hyperion_sim::sky::luminosity::LuminosityTables;
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::Magnitudes;
///
/// let galaxy = Galaxy::new(Seed::new(11));
/// let (tables, envelope) = (LuminosityTables::build(&galaxy), BrightnessEnvelope::build(&galaxy));
/// let offsets = CellOffsets::build(&galaxy);
/// let at = GalacticPosition::from_light_years([0.0, 26_000.0, 68.0]).ok_or("in the cube")?;
/// let query = SkyQuery::builder(Observer::new(at, UniverseTime::EPOCH)?, Magnitudes::new(6.5))
///     .build()?;
/// let mut noise = NoiseCache::with_capacity(1 << 16);
/// let plan = census_plan(&galaxy, &tables, &envelope, &query, &mut noise);
/// let mut ctx = SkyContext {
///     tables: &tables,
///     envelope: &envelope,
///     offsets: &offsets,
///     noise,
///     cells: &NoSkyCellCache,
///     sources: &[],
///     modifiers: &NoModifiers,
/// };
/// // A server runs the cells as jobs, each with its own context, and merges the parts.
/// let (mut stars, mut tallies) = (Vec::new(), CensusTallies::default());
/// for key in plan.cells() {
///     tallies.add(&census_cell(&galaxy, &mut ctx, key, &query, &mut stars));
/// }
/// assert!(!stars.is_empty());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn census_cell(
    galaxy: &Galaxy,
    ctx: &mut SkyContext<'_>,
    key: CellKey,
    query: &SkyQuery,
    out: &mut Vec<SkyStar>,
) -> CensusTallies {
    let mut tally = CensusTallies::default();
    let layer = key.layer();
    tally.layer_mut(layer).cells += 1;
    debug_assert!(ctx.offsets.is_for(galaxy), "another galaxy's offset bounds");
    let reach = CellReach::of(ctx.offsets, key, query);
    let floor = floor_at(galaxy, ctx.envelope, key, query, &reach);
    let mut records = Vec::new();
    ctx.cells.bright_subset(galaxy, key, floor, &mut records);
    tally.layer_mut(layer).candidates += u64::try_from(records.len()).unwrap_or(u64::MAX);
    for record in &records {
        record_stars(
            galaxy,
            ctx,
            record,
            query,
            (Bound::Applied, Some(&reach)),
            &mut tally,
            out,
        );
    }
    tally
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::features::centre::testing::milky_way_galaxy;
    use crate::galaxy::gas::modifiers::NoModifiers;
    use crate::galaxy::gas::noise::NoiseCache;
    use crate::galaxy::placement::generate_cell;
    use crate::id::CentreMemberId;
    use crate::observe::Observer;
    use crate::sky::census::cache::NoSkyCellCache;
    use crate::sky::testing::{milky_way_dark_tables, milky_way_envelope, milky_way_offsets};

    /// The Sun's place in the fixture, ly.
    const SUN: [f64; 3] = [0.0, 26_000.0, 68.0];

    /// An inner-bulge place, ly, where the stars are old.
    const BULGE: [f64; 3] = [0.0, 3_000.0, 0.0];

    fn context() -> SkyContext<'static> {
        SkyContext {
            tables: milky_way_dark_tables(),
            envelope: milky_way_envelope(),
            offsets: milky_way_offsets(),
            noise: NoiseCache::with_capacity(1 << 12),
            cells: &NoSkyCellCache,
            sources: &[],
            modifiers: &NoModifiers,
        }
    }

    fn position(ly: [f64; 3]) -> GalacticPosition {
        GalacticPosition::from_light_years(ly).expect("in the cube")
    }

    fn observer_at(ly: [f64; 3]) -> Observer {
        Observer::new(position(ly), UniverseTime::EPOCH).expect("an observer")
    }

    /// At least `n` of `layer`'s records, from the cells of a growing cube about `at`.
    fn records_near(layer: Layer, at: [f64; 3], n: usize) -> Vec<SystemRecord> {
        let galaxy = milky_way_galaxy();
        let size = f64::from(layer.cell_size_ly());
        let mut out = Vec::new();
        let mut cell = Vec::new();
        'walk: for k in 0_i32.. {
            for dx in -k..=k {
                for dy in -k..=k {
                    for dz in -k..=k {
                        if dx.abs().max(dy.abs()).max(dz.abs()) != k {
                            continue;
                        }
                        let p = [
                            at[0] + f64::from(dx) * size,
                            at[1] + f64::from(dy) * size,
                            at[2] + f64::from(dz) * size,
                        ];
                        let key = CellKey::containing(layer, &position(p)).expect("in the cube");
                        generate_cell(galaxy, key, &mut cell);
                        out.append(&mut cell);
                        if out.len() >= n {
                            break 'walk;
                        }
                    }
                }
            }
        }
        out
    }

    /// The first record of `layer` near `at` whose system, generated as `rebuilt` makes it, at
    /// `t`, `pick` accepts.
    fn find_system(
        layer: Layer,
        at: [f64; 3],
        rebuilt: impl Fn(&SystemRecord) -> SystemRecord,
        pick: impl Fn(&SystemStars, &crate::stellar::system::SystemState) -> bool,
    ) -> SystemRecord {
        let galaxy = milky_way_galaxy();
        for record in records_near(layer, at, 40_000) {
            let record = rebuilt(&record);
            let stars = SystemStars::generate(galaxy, &record);
            if let Some(state) = stars.state_at(UniverseTime::EPOCH)
                && pick(&stars, &state)
            {
                return record;
            }
        }
        panic!("no system of {layer:?} near {at:?} found");
    }

    /// A query from 20 ly beyond `record`'s epoch position, along +x, to `cut`.
    fn query_beside(record: &SystemRecord, cut: f64) -> SkyQuery {
        let [x, y, z] = record.epoch_position().to_light_years_f64();
        SkyQuery::builder(observer_at([x + 20.0, y, z]), Magnitudes::new(cut))
            .build()
            .expect("a valid query")
    }

    /// Each star's apparent V as the census measures it for `query`, by star index, with every
    /// star kept (the cut at its maximum), and the census's own list at `query`'s cut.
    fn measured(record: &SystemRecord, query: &SkyQuery) -> (Vec<SkyStar>, Vec<SkyStar>) {
        let galaxy = milky_way_galaxy();
        let all = SkyQuery::builder(*query.observer(), Magnitudes::new(11.0))
            .build()
            .expect("a valid query");
        let mut ctx = context();
        let (mut every, mut listed) = (Vec::new(), Vec::new());
        let mut tally = CensusTallies::default();
        census_record(
            galaxy,
            &mut ctx,
            record,
            &all,
            Bound::Ignored,
            &mut tally,
            &mut every,
        );
        census_record(
            galaxy,
            &mut ctx,
            record,
            query,
            Bound::Applied,
            &mut tally,
            &mut listed,
        );
        (every, listed)
    }

    /// Holds every system of `per_place` records at each of several places and layers to the
    /// star bound and its stars to their offset bounds.
    fn check_star_bounds(per_place: usize) {
        let galaxy = milky_way_galaxy();
        let mut most = 0_u8;
        let mut positions = Vec::new();
        for (layer, at) in [
            (Layer::A, SUN),
            (Layer::B, SUN),
            (Layer::C, SUN),
            (Layer::D, SUN),
            (Layer::E, SUN),
            (Layer::E, BULGE),
            (Layer::BrownDwarf, SUN),
        ] {
            for record in records_near(layer, at, per_place) {
                let stars = SystemStars::generate(galaxy, &record);
                let n = stars.star_count();
                assert_eq!(usize::from(n), stars.stars().len());
                assert!(n <= star_bound(&record), "{record:?} holds {n}");
                assert_eq!(
                    star_bound(&record) == 1,
                    always_single(layer),
                    "{record:?}: a forced single exactly in a layer of singles"
                );
                let stellar = stars
                    .stars()
                    .iter()
                    .filter(|s| s.initial_mass().value() >= 0.08)
                    .count();
                assert!(stellar <= MAX_COMPANIONS + 1, "{record:?}");
                most = most.max(n);
                let own = star_offset_bound(galaxy, &record).value();
                let cell = CellKey::of(record.id()).expect("a grid record");
                let cell_bound = cell_offset_bound(galaxy, cell).value();
                assert!(
                    own <= cell_bound,
                    "{record:?}: {own} over the cell's {cell_bound}"
                );
                let table = milky_way_offsets().of(cell).value();
                assert!(cell_bound <= table, "{record:?}: {cell_bound} over {table}");
                star_positions_at(stars.hierarchy(), UniverseTime::EPOCH, &mut positions);
                for (_, place) in &positions {
                    let r = place.metres().iter().map(|x| x * x).sum::<f64>().sqrt()
                        / METRES_PER_LIGHT_YEAR;
                    assert!(r <= own, "{record:?}: a star {r} ly out, bound {own}");
                }
            }
        }
        // The sample reaches quadruples, so the bound is exercised.
        assert!(most >= 4, "{most}");
    }

    #[test]
    fn no_grid_system_holds_more_than_the_star_bound() {
        check_star_bounds(400);
    }

    #[test]
    #[ignore = "slow: generates some 20,000 systems"]
    fn no_grid_system_holds_more_than_the_star_bound_densely() {
        check_star_bounds(3_000);
    }

    /// Holds every star's absolute V of `per_place` systems at each of several places and
    /// layers, at two emitted times, at or below its system's flux bound; returns the stars
    /// checked.
    fn check_flux_bound(per_place: usize) -> u32 {
        let galaxy = milky_way_galaxy();
        let envelope = milky_way_envelope();
        let earlier = UniverseTime::EPOCH
            .checked_sub(Span::from_julian_years(900).expect("a span"))
            .expect("in the window");
        let mut checked = 0_u32;
        for (layer, at) in [
            (Layer::A, SUN),
            (Layer::B, SUN),
            (Layer::C, SUN),
            (Layer::C, BULGE),
            (Layer::D, SUN),
            (Layer::D, BULGE),
            (Layer::E, SUN),
            (Layer::E, BULGE),
            (Layer::BrownDwarf, SUN),
        ] {
            for record in records_near(layer, at, per_place) {
                let stars = SystemStars::generate(galaxy, &record);
                for t in [UniverseTime::EPOCH, earlier] {
                    let bound = flux_bound(envelope, &record, t);
                    let Some(state) = stars.state_at(t) else {
                        assert_eq!(bound, None, "{record:?} is not born");
                        continue;
                    };
                    for star in state.stars() {
                        let Some(m_v) = absolute_v_of_state(star) else {
                            continue;
                        };
                        let bound = bound.unwrap_or_else(|| panic!("{record:?} shines"));
                        assert!(
                            m_v.value() >= bound.value(),
                            "{record:?}: {:?} at M_V {} above the bound {}",
                            star.phase(),
                            m_v.value(),
                            bound.value()
                        );
                        checked += 1;
                    }
                }
            }
        }
        checked
    }

    #[test]
    fn the_flux_bound_is_at_or_above_every_stars_light() {
        assert!(check_flux_bound(400) > 3_000);
    }

    #[test]
    #[ignore = "slow: generates some 36,000 systems"]
    fn the_flux_bound_is_at_or_above_every_stars_light_densely() {
        assert!(check_flux_bound(4_000) > 30_000);
    }

    /// `record` with its primary's initial mass and its age at the epoch replaced.
    fn with_mass_and_age(record: &SystemRecord, mass: f64, age: f64) -> SystemRecord {
        SystemRecord::from_parts(
            record.id(),
            *record.epoch_position(),
            record.origin(),
            record.population(),
            SolarMasses::new(mass),
            Years::new(age),
        )
    }

    #[test]
    fn a_white_dwarf_primary_beside_a_bright_companion_lists_the_companion() {
        // 3 M☉ lives some 0.4 Gyr; at 1 Gyr it is a white dwarf, and any companion below about
        // 2.2 M☉ is still on its main sequence.
        let record = find_system(
            Layer::D,
            SUN,
            |r| with_mass_and_age(r, 3.0, 1e9),
            |_, state| {
                let [primary, rest @ ..] = state.stars() else {
                    return false;
                };
                is_white_dwarf(primary.phase())
                    && rest
                        .iter()
                        .any(|s| absolute_v_of_state(s).is_some_and(|v| v.value() < 3.0))
            },
        );
        let query = query_beside(&record, 6.0);
        let (every, listed) = measured(&record, &query);
        assert!(every.iter().all(|s| s.star() != StarIndex::PRIMARY));
        assert!(!listed.is_empty(), "{record:?}");
        assert!(listed.iter().all(|s| s.star() != StarIndex::PRIMARY));
        let kept: Vec<SkyStar> = every.into_iter().filter(|s| s.v().value() <= 6.0).collect();
        assert_eq!(listed, kept, "the bound changed the answer");
        let mut tally = CensusTallies::default();
        let mut out = Vec::new();
        census_record(
            milky_way_galaxy(),
            &mut context(),
            &record,
            &query,
            Bound::Applied,
            &mut tally,
            &mut out,
        );
        assert!(tally.layer(Layer::D).without_photometry() >= 1);
        assert_eq!(
            tally.layer(Layer::D).accepted(),
            u64::try_from(out.len()).expect("few")
        );
    }

    fn is_white_dwarf(phase: Phase) -> bool {
        matches!(
            phase,
            Phase::HeliumWhiteDwarf | Phase::CarbonOxygenWhiteDwarf | Phase::OxygenNeonWhiteDwarf
        )
    }

    fn is_giant(phase: Phase) -> bool {
        matches!(
            phase,
            Phase::FirstGiantBranch
                | Phase::CoreHeliumBurning
                | Phase::EarlyAgb
                | Phase::ThermallyPulsingAgb
        )
    }

    #[test]
    fn a_post_agb_primary_beside_a_giant_takes_the_envelope_bound_and_lists_the_giant() {
        let galaxy = milky_way_galaxy();
        let envelope = milky_way_envelope();
        // A 2 M☉ primary at the end of its post-AGB crossing, where it is hottest and faintest in
        // V, beside a near-twin still on its giant branches.
        let at_end_of_life = |r: &SystemRecord| {
            let probe = with_mass_and_age(r, 2.0, 1e9);
            let model = SystemStars::generate(galaxy, &probe);
            let Some(life) = model.primary().lifetime() else {
                return probe;
            };
            // Its light is 20 years old at the observer: the age at the epoch is 20 years more.
            with_mass_and_age(r, 2.0, life.value() - 30.0)
        };
        let emitted = UniverseTime::EPOCH
            .checked_sub(Span::from_julian_years(20).expect("a span"))
            .expect("in range");
        let record = find_system(Layer::C, SUN, at_end_of_life, |stars, _| {
            let Some(state) = stars.state_at(emitted) else {
                return false;
            };
            let [primary, rest @ ..] = state.stars() else {
                return false;
            };
            let Some(v1) = absolute_v_of_state(primary) else {
                return false;
            };
            primary.phase() == Phase::PostAgb
                && rest
                    .iter()
                    .any(|s| is_giant(s.phase()) && absolute_v_of_state(s).is_some_and(|v| v < v1))
        });
        let query = query_beside(&record, 11.0);
        let (every, listed) = measured(&record, &query);
        let primary = every
            .iter()
            .find(|s| s.star() == StarIndex::PRIMARY)
            .expect("a post-AGB star shines");
        let giant = every
            .iter()
            .filter(|s| s.star() != StarIndex::PRIMARY)
            .min_by(|a, b| a.v().value().total_cmp(&b.v().value()))
            .expect("a companion");
        assert!(
            giant.v() < primary.v(),
            "the companion outshines its primary"
        );
        // A cut between them: a bound on the primary's own light would skip the system.
        let cut = f64::midpoint(giant.v().value(), primary.v().value());
        let query = query_beside(&record, cut);
        let (_, listed_at_cut) = measured(&record, &query);
        assert_eq!(listed_at_cut.len(), 1, "{listed_at_cut:?}");
        assert_eq!(listed_at_cut[0], *giant);
        let bound = flux_bound(envelope, &record, giant.emitted()).expect("it shines");
        let giant_m_v =
            giant.v().value() - giant.a_v().value() - distance_modulus(giant.distance().value());
        assert!(
            bound.value() <= giant_m_v,
            "{} over {giant_m_v}",
            bound.value()
        );
        assert_eq!(listed.len(), every.len());
    }

    #[test]
    fn a_centre_member_is_tallied_and_not_listed() {
        let record = SystemRecord::from_parts(
            SystemId::from(CentreMemberId::CENTRAL_BLACK_HOLE),
            position([0.0, 0.0, 0.0]),
            crate::galaxy::placement::SystemOrigin::CentreMember { attempt: 0 },
            crate::galaxy::Population::NuclearDisc,
            SolarMasses::new(20.0),
            Years::new(1e7),
        );
        let query = SkyQuery::builder(observer_at([0.0, 1.0, 0.0]), Magnitudes::new(11.0))
            .build()
            .expect("a valid query");
        let mut tally = CensusTallies::default();
        let mut out = Vec::new();
        for bound in [Bound::Applied, Bound::Ignored] {
            census_record(
                milky_way_galaxy(),
                &mut context(),
                &record,
                &query,
                bound,
                &mut tally,
                &mut out,
            );
        }
        assert!(out.is_empty());
        let total: u64 = Layer::ALL
            .iter()
            .map(|&l| tally.layer(l).centre_members())
            .sum();
        assert_eq!(total, 2);
    }

    #[test]
    fn the_observers_own_system_is_absent() {
        let record = find_system(Layer::C, SUN, Clone::clone, |_, state| {
            state
                .stars()
                .iter()
                .any(|s| absolute_v_of_state(s).is_some())
        });
        let query = query_beside(&record, 11.0);
        let (every, _) = measured(&record, &query);
        assert!(!every.is_empty());
        let own = SkyQuery::builder(*query.observer(), Magnitudes::new(11.0))
            .exclude(record.id())
            .build()
            .expect("a valid query");
        let (_, listed) = measured(&record, &own);
        assert!(listed.is_empty());
    }

    #[test]
    fn a_forced_single_takes_its_own_envelope_bound() {
        let galaxy = milky_way_galaxy();
        let envelope = milky_way_envelope();
        let records = records_near(Layer::BrownDwarf, SUN, 50);
        for record in &records {
            assert_eq!(star_bound(record), 1);
            assert_eq!(star_offset_bound(galaxy, record), LightYears::ZERO);
            let t = UniverseTime::EPOCH;
            let age = record.age_at(t);
            let expected = (age.value() > 0.0)
                .then(|| {
                    envelope.brightest(
                        record.layer(),
                        record.component().expect("a grid record"),
                        record.primary_initial_mass(),
                        (age, age),
                    )
                })
                .flatten();
            assert_eq!(flux_bound(envelope, record, t), expected, "{record:?}");
        }
        // A grid system takes the bound of one star of twice its primary's mass at any age up to
        // its own (R06.T16.b), with no factor for its count, since each star is kept alone
        // (R06.T8.f).
        let star = records_near(Layer::C, SUN, 1)[0];
        let age = star.age_at(UniverseTime::EPOCH);
        let one = envelope
            .brightest(
                star.layer(),
                star.component().expect("a grid record"),
                max_star_mass(star.primary_initial_mass()),
                (Years::ZERO, age),
            )
            .expect("a C star shines");
        let bound = flux_bound(envelope, &star, UniverseTime::EPOCH).expect("it shines");
        hyperion_testkit::float::assert_same_bits(one.value(), bound.value());
    }

    /// The same cells censused twice, in reverse order and with a cold noise cache of one entry
    /// against a warm one, give the same stars and tallies, bit for bit.
    #[test]
    fn a_cells_census_is_independent_of_its_cache_and_order() {
        let galaxy = milky_way_galaxy();
        let query = SkyQuery::builder(observer_at(SUN), Magnitudes::new(9.0))
            .build()
            .expect("a valid query");
        let keys: Vec<CellKey> = [Layer::A, Layer::B, Layer::C, Layer::D]
            .iter()
            .map(|&l| CellKey::containing(l, &position(SUN)).expect("in the cube"))
            .collect();
        let run = |keys: &[CellKey], ctx: &mut SkyContext<'_>| {
            keys.iter()
                .map(|&key| {
                    let mut out = Vec::new();
                    let tally = census_cell(galaxy, ctx, key, &query, &mut out);
                    (key, out, tally)
                })
                .collect::<Vec<_>>()
        };
        let mut warm = context();
        let _ = run(&keys, &mut warm); // warms the cache; its result is the next one's
        let mut forward = run(&keys, &mut warm);
        let mut cold = context();
        cold.noise = NoiseCache::with_capacity(1);
        let reversed: Vec<CellKey> = keys.iter().rev().copied().collect();
        let mut backward = run(&reversed, &mut cold);
        backward.reverse();
        forward.sort_by_key(|(k, _, _)| *k);
        backward.sort_by_key(|(k, _, _)| *k);
        assert_eq!(forward, backward);
        assert!(forward.iter().any(|(_, out, _)| !out.is_empty()));
    }

    #[test]
    fn tallies_add_every_layers_counts() {
        let mut a = CensusTallies::default();
        a.layer_mut(Layer::C).cells = 2;
        a.layer_mut(Layer::C).accepted = 5;
        a.layer_mut(Layer::C).listed = 2;
        let mut b = CensusTallies::default();
        b.layer_mut(Layer::C).cells = 1;
        b.layer_mut(Layer::E).candidates = 7;
        b.layer_mut(Layer::E).generated = 3;
        b.layer_mut(Layer::E).without_photometry = 1;
        b.layer_mut(Layer::A).centre_members = 4;
        a.add(&b);
        let c = a.layer(Layer::C);
        assert_eq!(
            (c.cells(), c.accepted(), c.listed(), c.candidates()),
            (3, 5, 2, 0)
        );
        let e = a.layer(Layer::E);
        assert_eq!(
            (e.candidates(), e.generated(), e.without_photometry()),
            (7, 3, 1)
        );
        assert_eq!(a.layer(Layer::A).centre_members(), 4);
        assert!(a.feature_members_absent());
    }

    #[test]
    fn the_eye_offset_bound_holds_over_the_colour_table() {
        use crate::tables::star_colour::{NORMAL, WHITE_DWARF};
        let dark = SkyBackground::new(
            luminance(crate::units::MagnitudesPerArcsec2::new(30.0)),
            SpRatio::REFERENCE,
        )
        .expect("a valid background");
        let mut largest = f64::NEG_INFINITY;
        // The table is interpolated bilinearly in ρ itself, so no colour exceeds its largest row.
        for row in NORMAL.iter().chain(&WHITE_DWARF) {
            let rho = SpRatio::new(row[3]).expect("a table row's ratio is valid");
            let offset = star_colour_offset(rho, &dark).value();
            largest = largest.max(offset);
        }
        assert!(largest < EYE_OFFSET_BOUND_MAG, "{largest}");
        assert!(
            largest > 0.3,
            "the hottest rows are near Design note 5's 0.453: {largest}"
        );
    }

    /// The census of a cell, skips and all, equals every record of the cell measured with no
    /// skip: the floor and the flux bound never change an answer.
    ///
    /// Since R06.T16.b's bound, at twice the primary's mass over ages from zero, no stellar
    /// record of the cells at 0, 1 and 3 cells out is skipped, so the brown dwarfs' cells there
    /// (a forced single's bound) and a block of A's 288–320 ly out (the floor, and a multiple
    /// system's bound) are censused too, where skips remain to be checked.
    #[test]
    fn a_cells_census_equals_its_unskipped_records() {
        let galaxy = milky_way_galaxy();
        let observer = observer_at(SUN);
        let query = SkyQuery::builder(observer, Magnitudes::new(7.0))
            .eye(crate::sky::eye::EyeObserver::default())
            .build()
            .expect("a valid query");
        let mut ctx = context();
        let mut cells = Vec::new();
        for layer in [
            Layer::A,
            Layer::B,
            Layer::C,
            Layer::D,
            Layer::E,
            Layer::BrownDwarf,
        ] {
            let size = f64::from(layer.cell_size_ly());
            for step in [0.0, 1.0, 3.0] {
                let p = [SUN[0] + step * size, SUN[1], SUN[2]];
                cells.push(CellKey::containing(layer, &position(p)).expect("in the cube"));
            }
        }
        // A's cells 36–40 cells (288–320 ly) out along x, five rows of them along y.
        for (x, y) in (36..=40).flat_map(|x| (-2..=2).map(move |y| (x, y))) {
            let p = [
                SUN[0] + f64::from(x) * 8.0,
                SUN[1] + f64::from(y) * 8.0,
                SUN[2],
            ];
            cells.push(CellKey::containing(Layer::A, &position(p)).expect("in the cube"));
        }
        let (mut listed, mut skipped) = (0_usize, 0_u64);
        let mut records = Vec::new();
        for key in cells {
            let mut census = Vec::new();
            let tally = census_cell(galaxy, &mut ctx, key, &query, &mut census);
            generate_cell(galaxy, key, &mut records);
            let mut every = Vec::new();
            let mut brute = CensusTallies::default();
            for record in &records {
                census_record(
                    galaxy,
                    &mut ctx,
                    record,
                    &query,
                    Bound::Ignored,
                    &mut brute,
                    &mut every,
                );
            }
            assert_eq!(census, every, "{key:?}");
            listed += census.len();
            skipped +=
                u64::try_from(records.len()).expect("few") - tally.layer(key.layer()).generated();
        }
        assert!(
            listed > 0 && skipped > 0,
            "{listed} listed, {skipped} skipped"
        );
    }

    /// The offset table is never below the exact bound of the cell it answers for, at 10⁴ random
    /// cells of every layer from the galactic centre to the root cube's corners (R06.T8.f), and it
    /// is tight: a node is some 4% farther out than the cell's corner at most.
    #[test]
    fn the_offset_table_is_at_least_the_exact_bound() {
        let galaxy = milky_way_galaxy();
        let offsets = milky_way_offsets();
        let layers = [
            Layer::A,
            Layer::B,
            Layer::C,
            Layer::D,
            Layer::E,
            Layer::BrownDwarf,
        ];
        let mut u = crate::sky::testing::uniforms(0x000f_f5e7);
        let mut next = || u.next().expect("endless");
        let mut worst = 1.0_f64;
        for k in 0..10_000_usize {
            let layer = layers[k % layers.len()];
            // Distances even in log from a light-year to beyond the cube's faces, in any
            // direction, held inside the cube.
            let r = math::exp(math::ln(1.0) + next() * math::ln(1.2e5));
            let (cos_t, phi) = (2.0 * next() - 1.0, 2.0 * core::f64::consts::PI * next());
            let sin_t = (1.0 - cos_t * cos_t).sqrt();
            let inside = 65_000.0;
            let p = [
                (r * sin_t * math::cos(phi)).clamp(-inside, inside),
                (r * sin_t * math::sin(phi)).clamp(-inside, inside),
                (r * cos_t).clamp(-inside, inside),
            ];
            let key = CellKey::containing(layer, &position(p)).expect("in the cube");
            let exact = cell_offset_bound(galaxy, key).value();
            let table = offsets.of(key).value();
            assert!(exact <= table, "{key:?}: the table's {table} under {exact}");
            if always_single(layer) {
                hyperion_testkit::float::assert_same_bits(table, 0.0);
            } else {
                worst = worst.max(table / exact);
            }
        }
        eprintln!("the offset table is at most {worst} times the exact bound");
        assert!(worst < 1.05, "the table is {worst} times the exact bound");
    }

    /// The bound before the drift never rejects a record whose bound after its retardation
    /// passes, over some 10⁴ records of every layer seen by observers at, near and 250 ly from the
    /// Sun, at the epoch and up to 900 years from it, in a galaxy whose systems move (R06.T8.f).
    #[test]
    fn the_bound_before_the_drift_never_rejects_what_the_bound_after_it_passes() {
        let moving = Galaxy::from_params(
            milky_way_galaxy().seed(),
            crate::galaxy::params::GalaxyParams::milky_way_like(),
        )
        .expect("the fixture builds")
        .with_full_potential();
        let envelope = milky_way_envelope();
        let offsets = CellOffsets::build(&moving);
        let at = |ly: [f64; 3], years: i64| {
            let t = UniverseTime::from_julian_years(years).expect("in the window");
            Observer::new(position(ly), t).expect("an observer")
        };
        let observers = [
            at(SUN, 0),
            at([SUN[0] + 250.0, SUN[1], SUN[2]], 900),
            at([SUN[0] - 120.0, SUN[1] + 40.0, SUN[2]], -700),
        ];
        let groups = [
            (Layer::A, 1_500),
            (Layer::B, 1_000),
            (Layer::BrownDwarf, 800),
            (Layer::C, 600),
            (Layer::D, 300),
            (Layer::E, 200),
        ];
        let mut records = Vec::new();
        for (layer, n) in groups {
            records.extend(records_near(layer, SUN, n));
        }
        let (mut checked, mut rejected, mut passed) = (0_u32, 0_u32, 0_u32);
        for observer in observers {
            let query = SkyQuery::builder(observer, Magnitudes::new(7.95))
                .eye(crate::sky::eye::EyeObserver::default())
                .build()
                .expect("a valid query");
            for record in &records {
                let key = CellKey::of(record.id()).expect("a grid record");
                let reach = CellReach::of(&offsets, key, &query);
                let before = passes_before_drift(envelope, record, &query, &reach);
                let drift = Drift::of_record(&moving, record).expect("a grid record moves");
                let r = retarded(query.observer(), &drift);
                let d = query
                    .observer()
                    .position()
                    .distance_to(r.apparent_position())
                    .value()
                    / METRES_PER_LIGHT_YEAR;
                let after = flux_bound(envelope, record, r.emitted())
                    .is_some_and(|m| m.value() <= faintest_listable(&query, d - reach.offset));
                assert!(before || !after, "{record:?} for {observer:?}");
                checked += 1;
                rejected += u32::from(!before);
                passed += u32::from(after);
            }
        }
        assert!(checked >= 10_000, "{checked}");
        assert!(
            rejected > 500 && passed > 500,
            "{rejected} rejected, {passed} passed"
        );
    }

    /// Over the stars of 10⁴ generated systems of every layer, near the Sun and in the bulge, no
    /// star that the cut before its sightline drops would have passed the cut after it: `A_V` is
    /// never negative, and adding it never lowers a magnitude (R06.T8.f).
    #[test]
    fn the_sightline_cut_never_drops_a_star_the_cut_keeps() {
        let galaxy = milky_way_galaxy();
        let query = SkyQuery::builder(observer_at(SUN), Magnitudes::new(7.95))
            .eye(crate::sky::eye::EyeObserver::default())
            .build()
            .expect("a valid query");
        let mut noise = NoiseCache::with_capacity(1 << 12);
        let groups = [
            (Layer::A, SUN, 3_300),
            (Layer::B, SUN, 3_100),
            (Layer::BrownDwarf, SUN, 500),
            (Layer::C, SUN, 2_400),
            (Layer::C, BULGE, 200),
            (Layer::D, SUN, 400),
            (Layer::E, SUN, 150),
            (Layer::E, BULGE, 150),
        ];
        let (mut systems, mut dropped, mut kept) = (0_u32, 0_u32, 0_u32);
        let mut positions = Vec::new();
        for (layer, at, n) in groups {
            for record in records_near(layer, at, n) {
                let drift = Drift::of_record(galaxy, &record).expect("a grid record");
                let r = retarded(query.observer(), &drift);
                let stars = SystemStars::generate(galaxy, &record);
                let Some(state) = stars.state_at(r.emitted()) else {
                    continue;
                };
                systems += 1;
                star_positions_at(stars.hierarchy(), r.emitted(), &mut positions);
                for (body, place) in &positions {
                    let index = usize::from(body.body_index());
                    let star = &state.stars()[index];
                    let Some(m_v) = absolute_v_of_state(star) else {
                        continue;
                    };
                    let apparent = place
                        .to_galactic(r.apparent_position())
                        .expect("in the cube");
                    let at = query.observer().position();
                    let d = at.distance_to(&apparent).value() / METRES_PER_LIGHT_YEAR;
                    let before = unextinguished_v(m_v, d);
                    let limit = kept_to(&query, &colour_of_state(star));
                    let a_v = sightline(
                        galaxy.gas(),
                        &apparent,
                        at,
                        NoiseMode::Realised,
                        STAR_SIGHTLINE_QUALITY,
                        &[],
                        &mut noise,
                    )
                    .a_v()
                    .value();
                    let v = before + a_v;
                    assert!(a_v >= 0.0 && v >= before, "{record:?}: A_V {a_v}");
                    if before > limit {
                        assert!(v > limit, "{record:?}");
                        dropped += 1;
                    } else {
                        kept += u32::from(v <= limit);
                    }
                }
            }
        }
        assert!(systems >= 10_000, "{systems} systems");
        assert!(
            dropped > 1_000 && kept > 10,
            "{dropped} dropped, {kept} kept"
        );
    }
}
