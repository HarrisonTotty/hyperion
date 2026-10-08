//! One cell of the census: its bright systems, each star placed and measured at its retarded time
//! (rendering plan R06, R06.T8.b; Design note 10).
//!
//! For each record the mass skip keeps (above the cell's floor,
//! [`BrightnessEnvelope::mass_floor`], from the cell's records or its cell cache's entry,
//! [`census_cell_with_cost`]): the observer's own system is left out; its light is
//! bounded by [`flux_bound`], then star by star ([`StarBounds`], R06.T8.g), before its motion is
//! built, at its epoch position's distance less the cell's pad and offset and over the ages the
//! light's travel then allows (R06.T8.f); a member of the galactic centre, whose orbit is not
//! built ([`TraceMotionError`]), is tallied and left out before anything else is built; the
//! system is found at its retarded time ([`retarded`] on [`Drift::of_record`]); its light is
//! bounded again, both ways, at the emitted time and the apparent position; and only if both
//! bounds can pass the cut is the system generated
//! ([`SystemStars::generate`]), each star read from the pair-evolved [`SystemStars::state_at`]'s
//! stars at the emitted time, placed by [`star_positions_at`] about the system's apparent position,
//! left out if it lies outside the region of the query's cone, dimmed by its distance and, unless
//! that alone already puts it past the cut, by its own V band's extinction along one [`sightline`]
//! to the observer, and kept if its V is brighter than the cut.
//!
//! The cut alone keeps a star, with or without the eye: the eye's cut already carries the largest
//! colour offset (Design note 5), each view applies a star's own, and the band subtracts the light
//! at the cut, so the listing and the band share one boundary (decided 2026-10-06,
//! `decision-r06-t9b-band.md`; R06.T8.k). For a cone they share its region too: a star is kept
//! only if the band's texel it lies in is in the cone's region, whose centre lies within the
//! cone's half-angle plus the band's largest texel radius ([`ConeRegion::holds`], by the lookup
//! the band places its overflow's stars by), where the band is complete (decided 2026-10-07,
//! `decision-r06-t8k-cone.md`; R06.T8.l). A star's V is M<sub>V</sub> + DM
//! plus its own V band's extinction behind its sightline's A<sub>V</sub>, its colour's
//! [`reddened`](StarColour::reddened) there, as the wire carries it (the ruling's addendum, item
//! 4).
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
//! That widened bound passes nearly every record of layers C to E near the Sun, so since R06.T8.g
//! each record that passes it is bounded again star by star, before its system is generated
//! ([`StarBounds`]; decided 2026-10-05, `decision-r06-census-cost.md`, and amended 2026-10-07,
//! `decision-p11-t16-hierarchy-bound.md`): plan 11's [`hierarchy_bound`] gives every star of
//! every attempt the generator can keep, exactly, plan 11's [`pair_light_bound`] says what each
//! pair of two stars can hold, and R06.T8.m's phase envelope ([`PhaseEnvelope`]) bounds each star
//! at its own mass, \[Fe/H\], η and age relative to its lifetime. A record with a pair that plan
//! 11 cannot bound keeps the widened bound alone. Both bounds are bounds, so a record either
//! rejects is skipped: the widened one stays first, since it costs least, and for the cell's floor.
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

use crate::coords::{GalacticPosition, SystemPosition};
use crate::galaxy::consts::LIGHT_YEARS_PER_PARSEC;
use crate::galaxy::gas::extinction::{NoiseMode, Quality, sightline};
use crate::galaxy::gas::modifiers::GasModifier;
use crate::galaxy::imf::MassBand;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::{CellKey, SystemKind, SystemRecord, generate_cell_where};
use crate::galaxy::query::{pad_for, pad_speed};
use crate::galaxy::{Galaxy, PointLy};
use crate::id::{BodyId, Layer, SystemId};
use crate::math;
use crate::observe::{Drift, Retardation, TraceMotionError, retarded};
use crate::stellar::binary::{PairLight, pair_light_bound};
use crate::stellar::draws::{StandardNormal, StarDraws};
use crate::stellar::multiplicity::{
    HierarchyBound, MAX_COMPANIONS, MultiplicityContext, RedrawAttempt, StarIndex, TIDAL_CUT_SHARE,
    hierarchy_bound, star_positions_at,
};
use crate::stellar::system::{SystemStars, draw_metallicity, grid_multiplicity, primary_eta};
use crate::stellar::{Composition, Phase};
use crate::time::{CLOCK_WINDOW_H, Span, UniverseTime};
use crate::units::consts::METRES_PER_LIGHT_YEAR;
use crate::units::{LightYears, Magnitudes, Metres, SolarMasses, Years};

use super::super::colour::StarColour;
use super::super::envelope::{BrightnessEnvelope, MAX_AGE_YEARS, always_single, max_star_mass};
use super::super::phase::PhaseEnvelope;
use super::super::photometry::{absolute_v_of_state, colour_of_state};
use super::cache::{
    BlockKey, BlockParams, CACHE_APPROACH_LY, CACHE_CUT_SLACK_MAG, CellNeed, CellOutcome,
    HeldRecord, Lookup, Rebuild, serve_from_block,
};
use super::query::{SkyContext, SkyQuery};

/// The quality of each star's sightline (Design note 10).
const STAR_SIGHTLINE_QUALITY: Quality =
    Quality::Budget(core::num::NonZeroU32::new(64).expect("64 is not zero"));

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

    /// Its apparent V: M<sub>V</sub> + DM plus its own V band's extinction, its colour's
    /// [`reddened`](StarColour::reddened) at [`a_v`](Self::a_v), the V the census cuts and the
    /// wire carries (R06.T8.k; `decision-r06-t9b-band.md`, addendum item 4).
    #[must_use]
    pub const fn v(&self) -> Magnitudes {
        self.v
    }

    /// The extinction along its sightline, plan 07's A<sub>V</sub>: the law's normalisation at
    /// 0.549 µm times the dust column, which every reddening column divides by (Design note 6).
    /// Its own V band is dimmed by `colour().reddened(a_v()).v_extinction()`: for the Sun's light
    /// about 1.004 times this as A<sub>V</sub> → 0, 0.999 at A<sub>V</sub> 2 and 0.977 at 10 (the
    /// reddening tables' solar point, `tables/star_colour_reddening*.rs`;
    /// `decision-r06-t9b-band.md`, addendum item 4).
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
/// listed (Design note 11): the counts a reply carries, the same whatever cell cache the census
/// reads (R06.T8.h). How the census reached them is its [`LayerCost`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct LayerTally {
    cells: u64,
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

    /// Systems generated, their flux bound passing.
    #[must_use]
    pub const fn generated(&self) -> u64 {
        self.generated
    }

    /// Stars kept: brighter than the cut (R06.T8.k), and in the query's cone's region where it has
    /// one (R06.T8.l).
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

    const fn add(&mut self, other: &Self) {
        self.cells += other.cells;
        self.generated += other.generated;
        self.accepted += other.accepted;
        self.listed += other.listed;
        self.without_photometry += other.without_photometry;
        self.centre_members += other.centre_members;
    }
}

/// The census's tallies, per layer of [`Layer::ALL`]: what a reply states, never a count that
/// depends on the census's path (R06.T8.h), which [`CensusCost`] keeps.
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

/// What one layer's census cost: the counts that depend on how the census reached its cells,
/// through a cell cache or not, and so are no part of a reply (R06.T8.h;
/// `decision-r06-t8h-warm.md`). A warm census and a cold one give the same [`LayerTally`]; these
/// may differ.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct LayerCost {
    candidates: u64,
    held: u64,
    prefiltered: u64,
    star_bounded: u64,
    unbounded: u64,
    pairs: PairTally,
    generated_listable: u64,
    served: u64,
    missed: u64,
    rebuilt_key: u64,
    rebuilt_window: u64,
    rebuilt_parameters: u64,
}

impl LayerCost {
    /// Records the mass skip kept: of a built cell's records, or of a served cell's held ones.
    #[must_use]
    pub const fn candidates(&self) -> u64 {
        self.candidates
    }

    /// Records the cell cache's entries hold, of the cells it served or built ([`HeldRecord`]).
    #[must_use]
    pub const fn held(&self) -> u64 {
        self.held
    }

    /// Of a served cell's held records past the floor, those whose stored light the pre-filter
    /// skipped before any bound was taken.
    #[must_use]
    pub const fn prefiltered(&self) -> u64 {
        self.prefiltered
    }

    /// Records bounded star by star ([`StarBounds`], R06.T8.g) for the query: those whose widened
    /// envelope's bound before their drift passed.
    #[must_use]
    pub const fn star_bounded(&self) -> u64 {
        self.star_bounded
    }

    /// Of the records bounded star by star, those holding a pair that plan 11 cannot bound, which
    /// keep the widened envelope's bound alone ([`RecordLight::Unbounded`]).
    #[must_use]
    pub const fn unbounded_records(&self) -> u64 {
        self.unbounded
    }

    /// The pairs of the records bounded star by star, over every attempt each lists, by plan 11's
    /// verdict.
    #[must_use]
    pub const fn pairs(&self) -> &PairTally {
        &self.pairs
    }

    /// Generated systems with a star whose V with no extinction passes the cut: those that no
    /// bound of a record's own realised stars could skip, the floor of what a cache of such
    /// bounds could save (`decision-r06-t8h-warm.md`, option (b)).
    #[must_use]
    pub const fn generated_listable(&self) -> u64 {
        self.generated_listable
    }

    /// Cells served from the cell cache.
    #[must_use]
    pub const fn served(&self) -> u64 {
        self.served
    }

    /// Cells the cache held no entry for, built at their block's parameters, or a new block's.
    #[must_use]
    pub const fn missed(&self) -> u64 {
        self.missed
    }

    /// Cells rebuilt at the query's parameters for `why`, each replacing its block.
    #[must_use]
    pub const fn rebuilt(&self, why: Rebuild) -> u64 {
        match why {
            Rebuild::Key => self.rebuilt_key,
            Rebuild::Window => self.rebuilt_window,
            Rebuild::Parameters => self.rebuilt_parameters,
        }
    }

    /// Counts one cell's `outcome`.
    const fn count(&mut self, outcome: CellOutcome) {
        match outcome {
            CellOutcome::Served => self.served += 1,
            CellOutcome::Missed => self.missed += 1,
            CellOutcome::Rebuilt(Rebuild::Key) => self.rebuilt_key += 1,
            CellOutcome::Rebuilt(Rebuild::Window) => self.rebuilt_window += 1,
            CellOutcome::Rebuilt(Rebuild::Parameters) => self.rebuilt_parameters += 1,
        }
    }

    const fn add(&mut self, other: &Self) {
        self.candidates += other.candidates;
        self.held += other.held;
        self.prefiltered += other.prefiltered;
        self.star_bounded += other.star_bounded;
        self.unbounded += other.unbounded;
        self.pairs.add(&other.pairs);
        self.generated_listable += other.generated_listable;
        self.served += other.served;
        self.missed += other.missed;
        self.rebuilt_key += other.rebuilt_key;
        self.rebuilt_window += other.rebuilt_window;
        self.rebuilt_parameters += other.rebuilt_parameters;
    }
}

/// The census's cost, per layer of [`Layer::ALL`] ([`LayerCost`]): no part of a reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CensusCost {
    layers: [LayerCost; Layer::ALL.len()],
}

impl Default for CensusCost {
    fn default() -> Self {
        Self {
            layers: [LayerCost::default(); Layer::ALL.len()],
        }
    }
}

impl CensusCost {
    /// `layer`'s cost.
    #[must_use]
    pub fn layer(&self, layer: Layer) -> &LayerCost {
        &self.layers[usize::from(layer.value())]
    }

    fn layer_mut(&mut self, layer: Layer) -> &mut LayerCost {
        &mut self.layers[usize::from(layer.value())]
    }

    /// Adds `other`'s counts to these.
    pub fn add(&mut self, other: &Self) {
        for (a, b) in self.layers.iter_mut().zip(&other.layers) {
            a.add(b);
        }
    }
}

/// A census's tallies and its cost, counted side by side.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
struct Counts {
    tallies: CensusTallies,
    cost: CensusCost,
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
/// The census adds its own V extinction to this ([`own_v_extinction`]), which is never negative,
/// so a star this alone puts past the cut stays past it.
#[must_use]
fn unextinguished_v(m_v: Magnitudes, d_ly: f64) -> f64 {
    m_v.value() + distance_modulus(d_ly.max(1e-6))
}

/// The extinction of a star of `colour`'s own V band behind a sightline of `a_v`, mag: v★(A) A,
/// the [`v_extinction`](crate::sky::colour::Reddened::v_extinction) of its
/// [`reddened`](StarColour::reddened) (R06.T8.k; `decision-r06-t9b-band.md`, addendum item 4),
/// zero bit for bit with no dust. Every band's transmission falls with A<sub>V</sub> from one on
/// every row of the colour table (R06.T9.e's tests), so it is never negative, which keeps the cut
/// before the sightline and the flux bound exact.
#[must_use]
fn own_v_extinction(colour: &StarColour, a_v: Magnitudes) -> f64 {
    let extinction = colour.reddened(a_v).v_extinction().value();
    debug_assert!(
        extinction >= 0.0,
        "a V extinction of {extinction} mag behind A_V {}",
        a_v.value()
    );
    extinction
}

/// The faintest absolute V a system's brightest possible star could have and still be listed at
/// `d_ly` light-years with no extinction, mag: the cut less the distance modulus. The eye adds
/// nothing: the census keeps each star to the cut alone (R06.T8.k).
#[must_use]
fn faintest_listable(query: &SkyQuery, d_ly: f64) -> f64 {
    query.cut().value() - distance_modulus(d_ly.max(1e-6))
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

/// What a star's pair leaves of the star's own bound (R06.T8.g): plan 11's
/// [`pair_light_bound`] verdict, read for one of the pair's two stars
/// (`decision-p11-t16-hierarchy-bound.md` §5).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StarLight {
    /// The star is its own single-star model and takes its own bound: a star of no pair the
    /// binary engine may run, or of a [`PairLight::Detached`] or [`PairLight::Unchanged`] pair.
    Own,
    /// The brighter of the star's own bound and this absolute V, margin included: a star of a
    /// [`PairLight::Bright`] pair, which may depart from its own model or leave a product of the
    /// pair in its place.
    OwnOr(Magnitudes),
    /// Nothing: a star of a [`PairLight::Remnants`] pair, none of whose stars or products lives in
    /// the window, while white dwarfs are dark in V (ask A4).
    Dark,
}

impl StarLight {
    /// The light each star of a pair answered `verdict` takes.
    #[must_use]
    pub(crate) const fn of(verdict: PairLight) -> Self {
        match verdict {
            PairLight::Detached | PairLight::Unchanged => Self::Own,
            PairLight::Remnants => Self::Dark,
            PairLight::Bright(m) => Self::OwnOr(m),
        }
    }

    /// The light of a star that takes `self` at one attempt and `other` at another: the least
    /// light that bounds both.
    #[must_use]
    const fn either(self, other: Self) -> Self {
        match (self, other) {
            (Self::Dark, light) | (light, Self::Dark) => light,
            (Self::Own, Self::Own) => Self::Own,
            (Self::Own, Self::OwnOr(m)) | (Self::OwnOr(m), Self::Own) => Self::OwnOr(m),
            (Self::OwnOr(a), Self::OwnOr(b)) => Self::OwnOr(brighter(a, b)),
        }
    }
}

/// The brighter of two magnitudes, `a` where they tie.
#[must_use]
const fn brighter(a: Magnitudes, b: Magnitudes) -> Magnitudes {
    if b.value() < a.value() { b } else { a }
}

/// One star of a [`StarBounds`]: the redraw attempt it was drawn at, its index there, its initial
/// mass and its Reimers η draw, each the generator's bit for bit, and the light its pair leaves it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundStar {
    attempt: RedrawAttempt,
    index: StarIndex,
    mass: SolarMasses,
    eta: StandardNormal,
    light: StarLight,
}

impl BoundStar {
    /// The attempt the star was drawn at: [`RedrawAttempt::FIRST`] for the primary, which is the
    /// same star at every attempt.
    #[must_use]
    pub const fn attempt(&self) -> RedrawAttempt {
        self.attempt
    }

    /// Its index in its attempt's hierarchy, which is also its body index.
    #[must_use]
    pub const fn index(&self) -> StarIndex {
        self.index
    }

    /// Its initial mass, as [`hierarchy_bound`] lists it.
    #[must_use]
    pub const fn mass(&self) -> SolarMasses {
        self.mass
    }

    /// Its η draw: its body's [`StarDraws`] at its attempt, the primary's own draws at attempt 0
    /// (the generator's `primary_eta`).
    #[must_use]
    pub const fn eta(&self) -> StandardNormal {
        self.eta
    }

    /// The light its pair leaves it; for the primary, the least that bounds it at every attempt.
    #[must_use]
    pub const fn light(&self) -> StarLight {
        self.light
    }

    /// The brightest absolute V the star can have at an age within `ages` (years, inclusive),
    /// margin included, of `composition`, its system's: its own phase-envelope bound
    /// ([`PhaseEnvelope::brightest`]), the brighter of that and its pair's, or `None` where it can
    /// shine at none of them. A star of a [`StarLight::OwnOr`] pair takes the pair's magnitude
    /// even where its own model is dark: a departing star can outlive its own model.
    #[must_use]
    pub fn brightest(
        &self,
        phase: &PhaseEnvelope,
        composition: &Composition,
        ages: (Years, Years),
    ) -> Option<Magnitudes> {
        let own = || phase.brightest(self.mass, composition, self.eta, ages);
        match self.light {
            StarLight::Own => own(),
            StarLight::OwnOr(m) => Some(own().map_or(m, |own| brighter(own, m))),
            StarLight::Dark => None,
        }
    }
}

/// Pairs bounded star by star, by plan 11's verdict (R06.T8.g's record of the shares).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct PairTally {
    detached: u64,
    unchanged: u64,
    remnants: u64,
    bright: u64,
    unbounded: u64,
}

impl PairTally {
    /// Pairs answered [`PairLight::Detached`].
    #[must_use]
    pub const fn detached(&self) -> u64 {
        self.detached
    }

    /// Pairs answered [`PairLight::Unchanged`].
    #[must_use]
    pub const fn unchanged(&self) -> u64 {
        self.unchanged
    }

    /// Pairs answered [`PairLight::Remnants`].
    #[must_use]
    pub const fn remnants(&self) -> u64 {
        self.remnants
    }

    /// Pairs answered [`PairLight::Bright`].
    #[must_use]
    pub const fn bright(&self) -> u64 {
        self.bright
    }

    /// Pairs that plan 11 cannot bound, which leave their records to the widened envelope.
    #[must_use]
    pub const fn unbounded(&self) -> u64 {
        self.unbounded
    }

    /// Every pair counted.
    #[must_use]
    pub const fn total(&self) -> u64 {
        self.detached + self.unchanged + self.remnants + self.bright + self.unbounded
    }

    /// Adds `other`'s counts to these.
    pub(crate) const fn add(&mut self, other: &Self) {
        self.detached += other.detached;
        self.unchanged += other.unchanged;
        self.remnants += other.remnants;
        self.bright += other.bright;
        self.unbounded += other.unbounded;
    }

    /// Counts one pair answered `verdict`.
    const fn count(&mut self, verdict: Option<PairLight>) {
        match verdict {
            Some(PairLight::Detached) => self.detached += 1,
            Some(PairLight::Unchanged) => self.unchanged += 1,
            Some(PairLight::Remnants) => self.remnants += 1,
            Some(PairLight::Bright(_)) => self.bright += 1,
            None => self.unbounded += 1,
        }
    }
}

/// What a record's stars, bounded one by one, can hold at some ages ([`StarBounds::brightest`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RecordLight {
    /// The record holds a pair that plan 11 cannot bound, so its stars have no bound of their own
    /// and the record keeps the widened envelope's ([`flux_bound`]) alone.
    Unbounded,
    /// No star of the system can shine in V then.
    Dark,
    /// The brightest absolute V any star of the system can have then, margin included.
    Brightest(Magnitudes),
}

impl RecordLight {
    /// Whether a star of this light could be listed where `faintest_listable` is the faintest
    /// absolute V listable: always for an unbounded record, which the widened envelope alone
    /// bounds, and never for a dark one.
    #[must_use]
    pub fn may_list(self, faintest_listable: Magnitudes) -> bool {
        match self {
            Self::Unbounded => true,
            Self::Dark => false,
            Self::Brightest(m) => m.value() <= faintest_listable.value(),
        }
    }
}

/// A record's light bounded star by star, before its system is generated (R06.T8.g; Design note
/// 10; decided 2026-10-05, `decision-r06-census-cost.md`, and amended 2026-10-07,
/// `decision-p11-t16-hierarchy-bound.md`).
///
/// It is built in the order the plan fixes:
/// 1. the record's composition ([`draw_metallicity`]);
/// 2. plan 11's [`hierarchy_bound`], the generator's own draw at every redraw attempt it can keep,
///    so each star's initial mass is exact, with its body and attempt, and so is each star–star
///    pair's drawn periastron;
/// 3. each such pair's [`pair_light_bound`] over the record's light-time ages;
/// 4. for each star, R06.T8.m's phase envelope ([`PhaseEnvelope::brightest`]) at its own mass,
///    \[Fe/H\], η and age relative to its lifetime, read by [`brightest`](Self::brightest).
///
/// A star of a [`Detached`](PairLight::Detached) or [`Unchanged`](PairLight::Unchanged) pair, or
/// of no pair the engine may run, takes its own bound; a [`Remnants`](PairLight::Remnants) pair
/// gives nothing while white dwarfs are dark (ask A4); a [`Bright`](PairLight::Bright) pair bounds
/// each of its stars by the brighter of its own bound and the pair's magnitude, which also bounds
/// the pair's products ([`StarLight`]). A pair that plan 11 cannot bound leaves the record
/// [`Unbounded`](RecordLight::Unbounded): the widened envelope at `max_star_mass` alone bounds
/// it. The primary is the same star at every attempt and is bounded once, by the least light that
/// bounds it at all of them, and by its own bound where the generator may keep it alone after its
/// last attempt ([`HierarchyBound::fallback`]).
///
/// Each η is its body's [`StarDraws`] at its attempt, and the primary's its record's own (the
/// generator's `primary_eta`). The bound reads the generator's existing words only, through the
/// generator's own functions, so nothing generated moves.
#[derive(Debug, Clone, PartialEq)]
pub struct StarBounds {
    composition: Composition,
    /// The light-time ages, years, inclusive, the pairs' verdicts hold over.
    ages: (Years, Years),
    primary: BoundStar,
    companions: Vec<BoundStar>,
    pairs: PairTally,
}

impl StarBounds {
    /// The bounds of the stars of `record`'s system in `galaxy`, its pairs' verdicts taken over
    /// the window of its light-time ages `ages` (years, inclusive): steps 1 to 3 of the order (see
    /// the [type](Self) documentation), each star's own bound left for
    /// [`brightest`](Self::brightest) to read at any ages within the window.
    ///
    /// # Panics
    ///
    /// As [`draw_metallicity`] and [`hierarchy_bound`] do, for a record of another galaxy.
    ///
    /// # Examples
    ///
    /// The census bounds a record's stars before it generates the system, and the generated
    /// stars lie within the bound:
    ///
    /// ```
    /// use hyperion_sim::Seed;
    /// use hyperion_sim::galaxy::Galaxy;
    /// use hyperion_sim::galaxy::placement::{CellKey, generate_cell};
    /// use hyperion_sim::id::Layer;
    /// use hyperion_sim::sky::census::{RecordLight, StarBounds};
    /// use hyperion_sim::sky::phase::PhaseEnvelope;
    /// use hyperion_sim::sky::photometry::absolute_v_of_state;
    /// use hyperion_sim::stellar::system::SystemStars;
    /// use hyperion_sim::time::UniverseTime;
    ///
    /// let galaxy = Galaxy::new(Seed::new(11));
    /// let mut cell = Vec::new();
    /// generate_cell(&galaxy, CellKey::new(Layer::D, [0, 406, 0])?, &mut cell);
    /// let record = cell.first().ok_or("the cell has systems")?;
    /// let age = record.age_at(UniverseTime::EPOCH);
    /// let bounds = StarBounds::of(&galaxy, record, (age, age));
    /// let light = bounds.brightest(PhaseEnvelope::shared(), (age, age));
    /// let state = SystemStars::generate(&galaxy, record)
    ///     .state_at(UniverseTime::EPOCH)
    ///     .ok_or("born")?;
    /// for m_v in state.stars().iter().filter_map(absolute_v_of_state) {
    ///     match light {
    ///         // A pair plan 11 cannot bound: the widened envelope bounds the record instead.
    ///         RecordLight::Unbounded => {}
    ///         RecordLight::Brightest(bound) => assert!(m_v >= bound),
    ///         RecordLight::Dark => panic!("a star shines where its bound says none can"),
    ///     }
    /// }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn of(galaxy: &Galaxy, record: &SystemRecord, ages: (Years, Years)) -> Self {
        let composition = draw_metallicity(galaxy, record);
        let hierarchy = hierarchy_bound(galaxy, record, &composition);
        Self::from_hierarchy(galaxy, record, composition, &hierarchy, ages)
    }

    /// [`of`](Self::of) from the record's `composition` and its `hierarchy`, steps 1 and 2 taken
    /// already: each pair's verdict over `ages`, and each star's η.
    ///
    /// They must be `record`'s own, [`draw_metallicity`]'s and [`hierarchy_bound`]'s, or the bound
    /// is not its system's: the masses are read from `hierarchy` and the primary's η from
    /// `record`. Debug builds check that the hierarchy's primary is the record's.
    #[must_use]
    pub fn from_hierarchy(
        galaxy: &Galaxy,
        record: &SystemRecord,
        composition: Composition,
        hierarchy: &HierarchyBound,
        ages: (Years, Years),
    ) -> Self {
        Self::with_verdicts(
            galaxy,
            record,
            (composition, ages),
            hierarchy,
            |a, b, periastron| pair_light_bound(a, b, periastron, &composition, ages.0..=ages.1),
        )
    }

    /// [`from_hierarchy`](Self::from_hierarchy) over `ages`, of `composition`, with each pair's
    /// verdict from `verdict`, given the pair's two initial masses and its drawn periastron: plan
    /// 11's for the census, any for the tests of the verdicts that P11.T17.c brings.
    #[must_use]
    fn with_verdicts(
        galaxy: &Galaxy,
        record: &SystemRecord,
        (composition, ages): (Composition, (Years, Years)),
        hierarchy: &HierarchyBound,
        mut verdict: impl FnMut(SolarMasses, SolarMasses, Metres) -> Option<PairLight>,
    ) -> Self {
        debug_assert_eq!(
            hierarchy.primary().body(),
            BodyId::new(record.id(), 0),
            "another record's hierarchy"
        );
        let seed = galaxy.seed();
        let mut pairs = PairTally::default();
        // The generator keeps the primary alone after a last attempt that carves.
        let mut primary_light = if hierarchy.fallback().is_some() {
            StarLight::Own
        } else {
            StarLight::Dark
        };
        let mut companions = Vec::with_capacity(
            hierarchy
                .attempts()
                .iter()
                .map(|listed| listed.stars().len().saturating_sub(1))
                .sum(),
        );
        let mut each = [StarLight::Own; MAX_COMPANIONS + 2];
        for listed in hierarchy.attempts() {
            let stars = listed.stars();
            let lights = each
                .get_mut(..stars.len())
                .expect("a hierarchy holds at most GRID_STAR_BOUND stars");
            lights.fill(StarLight::Own);
            for pair in listed.pairs() {
                let [a, b] = pair.stars().map(|s| usize::from(s.get()));
                let answer = verdict(
                    stars[a].initial_mass(),
                    stars[b].initial_mass(),
                    pair.periastron(),
                );
                pairs.count(answer);
                if let Some(answer) = answer {
                    let light = StarLight::of(answer);
                    lights[a] = light;
                    lights[b] = light;
                }
            }
            primary_light = primary_light.either(lights[0]);
            let attempt = listed.attempt();
            companions.extend(stars.iter().zip(&*lights).skip(1).map(|(slot, &light)| {
                BoundStar {
                    attempt,
                    index: u8::try_from(slot.body().body_index())
                        .ok()
                        .and_then(StarIndex::from_body)
                        .expect("a hierarchy's bodies are its stars, at most five"),
                    mass: slot.initial_mass(),
                    eta: StarDraws::eta_for_attempt(seed, slot.body(), u32::from(attempt.get())),
                    light,
                }
            }));
        }
        Self {
            composition,
            ages,
            primary: BoundStar {
                attempt: RedrawAttempt::FIRST,
                index: StarIndex::PRIMARY,
                mass: hierarchy.primary().initial_mass(),
                eta: primary_eta(galaxy, record),
                light: primary_light,
            },
            companions,
            pairs,
        }
    }

    /// The system's composition, every star's.
    #[must_use]
    pub const fn composition(&self) -> &Composition {
        &self.composition
    }

    /// The light-time ages, years, inclusive, over which the pairs' verdicts hold, and within
    /// which [`brightest`](Self::brightest) reads.
    #[must_use]
    pub const fn ages(&self) -> (Years, Years) {
        self.ages
    }

    /// The primary, bounded once for every attempt.
    #[must_use]
    pub const fn primary(&self) -> &BoundStar {
        &self.primary
    }

    /// Every companion of every attempt listed, attempt by attempt, each by its index.
    #[must_use]
    pub fn companions(&self) -> &[BoundStar] {
        &self.companions
    }

    /// Star `index` of attempt `attempt` as bounded: the primary at any attempt, a companion only
    /// at an attempt listed. For the tests, which hold each realised star to its own bound.
    #[cfg(test)]
    #[must_use]
    fn star(&self, attempt: RedrawAttempt, index: StarIndex) -> Option<&BoundStar> {
        if index == StarIndex::PRIMARY {
            return Some(&self.primary);
        }
        self.companions
            .iter()
            .find(|s| s.attempt == attempt && s.index == index)
    }

    /// The record's pairs, over every attempt listed, by plan 11's verdict.
    #[must_use]
    pub const fn pairs(&self) -> &PairTally {
        &self.pairs
    }

    /// What the system's stars can hold at an age within `ages` (years, inclusive): step 4 of the
    /// order, each star's [`BoundStar::brightest`] read from `phase` and the brightest kept. It
    /// allocates nothing.
    ///
    /// The pairs' verdicts hold within [`ages`](Self::ages) only, so `ages` must lie within it;
    /// debug builds check it.
    #[must_use]
    pub fn brightest(&self, phase: &PhaseEnvelope, ages: (Years, Years)) -> RecordLight {
        debug_assert!(
            self.ages.0 <= ages.0 && ages.1 <= self.ages.1,
            "ages {ages:?} outside the window {:?} the pairs were bounded over",
            self.ages
        );
        if self.pairs.unbounded > 0 {
            return RecordLight::Unbounded;
        }
        core::iter::once(&self.primary)
            .chain(&self.companions)
            .filter_map(|star| star.brightest(phase, &self.composition, ages))
            .reduce(brighter)
            .map_or(RecordLight::Dark, RecordLight::Brightest)
    }
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
    /// The box's nearest and farthest points from the observer, ly.
    near_ly: f64,
    far_ly: f64,
}

/// The nearest and farthest points of `key`'s box from `apex`, ly.
#[must_use]
fn box_distances_ly(key: CellKey, apex: [f64; 3]) -> (f64, f64) {
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
    (near_sq.sqrt(), far_sq.sqrt())
}

/// β of `layer`'s pad: the light-years its records can move in a year of light, at its
/// [`pad_speed`].
#[must_use]
fn pad_beta(layer: Layer) -> f64 {
    let year = UniverseTime::EPOCH
        .checked_add(Span::from_julian_years(1).expect("a year is a span"))
        .expect("a year after the epoch is a time");
    pad_for(year, pad_speed(layer)).value()
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
    /// 3,000 km/s. P08.T17 owns that premise: plan 08's
    /// [`epoch_velocity`](crate::galaxy::query::epoch_velocity), through which
    /// [`Drift::of_record`] reads every grid velocity, debug-asserts each speed below its layer's
    /// [`pad_speed`] (decided 2026-10-05, `decision-r06-pad-speed.md`), and R06.T8.j tests this pad
    /// and the plan's cells in a galaxy whose systems move.
    #[must_use]
    fn of(offsets: &CellOffsets, key: CellKey, query: &SkyQuery) -> Self {
        let apex = query.observer().position().to_light_years_f64();
        let (near_ly, far_ly) = box_distances_ly(key, apex);
        let t = query.observer().time();
        let beta = pad_beta(key.layer());
        let offset = offsets.of(key).value();
        let lead = t.since_epoch().as_julian_years_f64().abs();
        let pad = beta * (lead + far_ly + offset) / (1.0 - beta);
        Self {
            least: (near_ly - pad - offset).max(0.0),
            pad,
            offset,
            near_ly,
            far_ly,
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

/// The years by which a cell cache's windows ([`CellNeed`]) widen the light's ages beyond the
/// pad, an entry's and a query's alike (R06.T8.h): twice [`BEFORE_DRIFT_SLACK_YEARS`], so that
/// each record's ages before its drift lie inside the query's window with a year to spare against
/// the rounding of distances and ages, and so inside every entry's window that holds it.
const WINDOW_SLACK_YEARS: f64 = 2.0 * BEFORE_DRIFT_SLACK_YEARS;

/// The share of a box's nearest distance by which a cell cache's keys ([`CellNeed`]) bring it
/// nearer (R06.T8.h): twice [`BEFORE_DRIFT_SLACK_SHARE`], so that a query's key is never brighter
/// than the magnitude its records' bounds before the drift read, which bring each system nearer
/// by that share once.
const KEY_SLACK_SHARE: f64 = 2.0 * BEFORE_DRIFT_SLACK_SHARE;

/// What an entry built for `params` holds of `key` (R06.T8.h; `decision-r06-t8h-warm.md`
/// §2.1), for every observer within [`CACHE_APPROACH_LY`] of the builder's at any time within
/// ±H, at a cut at most [`CACHE_CUT_SLACK_MAG`] fainter:
///
/// - its **key**, the faintest absolute V listable at the box's least distance from any such
///   observer: the cut plus the slack, less the distance modulus of the box's nearest distance
///   less the approach, the largest pad such an observer can give the cell
///   (β (H + far + approach + offset) ÷ (1 − β), as [`CellReach::of`]'s at the farthest such
///   observer and time) and the offset;
/// - its **window**, every emitted time such an observer can receive, Julian years from the
///   epoch: from −H less the farthest light time (the box's farthest distance plus the approach,
///   the pad and the offset) to +H less the least, each widened by [`WINDOW_SLACK_YEARS`].
///
/// A query is served exactly when its own [`query_need`] lies within it ([`CellNeed::holds`]):
/// the approach and the slack only make that likely after a jump, and never decide it.
#[must_use]
pub(crate) fn entry_need(offsets: &CellOffsets, key: CellKey, params: &BlockParams) -> CellNeed {
    let (near, far) = box_distances_ly(key, params.at().to_light_years_f64());
    let h = CLOCK_WINDOW_H.as_julian_years_f64();
    let beta = pad_beta(key.layer());
    let offset = offsets.of(key).value();
    let pad = beta * (h + far + CACHE_APPROACH_LY + offset) / (1.0 - beta);
    let least = (near * (1.0 - KEY_SLACK_SHARE) - CACHE_APPROACH_LY - pad - offset).max(0.0);
    let cut = params.cut().value() + CACHE_CUT_SLACK_MAG;
    CellNeed::new(
        Magnitudes::new(cut - distance_modulus(least.max(1e-6))),
        (
            -h - (far + CACHE_APPROACH_LY + pad + offset) - WINDOW_SLACK_YEARS,
            h - least + WINDOW_SLACK_YEARS,
        ),
    )
}

/// What `query` needs of `key` (R06.T8.h): the faintest absolute V listable at the box's least
/// distance (its nearest point, brought nearer by [`KEY_SLACK_SHARE`], less the pad and the
/// offset), and every emitted time its records' light can have left them, Julian years from the
/// epoch: from the query's time less the farthest light time to its time less the least, each
/// widened by [`WINDOW_SLACK_YEARS`]. Every record's ages before its drift ([`BeforeDrift`]) lie
/// within the window, and the magnitude its bounds there read is no fainter than the key. The
/// census reads [`need_at`], from the reach it has already; the tests read this.
#[cfg(test)]
#[must_use]
pub(crate) fn query_need(offsets: &CellOffsets, key: CellKey, query: &SkyQuery) -> CellNeed {
    need_at(&CellReach::of(offsets, key, query), query)
}

/// [`query_need`] at the cell's `reach`.
#[must_use]
fn need_at(reach: &CellReach, query: &SkyQuery) -> CellNeed {
    let t = query.observer().time().since_epoch().as_julian_years_f64();
    let least = (reach.near_ly * (1.0 - KEY_SLACK_SHARE) - reach.pad - reach.offset).max(0.0);
    CellNeed::new(
        Magnitudes::new(faintest_listable(query, least)),
        (
            t - (reach.far_ly + reach.pad + reach.offset) - WINDOW_SLACK_YEARS,
            t - least + WINDOW_SLACK_YEARS,
        ),
    )
}

/// What the bounds before a record's drift read (R06.T8.f's step 5): the system's ages across
/// every light time its epoch distance allows, years, inclusive, and that distance less the
/// cell's pad and offset, ly.
#[derive(Debug, Clone, Copy, PartialEq)]
struct BeforeDrift {
    ages: (Years, Years),
    nearest_ly: f64,
}

impl BeforeDrift {
    /// Where and when `record`'s light can have left it, for `query` from a cell of `reach`.
    ///
    /// A record that fails a bound here would fail it after its retardation too, where
    /// [`census_record`] tests the bound at the emitted time and the apparent position: its pad
    /// bounds how far the record moves by the emitted time and by the retardation's first guess,
    /// so its light's age lies within the pad of its epoch distance and its apparent position is
    /// no nearer than that distance less the pad.
    #[must_use]
    fn of(record: &SystemRecord, query: &SkyQuery, reach: &CellReach) -> Self {
        let observer = query.observer();
        let d_epoch = observer
            .position()
            .distance_to(record.epoch_position())
            .value()
            / METRES_PER_LIGHT_YEAR;
        // A light-year is a Julian year of light, so the light's age in years is its distance in
        // ly.
        let now = record.age_at(observer.time()).value();
        Self {
            ages: (
                Years::new(now - (d_epoch + reach.pad) - BEFORE_DRIFT_SLACK_YEARS),
                Years::new(now - (d_epoch - reach.pad).max(0.0) + BEFORE_DRIFT_SLACK_YEARS),
            ),
            nearest_ly: d_epoch * (1.0 - BEFORE_DRIFT_SLACK_SHARE) - reach.pad - reach.offset,
        }
    }

    /// Whether the widened envelope's bound ([`flux_bound_over`]) can pass the cut here.
    #[must_use]
    fn passes(
        self,
        envelope: &BrightnessEnvelope,
        record: &SystemRecord,
        query: &SkyQuery,
    ) -> bool {
        flux_bound_over(envelope, record, self.ages)
            .is_some_and(|m| m.value() <= faintest_listable(query, self.nearest_ly))
    }
}

/// Whether `record`'s flux bound can pass the cut before its drift is built (R06.T8.f): the bound
/// over the system's ages across every light time its distance allows, at its epoch position's
/// distance less the cell's pad and offset. The tests' wrapper of [`BeforeDrift::passes`], which
/// the census asks first in [`star_bounds_before_drift`].
#[cfg(test)]
#[must_use]
fn passes_before_drift(
    envelope: &BrightnessEnvelope,
    record: &SystemRecord,
    query: &SkyQuery,
    reach: &CellReach,
) -> bool {
    BeforeDrift::of(record, query, reach).passes(envelope, record, query)
}

/// `record`'s stars bounded one by one before its drift is built (R06.T8.g), if one of them can
/// be listed; `None` if none can, or if the widened envelope's bound before the drift
/// ([`BeforeDrift::passes`]), which is asked first and costs least, cannot pass.
///
/// The stars are bounded over the light-time ages and at the distance of [`BeforeDrift`], so each
/// pair's verdict holds at every age the bound after the drift can read, and a record rejected
/// here would be rejected there. The records bounded and their pairs' verdicts are counted in
/// `cost`. The record's composition and hierarchy come from `drawn`, drawn there if not yet.
fn star_bounds_before_drift(
    galaxy: &Galaxy,
    envelope: &BrightnessEnvelope,
    record: &SystemRecord,
    query: &SkyQuery,
    (reach, drawn): (&CellReach, &mut Drawn),
    cost: &mut LayerCost,
) -> Option<StarBounds> {
    let before = BeforeDrift::of(record, query, reach);
    if !before.passes(envelope, record, query) {
        return None;
    }
    let bounds = drawn.star_bounds(galaxy, record, before.ages);
    cost.star_bounded += 1;
    cost.pairs.add(bounds.pairs());
    let light = bounds.brightest(PhaseEnvelope::shared(), before.ages);
    if light == RecordLight::Unbounded {
        cost.unbounded += 1;
    }
    light
        .may_list(Magnitudes::new(faintest_listable(query, before.nearest_ly)))
        .then_some(bounds)
}

/// A record's composition and hierarchy bound ([`draw_metallicity`], [`hierarchy_bound`]), drawn
/// at most once for every [`StarBounds`] a census takes of it (R06.T8.h): a cell built for the
/// cell cache bounds a record over its entry's window and over the query's ages before the drift
/// from one hierarchy. Each bound is [`StarBounds::of`]'s bit for bit.
#[derive(Debug, Default)]
struct Drawn(Option<(Composition, HierarchyBound)>);

impl Drawn {
    /// The record's [`StarBounds`] over `ages`, drawing its composition and hierarchy first if
    /// they are not yet drawn. `record` must be the same at every call.
    fn star_bounds(
        &mut self,
        galaxy: &Galaxy,
        record: &SystemRecord,
        ages: (Years, Years),
    ) -> StarBounds {
        let (composition, hierarchy) = self.0.get_or_insert_with(|| {
            let composition = draw_metallicity(galaxy, record);
            let hierarchy = hierarchy_bound(galaxy, record, &composition);
            (composition, hierarchy)
        });
        StarBounds::from_hierarchy(galaxy, record, *composition, hierarchy, ages)
    }
}

/// The light an entry for `need` keeps beside `record` (R06.T8.h; `decision-r06-t8h-warm.md`
/// §2.1), or `None` if no census its entry serves could generate the system:
///
/// - a rogue planet is never held, since the census lists no star of one;
/// - a record no density component placed is held [`RecordLight::Unbounded`], since the census
///   generates it unbounded;
/// - any other is held if its widened envelope's bound over the window's ages ([`flux_bound_over`])
///   and then its light star by star over them ([`StarBounds`], its hierarchy from `drawn`) can be
///   listed at the key, with that light.
///
/// Every bound only loosens as its ages widen, so a record the census of a query the entry serves
/// would generate, whose ages before the drift the window holds and whose magnitude there the key
/// bounds, is held, and its light passes the pre-filter.
fn held_light(
    galaxy: &Galaxy,
    envelope: &BrightnessEnvelope,
    record: &SystemRecord,
    need: &CellNeed,
    drawn: &mut Drawn,
) -> Option<RecordLight> {
    if record.kind() == SystemKind::RoguePlanet {
        return None;
    }
    if record.component().is_none() {
        return Some(RecordLight::Unbounded);
    }
    let ages = need.ages_of(record);
    let key = need.key();
    if !flux_bound_over(envelope, record, ages).is_some_and(|m| m.value() <= key.value()) {
        return None;
    }
    let light = drawn
        .star_bounds(galaxy, record, ages)
        .brightest(PhaseEnvelope::shared(), ages);
    light.may_list(key).then_some(light)
}

/// Whether `record`'s light can pass the cut after its retardation, at the emitted time
/// `emitted`, where `faintest` is the faintest absolute V listable: the widened envelope's bound
/// at the system's age then, and its stars' `bounds` (R06.T8.g), whose pairs' verdicts were taken
/// over the ages before the drift, which hold that age.
#[must_use]
fn passes_after_drift(
    envelope: &BrightnessEnvelope,
    record: &SystemRecord,
    bounds: &StarBounds,
    emitted: UniverseTime,
    faintest: Magnitudes,
) -> bool {
    if !flux_bound(envelope, record, emitted).is_some_and(|m| m.value() <= faintest.value()) {
        return false;
    }
    let age = record.age_at(emitted);
    let (young, old) = bounds.ages();
    let within = young <= age && age <= old;
    debug_assert!(
        within,
        "{record:?}: its age {age:?} at the emitted time lies outside the ages ({young:?}, \
         {old:?}) its pairs were bounded over"
    );
    // Outside that window the pairs' verdicts say nothing, so the record is generated.
    !within
        || bounds
            .brightest(PhaseEnvelope::shared(), (age, age))
            .may_list(faintest)
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
    floor_for(
        galaxy,
        envelope,
        key.layer(),
        Magnitudes::new(faintest_listable(query, reach.least)),
    )
}

/// The mass floor of `layer` where `faintest` is the faintest absolute V listable: [`floor_at`]'s
/// at a magnitude given, as a cell cache's entry reads it at its key (R06.T8.h).
#[must_use]
fn floor_for(
    galaxy: &Galaxy,
    envelope: &BrightnessEnvelope,
    layer: Layer,
    faintest: Magnitudes,
) -> SolarMasses {
    let component = galaxy
        .fields()
        .component_ids()
        .next()
        .expect("a galaxy has components");
    envelope.mass_floor(
        layer,
        component,
        faintest,
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
    let mut counts = Counts {
        tallies: *tally,
        cost: CensusCost::default(),
    };
    record_stars(
        galaxy,
        ctx,
        record,
        query,
        (bound, reach.as_ref(), &mut Drawn::default()),
        &mut counts,
        out,
    );
    *tally = counts.tallies;
}

/// `record` at its retarded time for `query`, if its light can pass the cut: `None` if a bound
/// rejects it, or if it is a member of the galactic centre, whose orbit is not built, which is
/// counted in `tally`.
///
/// Given its cell's `reach`, and if a density component placed it, its light is bounded before
/// its drift (R06.T8.f), by the widened envelope and then star by star (R06.T8.g), its
/// composition and hierarchy from `drawn`, and again after its retardation, at the emitted time
/// and the apparent position. A record no density component placed (a feature member's, once
/// T16.a brings them) has no envelope bound here and is always generated, so the census and the
/// brute force agree. The bounds' counts go to `cost`.
fn bright_retarded(
    galaxy: &Galaxy,
    envelope: &BrightnessEnvelope,
    record: &SystemRecord,
    query: &SkyQuery,
    (reach, drawn): (Option<&CellReach>, &mut Drawn),
    tally: &mut LayerTally,
    cost: &mut LayerCost,
) -> Option<Retardation> {
    let reach = reach.filter(|_| record.component().is_some());
    let star_bounds = match reach {
        Some(reach) => Some(star_bounds_before_drift(
            galaxy,
            envelope,
            record,
            query,
            (reach, drawn),
            cost,
        )?),
        None => None,
    };
    let drift = match Drift::of_record(galaxy, record) {
        Ok(drift) => drift,
        Err(TraceMotionError::CentreOrbitNotBuilt(_)) => {
            tally.centre_members += 1;
            return None;
        }
    };
    let observer = query.observer();
    let r = retarded(observer, &drift);
    // The distance to the apparent position, from which each star's own is measured, so that the
    // offset bound holds by the triangle inequality (the light age is the first guess's).
    let d_system = observer
        .position()
        .distance_to(r.apparent_position())
        .value()
        / METRES_PER_LIGHT_YEAR;
    if let (Some(reach), Some(bounds)) = (reach, &star_bounds)
        && !passes_after_drift(
            envelope,
            record,
            bounds,
            r.emitted(),
            Magnitudes::new(faintest_listable(query, d_system - reach.offset)),
        )
    {
        return None;
    }
    Some(r)
}

/// [`census_record`] with its cell's reach, which bounds the record's light when given and the
/// record has a density component, and its composition and hierarchy from `drawn`.
fn record_stars(
    galaxy: &Galaxy,
    ctx: &mut SkyContext<'_>,
    record: &SystemRecord,
    query: &SkyQuery,
    (bound, reach, drawn): (Bound, Option<&CellReach>, &mut Drawn),
    counts: &mut Counts,
    out: &mut Vec<SkyStar>,
) {
    if query.exclude() == Some(record.id()) || record.kind() == SystemKind::RoguePlanet {
        return;
    }
    let layer = record.layer();
    let Some(r) = bright_retarded(
        galaxy,
        ctx.envelope,
        record,
        query,
        (reach, drawn),
        counts.tallies.layer_mut(layer),
        counts.cost.layer_mut(layer),
    ) else {
        return;
    };
    let tally = &mut counts.tallies;
    let observer = query.observer();
    let emitted = r.emitted();
    let stars = SystemStars::generate(galaxy, record);
    let Some(state) = stars.state_at(emitted) else {
        return;
    };
    tally.layer_mut(layer).generated += 1;
    let mut positions: Vec<(BodyId, SystemPosition)> =
        Vec::with_capacity(usize::from(GRID_STAR_BOUND));
    star_positions_at(stars.hierarchy(), emitted, &mut positions);
    let observer_at = observer.position();
    let cut = query.cut().value();
    let mut modifiers: Vec<GasModifier> = Vec::new();
    let mut listable = false;
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
        // What a bound of the record's own realised stars could not skip, whatever the cone
        // (R06.T8.h's cost, `generated_listable`).
        listable |= unextinguished <= cut;
        // A cone's census keeps only the stars of its region's texels, so that its listing and the
        // band share the region exactly (R06.T8.l): the star's texel by the band's own lookup, of
        // the same displacement the band places an overflow star by. Both modes keep to it: it is
        // the kept test, not a skip.
        if let Some(region) = query.cone_region()
            && !region.holds(&observer_at.displacement_to(&apparent))
        {
            continue;
        }
        // The star's own V extinction is never negative (below), and adding it never lowers a
        // float: a star past the cut before it stays past it, so it takes no sightline (R06.T8.f).
        if bound == Bound::Applied && unextinguished > cut {
            continue;
        }
        let a_v = star_extinction(galaxy, ctx, &apparent, observer_at, &mut modifiers);
        let colour = colour_of_state(star);
        let v = unextinguished + own_v_extinction(&colour, a_v);
        if v > cut {
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
    counts.cost.layer_mut(layer).generated_listable += u64::from(listable);
}

/// The stars of `key` kept for `query`, appended to `out` (which is not cleared); its counts are
/// returned, for the caller to add up with [`CensusTallies::add`]. [`census_cell_with_cost`]
/// returns its cost beside them.
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
    census_cell_with_cost(galaxy, ctx, key, query, out).0
}

/// [`census_cell`], with the cell's cost beside its tallies: the counts that depend on how the
/// census reached the cell, through `ctx`'s cell cache or not (R06.T8.h).
///
/// With [`NoSkyCellCache`](super::NoSkyCellCache) the cell is placed at its floor and each record
/// censused, R06.T8.g's path. With a cache that keeps entries
/// ([`SkyCellCache`](super::SkyCellCache)), the cell is
/// served from its block if the block's entry holds the query's key and window, its held records
/// taken past the floor, each skipped unless its stored light may list at its distance before the
/// drift (the pre-filter), and the rest censused as R06.T8.g censuses them, their hierarchies
/// taken again. Otherwise the cell is built at its block's parameters if they serve the query, or
/// at the query's own, which then replace the block: placed at the entry's floor, each record held
/// if its light over the entry's window can be listed at its key, and each past the query's floor
/// censused from the same hierarchy. Either way the stars and tallies are R06.T8.g's, bit for bit.
///
/// # Panics
///
/// As [`SystemStars::generate`], for a key of another galaxy than `ctx`'s.
pub fn census_cell_with_cost(
    galaxy: &Galaxy,
    ctx: &mut SkyContext<'_>,
    key: CellKey,
    query: &SkyQuery,
    out: &mut Vec<SkyStar>,
) -> (CensusTallies, CensusCost) {
    let mut counts = Counts::default();
    counts.tallies.layer_mut(key.layer()).cells += 1;
    debug_assert!(ctx.offsets.is_for(galaxy), "another galaxy's offset bounds");
    let reach = CellReach::of(ctx.offsets, key, query);
    let floor = floor_at(galaxy, ctx.envelope, key, query, &reach);
    if ctx.cells.keeps_entries() {
        census_through_cache(galaxy, ctx, key, query, (&reach, floor), &mut counts, out);
    } else {
        census_placed(galaxy, ctx, key, query, (&reach, floor), &mut counts, out);
    }
    (counts.tallies, counts.cost)
}

/// The census of `key` with no entry: its records placed at the query's `floor` and each
/// censused, R06.T8.g's path.
fn census_placed(
    galaxy: &Galaxy,
    ctx: &mut SkyContext<'_>,
    key: CellKey,
    query: &SkyQuery,
    (reach, floor): (&CellReach, SolarMasses),
    counts: &mut Counts,
    out: &mut Vec<SkyStar>,
) {
    let mut records = Vec::new();
    generate_cell_where(galaxy, key, |m| m.value() >= floor.value(), &mut records);
    counts.cost.layer_mut(key.layer()).candidates +=
        u64::try_from(records.len()).unwrap_or(u64::MAX);
    for record in &records {
        record_stars(
            galaxy,
            ctx,
            record,
            query,
            (Bound::Applied, Some(reach), &mut Drawn::default()),
            counts,
            out,
        );
    }
}

/// The census of `key` through `ctx`'s cell cache (see [`census_cell_with_cost`]).
fn census_through_cache(
    galaxy: &Galaxy,
    ctx: &mut SkyContext<'_>,
    key: CellKey,
    query: &SkyQuery,
    (reach, floor): (&CellReach, SolarMasses),
    counts: &mut Counts,
    out: &mut Vec<SkyStar>,
) {
    let layer = key.layer();
    let need = need_at(reach, query);
    let mut params = BlockParams::of(query);
    let mut outcome = CellOutcome::Missed;
    if let Some(block) = ctx.cells.block(galaxy, BlockKey::of(key)) {
        let held = entry_need(ctx.offsets, key, block.params());
        let mut records = Vec::new();
        match serve_from_block(&block, key, &held, &need, &mut records) {
            Lookup::Served => {
                drop(block);
                census_served(galaxy, ctx, query, (reach, floor), &records, counts, out);
                counts.cost.layer_mut(layer).count(CellOutcome::Served);
                ctx.cells.note(key, CellOutcome::Served);
                return;
            }
            Lookup::NotBuilt if held.holds(&need) => params = *block.params(),
            Lookup::NotBuilt => outcome = CellOutcome::Rebuilt(Rebuild::Parameters),
            Lookup::Key => outcome = CellOutcome::Rebuilt(Rebuild::Key),
            Lookup::Window => outcome = CellOutcome::Rebuilt(Rebuild::Window),
        }
    }
    let entry = entry_need(ctx.offsets, key, &params);
    debug_assert!(
        entry.holds(&need),
        "{key:?}: the entry at {params:?} does not hold {query:?}'s need"
    );
    if !entry.holds(&need) {
        // A query's own parameters hold it whenever its time lies within ±H, as every query's
        // does (the builder refuses any other): kept for safety, the cell censused with no entry.
        census_placed(galaxy, ctx, key, query, (reach, floor), counts, out);
        counts.cost.layer_mut(layer).count(outcome);
        ctx.cells.note(key, outcome);
        return;
    }
    let held_floor = floor_for(galaxy, ctx.envelope, layer, entry.key());
    // The entry's floor is at or below the query's, its key being no brighter; the lower of the
    // two places every record either reads, should rounding ever part them.
    let lowest = held_floor.value().min(floor.value());
    let mut records = Vec::new();
    generate_cell_where(galaxy, key, |m| m.value() >= lowest, &mut records);
    let mut held = Vec::new();
    for record in &records {
        let mass = record.primary_initial_mass().value();
        let mut drawn = Drawn::default();
        if mass >= held_floor.value()
            && let Some(light) = held_light(galaxy, ctx.envelope, record, &entry, &mut drawn)
        {
            held.push(HeldRecord::new(*record, light));
        }
        if mass >= floor.value() {
            counts.cost.layer_mut(layer).candidates += 1;
            let generated = counts.tallies.layer(layer).generated();
            record_stars(
                galaxy,
                ctx,
                record,
                query,
                (Bound::Applied, Some(reach), &mut drawn),
                counts,
                out,
            );
            // Inclusion monotonicity, checked wherever an entry is built: a record this query's
            // census generates is held, and its light passes the pre-filter here, as it must for
            // every query the entry serves.
            debug_assert!(
                counts.tallies.layer(layer).generated() == generated
                    || held.last().is_some_and(|h| {
                        h.record() == record
                            && (record.component().is_none()
                                || h.light().may_list(Magnitudes::new(faintest_listable(
                                    query,
                                    BeforeDrift::of(record, query, reach).nearest_ly,
                                ))))
                    }),
                "{record:?}: generated for {query:?} but not held, or its light fails the \
                 pre-filter: a bound tightened as its window widened"
            );
        }
    }
    counts.cost.layer_mut(layer).held += u64::try_from(held.len()).unwrap_or(u64::MAX);
    counts.cost.layer_mut(layer).count(outcome);
    ctx.cells.keep(galaxy, key, &params, &held);
    ctx.cells.note(key, outcome);
}

/// The census of a served cell's `held` records: each past the query's `floor`, unless its stored
/// light cannot be listed at its distance before the drift (the pre-filter, which takes no bound),
/// censused as R06.T8.g censuses it, its hierarchy taken again (R06.T8.h).
fn census_served(
    galaxy: &Galaxy,
    ctx: &mut SkyContext<'_>,
    query: &SkyQuery,
    (reach, floor): (&CellReach, SolarMasses),
    held: &[HeldRecord],
    counts: &mut Counts,
    out: &mut Vec<SkyStar>,
) {
    for held in held {
        let record = held.record();
        let cost = counts.cost.layer_mut(record.layer());
        cost.held += 1;
        if record.primary_initial_mass().value() < floor.value() || floor.value().is_nan() {
            continue;
        }
        cost.candidates += 1;
        if record.component().is_some() {
            let before = BeforeDrift::of(record, query, reach);
            let faintest = Magnitudes::new(faintest_listable(query, before.nearest_ly));
            if !held.light().may_list(faintest) {
                cost.prefiltered += 1;
                continue;
            }
        }
        record_stars(
            galaxy,
            ctx,
            record,
            query,
            (Bound::Applied, Some(reach), &mut Drawn::default()),
            counts,
            out,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coords::UnitVector;
    use crate::galaxy::features::centre::testing::milky_way_galaxy;
    use crate::galaxy::gas::modifiers::NoModifiers;
    use crate::galaxy::gas::noise::NoiseCache;
    use crate::galaxy::placement::generate_cell;
    use crate::id::CentreMemberId;
    use crate::observe::Observer;
    use crate::sky::census::cache::NoSkyCellCache;
    use crate::sky::census::query::{Cone, census_plan, plan_cells};
    use crate::sky::testing::{
        milky_way_dark_tables, milky_way_envelope, milky_way_offsets, moving_galaxy,
    };
    use crate::time::ClockWindow;
    use crate::units::Degrees;

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

    /// Every float of `stars` that a census measures, as bits, in order: `PartialEq` holds 0.0 and
    /// −0.0 equal.
    fn star_bits(stars: &[SkyStar]) -> Vec<u64> {
        stars
            .iter()
            .flat_map(|s| {
                [s.v().value(), s.a_v().value(), s.distance().value()]
                    .into_iter()
                    .chain(s.apparent().offset_metres())
                    .map(hyperion_testkit::float::bits)
            })
            .collect()
    }

    /// One cell of each layer at, one cell out from and three cells out from the Sun along x.
    fn cells_by_the_sun() -> Vec<CellKey> {
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
        cells
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
        let giant_m_v = giant.v().value()
            - own_v_extinction(giant.colour(), giant.a_v())
            - distance_modulus(giant.distance().value());
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
        b.layer_mut(Layer::E).generated = 3;
        b.layer_mut(Layer::E).without_photometry = 1;
        b.layer_mut(Layer::A).centre_members = 4;
        a.add(&b);
        let c = a.layer(Layer::C);
        assert_eq!((c.cells(), c.accepted(), c.listed()), (3, 5, 2));
        let e = a.layer(Layer::E);
        assert_eq!((e.generated(), e.without_photometry()), (3, 1));
        assert_eq!(a.layer(Layer::A).centre_members(), 4);
        assert!(a.feature_members_absent());
    }

    #[test]
    fn costs_add_every_layers_counts() {
        let mut a = CensusCost::default();
        let mut b = CensusCost::default();
        b.layer_mut(Layer::E).candidates = 7;
        b.layer_mut(Layer::E).held = 9;
        b.layer_mut(Layer::E).prefiltered = 2;
        b.layer_mut(Layer::E).generated_listable = 1;
        b.layer_mut(Layer::E).star_bounded = 6;
        b.layer_mut(Layer::E).unbounded = 2;
        b.layer_mut(Layer::E).pairs.count(Some(PairLight::Detached));
        b.layer_mut(Layer::E).pairs.count(None);
        a.layer_mut(Layer::E).pairs.count(Some(PairLight::Remnants));
        a.layer_mut(Layer::E)
            .pairs
            .count(Some(PairLight::Unchanged));
        a.layer_mut(Layer::E)
            .pairs
            .count(Some(PairLight::Bright(Magnitudes::new(1.0))));
        for outcome in [
            CellOutcome::Served,
            CellOutcome::Served,
            CellOutcome::Missed,
            CellOutcome::Rebuilt(Rebuild::Key),
            CellOutcome::Rebuilt(Rebuild::Window),
            CellOutcome::Rebuilt(Rebuild::Parameters),
        ] {
            a.layer_mut(Layer::C).count(outcome);
        }
        a.add(&b);
        let c = a.layer(Layer::C);
        assert_eq!(
            (
                c.served(),
                c.missed(),
                c.rebuilt(Rebuild::Key),
                c.rebuilt(Rebuild::Window),
                c.rebuilt(Rebuild::Parameters),
                c.candidates()
            ),
            (2, 1, 1, 1, 1, 0)
        );
        let e = a.layer(Layer::E);
        assert_eq!(
            (
                e.candidates(),
                e.held(),
                e.prefiltered(),
                e.generated_listable()
            ),
            (7, 9, 2, 1)
        );
        assert_eq!((e.star_bounded(), e.unbounded_records()), (6, 2));
        let pairs = e.pairs();
        assert_eq!(
            (
                pairs.detached(),
                pairs.unchanged(),
                pairs.remnants(),
                pairs.bright(),
                pairs.unbounded(),
                pairs.total()
            ),
            (1, 1, 1, 1, 1, 5)
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
        let mut cells = cells_by_the_sun();
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
    /// Sun, at the epoch and up to 900 years from it, in a galaxy whose systems move (R06.T8.f):
    /// the widened envelope's bound, and the bound star by star after it (R06.T8.g), whose pairs'
    /// verdicts are taken over the ages before the drift, which hold the age at the emitted time.
    #[test]
    fn the_bound_before_the_drift_never_rejects_what_the_bound_after_it_passes() {
        let moving = moving_galaxy();
        let envelope = milky_way_envelope();
        let offsets = CellOffsets::build(moving);
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
        let (mut by_stars, mut passed_stars) = (0_u32, 0_u32);
        for observer in observers {
            let query = SkyQuery::builder(observer, Magnitudes::new(7.95))
                .eye(crate::sky::eye::EyeObserver::default())
                .build()
                .expect("a valid query");
            for record in &records {
                let key = CellKey::of(record.id()).expect("a grid record");
                let reach = CellReach::of(&offsets, key, &query);
                let before = passes_before_drift(envelope, record, &query, &reach);
                let before_stars = star_bounds_before_drift(
                    moving,
                    envelope,
                    record,
                    &query,
                    (&reach, &mut Drawn::default()),
                    &mut LayerCost::default(),
                )
                .is_some();
                let drift = Drift::of_record(moving, record).expect("a grid record moves");
                let r = retarded(query.observer(), &drift);
                let d = query
                    .observer()
                    .position()
                    .distance_to(r.apparent_position())
                    .value()
                    / METRES_PER_LIGHT_YEAR;
                let faintest = Magnitudes::new(faintest_listable(&query, d - reach.offset));
                let after = flux_bound(envelope, record, r.emitted())
                    .is_some_and(|m| m.value() <= faintest.value());
                assert!(before || !after, "{record:?} for {observer:?}");
                // The bound star by star after the drift, from the pairs' verdicts over the ages
                // before it, as the census reads it.
                let window = BeforeDrift::of(record, &query, &reach).ages;
                let age = record.age_at(r.emitted());
                assert!(
                    window.0 <= age && age <= window.1,
                    "{record:?} for {observer:?}: {age:?} outside {window:?}"
                );
                let after_stars = after
                    && StarBounds::of(moving, record, window)
                        .brightest(PhaseEnvelope::shared(), (age, age))
                        .may_list(faintest);
                assert!(
                    before_stars || !after_stars,
                    "{record:?} for {observer:?}: star by star"
                );
                checked += 1;
                rejected += u32::from(!before);
                passed += u32::from(after);
                by_stars += u32::from(before && !before_stars);
                passed_stars += u32::from(after_stars);
            }
        }
        eprintln!(
            "{checked} records: {rejected} rejected before the drift by the widened envelope and \
             {by_stars} more star by star; {passed} pass the widened envelope after it and \
             {passed_stars} both"
        );
        assert!(checked >= 10_000, "{checked}");
        assert!(
            rejected > 500 && passed > 500,
            "{rejected} rejected, {passed} passed"
        );
        assert!(
            by_stars > 500 && passed_stars > 10,
            "{by_stars} rejected star by star, {passed_stars} passed"
        );
    }

    /// Over the stars of 10⁴ generated systems of every layer, near the Sun and in the bulge, no
    /// star that the cut before its sightline drops would have passed the cut after it: `A_V` and
    /// each star's own V extinction behind it are never negative, and adding one never lowers a
    /// magnitude (R06.T8.f; R06.T8.k).
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
                    let limit = query.cut().value();
                    let a_v = sightline(
                        galaxy.gas(),
                        &apparent,
                        at,
                        NoiseMode::Realised,
                        STAR_SIGHTLINE_QUALITY,
                        &[],
                        &mut noise,
                    )
                    .a_v();
                    let own = own_v_extinction(&colour_of_state(star), a_v);
                    let v = before + own;
                    assert!(
                        a_v.value() >= 0.0 && own >= 0.0 && v >= before,
                        "{record:?}: A_V {}, its own V extinction {own}",
                        a_v.value()
                    );
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

    /// The eye moves no star in or out of the census (R06.T8.k): the cells of every layer by the
    /// Sun, censused at one cut with the eye asked and without it, list the same stars with the
    /// same tallies, bit for bit. Some of the stars lie within the colour table's largest offset of
    /// the cut, where the eye once moved each star's boundary by its own offset.
    #[test]
    fn a_census_with_the_eye_equals_one_without_it() {
        let galaxy = milky_way_galaxy();
        let cut = Magnitudes::new(9.0);
        let without = SkyQuery::builder(observer_at(SUN), cut)
            .build()
            .expect("a valid query");
        let with = SkyQuery::builder(observer_at(SUN), cut)
            .eye(crate::sky::eye::EyeObserver::default())
            .build()
            .expect("a valid query");
        let cells = cells_by_the_sun();
        let mut ctx = context();
        let mut run = |query: &SkyQuery| {
            let mut stars = Vec::new();
            let mut tallies = CensusTallies::default();
            for &key in &cells {
                tallies.add(&census_cell(galaxy, &mut ctx, key, query, &mut stars));
            }
            (stars, tallies)
        };
        let (eye_stars, eye_tallies) = run(&with);
        let (stars, tallies) = run(&without);
        assert_eq!(eye_stars, stars);
        assert_eq!(star_bits(&eye_stars), star_bits(&stars));
        assert_eq!(eye_tallies, tallies);
        let near_the_cut = stars
            .iter()
            .filter(|s| s.v().value() > cut.value() - 0.46)
            .count();
        eprintln!(
            "{} stars, {near_the_cut} within 0.46 mag of the cut, the same with the eye",
            stars.len()
        );
        assert!(near_the_cut > 0, "no star near the cut, of {}", stars.len());
    }

    /// Every star a cone's census keeps is brighter than the cut and in the cone's region, the
    /// band's texels about it (R06.T8.k, R06.T8.l), and they are the stars the same cells list
    /// with no cone whose texels lie in the region, star for star and bit for bit, while some of
    /// those cells' stars lie outside it: each cell of a 30° cone's plan within 60 ly of the Sun,
    /// at V 11, at the server's 64² band. The oracle of each cell, its every record measured with
    /// no skip, keeps the same stars.
    #[test]
    fn every_kept_star_is_brighter_than_the_cut_and_in_the_cones_region() {
        let galaxy = milky_way_galaxy();
        let observer = observer_at(SUN);
        let cut = Magnitudes::new(11.0);
        let cone = Cone::new(UnitVector::X, Degrees::new(30.0)).expect("a cone");
        let build = |cone: Option<Cone>| {
            let mut builder = SkyQuery::builder(observer, cut);
            if let Some(cone) = cone {
                builder = builder.cone(cone);
            }
            builder
                .build()
                .expect("a valid query")
                .with_caps_forced(LightYears::new(60.0))
                .expect("a forced cap")
        };
        let (narrow, wide) = (build(Some(cone)), build(None));
        let region = *narrow.cone_region().expect("a region");
        let mut ctx = context();
        let plan = census_plan(galaxy, ctx.tables, ctx.envelope, &narrow, &mut ctx.noise);
        let inside =
            |star: &SkyStar| region.holds(&observer.position().displacement_to(star.apparent()));
        let (mut kept, mut left_out) = (0_usize, 0_usize);
        let mut records = Vec::new();
        for key in plan.cells() {
            let (mut coned, mut all) = (Vec::new(), Vec::new());
            census_cell(galaxy, &mut ctx, key, &narrow, &mut coned);
            census_cell(galaxy, &mut ctx, key, &wide, &mut all);
            for star in &coned {
                assert!(star.v() <= cut, "{star:?}");
                assert!(inside(star), "{star:?} lies outside the cone's region");
            }
            let within: Vec<SkyStar> = all.iter().filter(|s| inside(s)).copied().collect();
            assert_eq!(coned, within, "{key:?}");
            assert_eq!(star_bits(&coned), star_bits(&within), "{key:?}");
            generate_cell(galaxy, key, &mut records);
            let mut oracle = Vec::new();
            let mut tally = CensusTallies::default();
            for record in &records {
                census_record(
                    galaxy,
                    &mut ctx,
                    record,
                    &narrow,
                    Bound::Ignored,
                    &mut tally,
                    &mut oracle,
                );
            }
            assert_eq!(coned, oracle, "{key:?}: the oracle");
            kept += coned.len();
            left_out += all.len() - within.len();
        }
        eprintln!("the cone keeps {kept} stars of its cells and leaves out {left_out}");
        assert!(kept > 0 && left_out > 0, "{kept} kept, {left_out} left out");
    }

    /// The census in motion (R06.T8.j; `decision-r06-pad-speed.md`): in the fixture built with its
    /// kinematic tables, observers at the Sun at the epoch, at +H and at −H, and 250 ly from it at
    /// +900 years, every record of every cell within each layer's forced cap (A 24, B 48, the brown
    /// dwarfs 48, C 96, D 192 and E 384 ly) plus a pad at 5,000 km/s over the light's earliest
    /// time, walked independently of [`pad_speed`] and generated whole, moves below its layer's
    /// [`pad_speed`]; lies within its cell's [`CellReach`] pad of its epoch position at the
    /// observer's time, at the retardation's first guess (the observer's time less the light time
    /// of the present distance, at which the light's age is taken) and at its emitted time; and,
    /// where its apparent position lies within its layer's cap, is in a cell of
    /// [`plan_cells`]'.
    #[test]
    fn the_census_plan_holds_every_record_its_caps_see() {
        let moving = moving_galaxy();
        let offsets = milky_way_offsets();
        assert!(offsets.is_for(moving));
        let h = crate::time::CLOCK_WINDOW_H.as_julian_years_f64();
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the clock window is 1,000 years, a whole number"
        )]
        let h = h.round() as i64;
        let at = |ly: [f64; 3], years: i64| {
            let t = UniverseTime::from_julian_years(years).expect("in the window");
            Observer::new(position(ly), t).expect("an observer")
        };
        let observers = [
            at(SUN, 0),
            at(SUN, h),
            at(SUN, -h),
            at([SUN[0] + 250.0, SUN[1], SUN[2]], 900),
        ];
        let caps = [
            (Layer::A, 24.0),
            (Layer::B, 48.0),
            (Layer::BrownDwarf, 48.0),
            (Layer::C, 96.0),
            (Layer::D, 192.0),
            (Layer::E, 384.0),
        ];
        let radii: Vec<(Layer, LightYears)> = caps
            .iter()
            .map(|&(layer, r)| (layer, LightYears::new(r)))
            .collect();
        let forced: Vec<crate::sky::caps::LayerCap> = radii
            .iter()
            .map(|&(layer, r)| crate::sky::caps::LayerCap::forced(layer, r))
            .collect();
        let mut counts = InMotion::default();
        for observer in observers {
            let query = SkyQuery::builder(observer, Magnitudes::new(11.0))
                .build()
                .expect("a valid query")
                .with_caps_forced_per_layer(&radii)
                .expect("forced caps");
            let planned: std::collections::BTreeSet<CellKey> =
                plan_cells(&query, forced.clone()).into_iter().collect();
            for &(layer, cap) in &caps {
                check_layer_in_motion(&query, &planned, layer, cap, &mut counts);
            }
        }
        let InMotion {
            checked,
            seen,
            fastest,
        } = counts;
        let fastest: Vec<String> = caps
            .iter()
            .map(|&(layer, _)| {
                let km_s = fastest[usize::from(layer.value())] / 1e3;
                format!("{layer:?} {km_s:.0}")
            })
            .collect();
        eprintln!(
            "{checked} records checked, {seen} seen within their caps; the fastest, km/s: {}",
            fastest.join(", ")
        );
        assert!(
            checked >= 30_000 && seen >= 10_000,
            "{checked} checked, {seen} seen"
        );
    }

    /// What [`the_census_plan_holds_every_record_its_caps_see`] counted: the records checked, those
    /// seen within their caps, and each layer's largest speed, m/s, by [`Layer::value`].
    #[derive(Debug, Default)]
    struct InMotion {
        checked: u32,
        seen: u32,
        fastest: [f64; Layer::ALL.len()],
    }

    /// Checks every record of `layer`'s cells within `cap` ly of `query`'s observer, plus a pad at
    /// 5,000 km/s over the light's earliest time, in [`moving_galaxy`], against its layer's
    /// [`pad_speed`], its cell's [`CellReach`] pad, and the `planned` cells where its apparent
    /// position lies within the cap; adds to `counts`.
    fn check_layer_in_motion(
        query: &SkyQuery,
        planned: &std::collections::BTreeSet<CellKey>,
        layer: Layer,
        cap: f64,
        counts: &mut InMotion,
    ) {
        let moving = moving_galaxy();
        let observer = query.observer();
        let t = observer.time();
        let walk_speed = crate::units::KilometresPerSecond::new(5_000.0);
        let earliest = t
            .checked_sub(
                Span::from_seconds_f64(cap * crate::units::consts::SECONDS_PER_JULIAN_YEAR)
                    .expect("a span"),
            )
            .expect("on the clock");
        let pad = pad_for(earliest, walk_speed)
            .value()
            .max(pad_for(t, walk_speed).value());
        let pad_speed_m_s = crate::units::MetresPerSecond::from(pad_speed(layer)).value();
        let ly = |a: &GalacticPosition, b: &GalacticPosition| {
            a.distance_to(b).value() / METRES_PER_LIGHT_YEAR
        };
        let mut records = Vec::new();
        let apex = observer.position().to_light_years_f64();
        for key in cells_meeting_ball(layer, apex, cap + pad) {
            let reach = CellReach::of(milky_way_offsets(), key, query);
            generate_cell(moving, key, &mut records);
            for record in &records {
                let drift = Drift::of_record(moving, record).expect("a grid record moves");
                let speed = drift.velocity().speed().value();
                assert!(speed < pad_speed_m_s, "{record:?} at {speed} m/s");
                let fastest = &mut counts.fastest[usize::from(layer.value())];
                *fastest = fastest.max(speed);
                let r = retarded(observer, &drift);
                let epoch = record.epoch_position();
                let present = crate::observe::Trajectory::position_at(&drift, t);
                // The retardation's first guess: the light time of the present distance back.
                let first_guess = t
                    .checked_sub(crate::observe::light_time(
                        observer.position().distance_to(&present),
                    ))
                    .expect("on the clock");
                let guessed = crate::observe::Trajectory::position_at(&drift, first_guess);
                let (now, guess, then) = (
                    ly(epoch, &present),
                    ly(epoch, &guessed),
                    ly(epoch, r.apparent_position()),
                );
                assert!(
                    now <= reach.pad && guess <= reach.pad && then <= reach.pad,
                    "{record:?} for {observer:?}: moved {now} ly by now, {guess} ly by the first \
                     guess and {then} ly by the emitted time, pad {}",
                    reach.pad
                );
                if ly(observer.position(), r.apparent_position()) <= cap {
                    assert!(
                        planned.contains(&key),
                        "{record:?} is seen within {cap} ly by {observer:?} from {key:?}, which \
                         the plan does not open"
                    );
                    counts.seen += 1;
                }
                counts.checked += 1;
            }
        }
    }

    /// The cells of `layer` whose box comes within `radius_ly` of `apex_ly`, by a walk of the
    /// cube of cells about it: [`plan_cells`]' oracle.
    fn cells_meeting_ball(layer: Layer, apex_ly: [f64; 3], radius_ly: f64) -> Vec<CellKey> {
        let size = f64::from(layer.cell_size_ly());
        #[expect(
            clippy::cast_possible_truncation,
            reason = "cell indices near the Sun, far inside i32"
        )]
        let index = |a: f64| (a / size).floor() as i32;
        let range = |k: usize| index(apex_ly[k] - radius_ly)..=index(apex_ly[k] + radius_ly);
        let mut cells = Vec::new();
        for x in range(0) {
            for y in range(1) {
                for z in range(2) {
                    let key = CellKey::new(layer, [x, y, z]).expect("in the cube");
                    let o = key.origin_ly();
                    let near_sq: f64 = (0..3)
                        .map(|k| {
                            let lo = f64::from(o[k]);
                            let near = apex_ly[k].clamp(lo, lo + size) - apex_ly[k];
                            near * near
                        })
                        .sum();
                    if near_sq.sqrt() <= radius_ly {
                        cells.push(key);
                    }
                }
            }
        }
        cells
    }

    /// The bound a star of `slot`, drawn at `attempt` of `record`'s system of `composition`, takes
    /// as its own: the phase envelope at its mass and its η, read from its full draws (the
    /// primary's own, a companion's at its attempt) rather than the census's η alone.
    fn own_bound(
        record: &SystemRecord,
        composition: &Composition,
        slot: &crate::stellar::multiplicity::StarSlot,
        attempt: RedrawAttempt,
        ages: (Years, Years),
    ) -> Option<Magnitudes> {
        let seed = milky_way_galaxy().seed();
        let draws = if slot.body().body_index() == 0 {
            assert_eq!(slot.body(), BodyId::new(record.id(), 0));
            StarDraws::for_star(seed, slot.body())
        } else {
            StarDraws::for_attempt(seed, slot.body(), u32::from(attempt.get()))
        };
        PhaseEnvelope::shared().brightest(slot.initial_mass(), composition, draws.eta(), ages)
    }

    /// What the ruling gives a record whose every pair is answered `verdict`, by a reading apart
    /// from [`StarBounds`]': each star of each attempt by its own bound, or by its pair's verdict,
    /// the primary at every attempt and again after the last where the fallback is listed.
    fn ruled_light(
        record: &SystemRecord,
        composition: &Composition,
        hierarchy: &HierarchyBound,
        verdict: Option<PairLight>,
        ages: (Years, Years),
    ) -> RecordLight {
        let Some(verdict) = verdict else {
            return if hierarchy
                .attempts()
                .iter()
                .any(crate::stellar::multiplicity::AttemptBound::may_carve)
            {
                RecordLight::Unbounded
            } else {
                ruled_light(
                    record,
                    composition,
                    hierarchy,
                    Some(PairLight::Detached),
                    ages,
                )
            };
        };
        let mut best: Option<f64> = None;
        let mut take = |m: Option<Magnitudes>| {
            if let Some(m) = m {
                best = Some(best.map_or(m.value(), |b| if m.value() < b { m.value() } else { b }));
            }
        };
        for listed in hierarchy.attempts() {
            let attempt = listed.attempt();
            for (i, slot) in listed.stars().iter().enumerate() {
                let own = own_bound(record, composition, slot, attempt, ages);
                let paired = listed
                    .pairs()
                    .iter()
                    .any(|p| p.stars().iter().any(|s| usize::from(s.get()) == i));
                take(match (paired, verdict) {
                    (false, _) | (true, PairLight::Detached | PairLight::Unchanged) => own,
                    (true, PairLight::Remnants) => None,
                    (true, PairLight::Bright(m)) => {
                        Some(own.map_or(m, |o| if o.value() < m.value() { o } else { m }))
                    }
                });
            }
        }
        if hierarchy.fallback().is_some() {
            take(own_bound(
                record,
                composition,
                hierarchy.primary(),
                RedrawAttempt::FIRST,
                ages,
            ));
        }
        best.map_or(RecordLight::Dark, |b| {
            RecordLight::Brightest(Magnitudes::new(b))
        })
    }

    /// The record, composition and hierarchy bound of the first of `layer`'s records near `at`
    /// born by the epoch that `pick` accepts.
    fn find_bound(
        layer: Layer,
        at: [f64; 3],
        n: usize,
        pick: impl Fn(&HierarchyBound) -> bool,
    ) -> (SystemRecord, Composition, HierarchyBound) {
        let galaxy = milky_way_galaxy();
        records_near(layer, at, n)
            .into_iter()
            .filter(|r| r.age_at(UniverseTime::EPOCH).value() > 0.0)
            .find_map(|record| {
                let composition = draw_metallicity(galaxy, &record);
                let hierarchy = hierarchy_bound(galaxy, &record, &composition);
                pick(&hierarchy).then_some((record, composition, hierarchy))
            })
            .unwrap_or_else(|| panic!("no record of {layer:?} near {at:?} found"))
    }

    /// Every verdict plan 11's pairs can take leaves each star the light the ruling gives it
    /// (`decision-p11-t16-hierarchy-bound.md` §§5–6), on synthetic answers for the verdicts that
    /// P11.T17.c brings: `Detached` and `Unchanged` each star's own bound, `Remnants` nothing,
    /// `Bright(M)` the brighter of each star's own and M, and none an unbounded record. The
    /// records are a triple of D near the Sun whose first attempt holds one pair the engine may
    /// run and a star of none, and a binary of C. Each star's mass and η are the generator's, bit
    /// for bit.
    #[test]
    fn each_verdict_leaves_the_stars_the_light_the_ruling_gives() {
        let galaxy = milky_way_galaxy();
        let phase = PhaseEnvelope::shared();
        let triple = find_bound(Layer::D, SUN, 4_000, |h| {
            let first = &h.attempts()[0];
            first.pairs().len() == 1 && first.stars().len() > 2
        });
        let binary = find_bound(Layer::C, SUN, 4_000, |h| {
            let first = &h.attempts()[0];
            first.pairs().len() == 1 && first.stars().len() == 2
        });
        let verdicts = [
            Some(PairLight::Detached),
            Some(PairLight::Unchanged),
            Some(PairLight::Remnants),
            Some(PairLight::Bright(Magnitudes::new(-30.0))),
            Some(PairLight::Bright(Magnitudes::new(4.0))),
            Some(PairLight::Bright(Magnitudes::new(50.0))),
            None,
        ];
        for (record, composition, hierarchy) in [&triple, &binary] {
            let age = record.age_at(UniverseTime::EPOCH);
            let ages = (age, age);
            let pairs: u64 = hierarchy
                .attempts()
                .iter()
                .map(|a| u64::try_from(a.pairs().len()).expect("few"))
                .sum();
            for verdict in verdicts {
                let bounds = StarBounds::with_verdicts(
                    galaxy,
                    record,
                    (*composition, ages),
                    hierarchy,
                    |_, _, _| verdict,
                );
                let read = bounds.brightest(phase, ages);
                let ruled = ruled_light(record, composition, hierarchy, verdict, ages);
                match (read, ruled) {
                    (RecordLight::Brightest(a), RecordLight::Brightest(b)) => {
                        hyperion_testkit::float::assert_same_bits(a.value(), b.value());
                    }
                    _ => assert_eq!(read, ruled, "{record:?}: {verdict:?}"),
                }
                assert_eq!(bounds.pairs().total(), pairs, "{record:?}");
                let counted = match verdict {
                    Some(PairLight::Detached) => bounds.pairs().detached(),
                    Some(PairLight::Unchanged) => bounds.pairs().unchanged(),
                    Some(PairLight::Remnants) => bounds.pairs().remnants(),
                    Some(PairLight::Bright(_)) => bounds.pairs().bright(),
                    None => bounds.pairs().unbounded(),
                };
                assert_eq!(counted, pairs, "{record:?}: {verdict:?}");
            }
        }
        // In the triple, a Remnants pair still leaves the star of no pair its own bound, and a
        // pair as bright as M −30 bounds the record by it.
        let (record, composition, hierarchy) = &triple;
        let age = record.age_at(UniverseTime::EPOCH);
        let remnants = StarBounds::with_verdicts(
            galaxy,
            record,
            (*composition, (age, age)),
            hierarchy,
            |_, _, _| Some(PairLight::Remnants),
        );
        assert_ne!(
            remnants.brightest(phase, (age, age)),
            RecordLight::Dark,
            "{record:?}"
        );
        let bright = StarBounds::with_verdicts(
            galaxy,
            record,
            (*composition, (age, age)),
            hierarchy,
            |_, _, _| Some(PairLight::Bright(Magnitudes::new(-30.0))),
        );
        assert_eq!(
            bright.brightest(phase, (age, age)),
            RecordLight::Brightest(Magnitudes::new(-30.0))
        );
    }

    /// Each star a record's bounds list is the generator's: its initial mass, bit for bit, and its
    /// η from its full draws at its attempt, the primary's its own at attempt 0 (R06.T8.g's
    /// step 4), over a triple of D and a binary of C near the Sun.
    #[test]
    fn each_bound_star_is_the_generators_star() {
        let galaxy = milky_way_galaxy();
        let triple = find_bound(Layer::D, SUN, 4_000, |h| {
            let first = &h.attempts()[0];
            first.pairs().len() == 1 && first.stars().len() > 2
        });
        let binary = find_bound(Layer::C, SUN, 4_000, |h| {
            let first = &h.attempts()[0];
            first.pairs().len() == 1 && first.stars().len() == 2
        });
        for (record, composition, hierarchy) in [&triple, &binary] {
            let age = record.age_at(UniverseTime::EPOCH);
            let ages = (age, age);
            let bounds = StarBounds::of(galaxy, record, ages);
            assert_eq!(bounds.composition(), composition);
            for listed in hierarchy.attempts() {
                for (i, slot) in listed.stars().iter().enumerate() {
                    let index =
                        StarIndex::from_body(u8::try_from(i).expect("few")).expect("a star");
                    let star = bounds.star(listed.attempt(), index).expect("listed");
                    let draws = if i == 0 {
                        StarDraws::for_star(galaxy.seed(), slot.body())
                    } else {
                        StarDraws::for_attempt(
                            galaxy.seed(),
                            slot.body(),
                            u32::from(listed.attempt().get()),
                        )
                    };
                    hyperion_testkit::float::assert_same_bits(
                        star.mass().value(),
                        slot.initial_mass().value(),
                    );
                    hyperion_testkit::float::assert_same_bits(
                        star.eta().value(),
                        draws.eta().value(),
                    );
                    assert_eq!(star.index(), index);
                }
            }
            assert_eq!(
                bounds.companions().len(),
                hierarchy
                    .attempts()
                    .iter()
                    .map(|a| a.stars().len() - 1)
                    .sum::<usize>()
            );
        }
    }

    /// The primary is bounded once, by the least light that bounds it at every attempt. A `Bright`
    /// verdict at the first attempt and `Detached` after it leave it the brighter of its own bound
    /// and the first's magnitude. `Remnants` everywhere still leaves it its own bound: the cover
    /// ends at an attempt with no pair the engine may run, where the primary is its own model, or
    /// lists the fallback, where the generator keeps it alone. The records are a binary of C near
    /// the Sun, redrawn once at least, and one of E whose eight attempts all may carve.
    #[test]
    fn the_primary_is_bounded_once_by_the_least_light_that_bounds_it_at_every_attempt() {
        let galaxy = milky_way_galaxy();
        let m = |v: f64| Magnitudes::new(v);
        // The union of the lights, each pair of them.
        for (a, b, both) in [
            (StarLight::Dark, StarLight::Dark, StarLight::Dark),
            (StarLight::Dark, StarLight::Own, StarLight::Own),
            (StarLight::Own, StarLight::Own, StarLight::Own),
            (
                StarLight::Own,
                StarLight::OwnOr(m(2.0)),
                StarLight::OwnOr(m(2.0)),
            ),
            (
                StarLight::Dark,
                StarLight::OwnOr(m(2.0)),
                StarLight::OwnOr(m(2.0)),
            ),
            (
                StarLight::OwnOr(m(2.0)),
                StarLight::OwnOr(m(-1.0)),
                StarLight::OwnOr(m(-1.0)),
            ),
        ] {
            assert_eq!(a.either(b), both, "{a:?} with {b:?}");
            assert_eq!(b.either(a), both, "{b:?} with {a:?}");
        }
        let binary = find_bound(Layer::C, SUN, 8_000, |h| {
            h.attempts().len() > 1
                && h.attempts()[0].pairs()[0]
                    .stars()
                    .contains(&StarIndex::PRIMARY)
        });
        let fallback = find_bound(Layer::E, SUN, 2_000, |h| h.fallback().is_some());
        assert_eq!(
            fallback.2.attempts().len(),
            usize::from(crate::stellar::multiplicity::MAX_REDRAWS)
        );
        for (record, composition, hierarchy) in [&binary, &fallback] {
            let age = record.age_at(UniverseTime::EPOCH);
            let with = |verdict: &mut dyn FnMut() -> PairLight| {
                StarBounds::with_verdicts(
                    galaxy,
                    record,
                    (*composition, (age, age)),
                    hierarchy,
                    |_, _, _| Some(verdict()),
                )
            };
            let remnants = with(&mut || PairLight::Remnants);
            assert_eq!(remnants.primary().light(), StarLight::Own, "{record:?}");
            let bright = with(&mut || PairLight::Bright(m(-1.0)));
            assert_eq!(
                bright.primary().light(),
                StarLight::OwnOr(m(-1.0)),
                "{record:?}"
            );
            let mut calls = 0_u32;
            let mixed = with(&mut || {
                calls += 1;
                if calls == 1 {
                    PairLight::Bright(m(-1.0))
                } else {
                    PairLight::Detached
                }
            });
            if hierarchy.attempts()[0].pairs()[0]
                .stars()
                .contains(&StarIndex::PRIMARY)
            {
                assert_eq!(
                    mixed.primary().light(),
                    StarLight::OwnOr(m(-1.0)),
                    "{record:?}"
                );
            }
            // It is the same star at every attempt.
            for attempt in hierarchy.attempts() {
                assert_eq!(
                    remnants.star(attempt.attempt(), StarIndex::PRIMARY),
                    Some(remnants.primary())
                );
            }
            let ruled = ruled_light(
                record,
                composition,
                hierarchy,
                Some(PairLight::Remnants),
                (age, age),
            );
            assert_eq!(
                remnants.brightest(PhaseEnvelope::shared(), (age, age)),
                ruled
            );
        }
    }

    /// A census of the cells by the Sun counts the records it bounds star by star and their
    /// pairs' verdicts: every record generated in C to E was bounded star by star, and some
    /// records so bounded are skipped.
    #[test]
    fn the_census_counts_the_records_it_bounds_star_by_star() {
        let galaxy = milky_way_galaxy();
        let query = SkyQuery::builder(observer_at(SUN), Magnitudes::new(7.95))
            .eye(crate::sky::eye::EyeObserver::default())
            .build()
            .expect("a valid query");
        let mut ctx = context();
        let (mut tallies, mut cost) = (CensusTallies::default(), CensusCost::default());
        let mut stars = Vec::new();
        for key in cells_by_the_sun() {
            let (t, c) = census_cell_with_cost(galaxy, &mut ctx, key, &query, &mut stars);
            tallies.add(&t);
            cost.add(&c);
        }
        let (mut bounded, mut generated, mut pairs) = (0, 0, PairTally::default());
        for layer in [Layer::C, Layer::D, Layer::E] {
            let (t, c) = (tallies.layer(layer), cost.layer(layer));
            eprintln!(
                "{layer:?}: {} records past the floor, {} bounded star by star ({} unbounded), \
                 {} generated ({} listable); pairs {:?}",
                c.candidates(),
                c.star_bounded(),
                c.unbounded_records(),
                t.generated(),
                c.generated_listable(),
                c.pairs()
            );
            assert!(c.star_bounded() <= c.candidates(), "{layer:?}");
            assert!(c.unbounded_records() <= c.star_bounded(), "{layer:?}");
            assert!(t.generated() <= c.star_bounded(), "{layer:?}");
            assert!(c.generated_listable() <= t.generated(), "{layer:?}");
            bounded += c.star_bounded();
            generated += t.generated();
            pairs.add(c.pairs());
        }
        assert!(
            generated < bounded && pairs.detached() > 0 && pairs.unbounded() > 0,
            "{generated} generated of {bounded} bounded; {pairs:?}"
        );
    }

    /// What [`check_realised_of`] found among some records of one layer at one place.
    #[derive(Debug, Clone, Copy, Default)]
    struct RealisedCheck {
        /// Systems born at one of the times at least.
        systems: u64,
        /// Shining stars checked against their own bound star by star, or the pair's.
        own: u64,
        /// Shining stars of an unbounded record, checked against the widened envelope.
        widened: u64,
        /// Records with a pair plan 11 cannot bound, at each time.
        unbounded: u64,
        /// The pairs' verdicts, at each time.
        pairs: PairTally,
        /// The least of M<sub>V</sub> less its bound over the stars checked star by star, mag.
        least_margin: f64,
    }

    impl RealisedCheck {
        fn add(&mut self, other: &Self) {
            self.systems += other.systems;
            self.own += other.own;
            self.widened += other.widened;
            self.unbounded += other.unbounded;
            self.pairs.add(&other.pairs);
            self.least_margin = self.least_margin.min(other.least_margin);
        }
    }

    /// Holds every shining star of `records`' realised systems at the epoch and 900 years before
    /// it no brighter than its bound: its own [`BoundStar`]'s at the attempt the generator kept,
    /// its pairs' verdicts taken at the system's age then, or the widened envelope's
    /// ([`flux_bound`]) where a pair of the record has none.
    fn check_realised_of(records: &[SystemRecord]) -> RealisedCheck {
        let galaxy = milky_way_galaxy();
        let phase = PhaseEnvelope::shared();
        let envelope = milky_way_envelope();
        let earlier = UniverseTime::EPOCH
            .checked_sub(Span::from_julian_years(900).expect("a span"))
            .expect("in the window");
        let mut check = RealisedCheck {
            least_margin: f64::INFINITY,
            ..RealisedCheck::default()
        };
        for record in records {
            let stars = SystemStars::generate(galaxy, record);
            let composition = draw_metallicity(galaxy, record);
            let hierarchy = hierarchy_bound(galaxy, record, &composition);
            let mut born = false;
            for t in [UniverseTime::EPOCH, earlier] {
                let Some(state) = stars.state_at(t) else {
                    continue;
                };
                born = true;
                let age = record.age_at(t);
                let ages = (age, age);
                let bounds =
                    StarBounds::from_hierarchy(galaxy, record, composition, &hierarchy, ages);
                check.pairs.add(bounds.pairs());
                let unbounded = bounds.brightest(phase, ages) == RecordLight::Unbounded;
                check.unbounded += u64::from(unbounded);
                for (i, star) in state.stars().iter().enumerate() {
                    let Some(m_v) = absolute_v_of_state(star) else {
                        continue;
                    };
                    let what = || format!("{record:?} at {t:?}: star {i}, {:?}", star.phase());
                    if unbounded {
                        let bound = flux_bound(envelope, record, t)
                            .unwrap_or_else(|| panic!("{}: unbounded where it shines", what()));
                        assert!(
                            m_v.value() >= bound.value(),
                            "{}: M_V {} over the widened bound {}",
                            what(),
                            m_v.value(),
                            bound.value()
                        );
                        check.widened += 1;
                        continue;
                    }
                    let index = StarIndex::from_body(u8::try_from(i).expect("few"))
                        .expect("a star of the system");
                    let listed = bounds
                        .star(stars.attempt(), index)
                        .unwrap_or_else(|| panic!("{}: its attempt is not listed", what()));
                    hyperion_testkit::float::assert_same_bits(
                        listed.mass().value(),
                        stars.stars()[i].initial_mass().value(),
                    );
                    let bound = listed
                        .brightest(phase, &composition, ages)
                        .unwrap_or_else(|| {
                            panic!(
                                "{}: shines at M_V {} where its bound, {:?}, says it cannot",
                                what(),
                                m_v.value(),
                                listed.light()
                            )
                        });
                    let margin = m_v.value() - bound.value();
                    assert!(
                        margin >= 0.0,
                        "{}: M_V {} over its bound {} ({:?})",
                        what(),
                        m_v.value(),
                        bound.value(),
                        listed.light()
                    );
                    check.least_margin = check.least_margin.min(margin);
                    check.own += 1;
                }
            }
            check.systems += u64::from(born);
        }
        check
    }

    /// [`check_realised_of`] over `per_place` records of each layer near the Sun and in the
    /// bulge, in four shares; prints each layer's figures and returns them all, with the systems
    /// born of each layer at both places together, by [`Layer::value`].
    fn check_realised(per_place: usize) -> (RealisedCheck, [u64; Layer::ALL.len()]) {
        let mut all = RealisedCheck {
            least_margin: f64::INFINITY,
            ..RealisedCheck::default()
        };
        let mut per_layer = [0_u64; Layer::ALL.len()];
        for (place, at) in [("near the Sun", SUN), ("in the bulge", BULGE)] {
            for layer in [
                Layer::A,
                Layer::B,
                Layer::C,
                Layer::D,
                Layer::E,
                Layer::BrownDwarf,
            ] {
                let records = records_near(layer, at, per_place);
                let records = &records[..per_place];
                // Four shares, share k the records k, k + 4, …, on threads of their own, or one
                // after another on wasm32-wasip1, which has none: the figures are the same.
                let share = |k: usize| {
                    let mine: Vec<SystemRecord> =
                        records.iter().skip(k).step_by(4).copied().collect();
                    check_realised_of(&mine)
                };
                #[cfg(not(target_family = "wasm"))]
                let parts: Vec<RealisedCheck> = std::thread::scope(|scope| {
                    let handles: Vec<_> = (0..4)
                        .map(|k| {
                            let share = &share;
                            scope.spawn(move || share(k))
                        })
                        .collect();
                    handles
                        .into_iter()
                        .map(|h| h.join().expect("a share's thread"))
                        .collect()
                });
                #[cfg(target_family = "wasm")]
                let parts: Vec<RealisedCheck> = (0..4).map(share).collect();
                let mut found = RealisedCheck {
                    least_margin: f64::INFINITY,
                    ..RealisedCheck::default()
                };
                for part in &parts {
                    found.add(part);
                }
                let p = found.pairs;
                eprintln!(
                    "{layer:?} {place}: {} systems; stars checked {} star by star (least margin \
                     {:.4} mag), {} against the widened envelope; records unbounded {}; pairs \
                     {} (detached {}, unchanged {}, remnants {}, bright {}, none {})",
                    found.systems,
                    found.own,
                    found.least_margin,
                    found.widened,
                    found.unbounded,
                    p.total(),
                    p.detached(),
                    p.unchanged(),
                    p.remnants(),
                    p.bright(),
                    p.unbounded()
                );
                per_layer[usize::from(layer.value())] += found.systems;
                all.add(&found);
            }
        }
        (all, per_layer)
    }

    /// The bound star by star holds for realised systems (R06.T8.g): 300 records of each layer
    /// near the Sun and in the bulge, at two emitted times.
    #[test]
    fn the_star_bound_holds_for_some_realised_systems() {
        let (all, _) = check_realised(300);
        assert!(all.own > 3_000 && all.pairs.detached() > 50, "{all:?}");
    }

    /// R06.T8.g's slow test: over 10⁵ records of each layer near the Sun and again in the bulge, at
    /// least 10⁵ realised systems of each layer, at the epoch and 900 years before it, every star
    /// of `state_at(t).stars()` is no
    /// brighter than its bound, star by star or, for a record with a pair plan 11 cannot bound,
    /// the widened envelope's. Each pair's verdict is taken at the system's age at that time, the
    /// tightest window the census asks. The figures, with the shares of plan 11's verdicts, are
    /// recorded in R06's Risks.
    #[test]
    #[ignore = "slow: generates 1.2 × 10⁶ systems, their pairs run through the binary engine"]
    fn the_star_bound_holds_for_realised_systems() {
        let (all, per_layer) = check_realised(100_000);
        eprintln!("all: {all:?}");
        for layer in [
            Layer::A,
            Layer::B,
            Layer::C,
            Layer::D,
            Layer::E,
            Layer::BrownDwarf,
        ] {
            let systems = per_layer[usize::from(layer.value())];
            assert!(
                systems >= 100_000,
                "{layer:?}: {systems} realised systems, under 10⁵"
            );
        }
    }

    /// [`Drawn`] gives [`StarBounds::of`]'s bounds, bit for bit, over each of two windows from one
    /// draw of the record's composition and hierarchy: the census's warm and cold paths both read
    /// it, so this holds them to the direct computation.
    #[test]
    fn a_drawn_hierarchy_gives_the_star_bounds_bit_for_bit() {
        use hyperion_testkit::float::bits;
        let galaxy = milky_way_galaxy();
        let phase = PhaseEnvelope::shared();
        let star_bits = |b: &StarBounds| -> Vec<u64> {
            core::iter::once(b.primary())
                .chain(b.companions())
                .flat_map(|s| [bits(s.mass().value()), bits(s.eta().value())])
                .chain([bits(b.composition().fe_h().value())])
                .collect()
        };
        let light_bits = |light: RecordLight| match light {
            RecordLight::Brightest(m) => Some(bits(m.value())),
            RecordLight::Dark | RecordLight::Unbounded => None,
        };
        let mut checked = 0_u32;
        for layer in [Layer::C, Layer::D, Layer::E] {
            for record in records_near(layer, SUN, 300).iter().take(300) {
                let age = record.age_at(UniverseTime::EPOCH).value();
                let mut drawn = Drawn::default();
                for ages in [
                    (Years::new(age - 4_000.0), Years::new(age + 1_000.0)),
                    (Years::new(age - 10.0), Years::new(age)),
                ] {
                    let got = drawn.star_bounds(galaxy, record, ages);
                    let direct = StarBounds::of(galaxy, record, ages);
                    assert_eq!(got, direct, "{record:?} over {ages:?}");
                    assert_eq!(star_bits(&got), star_bits(&direct), "{record:?}");
                    let (a, b) = (got.brightest(phase, ages), direct.brightest(phase, ages));
                    assert_eq!(a, b, "{record:?}");
                    assert_eq!(light_bits(a), light_bits(b), "{record:?}");
                    checked += 1;
                }
            }
        }
        assert_eq!(checked, 3 * 300 * 2);
    }

    /// Whether `wide`, a widened envelope's bound over a window, is no tighter than `narrow`, the
    /// same over a window within it: some, and no fainter, wherever `narrow` is some.
    fn flux_no_tighter(wide: Option<Magnitudes>, narrow: Option<Magnitudes>) -> bool {
        match (wide, narrow) {
            (_, None) => true,
            (None, Some(_)) => false,
            (Some(w), Some(n)) => w.value() <= n.value(),
        }
    }

    /// Whether `wide`, a record's light over a window, is no tighter than `narrow`, its light over
    /// a window within it, in the order dark, then brightest at a magnitude (looser as it falls),
    /// then unbounded.
    fn light_no_tighter(wide: RecordLight, narrow: RecordLight) -> bool {
        match (wide, narrow) {
            (RecordLight::Unbounded, _) | (_, RecordLight::Dark) => true,
            (_, RecordLight::Unbounded) | (RecordLight::Dark, RecordLight::Brightest(_)) => false,
            (RecordLight::Brightest(w), RecordLight::Brightest(n)) => w.value() <= n.value(),
        }
    }

    /// Each bound an entry keeps over its window is no tighter than over any window within it, the
    /// property on which a warm census's identity rests (R06.T8.h; `decision-r06-t8h-warm.md`):
    /// over 10⁴ records of each of C, D and E, near the Sun and in the bulge, the widened
    /// envelope's bound and the light star by star over each record's entry window, built for an
    /// observer at the place, against the ages before the drift of three queries the entry holds
    /// (from the place at the epoch, 900 ly along +x at +H, and 600 ly along −x at −H at a cut
    /// 0.1 fainter), and single ages at each window's ends and middle. R06.T8.n runs it again on
    /// P11.T17.c's verdicts.
    #[test]
    fn the_entry_light_holds_every_narrower_window() {
        let galaxy = milky_way_galaxy();
        let (envelope, offsets) = (milky_way_envelope(), milky_way_offsets());
        let phase = PhaseEnvelope::shared();
        let query = |at: [f64; 3], dx: f64, t: UniverseTime, cut: f64| {
            let observer =
                Observer::new(position([at[0] + dx, at[1], at[2]]), t).expect("an observer");
            SkyQuery::builder(observer, Magnitudes::new(cut))
                .build()
                .expect("a valid query")
        };
        let mut checked = 0_u64;
        for at in [SUN, BULGE] {
            let builder = query(at, 0.0, UniverseTime::EPOCH, 7.95);
            let params = BlockParams::of(&builder);
            let asked = [
                builder.clone(),
                query(at, 900.0, ClockWindow::END, 7.95),
                query(at, -600.0, ClockWindow::START, 8.05),
            ];
            for layer in [Layer::C, Layer::D, Layer::E] {
                for record in records_near(layer, at, 10_000).iter().take(10_000) {
                    let key = CellKey::of(record.id()).expect("a grid record");
                    let entry = entry_need(offsets, key, &params);
                    let wide = entry.ages_of(record);
                    let mut drawn = Drawn::default();
                    let flux = flux_bound_over(envelope, record, wide);
                    let light = drawn
                        .star_bounds(galaxy, record, wide)
                        .brightest(phase, wide);
                    for q in &asked {
                        assert!(
                            entry.holds(&query_need(offsets, key, q)),
                            "{record:?}: the entry holds {q:?}"
                        );
                        let before = BeforeDrift::of(record, q, &CellReach::of(offsets, key, q));
                        let (lo, hi) = before.ages;
                        let mid = Years::new(f64::midpoint(lo.value(), hi.value()));
                        for ages in [(lo, hi), (lo, lo), (mid, mid), (hi, hi)] {
                            assert!(
                                wide.0 <= ages.0 && ages.1 <= wide.1,
                                "{record:?}: {ages:?} outside the entry's {wide:?}"
                            );
                            let narrow = flux_bound_over(envelope, record, ages);
                            assert!(
                                flux_no_tighter(flux, narrow),
                                "{record:?} over {ages:?}: {flux:?} against {narrow:?}"
                            );
                            let narrow = drawn
                                .star_bounds(galaxy, record, ages)
                                .brightest(phase, ages);
                            assert!(
                                light_no_tighter(light, narrow),
                                "{record:?} over {ages:?}: {light:?} against {narrow:?}"
                            );
                            checked += 1;
                        }
                    }
                }
            }
        }
        assert_eq!(checked, 2 * 3 * 10_000 * 3 * 4);
    }

    /// Every record the census with no cache generates is held by an entry that serves its query,
    /// and its stored light passes the pre-filter there (R06.T8.h): over cells along x from 2,600
    /// ly on one side to 1,600 ly on the other, entries built at the Sun at V 6 and at V 9, and
    /// the queries each serves of a census at the Sun, 1,000 ly either way and at ±H, at V 6, and
    /// at the Sun at V 9.
    #[test]
    fn the_prefilter_keeps_every_record_the_census_generates() {
        let galaxy = milky_way_galaxy();
        let (envelope, offsets) = (milky_way_envelope(), milky_way_offsets());
        let along = |dx: f64| [SUN[0] + dx, SUN[1], SUN[2]];
        let query = |dx: f64, t: UniverseTime, cut: f64| {
            let observer = Observer::new(position(along(dx)), t).expect("an observer");
            SkyQuery::builder(observer, Magnitudes::new(cut))
                .build()
                .expect("a valid query")
        };
        let mut cells = Vec::new();
        for (layer, at) in [
            (Layer::A, &[-200.0, 0.0, 100.0][..]),
            (Layer::B, &[-200.0, 0.0, 100.0][..]),
            (
                Layer::C,
                &[-1_600.0, -800.0, 0.0, 200.0, 1_600.0, 2_600.0][..],
            ),
            (Layer::D, &[-1_600.0, 0.0, 200.0, 1_600.0, 2_600.0][..]),
            (Layer::E, &[-1_600.0, 0.0, 200.0, 1_600.0, 2_600.0][..]),
        ] {
            for &dx in at {
                cells.push(CellKey::containing(layer, &position(along(dx))).expect("in the cube"));
            }
        }
        let builders = [
            query(0.0, UniverseTime::EPOCH, 6.0),
            query(0.0, UniverseTime::EPOCH, 9.0),
        ];
        let asked = [
            query(0.0, UniverseTime::EPOCH, 6.0),
            query(1_000.0, UniverseTime::EPOCH, 6.0),
            query(-1_000.0, UniverseTime::EPOCH, 6.0),
            query(0.0, ClockWindow::END, 6.0),
            query(0.0, ClockWindow::START, 6.0),
            query(0.0, UniverseTime::EPOCH, 9.0),
        ];
        let (mut pairs, mut generated, mut prefiltered) = (0_u32, 0_u32, 0_u32);
        let mut records = Vec::new();
        for &key in &cells {
            for builder in &builders {
                let entry = entry_need(offsets, key, &BlockParams::of(builder));
                let held_floor = floor_for(galaxy, envelope, key.layer(), entry.key());
                for q in &asked {
                    let reach = CellReach::of(offsets, key, q);
                    if !entry.holds(&need_at(&reach, q)) {
                        continue;
                    }
                    pairs += 1;
                    let floor = floor_at(galaxy, envelope, key, q, &reach);
                    assert!(held_floor.value() <= floor.value(), "{key:?}: the floors");
                    generate_cell_where(galaxy, key, |m| m.value() >= floor.value(), &mut records);
                    for record in &records {
                        let cold = bright_retarded(
                            galaxy,
                            envelope,
                            record,
                            q,
                            (Some(&reach), &mut Drawn::default()),
                            &mut LayerTally::default(),
                            &mut LayerCost::default(),
                        );
                        let held =
                            held_light(galaxy, envelope, record, &entry, &mut Drawn::default());
                        let passes = held.is_some_and(|light| {
                            let before = BeforeDrift::of(record, q, &reach);
                            light.may_list(Magnitudes::new(faintest_listable(q, before.nearest_ly)))
                        });
                        if cold.is_some() {
                            generated += 1;
                            assert!(
                                passes,
                                "{record:?}: generated cold, held {held:?}, for {q:?}"
                            );
                        } else {
                            prefiltered += u32::from(!passes);
                        }
                    }
                }
            }
        }
        eprintln!(
            "{pairs} entries and queries served, {generated} records generated, {prefiltered} \
             others skipped by the pre-filter"
        );
        assert!(pairs > 100 && generated > 100 && prefiltered > 0);
    }
}
