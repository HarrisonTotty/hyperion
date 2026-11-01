//! A census of the binary classes' prior members in a slab about the solar circle (plan 11's test
//! helpers): every first-attempt pair of a fixed number of grid records, the records of each band
//! in proportion to its share of the galaxy's systems, run through the engine with no carve-out and
//! kept where [`carved_class`] puts it in a class. The records weigh alike, so the members' shares
//! are the population's. P11.T4's engine fixes measure the X-ray binaries and the accreting white
//! dwarfs on it.
//!
//! The slab is 24,000 ly along x, 4,000 ly along y about the point 26,000 ly out along +y, and
//! 3,000 ly thick, the fit's solar-circle site (P15.T10.b's census). Its records are split into
//! [`PARTS`] parts, each with its own stream of cells, so a census is the same on any number of
//! threads.

use hyperion_testkit::threads::in_shares;

use crate::galaxy::Galaxy;
use crate::galaxy::imf::{BandShares, MassBand};
use crate::galaxy::placement::{CellKey, SystemRecord, generate_cell};
use crate::id::Layer;
use crate::stellar::Phase;
use crate::stellar::binary::{BinaryTimeline, CarvedClass, carved_class};
use crate::stellar::multiplicity::{MultiplicityContext, RedrawAttempt};
use crate::stellar::remnant::RemnantKind;
use crate::stellar::system::{StarModel, SystemStars, draw_metallicity, primary_draws};
use crate::units::SolarMasses;

use super::Mix;
use super::members::{AccretingMarks, XrayMarks, accreting_marks, xray_marks};

/// The parts a census's records are split into, each with its own stream of cells.
pub(crate) const PARTS: usize = 64;

/// The threads a census runs on: its test reserves as many slots.
pub(crate) const THREADS: usize = 16;

/// Half the slab's extent along x, ly.
const HALF_LENGTH_LY: i32 = 12_000;
/// The slab's centre along y, ly.
const CENTRE_Y_LY: i32 = 26_000;
/// Half the slab's extent along y, ly.
const HALF_WIDTH_LY: i32 = 2_000;
/// Half the slab's thickness, ly.
const HALF_THICKNESS_LY: i32 = 1_500;

/// One member of a carved class in a census.
#[derive(Debug, Clone)]
pub(crate) struct CensusMember {
    /// The pair's timeline.
    pub(crate) timeline: BinaryTimeline,
    /// An accreting white dwarf's marks where its class reads them.
    pub(crate) accreting: Option<AccretingMarks>,
    /// An X-ray binary's marks where its class reads them.
    pub(crate) xray: Option<XrayMarks>,
}

impl CensusMember {
    /// An X-ray binary's compact star at its birth: the remnant its last supernova left, kind and
    /// mass, or `None` for a member that is not an X-ray binary or whose compact star no
    /// supernova of the pair left (a black hole formed in a collision).
    #[must_use]
    pub(crate) fn compact_birth(&self) -> Option<(RemnantKind, SolarMasses)> {
        let marks = self.xray.as_ref()?;
        let state = self.timeline.state_at(marks.age);
        let compact = |p: Phase| matches!(p, Phase::NeutronStar | Phase::BlackHole);
        let stars = state.stars();
        let a = usize::from(!compact(stars[0].phase()) && compact(stars[1].phase()));
        self.timeline
            .supernovae()
            .iter()
            .rfind(|s| s.component().index() == a && s.age() <= marks.age)
            .map(|s| (s.remnant().kind(), s.remnant().mass()))
    }
}

/// A census: the records scanned in each band, band A first, and the members found.
#[derive(Debug, Clone)]
pub(crate) struct Census {
    /// The records scanned in each band.
    pub(crate) scanned: [u64; 5],
    /// The members, in part order.
    pub(crate) members: Vec<CensusMember>,
}

/// A census of `records` grid records of `galaxy` in the slab, the members of `classes` among their
/// first attempts' pairs, on at most [`THREADS`] threads (the same members on any number).
///
/// Band b gives round(`records` × its share) records, split evenly between the parts. Each part
/// draws random cells of the band's layer in the slab from its own stream and takes their records
/// in order until it has its count.
#[must_use]
pub(crate) fn slab_census(galaxy: &Galaxy, records: u64, classes: &[CarvedClass]) -> Census {
    let shares = BandShares::of(galaxy.mass_function());
    let parts = u64::try_from(PARTS).expect("a few parts");
    let wanted: Vec<[u64; 5]> = (0..parts)
        .map(|p| {
            MassBand::ALL.map(|band| {
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    clippy::cast_precision_loss,
                    reason = "a record count, rounded, of a census's 10⁶–10⁷"
                )]
                let total = (records as f64 * shares.share(band)).round() as u64;
                total / parts + u64::from(p < total % parts)
            })
        })
        .collect();
    let found = in_shares(PARTS, THREADS, |p| {
        let mut members = Vec::new();
        let mut cell = Vec::new();
        for (index, band) in (0_u64..).zip(MassBand::ALL) {
            let mut left = wanted[p][usize::try_from(index).expect("five bands")];
            let layer = Layer::from(band);
            let size = i32::try_from(layer.cell_size_ly()).expect("a small cell size");
            let mut mix =
                Mix(0x0b17_04c0_0000_0000 ^ (index << 40) ^ u64::try_from(p).expect("a few parts"));
            while left > 0 {
                let key = CellKey::new(
                    layer,
                    [
                        mix.within(HALF_LENGTH_LY / size),
                        CENTRE_Y_LY / size + mix.within(HALF_WIDTH_LY / size),
                        mix.within(HALF_THICKNESS_LY / size),
                    ],
                )
                .expect("a cell of the grid");
                generate_cell(galaxy, key, &mut cell);
                for record in &cell {
                    if left == 0 {
                        break;
                    }
                    left -= 1;
                    members_of(galaxy, record, classes, &mut members);
                }
            }
        }
        members
    });
    let scanned = MassBand::ALL.map(|band| wanted.iter().map(|w| w[band.index()]).sum());
    Census {
        scanned,
        members: found.into_iter().flatten().collect(),
    }
}

/// The members of `classes` among the first attempt's pairs of grid record `record`.
fn members_of(
    galaxy: &Galaxy,
    record: &SystemRecord,
    classes: &[CarvedClass],
    out: &mut Vec<CensusMember>,
) {
    let age = record.age_at_epoch();
    let primary = StarModel::new_under(
        record.primary_initial_mass(),
        draw_metallicity(galaxy, record),
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
        let Some(class) = carved_class(timeline, age).filter(|c| classes.contains(c)) else {
            continue;
        };
        let initial = pair
            .stars()
            .map(|star| stars.stars()[usize::from(star.get())].initial_mass());
        out.push(CensusMember {
            timeline: timeline.clone(),
            accreting: accreting_marks(timeline, age, initial),
            xray: (class == CarvedClass::XrayBinary)
                .then(|| xray_marks(timeline, age, initial))
                .flatten(),
        });
    }
}
