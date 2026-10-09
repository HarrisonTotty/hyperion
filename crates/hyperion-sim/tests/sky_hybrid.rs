//! The hybrid sky's real boundary, measured from the caps' count alone, with no census (rendering
//! plan R13, R13.T1; Design notes 3 and 4). Slow.
//!
//! For layers C to E the real boundary R(u) is each ray's cap at the synthetic ceiling
//! V<sub>P</sub>, the cap `CapCount::measure` gives at cut V<sub>P</sub>, held within the same
//! ray's cap at the request's cut (`RayRadii::lesser`): the rule R13.T2.a's `real_boundary` takes.
//! At the six points of `caps_converge_in_rays`, at the eye's cut as R06.T9.d computes it and the
//! server asks it since R06.T11.d (`eye_cut` with the request's illumination, `Illumination::march`
//! at the point, and the eye-only request's caps by the eye's visibility as the cut's), and at the
//! uniform cuts 7.95 and 10.06, for V<sub>P</sub> of 4.0, 4.5, 5.0 and 5.5, the test records per
//! layer:
//!
//! - R(u) by ray (the median, 10th and 90th percentiles and largest of its rays), the rays the cut
//!   holds, and the sphere at the same count (`CapCount::spheres` at V<sub>P</sub>), against the
//!   cut's caps;
//! - the expected stars brighter than V 5, 6, 6.5, 7 and the cut beyond R(u), by ray and as the
//!   sphere (held within the cut's caps): those the synthetic tier and the band take, and their
//!   share of the whole sky's stars to the same depth, every layer's at any distance;
//! - the expected count brighter than V<sub>P</sub> beyond R(u), at the caps' own count and
//!   recounted;
//! - the systems within R(u), by ray, as the sphere, by ray with the cones widened by the band
//!   texel's radius ρ (fix (i) of `decision-r06-t8i-listing.md`), and within the cut's caps;
//! - T7.b's gap (that record's §2, "cheap measurement"): the expected stars brighter than the cut
//!   between the radius towards each ray and the radius towards the centre of its 64² band texel,
//!   both ways. The stars within the texel's radius and beyond the ray's own are the gap's bound,
//!   ignoring the cells the padded balls open; the reverse are the mirror's. With fix (i) the gap's
//!   bound is none by construction, which the test prints as a check.
//!
//! Every expected count, the systems and the gap are taken on the convergence test's recount
//! (`common::sky::caps_recount`), towards whose rays each boundary is read as the census reads it.
//! It asserts only what R13.T1 asks: R(u) is within the cut's caps on every ray, and the expected
//! count brighter than V<sub>P</sub> beyond it, at the caps' own count, is under one a layer.
//! R13's Risks record the figures.
//!
//! The points are measured on six threads, one a point, each with its own noise cache. On
//! wasm32-wasip1, which has no threads, the test is left out: it would take hours on one.

#![cfg(not(target_family = "wasm"))]

#[expect(
    dead_code,
    reason = "the boundary's record uses the sky helpers of tests/common alone"
)]
mod common;

use common::sky::{CAPS_POINTS, caps_recount};
use common::usize_as_f64;
use hyperion_sim::Seed;
use hyperion_sim::coords::{GalacticPosition, UnitVector};
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
use hyperion_sim::galaxy::gas::noise::NoiseCache;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::id::Layer;
use hyperion_sim::math;
use hyperion_sim::observe::Observer;
use hyperion_sim::sky::EyeObserver;
use hyperion_sim::sky::band::BandSpec;
use hyperion_sim::sky::caps::{
    CAPPED_LAYERS, CapCount, CapResolution, LayerCap, RayRadii, layer_caps,
    layer_caps_by_visibility,
};
use hyperion_sim::sky::census::{CellOffsets, NoSkyCellCache, SkyContext};
use hyperion_sim::sky::dgl::Illumination;
use hyperion_sim::sky::envelope::BrightnessEnvelope;
use hyperion_sim::sky::limits::{eye_cut, eye_visibility};
use hyperion_sim::sky::luminosity::LuminosityTables;
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::{LightYears, Magnitudes};

/// The synthetic ceilings V<sub>P</sub> measured, V (R13 Design note 4: 4.5 ruled, 5.0 in RM3's
/// interim).
const CEILINGS_V: [f64; 4] = [4.0, 4.5, 5.0, 5.5];

/// The depths, V, to which the stars beyond R(u) are counted besides the cut's.
const DEPTHS_V: [f64; 4] = [5.0, 6.0, 6.5, 7.0];

/// The uniform cuts besides the eye's, V: the eye's near the Sun before R06.T9.d (7.95), and a
/// camera's at 60° (10.06).
const UNIFORM_CUTS_V: [f64; 2] = [7.95, 10.06];

/// The layers with a real boundary at the ceiling; A, B and the brown dwarfs keep the cut's caps.
const BOUNDED_LAYERS: [Layer; 3] = [Layer::C, Layer::D, Layer::E];

/// Whether two cuts are one, V.
fn same_v(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

/// One request's cut and the caps it takes there.
struct Cut {
    name: String,
    v: f64,
    caps: Vec<LayerCap>,
}

/// One ceiling's caps and spheres at the caps' own resolution, and its count, for the count
/// beyond at V<sub>P</sub>.
struct Ceiling {
    v: f64,
    caps: Vec<LayerCap>,
    spheres: Vec<LayerCap>,
    count: CapCount,
}

/// One layer's boundary at one cut and ceiling, and what is measured of it.
struct Record {
    cut: usize,
    ceiling: usize,
    layer: Layer,
    /// R(u) by ray.
    rays: LayerCap,
    /// The sphere at V<sub>P</sub>, ly.
    sphere_ly: f64,
    /// The rays whose cap at the ceiling the cut's holds.
    clipped: usize,
    /// The share of the cut's rays nearer than the sphere.
    sphere_clipped: f64,
    /// At V<sub>P</sub>, at the caps' own count, by ray and as the sphere held within the cut's
    /// caps.
    own_beyond: [f64; 2],
    /// At V<sub>P</sub>, recounted.
    recount_beyond: [f64; 2],
    /// Beyond R(u), brighter than V 5, 6, 6.5 and 7 and the cut: by ray, then as the sphere.
    beyond: [[f64; 5]; 2],
    /// The systems within R(u) by ray, as the sphere, by ray with fix (i), and within the cut's
    /// caps.
    systems: [f64; 4],
    /// Brighter than the cut, by ray: within the texel's radius and beyond the ray's own (the
    /// gap's bound), the reverse (the mirror), and the gap's bound with fix (i); then the sphere's
    /// gap and mirror.
    gap: [f64; 5],
}

/// Per recount's cut, V, every layer's whole sky: its stars brighter than the cut at any distance.
type WholeSky = Vec<(f64, [f64; CAPPED_LAYERS.len()])>;

/// The `q` quantile of radii `sorted` ascending, ly.
fn quantile(sorted: &[f64], q: f64) -> f64 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "an index into a few thousand radii, rounded from a non-negative value"
    )]
    let at = (usize_as_f64(sorted.len() - 1) * q).round() as usize;
    sorted[at]
}

/// A cap's rays' radii, ly, ascending.
fn sorted_radii(rays: &RayRadii) -> Vec<f64> {
    let mut radii = rays.radii_ly().to_vec();
    radii.sort_by(f64::total_cmp);
    radii
}

/// The cap of `layer` in `caps`.
fn cap_of(caps: &[LayerCap], layer: Layer) -> &LayerCap {
    caps.iter()
        .find(|c| c.layer() == layer)
        .expect("a cap for every layer")
}

/// The direction through the centre of the 64² band texel `u` falls in.
fn texel_centre(u: UnitVector) -> UnitVector {
    let spec = BandSpec::STANDARD;
    let (face, row, column) = spec
        .texel_of(u.components())
        .expect("a direction falls in a texel");
    spec.texel_direction(face, row, column)
}

/// The radius towards `u` of `rays` with every cone widened by `by` radians: the largest radius
/// of the rays within the lattice's spacing and `by` of it, by a scan of every ray (fix (i) of
/// `decision-r06-t8i-listing.md`, with `by` the band texel's radius ρ).
fn widened_toward(rays: &RayRadii, by: f64, u: UnitVector) -> LightYears {
    let lattice = rays.lattice();
    let cos = math::cos(lattice.spacing().value() + by);
    LightYears::new(
        lattice
            .directions()
            .iter()
            .zip(rays.radii_ly())
            .filter(|(d, _)| d.dot(&u) >= cos)
            .map(|(_, &r)| r)
            .fold(0.0, f64::max),
    )
}

/// What every point's measure reads.
struct Fixture {
    galaxy: Galaxy,
    tables: LuminosityTables,
    envelope: BrightnessEnvelope,
    offsets: CellOffsets,
}

/// The cuts at `observer`: the eye's (R06.T9.d's, with the request's illumination, as the server
/// computes it), with the eye-only request's caps by the eye's visibility, then the uniform cuts
/// with their caps by ray.
fn cuts_at(fixture: &Fixture, observer: &Observer, cache: &mut NoiseCache) -> Vec<Cut> {
    let Fixture {
        galaxy,
        tables,
        envelope,
        offsets,
    } = fixture;
    let mut ctx = SkyContext {
        tables,
        envelope,
        offsets,
        noise: NoiseCache::with_capacity(1 << 16),
        cells: &NoSkyCellCache,
        sources: &[],
        modifiers: &NoModifiers,
    };
    let eye = EyeObserver::default();
    let light = Illumination::march(galaxy, &mut ctx, observer);
    let v = eye_cut(galaxy, &mut ctx, observer, &eye, Some(&light));
    let visibility = eye_visibility(galaxy, &mut ctx, observer, &eye, v, Some(&light));
    let seen = layer_caps_by_visibility(galaxy, tables, envelope, observer, &visibility, cache);
    let mut cuts = vec![Cut {
        name: format!("the eye's cut {:.3}, caps by visibility", v.value()),
        v: v.value(),
        caps: seen,
    }];
    for v in UNIFORM_CUTS_V {
        let cut = Magnitudes::new(v);
        cuts.push(Cut {
            name: format!("the uniform cut {v:.2}"),
            v,
            caps: layer_caps(galaxy, tables, envelope, observer, cut, cache),
        });
    }
    cuts
}

/// The ceilings' counts at `observer`, at the caps' own resolution, with their caps and spheres.
fn ceilings_at(fixture: &Fixture, observer: &Observer, cache: &mut NoiseCache) -> Vec<Ceiling> {
    CEILINGS_V
        .iter()
        .map(|&v| {
            let count = CapCount::measure(
                &fixture.galaxy,
                &fixture.tables,
                &fixture.envelope,
                observer,
                Magnitudes::new(v),
                CapResolution::STANDARD,
                cache,
            );
            Ceiling {
                v,
                caps: count.caps(),
                spheres: count.spheres(),
                count,
            }
        })
        .collect()
}

/// Each layer's boundary at each cut and ceiling, with what the caps' own counts say of it, and
/// each failure of the two asserts, named by `name`.
fn boundaries(
    name: &str,
    cuts: &[Cut],
    ceilings: &[Ceiling],
    failures: &mut Vec<String>,
) -> Vec<Record> {
    let mut records = Vec::new();
    for (c, cut) in cuts.iter().enumerate() {
        for (p, ceiling) in ceilings.iter().enumerate() {
            for layer in BOUNDED_LAYERS {
                let (high, low) = (cap_of(&ceiling.caps, layer), cap_of(&cut.caps, layer));
                let (high_rays, low_rays) = (
                    high.rays().expect("one radius a ray"),
                    low.rays().expect("one radius a ray"),
                );
                let rays = LayerCap::forced_by_ray(layer, high_rays.lesser(low_rays));
                let within = rays
                    .rays()
                    .expect("one radius a ray")
                    .radii_ly()
                    .iter()
                    .zip(low_rays.radii_ly())
                    .all(|(r, l)| r <= l);
                if !within {
                    failures.push(format!(
                        "{name}, {}, V_P {}, {layer:?}: R(u) beyond the cut's caps",
                        cut.name, ceiling.v
                    ));
                }
                let clipped = high_rays
                    .radii_ly()
                    .iter()
                    .zip(low_rays.radii_ly())
                    .filter(|(h, l)| h > l)
                    .count();
                let sphere_ly = cap_of(&ceiling.spheres, layer).radius().value();
                let nearer = low_rays
                    .radii_ly()
                    .iter()
                    .filter(|&&r| r < sphere_ly)
                    .count();
                let sphere_clipped = usize_as_f64(nearer) / usize_as_f64(low_rays.radii_ly().len());
                let sphere_toward =
                    |u| LightYears::new(sphere_ly.min(low.radius_toward(u).value()));
                let own_beyond = [
                    ceiling.count.stars_beyond(&rays),
                    ceiling.count.stars_beyond_toward(layer, sphere_toward),
                ];
                if own_beyond[0] >= 1.0 {
                    failures.push(format!(
                        "{name}, {}, V_P {}, {layer:?}: {} brighter than V_P beyond R(u)",
                        cut.name, ceiling.v, own_beyond[0]
                    ));
                }
                records.push(Record {
                    cut: c,
                    ceiling: p,
                    layer,
                    rays,
                    sphere_ly,
                    clipped,
                    sphere_clipped,
                    own_beyond,
                    recount_beyond: [f64::NAN; 2],
                    beyond: [[f64::NAN; 5]; 2],
                    systems: [f64::NAN; 4],
                    gap: [f64::NAN; 5],
                });
            }
        }
    }
    records
}

/// The recounts at `observer`, one cut at a time so that one is held at once: each record's counts
/// beyond, systems and gap, and every layer's whole sky at each recount's cut.
fn recount_records(
    fixture: &Fixture,
    observer: &Observer,
    (cuts, ceilings): (&[Cut], &[Ceiling]),
    records: &mut [Record],
    cache: &mut NoiseCache,
) -> WholeSky {
    let mut recount_cuts: Vec<f64> = CEILINGS_V
        .iter()
        .chain(&DEPTHS_V)
        .copied()
        .chain(cuts.iter().map(|c| c.v))
        .collect();
    recount_cuts.sort_by(f64::total_cmp);
    recount_cuts.dedup_by(|a, b| same_v(*a, *b));
    let rho = BandSpec::STANDARD.largest_texel_radius().value();
    let mut whole_sky = Vec::new();
    for f in recount_cuts {
        let count = CapCount::measure(
            &fixture.galaxy,
            &fixture.tables,
            &fixture.envelope,
            observer,
            Magnitudes::new(f),
            caps_recount(),
            cache,
        );
        whole_sky.push((
            f,
            CAPPED_LAYERS.map(|layer| count.stars_beyond_toward(layer, |_| LightYears::ZERO)),
        ));
        for record in records.iter_mut() {
            let cut = &cuts[record.cut];
            let low = cap_of(&cut.caps, record.layer);
            let rays = &record.rays;
            let sphere_ly = record.sphere_ly;
            let sphere_toward = |u| LightYears::new(sphere_ly.min(low.radius_toward(u).value()));
            let depth = DEPTHS_V.iter().position(|&d| same_v(d, f));
            let at_cut = same_v(cut.v, f).then_some(DEPTHS_V.len());
            for k in depth.into_iter().chain(at_cut) {
                record.beyond[0][k] = count.stars_beyond(rays);
                record.beyond[1][k] = count.stars_beyond_toward(record.layer, sphere_toward);
            }
            if same_v(ceilings[record.ceiling].v, f) {
                record.recount_beyond = [
                    count.stars_beyond(rays),
                    count.stars_beyond_toward(record.layer, sphere_toward),
                ];
            }
            if same_v(cut.v, f) {
                let by_ray = rays.rays().expect("one radius a ray");
                let widened = |u| widened_toward(by_ray, rho, u);
                let at_texel = |u| rays.radius_toward(texel_centre(u));
                let own = |u| rays.radius_toward(u);
                let sphere_at_texel =
                    |u| LightYears::new(sphere_ly.min(low.radius_toward(texel_centre(u)).value()));
                record.systems = [
                    count.systems_within(rays),
                    count.systems_within_toward(record.layer, sphere_toward),
                    count.systems_within_toward(record.layer, widened),
                    count.systems_within(low),
                ];
                record.gap = [
                    count.stars_within_and_beyond_toward(record.layer, at_texel, own),
                    count.stars_within_and_beyond_toward(record.layer, own, at_texel),
                    count.stars_within_and_beyond_toward(record.layer, at_texel, widened),
                    count.stars_within_and_beyond_toward(
                        record.layer,
                        sphere_at_texel,
                        sphere_toward,
                    ),
                    count.stars_within_and_beyond_toward(
                        record.layer,
                        sphere_toward,
                        sphere_at_texel,
                    ),
                ];
            }
        }
    }
    whole_sky
}

/// The boundary's measures at `point`, named `name`: the lines to print, and each failure of the
/// two asserts.
fn measure_point(fixture: &Fixture, name: &str, point: [f64; 3]) -> (Vec<String>, Vec<String>) {
    let observer = Observer::new(
        GalacticPosition::from_light_years(point).expect("in the cube"),
        UniverseTime::EPOCH,
    )
    .expect("an observer");
    let mut cache = NoiseCache::with_capacity(1 << 16);
    let cuts = cuts_at(fixture, &observer, &mut cache);
    let ceilings = ceilings_at(fixture, &observer, &mut cache);
    let mut failures = Vec::new();
    let mut records = boundaries(name, &cuts, &ceilings, &mut failures);
    let at = (cuts.as_slice(), ceilings.as_slice());
    let whole_sky = recount_records(fixture, &observer, at, &mut records, &mut cache);
    let mut lines = vec![format!(
        "{name} {point:?}: the eye's cut {:.3}; A, B and the brown dwarfs keep each cut's caps",
        cuts[0].v
    )];
    for (c, cut) in cuts.iter().enumerate() {
        report_cut(&mut lines, name, (c, cut), &ceilings, &records, &whole_sky);
    }
    (lines, failures)
}

/// The lines of cut `c` at the point named `name`: the cut's caps, each record's line, and per
/// ceiling the whole sky and the share of it beyond R(u).
fn report_cut(
    lines: &mut Vec<String>,
    name: &str,
    (c, cut): (usize, &Cut),
    ceilings: &[Ceiling],
    records: &[Record],
    whole_sky: &WholeSky,
) {
    lines.push(format!("{name}, {}:", cut.name));
    for layer in BOUNDED_LAYERS {
        let low = cap_of(&cut.caps, layer).rays().expect("one radius a ray");
        let sorted = sorted_radii(low);
        lines.push(format!(
            "  {layer:?} the cut's caps: rays median {:.0} ({:.0}–{:.0}), largest {:.0} ly",
            quantile(&sorted, 0.5),
            quantile(&sorted, 0.1),
            quantile(&sorted, 0.9),
            quantile(&sorted, 1.0),
        ));
    }
    for record in records.iter().filter(|r| r.cut == c) {
        lines.push(line(record, &ceilings[record.ceiling], cut));
    }
    let depths: Vec<f64> = DEPTHS_V.iter().copied().chain([cut.v]).collect();
    let whole: Vec<f64> = depths
        .iter()
        .map(|&d| {
            let (_, layers) = whole_sky
                .iter()
                .find(|(f, _)| same_v(*f, d))
                .expect("a recount at every depth");
            layers.iter().sum()
        })
        .collect();
    for (p, ceiling) in ceilings.iter().enumerate() {
        let share = |shape: usize| -> String {
            (0..depths.len())
                .map(|k| {
                    let beyond: f64 = records
                        .iter()
                        .filter(|r| r.cut == c && r.ceiling == p)
                        .map(|r| r.beyond[shape][k])
                        .sum();
                    format!("V{:.2} {:.2}%", depths[k], 100.0 * beyond / whole[k])
                })
                .collect::<Vec<_>>()
                .join(", ")
        };
        lines.push(format!(
            "  V_P {:.1}, every layer: the whole sky's stars {}; the share beyond R(u), C to E, by \
             ray {}; as the sphere {}",
            ceiling.v,
            depths
                .iter()
                .zip(&whole)
                .map(|(d, n)| format!("V{d:.2} {n:.4e}"))
                .collect::<Vec<_>>()
                .join(", "),
            share(0),
            share(1),
        ));
    }
}

/// One record's line.
fn line(record: &Record, ceiling: &Ceiling, cut: &Cut) -> String {
    let sorted = sorted_radii(record.rays.rays().expect("one radius a ray"));
    let [b, s] = record.beyond;
    let [rays, sphere, fix, whole] = record.systems;
    let [gap, mirror, fix_gap, sphere_gap, sphere_mirror] = record.gap;
    format!(
        "  V_P {:.1} {:?}: R(u) by ray median {:.0} ({:.0}–{:.0}), largest {:.0} ly, {} rays held \
         by the cut's; sphere {:.0} ly ({:.1}% of the cut's rays nearer). Beyond, by ray: V5 \
         {:.3e}, V6 {:.3e}, V6.5 {:.3e}, V7 {:.3e}, V{:.2} {:.3e}; brighter than V_P at the caps' \
         count {:.3}, recounted {:.3}. As the sphere: V5 {:.3e}, V6 {:.3e}, V6.5 {:.3e}, V7 \
         {:.3e}, V{:.2} {:.3e}; V_P {:.3}, recounted {:.3}. Systems: by ray {rays:.4e}, sphere \
         {sphere:.4e} ({:.3}×), fix (i) {fix:.4e} ({:.3}×), the cut's caps {whole:.4e} (R(u) by ray \
         {:.4}%). Gap at the cut, by ray: {gap:.3} (texel's within, own beyond), mirror \
         {mirror:.3}, with fix (i) {fix_gap:.2e}; sphere {sphere_gap:.3}, mirror \
         {sphere_mirror:.3}",
        ceiling.v,
        record.layer,
        quantile(&sorted, 0.5),
        quantile(&sorted, 0.1),
        quantile(&sorted, 0.9),
        quantile(&sorted, 1.0),
        record.clipped,
        record.sphere_ly,
        100.0 * record.sphere_clipped,
        b[0],
        b[1],
        b[2],
        b[3],
        cut.v,
        b[4],
        record.own_beyond[0],
        record.recount_beyond[0],
        s[0],
        s[1],
        s[2],
        s[3],
        cut.v,
        s[4],
        record.own_beyond[1],
        record.recount_beyond[1],
        sphere / rays,
        fix / rays,
        100.0 * rays / whole,
    )
}

#[test]
#[ignore = "slow: builds the luminosity tables and counts the caps at seven cuts and recounts \
            3,072 rays at ten, at six points"]
fn the_hybrid_boundary_is_recorded() {
    let galaxy = Galaxy::from_params(Seed::new(0x0926_0000), GalaxyParams::milky_way_like())
        .expect("the Milky Way-like parameters are valid");
    let fixture = Fixture {
        tables: LuminosityTables::build(&galaxy),
        envelope: BrightnessEnvelope::build(&galaxy),
        offsets: CellOffsets::build(&galaxy),
        galaxy,
    };
    let results: Vec<(Vec<String>, Vec<String>)> = std::thread::scope(|scope| {
        let handles: Vec<_> = CAPS_POINTS
            .iter()
            .map(|&(name, point)| {
                let fixture = &fixture;
                scope.spawn(move || measure_point(fixture, name, point))
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("a point's measure does not panic"))
            .collect()
    });
    let mut failures = Vec::new();
    for (lines, failed) in results {
        for l in lines {
            eprintln!("{l}");
        }
        failures.extend(failed);
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
