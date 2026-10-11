//! The sky near the Sun, pinned for every target: the eye's cut, the census's listing and the band
//! march's texels, each compared with a golden file (`decision-test-coverage.md`, W-R1, 2026-10-11).
//!
//! The tests of [`band`](super::band), [`limits`](super::limits), [`caps`](super::caps),
//! [`dgl`](super::dgl), [`luminosity`](super::luminosity) and the census's cell and query compare
//! computations of one run with each other, or with a tolerance, so they prove nothing about the
//! bits one target gives against another's. The golden here holds the bits themselves, blessed
//! natively and compared by `just test-wasm-fast` on wasm32-wasip1. It is the cross-target pin of
//! the march, the eye's cut and the census, which no golden held on any target before it.
//!
//! The module is its own so that the ruling's filter of the sky's table modules
//! (`sky::(band|limits|caps|dgl|luminosity|census::(cell|query))::`), whose tests `ci`'s wasip1 run
//! is to leave to the daily slow pass once the filter lands, never reaches it.

use std::sync::Arc;

use hyperion_testkit::float::bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::{GoldenWriter, f32_digest};

use crate::galaxy::features::centre::testing::milky_way_galaxy;
use crate::units::{LightYears, Magnitudes};

use super::EyeObserver;
use super::band::{BandSpec, BandTexel, CompleteTo, CubeFace, march_rows, sum_rows};
use super::caps::{SYNTHETIC_CEILING_V, served_ceiling};
use super::census::{
    CensusPlan, CensusTallies, SkyCensus, SkyContext, SkyQuery, census_cell, census_plan,
    merge_shells,
};
use super::limits::{EyeCutSteps, PRE_PASS_SPEC, pre_pass_rows};
use super::testing::{SUN_LY, milky_way_context, sun_illumination, sun_observer};

/// How far the pinned census looks, ly: every layer's cap is forced to it.
///
/// At the eye's cut near the Sun, about V 8.18, and RM3's synthetic ceiling of V 5.0, the census
/// opens 526 cells and lists 66 stars: A 4, B 36, C 23 and D 3, singles and companions. At the
/// ceiling C, D and E list only their stars within the cap towards each star's texel; the cells
/// opened whole hold more (R06's one-shot census of them, with no ceiling, lists C 108, D 29 and
/// E 10). No E star and no brown dwarf that bright lies within it. A census this near costs a few
/// seconds, beside the tables' build.
const CENSUS_RADIUS_LY: f64 = 30.0;

/// The pinned band's face side, texels: the eye cut's pre-pass's 16², 1,536 rays.
const BAND_FACE_TEXELS: u16 = 16;

/// The sky the server serves the default eye near the Sun at a band of 16² texels a face, every cap
/// forced to [`CENSUS_RADIUS_LY`], pinned bit for bit for every target.
///
/// The observer is the fixture's Sun at the epoch, the eye the default, and the request lit by the
/// Sun's illumination (R06.T9.g). Each step is the sim's half of `hyperion-server`'s
/// `compute::sky`, which runs the same calls as jobs:
///
/// 1. The eye's cut ([`eye_cut_in_steps`]).
/// 2. The eye-only request at that cut, its caps forced to [`CENSUS_RADIUS_LY`], at the synthetic
///    ceiling [`served_ceiling`] gives its cut and caps, as the server's plan states it (rendering
///    plan R13, R13.T2.b): near the Sun, RM3's V 5.0, so that C, D and E list only within their
///    caps towards each star's band texel. The ceiling is written.
/// 3. Its census ([`census_listed`]).
/// 4. Its band, marched for its plan's replies and summed with the census at the radii it is
///    complete to ([`band_summed`]).
///
/// It builds the luminosity tables, as every test of the sky that reads them does: most of its
/// minute natively (2026-10-11, at a load of about 15; provisional). The illumination, the two
/// pre-pass bands, the census and the band take some 15 s of it.
#[test]
fn near_the_sun_golden() {
    let mut ctx = milky_way_context();
    let mut w = GoldenWriter::new();
    w.header(crate::GENERATOR_VERSION.get());
    w.line(&format!(
        "near the Sun ({}, {}, {}) ly at the epoch, the default eye, lit by its illumination",
        SUN_LY[0], SUN_LY[1], SUN_LY[2]
    ));
    let eye = EyeObserver::default();
    let cut = eye_cut_in_steps(&mut ctx, &eye, &mut w);
    let spec = BandSpec::new(BAND_FACE_TEXELS, BandSpec::STANDARD.nodes_per_decade())
        .expect("16² faces on the standard nodes are a band");
    let query = SkyQuery::builder(sun_observer(), cut)
        .eye(eye)
        .illumination(Arc::clone(sun_illumination()))
        .band_spec(spec)
        .build()
        .expect("the eye-only request at its cut")
        .with_caps_forced(LightYears::new(CENSUS_RADIUS_LY))
        .expect("a forced cap within the cube");
    let ceiling = served_ceiling(
        query.cut(),
        Magnitudes::new(SYNTHETIC_CEILING_V),
        query.forced_caps().expect("the query's caps are forced"),
    )
    .expect("near the Sun the eye's cut is deeper than the ceiling");
    w.f64("the synthetic ceiling, V", ceiling.value());
    let query = query
        .with_synthetic_ceiling(ceiling)
        .expect("a served ceiling is no fainter than the cut");
    let (plan, census) = census_listed(&mut ctx, &query, &mut w);
    band_summed(&mut ctx, &query, &plan, &census, &mut w);
    golden!("sky/near_sun", w.as_str());
}

/// The eye's cut near the Sun for `eye`, lit by the Sun's illumination, as the server takes it.
///
/// [`EyeCutSteps`] takes each pass from the faces' pre-pass rows ([`pre_pass_rows`]; R06.T9.d,
/// R06.T11.g). The cut each pass's band was marched at, then the cut, are written to `w`.
fn eye_cut_in_steps(
    ctx: &mut SkyContext<'_>,
    eye: &EyeObserver,
    w: &mut GoldenWriter,
) -> Magnitudes {
    let (galaxy, sun, light) = (milky_way_galaxy(), sun_observer(), sun_illumination());
    w.line("the eye's cut, in its steps");
    let mut steps = EyeCutSteps::new(sun, *eye);
    let mut pass = 0_u32;
    while let Some(band_cut) = steps.next_band_cut() {
        pass += 1;
        w.f64(&format!("  pass {pass} marched at V"), band_cut.value());
        steps.take(CubeFace::ALL.into_iter().map(|face| {
            pre_pass_rows(
                galaxy,
                ctx,
                &sun,
                eye,
                band_cut,
                Some(light),
                face,
                0..PRE_PASS_SPEC.face_texels(),
            )
        }));
    }
    let cut = steps
        .cut()
        .expect("the steps end once the eye's cut is known");
    w.f64("  the eye's cut, V", cut.value());
    cut
}

/// The census of `query`, whose caps are forced, and its plan, as the server takes them.
///
/// Its plan's cells are opened one by one in the plan's order and merged at the query's `n_max` by
/// [`merge_shells`], to the plan's whole completeness, as the server merges its last reply. Each
/// listed star's system ID, star index, layer and V are written to `w`, in the census's order.
fn census_listed(
    ctx: &mut SkyContext<'_>,
    query: &SkyQuery,
    w: &mut GoldenWriter,
) -> (CensusPlan, SkyCensus) {
    let galaxy = milky_way_galaxy();
    let plan = census_plan(galaxy, ctx.tables, ctx.envelope, query, &mut ctx.noise);
    let mut stars = Vec::new();
    let mut tallies = CensusTallies::default();
    for key in plan.cells() {
        tallies.add(&census_cell(galaxy, ctx, key, query, &mut stars));
    }
    let census = merge_shells([(stars, tallies)], query.n_max(), plan.complete());
    assert!(
        census.overflow().is_empty(),
        "the census lists every star it keeps"
    );
    // The counts are labelled lines, so that the heading above the stars, which `golden_diff.py`
    // keys their lines on, stays the same when a count moves.
    w.line(&format!(
        "the census at the eye's cut, every cap forced to {CENSUS_RADIUS_LY} ly"
    ));
    w.line(&format!("  cells = {}", plan.cell_count()));
    w.line(&format!("  stars listed = {}", census.listed().len()));
    for star in census.listed() {
        w.f64(
            &format!(
                "  {:#018x}/{} {:?} V",
                star.system().raw(),
                star.star().get(),
                star.layer()
            ),
            star.v().value(),
        );
    }
    (plan, census)
}

/// The band of `query` at its band spec, as the server marches and sums a reply's.
///
/// Each face is marched by [`march_rows`] once for every reply `plan`, the query's, can state,
/// then summed with `census` by [`sum_rows`] at the radii `census` is complete to (R06.T9.f). Every
/// texel's photopic luminance and S/P ratio, its scotopic light over its photopic, are written to
/// `w` face by face in layer order, rows from the top and columns from the left, then a digest of
/// every texel's chroma.
fn band_summed(
    ctx: &mut SkyContext<'_>,
    query: &SkyQuery,
    plan: &CensusPlan,
    census: &SkyCensus,
    w: &mut GoldenWriter,
) {
    let spec = query.band_spec();
    let complete_to: &CompleteTo = census
        .completeness()
        .expect("a census of shells")
        .complete_to();
    let replies = plan.replies();
    let mut band: Vec<BandTexel> = Vec::new();
    for face in CubeFace::ALL {
        let march = march_rows(
            milky_way_galaxy(),
            ctx,
            query,
            replies.iter().cloned(),
            &spec,
            face,
            0..spec.face_texels(),
        );
        sum_rows(&march, census, complete_to, &mut band);
    }
    let side = usize::from(spec.face_texels());
    let per_face = side * side;
    assert_eq!(band.len(), CubeFace::ALL.len() * per_face);
    // One line a texel, its two values' bits in hexadecimal without the `0x` of `u64_hex`: 1,536
    // lines of 47 bytes, which keep the file well within its 100 kB.
    w.line(&format!(
        "the band at {side}² texels a face, {} nodes a decade, complete to the census's radii; \
         each texel, by face, row and column: the IEEE 754 bits, in hexadecimal, of its photopic \
         luminance (cd m⁻²) and of its S/P ratio",
        spec.nodes_per_decade()
    ));
    for (face, texels) in CubeFace::ALL.iter().zip(band.chunks_exact(per_face)) {
        for (i, texel) in texels.iter().enumerate() {
            w.line(&format!(
                "{face:?} {:02} {:02} = {:016x} {:016x}",
                i / side,
                i % side,
                bits(texel.luminance().value()),
                bits(texel.sp_ratio())
            ));
        }
    }
    let chroma: Vec<f32> = band.iter().flat_map(BandTexel::chroma).collect();
    // The digest hashes the bits unchecked, and a NaN's are not portable.
    assert!(
        chroma.iter().all(|c| c.is_finite()),
        "every texel's chroma is finite"
    );
    w.u64_hex(
        "every texel's red and green chroma (f32), digest",
        f32_digest(&chroma),
    );
}
