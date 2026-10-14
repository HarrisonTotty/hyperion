//! The surveyed readings on the synthetic worlds, through a client's own field, and on analytic
//! ground (plan R10, T5).

use std::cell::RefCell;

use hyperion_base::units::Metres;
use hyperion_testkit::float::bits;
use hyperion_testkit::lcg::Lcg;
use hyperion_testkit::order::assert_order_independent;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::cube::Face;
use crate::field::{
    ClimateCell, CoarseCrater, CoarseField, Cover, CoverRange, FieldHeader, SynthesisCell,
    cell_index,
};
use crate::synth::test_support::{SEED, king_distances, point_in, random_cells, world};
use crate::testing::SyntheticWorld;
use crate::wire::{decode_payload, encode_payload};

/// The four bodies of the synthetic worlds, which have ground finer than their cells.
const BODIES: [SyntheticWorld; 4] = [
    SyntheticWorld::EarthLike,
    SyntheticWorld::MarsLike,
    SyntheticWorld::MoonLike,
    SyntheticWorld::CeresLike,
];

/// The code of a survey at `metres`.
fn code(metres: f64) -> ResolutionCode {
    ResolutionCode::from_resolution(Metres::new(metres)).unwrap()
}

/// A view of `inner`, a field that holds every cell, surveyed as `code_of` says.
struct Surveyed<'a> {
    inner: &'a CoarseField,
    code_of: Box<dyn Fn(PatchKey) -> Option<ResolutionCode>>,
}

impl<'a> Surveyed<'a> {
    /// `inner` surveyed everywhere at `code`.
    fn everywhere(inner: &'a CoarseField, code: ResolutionCode) -> Self {
        Self {
            inner,
            code_of: Box::new(move |_| Some(code)),
        }
    }
}

impl FieldView for Surveyed<'_> {
    fn header(&self) -> &FieldHeader {
        self.inner.header()
    }

    fn cell(&self, cell: PatchKey) -> Option<&SynthesisCell> {
        self.inner.cell(cell)
    }

    fn climate(&self, cell: PatchKey) -> Option<&ClimateCell> {
        self.inner.climate(cell)
    }

    fn craters_reaching(&self, cell: PatchKey) -> impl Iterator<Item = &CoarseCrater> + '_ {
        self.inner.craters_reaching(cell)
    }
}

impl SurveyView for Surveyed<'_> {
    fn resolution(&self, cell: PatchKey) -> Option<ResolutionCode> {
        (self.code_of)(cell)
    }
}

/// `count` points at random in each of `cells` cells of `w`'s field level, as face coordinates
/// and their unit directions.
fn sample_points(w: SyntheticWorld, cells: usize, count: usize, rng: &mut Lcg) -> Vec<[f64; 3]> {
    let level = world(w).header().level().get();
    random_cells(level, cells, rng)
        .into_iter()
        .flat_map(|cell| {
            (0..count)
                .map(|_| point_in(cell, rng.next_f64(), rng.next_f64()))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The face coordinates of the unit direction `dir`.
fn at(dir: [f64; 3]) -> FaceUv {
    xyz_to_face_uv(dir)
}

/// The unit direction a reading at `dir`'s face coordinates reads.
fn read_dir(dir: [f64; 3]) -> [f64; 3] {
    unit_dir(face_uv_to_xyz(at(dir)))
}

/// The cell of the field's level holding `dir`.
fn cell_of(field: &impl FieldView, dir: [f64; 3]) -> PatchKey {
    PatchKey::containing(field.header().level().get(), dir).unwrap()
}

#[test]
fn at_a_band_limit_survey_the_elevation_is_the_ground_s_and_certain() {
    let mut rng = Lcg::new(0x7265_6164_0001);
    for &w in SyntheticWorld::ALL {
        let field = world(w);
        let figure = field.header().figure();
        // A pass at 2 m codes "2.15 m or finer", which reaches the band limit; so does a finer one.
        for survey in [code(2.0), code(0.5)] {
            let view = Surveyed::everywhere(field, survey);
            let synth = Synthesiser::new(&view, SEED);
            let mut cache = SynthCache::new();
            for dir in sample_points(w, 4, 6, &mut rng) {
                let reading = surveyed_reading(&synth, &mut cache, at(dir), SLOPE_BASELINES[0])
                    .unwrap()
                    .unwrap();
                let ground = ground_at(&synth, &mut cache, &figure.point(read_dir(dir))).unwrap();
                assert_eq!(
                    bits(reading.elevation().value()),
                    bits(ground.height().value()),
                    "{w:?} {dir:?}"
                );
                assert!(reading.is_resolved(), "{w:?} {dir:?}");
                assert_eq!(bits(reading.elevation_sigma().value()), bits(0.0));
                assert_eq!(bits(reading.slope_sigma().value()), bits(0.0));
                assert_eq!(reading.slope_baseline(), SLOPE_BASELINES[0]);
            }
        }
        // Just coarser than the band limit's code is not resolved.
        assert!(!survey_level(&figure, code(2.3)).resolved, "{w:?}");
    }
}

#[test]
fn a_coarser_survey_is_read_at_the_deepest_level_it_resolves() {
    let mut rng = Lcg::new(0x7265_6164_0002);
    for &w in &BODIES {
        let field = world(w);
        let figure = field.header().figure();
        for metres in [3.0, 20.0, 100.0, 750.0, 5_000.0] {
            let survey = code(metres);
            let level = survey_level(&figure, survey);
            assert!(!level.resolved, "{w:?} at {metres} m");
            // Its shortest wavelength codes no finer than the survey; the next level's codes finer.
            assert!(
                code(level.wavelength.value()) >= survey,
                "{w:?} at {metres} m"
            );
            let deeper = BandLevel::new(level.band.get() + 1).unwrap();
            assert!(
                code(shortest_wavelength(&figure, deeper).value()) < survey,
                "{w:?} at {metres} m"
            );
            let view = Surveyed::everywhere(field, survey);
            let synth = Synthesiser::new(&view, SEED);
            let mut cache = SynthCache::new();
            for dir in sample_points(w, 2, 4, &mut rng) {
                let reading = surveyed_reading(&synth, &mut cache, at(dir), SLOPE_BASELINES[2])
                    .unwrap()
                    .unwrap();
                let dir = read_dir(dir);
                let cell = cell_of(field, dir);
                let height = synth.height_at(&mut cache, dir, level.band).unwrap();
                assert_eq!(
                    bits(reading.elevation().value()),
                    bits(height.height_m),
                    "{w:?} at {metres} m"
                );
                let omitted = synth.omitted_rms(cell, level.band).unwrap().value();
                let sigma = reading.elevation_sigma().value();
                assert_eq!(bits(sigma), bits(omitted));
                assert_eq!(reading.resolution(), survey.resolution().unwrap());
                // At least R09's RMS at the level's shortest wavelength, and so at the finest
                // resolution the survey's code admits: the safe side.
                let at_lambda = synth.unresolved_rms(cell, level.wavelength).unwrap();
                assert!(sigma >= at_lambda.value(), "{w:?} at {metres} m");
                assert!(!reading.is_resolved());
            }
        }
    }
}

/// A client's field of `w` from a payload of the cells `surveyed` at `code`, with their margin.
fn client_field(w: SyntheticWorld, surveyed: &[PatchKey], code: ResolutionCode) -> PartialField {
    let field = world(w);
    let mut indices: Vec<u32> = surveyed.iter().map(|&c| cell_index(c)).collect();
    indices.sort_unstable();
    indices.dedup();
    let cover = Cover::from_ranges(
        indices
            .iter()
            .map(|&i| CoverRange::new(i, i + 1, code).unwrap()),
    )
    .unwrap();
    let blocks = decode_payload(&encode_payload(field, &cover, None)).unwrap();
    let mut client = PartialField::new(blocks[0].header().unwrap().clone());
    for block in &blocks {
        client.insert(block).unwrap();
    }
    client
}

#[test]
fn unsurveyed_ground_has_no_reading() {
    let w = SyntheticWorld::MoonLike;
    let level = world(w).header().level().get();
    let centre = PatchKey::new(Face::PosY, level, 20, 30).unwrap();
    let client = client_field(w, &[centre], code(50.0));
    let synth = Synthesiser::new(&client, SEED);
    let mut cache = SynthCache::new();
    let ring = king_distances(centre, 7);
    let reading = |cell: PatchKey, cache: &mut SynthCache| {
        surveyed_reading(
            &synth,
            cache,
            at(point_in(cell, 0.4, 0.6)),
            SLOPE_BASELINES[0],
        )
    };
    assert!(reading(centre, &mut cache).unwrap().is_some());
    for (&cell, &steps) in &ring {
        match steps {
            0 => {}
            // A margin cell is held, and its heights may be known, but it is not surveyed.
            1..=5 => {
                assert_eq!(client.resolution(cell), Some(ResolutionCode::NONE));
                assert_eq!(reading(cell, &mut cache).unwrap(), None, "{cell:?}");
            }
            _ => {
                assert_eq!(client.resolution(cell), None);
                assert_eq!(reading(cell, &mut cache).unwrap(), None, "{cell:?}");
            }
        }
    }
}

#[test]
fn a_reading_through_a_client_s_field_is_the_whole_field_s() {
    let mut rng = Lcg::new(0x7265_6164_0003);
    let w = SyntheticWorld::MarsLike;
    let field = world(w);
    let level = field.header().level().get();
    let cells = random_cells(level, 3, &mut rng);
    let survey = code(40.0);
    let client = client_field(w, &cells, survey);
    let whole = Surveyed::everywhere(field, survey);
    let (a, b) = (
        Synthesiser::new(&client, SEED),
        Synthesiser::new(&whole, SEED),
    );
    let mut cache = SynthCache::new();
    for cell in cells {
        for _ in 0..4 {
            let dir = point_in(cell, rng.next_f64(), rng.next_f64());
            let baseline = SLOPE_BASELINES[1];
            let from_client = surveyed_reading(&a, &mut cache, at(dir), baseline).unwrap();
            let from_whole = surveyed_reading(&b, &mut SynthCache::new(), at(dir), baseline);
            assert_eq!(from_client, from_whole.unwrap(), "{cell:?}");
        }
    }
}

/// `a` less `b`.
fn sub3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

#[test]
fn over_a_short_baseline_the_resolved_slope_is_the_gradient_s_tangential_part() {
    let mut rng = Lcg::new(0x7265_6164_0004);
    for &w in &BODIES {
        let field = world(w);
        let figure = field.header().figure();
        for survey in [code(10.0), code(300.0)] {
            let view = Surveyed::everywhere(field, survey);
            let synth = Synthesiser::new(&view, SEED);
            let mut cache = SynthCache::new();
            let level = survey_level(&figure, survey);
            for dir in sample_points(w, 3, 3, &mut rng) {
                let sample = synth.height_at(&mut cache, dir, level.band).unwrap();
                let cell = cell_of(field, dir);
                let baseline = Metres::new(0.01);
                let terms = slope_terms(
                    &synth,
                    &mut cache,
                    dir,
                    sample.height_m,
                    cell,
                    level,
                    baseline,
                )
                .unwrap();
                // The gradient's tangential part (R09.T4), per metre of datum, and per metre at the
                // ground's height along each principal direction: east along the prime vertical,
                // of radius N, north along the meridian, of radius M.
                let up = figure.normal(dir);
                let g = sample.gradient;
                let along = dot(g, up);
                let tangent = [0, 1, 2].map(|k| g[k] - along * up[k]);
                let (east, north) = east_north(up);
                let (m, n) = figure.curvature_radii_m(dir);
                let h = sample.height_m;
                let expected = [
                    dot(tangent, east) * n / (n + h),
                    dot(tangent, north) * m / (m + h),
                ];
                for (got, want) in terms.resolved.iter().zip(expected) {
                    assert!(
                        (got - want).abs() <= 1e-4 * want.abs() + 1e-7,
                        "{w:?} {dir:?}: {got} against {want}"
                    );
                }
            }
        }
    }
    // The axes are a right-handed tangent frame, at a pole too.
    for up in [
        [0.0, 0.0, 1.0],
        [0.0, 0.0, -1.0],
        unit_dir([1.0, -2.0, 0.5]),
    ] {
        let (east, north) = east_north(up);
        for (a, b) in [(east, north), (east, up), (north, up)] {
            assert!(dot(a, b).abs() < 1e-15);
        }
        let rh = sub3(cross(east, north), up);
        assert!(dot(rh, rh) < 1e-30, "{up:?}");
    }
}

#[test]
fn at_long_and_short_baselines_the_unresolved_slope_takes_design_note_7_s_limits() {
    let mut rng = Lcg::new(0x7265_6164_0005);
    for &w in &BODIES {
        let field = world(w);
        let figure = field.header().figure();
        let survey = code(10.0);
        let view = Surveyed::everywhere(field, survey);
        let synth = Synthesiser::new(&view, SEED);
        let mut cache = SynthCache::new();
        let level = survey_level(&figure, survey);
        let lambda = level.wavelength.value();
        for dir in sample_points(w, 3, 2, &mut rng) {
            let cell = cell_of(field, dir);
            let height = synth
                .height_at(&mut cache, dir, level.band)
                .unwrap()
                .height_m;
            let mut variance = |b: f64| {
                slope_terms(&synth, &mut cache, dir, height, cell, level, Metres::new(b))
                    .unwrap()
                    .axis_variance
            };
            let sigma = synth.omitted_rms(cell, level.band).unwrap().value();
            // Much longer than every unresolved band: √v → √2 σ ÷ b.
            let long = 300.0 * lambda;
            let ratio = variance(long).sqrt() * long / (2.0_f64.sqrt() * sigma);
            assert!((ratio - 1.0).abs() < 0.05, "{w:?} {dir:?}: {ratio}");
            // Much shorter than the band limit: v tends to the bands' gradient variance, a
            // constant, as D(b) tends to it times b².
            let (fine, finer) = (variance(1e-3), variance(1e-4));
            assert!(fine > 0.0, "{w:?} {dir:?}");
            assert!(
                (fine / finer - 1.0).abs() < 1e-2,
                "{w:?} {dir:?}: {fine} {finer}"
            );
        }
    }
}

#[test]
fn the_rms_slope_is_never_below_the_resolved_slope() {
    let mut rng = Lcg::new(0x7265_6164_0006);
    for &w in SyntheticWorld::ALL {
        let field = world(w);
        let figure = field.header().figure();
        for survey in [code(1.0), code(8.0), code(150.0), code(2_000.0)] {
            let view = Surveyed::everywhere(field, survey);
            let synth = Synthesiser::new(&view, SEED);
            let mut cache = SynthCache::new();
            let level = survey_level(&figure, survey);
            for dir in sample_points(w, 2, 2, &mut rng) {
                for baseline in SLOPE_BASELINES {
                    let reading = reading_over(&synth, &mut cache, dir, baseline)
                        .unwrap()
                        .unwrap();
                    let height = reading.elevation().value();
                    let cell = cell_of(field, dir);
                    let terms = slope_terms(&synth, &mut cache, dir, height, cell, level, baseline)
                        .unwrap();
                    let resolved = math::atan(terms.resolved_magnitude());
                    let rms = reading.slope_rms().value();
                    assert!(rms >= resolved, "{w:?} {dir:?}: {rms} below {resolved}");
                    if terms.axis_variance > 0.0 {
                        assert!(rms > resolved);
                        assert!(reading.slope_sigma().value() > 0.0);
                    }
                    assert!(rms < FRAC_PI_2 && reading.slope_sigma().value() >= 0.0);
                }
            }
        }
    }
}

#[test]
fn readings_do_not_depend_on_the_cache_or_their_order() {
    let mut rng = Lcg::new(0x7265_6164_0007);
    let w = SyntheticWorld::EarthLike;
    let view = Surveyed::everywhere(world(w), code(25.0));
    let synth = Synthesiser::new(&view, SEED);
    let shared = RefCell::new(SynthCache::new());
    let points = sample_points(w, 3, 3, &mut rng);
    assert_order_independent(&points, |&dir| {
        let read = |cache: &mut SynthCache| {
            surveyed_reading(&synth, cache, at(dir), SLOPE_BASELINES[0])
                .unwrap()
                .map(|r| {
                    [
                        r.elevation().value(),
                        r.elevation_sigma().value(),
                        r.slope_rms().value(),
                        r.slope_sigma().value(),
                    ]
                    .map(bits)
                })
        };
        let warm = read(&mut shared.borrow_mut());
        assert_eq!(warm, read(&mut SynthCache::new()));
        warm
    });
}

#[test]
fn the_rice_spread_is_rayleigh_s_with_no_resolved_slope_and_tends_to_sigma() {
    assert_eq!(bits(laguerre_half(0.0)), bits(1.0));
    for variance in [1e-6, 0.04, 3.0] {
        let rayleigh = (variance * (2.0 - FRAC_PI_2)).sqrt();
        let got = rice_sigma(0.0, variance);
        assert!((got / rayleigh - 1.0).abs() < 1e-14, "{got} {rayleigh}");
        // Far above the spread, σ (1 − 1 ÷ 8x) to first order.
        let sigma = variance.sqrt();
        let nu = 1e3 * sigma;
        let x = nu * nu / (2.0 * variance);
        let far = rice_sigma(nu, variance);
        assert!((far / sigma - (1.0 - 1.0 / (8.0 * x))).abs() < 1e-12);
    }
    assert_eq!(bits(rice_sigma(0.3, 0.0)), bits(0.0));
}

#[test]
fn kummer_s_series_and_the_asymptotic_series_agree_where_they_meet() {
    for x in [RICE_ASYMPTOTIC_FROM, 30.0, 60.0] {
        let l = laguerre_half(x);
        let series = 2.0 + 2.0 * x - FRAC_PI_2 * l * l;
        let r = 1.0 / x;
        let asymptotic = 1.0 - r * (0.25 + r * (0.125 + r * (11.0 / 64.0)));
        assert!(
            (series - asymptotic).abs() < 1e-6,
            "x = {x}: {series} against {asymptotic}"
        );
    }
}

/// A standard normal pair from `rng`, by Box and Muller's transform.
fn normal_pair(rng: &mut Lcg) -> (f64, f64) {
    let u = 1.0 - rng.next_f64();
    let r = (-2.0 * math::ln(u)).sqrt();
    let (s, c) = math::sin_cos(2.0 * core::f64::consts::PI * rng.next_f64());
    (r * c, r * s)
}

#[test]
fn the_rice_spread_matches_a_sample_of_slopes() {
    let mut rng = Lcg::new(0x7265_6164_0008);
    let draws = 200_000;
    for nu in [0.0, 0.5, 1.0, 2.0, 4.0, 12.0] {
        let (mut sum, mut sum2) = (0.0, 0.0);
        for _ in 0..draws {
            let (a, b) = normal_pair(&mut rng);
            let g = ((nu + a) * (nu + a) + b * b).sqrt();
            sum += g;
            sum2 += g * g;
        }
        let n = f64::from(draws);
        let mean = sum / n;
        let sampled = (sum2 / n - mean * mean).sqrt();
        let expected = rice_sigma(nu, 1.0);
        // The sample's standard deviation is good to about 0.2% here.
        assert!(
            (sampled / expected - 1.0).abs() < 0.01,
            "ν = {nu}: {sampled} against {expected}"
        );
    }
}

/// Analytic ground over a sphere: the plane n · p = D in space, raised by `delta` along each
/// point's normal, surveyed where `surveyed` says, with one unresolved band of RMS `sigma_k` and
/// wavelength `band_m`.
struct PlaneGround {
    radius_m: f64,
    normal: [f64; 3],
    offset_m: f64,
    delta_m: f64,
    sigma_k: f64,
    band_m: f64,
    surveyed: Box<dyn Fn([f64; 3]) -> Survey>,
    ceiling_m: f64,
}

/// The coarse level the analytic ground keeps its survey by.
const PLANE_LEVEL: u8 = 6;

impl PlaneGround {
    /// The plane at distance `offset_m` from the centre of a sphere of radius 10⁶ m along
    /// `normal`, surveyed everywhere at `code`.
    fn new(normal: [f64; 3], offset_m: f64, code: ResolutionCode) -> Self {
        Self {
            radius_m: 1e6,
            normal: unit_dir(normal),
            offset_m,
            delta_m: 0.0,
            sigma_k: 5.0,
            band_m: 20.0,
            surveyed: Box::new(move |dir| Survey::Surveyed {
                cell: PatchKey::containing(PLANE_LEVEL, dir).unwrap(),
                code,
            }),
            ceiling_m: 2e5,
        }
    }

    /// The exact range to the raised plane from `o` along the unit `d`, for `delta_m` = 0.
    fn exact_range(&self, o: [f64; 3], d: [f64; 3]) -> f64 {
        (self.offset_m - dot(self.normal, o)) / dot(self.normal, d)
    }
}

impl SurveyedGround for PlaneGround {
    type Cache = ();
    type Error = std::convert::Infallible;

    fn figure(&self) -> Spheroid {
        Spheroid::sphere(self.radius_m)
    }

    fn field_level(&self) -> u8 {
        PLANE_LEVEL
    }

    fn survey(&self, dir: [f64; 3]) -> Survey {
        (self.surveyed)(dir)
    }

    fn height(
        &self,
        _cache: &mut (),
        dir: [f64; 3],
        _level: BandLevel,
    ) -> Result<HeightSample, Self::Error> {
        let along = dot(self.normal, dir);
        assert!(along > 0.0, "the plane faces the direction");
        // h(d) = D ÷ (n · d) − R, and its gradient at the sphere point R d, through d = P ÷ |P|.
        let scale = -self.offset_m / (self.radius_m * along * along);
        Ok(HeightSample {
            height_m: self.offset_m / along - self.radius_m + self.delta_m,
            gradient: [0, 1, 2].map(|k| scale * (self.normal[k] - along * dir[k])),
        })
    }

    fn held_height(&self, cache: &mut (), dir: [f64; 3], level: BandLevel) -> Option<HeightSample> {
        self.height(cache, dir, level).ok()
    }

    fn ground(&self, cache: &mut (), p: &[f64; 3]) -> Result<Metres, Self::Error> {
        let (dir, _) = datum_foot(&self.figure(), p);
        let band = BandLevel::new(MAX_LEVEL).unwrap();
        Ok(Metres::new(self.height(cache, dir, band)?.height_m))
    }

    fn omitted_rms(&self, _cell: PatchKey, _level: BandLevel) -> Result<Metres, Self::Error> {
        Ok(Metres::new(self.sigma_k))
    }

    fn omitted_structure(
        &self,
        _cell: PatchKey,
        _level: BandLevel,
        lag: Metres,
    ) -> Result<SquareMetres, Self::Error> {
        let k = 2.0 * core::f64::consts::PI / self.band_m;
        let s2 = self.sigma_k * self.sigma_k;
        Ok(SquareMetres::new(
            2.0 * s2 * (1.0 - math::j0(k * lag.value())),
        ))
    }

    fn ceiling_m(&self) -> f64 {
        self.ceiling_m
    }
}

/// The unit direction at angle `alpha` from the pole toward +x and `beta` about the pole.
fn polar(alpha: f64, beta: f64) -> [f64; 3] {
    let (sa, ca) = math::sin_cos(alpha);
    let (sb, cb) = math::sin_cos(beta);
    [sa * cb, sa * sb, ca]
}

/// The ground the march reached.
fn ground_of(sight: LineOfSight) -> SlantRange {
    match sight {
        LineOfSight::Ground(range) => range,
        other => panic!("the line of sight meets no ground: {other:?}"),
    }
}

/// A line of sight onto the plane: from `height_m` above it over the direction at `alpha` from the
/// pole, looking down at `depression` below the plane's own horizontal toward `azimuth`.
fn sight_onto(
    plane: &PlaneGround,
    alpha: f64,
    height_m: f64,
    depression: f64,
    azimuth: f64,
) -> ([f64; 3], [f64; 3]) {
    let foot = polar(alpha, 0.3);
    let h = plane.offset_m / dot(plane.normal, foot) - plane.radius_m;
    let o = [0, 1, 2].map(|k| (plane.radius_m + h + height_m) * foot[k]);
    // A tangent of the plane and the plane's normal.
    let (east, north) = east_north(plane.normal);
    let (sz, cz) = math::sin_cos(azimuth);
    let (sd, cd) = math::sin_cos(depression);
    let d = [0, 1, 2].map(|k| cd * (cz * east[k] + sz * north[k]) - sd * plane.normal[k]);
    (o, d)
}

#[test]
fn the_slant_range_to_a_plane_is_exact() {
    let tilt = polar(0.15, 1.1);
    let plane = PlaneGround::new(tilt, 1.001e6, code(100.0));
    for (alpha, depression, azimuth) in [
        (0.15, 1.2, 0.0),
        (0.1, 0.5, 2.0),
        (0.2, 0.2, 4.0),
        (0.3, 0.9, 5.5),
        (0.05, 0.05, 1.0),
    ] {
        let (o, d) = sight_onto(&plane, alpha, 3_000.0, depression, azimuth);
        let sight = march(&plane, &mut (), &o, &d, Metres::new(1e6)).unwrap();
        let range = ground_of(sight);
        let exact = plane.exact_range(o, d);
        assert!(
            (range.range().value() - exact).abs() < 2e-6,
            "at {alpha}, {depression}: {} against {exact}",
            range.range().value()
        );
        assert!(!range.is_resolved());
        let hit = range.hit_m();
        assert!((dot(plane.normal, hit) - plane.offset_m).abs() < 1e-5);
        // The hit's foot is where the reading would read the ground under it.
        let foot = unit_dir(face_uv_to_xyz(range.at()));
        let along = unit_dir(hit);
        assert!(dot(sub3(foot, along), sub3(foot, along)) < 1e-24);
    }
}

#[test]
fn the_slant_range_s_sigma_follows_the_hit_s_linearisation() {
    let tilt = polar(0.15, 1.1);
    for (alpha, depression) in [(0.15, 1.2), (0.1, 0.5), (0.25, 0.3), (0.3, 0.9)] {
        let mut plane = PlaneGround::new(tilt, 1.001e6, code(100.0));
        let (o, d) = sight_onto(&plane, alpha, 3_000.0, depression, 0.7);
        let range = ground_of(march(&plane, &mut (), &o, &d, Metres::new(1e6)).unwrap());
        let hit = range.hit_m();
        // cos θ_s between the plane's normal and the sphere's, sin γ_s the ray's to the plane.
        let cos_slope = dot(plane.normal, unit_dir(hit));
        let sin_grazing = -dot(plane.normal, unit_dir(d));
        let expected = plane.sigma_k * cos_slope / sin_grazing;
        let sigma = range.sigma().unwrap().value();
        assert!((sigma / expected - 1.0).abs() < 1e-9, "{sigma} {expected}");
        // And it is the range's own change with the ground's height, dt ÷ dδ.
        let delta = 0.25;
        plane.delta_m = delta;
        let raised = ground_of(march(&plane, &mut (), &o, &d, Metres::new(1e6)).unwrap());
        let rate = (range.range().value() - raised.range().value()) / delta;
        assert!(
            (rate * plane.sigma_k / sigma - 1.0).abs() < 1e-4,
            "{rate} against {}",
            sigma / plane.sigma_k
        );
    }
}

#[test]
fn a_line_of_sight_under_the_grazing_threshold_has_no_sigma() {
    let tilt = polar(0.1, 0.0);
    let survey = code(100.0);
    let plane = PlaneGround::new(tilt, 1.0005e6, survey);
    let level = survey_level(&plane.figure(), survey);
    let lambda = level.wavelength;
    let cell = PatchKey::containing(PLANE_LEVEL, tilt).unwrap();
    let v = plane
        .omitted_structure(cell, level.band, lambda)
        .unwrap()
        .value()
        / (lambda.value() * lambda.value());
    let threshold = GRAZING_FACTOR * v.sqrt();
    for (factor, grazing) in [(0.8, true), (1.25, false)] {
        let depression = math::asin(factor * threshold);
        let (o, d) = sight_onto(&plane, 0.1, 50.0, depression, 0.4);
        let range = ground_of(march(&plane, &mut (), &o, &d, Metres::new(1e6)).unwrap());
        assert_eq!(range.is_grazing(), grazing, "at {factor} of the threshold");
        assert_eq!(range.sigma().is_none(), grazing);
    }
    // Never on a resolved range.
    let resolved = PlaneGround::new(tilt, 1.0005e6, code(1.0));
    let (o, d) = sight_onto(&resolved, 0.1, 50.0, 0.002, 0.4);
    let range = ground_of(march(&resolved, &mut (), &o, &d, Metres::new(1e6)).unwrap());
    assert!(range.is_resolved() && !range.is_grazing());
    assert_eq!(range.sigma(), Some(Metres::ZERO));
}

#[test]
fn a_line_of_sight_that_leaves_the_survey_before_it_meets_the_ground_has_no_range() {
    let tilt = [0.0, 0.0, 1.0];
    let survey = code(100.0);
    // Surveyed toward −x of the pole, a margin beyond, and nothing held past that.
    let region = move |dir: [f64; 3]| {
        let cell = PatchKey::containing(PLANE_LEVEL, dir).unwrap();
        if dir[0] < 0.01 {
            Survey::Surveyed { cell, code: survey }
        } else if dir[0] < 0.02 {
            Survey::Margin
        } else {
            Survey::NotHeld
        }
    };
    let mut plane = PlaneGround::new(tilt, 1.0002e6, survey);
    plane.surveyed = Box::new(region);
    plane.ceiling_m = 2_000.0;
    let toward_x = |alpha: f64, height: f64, depression: f64| {
        let foot = polar(alpha, 0.0);
        let h = plane.offset_m / dot(plane.normal, foot) - plane.radius_m;
        let o = [0, 1, 2].map(|k| (plane.radius_m + h + height) * foot[k]);
        let (sd, cd) = math::sin_cos(depression);
        (o, [cd, 0.0, -sd])
    };
    // Low over the survey, heading out of it, and down too gently to meet the ground before it
    // leaves: it clears the margin's ground, then reaches ground no one holds.
    let (o, d) = toward_x(0.0, 300.0, 0.004);
    assert_eq!(
        march(&plane, &mut (), &o, &d, Metres::new(1e6)).unwrap(),
        LineOfSight::NotSurveyed
    );
    // Down onto the margin's ground, which is held but not surveyed.
    let (o, d) = toward_x(0.009, 100.0, 0.03);
    assert_eq!(
        march(&plane, &mut (), &o, &d, Metres::new(1e6)).unwrap(),
        LineOfSight::NotSurveyed
    );
    // The same line of sight from nearer the margin's start meets surveyed ground first.
    let (o, d) = toward_x(0.004, 100.0, 0.03);
    let range = ground_of(march(&plane, &mut (), &o, &d, Metres::new(1e6)).unwrap());
    assert!((range.range().value() - plane.exact_range(o, d)).abs() < 2e-6);
    // From far above ground no one holds, a line of sight down onto the survey meets it.
    let (o, _) = toward_x(0.05, 400_000.0, 0.0);
    let target = [0, 1, 2].map(|k| (plane.offset_m) * polar(-0.002, 0.0)[k]);
    let d = unit_dir(sub3(target, o));
    let range = ground_of(march(&plane, &mut (), &o, &d, Metres::new(1e7)).unwrap());
    assert!((range.range().value() - plane.exact_range(o, d)).abs() < 2e-6);
    // Looking up, away from every ground, or short of it, meets none.
    let (o, _) = toward_x(0.0, 300.0, 0.0);
    let up = unit_dir([0.1, 0.0, 1.0]);
    assert_eq!(
        march(&plane, &mut (), &o, &up, Metres::new(1e7)).unwrap(),
        LineOfSight::NoGround
    );
    let (o, d) = toward_x(-0.005, 300.0, 0.3);
    assert_eq!(
        march(&plane, &mut (), &o, &d, Metres::new(100.0)).unwrap(),
        LineOfSight::NoGround
    );
    // From under the ground the range is zero.
    let (o, d) = toward_x(-0.005, -10.0, 0.3);
    let range = ground_of(march(&plane, &mut (), &o, &d, Metres::new(1e3)).unwrap());
    assert_eq!(bits(range.range().value()), bits(0.0));
}

#[test]
fn per_band_the_slope_uncertainty_tends_to_its_limits() {
    let tilt = polar(0.2, 0.5);
    let survey = code(100.0);
    let mut plane = PlaneGround::new(tilt, 1.001e6, survey);
    let figure = plane.figure();
    let level = survey_level(&figure, survey);
    // 0.3 rad from the plane's foot, so that the resolved slope is about 0.31.
    let dir = polar(0.5, 0.5);
    let cell = PatchKey::containing(PLANE_LEVEL, dir).unwrap();
    let height = plane.height(&mut (), dir, level.band).unwrap().height_m;
    let k = 2.0 * core::f64::consts::PI / plane.band_m;
    for sigma_k in [0.02, 5.0] {
        plane.sigma_k = sigma_k;
        let terms = |b: f64| {
            slope_terms(&plane, &mut (), dir, height, cell, level, Metres::new(b)).unwrap()
        };
        // Much longer than the band's wavelength: √v → √2 σ_k ÷ b, J₀ falling as (k b)^−½,
        // under 0.32% here.
        let long = 1e4 * plane.band_m;
        let ratio = terms(long).axis_variance.sqrt() * long / (2.0_f64.sqrt() * sigma_k);
        assert!((ratio - 1.0).abs() < 2e-3, "{ratio}");
        // Much shorter: √v → the band's gradient σ per axis, σ_k k ÷ √2.
        let short = 1e-3 * plane.band_m;
        let ratio = terms(short).axis_variance.sqrt() * 2.0_f64.sqrt() / (sigma_k * k);
        assert!((ratio - 1.0).abs() < 1e-5, "{ratio}");
        // And the reading's own spread is √v over a resolved slope far steeper than √v, and
        // Rayleigh's over none.
        let terms = terms(SLOPE_BASELINES[0].value());
        let (g0, v) = (terms.resolved_magnitude(), terms.axis_variance);
        assert!((g0 - math::tan(0.3)).abs() < 1e-3, "{g0}");
        if g0 > 10.0 * v.sqrt() {
            assert!((rice_sigma(g0, v) / v.sqrt() - 1.0).abs() < 0.01);
        }
        let rayleigh = (v * (2.0 - FRAC_PI_2)).sqrt();
        assert!((rice_sigma(0.0, v) / rayleigh - 1.0).abs() < 1e-14);
    }
}

#[test]
fn a_line_of_sight_meets_a_world_where_its_ground_is() {
    let mut rng = Lcg::new(0x7265_6164_0009);
    for &w in SyntheticWorld::ALL {
        let field = world(w);
        let figure = field.header().figure();
        let survey = code(30.0);
        let view = Surveyed::everywhere(field, survey);
        let synth = Synthesiser::new(&view, SEED);
        let mut cache = SynthCache::new();
        let level = survey_level(&figure, survey);
        for dir in sample_points(w, 2, 1, &mut rng) {
            let ground = synth
                .height_at(&mut cache, dir, level.band)
                .unwrap()
                .height_m;
            let origin = figure.surface_point(dir, ground + 1_500.0);
            let up = figure.normal(dir);
            let (east, _) = east_north(up);
            // 30° below the horizontal.
            let sight = unit_dir([0, 1, 2].map(|k| 0.75_f64.sqrt() * east[k] - 0.5 * up[k]));
            let line = slant_range(&synth, &mut cache, &origin, &sight, Metres::new(1e5)).unwrap();
            let range = ground_of(line);
            let reach = range.range().value();
            let (foot, height) = datum_foot(&figure, &range.hit_m());
            let under = synth
                .height_at(&mut cache, foot, level.band)
                .unwrap()
                .height_m;
            assert!(
                height <= under && under - height < 1e-5,
                "{w:?} {dir:?}: {height} {under}"
            );
            // No point of the line before it is under the ground.
            let step = 0.25 * level.wavelength.value();
            let mut along = 0.0;
            while along < reach - step {
                let point = [0, 1, 2].map(|k| origin[k] + along * sight[k]);
                let (foot, height) = datum_foot(&figure, &point);
                let floor = synth
                    .height_at(&mut cache, foot, level.band)
                    .unwrap()
                    .height_m;
                assert!(
                    height > floor,
                    "{w:?} {dir:?}: under the ground at {along} m of {reach} m"
                );
                along += step;
            }
            let sigma = range.sigma().map_or(0.0, Metres::value);
            assert!(sigma >= 0.0 && sigma.is_finite());
        }
    }
}

#[test]
fn an_orbital_survey_s_quoted_sigma_holds_its_ground() {
    // A small sample of the slow pooled case (crates/hyperion-sim/tests/surface_reading_worlds.rs),
    // on a world whose craters R09's closed forms hold (the plan's "Deviations in T5, as built").
    let mut rng = Lcg::new(0x7265_6164_000a);
    let w = SyntheticWorld::MarsLike;
    let field = world(w);
    let figure = field.header().figure();
    let view = Surveyed::everywhere(field, code(100.0));
    let synth = Synthesiser::new(&view, SEED);
    let mut cache = SynthCache::new();
    let (mut residual2, mut predicted2, mut z2, mut count) = (0.0, 0.0, 0.0, 0.0);
    for dir in sample_points(w, 24, 24, &mut rng) {
        let reading = surveyed_reading(&synth, &mut cache, at(dir), SLOPE_BASELINES[0])
            .unwrap()
            .unwrap();
        let ground = ground_at(&synth, &mut cache, &figure.point(read_dir(dir))).unwrap();
        let r = ground.height().value() - reading.elevation().value();
        let s = reading.elevation_sigma().value();
        assert!(s > 0.0, "{dir:?}");
        residual2 += r * r;
        predicted2 += s * s;
        z2 += (r / s) * (r / s);
        count += 1.0;
    }
    let rms_ratio = (residual2 / predicted2).sqrt();
    let z_variance = z2 / count;
    eprintln!("RMS ÷ σ = {rms_ratio}, var z = {z_variance}");
    assert!((rms_ratio - 1.0).abs() < 0.25, "RMS ÷ σ = {rms_ratio}");
    assert!((z_variance - 1.0).abs() < 0.4, "var z = {z_variance}");
}

#[test]
fn a_line_of_sight_through_a_client_s_field_is_the_whole_field_s() {
    let mars = SyntheticWorld::MarsLike;
    let field = world(mars);
    let figure = field.header().figure();
    let level = field.header().level().get();
    let centre = PatchKey::new(Face::PosY, level, 40, 50).unwrap();
    let block: Vec<PatchKey> = king_distances(centre, 1).into_keys().collect();
    let survey = code(40.0);
    let client = client_field(mars, &block, survey);
    let whole = Surveyed::everywhere(field, survey);
    let partial = Synthesiser::new(&client, SEED);
    let full = Synthesiser::new(&whole, SEED);
    let band = survey_level(&figure, survey).band;
    let mut cache = SynthCache::new();
    let dir = point_in(centre, 0.5, 0.5);
    let ground = full.height_at(&mut cache, dir, band).unwrap().height_m;
    let up = figure.normal(dir);
    let (east, _) = east_north(up);
    let line = |height: f64, depression: f64| {
        let origin = figure.surface_point(dir, ground + height);
        let (sin, cos) = math::sin_cos(depression);
        (origin, [0, 1, 2].map(|k| cos * east[k] - sin * up[k]))
    };
    // Down onto the survey: the same range as over the whole field.
    let (origin, sight) = line(1_500.0, 0.5);
    let reach = Metres::new(1e5);
    let from_client = ground_of(slant_range(&partial, &mut cache, &origin, &sight, reach).unwrap());
    let from_whole = ground_of(slant_range(&full, &mut cache, &origin, &sight, reach).unwrap());
    assert!((from_client.range().value() - from_whole.range().value()).abs() <= 2e-6);
    assert_eq!(from_client.sigma(), from_whole.sigma());
    // Rising gently out of the survey, over its margin and then ground no one holds, below the
    // ceiling of what the body may have there: not surveyed.
    let (origin, sight) = line(3_000.0, -0.002);
    let far = Metres::new(1e6);
    let sighting = slant_range(&partial, &mut cache, &origin, &sight, far).unwrap();
    assert_eq!(sighting, LineOfSight::NotSurveyed);
    // A client holding no cell can say nothing.
    let empty = PartialField::new(field.header().clone());
    let none = Synthesiser::new(&empty, SEED);
    let (origin, sight) = line(1_500.0, 0.5);
    let sighting = slant_range(&none, &mut SynthCache::new(), &origin, &sight, reach).unwrap();
    assert_eq!(sighting, LineOfSight::NotSurveyed);
    // The level-keyed forms answer `None` for a cell the client does not hold.
    let unheld = PatchKey::new(Face::NegZ, level, 3, 3).unwrap();
    assert_eq!(client.resolution(unheld), None);
    assert_eq!(partial.omitted_variance(unheld, band), None);
    assert_eq!(partial.omitted_rms(unheld, band), None);
    assert_eq!(
        partial.omitted_structure_function(unheld, band, Metres::new(2.0)),
        None
    );
}

/// Asserts `got` is `want` to a part in 10¹².
fn assert_close(got: f64, want: f64, what: &str) {
    let scale = num::max(got.abs(), want.abs());
    assert!(
        (got - want).abs() <= 1e-12 * scale,
        "{what}: {got} against {want}"
    );
}

#[test]
fn the_level_keyed_forms_count_what_each_level_leaves_out() {
    use crate::synth::channels::incision::mean_cell_width;
    use crate::synth::relief::{ReliefStyle, finest_octave, first_octave};
    let mut rng = Lcg::new(0x7265_6164_000c);
    for &w in SyntheticWorld::ALL {
        let field = world(w);
        let header = field.header();
        let synth = Synthesiser::new(field, SEED);
        let limit = survey_level(&header.figure(), code(1.0)).band.get();
        let coarse = header.level();
        let radius_m = header.radius().value();
        let octaves = synth.craters().octaves().octaves();
        for cell in random_cells(coarse.get(), 4, &mut rng) {
            let style = ReliefStyle::of(field.cell(cell).unwrap());
            for n in [0, coarse.get(), 9, 12, limit - 1, limit, MAX_LEVEL] {
                let level = BandLevel::new(n).unwrap();
                let set = synth.level_set(level);
                let omitted = synth.omitted_variance(cell, level).unwrap();
                let at = format!("{w:?} {cell:?} at level {n}");
                // The structural octaves the level does not evaluate, each whole.
                let last = set
                    .relief_through()
                    .unwrap_or_else(|| first_octave(coarse) - 1);
                let mut relief = 0.0;
                for m in last + 1..=finest_octave(header.radius()) {
                    let lift = math::powf(synth.relief().belt_factor(m), style.belt());
                    let a = style.amplitude() * lift;
                    relief += a * a * header.spectrum().level_variance(m).value();
                }
                assert_close(omitted.relief().value(), relief, &at);
                // The channel levels after the last.
                let width = set.channels_through().map_or_else(
                    || 2.0 * mean_cell_width(radius_m, coarse.get()),
                    |k| mean_cell_width(radius_m, k),
                );
                let channels = synth.unresolved_variance(cell, Metres::new(width)).unwrap();
                assert_eq!(
                    bits(omitted.channels().value()),
                    bits(channels.channels().value()),
                    "{at}"
                );
                // The small-crater octaves the level does not keep, each whole.
                let left_out = set.craters_through().map_or(0, |k| usize::from(k) + 1);
                let craters: f64 = octaves[left_out..]
                    .iter()
                    .map(|o| o.variance().value())
                    .sum();
                assert_close(omitted.craters().value(), craters, &at);
                // The coarse rims at the level's own band.
                let rims = synth.unresolved_variance(cell, set.shortest_wavelength());
                assert_eq!(
                    bits(omitted.rims().value()),
                    bits(rims.unwrap().rims().value()),
                    "{at}"
                );
                if n >= limit {
                    assert_eq!(bits(omitted.total().value()), bits(0.0), "{at}");
                }
            }
            // A level that evaluates none of them leaves out the whole of each below the cell.
            let none = BandLevel::new(0).unwrap();
            let set = synth.level_set(none);
            assert!(set.relief_through().is_none() && set.channels_through().is_none());
            assert!(set.craters_through().is_none(), "{w:?}");
            let whole = synth.unresolved_variance(cell, Metres::new(1e12)).unwrap();
            let omitted = synth.omitted_variance(cell, none).unwrap();
            for (got, want) in [
                (omitted.relief(), whole.relief()),
                (omitted.channels(), whole.channels()),
                (omitted.craters(), whole.craters()),
            ] {
                assert_close(got.value(), want.value(), &format!("{w:?} {cell:?}"));
            }
        }
    }
}

#[test]
fn the_level_keyed_forms_are_the_synthesiser_s_and_shrink_as_the_level_deepens() {
    let mut rng = Lcg::new(0x7265_6164_000d);
    for &w in &BODIES {
        let field = world(w);
        let synth = Synthesiser::new(field, SEED);
        let level = field.header().level().get();
        for cell in random_cells(level, 3, &mut rng) {
            let mut previous = f64::INFINITY;
            for n in 0..=MAX_LEVEL {
                let band = BandLevel::new(n).unwrap();
                let variance = synth.omitted_variance(cell, band).unwrap().total().value();
                assert!(variance <= previous, "{w:?} {cell:?} at level {n}");
                previous = variance;
                let rms = synth.omitted_rms(cell, band).unwrap().value();
                assert_eq!(bits(rms), bits(variance.sqrt()));
                // The structure function saturates at twice the variance at long lags.
                let lag = Metres::new(1e7);
                let d = synth.omitted_structure_function(cell, band, lag).unwrap();
                assert!(
                    d.value() <= 2.2 * variance + 1e-300,
                    "{w:?} {cell:?} at level {n}"
                );
            }
        }
    }
}

/// The mean squares of what a level leaves out of the finest level's ground, contribution by
/// contribution, against R09's level-keyed closed forms and its wavelength-keyed ones at `λ_n`,
/// and the small craters' part split into the octaves the level leaves out and the sharpening of
/// the ones it keeps: the figures of the plan's "Deviations in T5, as built", on 24 cells × 12
/// points a world and survey (`--include-ignored --nocapture`).
#[test]
#[ignore = "measurement, by hand: cargo test -p hyperion-surface --lib reading::tests::the_ground_a_level_leaves_out_by_contribution -- --ignored --nocapture"]
fn the_ground_a_level_leaves_out_by_contribution() {
    use crate::synth::craters::CraterCache;
    let parts = ["relief", "channels", "craters", "rims"];
    for &w in &BODIES {
        let mut rng = Lcg::new(0x7265_6164_000b);
        let field = world(w);
        let figure = field.header().figure();
        for metres in [10.0, 100.0, 1_000.0] {
            let survey = code(metres);
            let view = Surveyed::everywhere(field, survey);
            let synth = Synthesiser::new(&view, SEED);
            let mut cache = SynthCache::new();
            let mut craters = CraterCache::new();
            let level = survey_level(&figure, survey);
            let limit = survey_level(&figure, code(1.0)).band;
            let kept = synth.level_set(level.band).craters_through();
            let all = synth.level_set(limit).craters_through();
            let (mut realised, mut at_cutoff, mut at_lambda) = ([0.0; 4], [0.0; 4], [0.0; 4]);
            let (mut sharpening, mut left_out, mut count) = (0.0, 0.0, 0.0);
            for dir in sample_points(w, 24, 12, &mut rng) {
                let coarse = synth.contributions(&mut cache, dir, level.band).unwrap();
                let fine = synth.contributions(&mut cache, dir, limit).unwrap();
                let pairs = [
                    (fine.relief, coarse.relief),
                    (fine.channels, coarse.channels),
                    (fine.craters, coarse.craters),
                    (fine.rims, coarse.rims),
                ];
                for (sum, (f, c)) in realised.iter_mut().zip(pairs) {
                    *sum += (f.height_m - c.height_m) * (f.height_m - c.height_m);
                }
                let cell = cell_of(field, dir);
                let omitted = synth.omitted_variance(cell, level.band).unwrap();
                for (sum, part) in at_cutoff.iter_mut().zip([
                    omitted.relief(),
                    omitted.channels(),
                    omitted.craters(),
                    omitted.rims(),
                ]) {
                    *sum += part.value();
                }
                let closed = synth.unresolved_variance(cell, level.wavelength).unwrap();
                for (sum, part) in at_lambda.iter_mut().zip([
                    closed.relief(),
                    closed.channels(),
                    closed.craters(),
                    closed.rims(),
                ]) {
                    *sum += part.value();
                }
                if let (Some(kept), Some(all)) = (kept, all) {
                    let small = synth.craters();
                    let blunt = small.at(dir, level.band, kept, &mut craters).height_m;
                    let sharp = small.at(dir, limit, kept, &mut craters).height_m;
                    let every = small.at(dir, limit, all, &mut craters).height_m;
                    sharpening += (sharp - blunt) * (sharp - blunt);
                    left_out += (every - sharp) * (every - sharp);
                }
                count += 1.0;
            }
            eprintln!(
                "{w:?} at {metres} m: level {}, λ_n {:.2} m",
                level.band.get(),
                level.wavelength.value()
            );
            for (k, part) in parts.iter().enumerate() {
                eprintln!(
                    "  {part:9} realised {:12.4} m², level-keyed {:12.4}, at λ_n {:12.4}",
                    realised[k] / count,
                    at_cutoff[k] / count,
                    at_lambda[k] / count
                );
            }
            eprintln!(
                "  craters: octaves left out {:.4} m², the kept ones' sharpening {:.4} m²",
                left_out / count,
                sharpening / count
            );
        }
    }
}
