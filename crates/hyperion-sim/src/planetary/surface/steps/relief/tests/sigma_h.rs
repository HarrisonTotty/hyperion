//! R09.T12.e's tests (`09`'s T12.e list as main's rulings of 2026-10-19 amend it): the structural
//! spectrum, the budget on the reconstructed field, the affine scale and the craters' bytes, the
//! sea, and the realised figures, on the reference worlds, each run once a test binary from the
//! staged relief its parent module keeps.
//!
//! Each check is a function of a reference world. The fast suite runs them on the reference Mars,
//! Moon and Ceres; the reference Earth's pass with R09.T12.e takes about 9 s natively, and since
//! nextest runs each test in a process of its own it would take that a test, so one slow test,
//! `the_reference_earth_holds_r09_t12e_s_tests`, runs every check on the Earth, with the tests that
//! only the Earth can hold (test 2's asserted band, test 4, its sea and its hypsometry).

use hyperion_base::rng::DetailSeed;
use hyperion_surface::craters::{CraterParams, CraterParamsParts, crater_shape};
use hyperion_surface::cube::{Face, PatchKey};
use hyperion_surface::field::{
    CoarseCrater, MaterialPalette, PaletteRole, SynthesisCell, boundary_diameter, cell_at_index,
    cell_index,
};
use hyperion_surface::spheroid::Spheroid;
use hyperion_surface::synth::channels::{ChannelBody, channel_variance};
use hyperion_surface::synth::craters::CraterOctaves;
use hyperion_surface::synth::craters::rims::{CoarseRims, kept_relief};
use hyperion_surface::synth::relief::{
    INTERIOR_AMPLITUDE, Relief, StructuralVariance, local_variance,
};
use hyperion_surface::synth::{BandSpectrum, Synthesiser};

use super::*;
use crate::planetary::surface::inputs::AreaShare;
use crate::planetary::surface::steps::craters::{self, Phase, smoothed_in_by};
use crate::planetary::surface::steps::relief::budget::{Match, Rule};
use crate::planetary::surface::steps::relief::quadrature::{
    Moments, NODES, Values, each_node, gauss_legendre,
};
use crate::planetary::surface::steps::relief::sea::Sea;
use crate::units::MetresPerSecondSquared;

/// The reference Moon, its plates and relief, computed once.
fn moon() -> &'static Relieved {
    static MOON: OnceLock<Relieved> = OnceLock::new();
    MOON.get_or_init(|| relieve(reference::moon_like()))
}

/// The reference Ceres, its plates and relief, computed once.
fn ceres() -> &'static Relieved {
    static CERES: OnceLock<Relieved> = OnceLock::new();
    CERES.get_or_init(|| relieve(reference::ceres_like()))
}

/// A reference world.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reference {
    Earth,
    Mars,
    Moon,
    Ceres,
}

/// The three worlds the fast suite holds every check on.
const CHEAP: [Reference; 3] = [Reference::Mars, Reference::Moon, Reference::Ceres];

/// The four, in the order R09's records list them.
const WORLDS: [Reference; 4] = [
    Reference::Earth,
    Reference::Mars,
    Reference::Moon,
    Reference::Ceres,
];

/// A reference world after the coarse craters and R09.T12.e's second entry, its stages kept.
struct Finished {
    relieved: &'static Relieved,
    /// The state after the coarse craters, before the entry.
    cratered: Working,
    matched: Match,
    sea: Sea,
    figures: budget::Figures,
    /// The state after the entry.
    working: Working,
    /// That state quantised.
    field: CoarseField,
}

impl Finished {
    fn pass(&self) -> Pass<'_> {
        self.relieved.world.pass(reference::SEED)
    }

    fn inputs(&self) -> &CoarseInputs {
        &self.relieved.world.inputs
    }

    fn grid(&self) -> &CellGraph {
        &self.relieved.world.grid
    }
}

/// The coarse craters and R09.T12.e's entry on `relieved`, as [`run_after_craters`] runs them,
/// which `the_staged_entry_is_the_entry` holds to it, with each stage's record.
fn finish(relieved: &'static Relieved) -> Finished {
    let pass = relieved.world.pass(reference::SEED);
    let mut working = relieved.relief.clone();
    craters::run(&pass, &mut working);
    let cratered = working.clone();
    working.spectrum = structural_spectrum(pass.inputs());
    let matched = budget::match_sigma_h(&pass, &mut working);
    let sea = sea::place(&pass, &mut working);
    let figures = budget::figures(&pass, &mut working, Phase::BeforeErosion, &matched.booked);
    let field =
        quantise(&relieved.world.inputs, working.clone()).expect("R09.T12.e's state quantises");
    Finished {
        relieved,
        cratered,
        matched,
        sea,
        figures,
        working,
        field,
    }
}

/// `world` after R09.T12.e, computed once a test binary.
fn finished(world: Reference) -> &'static Finished {
    static EARTH: OnceLock<Finished> = OnceLock::new();
    static MARS: OnceLock<Finished> = OnceLock::new();
    static MOON: OnceLock<Finished> = OnceLock::new();
    static CERES: OnceLock<Finished> = OnceLock::new();
    match world {
        Reference::Earth => EARTH.get_or_init(|| finish(earth())),
        Reference::Mars => MARS.get_or_init(|| finish(mars())),
        Reference::Moon => MOON.get_or_init(|| finish(moon())),
        Reference::Ceres => CERES.get_or_init(|| finish(ceres())),
    }
}

/// The reconstructed area mean and variance of the cells' `values` on `grid`, by the pass's
/// quadrature at `nodes` a side.
fn reconstructed(grid: &CellGraph, values: &[f64], nodes: usize) -> (f64, f64) {
    let field = Values::new(grid, values);
    let mut moments = Moments::new([quadrature::cell_mean(grid, values)]);
    each_node(grid, &gauss_legendre(nodes), |n, dir, w| {
        moments.add(w, [field.at(n, dir)]);
    });
    (moments.mean(0), moments.covariance(0, 0))
}

/// The cells' area-weighted variance of `values` on `grid`.
fn cell_variance(grid: &CellGraph, values: &[f64]) -> f64 {
    let mut moments = Moments::new([quadrature::cell_mean(grid, values)]);
    for (&v, &w) in values.iter().zip(grid.solid_angles_sr()) {
        moments.add(w, [v]);
    }
    moments.covariance(0, 0)
}

/// The coarse craters' summed lift on `world`'s cells, each crater of `listed` at its byte, those
/// smoothed in by the end of `phase`.
fn crater_lift(world: &Finished, listed: &[CoarseCrater], phase: Phase) -> Vec<f64> {
    let pass = world.pass();
    let inputs = world.inputs();
    let rims = CoarseRims::for_body(*inputs.craters(), inputs.figure(), pass.level());
    let mut summed = vec![0.0; world.working.cells.len()];
    for crater in listed {
        if smoothed_in_by(&pass, crater.age, phase) {
            craters::each_lift(world.grid(), &inputs.figure(), &rims, crater, |cell, l| {
                summed[slot(cell)] += l;
            });
        }
    }
    summed
}

/// The elevations of `working`'s cells.
fn elevations(working: &Working) -> Vec<f64> {
    working.cells.iter().map(|c| c.elevation.value()).collect()
}

/// The reconstructed `σ_h` of `world`'s finished field with the craters that wait for the erosion
/// step smoothed in as the solve counts them, √(`Var_recon` + L) over every crater: the whole
/// surface's once the late craters arrive, in this staging, which skips the erosion step and so
/// books no channels (§2.3.4's test 1). The whole pass's, the erosion's re-match and its re-routed
/// channels in, is [`the_reference_mars_s_whole_pass_realises_its_sigma_h`]'s, and a world wet
/// now's the erosion's own tests'.
fn whole_sigma_h(world: &Finished) -> f64 {
    let pass = world.pass();
    let mut working = world.working.clone();
    craters::run_after_erosion(&pass, &mut working);
    working.realised_sigma_h.value()
}

/// Test 1 on `world`: after the step the whole surface's reconstructed `σ_h` with every crater
/// in, the root of `Var_recon` + `S_s` + `K_w` + `R_inc` + `C_c`, is plan 14's to 10⁻⁶; and where
/// no crater waits for the erosion step the header's realised `σ_h` is it already.
fn assert_whole_sigma_h(world: Reference) {
    let f = finished(world);
    let target = f.inputs().sigma_h().value();
    assert_ne!(f.matched.rule, Rule::Unscaled, "{world:?}");
    let whole = whole_sigma_h(f);
    eprintln!(
        "{world:?}: the whole surface's σ_h {whole} m against {target} m, Δ {:.3e} relative",
        whole / target - 1.0
    );
    assert!(
        (whole / target - 1.0).abs() <= 1e-6,
        "{world:?}: {whole} m against {target} m"
    );
    let waiting = f
        .working
        .craters
        .iter()
        .any(|c| craters::waits(&f.pass(), c.age));
    assert_eq!(waiting, world == Reference::Mars, "{world:?}");
    if !waiting {
        let header = f.field.header().realised_sigma_h().value();
        assert!(
            (header / target - 1.0).abs() <= 1e-6,
            "{world:?}: {header} m against {target} m"
        );
    }
}

/// Test 2 on `world`: the local share finer than the coarse cell's Nyquist wavelength, ÷ `σ_h`²,
/// printed with its parts against its band ([`TEST_2_BANDS`]).
fn test_2_share(world: Reference) -> f64 {
    let f = finished(world);
    let sigma = f.inputs().sigma_h().value();
    let finer = f.matched.finer_after;
    let share = finer.total().value() / (sigma * sigma);
    eprintln!(
        "{world:?}: {:.2}% (structural {:.3}%, small craters {:.3}%, rims {:.3}%, channels \
         {:.3}%)",
        100.0 * share,
        100.0 * finer.structural.value() / (sigma * sigma),
        100.0 * finer.small_craters.value() / (sigma * sigma),
        100.0 * finer.sharpening.value() / (sigma * sigma),
        100.0 * finer.channels.value() / (sigma * sigma),
    );
    share
}

/// Test 5 on `world`: the realised structural share is ⟨a²⟩ `local_variance` to 10⁻¹² where no
/// cell is on a belt, and on the reference Earth exceeds it by 0–15%, by its belts' lift
/// (`decision-r09-t5.md` item 8).
fn assert_structural_share(world: Reference) {
    let f = finished(world);
    let local = local_variance(&f.working.spectrum, f.pass().level()).value();
    let before = f.matched.before;
    let ratio = before.structural_m2 / (before.amplitude_square * local);
    let belted = budget::records(f.inputs(), &f.working).any(|r| ReliefStyle::of(&r).belt() > 0.0);
    assert_eq!(belted, world == Reference::Earth, "{world:?}");
    if belted {
        assert!((1.0..=1.15).contains(&ratio), "{world:?}: {ratio}");
    } else {
        assert!((ratio - 1.0).abs() <= 1e-12, "{world:?}: {ratio}");
    }
}

/// Test 6 on `world`: the realised relief, the cells' range, lies at 6–16 `σ_h` (Design note 3's
/// observed 7–15, widened by one `σ_h` each way), and bounds the reconstructed range, which is
/// reported beside it. Main expected the reference Moon might miss (no large basin on this seed,
/// and the coarse profile's filleted rims); it held at 6.4 `σ_h` as built, 6.5 after R09.T7.i and
/// 6.9 after R09.T7.j, whose wear lowers its craters' relief and whose stretch of the non-crater
/// field (`s_N` 1.95) raises the rest.
fn assert_relief(world: Reference) {
    let f = finished(world);
    let sigma = f.inputs().sigma_h().value();
    let relief = f.working.realised_relief.value();
    let z = elevations(&f.working);
    let lowest = z.iter().copied().fold(f64::INFINITY, num::min);
    let highest = z.iter().copied().fold(f64::NEG_INFINITY, num::max);
    assert_same_bits(relief, highest - lowest);
    assert!(f.figures.reconstructed_range_m <= relief, "{world:?}");
    eprintln!(
        "{world:?}: relief {relief:.0} m, {:.2} σ_h; reconstructed range {:.0} m",
        relief / sigma,
        f.figures.reconstructed_range_m
    );
    assert!(
        (6.0..=16.0).contains(&(relief / sigma)),
        "{world:?}: {} σ_h",
        relief / sigma
    );
}

/// Test 7 on `world`: the ocean, the cells the flood took, covers at least the ocean fraction and
/// exceeds it by no more than the cells that joined at the final level, on the reference Earth;
/// and no cell is flooded, the sea level the lowest cell's elevation, where the fraction is zero.
fn assert_ocean(world: Reference) {
    let f = finished(world);
    let fraction = f.inputs().ocean_fraction();
    let sea = &f.sea;
    if world == Reference::Earth {
        assert!(sea.ocean_share >= fraction, "{sea:?}");
        assert!(
            sea.ocean_share - fraction <= sea.final_level_share,
            "{} over {fraction} past the final level's {}",
            sea.ocean_share,
            sea.final_level_share
        );
        assert!(sea.flooded_cells > 0);
    } else {
        assert_same_bits(fraction, 0.0);
        assert_eq!(sea.flooded_cells, 0, "{world:?}");
        assert!(
            f.working.cells.iter().all(|c| c.liquid_entry.is_none()
                && c.water_surface
                    .value()
                    .total_cmp(&c.elevation.value())
                    .is_eq()),
            "{world:?}"
        );
        let lowest = elevations(&f.working)
            .into_iter()
            .fold(f64::INFINITY, num::min);
        assert_same_bits(f.working.sea_level.value(), lowest);
    }
}

/// Test 8 on `world`: the header reports the realised `σ_h` and relief and carries the sea level
/// and the structural spectrum, the quantised field against the working state.
fn assert_header(world: Reference) {
    let f = finished(world);
    let header = f.field.header();
    assert_same_bits(
        header.realised_sigma_h().value(),
        f.working.realised_sigma_h.value(),
    );
    assert_same_bits(header.realised_sigma_h().value(), f.figures.sigma_h_m);
    assert_same_bits(
        header.realised_relief().value(),
        f.working.realised_relief.value(),
    );
    assert_same_bits(
        header.sea_level().value(),
        f.working.sea_level.value() + 0.0,
    );
    assert_same_bits(header.sea_level().value(), f.sea.level_m + 0.0);
    assert_eq!(*header.spectrum(), f.working.spectrum, "{world:?}");
    assert_eq!(
        *header.spectrum(),
        structural_spectrum(f.inputs()),
        "{world:?}"
    );
}

/// On `world`, after the scale every coarse crater's lift is its byte's kept relief times its
/// coarse profile at each reached cell, bit for bit, and each cell's elevation is μ + `s_N`
/// (N − μ) with the craters that do not wait smoothed in in list order, bit for bit (T7.c's one
/// profile); each byte is the nearest integer to 255 (1 − s\* g) where s\* ≤ 1, and R09.T12.d's
/// where s\* > 1.
fn assert_one_profile(world: Reference) {
    let f = finished(world);
    let pass = f.pass();
    let inputs = f.inputs();
    let figure = inputs.figure();
    let rims = CoarseRims::for_body(*inputs.craters(), figure, pass.level());
    let s_star = f.matched.whole_scale.unwrap();
    for (before, after) in f.cratered.craters.iter().zip(&f.working.craters) {
        let want = if f.matched.rule == Rule::Whole {
            (255.0 * (1.0 - s_star * kept_relief(before)))
                .round()
                .clamp(0.0, 255.0)
        } else {
            f64::from(before.degradation)
        };
        assert_same_bits(f64::from(after.degradation), want);
        let profile = rims.coarse_profile(after.diameter);
        let centre = figure.point(after.centre);
        let rim = 0.5 * after.diameter.value();
        let mut cells = after.reach.cells();
        craters::each_lift(f.grid(), &figure, &rims, after, |cell, lift| {
            assert_eq!(Some(cell), cells.next());
            let p = figure.point(f.grid().centre(cell));
            let gap = [p[0] - centre[0], p[1] - centre[1], p[2] - centre[2]];
            let chord = (gap[0] * gap[0] + gap[1] * gap[1] + gap[2] * gap[2]).sqrt();
            assert_same_bits(lift, kept_relief(after) * profile.at(chord / rim).height_m);
        });
    }
    // The elevation, rebuilt from the state before the entry.
    let placed = crater_lift(f, &f.cratered.craters, Phase::BeforeErosion);
    let (mean, s_n) = (f.matched.mean_m, f.matched.non_crater_scale);
    let mut z: Vec<f64> = f
        .cratered
        .cells
        .iter()
        .zip(&placed)
        .map(|(c, p)| mean + s_n * ((c.elevation.value() - p) - mean))
        .collect();
    for crater in f
        .working
        .craters
        .iter()
        .filter(|c| smoothed_in_by(&pass, c.age, Phase::BeforeErosion))
    {
        craters::each_lift(f.grid(), &figure, &rims, crater, |cell, l| {
            z[slot(cell)] = z[slot(cell)] + l;
        });
    }
    for (n, (cell, &want)) in f.working.cells.iter().zip(&z).enumerate() {
        assert_same_bits(cell.elevation.value(), want);
        assert!(
            cell.liquid_entry.is_some()
                || cell
                    .water_surface
                    .value()
                    .total_cmp(&cell.elevation.value())
                    .is_eq(),
            "{world:?} cell {n}"
        );
    }
}

/// The scale's rule and figures on `world`: s\* and `s_N` positive and finite, the rule s\*'s,
/// `s_N` within the bytes' rounding of s\* where s\* ≤ 1, and where s\* > 1 every crater keeps
/// its byte, R1's rule (the craters before the entry against after, in list order), the largest
/// of them positive so that the branch is not vacuous; and the second quadrature read exactly
/// where s\* ≤ 1 and the bytes before differ (`scale_bytes`' test). Reported beside main's
/// estimates (§2.2.9). Re-derived for R09.T7.j (`decision-r09-t7ij-refresh.md` Q1.1): as built
/// it asserted the largest byte 0 where s\* > 1, true only of R09.T12.d's fresh bytes, which T7.j's
/// wear makes positive on every cratered world.
fn assert_scale_rule(world: Reference) {
    let f = finished(world);
    let m = &f.matched;
    let s_star = m.whole_scale.unwrap();
    let s_n = m.non_crater_scale;
    eprintln!(
        "{world:?}: s* {s_star:.4}, s_N {s_n:.4}, rule {:?}, largest byte {}, T {:.4e} m², V_N \
         {:.4e}, K {:.4e}, V_C {:.4e}, R_inc {:.4e}",
        m.rule,
        m.largest_byte,
        m.target_m2,
        m.non_crater_m2,
        m.covariance_m2,
        m.craters_m2,
        m.before.increase_m2
    );
    assert!(s_star.is_finite() && s_star > 0.0 && s_n.is_finite() && s_n > 0.0);
    let before: Vec<u8> = f.cratered.craters.iter().map(|c| c.degradation).collect();
    let after: Vec<u8> = f.working.craters.iter().map(|c| c.degradation).collect();
    assert_eq!(before.len(), after.len(), "{world:?}");
    if s_star <= 1.0 {
        assert_eq!(m.rule, Rule::Whole);
        assert!(
            (s_n / s_star - 1.0).abs() < 0.02,
            "{world:?}: {s_n} against {s_star}"
        );
    } else {
        assert_eq!(m.rule, Rule::NonCrater);
        assert_eq!(after, before, "{world:?}");
        let largest = before.iter().copied().max().unwrap_or(0);
        assert_eq!(m.largest_byte, largest, "{world:?}");
        assert!(
            largest > 0,
            "{world:?}: every byte 0 where the craters keep theirs"
        );
    }
    let differ = before.windows(2).any(|pair| pair[0] != pair[1]);
    assert_eq!(
        m.second_quadrature,
        s_star <= 1.0 && differ,
        "{world:?}: s* {s_star}, bytes before differ: {differ}"
    );
}

/// The header-free closed forms are the field's on `world` (ruling R2): on its quantised field,
/// the seed-free structural variance from the working spectrum, level and radius is the field's
/// relief's point variance bit for bit; the small craters' `for_body` octaves are
/// `CraterOctaves::new` of the field's header; each sampled cell's channels' share from its record
/// is the field's, bit for bit; and on the sampled cells one coarse crater reaches, its sharpening
/// over its reach's disc is the field's rims' share to 10⁻¹².
fn assert_header_free_forms(world: Reference) {
    let f = finished(world);
    let header = f.field.header();
    let inputs = f.inputs();
    let pass = f.pass();
    let free = StructuralVariance::new(&f.working.spectrum, pass.level(), inputs.radius());
    let relief = Relief::new(header, DetailSeed::new(0x7431_3265));
    for a in [INTERIOR_AMPLITUDE, 0.5, 1.0, 1.7, 2.5] {
        for b in [0.0, 0.3, 1.0] {
            assert_same_bits(
                free.point_variance(a, b).value(),
                relief.point_variance(a, b).value(),
            );
        }
    }
    let d_b = boundary_diameter(pass.level(), inputs.radius());
    assert_eq!(
        CraterOctaves::for_body(
            *inputs.craters(),
            inputs.figure(),
            d_b,
            inputs.surface_age()
        ),
        CraterOctaves::new(header),
        "{world:?}"
    );
    let lambda = budget::Body::of(&pass, &f.working.spectrum).nyquist_wavelength;
    let body = ChannelBody {
        radius: inputs.radius(),
        level: pass.level().get(),
        gravity: inputs.craters().gravity(),
    };
    let rims = CoarseRims::new(header);
    let synthesiser = Synthesiser::new(&f.field, DetailSeed::new(0x7431_3265));
    let mut one_crater = 0;
    let every = if world == Reference::Earth { 97 } else { 7 };
    for n in (0..f.grid().len()).step_by(every) {
        let key = cell_at_index(pass.level().get(), n).unwrap();
        let field_side = synthesiser.unresolved_variance(key, lambda).unwrap();
        let record = &f.field.synthesis()[slot(n)];
        assert_same_bits(
            channel_variance(record, body, lambda).value(),
            field_side.channels().value(),
        );
        let reaching: Vec<&CoarseCrater> = f.field.craters_reaching(key).collect();
        if let [crater] = reaching[..] {
            let shape = crater_shape(header.craters(), crater.diameter);
            let rim_m = 0.5 * crater.diameter.value();
            let reach = (1.0 + shape.exterior_width()) * rim_m;
            let form =
                rims.sharpening_moment_m4(crater, lambda) / (core::f64::consts::PI * reach * reach);
            let field_rims = field_side.rims().value();
            assert!(
                (form - field_rims).abs() <= 1e-12 * field_rims.abs(),
                "{world:?} cell {n}: {form} against {field_rims}"
            );
            one_crater += 1;
        }
    }
    if world != Reference::Earth {
        assert!(one_crater > 0, "{world:?}");
    }
}

/// `C_c`, the channels' share, is +0 whole and finer than `λ_c` on `world` before the erosion step,
/// every cell's flow terminal (§2.3.4: computed anyway, for R09.T14.d's second match).
fn assert_no_channels(world: Reference) {
    let m = &finished(world).matched;
    for share in [
        m.before.channels_m2,
        m.after.channels_m2,
        m.finer_before.channels.value(),
        m.finer_after.channels.value(),
        m.booked.channels_m2,
        m.booked.channels_finer_m2,
    ] {
        assert_same_bits(share, 0.0);
    }
}

/// On `world`, the figures after the entry read the match's book as a fresh book of the finished
/// state would give them, bit for bit: neither the scale nor the sea changes a code the book reads
/// (the determinism audit's consider).
fn assert_fresh_book(world: Reference) {
    let f = finished(world);
    let pass = f.pass();
    let mut working = f.working.clone();
    let fresh = budget::book(&pass, &working, &budget::Body::of(&pass, &working.spectrum));
    assert_eq!(fresh, f.matched.booked, "{world:?}");
    let figures = budget::figures(&pass, &mut working, Phase::BeforeErosion, &fresh);
    assert_eq!(figures, f.figures, "{world:?}");
    assert_eq!(working, f.working, "{world:?}");
}

/// The staged second entry of `finish` is [`run_after_craters`], bit for bit, on the three worlds
/// cheaper than the Earth (the Earth's runs the same code).
#[test]
fn the_staged_entry_is_the_entry() {
    for world in CHEAP {
        let staged = finished(world);
        let pass = staged.pass();
        let mut working = staged.cratered.clone();
        run_after_craters(&pass, &mut working);
        assert_eq!(working, staged.working, "{world:?}");
    }
}

/// Test 1 on the reference Mars, Moon and Ceres, the reference Mars's 81 craters younger than its
/// wet epoch's end among them (ruling R4).
#[test]
fn the_whole_surface_s_sigma_h_is_plan_14_s() {
    for world in CHEAP {
        assert_whole_sigma_h(world);
    }
}

/// Test 1 through the whole pass on the reference Mars: its realised `σ_h` after the pass, the
/// craters younger than its wet epoch's end smoothed in after R09.T14's erosion and its second
/// match, is plan 14's to [`MARS_WHOLE_PASS_SIGMA_H_TOLERANCE`].
///
/// The erosion's second match books the channels' share `C_c` from the routed field before the
/// late craters, which `erosion::route_final` then re-routes: 108,176 m² booked, 111,126 m² after
/// them, so the realised `σ_h` lands 1.75 × 10⁻⁴ above plan 14's (measured 2026-10-20). The
/// ruled iteration on the predicted records (decision-r09-t14-choices, "T14.d follow-up" C)
/// would close it; under the owner's features-first direction of 2026-10-20 it is recorded as a
/// finding in plan 09's R09.T14 record instead, the test asserting the gap's bound.
#[test]
fn the_reference_mars_s_whole_pass_realises_its_sigma_h() {
    let inputs = reference::mars_like();
    let field = crate::planetary::surface::coarse_pass(reference::SEED, &inputs);
    let realised = field.header().realised_sigma_h().value();
    let target = inputs.sigma_h().value();
    eprintln!(
        "the reference Mars's whole pass: σ_h {realised} m against {target} m, Δ {:.3e} relative",
        realised / target - 1.0
    );
    assert!(
        (realised / target - 1.0).abs() <= MARS_WHOLE_PASS_SIGMA_H_TOLERANCE,
        "{realised} m against {target} m"
    );
}

/// The reference Mars's whole pass's bound on its realised `σ_h` against plan 14's, relative:
/// 2 × 10⁻⁴, above the measured 1.75 × 10⁻⁴ of the late craters' re-routed channels (see
/// [`the_reference_mars_s_whole_pass_realises_its_sigma_h`]); the other worlds' whole passes hold
/// 10⁻⁶.
const MARS_WHOLE_PASS_SIGMA_H_TOLERANCE: f64 = 2e-4;

/// Test 2's bands, Design note 7's measured shares above `l_N` (the plan's own computation: 7, 5
/// and 0.6%): each world's band and whether it is asserted, under main's ruling R7 (asserted
/// where it holds, a miss printed with its parts, recorded and commented, never loosened).
const TEST_2_BANDS: [(Reference, (f64, f64), Band); 4] = [
    (Reference::Earth, (0.003, 0.01), Band::Asserted),
    // Mars holds on its band's edge after R09.T7.j (0.99%; main's 1.0–1.1%). If a later change
    // makes it miss, it moves to `Band::Reported` under R7, recorded; the band is never widened.
    (Reference::Mars, (0.003, 0.01), Band::Asserted),
    (Reference::Moon, (0.04, 0.10), Band::Asserted),
    // Ceres misses below its band after R09.T7.j (2.46%), as main foresaw (2.4–2.7%): its octaves
    // of 25–50 and 50–100 km are saturated (P ÷ S 1.01 and 1.32, E[g²] 0.264) and that of
    // 100–200 km just under the cap (0.93, E[g²] 0.281), so they take nearly the whole wear (the
    // brief estimated P ÷ S 0.85–0.96, E[g²] 0.27–0.30); and plan 14's `k_comp` is fitted to
    // Ceres's whole non-structural relief, not all of which need be craters (addendum 4, D4.4).
    // Recorded for P14.T48.a and R09.T12.d's re-validation (`09`'s deferred findings of
    // R09.T7.i–j).
    (Reference::Ceres, (0.03, 0.08), Band::Reported),
];

/// Whether a world's test-2 band is asserted or, where the world misses it, reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Band {
    Asserted,
    Reported,
}

/// Test 2 on `world` against its band ([`TEST_2_BANDS`]): asserted where it holds; a miss
/// printed with its parts.
fn assert_test_2_band(world: Reference) {
    let share = test_2_share(world);
    assert!(share.is_finite() && share > 0.0, "{world:?}: {share}");
    let &(_, (low, high), band) = TEST_2_BANDS
        .iter()
        .find(|(w, _, _)| *w == world)
        .expect("every reference world has a band");
    let holds = (low..=high).contains(&share);
    match band {
        Band::Asserted => assert!(holds, "{world:?}: {share} outside {low}–{high}"),
        Band::Reported => eprintln!(
            "{world:?}: test 2 misses its band, {:.2}% against {:.1}–{:.0}% (recorded)",
            100.0 * share,
            100.0 * low,
            100.0 * high
        ),
    }
}

/// Test 2 on the reference Mars, Moon and Ceres against Design note 7's bands (0.3–1% on Mars,
/// 4–10% on the Moon, 3–8% on Ceres), each asserted where it holds after R09.T7.j's wear and a
/// miss printed with its parts ([`TEST_2_BANDS`]); the Earth's in the Earth's slow test. As built
/// all three missed, the crater model's calibration (§2.2.9), which R09.T7.i–j's fixes address.
#[test]
fn the_local_share_finer_than_the_coarse_cell_is_in_its_band_where_it_holds() {
    for world in CHEAP {
        assert_test_2_band(world);
    }
}

/// The reference world's crater term in expectation, rendered at the band limit, and plan 14's
/// basin share, metres: `√(K_w + Σ_k e_k ⟨E_k[g²](D) ∫F(w₂)² dA⟩_k ÷ A)`, `K_w` the small
/// craters' whole share, `e_k` each coarse octave's expected count (R09.T12.d's `octaves`),
/// `E_k[g²](D)` the mean g² of its craters of diameter D by the closed form
/// (`OctaveWear::mean_square_relief`), `⟨·⟩_k` the mean over its diameters by the production's
/// distribution at eight Gauss–Legendre nodes in the diameter's share, as `CraterOctaves` takes
/// them, and A the body's area; and plan 14's `Relief::of` basin share for the world's crust,
/// gravity, radius and crater density. The brief wrote `E_k[g²] ⟨∫F(w₂)² dA⟩_k`: the two agree
/// where `E[g²]` is the same at every diameter (every lunar coarse octave, 0.2638), and the node
/// form is the exact expectation where it varies with D (Mars, Ceres).
fn crater_term_in_expectation(world: Reference) -> (f64, f64) {
    use crate::planetary::hooks::figures::relief::{Relief, ReliefInputs};
    use crate::planetary::hooks::figures::{Crust as BasinCrust, TectonicRegime};
    use crate::planetary::surface::steps::craters::tests::analytic_square;
    use crate::tables::gauss_legendre::{GL8_NODES, GL8_WEIGHTS};
    use hyperion_surface::craters::{UnitUniform, diameter_in_octave};
    use hyperion_surface::geometry::BAND_LIMIT_M;
    use hyperion_surface::synth::craters::CraterProfile;

    let world_inputs = match world {
        Reference::Earth => reference::earth_like(),
        Reference::Mars => reference::mars_like(),
        Reference::Moon => reference::moon_like(),
        Reference::Ceres => reference::ceres_like(),
    };
    let built = World::new(world_inputs);
    let pass = built.pass(reference::SEED);
    let inputs = pass.inputs();
    let params = inputs.craters();
    // K_w, the budget's: every small octave whole, at twice D_b.
    let d_b = boundary_diameter(pass.level(), inputs.radius());
    let small = CraterOctaves::for_body(*params, inputs.figure(), d_b, inputs.surface_age())
        .unresolved_variance(d_b * 2.0)
        .value();
    let mut coarse = 0.0;
    for octave in craters::octaves(&pass) {
        let mut mean = 0.0;
        for (&node, &weight) in GL8_NODES.iter().zip(&GL8_WEIGHTS) {
            let share =
                UnitUniform::new(f64::midpoint(1.0, node)).expect("a node's share lies in [0, 1)");
            let diameter = diameter_in_octave(params, octave.diameters, share);
            let rim = 0.5 * diameter.value();
            let ground = CraterProfile::filleted(
                crater_shape(params, diameter),
                Metres::new(0.5 * BAND_LIMIT_M),
            );
            let square_m4 = core::f64::consts::PI * rim * rim * analytic_square(&ground);
            mean += 0.5 * weight * octave.wear.mean_square_relief(Metres::new(rim)) * square_m4;
        }
        coarse += octave.expected * mean;
    }
    let area_m2 = craters::spheroid_area_m2(&inputs.figure());
    let term = (small + coarse / area_m2).sqrt();
    let crust = if world == Reference::Ceres {
        BasinCrust::IceRich
    } else {
        BasinCrust::Silicate
    };
    let basin = Relief::of(&ReliefInputs {
        regime: TectonicRegime::StagnantLid,
        continental_fraction: 0.0,
        crust,
        surface_gravity: inputs.gravity(),
        radius: inputs.radius(),
        crater_density: params.n_1km(),
        volcanism: 0.0,
        lithosphere: Metres::new(100e3),
    })
    .basin()
    .value();
    eprintln!(
        "{world:?}: craters in expectation, rendered, {term:.0} m (small {:.0} m, coarse {:.0} m) \
         against plan 14's basin share {basin:.0} m",
        small.sqrt(),
        (coarse / area_m2).sqrt()
    );
    (term, basin)
}

/// Plan 14's basin share against the reference Moon's crater term in expectation, rendered at the
/// band limit, which R09.T7.j's wear brings within 25% of it (main's estimate 1,883–2,020 m;
/// 3,671 m before; a check the law was not fitted to); Mars's and Ceres's printed beside plan
/// 14's 0.89 and 1.87 km. 2.05 km is LOLA's 2.239 km `σ_h` with the stagnant lid's 0.9 km removed
/// in quadrature (`BASIN_RELIEF`), so it holds the Moon's degree-1 shape and its nearside–farside
/// asymmetry too (a centre-of-figure offset of about 1.9 km, from memory): an upper reference for
/// the craters, which the 25% spans.
#[test]
fn the_moon_s_craters_in_expectation_are_plan_14_s_basin_share() {
    for world in CHEAP {
        let (term, basin) = crater_term_in_expectation(world);
        if world == Reference::Moon {
            assert!((basin - 2_050.0).abs() < 10.0, "{basin}");
            assert!(
                (term / basin - 1.0).abs() <= 0.25,
                "{term} m against {basin} m"
            );
        }
    }
}

/// Test 3: the craters, small and coarse, supply at least 80% of the reference Moon's local share
/// finer than its coarse cell.
#[test]
fn the_craters_supply_most_of_the_moon_s_local_share() {
    let finer = finished(Reference::Moon).matched.finer_after;
    let share = finer.craters().value() / finer.total().value();
    assert!(share >= 0.8, "{share}");
}

/// Test 5 on the reference Mars, Moon and Ceres, which have no belt.
#[test]
fn the_realised_structural_share_off_the_belts_is_the_styles_mean() {
    for world in CHEAP {
        assert_structural_share(world);
    }
}

/// Test 6 on the reference Mars, Moon and Ceres.
#[test]
fn the_realised_relief_is_six_to_sixteen_sigma_h() {
    for world in CHEAP {
        assert_relief(world);
    }
}

/// Test 7 on the reference Mars, Moon and Ceres: no cell flooded.
#[test]
fn no_cell_is_flooded_where_the_ocean_fraction_is_zero() {
    for world in CHEAP {
        assert_ocean(world);
    }
}

/// Test 8 on the reference Mars, Moon and Ceres.
#[test]
fn the_header_carries_the_figures_the_sea_level_and_the_spectrum() {
    for world in CHEAP {
        assert_header(world);
    }
}

/// The one profile after the scale on the reference Mars, Moon and Ceres.
#[test]
fn after_the_scale_each_crater_is_one_profile_at_its_byte() {
    for world in CHEAP {
        assert_one_profile(world);
    }
}

/// The scale's rule on the reference Mars, Moon and Ceres.
#[test]
fn the_scale_follows_its_rule() {
    for world in CHEAP {
        assert_scale_rule(world);
    }
}

/// The header-free forms on the reference Mars, Moon and Ceres.
#[test]
fn the_header_free_forms_are_the_field_s() {
    for world in CHEAP {
        assert_header_free_forms(world);
    }
}

/// `C_c` is zero on the reference Mars, Moon and Ceres.
#[test]
fn the_channels_share_is_zero_before_the_erosion() {
    for world in CHEAP {
        assert_no_channels(world);
    }
}

/// The figures read the match's book as a fresh one on the reference Mars, Moon and Ceres.
#[test]
fn the_figures_read_the_match_s_book_as_a_fresh_one() {
    for world in CHEAP {
        assert_fresh_book(world);
    }
}

/// Every check on the reference Earth, with the tests only it can hold: test 2's band, 0.3–1%
/// (Design note 7's 0.6%), asserted; test 4, ⟨a²⟩ 0.6–1.2 (`decision-r09-t5.md` item 1: Earth's
/// areas give 0.8–1.1); test 5's belts' lift; test 7's ocean of 0.7095; its one ocean through
/// quantisation; its hypsometry after the scale; the uniform cells' shortcut on its sparse fields;
/// and R09.T13.d's climate step on its sea.
#[test]
#[ignore = "slow: R09.T12.e's tests on the reference Earth, whose pass with T12.e takes about 9 s, \
            then its climate step"]
fn the_reference_earth_holds_r09_t12e_s_tests() {
    let world = Reference::Earth;
    assert_whole_sigma_h(world);
    assert_test_2_band(world);
    let a2 = finished(world).matched.before.amplitude_square;
    assert!((0.6..=1.2).contains(&a2), "{a2}");
    assert_structural_share(world);
    assert_relief(world);
    assert_ocean(world);
    assert_header(world);
    assert_one_profile(world);
    assert_scale_rule(world);
    assert_header_free_forms(world);
    assert_no_channels(world);
    assert_fresh_book(world);
    let f = finished(world);
    print_sea(world, f);
    assert_the_sea_holds(f.grid(), f.inputs(), &f.working, &f.field, &f.sea);
    assert_earth_hypsometry();
    let records: Vec<SynthesisCell> = budget::records(f.inputs(), &f.working).collect();
    let b: Vec<f64> = records.iter().map(|r| ReliefStyle::of(r).belt()).collect();
    let lift = crater_lift(f, &f.working.craters, Phase::AfterErosion);
    for values in [lift, b] {
        assert!(uniform_reads(f.grid(), &values, 97) > 0);
    }
    assert_the_climate_keeps_the_sea(world);
}

/// R09.T13.d's climate step on R09.T12.e's sea (decision-r09-t12-coarse R8): run on `world` after
/// the entry, it moves no ground, water surface or sea level, gives every flooded cell a liquid and
/// no dry cell one, and on a world of one liquid leaves each flooded cell the main liquid's entry.
/// What it placed is printed.
fn assert_the_climate_keeps_the_sea(world: Reference) {
    let f = finished(world);
    let pass = f.pass();
    let mut climate = f.working.clone();
    crate::planetary::surface::steps::climate::run(&pass, &mut climate);
    assert_same_bits(climate.sea_level.value(), f.working.sea_level.value());
    let one_liquid = f.inputs().liquids().len() == 1;
    let omega = f.grid().solid_angles_sr();
    let total: f64 = omega.iter().sum();
    let (mut sea, mut iced, mut sea_iced) = (0.0, 0.0, 0.0);
    for (n, ((was, now), &w)) in f
        .working
        .cells
        .iter()
        .zip(&climate.cells)
        .zip(omega)
        .enumerate()
    {
        assert_same_bits(now.elevation.value(), was.elevation.value());
        assert_same_bits(now.water_surface.value(), was.water_surface.value());
        let under = now.water_surface.value() > now.elevation.value();
        assert_eq!(now.liquid_entry.is_some(), under, "{world:?}, cell {n}");
        if one_liquid {
            assert_eq!(now.liquid_entry, was.liquid_entry, "{world:?}, cell {n}");
        }
        let ice = w * now.ice_fraction;
        sea += if under { w } else { 0.0 };
        iced += ice;
        sea_iced += if under { ice } else { 0.0 };
    }
    eprintln!(
        "{world:?} under the climate: sea {:.6} of the sphere, ice {:.6}, of it on the sea {:.6}",
        sea / total,
        iced / total,
        sea_iced / total
    );
}

/// The hypsometry after the scale: on the reference Earth, relative to its sea level, T12.a's three
/// bands still hold (`decision-r09-t12-coarse.md` item 1): the land mode within 300 m of +125 m,
/// the ocean's modal 250 m bin among the centres −5,125 to −4,125 m, and the mean below −2 km from
/// −4,731 to −3,981 m (Earth2014's −4,281 m with +300 m and −450 m).
fn assert_earth_hypsometry() {
    let f = finished(Reference::Earth);
    let sea = f.working.sea_level.value();
    let z: Vec<f64> = f
        .working
        .cells
        .iter()
        .map(|c| c.elevation.value() - sea)
        .collect();
    Bands::of(&z, f.grid().solid_angles_sr()).assert_earth2014_s("the scaled reference Earth");
}

/// A small Earth-like world, 500 km across a sphere, its ocean fraction `fraction` of water.
fn small_sea(fraction: f64) -> CoarseInputs {
    let earth = reference::earth_like();
    builder_of(&earth)
        .figure(Spheroid::sphere(5.0e5))
        .liquids([AreaShare {
            substance: earth.liquids()[0].substance,
            area_fraction: fraction,
        }])
        .build()
        .unwrap()
}

/// The whole pass's steps through R09.T12.e on `inputs`, with its sea's record.
fn through_the_sea(inputs: &CoarseInputs) -> (World, Working, Sea) {
    let world = World::new(inputs.clone());
    let pass = world.pass(reference::SEED);
    let mut working = Working::initial(&pass);
    crate::planetary::surface::steps::plates::run(&pass, &mut working);
    run(&pass, &mut working);
    craters::run(&pass, &mut working);
    working.spectrum = structural_spectrum(pass.inputs());
    let _ = budget::match_sigma_h(&pass, &mut working);
    let sea = sea::place(&pass, &mut working);
    (world, working, sea)
}

/// Test 7 on builder worlds: an ocean fraction of 0 floods nothing and puts the sea level at the
/// lowest cell; 1 floods every cell but the highest, the sea level its elevation; and 0.4 covers
/// the fraction, exceeding it by no more than the final level's cells, as one ocean.
#[test]
fn an_ocean_of_nothing_and_of_everything_on_builder_worlds() {
    let (_, working, sea) = through_the_sea(&small_sea(0.0));
    let z = elevations(&working);
    let lowest = z.iter().copied().fold(f64::INFINITY, num::min);
    assert_same_bits(working.sea_level.value(), lowest);
    assert_eq!(sea.flooded_cells, 0);
    assert!(working.cells.iter().all(|c| c.liquid_entry.is_none()));

    let (_, working, sea) = through_the_sea(&small_sea(1.0));
    let z = elevations(&working);
    let highest = z.iter().copied().fold(f64::NEG_INFINITY, num::max);
    assert_same_bits(working.sea_level.value(), highest);
    let tops = z.iter().filter(|&&h| h.total_cmp(&highest).is_eq()).count();
    assert_eq!(sea.flooded_cells, z.len() - tops);
    for (cell, &h) in working.cells.iter().zip(&z) {
        assert_eq!(cell.liquid_entry.is_some(), h < highest);
    }

    let inputs = small_sea(0.4);
    let (world, working, sea) = through_the_sea(&inputs);
    assert!(sea.ocean_share >= 0.4, "{}", sea.ocean_share);
    assert!(sea.ocean_share - 0.4 <= sea.final_level_share, "{sea:?}");
    let field = quantise(&world.inputs, working.clone()).unwrap();
    assert_the_sea_holds(&world.grid, &world.inputs, &working, &field, &sea);
}

/// The flooded cells of `field` (its water above its ground) are exactly those the working state
/// flooded, each naming the main liquid with its water at the sea level's code; no dry cell below
/// the sea level has a flooded edge neighbour, all on the field's codes; and the sea is one ocean:
/// every flooded cell is one `sea` took, reached from its seed through cells it took.
///
/// The ocean is joined through the cells the flood took, its shore at the sea level's code among
/// them, which stay dry under the fill by the codes, rather than through the flooded cells alone
/// (a test deviation recorded in the plan): a depression that joins at the sea level through a
/// saddle at exactly that level is flooded, and its saddle dry.
fn assert_the_sea_holds(
    grid: &CellGraph,
    inputs: &CoarseInputs,
    working: &Working,
    field: &CoarseField,
    sea: &Sea,
) {
    let main = inputs.main_liquid();
    let sea_mm = SynthesisCell::quantise_height(working.sea_level).unwrap();
    let cells = field.synthesis();
    let wet: Vec<bool> = cells
        .iter()
        .map(|c| c.water_surface_mm > c.elevation_mm)
        .collect();
    for (n, ((record, state), &w)) in cells.iter().zip(&working.cells).zip(&wet).enumerate() {
        assert_eq!(w, state.liquid_entry.is_some(), "cell {n}");
        if w {
            assert_eq!(record.liquid_entry(), main, "cell {n}");
            assert_eq!(record.water_surface_mm, sea_mm, "cell {n}");
            assert!(
                state.elevation.value() < working.sea_level.value(),
                "cell {n}"
            );
        } else {
            assert_eq!(record.water_surface_mm, record.elevation_mm, "cell {n}");
        }
    }
    for n in 0..grid.len() {
        if !wet[slot(n)] && cells[slot(n)].elevation_mm < sea_mm {
            for m in grid.edge_neighbours(n) {
                assert!(!wet[slot(m)], "dry {n} below the sea beside flooded {m}");
            }
        }
    }
    let mut taken = vec![false; wet.len()];
    for &c in &sea.taken {
        taken[slot(c)] = true;
    }
    let mut seen = vec![false; wet.len()];
    let mut frontier: Vec<u32> = sea.seed.clone();
    for &c in &frontier {
        assert!(taken[slot(c)], "seed cell {c} not taken");
        seen[slot(c)] = true;
    }
    while let Some(c) = frontier.pop() {
        for m in grid.edge_neighbours(c) {
            if taken[slot(m)] && !seen[slot(m)] {
                seen[slot(m)] = true;
                frontier.push(m);
            }
        }
    }
    for (n, &w) in wet.iter().enumerate() {
        assert!(
            !w || (taken[n] && seen[n]),
            "flooded cell {n} apart from the ocean"
        );
    }
}

/// A level-5 sphere of 500 km with the reference Earth's palette and ocean fraction, its elevation
/// hand-made: an ocean basin about one pole, an enclosed pit below the sea level about the other,
/// and every elevation within 0.2 mm of a whole 50 m step, so that the shore holds cells a
/// fraction of a millimetre below the sea level, whose code is the sea level's.
fn basin_world() -> (World, Working) {
    let inputs = builder_of(&reference::earth_like())
        .figure(Spheroid::sphere(5.0e5))
        .build()
        .unwrap();
    let world = World::new(inputs);
    let mut working = Working::initial(&world.pass(reference::SEED));
    let pit = [0.0, 0.0, -1.0];
    for (n, (cell, &centre)) in working
        .cells
        .iter_mut()
        .zip(world.grid.centres())
        .enumerate()
    {
        // Low towards +z, high towards −z, with a deep pit at −z; within a step, five offsets of
        // −0.2 to +0.2 mm, all of the step's one code.
        let class = u32::try_from(n % 5).unwrap();
        let offset = 1e-4 * f64::from(class) - 2e-4;
        let mut z = 50.0 * (80.0 * (0.2 - centre[2])).round() + offset;
        let gap = [centre[0] - pit[0], centre[1] - pit[1], centre[2] - pit[2]];
        if gap[0] * gap[0] + gap[1] * gap[1] + gap[2] * gap[2] < 0.01 {
            z = -9_000.0;
        }
        cell.elevation = Metres::new(z);
        cell.water_surface = Metres::new(z);
    }
    (world, working)
}

/// On a hand-made basin (`basin_world`), the enclosed pit below the sea level stays dry (ruling
/// R8), the sea is one ocean, and the shore's cells a fraction of a millimetre below the sea level
/// stay dry with the cells at it, as the quantised field shows them.
#[test]
fn an_enclosed_depression_and_the_shore_at_the_sea_level_s_code_stay_dry() {
    let (world, mut working) = basin_world();
    let pass = world.pass(reference::SEED);
    let sea = sea::place(&pass, &mut working);
    let sea_mm = SynthesisCell::quantise_height(working.sea_level).unwrap();
    let just_below = working
        .cells
        .iter()
        .filter(|c| {
            c.elevation.value() < sea.level_m
                && SynthesisCell::quantise_height(c.elevation).unwrap() == sea_mm
        })
        .inspect(|c| assert!(c.liquid_entry.is_none(), "{c:?} at the sea level's code"))
        .count();
    assert!(
        just_below > 0,
        "no ground a fraction of a millimetre below the sea: {sea:?}"
    );
    let pit_cells: Vec<usize> = working
        .cells
        .iter()
        .enumerate()
        .filter(|(_, c)| c.elevation.value() < -8_000.0)
        .map(|(n, _)| n)
        .collect();
    assert_ne!(pit_cells, [] as [usize; 0]);
    for &n in &pit_cells {
        let cell = &working.cells[n];
        assert!(
            cell.elevation.value() < sea.level_m,
            "the pit lies below the sea"
        );
        assert!(cell.liquid_entry.is_none(), "the pit at {n} stays dry");
    }
    assert!(sea.dry_below_share > 0.0);
    assert!(sea.ocean_share >= world.inputs.ocean_fraction());
    let field = quantise(&world.inputs, working.clone()).unwrap();
    assert_the_sea_holds(&world.grid, &world.inputs, &working, &field, &sea);
}

/// On the hand-made basin, the reflood keeps the sea level it is given: cells raised above it
/// drain, a dry cell beside the ocean lowered below it floods, and twice gives the same state, bit
/// for bit, which quantises as one ocean.
#[test]
fn the_reflood_keeps_the_sea_level_and_follows_the_ground() {
    let (world, mut working) = basin_world();
    let pass = world.pass(reference::SEED);
    let placed = sea::place(&pass, &mut working);
    let level = working.sea_level;
    let main = world.inputs.main_liquid();
    // Lower a dry cell beside the ocean below the sea, and raise two flooded cells away from it
    // above, as late craters' bowls and rims would.
    let shore = (0..world.grid.len())
        .find(|&n| {
            working.cells[slot(n)].liquid_entry.is_none()
                && working.cells[slot(n)].elevation.value() > level.value()
                && world
                    .grid
                    .edge_neighbours(n)
                    .into_iter()
                    .any(|m| working.cells[slot(m)].liquid_entry.is_some())
        })
        .expect("the ocean has a dry shore");
    let beside = world.grid.edge_neighbours(shore);
    let flooded: Vec<usize> = (0..world.grid.len())
        .filter(|&n| working.cells[slot(n)].liquid_entry.is_some() && !beside.contains(&n))
        .map(slot)
        .collect();
    let raised = [flooded[flooded.len() / 3], flooded[2 * flooded.len() / 3]];
    for &n in &raised {
        let z = Metres::new(level.value() + 500.0);
        working.cells[n].elevation = z;
        working.cells[n].water_surface = z;
    }
    let lowered = Metres::new(level.value() - 50.0);
    working.cells[slot(shore)].elevation = lowered;
    working.cells[slot(shore)].water_surface = lowered;
    let mut again = working.clone();
    let sea = sea::reflood(&pass, &mut working);
    let _ = sea::reflood(&pass, &mut again);
    assert_eq!(working, again);
    assert_same_bits(working.sea_level.value(), level.value());
    assert_same_bits(sea.level_m, placed.level_m);
    for &n in &raised {
        assert!(working.cells[n].liquid_entry.is_none(), "raised cell {n}");
    }
    assert_eq!(working.cells[slot(shore)].liquid_entry, main);
    let field = quantise(&world.inputs, working.clone()).unwrap();
    assert_the_sea_holds(&world.grid, &world.inputs, &working, &field, &sea);
}

/// The reflood leaves a lake as it is and drains a stale entry (decision-r09-t14-choices, "T14.d
/// follow-up" B): on the hand-made basin with its sea placed, a dry land cell 1 km and more above
/// the sea level given a water surface 30 m above its ground, off the sea level, and the main
/// liquid's entry, a lake's cell, keeps both to the bit; a dry cell given a stale entry, its
/// water surface at its ground as a late crater leaves it, loses the entry and keeps its water
/// surface at its ground; and every ocean cell keeps its liquid and the sea level.
#[test]
fn the_reflood_keeps_a_lake_and_drains_a_stale_entry() {
    let (world, mut working) = basin_world();
    let pass = world.pass(reference::SEED);
    let _ = sea::place(&pass, &mut working);
    let level = working.sea_level;
    let main = world.inputs.main_liquid();
    assert!(main.is_some(), "the basin world has a main liquid");
    let high: Vec<usize> = working
        .cells
        .iter()
        .enumerate()
        .filter(|(_, c)| c.liquid_entry.is_none() && c.elevation.value() > level.value() + 1_000.0)
        .map(|(n, _)| n)
        .collect();
    assert!(high.len() >= 2, "{} high dry cells", high.len());
    let (lake, stale) = (high[0], high[high.len() / 2]);
    let surface = Metres::new(working.cells[lake].elevation.value() + 30.0);
    working.cells[lake].water_surface = surface;
    working.cells[lake].liquid_entry = main;
    working.cells[stale].liquid_entry = main;
    let before = working.clone();
    let _ = sea::reflood(&pass, &mut working);
    assert_same_bits(working.cells[lake].water_surface.value(), surface.value());
    assert_eq!(
        working.cells[lake].liquid_entry, main,
        "the lake keeps its liquid"
    );
    assert_eq!(
        working.cells[stale].liquid_entry, None,
        "the stale entry is drained"
    );
    assert_same_bits(
        working.cells[stale].water_surface.value(),
        working.cells[stale].elevation.value(),
    );
    assert_same_bits(working.sea_level.value(), level.value());
    let mut ocean = 0_u32;
    for (n, (was, now)) in before.cells.iter().zip(&working.cells).enumerate() {
        if n == lake || n == stale || was.liquid_entry.is_none() {
            continue;
        }
        ocean += 1;
        assert_eq!(now.liquid_entry, was.liquid_entry, "ocean cell {n}");
        assert_same_bits(now.water_surface.value(), level.value());
    }
    assert!(ocean > 0, "the basin world has an ocean");
}

/// A Mars-like world of 1,500 km, a level-6 sphere, with an ocean of water over 0.3 of it: the
/// reference Mars's wet epoch, which ends before its surface age runs out, so that some coarse
/// craters wait for the erosion step, and a sea.
fn small_wet_mars() -> CoarseInputs {
    let mars = reference::mars_like();
    let earth = reference::earth_like();
    let water = earth
        .palette()
        .entries()
        .iter()
        .find(|e| e.role == PaletteRole::Liquid)
        .cloned()
        .unwrap();
    let mut entries = mars.palette().entries().to_vec();
    entries.push(water.clone());
    // A palette's entries run in strictly increasing (role, key).
    entries.sort_by_key(|e| (e.role, e.substance));
    builder_of(&mars)
        .figure(Spheroid::sphere(1.5e6))
        .liquids([AreaShare {
            substance: water.substance,
            area_fraction: 0.3,
        }])
        .palette(MaterialPalette::new(entries).unwrap())
        .build()
        .unwrap()
}

/// On a world with both a sea and craters that wait for the erosion step (none of the reference
/// worlds has both), `craters::run_after_erosion` smooths the late craters in, refloods the ocean
/// against the sea level as placed and recomputes the figures: the sea level is kept, the sea is
/// one ocean through quantisation, the whole surface's realised `σ_h` is plan 14's to 10⁻⁶ while
/// the erosion is a no-op (rulings R4 and R8), and twice gives the same state, bit for bit.
#[test]
fn the_late_craters_reflood_the_ocean_and_keep_the_whole_sigma_h() {
    let inputs = small_wet_mars();
    let world = World::new(inputs.clone());
    let pass = world.pass(reference::SEED);
    let mut working = Working::initial(&pass);
    crate::planetary::surface::steps::plates::run(&pass, &mut working);
    run(&pass, &mut working);
    craters::run(&pass, &mut working);
    run_after_craters(&pass, &mut working);
    let waiting = working
        .craters
        .iter()
        .filter(|c| craters::waits(&pass, c.age))
        .count();
    assert!(waiting > 0, "no crater waits");
    assert!(working.cells.iter().any(|c| c.liquid_entry.is_some()));
    let level = working.sea_level;
    let mut after = working.clone();
    craters::run_after_erosion(&pass, &mut after);
    let mut again = working.clone();
    craters::run_after_erosion(&pass, &mut again);
    assert_eq!(after, again);
    assert_ne!(elevations(&after), elevations(&working));
    assert_same_bits(after.sea_level.value(), level.value());
    let target = inputs.sigma_h().value();
    let realised = after.realised_sigma_h.value();
    eprintln!(
        "the small wet Mars after its late craters: σ_h {realised} m against {target} m, Δ {:.3e} \
         relative",
        realised / target - 1.0
    );
    assert!(
        (realised / target - 1.0).abs() <= 1e-6,
        "{realised} m against {target} m"
    );
    // The reflood is idempotent, so a second one gives the first's record.
    let mut probe = after.clone();
    let sea = sea::reflood(&pass, &mut probe);
    assert_eq!(probe, after);
    let field = quantise(&inputs, after.clone()).unwrap();
    assert_the_sea_holds(&world.grid, &inputs, &after, &field, &sea);
}

/// The reflood keeps the liquids the climate step shared the ocean among (R09.T13.d's `seas`): on
/// the small wet Mars, every other flooded cell given a second liquid's entry as the climate step
/// would give it, after `craters::run_after_erosion` a cell flooded before and after keeps its
/// entry, a cell the late craters newly flood takes the main liquid's, and a cell they raise out
/// of the sea is dry, its water surface its ground, with no liquid entry.
#[test]
fn the_reflood_keeps_the_liquid_each_flooded_cell_held() {
    let inputs = small_wet_mars();
    let world = World::new(inputs.clone());
    let pass = world.pass(reference::SEED);
    let mut working = Working::initial(&pass);
    crate::planetary::surface::steps::plates::run(&pass, &mut working);
    run(&pass, &mut working);
    craters::run(&pass, &mut working);
    run_after_craters(&pass, &mut working);
    let main = inputs.main_liquid().expect("the world has a liquid");
    // A stand-in for a second liquid's entry, which the reflood carries and never reads.
    let second = main
        .checked_add(1)
        .expect("a palette has fewer than 255 entries");
    for (n, cell) in working
        .cells
        .iter_mut()
        .filter(|c| c.liquid_entry.is_some())
        .enumerate()
    {
        if n % 2 == 1 {
            cell.liquid_entry = Some(second);
        }
    }
    let before = working.clone();
    craters::run_after_erosion(&pass, &mut working);
    let (mut kept, mut flooded, mut drained) = (0_u32, 0_u32, 0_u32);
    for (n, (was, now)) in before.cells.iter().zip(&working.cells).enumerate() {
        match (was.liquid_entry, now.liquid_entry) {
            (Some(held), Some(entry)) => {
                assert_eq!(entry, held, "cell {n}");
                kept += 1;
            }
            (None, Some(entry)) => {
                assert_eq!(entry, main, "cell {n}");
                flooded += 1;
            }
            (held, None) => {
                assert_same_bits(now.water_surface.value(), now.elevation.value());
                drained += u32::from(held.is_some());
            }
        }
        if now.liquid_entry.is_some() {
            assert!(
                now.water_surface.value() > now.elevation.value(),
                "cell {n}"
            );
        }
    }
    eprintln!("kept {kept}, newly flooded {flooded}, drained {drained}");
    assert!(kept > 0 && flooded > 0 && drained > 0, "a path untested");
    assert!(
        working.cells.iter().any(|c| c.liquid_entry == Some(second)),
        "the second liquid was lost"
    );
}

/// Where the coarse craters' bytes differ, C′ is not C times one factor, so the match reads it in a
/// second quadrature: on the reference Moon asked for a `σ_h` of 1,200 m, whose own R09.T7.j bytes
/// differ and whose whole field must scale down (s\* about 0.7), the scale still meets the `σ_h` to
/// 10⁻⁶, each byte is the nearest integer to 255 (1 − s\* g), and the state quantises. Re-derived
/// for R09.T7.j (`decision-r09-t7ij-refresh.md` Q1.2): as built it ran on the reference Moon itself
/// with every other crater set to a byte of 40, since T12.d's bytes were 0 and the Moon's s\* 0.79;
/// T7.j takes that s\* above 1, where no second quadrature is read. Only R09.T12.e's match reads
/// `σ_h`, so the Moon's craters and non-crater field are the reference Moon's.
#[test]
fn uneven_bytes_are_read_in_a_second_quadrature() {
    let inputs = builder_of(&reference::moon_like())
        .sigma_h(Metres::new(1_200.0))
        .build()
        .unwrap();
    let world = World::new(inputs);
    let pass = world.pass(reference::SEED);
    let inputs = pass.inputs();
    let mut working = Working::initial(&pass);
    crate::planetary::surface::steps::plates::run(&pass, &mut working);
    run(&pass, &mut working);
    craters::run(&pass, &mut working);
    let bytes: Vec<u8> = working.craters.iter().map(|c| c.degradation).collect();
    assert!(
        bytes.windows(2).any(|pair| pair[0] != pair[1]),
        "the reference Moon's bytes are alike"
    );
    let before: Vec<f64> = working.craters.iter().map(kept_relief).collect();
    working.spectrum = structural_spectrum(inputs);
    let matched = budget::match_sigma_h(&pass, &mut working);
    let s_star = matched.whole_scale.unwrap();
    eprintln!("the reference Moon at a σ_h of 1,200 m: s* {s_star:.4}");
    assert!(s_star <= 1.0, "{s_star}");
    assert!(matched.second_quadrature);
    assert_eq!(matched.rule, Rule::Whole);
    for (crater, &g) in working.craters.iter().zip(&before) {
        let want = (255.0 * (1.0 - s_star * g)).round().clamp(0.0, 255.0);
        assert_same_bits(f64::from(crater.degradation), want);
    }
    let _ = sea::place(&pass, &mut working);
    let figures = budget::figures(&pass, &mut working, Phase::BeforeErosion, &matched.booked);
    let target = inputs.sigma_h().value();
    assert!(
        (figures.sigma_h_m / target - 1.0).abs() <= 1e-6,
        "{} m against {target} m",
        figures.sigma_h_m
    );
    quantise(inputs, working).unwrap();
}

/// Where no positive scale meets plan 14's `σ_h` (a reference Moon asked for 10 m, below its own
/// local share), the elevation and the craters' bytes are left as the craters left them, bit for
/// bit (labelled `Rule::Unscaled`), and the entry still quantises.
#[test]
fn a_sigma_h_no_scale_can_meet_leaves_the_field_unscaled() {
    let inputs = builder_of(&reference::moon_like())
        .sigma_h(Metres::new(10.0))
        .build()
        .unwrap();
    let world = World::new(inputs);
    let pass = world.pass(reference::SEED);
    let mut working = Working::initial(&pass);
    crate::planetary::surface::steps::plates::run(&pass, &mut working);
    run(&pass, &mut working);
    craters::run(&pass, &mut working);
    let cratered = working.clone();
    working.spectrum = structural_spectrum(pass.inputs());
    let matched = budget::match_sigma_h(&pass, &mut working);
    assert_eq!(matched.rule, Rule::Unscaled);
    assert_eq!(matched.whole_scale, None);
    assert_same_bits(matched.non_crater_scale, 1.0);
    assert_eq!(working.cells, cratered.cells);
    assert_eq!(working.craters, cratered.craters);
    let mut entry = cratered.clone();
    run_after_craters(&pass, &mut entry);
    quantise(&world.inputs, entry).unwrap();
}

/// Test 9: the structural spectrum's class (`decision-r09-t5.md` item 3, ruling R9): the reference
/// Earth a silicate mobile lid at 200 m (its height factor 1), Mars, the Moon and Ceres silicate
/// stagnant lids at 60 m; an ice surface and an ice shell ice-rich at 50 m, an ice-rich mobile lid
/// too; and the mobile lid's anchor halves at twice the Earth's gravity.
#[test]
fn the_structural_spectrum_follows_the_crust_class() {
    let anchored = |inputs: &CoarseInputs, shape: SpectrumShape, rms: f64| {
        let want = BandSpectrum::anchored(shape, inputs.radius(), Metres::new(rms));
        assert_eq!(structural_spectrum(inputs), want);
        assert_same_bits(structural_anchor(inputs).value(), rms);
    };
    anchored(&reference::earth_like(), SpectrumShape::SILICATE, 200.0);
    for inputs in [
        reference::mars_like(),
        reference::moon_like(),
        reference::ceres_like(),
    ] {
        anchored(&inputs, SpectrumShape::SILICATE, 60.0);
    }
    let ice = builder_of(&reference::ceres_like())
        .surface(
            reference::ceres_like().surface_state(),
            SurfaceMaterial::Ice,
        )
        .build()
        .unwrap();
    anchored(&ice, SpectrumShape::ICE_RICH, 50.0);
    let shell = builder_of(&reference::ceres_like())
        .tectonics(TectonicRegime::IceShell, 0.0)
        .build()
        .unwrap();
    anchored(&shell, SpectrumShape::ICE_RICH, 50.0);
    let earth = reference::earth_like();
    let ice_plates = builder_of(&earth)
        .surface(earth.surface_state(), SurfaceMaterial::Ice)
        .build()
        .unwrap();
    anchored(&ice_plates, SpectrumShape::ICE_RICH, 50.0);
    let gravity = MetresPerSecondSquared::new(2.0 * convergent::EARTH_GRAVITY.value());
    let craters = CraterParams::new(CraterParamsParts {
        gravity,
        ..*earth.craters().parts()
    })
    .unwrap();
    let heavy = builder_of(&earth)
        .gravity(gravity)
        .craters(craters)
        .build()
        .unwrap();
    assert_same_bits(structural_anchor(&heavy).value(), 100.0);
}

/// The same seed twice gives the same entry, bit for bit, on the reference Moon and Ceres.
#[test]
fn the_same_seed_gives_the_same_entry() {
    for world in [Reference::Moon, Reference::Ceres] {
        let f = finished(world);
        let pass = f.pass();
        let mut again = f.cratered.clone();
        run_after_craters(&pass, &mut again);
        let field = quantise(f.inputs(), again).unwrap();
        assert_eq!(field, f.field, "{world:?}");
    }
}

/// The Gauss–Legendre rules are the Legendre polynomials' roots and integrate x^k exactly to
/// k = 2n − 1; and each cell's quadrature weights sum to its solid angle, the sphere's to 4π.
#[test]
fn the_quadrature_s_rules_and_weights_are_exact() {
    for n in 2..=4 {
        let rule = gauss_legendre(n);
        assert_eq!(rule.len(), n);
        for k in 0..2 * n {
            let exponent = i32::try_from(k).unwrap();
            let sum: f64 = rule.iter().map(|&(x, w)| w * math::powi(x, exponent)).sum();
            let want = if k % 2 == 0 {
                2.0 / f64::from(exponent + 1)
            } else {
                0.0
            };
            assert!((sum - want).abs() < 1e-15, "n {n}, x^{k}: {sum}");
        }
        for &(x, _) in &rule {
            let p = match n {
                2 => 0.5 * (3.0 * x * x - 1.0),
                3 => 0.5 * (5.0 * x * x * x - 3.0 * x),
                _ => (35.0 * x * x * x * x - 30.0 * x * x + 3.0) / 8.0,
            };
            assert!(p.abs() < 1e-15, "P_{n}({x}) = {p}");
        }
    }
    let grid = CellGraph::new(5).unwrap();
    let mut per_cell = vec![0.0; grid.cells().len()];
    each_node(&grid, &gauss_legendre(3), |n, dir, w| {
        let norm = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
        assert!((norm - 1.0).abs() < 1e-15);
        assert_eq!(cell_index(PatchKey::containing(5, dir).unwrap()), n);
        per_cell[slot(n)] += w;
    });
    for (&sum, &solid) in per_cell.iter().zip(grid.solid_angles_sr()) {
        assert!((sum / solid - 1.0).abs() < 1e-14);
    }
    let sphere: f64 = per_cell.iter().sum();
    assert!((sphere / (4.0 * core::f64::consts::PI) - 1.0).abs() < 1e-12);
}

/// The reconstructed variance has converged at three nodes a side, the smallest rule within 10⁻⁴
/// of four's: on the reference Moon, where two lie farther (the reference Mars's is measured by
/// hand, `measure_r09_t12e`).
#[test]
fn the_quadrature_has_converged_at_three_nodes_a_side() {
    let f = finished(Reference::Moon);
    let z = elevations(&f.working);
    let (_, two) = reconstructed(f.grid(), &z, 2);
    let (_, three) = reconstructed(f.grid(), &z, NODES);
    let (_, four) = reconstructed(f.grid(), &z, 4);
    assert_eq!(NODES, 3);
    assert!((three / four - 1.0).abs() <= 1e-4, "{three} against {four}");
    assert!((two / four - 1.0).abs() > 1e-4, "{two} against {four}");
}

/// The quadrature's value of `values` on `grid` against the interpolant's at every node of every
/// `every`-th cell, bit for bit; the count of nodes at which the shortcut took the cell's own
/// value.
fn uniform_reads(grid: &CellGraph, values: &[f64], every: u32) -> u32 {
    let field = Values::new(grid, values);
    let mut uniform = 0_u32;
    each_node(grid, &gauss_legendre(NODES), |n, dir, _| {
        if n % every == 0 {
            let at = field.at(n, dir);
            assert_same_bits(at, field.interpolated(dir));
            uniform += u32::from(at.total_cmp(&values[slot(n)]).is_eq());
        }
    });
    uniform
}

/// Where a cell has no differing value within the synthesis margin, the quadrature takes its own
/// value for the interpolant's, bit for bit as the interpolant gives it: at every node of the
/// reference Moon and Ceres for their elevations, crater lifts, amplitudes and belt weights, and at
/// a sample of the reference Mars's cells for its crater lift and amplitudes.
#[test]
fn the_uniform_cells_are_read_as_the_interpolant_gives_them() {
    for world in [Reference::Moon, Reference::Ceres] {
        let f = finished(world);
        let records: Vec<SynthesisCell> = budget::records(f.inputs(), &f.working).collect();
        let a: Vec<f64> = records
            .iter()
            .map(|r| ReliefStyle::of(r).amplitude())
            .collect();
        let b: Vec<f64> = records.iter().map(|r| ReliefStyle::of(r).belt()).collect();
        let lift = crater_lift(f, &f.working.craters, Phase::AfterErosion);
        for values in [elevations(&f.working), lift, a, b] {
            let _ = uniform_reads(f.grid(), &values, 1);
        }
    }
    let f = finished(Reference::Mars);
    let records: Vec<SynthesisCell> = budget::records(f.inputs(), &f.working).collect();
    let a: Vec<f64> = records
        .iter()
        .map(|r| ReliefStyle::of(r).amplitude())
        .collect();
    let lift = crater_lift(f, &f.working.craters, Phase::AfterErosion);
    for values in [lift, a] {
        assert!(uniform_reads(f.grid(), &values, 13) > 0);
    }
}

/// Each cell's king distance on `grid` from the cells `from`, through edge and corner neighbours,
/// out to `reach` moves; `None` beyond.
fn king_distances(grid: &CellGraph, from: &[u32], reach: u8) -> Vec<Option<u8>> {
    let mut distance = vec![None; grid.cells().len()];
    let mut ring = from.to_vec();
    for &c in &ring {
        distance[slot(c)] = Some(0);
    }
    for step in 1..=reach {
        let mut next = Vec::new();
        for &c in &ring {
            for m in grid
                .edge_neighbours(c)
                .into_iter()
                .chain(grid.vertex_neighbours(c))
            {
                if distance[slot(m)].is_none() {
                    distance[slot(m)] = Some(step);
                    next.push(m);
                }
            }
        }
        ring = next;
    }
    distance
}

/// The uniform cells' shortcut at every coarse level, 5 to 8, beside single-cell spikes where the
/// interpolant reads farthest, at a cube corner and at the ends and middle of face edges: at every
/// node of every cell 4 to 7 king moves from a spike, the shortcut's value is the interpolant's,
/// bit for bit (the determinism audit's consider).
#[test]
fn the_uniform_cells_hold_beside_spikes_at_every_coarse_level() {
    for level in 5..=8 {
        let grid = CellGraph::new(level).unwrap();
        let last = (1_u32 << level) - 1;
        let spikes: Vec<u32> = [
            (Face::PosX, 0, 0),
            (Face::PosX, last, last),
            (Face::PosX, 0, last / 2),
            (Face::PosY, last, 0),
            (Face::PosZ, last / 2, last),
        ]
        .into_iter()
        .map(|(face, i, j)| cell_index(PatchKey::new(face, level, i, j).unwrap()))
        .collect();
        let mut values = vec![0.0; grid.cells().len()];
        for &s in &spikes {
            values[slot(s)] = 1.0;
        }
        let distance = king_distances(&grid, &spikes, 7);
        let field = Values::new(&grid, &values);
        let mut checked = 0_u32;
        each_node(&grid, &gauss_legendre(NODES), |n, dir, _| {
            if distance[slot(n)].is_some_and(|d| d >= 4) {
                assert_same_bits(field.at(n, dir), field.interpolated(dir));
                checked += 1;
            }
        });
        assert!(checked > 0, "level {level}");
    }
}

/// The reference worlds' budgets, scales, shares, seas and hypsometry, printed for the plan's
/// record (R09.T12.e's report, §7): run by hand.
#[test]
#[ignore = "measurement: R09.T12.e's figures on the reference worlds, printed"]
fn measure_r09_t12e() {
    for world in WORLDS {
        let f = finished(world);
        print_budget(world, f);
        print_shares(f);
        print_sea(world, f);
    }
    let inputs = reference::mars_like();
    let field = crate::planetary::surface::coarse_pass(reference::SEED, &inputs);
    let lowest_mm = field.synthesis().iter().map(|c| c.elevation_mm).min();
    eprintln!(
        "Mars after the whole pass: realised σ_h {:.4} m against {} m, relief {:.0} m; sea level \
         {:.3} m, lowest cell {lowest_mm:?} mm",
        field.header().realised_sigma_h().value(),
        inputs.sigma_h().value(),
        field.header().realised_relief().value(),
        field.header().sea_level().value()
    );
}

/// `world`'s scale and local share, printed.
fn print_budget(world: Reference, f: &Finished) {
    let pass = f.pass();
    let sigma = f.inputs().sigma_h().value();
    let s2 = sigma * sigma;
    let m = &f.matched;
    eprintln!("== {world:?}: σ_h {sigma} m, level {}", pass.level().get());
    eprintln!(
        "  s* {:.4}, s_N {:.4}, rule {:?}, largest byte {}, second quadrature {}",
        m.whole_scale.unwrap_or(f64::NAN),
        m.non_crater_scale,
        m.rule,
        m.largest_byte,
        m.second_quadrature
    );
    eprintln!(
        "  T {:.5e} m² ({:.4} σ_h²); V_N {:.5e}, K {:.5e}, V_C {:.5e}, R_inc {:.5e}; after K′ \
         {:.5e}, V_C′ {:.5e}, R_inc′ {:.5e}",
        m.target_m2,
        m.target_m2 / s2,
        m.non_crater_m2,
        m.covariance_m2,
        m.craters_m2,
        m.before.increase_m2,
        m.after_m2.0,
        m.after_m2.1,
        m.after.increase_m2
    );
    let (cn, ck, cc) = m.cells_m2;
    eprintln!(
        "  cells: V_N {cn:.5e}, K {ck:.5e}, V_C {cc:.5e}; low-pass V_N {:.3}%, V_C {:.3}%",
        100.0 * (1.0 - m.non_crater_m2 / cn),
        100.0 * (1.0 - m.craters_m2 / cc.max(f64::MIN_POSITIVE))
    );
    let local = local_variance(&f.working.spectrum, pass.level()).value();
    for (label, l, fin) in [
        ("before", m.before, m.finer_before),
        ("after", m.after, m.finer_after),
    ] {
        eprintln!(
            "  {label}: whole L {:.4}% (S_s {:.4}%, K_w {:.4}%, R_inc {:.4}% of which sharpening \
             {:.4}% and cross {:.4}%, C_c {:.4}%); finer than λ_c {:.4}% (S_s {:.4}%, small \
             {:.4}%, rims {:.4}%, channels {:.4}%)",
            100.0 * l.total_m2() / s2,
            100.0 * l.structural_m2 / s2,
            100.0 * l.small_craters_m2 / s2,
            100.0 * l.increase_m2 / s2,
            100.0 * l.sharpening_m2 / s2,
            100.0 * (l.increase_m2 - l.sharpening_m2) / s2,
            100.0 * l.channels_m2 / s2,
            100.0 * fin.total().value() / s2,
            100.0 * fin.structural.value() / s2,
            100.0 * fin.small_craters.value() / s2,
            100.0 * fin.sharpening.value() / s2,
            100.0 * fin.channels.value() / s2
        );
    }
    eprintln!(
        "  ⟨a²⟩ {:.4}; S_s ÷ ⟨a²⟩ local_variance {:.5}; local_variance {:.4e} m² ({:.1} m)",
        m.before.amplitude_square,
        m.before.structural_m2 / (m.before.amplitude_square * local),
        local,
        local.sqrt()
    );
}

/// `world`'s figures and each share's RMS, on the cells and reconstructed, before and after the
/// scale, with the quadrature's convergence, printed.
fn print_shares(f: &Finished) {
    let grid = f.grid();
    let sigma = f.inputs().sigma_h().value();
    let m = &f.matched;
    let fig = &f.figures;
    eprintln!(
        "  realised σ_h {:.3} m ({:.7} σ_h), √Var_recon {:.1} m, cells' s.d. {:.1} m; relief {:.0} \
         m ({:.2} σ_h), reconstructed range {:.0} m ({:.2} σ_h); whole with every crater {:.3} m",
        fig.sigma_h_m,
        fig.sigma_h_m / sigma,
        fig.reconstructed_m2.sqrt(),
        fig.cells_m2.sqrt(),
        fig.relief_m,
        fig.relief_m / sigma,
        fig.reconstructed_range_m,
        fig.reconstructed_range_m / sigma,
        whole_sigma_h(f)
    );
    for (label, values, scaled) in &component_parts(f) {
        let (_, recon) = reconstructed(grid, values, NODES);
        let cells = cell_variance(grid, values);
        let after = scaled.as_ref().map(|v| {
            let (_, r) = reconstructed(grid, v, NODES);
            (cell_variance(grid, v).sqrt(), r.sqrt())
        });
        eprintln!(
            "  {label}: cells {:.1} m, reconstructed {:.1} m; after {:?}",
            cells.sqrt(),
            recon.sqrt(),
            after.map(|(c, r)| (c.round(), r.round()))
        );
    }
    eprintln!(
        "  craters as rendered: before {:.1} m, after {:.1} m",
        (m.craters_m2 + m.before.increase_m2).sqrt(),
        (m.after_m2.1 + m.after.increase_m2).sqrt()
    );
    for n in [2, 3, 4] {
        let (_, v) = reconstructed(grid, &elevations(&f.working), n);
        eprintln!("  Var_recon at n = {n}: {v:.9e} m²");
    }
}

/// `world`'s sea, and on the reference Earth its hypsometry relative to the sea level, printed.
fn print_sea(world: Reference, f: &Finished) {
    let s = &f.sea;
    eprintln!(
        "  sea: ℓ_q {:.2} m, ℓ {:.2} m, ocean {:.6}, excess {:.3e}, final level {:.3e}, flooded \
         {:.6} ({} cells), dry below ℓ {:.6}, seed {} cells",
        s.quantile_m,
        s.level_m,
        s.ocean_share,
        s.ocean_share - f.inputs().ocean_fraction(),
        s.final_level_share,
        s.flooded_share,
        s.flooded_cells,
        s.dry_below_share,
        s.seed.len()
    );
    if world == Reference::Earth {
        let z: Vec<f64> = f
            .working
            .cells
            .iter()
            .map(|c| c.elevation.value() - s.level_m)
            .collect();
        hypsometry_line("hypsometry relative to ℓ", &z, f.grid().solid_angles_sr());
    }
}

/// A share of a coarse elevation: its name and its cells' values before the scale and after it.
type Part = (&'static str, Vec<f64>, Option<Vec<f64>>);

/// Each share of `world`'s coarse elevation: the population or populations, the convergent
/// landforms or the provinces, and the coarse craters.
fn component_parts(world: &Finished) -> Vec<Part> {
    let pass = world.pass();
    let s_n = world.matched.non_crater_scale;
    let relief = elevations(&world.relieved.relief);
    let population: Vec<f64> = if let Some(populations) = &world.relieved.populations {
        elevations(populations)
    } else {
        let mut lid = Working::initial(&pass);
        crate::planetary::surface::steps::plates::run(&pass, &mut lid);
        lid_population(&pass, &mut lid);
        elevations(&lid)
    };
    let rest: Vec<f64> = relief.iter().zip(&population).map(|(r, p)| r - p).collect();
    let scaled = |v: &[f64]| -> Option<Vec<f64>> { Some(v.iter().map(|x| s_n * x).collect()) };
    let label = if world.relieved.populations.is_some() {
        "landforms"
    } else {
        "provinces"
    };
    let before = crater_lift(world, &world.cratered.craters, Phase::AfterErosion);
    let after = crater_lift(world, &world.working.craters, Phase::AfterErosion);
    vec![
        ("population(s)", population.clone(), scaled(&population)),
        (label, rest.clone(), scaled(&rest)),
        ("coarse craters", before, Some(after)),
    ]
}

/// The cost probes, timed by hand from outside the test binary (the sim reads no clock): each runs
/// the plates, the relief and the coarse craters on one reference world, then R09.T12.e's entry or
/// not, so that the difference between the two is the entry's cost.
mod cost {
    use super::*;

    /// The pass on `inputs` through the coarse craters, then R09.T12.e's entry where `entry`.
    fn through(inputs: CoarseInputs, entry: Entry) {
        let world = World::new(inputs);
        let pass = world.pass(reference::SEED);
        let mut working = Working::initial(&pass);
        crate::planetary::surface::steps::plates::run(&pass, &mut working);
        run(&pass, &mut working);
        craters::run(&pass, &mut working);
        if entry == Entry::Run {
            run_after_craters(&pass, &mut working);
        }
        assert_ne!(working.cells, []);
    }

    /// Whether a probe runs R09.T12.e's entry.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Entry {
        Skip,
        Run,
    }

    #[test]
    #[ignore = "cost: the reference Earth through its coarse craters, timed by hand"]
    fn earth_cratered() {
        through(reference::earth_like(), Entry::Skip);
    }

    #[test]
    #[ignore = "cost: the reference Earth through R09.T12.e, timed by hand"]
    fn earth_entry() {
        through(reference::earth_like(), Entry::Run);
    }

    #[test]
    #[ignore = "cost: the reference Mars through its coarse craters, timed by hand"]
    fn mars_cratered() {
        through(reference::mars_like(), Entry::Skip);
    }

    #[test]
    #[ignore = "cost: the reference Mars through R09.T12.e, timed by hand"]
    fn mars_entry() {
        through(reference::mars_like(), Entry::Run);
    }

    #[test]
    #[ignore = "cost: the reference Moon through its coarse craters, timed by hand"]
    fn moon_cratered() {
        through(reference::moon_like(), Entry::Skip);
    }

    #[test]
    #[ignore = "cost: the reference Moon through R09.T12.e, timed by hand"]
    fn moon_entry() {
        through(reference::moon_like(), Entry::Run);
    }

    #[test]
    #[ignore = "cost: the reference Ceres through its coarse craters, timed by hand"]
    fn ceres_cratered() {
        through(reference::ceres_like(), Entry::Skip);
    }

    #[test]
    #[ignore = "cost: the reference Ceres through R09.T12.e, timed by hand"]
    fn ceres_entry() {
        through(reference::ceres_like(), Entry::Run);
    }
}
