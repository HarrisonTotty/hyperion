//! Test helpers: those shared by the census bounds' tests (P11.T17.a's `light.rs` and P11.T17.b's
//! `pair_light`), a splitmix64 stream and the generator's own star–star pairs; and, with the
//! `testing` feature for other crates' tests too, the binary classes' prior members
//! ([`brute_force_class_members`], P11.T8.a) and the engine's own mergers
//! ([`brute_force_merger_members_in`], P11.T8.c), which plan 15's P15.T10.b fits the samplers to.
//!
//! The census bounds' helpers serve this crate's own tests alone, so they are built for tests
//! only; the splitmix64 stream serves both.

#[cfg(test)]
pub(crate) mod census;
mod members;
mod mergers;
#[cfg(test)]
pub(crate) mod remnant_pair;

pub use mergers::{MergerMember, brute_force_merger_members_in};

pub use members::{
    AccretingMarks, CensusPart, CensusRecords, CensusSite, ClassCensus, ClassMember, MemberCells,
    XrayMarks, brute_force_class_members, brute_force_class_members_in, class_census_in,
};

#[cfg(test)]
use crate::Seed;
#[cfg(test)]
use crate::coords::GalacticPosition;
#[cfg(test)]
use crate::galaxy::Galaxy;
#[cfg(test)]
use crate::galaxy::Population;
#[cfg(test)]
use crate::galaxy::params::GalaxyParams;
#[cfg(test)]
use crate::galaxy::placement::{CellKey, generate_cell};
#[cfg(test)]
use crate::galaxy::placement::{SystemOrigin, SystemRecord};
#[cfg(test)]
use crate::id::Layer;
#[cfg(test)]
use crate::stellar::binary::BinaryInput;
#[cfg(test)]
use crate::stellar::draws::StarDraws;
#[cfg(test)]
use crate::stellar::multiplicity::{
    HierarchyNode, RedrawAttempt, SlotKind, StarIndex, draw_hierarchy_of_composition,
};
#[cfg(test)]
use crate::stellar::system::{draw_metallicity, grid_multiplicity, primary_draws};
#[cfg(test)]
use crate::units::{SolarMasses, Years};
#[cfg(test)]
use hyperion_testkit::lcg::Lcg;

/// A splitmix64 stream.
pub(crate) struct Mix(pub(crate) u64);

impl Mix {
    pub(crate) fn word(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    #[cfg(test)]
    pub(crate) fn unit(&mut self) -> f64 {
        #[expect(
            clippy::cast_precision_loss,
            reason = "a 53-bit word and 2⁵³ are exact in an f64"
        )]
        let u = (self.word() >> 11) as f64 / (1_u64 << 53) as f64;
        u
    }

    /// A whole number in `-k..=k`.
    pub(crate) fn within(&mut self, k: i32) -> i32 {
        let span = u64::try_from(2 * k + 1).expect("a positive span");
        i32::try_from(self.word() % span).expect("a small number") - k
    }
}

/// The Milky Way fixture under `seed`.
#[cfg(test)]
#[must_use]
pub(crate) fn milky_way(seed: u64) -> Galaxy {
    Galaxy::from_params(Seed::new(seed), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral")
}

/// `n` star–star pairs of `layer` near the solar circle, as the generator draws them (plan 11's
/// `run_pairs`), from `mix_seed`'s stream: up to three records from each of random cells within
/// 4,000 ly along x and 2,000 ly along y of the point 26,000 ly out along +y, each record's
/// hierarchy at attempt (its count mod 8), its composition, its primary's draws and its
/// companions' at that attempt, with the record's age at the epoch, years.
#[cfg(test)]
pub(crate) fn generated_pairs(
    galaxy: &Galaxy,
    layer: Layer,
    n: usize,
    mix_seed: u64,
) -> Vec<(BinaryInput, f64)> {
    let mut mix = Mix(mix_seed);
    let size = i32::try_from(layer.cell_size_ly()).expect("a small cell size");
    let (along, across) = ((4_000 / size).max(1), (2_000 / size).max(1));
    let mut cell = Vec::new();
    let mut pairs = Vec::with_capacity(n + 8);
    let mut records = 0_u32;
    while pairs.len() < n {
        let key = CellKey::new(
            layer,
            [mix.within(along), 26_000 / size + mix.within(across), 0],
        )
        .expect("a cell of the grid");
        generate_cell(galaxy, key, &mut cell);
        for record in cell.iter().take(3) {
            let attempt = RedrawAttempt::new(u8::try_from(records % 8).expect("below 8"))
                .expect("an attempt below eight");
            records += 1;
            let comp = draw_metallicity(galaxy, record);
            let h = draw_hierarchy_of_composition(
                galaxy,
                record,
                &comp,
                grid_multiplicity(record),
                attempt,
            );
            let draws = |s: StarIndex| {
                if s == StarIndex::PRIMARY {
                    primary_draws(galaxy, record)
                } else {
                    let attempt = u32::from(attempt.get());
                    StarDraws::for_attempt(galaxy.seed(), h.star(s).body(), attempt)
                }
            };
            for (node, orbit) in h.pairs() {
                let HierarchyNode::Pair { inner, outer, .. } = *h.node(node) else {
                    continue;
                };
                let (HierarchyNode::Star(a), HierarchyNode::Star(b)) =
                    (*h.node(inner), *h.node(outer))
                else {
                    continue;
                };
                if [a, b]
                    .iter()
                    .any(|&s| h.star(s).kind() == SlotKind::BrownDwarf)
                {
                    continue;
                }
                let input = BinaryInput::new(
                    h.star(a).initial_mass(),
                    h.star(b).initial_mass(),
                    comp,
                    *orbit,
                    [draws(a), draws(b)],
                    record.age_at_epoch(),
                )
                .expect("a pair's stars are of 0.08-150 M_sun with a finite age");
                pairs.push((input, record.age_at_epoch().value()));
            }
        }
    }
    pairs.truncate(n);
    pairs
}

/// `n` massive grid systems of `galaxy`, their primaries drawn from its mass function on
/// 8–150 M☉ by `lcg_seed`'s stream: records of the young thin disc at the solar circle, in its
/// first density component, each with a layer-E candidate ID of its own, and 60 Myr old, so that
/// every primary of 8 M☉ and up has died by the epoch, as `tests/binary_system.rs`' census of the
/// stripped marks draws them (P11.T11). Their placement class is plan 08's alive, so each draws
/// its companions freely.
#[cfg(test)]
#[must_use]
pub(crate) fn massive_records(galaxy: &Galaxy, n: u32, lcg_seed: u64) -> Vec<SystemRecord> {
    let imf = galaxy.mass_function();
    let at = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0])
        .expect("the solar circle is inside the galaxy");
    let component = galaxy.fields().component_id(0).expect("components");
    let mut lcg = Lcg::new(lcg_seed);
    (0..n)
        .map(|i| {
            let m = imf.quantile_in(8.0, 150.0, lcg.next_f64());
            let key = CellKey::new(
                Layer::E,
                [
                    i32::try_from(i / 64 % 256).expect("small"),
                    100 + i32::try_from(i / 64 / 256).expect("small"),
                    0,
                ],
            )
            .expect("a cell of the grid");
            SystemRecord::from_parts(
                key.candidate_id(i % 64).expect("a candidate"),
                at,
                SystemOrigin::Grid(component),
                Population::YoungThinDisc,
                SolarMasses::new(m),
                Years::new(6.0e7),
            )
        })
        .collect()
}

/// Whether `timeline` shows a white dwarf above the Chandrasekhar mass at a segment's edge or at a
/// knot of a remnant's mass, at or before `until` years (P11.T4's dwarf limit, `dwarf_limit`).
#[cfg(test)]
#[must_use]
pub(crate) fn shows_a_dwarf_above_the_limit(
    timeline: &crate::stellar::binary::BinaryTimeline,
    until: f64,
) -> bool {
    dwarfs_above_the_limit(timeline, until).contains(&true)
}

/// [`shows_a_dwarf_above_the_limit`] for each member, primary first.
#[cfg(test)]
#[must_use]
pub(crate) fn dwarfs_above_the_limit(
    timeline: &crate::stellar::binary::BinaryTimeline,
    until: f64,
) -> [bool; 2] {
    use crate::stellar::Phase;
    use crate::stellar::remnant::structure::CHANDRASEKHAR_MASS;

    members_showing(timeline, until, |s| {
        matches!(
            s.phase(),
            Phase::HeliumWhiteDwarf | Phase::CarbonOxygenWhiteDwarf | Phase::OxygenNeonWhiteDwarf
        ) && s.mass() > CHANDRASEKHAR_MASS
    })
}

/// Whether each member of `timeline`, primary first, shows an oxygen–neon white dwarf above the
/// capture mass, 1.37 M☉, at a segment's edge or at a knot of a remnant's mass, at or before
/// `until` years (P11.T4's kept-core endings).
#[cfg(test)]
#[must_use]
pub(crate) fn oxygen_neon_dwarfs_above_the_capture_mass(
    timeline: &crate::stellar::binary::BinaryTimeline,
    until: f64,
) -> [bool; 2] {
    use crate::stellar::Phase;
    use crate::stellar::remnant::collapse::OXYGEN_NEON_CAPTURE_MASS;

    members_showing(timeline, until, |s| {
        s.phase() == Phase::OxygenNeonWhiteDwarf && s.mass() > OXYGEN_NEON_CAPTURE_MASS
    })
}

/// Whether each member of `timeline`, primary first, shows a state that `shows` holds of at a
/// segment's edge or at a knot of a remnant's mass, at or before `until` years.
#[cfg(test)]
#[must_use]
fn members_showing(
    timeline: &crate::stellar::binary::BinaryTimeline,
    until: f64,
    shows: impl Fn(&crate::stellar::StarState) -> bool,
) -> [bool; 2] {
    use crate::stellar::binary::star::Member;

    let mut above = [false; 2];
    let ages = timeline
        .segments()
        .iter()
        .flat_map(|s| {
            let knots = s.members().iter().flat_map(|m| match m {
                Member::Remnant { mass, .. } => mass.knots(),
                Member::Track { .. }
                | Member::Shaped { .. }
                | Member::MainSequence { .. }
                | Member::Cooling { .. }
                | Member::Frozen { .. }
                | Member::Gone => &[],
            });
            [s.start().value(), s.end().value()]
                .into_iter()
                .chain(knots.map(|k| k[0]))
        })
        .filter(|&age| age <= until);
    for age in ages {
        for (k, s) in timeline
            .state_at(Years::new(age))
            .stars()
            .iter()
            .enumerate()
        {
            above[k] |= shows(s);
        }
    }
    above
}

/// The kinds of the carbon–oxygen and oxygen–neon dwarfs that `timeline`'s members become giants
/// from and come back to, in order, each as (before, after, merged) (P11.T4's swollen dwarf): a
/// member that is such a dwarf in one segment, a giant the binary carries in a later one, and such
/// a dwarf again in the next after that; merged where the other member is gone in a segment
/// between, so that the giant is a merger product placed on a track of its own rather than the
/// swollen dwarf's. Read from the segments' members, since a swollen giant can live for no time
/// between a swell and a common envelope that strips it.
#[cfg(test)]
#[must_use]
pub(crate) fn swollen_dwarf_kinds(
    timeline: &crate::stellar::binary::BinaryTimeline,
) -> Vec<(crate::stellar::Phase, crate::stellar::Phase, bool)> {
    use crate::stellar::Phase;
    use crate::stellar::binary::star::Member;

    let mut found = Vec::new();
    for k in 0..2 {
        // The last dwarf the member was, whether it has been a carried giant since, and whether
        // the other member has been gone since.
        let mut dwarf: Option<(Phase, bool, bool)> = None;
        for segment in timeline.segments() {
            let merged = segment.members()[1 - k].is_gone();
            dwarf = match (&segment.members()[k], dwarf) {
                (
                    Member::Remnant {
                        phase: phase @ (Phase::CarbonOxygenWhiteDwarf | Phase::OxygenNeonWhiteDwarf),
                        ..
                    },
                    before,
                ) => {
                    if let Some((before, true, was_merged)) = before {
                        found.push((before, *phase, was_merged));
                    }
                    Some((*phase, false, false))
                }
                (Member::Shaped { .. }, Some((before, _, was_merged))) => {
                    Some((before, true, was_merged || merged))
                }
                (Member::Shaped { .. }, None)
                | (
                    Member::Track { .. }
                    | Member::MainSequence { .. }
                    | Member::Cooling { .. }
                    | Member::Frozen { .. }
                    | Member::Remnant { .. }
                    | Member::Gone,
                    _,
                ) => None,
            };
        }
    }
    found
}

/// Whether `input`'s primary dies when plan 06 says, its death pinned (design note 16).
#[cfg(test)]
#[must_use]
pub(crate) fn pinned_primary(input: &BinaryInput) -> bool {
    super::evolve::pinned_death_age_years(input).is_some()
}
