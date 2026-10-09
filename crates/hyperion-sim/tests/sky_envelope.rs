//! The brightness envelope against dense tracks and against pair-evolved systems, and the premise
//! of the census's n × F₁ bound (rendering plan R06, R06.T6.b and T16.b; Design notes 8 and 10).
//! Slow.
//!
//! - `envelope_bounds_dense_tracks`: for 10⁴ masses drawn in each layer's band, at a metallicity
//!   and a Reimers η drawn across the envelope's span (\[Fe/H\] −2.3 to +0.4, η within ±3.5σ), and
//!   at ages drawn across 10⁴ years to 13.5 Gyr, no star is brighter in V than the envelope, and
//!   none uses more than 0.1 mag of its 0.3 mag margin. It reads the checked-in table,
//!   `tables::sky_envelope`, through `BrightnessEnvelope::build`.
//! - `envelope_bounds_pair_states` (R06.T16.b): over 10⁴ realised multiple systems, a thousand of
//!   each stellar layer near the Sun, where the thin disc holds young systems, and a thousand in
//!   the old bulge, at the epoch and 900 years before it, no star of the pair-evolved
//!   `SystemStars::state_at(t).stars()` is brighter in V than the envelope at `max_star_mass` of
//!   its primary over ages from zero to the system's, its 0.3 mag margin included. Those are the
//!   bound and the ages the census takes for a multiple system, star by star, without its factor
//!   for the star count.
//! - `the_fitted_envelope_is_the_build_rounded_brighter`: the checked-in table is the envelope
//!   [`BrightnessEnvelope::build_with`] builds from the tracks today, each value rounded brighter
//!   by `to_millimag`, so a change to the tracks that the fit's fingerprint misses still fails here.
//!
//! The plan's second test, that V never falls with mass along the early phases at a fixed age (the
//! premise of Design note 10's n × F₁ bound), was dropped (decided 2026-10-03): the premise is false
//! in V by up to at least 0.38 mag on the pre-main sequence, and since the multiple-system ruling
//! (2026-10-02, item 2) n × F₁ is used only for single stars, where it is the star's own flux.

use hyperion_sim::Seed;
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::imf::MassBand;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{CellKey, SystemRecord, generate_cell};
use hyperion_sim::id::{Layer, SystemId};
use hyperion_sim::math;
use hyperion_sim::sky::envelope::{
    BrightnessEnvelope, FE_H_NODES, MARGIN_MAG, SAMPLES_PER_PHASE, from_millimag, max_star_mass,
    to_millimag,
};
use hyperion_sim::sky::photometry::absolute_v_of_state;
use hyperion_sim::stellar::Composition;
use hyperion_sim::stellar::draws::{StandardNormal, StarDraws, StarDrawsParts};
use hyperion_sim::stellar::sse::{MIN_INITIAL_MASS, Track};
use hyperion_sim::stellar::substellar;
use hyperion_sim::stellar::system::SystemStars;
use hyperion_sim::time::{Span, UniverseTime};
use hyperion_sim::units::{Dex, HeliumExcess, Magnitudes, SolarMasses, Years};
use hyperion_testkit::lcg::Lcg;

/// Masses per layer.
const MASSES: u32 = 10_000;

/// Ages sampled per track.
const AGES: u32 = 24;

#[test]
#[ignore = "slow: builds 5 × 10^4 tracks"]
fn envelope_bounds_dense_tracks() {
    let galaxy = Galaxy::from_params(Seed::new(0x0926_0000), GalaxyParams::milky_way_like())
        .expect("the Milky Way-like parameters are valid");
    let envelope = BrightnessEnvelope::build(&galaxy);
    let component = galaxy.fields().component_ids().next().expect("a component");
    let mut lcg = Lcg::new(0x5e6_0006_b000_0001);
    let mut worst_use = f64::NEG_INFINITY;
    let mut violations: std::collections::BTreeMap<String, (u32, f64, String)> =
        std::collections::BTreeMap::new();
    for band in [
        MassBand::BrownDwarf,
        MassBand::A,
        MassBand::B,
        MassBand::C,
        MassBand::D,
        MassBand::E,
    ] {
        let layer = Layer::from(band);
        let (ln_lo, ln_hi) = (math::ln(band.lo()), math::ln(band.hi()));
        for _ in 0..MASSES {
            let m = math::exp(ln_lo + (ln_hi - ln_lo) * lcg.next_f64());
            let fe_h = -2.3 + 2.7 * lcg.next_f64();
            // From η = 0, no Reimers wind, the physical extreme, to +7σ.
            let eta = -0.5 / 0.07 + (7.0 + 0.5 / 0.07) * lcg.next_f64();
            let composition = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
            let ages: Vec<f64> = (0..AGES)
                .map(|_| {
                    math::exp(math::ln(1e4) + (math::ln(1.35e10) - math::ln(1e4)) * lcg.next_f64())
                })
                .collect();
            let states: Vec<_> = if m < MIN_INITIAL_MASS.value() {
                ages.iter()
                    .map(|&a| {
                        substellar::cooling(SolarMasses::new(m), Years::new(a), &composition)
                            .expect("inside the fits")
                    })
                    .collect()
            } else {
                let draws = StarDraws::from_parts(StarDrawsParts {
                    eta: StandardNormal::new(eta).expect("finite"),
                    ..StarDrawsParts::MEDIAN
                });
                let track = Track::to_age(
                    SolarMasses::new(m),
                    &composition,
                    &draws,
                    Years::new(1.35e10),
                );
                ages.iter()
                    .map(|&a| track.state_at(Years::new(a)))
                    .collect()
            };
            for (&age, state) in ages.iter().zip(&states) {
                let Some(v) = absolute_v_of_state(state) else {
                    continue;
                };
                let bound = envelope
                    .brightest(
                        layer,
                        component,
                        SolarMasses::new(m),
                        (Years::new(age), Years::new(age)),
                    )
                    .map_or(f64::INFINITY, Magnitudes::value);
                // How much of the margin the star uses: positive where it is brighter than the
                // envelope without its margin.
                let used = (bound + MARGIN_MAG) - v.value();
                worst_use = worst_use.max(used);
                if v.value() < bound || used > 0.1 {
                    let entry = violations
                        .entry(format!("{band:?} {:?}", state.phase()))
                        .or_insert((0, f64::NEG_INFINITY, String::new()));
                    entry.0 += 1;
                    if used > entry.1 {
                        entry.1 = used;
                        entry.2 = format!(
                            "m {m} [Fe/H] {fe_h:.2} η {eta:.2} age {age:.4e}: V {} against {bound}",
                            v.value()
                        );
                    }
                }
            }
        }
    }
    eprintln!(
        "the most margin used: {worst_use:.4} mag; {} violations",
        violations.len()
    );
    assert!(violations.is_empty(), "{violations:#?}");
}

/// Multiple systems per layer and place in `envelope_bounds_pair_states`.
const MULTIPLES: u32 = 1_000;

/// The records of `layer`'s cells about `at` (light-years), in shells of cells outward, until the
/// shell that holds the `n`th; the cells' own order within each shell.
fn records_about(galaxy: &Galaxy, layer: Layer, at: [f64; 3], n: usize) -> Vec<SystemRecord> {
    let size = f64::from(layer.cell_size_ly());
    let (mut out, mut cell) = (Vec::new(), Vec::new());
    for k in 0_i32.. {
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
                    let place = GalacticPosition::from_light_years(p).expect("in the cube");
                    let key = CellKey::containing(layer, &place).expect("in the cube");
                    generate_cell(galaxy, key, &mut cell);
                    out.append(&mut cell);
                }
            }
        }
        if out.len() >= n {
            return out;
        }
    }
    unreachable!("the shells grow without end")
}

/// The merger products pinned by `tests/sky_census.rs` (R06.T16.b), each brighter than the
/// envelope at its primary's own mass and age: two first-giant-branch stars and one on the main
/// sequence, near the Sun.
const PINNED_MERGERS: [u64; 3] = [
    0x21fe_5648_7ff0_0001,
    0x4204_6c99_ff00_000a,
    0x41fe_eca2_0000_0000,
];

/// What `envelope_bounds_pair_states` has seen of a group of systems, and every star above its
/// bound.
#[derive(Debug, Default)]
struct PairCheck {
    systems: u32,
    engine_run: u32,
    young: u32,
    old: u32,
    gained: u32,
    beyond_m1: u32,
    worst_use: Option<f64>,
    violations: Vec<String>,
}

impl PairCheck {
    /// Holds every star of `stars`, `record`'s system, at the epoch and 900 years before it, to
    /// the envelope at `max_star_mass` of its primary over ages from zero to the system's.
    fn check(&mut self, envelope: &BrightnessEnvelope, record: &SystemRecord, stars: &SystemStars) {
        let earlier = UniverseTime::EPOCH
            .checked_sub(Span::from_julian_years(900).expect("a span"))
            .expect("in the window");
        self.systems += 1;
        self.engine_run += u32::from(!stars.pairs().is_empty());
        let (layer, m1) = (record.layer(), record.primary_initial_mass());
        let component = record.component().expect("a grid record");
        for t in [UniverseTime::EPOCH, earlier] {
            let Some(state) = stars.state_at(t) else {
                continue;
            };
            let age = record.age_at(t);
            if t == UniverseTime::EPOCH {
                self.young += u32::from(age.value() < 1e9);
                self.old += u32::from(age.value() >= 5e9);
            }
            let bound = envelope
                .brightest(layer, component, max_star_mass(m1), (Years::ZERO, age))
                .map_or(f64::INFINITY, Magnitudes::value);
            let alone = envelope
                .brightest(layer, component, m1, (age, age))
                .map_or(f64::INFINITY, Magnitudes::value);
            for (i, star) in state.stars().iter().enumerate() {
                self.gained += u32::from(star.mass() > m1);
                let Some(v) = absolute_v_of_state(star) else {
                    continue;
                };
                let v = v.value();
                self.beyond_m1 += u32::from(v < alone);
                let used = bound + MARGIN_MAG - v;
                self.worst_use = Some(self.worst_use.map_or(used, |w| w.max(used)));
                if v < bound {
                    self.violations.push(format!(
                        "{:?} star {i} at {t:?}: {:?} of {:?} (m₁ {}, age {}): V {v} against \
                         {bound}",
                        record.id(),
                        star.phase(),
                        star.mass(),
                        m1.value(),
                        age.value()
                    ));
                }
            }
        }
    }

    fn report(&self, what: &str) {
        eprintln!(
            "{what}: {} systems ({} with a pair the engine ran; {} under 1 Gyr, {} over 5 Gyr); \
             {} star states above m₁, {} brighter than the envelope at m₁ and their age; the \
             most margin used {:?} mag",
            self.systems,
            self.engine_run,
            self.young,
            self.old,
            self.gained,
            self.beyond_m1,
            self.worst_use
        );
    }
}

#[test]
#[ignore = "slow: generates some 25,000 systems, a fifth of them of layer E"]
fn envelope_bounds_pair_states() {
    let galaxy = Galaxy::from_params(Seed::new(0x0926_0000), GalaxyParams::milky_way_like())
        .expect("the Milky Way-like parameters are valid");
    let envelope = BrightnessEnvelope::build(&galaxy);
    // Near the Sun the thin disc holds systems of every age from a few Myr; the bulge's are old.
    let places = [
        ("Sun", [0.0, 26_000.0, 68.0]),
        ("bulge", [0.0, 3_000.0, 0.0]),
    ];
    let mut multiples = 0_u32;
    let mut violations: Vec<String> = Vec::new();
    for layer in [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E] {
        for (name, at) in places {
            let mut group = PairCheck::default();
            let mut wanted = 3 * usize::try_from(MULTIPLES).expect("small");
            let mut records = records_about(&galaxy, layer, at, wanted);
            let mut next = 0;
            while group.systems < MULTIPLES {
                // A shell can overshoot twice the records asked for, so ask past what is held.
                while next >= records.len() {
                    wanted = (2 * wanted).max(records.len() + 1);
                    records = records_about(&galaxy, layer, at, wanted);
                }
                let record = &records[next];
                next += 1;
                let stars = SystemStars::generate(&galaxy, record);
                if stars.star_count() >= 2 {
                    group.check(&envelope, record, &stars);
                }
            }
            group.report(&format!("{layer:?} near the {name}, of {next} systems"));
            multiples += group.systems;
            violations.append(&mut group.violations);
        }
    }
    // The pinned mergers, which the random sample does not reach.
    let mut pinned = PairCheck::default();
    for raw in PINNED_MERGERS {
        let id = SystemId::from_raw(raw).expect("a system ID");
        let mut cell = Vec::new();
        generate_cell(&galaxy, CellKey::of(id).expect("a grid system"), &mut cell);
        let record = cell.iter().find(|r| r.id() == id).expect("in its cell");
        pinned.check(&envelope, record, &SystemStars::generate(&galaxy, record));
    }
    pinned.report("the pinned mergers");
    assert_eq!(
        pinned.beyond_m1, 6,
        "each pinned merger, at both times, beyond the bound at m₁"
    );
    violations.append(&mut pinned.violations);
    eprintln!(
        "{multiples} multiple systems; {} violations",
        violations.len()
    );
    assert!(multiples >= 10_000, "{multiples}");
    assert!(violations.is_empty(), "{violations:#?}");
}

#[test]
#[ignore = "slow: builds the envelope from some 9,000 tracks"]
fn the_fitted_envelope_is_the_build_rounded_brighter() {
    let galaxy = Galaxy::from_params(Seed::new(0x0926_0000), GalaxyParams::milky_way_like())
        .expect("the Milky Way-like parameters are valid");
    let fitted = BrightnessEnvelope::build(&galaxy);
    let built = BrightnessEnvelope::build_with(&FE_H_NODES, SAMPLES_PER_PHASE);
    let ((fitted_masses, fitted_rows), (masses, rows)) = (fitted.rows(), built.rows());
    assert_eq!(fitted_masses, masses);
    assert_eq!(fitted_rows.len(), rows.len());
    let mut moved = 0_u32;
    for (j, (fitted_row, row)) in fitted_rows.iter().zip(rows).enumerate() {
        for (k, (&stored, &v)) in fitted_row.iter().zip(row).enumerate() {
            let expected = to_millimag(v).map_or(f64::INFINITY, from_millimag);
            if stored.total_cmp(&expected).is_ne() {
                moved += 1;
                if moved <= 10 {
                    eprintln!(
                        "node {j} ({} M☉), bin {k}: table {stored}, build {v}",
                        masses[j]
                    );
                }
            }
        }
    }
    assert_eq!(
        moved, 0,
        "bins where the table is not the build rounded brighter"
    );
}
