//! The binary classes' prior members (plan 11's test helpers; P11.T8.a): every first-attempt
//! binary of the grid that [`carved_class`] puts in a class, drawn with no carve-out, against which
//! a class's sampler is checked (P11.T8) and plan 15's P15.T10.b fits it.

use crate::galaxy::Galaxy;
use crate::galaxy::imf::MassBand;
use crate::galaxy::placement::{CellKey, SystemRecord, generate_cell};
use crate::id::Layer;
use crate::orbit::KeplerElements;
use crate::rng::Threshold;
use crate::stellar::Phase;
use crate::stellar::binary::classify::{
    MAGNETIC_CV_SHARE, nova_recurrence_period, white_dwarf_feeding,
};
use crate::stellar::binary::{BinaryTimeline, CarvedClass, carved_class};
use crate::stellar::multiplicity::{MultiplicityContext, RedrawAttempt};
use crate::stellar::system::{StarModel, SystemStars, draw_metallicity, primary_draws};
use crate::tables::binary::CLASS_SHARES;
use crate::units::{Seconds, SolarMasses, SolarMassesPerYear, Years};

use super::Mix;

/// What an accreting white dwarf member is where [`carved_class`] reads it: the marks its class's
/// sampler draws.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AccretingMarks {
    /// The age read, Julian years: the epoch's, or the first of the horizon at which it accretes.
    pub age: Years,
    /// The white dwarf's mass, M☉.
    pub white_dwarf_mass: SolarMasses,
    /// The donor's mass, M☉.
    pub donor_mass: SolarMasses,
    /// The donor's phase.
    pub donor_phase: Phase,
    /// The orbital period, or `None` for a pair with no orbit then (none of the classes').
    pub period: Option<Seconds>,
    /// The rate onto the white dwarf, M☉ yr⁻¹.
    pub transfer_rate: SolarMassesPerYear,
    /// The nova recurrence period, Julian years.
    pub recurrence: Years,
    /// Whether the white dwarf is magnetic, by its own draw.
    pub magnetic: bool,
}

/// One prior member of a carved class: the grid record it came from, its pair's timeline and,
/// for an accreting white dwarf, its marks.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassMember {
    record: SystemRecord,
    timeline: BinaryTimeline,
    accreting: Option<AccretingMarks>,
}

impl ClassMember {
    /// The grid record.
    #[must_use]
    pub const fn record(&self) -> &SystemRecord {
        &self.record
    }

    /// The pair's timeline, from the engine.
    #[must_use]
    pub const fn timeline(&self) -> &BinaryTimeline {
        &self.timeline
    }

    /// An accreting white dwarf's marks where its class reads them.
    #[must_use]
    pub const fn accreting(&self) -> Option<&AccretingMarks> {
        self.accreting.as_ref()
    }
}

/// `n` prior members of `class` near the solar circle (plan 11's test helpers): the first
/// attempt of every two-star pair of grid records, run through the engine with no carve-out, kept
/// where [`carved_class`] is `class`, from the layers in the shares [`CLASS_SHARES`] gives the
/// class's hosts. [`brute_force_class_members_in`] of stream 0.
///
/// It runs the binary engine on every pair that can interact: some milliseconds a pair, so a test
/// of thousands of members is a slow one.
#[must_use]
pub fn brute_force_class_members(
    galaxy: &Galaxy,
    class: CarvedClass,
    n: usize,
) -> Vec<ClassMember> {
    brute_force_class_members_in(galaxy, class, n, 0)
}

/// [`brute_force_class_members`] on the cells of stream `stream`, so that a test can split the
/// sampling between threads, each with its own stream, and join the members in stream order.
///
/// Each layer with a share s of the class's hosts contributes `round(n s)` members, from random
/// cells within 4,000 ly along x and 2,000 ly along y of the point 26,000 ly out along +y, up to
/// eight records a cell.
///
/// # Panics
///
/// Never: the layers' cells are a few light-years to 128 ly, and the cells it opens lie inside the
/// root cube.
#[must_use]
pub fn brute_force_class_members_in(
    galaxy: &Galaxy,
    class: CarvedClass,
    n: usize,
    stream: u64,
) -> Vec<ClassMember> {
    let row = CLASS_SHARES.row(class);
    let mut members = Vec::with_capacity(n + 8);
    let mut cell = Vec::new();
    for (index, band) in (0_u64..).zip(MassBand::ALL) {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss,
            reason = "a member count, rounded, of a test's few thousand"
        )]
        let wanted = (n as f64 * row.layer_share(band)).round() as usize;
        if wanted == 0 {
            continue;
        }
        let layer = Layer::from(band);
        let mut mix = Mix(0x0b17_08a0 ^ (stream << 8) ^ index);
        let size = i32::try_from(layer.cell_size_ly()).expect("a small cell size");
        let (along, across) = ((4_000 / size).max(1), (2_000 / size).max(1));
        let start = members.len();
        while members.len() - start < wanted {
            let key = CellKey::new(
                layer,
                [mix.within(along), 26_000 / size + mix.within(across), 0],
            )
            .expect("a cell of the grid");
            generate_cell(galaxy, key, &mut cell);
            for record in cell.iter().take(8) {
                members_of(galaxy, record, class, &mut members);
            }
        }
        members.truncate(start + wanted);
    }
    members
}

/// The members of `class` among the first attempt's pairs of grid record `record`, of a stellar
/// layer.
fn members_of(
    galaxy: &Galaxy,
    record: &SystemRecord,
    class: CarvedClass,
    out: &mut Vec<ClassMember>,
) {
    let composition = draw_metallicity(galaxy, record);
    let age = record.age_at_epoch();
    let primary = StarModel::new_under(
        record.primary_initial_mass(),
        composition,
        primary_draws(galaxy, record),
        age,
        galaxy.rules().stellar,
    )
    .expect("a stellar layer's primary is a star with a finite age");
    let stars = SystemStars::at_attempt(
        galaxy,
        record,
        &primary,
        MultiplicityContext::Free,
        RedrawAttempt::FIRST,
    );
    for pair in stars.pairs() {
        let timeline = pair.timeline();
        if carved_class(timeline, age) != Some(class) {
            continue;
        }
        out.push(ClassMember {
            record: *record,
            timeline: timeline.clone(),
            accreting: accreting_marks(timeline, age),
        });
    }
}

/// An accreting white dwarf's marks where [`carved_class`] reads them, or `None` for a pair whose
/// white dwarf no age of the horizon feeds.
fn accreting_marks(timeline: &BinaryTimeline, age_at_epoch: Years) -> Option<AccretingMarks> {
    let (age, accretor, rate) = white_dwarf_feeding(timeline, age_at_epoch)?;
    let state = timeline.state_at(age);
    let a = accretor.index();
    let [white_dwarf, donor] = [&state.stars()[a], &state.stars()[1 - a]];
    let magnetic = timeline
        .context()
        .draws(a)
        .magnetism()
        .is_below(Threshold::from_probability(MAGNETIC_CV_SHARE));
    Some(AccretingMarks {
        age,
        white_dwarf_mass: white_dwarf.mass(),
        donor_mass: donor.mass(),
        donor_phase: donor.phase(),
        period: state.orbit().map(KeplerElements::period),
        transfer_rate: rate,
        recurrence: nova_recurrence_period(white_dwarf.mass(), rate),
        magnetic,
    })
}
