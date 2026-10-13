//! The layer caps against a finer count (rendering plan R06, R06.T7 and R06.T7.b; decided
//! 2026-10-03, `decision-r06-t7-caps.md`, and 2026-10-05, `decision-r06-census-cost-signoff.md`,
//! question 3). Slow.
//!
//! At six points across the galaxy, every layer's expected count of stars brighter than the cut
//! beyond its caps, one radius a ray, recounted with 3,072 rays and twice the radial steps, is under
//! 1.5: the caps' own claim (under 1 at their own resolution) holds against an independent finer
//! count, towards whose rays the caps' radii are read as the census reads them, each through its
//! own profile. It holds for the caps at the uniform cuts of the eye near the Sun (7.95), of the
//! eye 2,000 ly above it (8.54) and of a camera at 60° (10.06), and for the caps by the eye's
//! visibility at each point's own eye's cut. If it fails, the neighbour widening grows; the gate is
//! not loosened.
//!
//! For R06.T17's record it prints, for each layer, the sphere at the same count
//! (`CapCount::spheres`, the radius the brainstorm's figures describe) at the uniform cuts, the
//! rays' median, 10th and 90th percentiles and largest, the median of the rays within 10° of the
//! plane, and the largest ray's direction (`decision-r06-t7b-brackets.md`): its galactic
//! longitude, from the direction to the galactic centre towards spinward, as the IAU's l runs, and
//! its latitude towards the frame's north, +z, which is the IAU's south galactic pole, since the
//! frame's galaxy turns counter-clockwise seen from +z and the Milky Way clockwise seen from the
//! north galactic pole (Blaauw, Gum, Pawsey and Westerhout 1960, MNRAS 121, 123): so the IAU's b
//! is −b. Since the caps widen each ray to its neighbours', the largest radius is a patch of rays,
//! and its direction is the patch's centre. Beside the rays it prints the expected count of D's
//! and E's stars beyond 10,000 ly more than 30° from the plane, the brackets ruling's finding read
//! as a count; near the Sun the camera's caps at its black-sky limit; and the range query's check
//! (open question 19).
//!
//! For R06.T11.h's order (decided 2026-10-13, `decision-r06-t17-findings.md` §1), a second slow
//! test counts, near the Sun on the test fixture and on the server's galaxy, each of C's, D's and
//! E's expected stars brighter than V 3.0 between 1,000 ly and its real boundary at the served
//! eye's cut and ceiling, from the caps' count alone with no census, and holds D's to at most a
//! third of the three's, the rule by which the server's delivery leaves D's far shells with C's.

#[expect(
    dead_code,
    reason = "the caps' slow tests use the sky helpers of tests/common alone"
)]
mod common;

use common::sky::{CAPS_POINTS, caps_recount};
use hyperion_sim::Seed;
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
use hyperion_sim::galaxy::gas::noise::NoiseCache;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::id::Layer;
use hyperion_sim::math;
use hyperion_sim::observe::Observer;
use hyperion_sim::sky::EyeObserver;
use hyperion_sim::sky::caps::{
    CAPPED_LAYERS, CapCount, CapResolution, LayerCap, expected_beyond_caps,
    expected_beyond_caps_by_visibility, layer_caps_by_visibility,
};
use hyperion_sim::sky::census::{CellOffsets, NoSkyCellCache, SkyContext};
use hyperion_sim::sky::envelope::BrightnessEnvelope;
use hyperion_sim::sky::limits::{eye_cut, eye_visibility};
use hyperion_sim::sky::luminosity::LuminosityTables;
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::{LightYears, Magnitudes};

/// The median of a cap's rays' radii, ly.
fn median_ray(cap: &LayerCap) -> f64 {
    let mut radii = cap.rays().expect("one radius a ray").radii_ly().to_vec();
    radii.sort_by(f64::total_cmp);
    radii[radii.len() / 2]
}

/// The `q`-quantile of `sorted`, ascending, by the nearest rank, as `sky_realised` and
/// `sky_hybrid` take it: at 1,536 rays its median is [`median_ray`]'s ray, the 769th.
fn quantile(sorted: &[f64], q: f64) -> f64 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "a rank of a few thousand rays, from a fraction in [0, 1]"
    )]
    let rank = ((sorted.len() - 1) as f64 * q).round() as usize;
    sorted[rank]
}

/// A direction's galactic longitude and latitude seen from `at`, ly in the galactic frame,
/// degrees: l from the direction in the plane to the galactic centre, −p̂ (−y from the Sun on +y),
/// towards spinward, ẑ × p̂ (−x there, the frame's galaxy turning counter-clockwise seen from +z);
/// b towards +z. `at` lies off the z axis.
fn longitude_latitude(u: [f64; 3], at: [f64; 3]) -> (f64, f64) {
    let r = (at[0] * at[0] + at[1] * at[1]).sqrt();
    let (centre, spinward) = ([-at[0] / r, -at[1] / r], [-at[1] / r, at[0] / r]);
    let along = |axis: [f64; 2]| u[0] * axis[0] + u[1] * axis[1];
    let l = math::atan2(along(spinward), along(centre))
        .to_degrees()
        .rem_euclid(360.0);
    let b = math::asin(u[2].clamp(-1.0, 1.0)).to_degrees();
    (l, b)
}

/// R06.T17's record of a cap by ray: its rays' median, 10th and 90th percentiles and largest, the
/// median of those within 10° of the plane, and the largest's direction.
fn ray_record(cap: &LayerCap, at: [f64; 3]) -> String {
    let rays = cap.rays().expect("one radius a ray");
    let directions = rays.lattice().directions();
    let radii = rays.radii_ly();
    let mut sorted = radii.to_vec();
    sorted.sort_by(f64::total_cmp);
    let mut in_plane: Vec<f64> = radii
        .iter()
        .zip(directions)
        .filter(|(_, u)| longitude_latitude(u.components(), at).1.abs() < 10.0)
        .map(|(&r, _)| r)
        .collect();
    in_plane.sort_by(f64::total_cmp);
    let largest = sorted.last().copied().expect("rays");
    // The caps widen each ray to its neighbours', so the largest radius is held by a patch of rays
    // about the farthest window: its direction is the patch's centre, and its extent the patch's
    // farthest ray from that centre (the science check of R06.T17's record).
    let patch: Vec<[f64; 3]> = radii
        .iter()
        .zip(directions)
        .filter(|&(&r, _)| r.total_cmp(&largest).is_eq())
        .map(|(_, u)| u.components())
        .collect();
    let sum = patch
        .iter()
        .fold([0.0; 3], |s, u| [s[0] + u[0], s[1] + u[1], s[2] + u[2]]);
    let norm = (sum[0] * sum[0] + sum[1] * sum[1] + sum[2] * sum[2]).sqrt();
    let centre = sum.map(|c| c / norm);
    let extent = patch
        .iter()
        .map(|u| {
            let dot = u[0] * centre[0] + u[1] * centre[1] + u[2] * centre[2];
            math::acos(dot.clamp(-1.0, 1.0)).to_degrees()
        })
        .fold(0.0, f64::max);
    let (l, b) = longitude_latitude(centre, at);
    let toward = format!(
        "at l {l:.0}° b {b:+.0}° (the centre of {} rays within {extent:.0}° of it)",
        patch.len()
    );
    // The brackets ruling's finding: a ray beyond 10,000 ly more than 30° from the plane.
    let high = radii
        .iter()
        .zip(directions)
        .filter(|&(&r, u)| r > 10_000.0 && longitude_latitude(u.components(), at).1.abs() > 30.0)
        .count();
    format!(
        "rays median {:.0} (10th {:.0}, 90th {:.0}), largest {largest:.0} {toward}, within 10° of \
         the plane median {:.0} ({} rays); {high} rays beyond 10,000 ly at |b| over 30°",
        quantile(&sorted, 0.5),
        quantile(&sorted, 0.1),
        quantile(&sorted, 0.9),
        quantile(&in_plane, 0.5),
        in_plane.len()
    )
}

/// Prints each layer's caps and recount at `point`, and adds each failure of the gate to
/// `failures`.
fn check(
    point: [f64; 3],
    what: &str,
    caps: &[LayerCap],
    beyond: &[f64],
    failures: &mut Vec<String>,
) {
    for ((layer, cap), fine_beyond) in CAPPED_LAYERS.iter().zip(caps).zip(beyond) {
        eprintln!(
            "{point:?} {what} {layer:?}: rays median {:.0} ly, largest {:.0} (rule {:.0}), beyond \
             {:.3}, at 3,072 rays {fine_beyond:.3}",
            median_ray(cap),
            cap.radius().value(),
            cap.rule_bound().value(),
            cap.expected_beyond()
        );
        eprintln!("{point:?} {what} {layer:?}: {}", ray_record(cap, point));
        if *fine_beyond >= 1.5 {
            failures.push(format!("{point:?} {what} {layer:?}: {fine_beyond}"));
        }
    }
}

#[test]
#[ignore = "slow: builds the luminosity tables and counts 3,072 rays at six points, four times"]
fn caps_converge_in_rays() {
    let galaxy = Galaxy::from_params(Seed::new(0x0926_0000), GalaxyParams::milky_way_like())
        .expect("the Milky Way-like parameters are valid");
    let tables = LuminosityTables::build(&galaxy);
    let envelope = BrightnessEnvelope::build(&galaxy);
    let offsets = CellOffsets::build(&galaxy);
    let fine = caps_recount();
    let eye = EyeObserver::default();
    let mut failures = Vec::new();
    for (_, point) in CAPS_POINTS {
        let observer = Observer::new(
            GalacticPosition::from_light_years(point).expect("in the cube"),
            UniverseTime::EPOCH,
        )
        .expect("an observer");
        let mut cache = NoiseCache::with_capacity(1 << 16);
        for cut in [7.95, 8.54, 10.06].map(Magnitudes::new) {
            // `layer_caps`, from its count, with the spheres at the same count beside them.
            let count = CapCount::measure(
                &galaxy,
                &tables,
                &envelope,
                &observer,
                cut,
                CapResolution::STANDARD,
                &mut cache,
            );
            let caps = count.caps();
            for (sphere, cap) in count.spheres().iter().zip(&caps) {
                eprintln!(
                    "{point:?} at V {:.2} {:?}: the sphere at the same count {:.0} ly; systems \
                     opened {:.4e} by ray, {:.4e} in the sphere",
                    cut.value(),
                    sphere.layer(),
                    sphere.radius().value(),
                    count.systems_within(cap),
                    count.systems_within(sphere)
                );
            }
            // The brackets ruling's finding read as a count, not a ray's length (the science
            // check): D's and E's expected stars brighter than the cut beyond 10,000 ly more than
            // 30° from the plane, every other ray read to its rule bound, beyond which none counts.
            for cap in caps
                .iter()
                .filter(|c| matches!(c.layer(), Layer::D | Layer::E))
            {
                let beyond_high = count.stars_beyond_toward(cap.layer(), |u| {
                    if longitude_latitude(u.components(), point).1.abs() > 30.0 {
                        LightYears::new(10_000.0)
                    } else {
                        cap.rule_bound()
                    }
                });
                eprintln!(
                    "{point:?} at V {:.2} {:?}: {beyond_high:.4} stars expected beyond 10,000 ly \
                     at |b| over 30°",
                    cut.value(),
                    cap.layer()
                );
            }
            let beyond = expected_beyond_caps(
                &galaxy, &tables, &envelope, &observer, cut, fine, &caps, &mut cache,
            );
            let what = format!("at V {:.2}", cut.value());
            check(point, &what, &caps, &beyond, &mut failures);
        }
        // The caps by the eye's visibility, at the point's own eye's cut.
        let mut ctx = SkyContext {
            tables: &tables,
            envelope: &envelope,
            offsets: &offsets,
            noise: NoiseCache::with_capacity(1 << 16),
            cells: &NoSkyCellCache,
            sources: &[],
            modifiers: &NoModifiers,
        };
        let own = eye_cut(&galaxy, &mut ctx, &observer, &eye, None);
        let visibility = eye_visibility(&galaxy, &mut ctx, &observer, &eye, own, None);
        let seen = layer_caps_by_visibility(
            &galaxy,
            &tables,
            &envelope,
            &observer,
            &visibility,
            &mut cache,
        );
        let beyond = expected_beyond_caps_by_visibility(
            &galaxy,
            &tables,
            &envelope,
            &observer,
            &visibility,
            fine,
            &seen,
            &mut cache,
        );
        check(
            point,
            &format!("by the eye's visibility at V {:.3}", own.value()),
            &seen,
            &beyond,
            &mut failures,
        );
    }
    camera_black_sky_record(&galaxy, &tables, &envelope);
    assert!(failures.is_empty(), "{failures:#?}");
}

/// The camera's black-sky limit at 60°, V: Design note 18's read noise alone, 0.095 mag deeper
/// than its μ 24 figure, 10.06 (`decision-r06-t7b-brackets.md`).
const CAMERA_BLACK_SKY_V: f64 = 10.15;

/// R06.T17's record for the camera-budget ruling (`decision-r06-t7b-brackets.md`), printed and not
/// gated: near the Sun, from `layer_caps`' count alone and with no census run, the caps by ray at
/// the camera's black-sky limit beside those at its μ 24 cut, and the systems each opens.
fn camera_black_sky_record(
    galaxy: &Galaxy,
    tables: &LuminosityTables,
    envelope: &BrightnessEnvelope,
) {
    let (_, sun) = CAPS_POINTS[0];
    let observer = Observer::new(
        GalacticPosition::from_light_years(sun).expect("in the cube"),
        UniverseTime::EPOCH,
    )
    .expect("an observer");
    let mut cache = NoiseCache::with_capacity(1 << 16);
    let mut opened = [0.0; 2];
    for (k, cut) in [10.06, CAMERA_BLACK_SKY_V].into_iter().enumerate() {
        let count = CapCount::measure(
            galaxy,
            tables,
            envelope,
            &observer,
            Magnitudes::new(cut),
            CapResolution::STANDARD,
            &mut cache,
        );
        for cap in count.caps() {
            let systems = count.systems_within(&cap);
            opened[k] += systems;
            eprintln!(
                "near the Sun at the camera's V {cut:.2} {:?}: {}; {systems:.4e} systems opened",
                cap.layer(),
                ray_record(&cap, sun)
            );
        }
    }
    eprintln!(
        "near the Sun the camera's caps open {:.4e} systems at V 10.06 and {:.4e} at its black-sky \
         V {CAMERA_BLACK_SKY_V}: {:+.1}%",
        opened[0],
        opened[1],
        100.0 * (opened[1] / opened[0] - 1.0)
    );
    range_query_record(galaxy, tables, envelope);
}

/// The range query's own expected counts of D and E within 500 ly of (0, 26,000, 0) ly
/// (`benches/range_query.rs`' module docs, perf08's re-measure of 2026-09-29 at version 15, the
/// same field at version 21).
const RANGE_EXPECTED: (f64, f64) = (29_545.0, 8_170.0);

/// The systems `range_500ly_floor_d` realises within 500 ly at version 21, which the bench asserts:
/// 29,412 of D and 8,256 of E (R06.T17's re-measure; 37,675 at version 15).
const RANGE_REALISED: f64 = 37_668.0;

/// Prints R06.T17's check of the per-layer counts against the range query's `range_500ly_floor_d`.
///
/// For open question 19: the systems of D and E within 500 ly of (0, 26,000, 0) ly, the bench's
/// point, by the caps' count, beside the range query's own expected counts and its realised count,
/// on another seed of the same parameters. The caps' count integrates by the trapezoid in ln r and
/// splits its boundary's interval evenly, which over-counts a sphere by about 1% (the science
/// check), so the comparison is the range query's expectation with its realisation. Printed, not
/// gated.
fn range_query_record(galaxy: &Galaxy, tables: &LuminosityTables, envelope: &BrightnessEnvelope) {
    let observer = Observer::new(
        GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("in the cube"),
        UniverseTime::EPOCH,
    )
    .expect("an observer");
    let count = CapCount::measure(
        galaxy,
        tables,
        envelope,
        &observer,
        Magnitudes::new(7.95),
        CapResolution::STANDARD,
        &mut NoiseCache::with_capacity(1 << 16),
    );
    let within = |layer| count.systems_within(&LayerCap::forced(layer, LightYears::new(500.0)));
    let (d, e) = (within(Layer::D), within(Layer::E));
    let expected = RANGE_EXPECTED.0 + RANGE_EXPECTED.1;
    eprintln!(
        "within 500 ly of (0, 26,000, 0) ly the caps' count gives {d:.1} systems of D and {e:.1} \
         of E, {:.1} together, {:+.2}% over the range query's own expected {expected:.0} (D \
         {:+.2}%, E {:+.2}%); the range query realises {RANGE_REALISED:.0}, {:+.2}% of its expectation \
         (Poisson σ {:.2}%)",
        d + e,
        100.0 * ((d + e) / expected - 1.0),
        100.0 * (d / RANGE_EXPECTED.0 - 1.0),
        100.0 * (e / RANGE_EXPECTED.1 - 1.0),
        100.0 * (RANGE_REALISED / expected - 1.0),
        100.0 / expected.sqrt()
    );
}

/// R06.T11.h's record of the far stars brighter than V 3.0, layer by layer.
///
/// It is left out on wasm32-wasip1, which has no threads: it pins no bit, and its native run is the
/// record.
#[cfg(not(target_family = "wasm"))]
mod far_bright {
    use std::sync::Arc;

    use hyperion_sim::Seed;
    use hyperion_sim::coords::GalacticPosition;
    use hyperion_sim::galaxy::Galaxy;
    use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
    use hyperion_sim::galaxy::gas::noise::NoiseCache;
    use hyperion_sim::galaxy::params::GalaxyParams;
    use hyperion_sim::id::Layer;
    use hyperion_sim::observe::Observer;
    use hyperion_sim::sky::EyeObserver;
    use hyperion_sim::sky::caps::{
        CAP_RAYS, CapCount, CapCountPlan, CapLattice, RayExtinctions, SUB_RAYS,
        SYNTHETIC_CEILING_V, real_boundary, served_ceiling,
    };
    use hyperion_sim::sky::census::{CellOffsets, NoSkyCellCache, SkyContext};
    use hyperion_sim::sky::dgl::Illumination;
    use hyperion_sim::sky::envelope::BrightnessEnvelope;
    use hyperion_sim::sky::limits::{eye_cut, eye_visibility};
    use hyperion_sim::sky::luminosity::LuminosityTables;
    use hyperion_sim::time::UniverseTime;
    use hyperion_sim::units::{LightYears, Magnitudes};

    use crate::common::sky::SUN_LY;

    /// The apparent V brighter than which R06.T11.h counts the far stars: the V 3.0 delivery's
    /// (R06.T17).
    const BRIGHT_V: f64 = 3.0;

    /// The inner edge, ly, from which the server censuses E's shells a step ahead of C's and D's of
    /// the same edge (R06.T11.h): the fixed shell edge of 1,000 ly.
    const FAR_FROM_LY: f64 = 1_000.0;

    /// The seed of the server's sky tests and bench (`hyperion-server`'s `tests/sky.rs` and
    /// `benches/sky.rs`), whose galaxy the server builds as `Galaxy::new(seed)` with its full
    /// potential.
    const SERVER_SEED: u64 = 0x4d2;

    /// One galaxy's expected stars brighter than [`BRIGHT_V`] near the Sun for C, D and E, in that
    /// order, from the caps' count: within both [`FAR_FROM_LY`] and the real boundary R(u), between
    /// the two, and within R(u); and the record's line, printed once both galaxies are counted.
    struct FarBright {
        near: [f64; 3],
        between: [f64; 3],
        within: [f64; 3],
        line: String,
    }

    /// R06.T11.h's record for `galaxy`, named `name`.
    ///
    /// Near the Sun at the epoch, at the eye's cut as the server computes it for the default eye
    /// (`eye_cut` with the request's illumination, `Illumination::march` at the point), with its
    /// caps by the eye's visibility, the ceiling the server states (`served_ceiling` of
    /// [`SYNTHETIC_CEILING_V`]) and the real boundary of the two counts over the same rays
    /// (`real_boundary`): each of C's, D's and E's expected stars brighter than [`BRIGHT_V`],
    /// counted over the same rays at that cut, within both 1,000 ly and R(u), between them, and
    /// within R(u). The server's census reads these caps bit for bit (its `compute::sky` tests of
    /// the plan), so the count between is that of the served census's shells beyond 1,000 ly, the
    /// cells' edges aside.
    fn far_bright_record(name: &str, galaxy: &Galaxy) -> FarBright {
        let tables = LuminosityTables::build(galaxy);
        let envelope = BrightnessEnvelope::build(galaxy);
        let offsets = CellOffsets::build(galaxy);
        let observer = Observer::new(
            GalacticPosition::from_light_years(SUN_LY).expect("in the cube"),
            UniverseTime::EPOCH,
        )
        .expect("an observer");
        let mut ctx = SkyContext {
            tables: &tables,
            envelope: &envelope,
            offsets: &offsets,
            noise: NoiseCache::with_capacity(1 << 16),
            cells: &NoSkyCellCache,
            sources: &[],
            modifiers: &NoModifiers,
        };
        let eye = EyeObserver::default();
        let light = Illumination::march(galaxy, &mut ctx, &observer);
        let cut = eye_cut(galaxy, &mut ctx, &observer, &eye, Some(&light));
        let visibility = eye_visibility(galaxy, &mut ctx, &observer, &eye, cut, Some(&light));
        let rays = RayExtinctions::measure_clearest(
            galaxy,
            observer.position(),
            Arc::new(CapLattice::new(CAP_RAYS)),
            SUB_RAYS,
            &mut ctx.noise,
        );
        let count = |plan: CapCountPlan| {
            let part = plan.count_rays(galaxy, &tables, &rays, 0..plan.rays());
            plan.join([part])
        };
        let at_cut = count(CapCount::plan_by_visibility_over(
            galaxy,
            &tables,
            &envelope,
            &observer,
            &visibility,
            &rays,
        ));
        let caps = at_cut.caps();
        let ceiling = served_ceiling(cut, Magnitudes::new(SYNTHETIC_CEILING_V), &caps)
            .expect("near the Sun the eye's cut is deeper than the ceiling");
        let at_ceiling = count(CapCount::plan_over(
            galaxy, &tables, &envelope, &observer, ceiling, &rays,
        ));
        let real = real_boundary(&at_ceiling, &at_cut, &caps);
        let bright = count(CapCount::plan_over(
            galaxy,
            &tables,
            &envelope,
            &observer,
            Magnitudes::new(BRIGHT_V),
            &rays,
        ));
        let far = LightYears::new(FAR_FROM_LY);
        let mut record = FarBright {
            near: [0.0; 3],
            between: [0.0; 3],
            within: [0.0; 3],
            line: String::new(),
        };
        let mut lines = Vec::new();
        for (k, layer) in [Layer::C, Layer::D, Layer::E].into_iter().enumerate() {
            let boundary = real
                .iter()
                .find(|cap| cap.layer() == layer)
                .expect("a real boundary a layer");
            let r = |u| boundary.radius_toward(u);
            record.near[k] = bright.stars_within_and_beyond_toward(
                layer,
                |u| LightYears::new(r(u).value().min(FAR_FROM_LY)),
                |_| LightYears::ZERO,
            );
            record.between[k] = bright.stars_within_and_beyond_toward(layer, r, |_| far);
            record.within[k] =
                bright.stars_within_and_beyond_toward(layer, r, |_| LightYears::ZERO);
            lines.push(format!(
                "{layer:?} {:.3} within 1,000 ly, {:.3} between 1,000 ly and R(u), {:.3} within \
                 R(u) (R(u) largest {:.0} ly)",
                record.near[k],
                record.between[k],
                record.within[k],
                boundary.radius().value()
            ));
        }
        let [c, d, e] = record.between;
        record.line = format!(
            "{name} near the Sun, at the served eye's cut V {:.3} and ceiling V {:.1}, the \
             expected stars brighter than V {BRIGHT_V:.1} by the caps' count: {}; between \
             1,000 ly and R(u), C to E {:.3}, D's share {:.3} and E's {:.3}",
            cut.value(),
            ceiling.value(),
            lines.join("; "),
            c + d + e,
            d / (c + d + e),
            e / (c + d + e)
        );
        record
    }

    /// Near the Sun, on the test fixture and on the server's galaxy, D's expected stars brighter
    /// than V 3.0 between 1,000 ly and its real boundary are at most a third of C's, D's and E's
    /// together, by the caps' count alone with no census (R06.T11.h; decided 2026-10-13,
    /// `decision-r06-t17-findings.md` §1).
    ///
    /// It is the rule by which the server's delivery censuses E's shells beyond 1,000 ly a step
    /// ahead of C's and D's, D's left with C's (`hyperion-server`'s
    /// `compute::sky::delivery_steps`): were D's share over a third, D's shells would move with
    /// E's. The figures are in R06's Risks ("Deviations in T11.h, as built"). The two galaxies are
    /// counted on two threads, one each, and their records printed after, the fixture's first.
    #[test]
    #[ignore = "slow: builds two galaxies' luminosity tables and counts the caps near the Sun \
                three times on each"]
    fn beyond_1000_ly_d_holds_at_most_a_third_of_the_far_stars_brighter_than_v_3() {
        let records = std::thread::scope(|scope| {
            let fixture = scope.spawn(|| {
                let galaxy =
                    Galaxy::from_params(Seed::new(0x0926_0000), GalaxyParams::milky_way_like())
                        .expect("the Milky Way-like parameters are valid");
                far_bright_record("the test fixture", &galaxy)
            });
            let server = scope.spawn(|| {
                let galaxy = Galaxy::new(Seed::new(SERVER_SEED)).with_full_potential();
                far_bright_record("the server's galaxy", &galaxy)
            });
            [
                (
                    "the test fixture",
                    fixture.join().expect("the fixture's record"),
                ),
                (
                    "the server's galaxy",
                    server.join().expect("the server galaxy's record"),
                ),
            ]
        });
        for (_, record) in &records {
            eprintln!("{}", record.line);
        }
        for (name, record) in records {
            let [c, d, e] = record.between;
            assert!(
                c >= 0.0 && d >= 0.0 && e >= 0.0 && c + d + e > 0.0,
                "{name}: counts {:?}",
                record.between
            );
            assert!(
                d <= (c + d + e) / 3.0,
                "{name}: D's {d:.3} of the {:.3} stars brighter than V {BRIGHT_V} between \
                 1,000 ly and R(u) is over a third, so D's shells move with E's (R06.T11.h); \
                 within both {:?}, within R(u) {:?}",
                c + d + e,
                record.near,
                record.within
            );
        }
    }
}
