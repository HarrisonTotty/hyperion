//! The merger classes' prior members (plan 11's test helpers; P11.T8.c–d): the engine's own
//! mergers of first-attempt grid pairs, against which a merger class's sampler is checked and plan
//! 15's P15.T10.b fits it.
//!
//! From generator version 31 a record's first attempt is the generator's, under its own
//! multiplicity and on its primary's kept kick, so a displaced record has none (`mergers_of`), as
//! the class census's (P15.T10.b's merger step).
//!
//! A merger falls in the source horizon, L + 2H ≈ 2.6 × 10⁵ years, of about one grid system in
//! 10⁶ for the stellar mergers and one in 10¹⁰ for the neutron-star mergers, so the members of
//! [`brute_force_class_members`](super::brute_force_class_members), whose pairs are in a class
//! now, would cost hours to days. The members here are instead every pair the engine merged at any
//! age it ran, each kept where [`carved_class`] at its own merger age, the epoch at which its
//! merger is in the horizon, is the class. Pair by pair the engine's merger is the same whenever
//! it is seen: a pair's draws and orbit do not depend on its age.
//!
//! What the age changes is which pairs are seen. A grid system of age a is in the sample if it
//! merged by then, T ≤ a, which for a component with age density f and survival S(T) = P(a ≥ T)
//! happens at the rate S(T); a merger in the horizon now needs a ∈ [T − H, T + L + H], at the
//! rate f(T) (L + 2H). So each member carries the weight f(T) ÷ S(T), its component's age hazard
//! at its merger age ([`MergerMember::weight`]), and the weighted members are the mergers in the
//! horizon, as the catalogue holds them, to the extent that the record's composition (which follows
//! its age) leaves its pair's merger alone.

use crate::galaxy::imf::{BandShares, MassBand};
use crate::galaxy::placement::{CellKey, SystemOrigin, SystemRecord, generate_cell};
use crate::galaxy::{Galaxy, Population};
use crate::id::Layer;
use crate::stellar::binary::carve::redraws;
use crate::stellar::binary::{BinaryState, BinaryTimeline, CarvedClass, SegmentKind, carved_class};
use crate::stellar::multiplicity::RedrawAttempt;
use crate::stellar::system::{SystemStars, draw_metallicity, grid_multiplicity};
use crate::tables::binary::CLASS_SHARES;
use crate::units::{SolarMasses, Years};

use super::Mix;
use super::members::{CensusPart, CensusRecords, CensusSite, census_cells};

/// One of the engine's mergers of a class: the grid record its pair came from, the pair's
/// timeline, its merger age T and its weight as a merger in the horizon.
#[derive(Debug, Clone, PartialEq)]
pub struct MergerMember {
    record: SystemRecord,
    timeline: BinaryTimeline,
    merger_age: Years,
    weight: f64,
    /// The pair's stars' initial masses, M☉, in the engine's order.
    initial: [SolarMasses; 2],
}

impl MergerMember {
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

    /// The pair's merger age T, Julian years since the stars' onset of collapse.
    #[must_use]
    pub const fn merger_age(&self) -> Years {
        self.merger_age
    }

    /// Its weight as a merger in the source horizon now: its component's age density at T over
    /// the share of the component's born systems at least T old (module documentation).
    #[must_use]
    pub const fn weight(&self) -> f64 {
        self.weight
    }

    /// The pair's stars' initial masses, in the engine's order.
    #[must_use]
    pub const fn initial_masses(&self) -> [SolarMasses; 2] {
        self.initial
    }

    /// The pair before T, as [`carved_class`] reads it: its state halfway through the last
    /// segment that began before T and was not the merged star's.
    ///
    /// # Panics
    ///
    /// Never: a member's timeline merged after its first segment began.
    #[must_use]
    pub fn before(&self) -> BinaryState {
        let t = self.merger_age.value();
        let segment = self
            .timeline
            .segments()
            .iter()
            .rfind(|s| s.start().value() < t && s.kind() != SegmentKind::Merged)
            .expect("a pair that merged was a pair before");
        self.timeline
            .state_at(Years::new(f64::midpoint(segment.start().value(), t)))
    }

    /// The merged star at T, or nothing left (a phase of no remnant).
    #[must_use]
    pub fn product(&self) -> crate::stellar::StarState {
        self.timeline.state_at(self.merger_age).stars()[0]
    }
}

/// `n` of the engine's mergers of `class` near the solar circle (plan 11's test helpers): the
/// first attempt of every two-star pair of grid records, run through the engine with no carve-out,
/// kept where the pair merged and [`carved_class`] at its merger age is `class`, from the layers in
/// the shares [`CLASS_SHARES`] gives the class's hosts, on the cells of stream `stream`.
///
/// Each layer with a share s of the class's hosts contributes `round(n s)` members, from random
/// cells within 4,000 ly along x and 2,000 ly along y of the point 26,000 ly out along +y, up to
/// eight records a cell, as [`brute_force_class_members_in`](super::brute_force_class_members_in)
/// draws them.
///
/// # Panics
///
/// Never: the layers' cells are a few light-years to 128 ly, and the cells it opens lie inside the
/// root cube.
#[must_use]
pub fn brute_force_merger_members_in(
    galaxy: &Galaxy,
    class: CarvedClass,
    n: usize,
    stream: u64,
) -> Vec<MergerMember> {
    let row = CLASS_SHARES.row(class);
    let mut members = Vec::with_capacity(n + 8);
    let mut cell = Vec::new();
    for (index, band) in (0_u64..).zip(MassBand::ALL) {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss,
            reason = "a member count, rounded, of a test's few hundred"
        )]
        let wanted = (n as f64 * row.layer_share(band)).round() as usize;
        if wanted == 0 {
            continue;
        }
        let layer = Layer::from(band);
        let mut mix = Mix(0x0b17_08c0 ^ (stream << 8) ^ index);
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
                mergers_of(galaxy, record, class, &mut members);
            }
        }
        members.truncate(start + wanted);
    }
    members
}

/// What a merger census found ([`merger_census_in`]): the records it ran, its members as its
/// reader made them, in the order met, and the sums of their weights and of their squares, which
/// give its Kish effective size.
#[derive(Debug, Clone, PartialEq)]
pub struct MergerCensus<T> {
    scanned: [u64; 7],
    members: Vec<T>,
    weights: (f64, f64),
}

impl<T> MergerCensus<T> {
    /// The grid records the census ran of `population`, its layers together.
    #[must_use]
    pub const fn scanned(&self, population: Population) -> u64 {
        self.scanned[population.index()]
    }

    /// The grid records the census ran, every population's and its layers together.
    #[must_use]
    pub fn scanned_total(&self) -> u64 {
        self.scanned.iter().sum()
    }

    /// The members, as the census's reader made them, in the order met.
    #[must_use]
    pub fn members(&self) -> &[T] {
        &self.members
    }

    /// The members, taken out of the census.
    #[must_use]
    pub fn into_members(self) -> Vec<T> {
        self.members
    }

    /// The members' Kish effective size, (Σ w)² ÷ Σ w², their weights [`MergerMember::weight`]'s;
    /// 0 for none.
    #[must_use]
    pub fn kish_size(&self) -> f64 {
        let (sum, squares) = self.weights;
        if squares > 0.0 {
            sum * sum / squares
        } else {
            0.0
        }
    }

    /// This census and `other` together, `other`'s members after this one's.
    #[must_use]
    pub fn joined(mut self, other: Self) -> Self {
        for (mine, theirs) in self.scanned.iter_mut().zip(other.scanned) {
            *mine += theirs;
        }
        self.members.extend(other.members);
        self.weights = (
            self.weights.0 + other.weights.0,
            self.weights.1 + other.weights.1,
        );
        self
    }
}

/// The bands whose layers a merger census of `class` runs: every stellar band for the stellar
/// mergers, since a stellar merger's primary may be of any (the dynamical main sequence's donors,
/// 0.1–0.7 M☉, are layers A's and B's), where the class census runs only layers C–E; and layers D
/// and E for the neutron-star mergers, whose primaries collapse (D for an electron capture), so
/// that a census's records are spent where its members are.
///
/// # Panics
///
/// If `class` is not a merger class.
#[must_use]
pub fn merger_census_bands(class: CarvedClass) -> &'static [MassBand] {
    match class {
        CarvedClass::StellarMerger => &MassBand::ALL,
        CarvedClass::NeutronStarMerger => &[MassBand::D, MassBand::E],
        CarvedClass::AccretingWdFast | CarvedClass::AccretingWdSlow | CarvedClass::XrayBinary => {
            panic!("a merger census is of a merger class: {class:?}")
        }
    }
}

/// What a merger census runs ([`merger_census_in`]): its site's cells of its bands' layers, the
/// populations whose records it takes, and how many records, its bands together.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MergerCensusPlan<'a> {
    /// The bands whose layers it runs ([`merger_census_bands`]'s for its class).
    pub bands: &'a [MassBand],
    /// The site whose cells it takes.
    pub site: CensusSite,
    /// The populations whose records it takes.
    pub whose: CensusRecords,
    /// The records it runs, its bands together, each band's in its share of the bands' systems.
    pub records: u64,
}

/// A census of the engine's mergers of `class` among `plan`'s records of its bands' layers at its
/// site, of its populations, from the cells of `part` (plan 15's P15.T10.b, the merger classes' reference; the bands
/// [`merger_census_bands`]'s): each layer's records in its band's share of the bands' systems, from cells of its own
/// met once each in [`class_census_in`](super::class_census_in)'s order, each record's first attempt run through the
/// engine as the generator draws it (`mergers_of`), and each pair it merged into `class` kept,
/// weighted by its component's age hazard at its merger age ([`MergerMember::weight`]). `read`
/// makes each member what the census keeps of it as it is found: a merger's timeline is some
/// 90 kB, so a census of tens of thousands keeps their marks, not their timelines.
///
/// Two parts of one count share no cell, so they are independent samples, as the class census's
/// are; a part whose cells run out before it has a layer's records stops there.
///
/// # Panics
///
/// If the site's cells leave the root cube.
pub fn merger_census_in<T>(
    galaxy: &Galaxy,
    class: CarvedClass,
    plan: MergerCensusPlan<'_>,
    part: CensusPart,
    mut read: impl FnMut(MergerMember) -> T,
) -> MergerCensus<T> {
    let MergerCensusPlan {
        bands,
        site,
        whose,
        records,
    } = plan;
    let shares = BandShares::of(galaxy.mass_function());
    let in_bands = bands
        .iter()
        .fold(0.0, |sum, &band| sum + shares.share(band));
    let mut census = MergerCensus {
        scanned: [0; 7],
        members: Vec::new(),
        weights: (0.0, 0.0),
    };
    let mut cell = Vec::new();
    let mut found = Vec::new();
    for &band in bands {
        #[expect(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a record count under 2⁵³, rounded to a whole count"
        )]
        let in_band = (records as f64 * shares.share(band) / in_bands).round() as u64;
        let mut ran = 0_u64;
        for key in census_cells(site, Layer::from(band), part) {
            if ran >= in_band {
                break;
            }
            generate_cell(galaxy, key, &mut cell);
            for record in &cell {
                if ran == in_band {
                    break;
                }
                if !whose.takes(record.population()) {
                    continue;
                }
                census.scanned[record.population().index()] += 1;
                ran += 1;
                mergers_of(galaxy, record, class, &mut found);
                for member in found.drain(..) {
                    let w = member.weight;
                    census.weights = (census.weights.0 + w, census.weights.1 + w * w);
                    census.members.push(read(member));
                }
            }
        }
    }
    census
}

/// The mergers of `class` among the first attempt's pairs of grid record `record`.
///
/// From 31 they are the pairs of the system the generator draws there, before any redraw
/// (`SystemStars::generate_at`), under the record's own multiplicity ([`grid_multiplicity`]) and on
/// its primary's model as every reader takes it, the kept attempt of plan 08's kick loop for a
/// record that constrains its kick, as the class census's members are; a record whose system the
/// carve-out never redraws ([`redraws`]), a displaced one, which the generator shows single, has
/// none (P15.T10.b; train 27's finding). To 30 they were the first attempt's pairs under `Free`
/// multiplicity on the primary's own draws.
fn mergers_of(
    galaxy: &Galaxy,
    record: &SystemRecord,
    class: CarvedClass,
    out: &mut Vec<MergerMember>,
) {
    let SystemOrigin::Grid(component) = record.origin() else {
        return;
    };
    let ctx = grid_multiplicity(record);
    if !redraws(galaxy, record, ctx) {
        return;
    }
    let stars = SystemStars::generate_at(
        galaxy,
        record,
        &draw_metallicity(galaxy, record),
        ctx,
        RedrawAttempt::FIRST,
    );
    let ages = galaxy.fields().component(component).ages();
    for pair in stars.pairs() {
        let timeline = pair.timeline();
        let Some(t) = timeline.merger_age() else {
            continue;
        };
        if carved_class(timeline, t) != Some(class) {
            continue;
        }
        let survival = 1.0 - ages.cdf(t);
        if survival <= 0.0 {
            continue;
        }
        out.push(MergerMember {
            record: *record,
            timeline: timeline.clone(),
            merger_age: t,
            weight: ages.pdf(t) / survival,
            initial: pair
                .stars()
                .map(|star| stars.stars()[usize::from(star.get())].initial_mass()),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::POPULATIONS;
    use crate::stellar::system::PairTimeline;

    /// The fits' census galaxy (`hyperion-fit`'s binary manifests) at generator version 31's
    /// rules, at which the merger step is held (P15.T10.b).
    fn census_galaxy() -> Galaxy {
        Galaxy::from_params_for_binary_refit(
            crate::rng::Seed::new(0x0b17_15a0_0000_0001),
            crate::galaxy::params::GalaxyParams::milky_way_like(),
            crate::GeneratorVersion::new(31),
        )
        .expect("the Milky Way fixture builds")
    }

    /// The grid record of ID `raw` in `galaxy`.
    fn grid_record(galaxy: &Galaxy, raw: u64) -> SystemRecord {
        let id = crate::id::SystemId::from_raw(raw).expect("a grid ID");
        let mut cell = Vec::new();
        generate_cell(galaxy, CellKey::of(id).expect("a grid cell"), &mut cell);
        *cell
            .iter()
            .find(|record| record.id() == id)
            .expect("the cell holds its record")
    }

    /// From 31 a merger census's mergers of a record are those of its pairs at the generator's
    /// first attempt, as the class census's members are (P15.T10.b; train 27's finding): under the
    /// record's own multiplicity, so that a displaced record, which the generator shows single,
    /// has none, and on its primary's kept attempt of plan 08's kick loop. On the fits' census
    /// galaxy the census took the displaced remnant `0x81d8b2e000000008` and the retained
    /// `0x81d8b2e000000002` for stellar mergers their primaries' own draws make; the retained
    /// `0x81d8b2e000000001` is a stellar merger and `0x81dbb2e00000000d` a neutron-star merger at
    /// the generator's first attempt too.
    #[test]
    fn from_31_a_record_s_mergers_are_its_pairs_at_the_generator_s_first_attempt() {
        let galaxy = census_galaxy();
        let mut generated_mergers = 0;
        for (raw, class) in [
            (0x81d8_b2e0_0000_0008, CarvedClass::StellarMerger),
            (0x81d8_b2e0_0000_0002, CarvedClass::StellarMerger),
            (0x81d8_b2e0_0000_0001, CarvedClass::StellarMerger),
            (0x81db_b2e0_0000_000d, CarvedClass::NeutronStarMerger),
        ] {
            let record = grid_record(&galaxy, raw);
            let mut found = Vec::new();
            mergers_of(&galaxy, &record, class, &mut found);
            let stars = SystemStars::generate_at(
                &galaxy,
                &record,
                &draw_metallicity(&galaxy, &record),
                grid_multiplicity(&record),
                RedrawAttempt::FIRST,
            );
            let generated: Vec<&BinaryTimeline> = stars
                .pairs()
                .iter()
                .map(PairTimeline::timeline)
                .filter(|timeline| {
                    timeline
                        .merger_age()
                        .is_some_and(|t| carved_class(timeline, t) == Some(class))
                })
                .collect();
            let census: Vec<&BinaryTimeline> = found.iter().map(MergerMember::timeline).collect();
            assert!(
                census == generated,
                "{raw:#x}: the census's {} mergers against the generator's {}",
                census.len(),
                generated.len()
            );
            generated_mergers += generated.len();
        }
        assert_eq!(
            generated_mergers, 2,
            "the last two records keep their mergers"
        );
    }

    /// A merger census of two parts of one count meets cells of its parts alone, counts the
    /// records of the populations it takes by population, and gives its members' Kish size;
    /// joined, the parts are each one's members in order.
    #[test]
    fn a_merger_census_s_parts_share_no_cell_and_count_their_records() {
        let galaxy = census_galaxy();
        let class = CarvedClass::StellarMerger;
        let plan = MergerCensusPlan {
            bands: merger_census_bands(class),
            site: CensusSite::new([0, 26_000, 0], [600, 600, 300]),
            whose: CensusRecords::All,
            records: 3_000,
        };
        let parts = [0, 1].map(|k| {
            merger_census_in(&galaxy, class, plan, CensusPart::new(k, 2), |m| {
                (m.record().id(), m.weight())
            })
        });
        for census in &parts {
            let records: u64 = POPULATIONS.iter().map(|&p| census.scanned(p)).sum();
            assert_eq!(records, census.scanned_total());
            let (sum, squares) = census
                .members()
                .iter()
                .fold((0.0, 0.0), |(s, q), (_, w)| (s + w, q + w * w));
            assert!((census.kish_size() - sum * sum / squares).abs() < 1e-9 * census.kish_size());
        }
        let cells = |k: usize| -> std::collections::BTreeSet<CellKey> {
            parts[k]
                .members()
                .iter()
                .map(|(id, _)| CellKey::of(*id).expect("a grid record"))
                .collect()
        };
        assert!(cells(0).is_disjoint(&cells(1)));
        assert!(!parts[0].members().is_empty() && !parts[1].members().is_empty());
        let [a, b] = parts;
        let (n, m) = (a.members().len(), b.members().len());
        let joined = a.clone().joined(b.clone());
        assert_eq!(joined.members().len(), n + m);
        assert_eq!(&joined.members()[..n], a.members());
        assert_eq!(&joined.members()[n..], b.members());
        assert_eq!(
            joined.scanned_total(),
            a.scanned_total() + b.scanned_total()
        );
    }

    /// The readings of the pinned stellar and neutron-star mergers are the classes' marks: the
    /// ranks in [0, 1], the layer the primary's band, a mass ratio of a companion no heavier than
    /// its primary, and a neutron-star merger's compact stars.
    #[test]
    fn the_pinned_mergers_readings_are_their_classes_marks() {
        use super::super::{NsmReading, StellarMergerReading};
        let galaxy = census_galaxy();
        for (raw, class) in [
            (0x81d8_b2e0_0000_0001, CarvedClass::StellarMerger),
            (0x81db_b2e0_0000_000d, CarvedClass::NeutronStarMerger),
        ] {
            let record = grid_record(&galaxy, raw);
            let mut found = Vec::new();
            mergers_of(&galaxy, &record, class, &mut found);
            let member = found.first().expect("the pinned record merges");
            match class {
                CarvedClass::StellarMerger => {
                    let reading = StellarMergerReading::of(&galaxy, member).unwrap();
                    assert!((0.0..=1.0).contains(&reading.age_rank));
                    assert!(reading.layer.lo() <= reading.primary_mass);
                    assert!(reading.primary_mass < reading.layer.hi());
                    assert!(
                        reading.mass_ratio > 0.0 && reading.mass_ratio <= 1.0,
                        "{reading:?}"
                    );
                    assert!((reading.weight - member.weight()).abs() < 1e-300);
                }
                CarvedClass::NeutronStarMerger => {
                    let reading = NsmReading::of(&galaxy, member).unwrap();
                    assert!(reading.kind.is_some(), "{reading:?}");
                    assert!(reading.neutron_star_masses[0].is_some());
                    for rank in reading
                        .neutron_star_progenitor_ranks(&galaxy)
                        .into_iter()
                        .flatten()
                        .chain(reading.black_hole_progenitor_rank(&galaxy))
                    {
                        assert!((0.0..=1.0).contains(&rank));
                    }
                }
                CarvedClass::AccretingWdFast
                | CarvedClass::AccretingWdSlow
                | CarvedClass::XrayBinary => unreachable!("a merger class"),
            }
        }
    }
}
