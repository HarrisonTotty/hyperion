//! A dwarf core's members (plan 10, P10.T7.b), placed in its nested grid band by band with plan
//! 09's machinery (P09.T21, T22) under [`DwarfCoreMemberId`].
//!
//! # Placement
//!
//! As a catalogue feature's ([`FeatureInterior`](crate::galaxy::features::members::FeatureInterior)):
//! a cell's candidates are a Poisson draw on `member.cell` of mean its bound over the cell
//! ([`MemberClassTable::proposal`]) times its volume, keyed by `ObjectKey::cell(word)` with the
//! word the ID of the cell's candidate 0 in its band, clamped at the 13-bit index's 8,192 with a
//! debug assertion; candidate `i`'s three uniforms on `member.position` put it in its cell and one
//! mark on `member.accept` picks its class or rejects it ([`MemberClassTable::pick`]). The core's
//! classes all share its one profile (P10.T7.a), so a cell's bound is its nearest corner's.
//!
//! # A member
//!
//! - **Its star**, attempt after attempt as plan 09's centre draws one (P09.T27): attempt `k`
//!   reads word `k` of `dwarf.core.age`, an age by the inverse transform of the core's
//!   [`StarFormationHistory`], and six words of `member.mass` from `6k`, a primary's initial mass
//!   from the galaxy's mass function in the class's range, a mark and a pair's systemic speed, as
//!   a feature member's. A living star is kept if it lies below its age's turn-off at its
//!   composition, and a remnant class's star if its remnant is of the class's kind: every remnant
//!   the core formed stays (P10.T7.a), so no kick is judged. The loop is bounded at
//!   [`MAX_ATTEMPTS`] with a debug assertion; its expected length is the class's share of its
//!   band's range that a uniform age leaves of the right kind.
//! - **Its composition**: the core's mean \[Fe/H\] plus its spread times one normal of
//!   `dwarf.core.abundance` (P10.T7.a, as ruled), with no helium excess.
//! - **Its velocity**: the core's bulk motion plus an isotropic normal of the core's dispersion,
//!   flat in radius ([`DwarfCoreModel::dispersion_at`]), on `member.velocity`: a Box–Muller pair
//!   and the first normal of a second, as a feature member's first attempt. It is not cut at an
//!   escape speed, as a cluster member's is: the isothermal halo the extent assumes is untruncated
//!   in its distribution of velocities, and a cut at the truncated halo's `2σ √(1 + ln(r_J ÷ r))`
//!   would take a quarter of the members at the extent, against the flat dispersion measured
//!   there (Walker et al. 2007).
//! - **Its record**: origin [`SystemOrigin::GlobalListMember`] with its attempt, the halo's
//!   population, its primary's initial mass and its age (Design note 13).
//!
//! # Capacity
//!
//! Plan 01's slot gives a core 16 cells an axis on 8 levels, so a cell at radius `r` spans about
//! `r ÷ 6` whatever the grid's width and a core's fullest cell expects about 10⁻³ of its systems:
//! one above some 4 × 10⁶ M☉ overflows the 13-bit index. [`DwarfCoreModel::check_index_headroom`]
//! says so as a typed error, as plan 09's centre's check does (ruling 144.5a); whoever plays a
//! core calls it before placing members (P10.T7.b, as built: 12 of the 252 cores of 64 drawn
//! galaxies' dwarfs, as drawn and unscaled, fail it).
//!
//! # The feature-level list
//!
//! A core's list (band 7) is keyed by the ID of its member zero, which it does not have, and its
//! index is 13 bits ([`ListIndex::Feature`]). Its classes are plan 09's supernova classes (Type Ia
//! from the delay-time distribution on the core's own history, core collapse only while it still
//! forms stars), which wait on P09.T33, T34 and T36: until they register, the list is empty.
//!
//! [`NestedGrid`]: crate::galaxy::features::nested::NestedGrid
//! [`MemberClassTable::proposal`]: crate::galaxy::features::interior::MemberClassTable::proposal
//! [`MemberClassTable::pick`]: crate::galaxy::features::interior::MemberClassTable::pick

use std::error::Error;
use std::fmt;

use crate::coords::{GalacticDisplacement, GalacticPosition, GalacticVelocity};
use crate::galaxy::features::interior::{CellProposal, ClassKind, LocalCell, MemberClass};
use crate::galaxy::features::members::{
    ATTEMPT_WORDS, FeatureLevelList, INTERNAL_SPEED_SIGMAS, ListIndex, MAX_ATTEMPTS, MemberZero,
    remnant_fits, systemic_speed,
};
use crate::galaxy::features::nested::NestedCell;
use crate::galaxy::imf::MassBand;
use crate::galaxy::placement::{ResolveSystemError, SystemOrigin, SystemRecord};
use crate::galaxy::{Galaxy, PointLy, Population};
use crate::id::{DwarfCoreMemberId, MemberSlot, SystemId};
use crate::rng::{ObjectKey, Stream, tags};
use crate::stellar::Composition;
use crate::stellar::sse::turn_off_mass;
use crate::units::consts::METRES_PER_LIGHT_YEAR;
use crate::units::{Dex, HeliumExcess, KilometresPerSecond, SolarMasses, Years};

use super::super::DwarfCoreNumber;
use super::super::members::GlobalMemberRecord;
use super::{DwarfCoreModel, StarFormationHistory};

/// Standard deviations of a Poisson count the index must spare (plan 03, Design note 6), as plan
/// 09's centre spares.
const HEADROOM_SIGMAS: f64 = 8.0;

/// A dwarf core can draw more candidates in a cell than its index numbers
/// ([`DwarfCoreModel::check_index_headroom`]): its fullest cell's expected candidates plus eight
/// standard deviations reach 8,192.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExceedDwarfCoreIndexError {
    /// The core.
    pub number: DwarfCoreNumber,
    /// Its fullest cell's expected candidates.
    pub expected: f64,
}

impl fmt::Display for ExceedDwarfCoreIndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "dwarf core {}'s fullest cell expects {:.0} candidates, within eight standard \
             deviations of its index's {}",
            self.number.get(),
            self.expected,
            MemberSlot::INDEX_LIMIT
        )
    }
}

impl Error for ExceedDwarfCoreIndexError {}

/// The relative margin [`DwarfCoreModel::member_speed_bound`] is rounded up by, as plan 09's
/// features' bound is: a member's speed is the norm of its bulk plus its internal velocity, which
/// can pass their sum by a few units in the last place.
const SPEED_BOUND_MARGIN: f64 = 1e-9;

/// The youngest age a turn-off is read at, years: 10⁴, as plan 09's centre reads it, past the
/// zero a history still forming stars reaches.
const YOUNGEST_TURN_OFF_AGE: f64 = 1e4;

impl DwarfCoreModel {
    /// The ID of candidate `index` of `band` in `cell`.
    ///
    /// # Panics
    ///
    /// If `cell` is not an owned cell of the core's 16-cell, 8-level grid or `index` is 8,192 or
    /// more, which [`NestedGrid`]'s cells and the clamp rule out.
    #[must_use]
    pub fn member_id(&self, band: MassBand, cell: NestedCell, index: u16) -> DwarfCoreMemberId {
        DwarfCoreMemberId::new(
            self.number().get(),
            MemberSlot::InCell {
                band: band.layer(),
                level: cell.level(),
                cell: cell.cell(),
                index,
            },
        )
        .expect("an owned cell of a 16-cell, 8-level grid and an index below 8,192 make an ID")
    }

    /// How `band`'s candidates are proposed in `cell`, and the cell's box.
    #[must_use]
    fn proposal(&self, band: MassBand, cell: NestedCell) -> (CellProposal, LocalCell) {
        let local = self.grid().local_cell(cell);
        (self.table().proposal(band, &local), local)
    }

    /// The expected candidates of `band` in `cell`: its bound times its volume, what a census sums
    /// (P09.T23).
    #[must_use]
    pub fn expected_candidates(&self, band: MassBand, cell: NestedCell) -> f64 {
        let (proposal, local) = self.proposal(band, cell);
        proposal.expected(&local)
    }

    /// The candidate count of `band` in `cell`: a Poisson draw on the cell's stream, clamped at
    /// [`MemberSlot::INDEX_LIMIT`] (module documentation).
    ///
    /// # Panics
    ///
    /// In debug builds, if the draw exceeds the index.
    #[must_use]
    pub fn candidate_count(&self, galaxy: &Galaxy, band: MassBand, cell: NestedCell) -> u16 {
        let mean = self.expected_candidates(band, cell);
        count_from_mean(galaxy, self.member_id(band, cell, 0), mean)
    }

    /// Candidate `index` of `band` in `cell`, if the thinning keeps it and its draw does not
    /// exhaust its attempts, with its local position (ly from the core's centre). The index is
    /// not checked against the count.
    #[must_use]
    pub fn candidate(
        &self,
        galaxy: &Galaxy,
        band: MassBand,
        cell: NestedCell,
        index: u16,
    ) -> Option<(GlobalMemberRecord, PointLy)> {
        let (proposal, local) = self.proposal(band, cell);
        self.candidate_under(galaxy, band, cell, index, &proposal, &local)
    }

    /// [`candidate`](Self::candidate) with the cell's bound and box given.
    fn candidate_under(
        &self,
        galaxy: &Galaxy,
        band: MassBand,
        cell: NestedCell,
        index: u16,
        proposal: &CellProposal,
        local: &LocalCell,
    ) -> Option<(GlobalMemberRecord, PointLy)> {
        let id = self.member_id(band, cell, index);
        let system = SystemId::from(id);
        let key = ObjectKey::from(system);
        let seed = galaxy.seed();
        let mut position = Stream::open(seed, tags::MEMBER_POSITION, key);
        let u = [0; 3].map(|_| position.uniform());
        let (p, bound) = proposal.place(local, u)?;
        let mark = Stream::open(seed, tags::MEMBER_ACCEPT, key).mark();
        let class = self.table().pick(band, &p, mark, bound)?;
        let composition = self.member_composition(galaxy, key);
        let (primary, attempt, age) =
            draw_star(galaxy, &class, system, self.history(), &composition)?;
        let record = SystemRecord::from_parts(
            system,
            self.galactic(&p),
            SystemOrigin::GlobalListMember {
                attempt: u16::try_from(attempt).expect("an attempt below 4,096 fits in u16"),
            },
            Population::Halo,
            SolarMasses::new(primary),
            age,
        );
        let velocity = self.member_velocity(galaxy, key);
        Some((
            GlobalMemberRecord::new(record, class, composition, velocity),
            p,
        ))
    }

    /// A member's composition: the core's mean \[Fe/H\] plus its spread times one normal of
    /// `dwarf.core.abundance` (module documentation).
    #[must_use]
    fn member_composition(&self, galaxy: &Galaxy, key: ObjectKey) -> Composition {
        let offset = Stream::open(galaxy.seed(), tags::DWARF_CORE_ABUNDANCE, key).standard_normal();
        let fe_h = Dex::new(self.fe_h().value() + self.fe_h_spread().value() * offset);
        Composition::from_fe_h(fe_h, HeliumExcess::ZERO)
    }

    /// A member's velocity (module documentation).
    #[must_use]
    fn member_velocity(&self, galaxy: &Galaxy, key: ObjectKey) -> GalacticVelocity {
        let sigma = self.dispersion().value() * 1e3;
        let mut stream = Stream::open(galaxy.seed(), tags::MEMBER_VELOCITY, key);
        let (first, second) = stream.standard_normal_pair();
        let third = stream.standard_normal();
        let [bx, by, bz] = self.velocity().metres_per_second();
        GalacticVelocity::new([bx + first * sigma, by + second * sigma, bz + third * sigma])
    }

    /// Whether its fullest cell's expected candidates plus eight standard deviations stay under
    /// the 13-bit index's 8,192, as plan 09's centre checks its own (P09.T27).
    ///
    /// Plan 01's slot gives a core 16 cells an axis on 8 levels, each cell twice the size of the
    /// last level's, so a cell at radius `r` spans about `r ÷ 6`, and its count, about
    /// `ρ(r) (r ÷ 6)³`, does not depend on the grid's width: a core's fullest cell expects about
    /// 10⁻³ of its systems (P10.T7.b, as built), and a core above some 4 × 10⁶ M☉ overflows the
    /// index, whose clamp would drop members. Whoever plays a core calls this first, as plan 04
    /// calls plan 03's check.
    ///
    /// # Errors
    ///
    /// [`ExceedDwarfCoreIndexError`] if the fullest cell's expected candidates plus eight standard
    /// deviations reach 8,192.
    pub fn check_index_headroom(&self) -> Result<(), ExceedDwarfCoreIndexError> {
        let expected = self.peak_candidates();
        if expected + HEADROOM_SIGMAS * expected.sqrt() < f64::from(MemberSlot::INDEX_LIMIT) {
            Ok(())
        } else {
            Err(ExceedDwarfCoreIndexError {
                number: self.number(),
                expected,
            })
        }
    }

    /// A bound on the speed of every member in its grid, km/s, galactic: |bulk| plus
    /// [`INTERNAL_SPEED_SIGMAS`] dispersions, the most a member's internal velocity can draw,
    /// rounded up by 1 + 10⁻⁹ (plan 09's `FeatureInterior::member_speed_bound`).
    #[must_use]
    pub fn member_speed_bound(&self) -> KilometresPerSecond {
        let bulk = KilometresPerSecond::from(self.velocity().speed()).value();
        let internal = INTERNAL_SPEED_SIGMAS * self.dispersion().value();
        KilometresPerSecond::new((bulk + internal) * (1.0 + SPEED_BOUND_MARGIN))
    }

    /// The galactic position of the local point `p` of a member.
    ///
    /// # Panics
    ///
    /// If `p` is so far out that the position leaves the addressable range, which no member's
    /// can: members lie within the core's extent, at most 20,000 ly from a centre in the root
    /// cube.
    #[must_use]
    fn galactic(&self, p: &PointLy) -> GalacticPosition {
        let centre = self.position().metres();
        let at = [
            centre[0] + p.x * METRES_PER_LIGHT_YEAR,
            centre[1] + p.y * METRES_PER_LIGHT_YEAR,
            centre[2] + p.z * METRES_PER_LIGHT_YEAR,
        ];
        GalacticPosition::ORIGIN
            .translated(GalacticDisplacement::new(at))
            .expect("a member within 20,000 ly of a core in the root cube is addressable")
    }

    /// Every accepted member of `band` in `cell`, in index order, with its local position, into
    /// `out` (cleared first).
    pub fn members_in_cell(
        &self,
        galaxy: &Galaxy,
        band: MassBand,
        cell: NestedCell,
        out: &mut Vec<(GlobalMemberRecord, PointLy)>,
    ) {
        out.clear();
        let (proposal, local) = self.proposal(band, cell);
        let n = count_from_mean(
            galaxy,
            self.member_id(band, cell, 0),
            proposal.expected(&local),
        );
        out.extend(
            (0..n).filter_map(|i| self.candidate_under(galaxy, band, cell, i, &proposal, &local)),
        );
    }

    /// The core's feature-level list (module documentation): keyed by the ID of its member zero,
    /// which it does not have, with a 13-bit index and no class yet.
    ///
    /// # Panics
    ///
    /// Never: a core's number is below 4, and its centre lies in the root cube.
    #[must_use]
    pub fn feature_level_list(&self, galaxy: &Galaxy) -> FeatureLevelList {
        let zero =
            DwarfCoreMemberId::new(self.number().get(), MemberSlot::FeatureLevel { index: 0 })
                .expect("a core's number is below 4 and index 0 is valid");
        let centre = GalacticPosition::ORIGIN
            .translated(self.position())
            .expect("a core in the root cube is addressable");
        FeatureLevelList::with_classes(
            galaxy,
            SystemId::from(zero).raw(),
            &centre,
            MemberZero::Absent,
            ListIndex::Feature,
            Vec::new(),
        )
    }

    /// The most candidates any band expects in an owned cell of its grid where the bound can
    /// peak
    /// ([`MemberClassTable::peak_candidates`](crate::galaxy::features::interior::MemberClassTable::peak_candidates)).
    #[must_use]
    pub fn peak_candidates(&self) -> f64 {
        self.table().peak_candidates(self.grid())
    }
}

/// A member's star of `class` (module documentation): its primary's initial mass, the attempt its
/// draws are read at, and its age; `None` if the draw exhausts its attempts.
///
/// # Panics
///
/// In debug builds, if the draw exhausts its attempts.
fn draw_star(
    galaxy: &Galaxy,
    class: &MemberClass,
    system: SystemId,
    history: &StarFormationHistory,
    composition: &Composition,
) -> Option<(f64, u32, Years)> {
    let seed = galaxy.seed();
    let key = ObjectKey::from(system);
    let (lo, hi) = (
        class.initial_mass_range.0.value(),
        class.initial_mass_range.1.value(),
    );
    let imf = galaxy.mass_function();
    let mut ages = Stream::open(seed, tags::DWARF_CORE_AGE, key);
    let mut masses = Stream::open(seed, tags::MEMBER_MASS, key);
    for attempt in 0..MAX_ATTEMPTS {
        ages.seek(u64::from(attempt));
        let age = history.age_quantile(ages.uniform());
        masses.seek(ATTEMPT_WORDS * u64::from(attempt));
        let mass = imf.quantile_in(lo, hi, masses.uniform());
        let _mark = masses.mark();
        let systemic = systemic_speed(&mut masses);
        let keep = match class.kind {
            ClassKind::Living | ClassKind::Tail => {
                let turn_off = turn_off_mass(
                    Years::new(age.value().max(YOUNGEST_TURN_OFF_AGE)),
                    composition,
                );
                mass < turn_off.value()
            }
            ClassKind::WhiteDwarf | ClassKind::NeutronStar | ClassKind::BlackHole => remnant_fits(
                class.kind,
                mass,
                composition,
                (seed, system, attempt),
                age,
                (f64::INFINITY, systemic),
            ),
        };
        if keep {
            return Some((mass, attempt, age));
        }
    }
    debug_assert!(false, "a core member of {class:?} exhausted its attempts");
    None
}

/// The count of a cell whose candidate 0 is `first` and whose mean is `mean`, clamped at the
/// index (module documentation).
///
/// # Panics
///
/// In debug builds, if the draw passes the index.
fn count_from_mean(galaxy: &Galaxy, first: DwarfCoreMemberId, mean: f64) -> u16 {
    let word = SystemId::from(first).raw();
    let drawn = Stream::open(galaxy.seed(), tags::MEMBER_CELL, ObjectKey::cell(word)).poisson(mean);
    let limit = MemberSlot::INDEX_LIMIT;
    debug_assert!(
        drawn <= u64::from(limit),
        "a dwarf core's cell drew {drawn} candidates on a mean of {mean}, past its index's {limit}"
    );
    u16::try_from(drawn.min(u64::from(limit))).expect("a count clamped at 8,192 fits in u16")
}

/// The member `id` names in the core `model` of `galaxy`: a candidate in a cell, or nothing at
/// feature level, whose classes wait on plan 09's supernova processes (module documentation).
///
/// # Errors
///
/// [`ResolveSystemError::NoSuchSystem`] if `id` is not `model`'s core's, its band is not
/// stellar, its cell is not one its grid owns, its index is at or past its cell's count, its
/// candidate was rejected or its draw exhausted its attempts, or it is on the feature-level list.
pub fn resolve_dwarf_core_member(
    galaxy: &Galaxy,
    model: &DwarfCoreModel,
    id: DwarfCoreMemberId,
) -> Result<GlobalMemberRecord, ResolveSystemError> {
    if id.number() != model.number().get() {
        return Err(ResolveSystemError::NoSuchSystem);
    }
    match id.slot() {
        // The list registers no class until plan 09's supernova processes do.
        MemberSlot::FeatureLevel { .. } => Err(ResolveSystemError::NoSuchSystem),
        MemberSlot::InCell {
            band,
            level,
            cell,
            index,
        } => {
            let band = MassBand::of_layer(band);
            if !band.is_stellar() {
                return Err(ResolveSystemError::NoSuchSystem);
            }
            let cell = model
                .grid()
                .cell(level, cell)
                .ok_or(ResolveSystemError::NoSuchSystem)?;
            let (proposal, local) = model.proposal(band, cell);
            let count = count_from_mean(
                galaxy,
                model.member_id(band, cell, 0),
                proposal.expected(&local),
            );
            if index >= count {
                return Err(ResolveSystemError::NoSuchSystem);
            }
            model
                .candidate_under(galaxy, band, cell, index, &proposal, &local)
                .map(|(member, _)| member)
                .ok_or(ResolveSystemError::NoSuchSystem)
        }
    }
}

#[cfg(test)]
mod tests;
