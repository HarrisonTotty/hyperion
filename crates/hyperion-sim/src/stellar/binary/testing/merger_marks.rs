//! What the merger classes' samplers draw, read off the engine's own mergers (plan 15's P15.T10.b,
//! its merger step, held at 31): each stellar merger as `tables::binary`'s `MERGER_SAMPLER` reads
//! it, and each neutron-star merger as `NSM_SAMPLER` does, so that `hyperion-fit`'s tasks fit the
//! samplers to the merger census and the sim's tests draw against it with one reading.
//!
//! A merger is read at its merger age T, the pair just before it ([`MergerMember::before`]'s
//! state at T less a part in 10⁹), on the initial masses its stars had. The class draws a merger in
//! the source horizon now, so its system's age at the epoch is T to within L + 2H, some 2.6 × 10⁵
//! years, and the age rank is T's in its component's law among the born systems. A rank whose
//! range a table's limits set (the contact primary's lightest mass) is read for the limits it is
//! given.

use crate::galaxy::Galaxy;
use crate::galaxy::imf::MassBand;
use crate::galaxy::placement::SystemOrigin;
use crate::stellar::Phase;
use crate::stellar::binary::scan::{EVOLVED_PHASES, companion_ratio_mark, heaviest_living};
use crate::stellar::binary::star::Kind;
use crate::stellar::binary::{BinaryState, SegmentKind};
use crate::stellar::draws::StarDraws;
use crate::stellar::sse::{MIN_INITIAL_MASS, TrackOptions, turn_off_mass};
use crate::stellar::{Composition, StarState};
use crate::tables::binary::{
    MergerChannel, MergerMark, NSM_BLACK_HOLE_PROGENITORS, NSM_NEUTRON_STAR_PROGENITORS,
    NsmPairKind,
};
use crate::units::{SolarMasses, Years};

use super::mergers::MergerMember;

/// The rank, in [0, 1], of `m` in the galaxy's mass function over `[lo, hi]`, held at the ends; 0
/// for an empty range.
fn mass_rank(galaxy: &Galaxy, (lo, hi): (f64, f64), m: f64) -> f64 {
    let mass_function = galaxy.mass_function();
    let whole = mass_function.integral(lo, hi);
    if whole > 0.0 {
        (mass_function.integral(lo, m.clamp(lo, hi)) / whole).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// The age rank of a merger at `merger_age` in the component of `member`'s record: its age's in the
/// component's law among the born systems, as `BinaryHost::age_at_rank` inverts it; `None` for a
/// record of no component.
fn age_rank(galaxy: &Galaxy, member: &MergerMember, merger_age: Years) -> Option<f64> {
    let SystemOrigin::Grid(component) = member.record().origin() else {
        return None;
    };
    let ages = galaxy.fields().component(component).ages();
    let unborn = ages.cdf(Years::ZERO);
    let born = 1.0 - unborn;
    (born > 0.0).then(|| ((ages.cdf(merger_age) - unborn) / born).clamp(0.0, 1.0))
}

/// The pair just before it merged: its state at T less a part in 10⁹.
fn just_before(member: &MergerMember) -> BinaryState {
    let t = member.merger_age().value();
    member.timeline().state_at(Years::new(t * (1.0 - 1e-9)))
}

/// The composition of `member`'s pair.
fn composition_of(member: &MergerMember) -> &Composition {
    member.timeline().context().composition()
}

/// One of the engine's stellar mergers as the stellar merger class's sampler reads it
/// (`MERGER_SAMPLER`'s marks and choices).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StellarMergerReading {
    /// Its weight as a merger in the source horizon ([`MergerMember::weight`]).
    pub weight: f64,
    /// Its channel among the class's ([`MergerChannel`]), or `None` for one the class's channels
    /// do not cover: a pair with a compact or helium star, two evolved stars, a star still
    /// contracting, or two main-sequence stars out of contact whose heavier is no low-mass one.
    pub channel: Option<MergerChannel>,
    /// The band of the class's primary by its initial mass: the heavier star of two on the main
    /// sequence, or the evolved donor.
    pub layer: MassBand,
    /// [`MergerMark::AgeRank`]'s value: T's rank in its component's law among the born systems.
    pub age_rank: f64,
    /// The class's primary's initial mass, M☉.
    pub primary_mass: f64,
    /// The companion's initial mass over the primary's.
    pub mass_ratio: f64,
    /// The primary's phase just before the merger.
    pub primary_phase: Phase,
    /// The merger age T, Julian years.
    pub merger_age: f64,
    /// The turn-off mass at T at the pair's composition, M☉.
    pub turn_off: f64,
    /// For an evolved donor, the heaviest initial mass still living at T on the donor's own draws,
    /// M☉, as the class bounds its donor's range (`scan::merger`'s `heaviest_living`); the
    /// turn-off for the others.
    pub living_end: f64,
}

impl StellarMergerReading {
    /// `member` as the class's sampler reads it, or `None` for a record of no density component.
    #[must_use]
    pub fn of(galaxy: &Galaxy, member: &MergerMember) -> Option<Self> {
        let t = member.merger_age();
        let age_rank = age_rank(galaxy, member, t)?;
        let before = just_before(member);
        let stars: [StarState; 2] = *before.stars();
        let phases = stars.map(|s| s.phase());
        let initial = member.initial_masses().map(SolarMasses::value);
        let composition = composition_of(member);
        let turn_off = turn_off_mass(t, composition).value();
        let both_main_sequence = phases.iter().all(|&p| p == Phase::MainSequence);
        let evolved =
            |k: usize| EVOLVED_PHASES.contains(&phases[k]) && phases[1 - k] == Phase::MainSequence;
        // The class's primary: the heavier of two main-sequence stars, or the evolved donor. Two
        // main-sequence stars are a contact pair, or a low-mass (BSE's k = 0) donor the engine
        // merges dynamically; a heavier main-sequence donor's merger is not the class's.
        let (primary, channel) = if both_main_sequence {
            let primary = usize::from(initial[1] > initial[0]);
            let low_mass = Kind::of(Phase::MainSequence, stars[primary].mass().value())
                == Kind::LowMassMainSequence;
            let contact = member
                .timeline()
                .segments()
                .iter()
                .rfind(|s| s.start().value() < t.value() && s.kind() != SegmentKind::Merged)
                .is_some_and(|s| s.kind() == SegmentKind::Contact);
            let channel = if contact {
                Some(MergerChannel::Contact)
            } else if low_mass {
                Some(MergerChannel::DynamicalMainSequence)
            } else {
                None
            };
            (primary, channel)
        } else if evolved(0) {
            (0, Some(MergerChannel::EvolvedDonor))
        } else if evolved(1) {
            (1, Some(MergerChannel::EvolvedDonor))
        } else {
            (usize::from(initial[1] > initial[0]), None)
        };
        let primary_mass = initial[primary];
        let living_end = if channel == Some(MergerChannel::EvolvedDonor) {
            let draws: &StarDraws = member.timeline().context().draws(primary);
            let options = TrackOptions::default().with_rules(galaxy.rules().stellar);
            heaviest_living(composition, draws, options, t, SolarMasses::new(turn_off)).value()
        } else {
            turn_off
        };
        Some(Self {
            weight: member.weight(),
            channel,
            layer: band_of(primary_mass),
            age_rank,
            primary_mass,
            mass_ratio: initial[1 - primary] / primary_mass,
            primary_phase: phases[primary],
            merger_age: t.value(),
            turn_off,
            living_end,
        })
    }

    /// The mark the class draws the companion's mass ratio from: the contact pair's, the dynamical
    /// pair's, or an evolved donor's by its kind (`scan::merger`'s `companion_ratio_mark`).
    #[must_use]
    pub const fn ratio_mark(&self) -> Option<MergerMark> {
        match self.channel {
            Some(MergerChannel::Contact) => Some(MergerMark::ContactMassRatio),
            Some(MergerChannel::DynamicalMainSequence) => Some(MergerMark::DynamicalMassRatio),
            Some(MergerChannel::EvolvedDonor) => Some(companion_ratio_mark(self.primary_phase)),
            None => None,
        }
    }

    /// The range of a contact pair's primary in `band` the class draws it from at T: from the
    /// band's lower edge, plan 06's least track mass and `lightest` (the least track mass over the
    /// contact mass ratio's largest) to the band's upper edge and the turn-off then.
    #[must_use]
    pub fn contact_range(&self, band: MassBand, lightest: f64) -> (f64, f64) {
        let floor = MIN_INITIAL_MASS.value();
        (
            band.lo().max(floor).max(lightest),
            band.hi().min(self.turn_off),
        )
    }

    /// [`MergerMark::PrimaryRank`]'s value for a contact pair: its primary's rank in the galaxy's
    /// mass function over [`contact_range`](Self::contact_range) of its band.
    #[must_use]
    pub fn contact_primary_rank(&self, galaxy: &Galaxy, lightest: f64) -> f64 {
        mass_rank(
            galaxy,
            self.contact_range(self.layer, lightest),
            self.primary_mass,
        )
    }

    /// The range of an evolved donor in `band` the class draws it from at T: from the band's lower
    /// edge and the turn-off then to the band's upper edge and [`living_end`](Self::living_end).
    #[must_use]
    pub fn evolved_range(&self, band: MassBand) -> (f64, f64) {
        (band.lo().max(self.turn_off), band.hi().min(self.living_end))
    }

    /// [`MergerMark::EvolvedPrimaryRank`]'s value for an evolved donor: its rank in the galaxy's
    /// mass function over [`evolved_range`](Self::evolved_range) of its band.
    #[must_use]
    pub fn evolved_primary_rank(&self, galaxy: &Galaxy) -> f64 {
        mass_rank(galaxy, self.evolved_range(self.layer), self.primary_mass)
    }

    /// The layers, A–E, the class's layer choice is open to at this merger's T for its channel, as
    /// the scan restricts it (`lightest` the contact pair's): for a contact pair those whose
    /// [`contact_range`](Self::contact_range) is not empty, for an evolved donor those that meet
    /// the living masses past the turn-off; none for a dynamical pair, whose donor's mass is drawn
    /// with no layer, and for a merger the class does not cover.
    #[must_use]
    pub fn layers_open(&self, lightest: f64) -> [bool; 5] {
        MassBand::ALL.map(|band| match self.channel {
            Some(MergerChannel::Contact) => {
                let (lo, hi) = self.contact_range(band, lightest);
                lo < hi
            }
            Some(MergerChannel::EvolvedDonor) => {
                band.lo() < self.living_end && self.turn_off < band.hi()
            }
            Some(MergerChannel::DynamicalMainSequence) | None => false,
        })
    }
}

/// The stellar band of an initial mass `m`, M☉: the heaviest whose lower edge is at most `m`, A for
/// a mass below every edge.
#[must_use]
fn band_of(m: f64) -> MassBand {
    MassBand::ALL
        .iter()
        .rev()
        .find(|band| band.lo() <= m)
        .copied()
        .unwrap_or(MassBand::A)
}

/// One of the engine's neutron-star mergers as the neutron-star merger class's sampler reads it
/// (`NSM_SAMPLER`'s marks and choice).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NsmReading {
    /// Its weight as a merger in the source horizon ([`MergerMember::weight`]).
    pub weight: f64,
    /// Its pair's kind, or `None` for a pair of two black holes, which the class does not draw.
    pub kind: Option<NsmPairKind>,
    /// The neutron stars' masses just before the merger, M☉, heavier first, the second `None` for
    /// a black hole's pair.
    pub neutron_star_masses: [Option<f64>; 2],
    /// The black hole's mass just before the merger, M☉, for a black hole's pair.
    pub black_hole_mass: Option<f64>,
    /// The neutron stars' progenitors' initial masses, M☉, in the order of
    /// [`neutron_star_masses`](Self::neutron_star_masses).
    pub neutron_star_progenitors: [Option<f64>; 2],
    /// The black hole's progenitor's initial mass, M☉.
    pub black_hole_progenitor: Option<f64>,
    /// `NsmMark::AgeRank`'s value: T's rank in its component's law among the born systems.
    pub age_rank: f64,
}

impl NsmReading {
    /// `member` as the class's sampler reads it, or `None` for a record of no density component or
    /// a pair that is not two compact stars just before it merged.
    #[must_use]
    pub fn of(galaxy: &Galaxy, member: &MergerMember) -> Option<Self> {
        let t = member.merger_age();
        let age_rank = age_rank(galaxy, member, t)?;
        let stars = *just_before(member).stars();
        let initial = member.initial_masses().map(SolarMasses::value);
        let neutron: Vec<(f64, f64)> = (0..2)
            .filter(|&k| stars[k].phase() == Phase::NeutronStar)
            .map(|k| (stars[k].mass().value(), initial[k]))
            .collect();
        let hole: Vec<(f64, f64)> = (0..2)
            .filter(|&k| stars[k].phase() == Phase::BlackHole)
            .map(|k| (stars[k].mass().value(), initial[k]))
            .collect();
        let kind = match (neutron.len(), hole.len()) {
            (2, 0) => Some(NsmPairKind::DoubleNeutronStar),
            (1, 1) => Some(NsmPairKind::NeutronStarBlackHole),
            (0, 2) => None,
            _ => return None,
        };
        let mut neutron = neutron;
        neutron.sort_by(|a, b| b.0.total_cmp(&a.0));
        let at = |k: usize| neutron.get(k).copied();
        Some(Self {
            weight: member.weight(),
            kind,
            neutron_star_masses: [at(0).map(|n| n.0), at(1).map(|n| n.0)],
            black_hole_mass: hole.first().map(|h| h.0),
            neutron_star_progenitors: [at(0).map(|n| n.1), at(1).map(|n| n.1)],
            black_hole_progenitor: hole.first().map(|h| h.1),
            age_rank,
        })
    }

    /// `NsmMark::NeutronStarProgenitorRank`'s values: each neutron star's progenitor's rank in the
    /// galaxy's mass function over [`NSM_NEUTRON_STAR_PROGENITORS`], held at its ends.
    #[must_use]
    pub fn neutron_star_progenitor_ranks(&self, galaxy: &Galaxy) -> [Option<f64>; 2] {
        self.neutron_star_progenitors
            .map(|m| m.map(|m| mass_rank(galaxy, NSM_NEUTRON_STAR_PROGENITORS, m)))
    }

    /// `NsmMark::BlackHoleProgenitorRank`'s value: the black hole's progenitor's rank in the
    /// galaxy's mass function over [`NSM_BLACK_HOLE_PROGENITORS`], held at its ends.
    #[must_use]
    pub fn black_hole_progenitor_rank(&self, galaxy: &Galaxy) -> Option<f64> {
        self.black_hole_progenitor
            .map(|m| mass_rank(galaxy, NSM_BLACK_HOLE_PROGENITORS, m))
    }
}
