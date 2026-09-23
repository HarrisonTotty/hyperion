//! System assembly: a system's metallicity draw, its stars' models, and the summaries and briefs
//! the server sends for a system and for each row of a range query (plan 06, phase G).
//!
//! This is the only part of the stellar stage that reads plan 03's placement: everything else is a
//! pure function of mass, composition, the star's draws and age.
//!
//! [`draw_metallicity`] (P06.T3) gives every star of a grid system one [`Composition`]: its
//! \[Fe/H\] is drawn once per system from the distribution plan 02's fields give the system's own
//! density component at its place and age (brainstorm, "Fields": metallicity "falls with galactic
//! radius … and, beyond about 8 Gyr, with age, with scatter"). The stars' models and the summaries
//! (P06.T29) come later.

use crate::galaxy::fields::Component;
use crate::galaxy::placement::SystemRecord;
use crate::galaxy::{Galaxy, PointLy};
use crate::rng::{ObjectKey, Stream, tags};
use crate::stellar::Composition;
use crate::units::{Dex, HeliumExcess, Years};

/// The composition every star of the grid system `record` shares: \[Fe/H\] drawn from its density
/// component's distribution at its epoch position and age, and no helium excess.
///
/// The component is `record`'s own ([`SystemRecord::component`]), and its
/// [`metallicity`](Component::metallicity) at the epoch position, read as a
/// [`PointLy`], and at the age at the epoch returns the normal distribution of \[Fe/H\] for that
/// population or halo component, place and age (plan 02, P02.T7.e, with rulings 7 and 21 of
/// 2026-09-22). The system's \[Fe/H\] is its mean plus its standard deviation times one standard
/// normal: two words of the stream `system.metallicity`, keyed by the system's ID
/// ([`tags::SYSTEM_METALLICITY`]). So it depends on the seed, the ID and the fields alone, and
/// neither on what else was generated nor on the time asked about: a system's metallicity is fixed
/// at its birth.
///
/// - A system not yet born at the epoch (a negative age, down to −H in the populations that still
///   form stars) reads its distribution at age zero.
/// - The helium excess is zero for every grid system (plan 06, design note 5); only plan 09's
///   cluster members carry one, and they bring their own [`Composition`].
/// - The function is for grid records, whose origin is
///   [`SystemOrigin::Grid`](crate::galaxy::placement::SystemOrigin::Grid) and whose
///   [`component`](SystemRecord::component) is therefore `Some`. A record without a component,
///   which no origin of this generator version builds, fails a debug assertion and in release
///   reads the first component of its population.
///
/// # Panics
///
/// If the record's component does not index `galaxy`'s fields, as a record placed in a galaxy
/// with more components would not ([`Fields::component`](crate::galaxy::fields::Fields::component)).
/// In debug builds also if the record has no component.
///
/// # Examples
///
/// Two systems of one cell at the solar circle draw their abundances independently from the
/// same thin-disc distribution, which is solar at the Sun's radius with a scatter of 0.2 dex:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::placement::{CellKey, generate_cell};
/// use hyperion_sim::id::Layer;
/// use hyperion_sim::stellar::system::draw_metallicity;
///
/// let galaxy = Galaxy::new(Seed::new(11));
/// let key = CellKey::new(Layer::C, [0, 812, 0])?;
/// let mut cell = Vec::new();
/// generate_cell(&galaxy, key, &mut cell);
/// for record in cell.iter().take(2) {
///     let composition = draw_metallicity(&galaxy, record);
///     // Within five standard deviations of a mean that lies within a few tenths of solar.
///     assert!(composition.fe_h().value().abs() < 1.5);
///     // The same system always draws the same abundance.
///     assert_eq!(composition, draw_metallicity(&galaxy, record));
/// }
/// # Ok::<(), hyperion_sim::galaxy::placement::BuildCellKeyError>(())
/// ```
#[must_use]
pub fn draw_metallicity(galaxy: &Galaxy, record: &SystemRecord) -> Composition {
    let distribution = component_of(galaxy, record)
        .metallicity(&PointLy::from(record.epoch_position()), age_read(record));
    let mut stream = Stream::open(
        galaxy.seed(),
        tags::SYSTEM_METALLICITY,
        ObjectKey::from(record.id()),
    );
    let fe_h = stream.normal(distribution.mean().value(), distribution.sigma().value());
    Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO)
}

/// The age at which a system reads its component's metallicity: its age at the epoch, or zero for
/// a system not yet born then.
#[must_use]
fn age_read(record: &SystemRecord) -> Years {
    let age = record.age_at_epoch();
    if age.value() < 0.0 { Years::ZERO } else { age }
}

/// The density component whose laws a grid record follows: its own, or for a record without one,
/// which no origin of this generator version builds, the first component of its population.
#[must_use]
fn component_of<'g>(galaxy: &'g Galaxy, record: &SystemRecord) -> &'g Component {
    let fields = galaxy.fields();
    debug_assert!(
        record.component().is_some(),
        "the metallicity draw is for grid records, which carry a component: {:?}",
        record.id()
    );
    match record.component() {
        Some(id) => fields.component(id),
        None => fields
            .components()
            .iter()
            .find(|c| c.population() == record.population())
            .expect("every population has at least one density component"),
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::coords::GalacticPosition;
    use crate::galaxy::Population;
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::placement::{CellKey, SystemOrigin};
    use crate::id::Layer;
    use crate::rng::Seed;
    use crate::units::SolarMasses;

    const SEED: u64 = 0x0600_0003_5eed_0001;

    fn galaxy() -> Galaxy {
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture's gas is mostly neutral")
    }

    /// A grid record of the young thin disc (component 0) at the Sun-like point with `age`.
    fn young_disc_record(galaxy: &Galaxy, index: u32, age: f64) -> SystemRecord {
        let key = CellKey::new(Layer::C, [0, 812, 0]).unwrap();
        SystemRecord::from_parts(
            key.candidate_id(index).unwrap(),
            GalacticPosition::from_light_years([0.0, 26_000.0, 30.0]).unwrap(),
            SystemOrigin::Grid(galaxy.fields().component_id(0).unwrap()),
            Population::YoungThinDisc,
            SolarMasses::new(1.0),
            Years::new(age),
        )
    }

    #[test]
    fn the_abundance_is_the_components_mean_plus_its_sigma_times_one_normal() {
        let galaxy = galaxy();
        let record = young_disc_record(&galaxy, 5, 2.0e7);
        let expected = galaxy
            .fields()
            .component(record.component().unwrap())
            .metallicity(&PointLy::new(0.0, 26_000.0, 30.0), Years::new(2.0e7));
        let mut stream = Stream::open(
            galaxy.seed(),
            tags::SYSTEM_METALLICITY,
            ObjectKey::from(record.id()),
        );
        let z = stream.standard_normal();
        let drawn = draw_metallicity(&galaxy, &record);
        assert_same_bits(
            drawn.fe_h().value(),
            expected.mean().value() + expected.sigma().value() * z,
        );
        assert_same_bits(
            drawn.z().value(),
            Composition::from_fe_h(drawn.fe_h(), HeliumExcess::ZERO)
                .z()
                .value(),
        );
        assert_same_bits(drawn.helium_excess().value(), 0.0);
    }

    #[test]
    fn a_system_not_yet_born_reads_its_distribution_at_age_zero() {
        let galaxy = galaxy();
        let unborn = young_disc_record(&galaxy, 9, -3.0e7);
        let newborn = young_disc_record(&galaxy, 9, 0.0);
        assert_eq!(age_read(&unborn), Years::ZERO);
        // Every law of this generator version is flat below 8 Gyr, so the draw itself cannot show
        // the rule yet; `age_read` above is what holds it.
        assert_eq!(
            draw_metallicity(&galaxy, &unborn),
            draw_metallicity(&galaxy, &newborn)
        );
        // An old system reads its own age.
        let old = young_disc_record(&galaxy, 9, 9.5e9);
        assert_same_bits(age_read(&old).value(), 9.5e9);
    }

    #[test]
    fn systems_draw_independently_and_the_same_system_draws_the_same_abundance() {
        let galaxy = galaxy();
        let records: Vec<SystemRecord> = (0..32)
            .map(|i| young_disc_record(&galaxy, i, 1.0e7))
            .collect();
        let draws: Vec<f64> = records
            .iter()
            .map(|r| draw_metallicity(&galaxy, r).fe_h().value())
            .collect();
        for (i, a) in draws.iter().enumerate() {
            for b in &draws[..i] {
                assert!(
                    a.total_cmp(b).is_ne(),
                    "two systems drew the same abundance"
                );
            }
        }
        hyperion_testkit::order::assert_order_independent(&records, |r| {
            draw_metallicity(&galaxy, r)
        });
    }
}
