//! The naked eye's limit in every direction of the band: the limit map (rendering plan R06,
//! R06.T9.c; Design notes 4 and 5).
//!
//! Each texel's limit is Crumey's threshold ([`naked_eye_limit`]) against the background the eye
//! sees in the direction of the texel's centre, the direction its ray takes: the band's light there,
//! the stars the census did not list ([`BandTexel`]), plus the veiling glare of every listed star
//! within 100° of it, CIE 146:2002's general disability glare ([`veiling_luminance`]; Vos 2003,
//! Lighting Res. Technol. 35, 163), which Design note 4 takes for the "standard way" (Adrian 1989,
//! Lighting Res. Technol. 21, 181) to which Crumey (2014, MNRAS 442, 2600, §1.6.3) leaves glare.
//!
//! The glare is taken as the stars' own light scattered in the eye, in each star's colour (Design
//! note 4). Intraocular straylight is not spectrally neutral (near λ⁻⁴ in young, well-pigmented
//! eyes, with a red component added in light ones; Coppens, Franssen and van den Berg 2006, Exp.
//! Eye Res. 82, 688), so the veil's rod weight is uncertain by some tens of per cent: up to about
//! 0.1 mag where the veil dominates a texel. With E★ a listed star's photopic illuminance at the eye after its own reddening, ρ★ its reddened S/P
//! ratio (`star.colour().reddened(star.a_v()).sp_ratio()`, R06.T9.e) and K(θ) the veil per lux at
//! θ from the texel's centre (θ clamped at 0.1°, nothing beyond 100°), a texel of band luminance B
//! and ratio ρ is seen against
//!
//! - a photopic luminance B′ = B + Σ E★ K(θ★), and
//! - an S/P ratio ρ′ = (ρ B + Σ ρ★ E★ K(θ★)) ÷ B′, the band's and the veils' light together.
//!
//! In a scotopic background the threshold then reads (ρ′ ÷ 1.408) B′ = (ρ B + Σ ρ★ E★ K) ÷ 1.408 of
//! Blackwell's light: each star's veil weighed by ρ★ ÷ 1.408 as the band's light is by ρ ÷ 1.408,
//! Design note 4's rod weighting. In a mesopic one each light is weighed by its MES2 mesopic
//! luminance at the photopic weight of the whole background, band and veils together, since the
//! eye adapts to all it sees ([`blackwell_equivalent_factor`]'s construction). F stays the
//! observer's: the glare is modelled, not folded into F (R06's Risks, "Glare double count").
//!
//! The band's own luminance, chroma and ρ are left as they are: the glare is the eye's, not the
//! sky's light, and the band layer draws only the sky's. The glare of the observer's own stars and
//! of sunlit bodies is not in the map (Design note 4).
//!
//! [`blackwell_equivalent_factor`]: super::eye::blackwell_equivalent_factor

use std::ops::Range;

use crate::coords::UnitVector;
use crate::math;
use crate::observe::Observer;
use crate::units::consts::RADIANS_PER_DEGREE;
use crate::units::{CandelasPerSquareMetre, Degrees, Lux, Magnitudes};

use super::band::{BandSpec, BandTexel, CubeFace, unextinguished_lux};
use super::census::{SkyStar, sky_order};
use super::eye::{
    EyeObserver, GLARE_MAX_ANGLE_DEG, SkyBackground, SpRatio, naked_eye_limit, veiling_luminance,
};

/// How far past the glare's reach of 100° a source's angle is still taken from its cosine, degrees:
/// a source whose cosine lies within this of the reach's is given its angle, and
/// [`veiling_luminance`] alone decides whether it veils, so the cheap test on the cosine never
/// parts from the angle's.
const REACH_MARGIN_DEG: f64 = 0.01;

/// One listed star as the glare reads it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct GlareSource {
    /// Its direction from the observer, on the galactic axes.
    direction: UnitVector,
    /// Its photopic illuminance at the eye after its own reddening, lux.
    photopic: f64,
    /// Its scotopic light at the eye, its photopic illuminance times its reddened S/P ratio, lux.
    scotopic: f64,
}

/// The glare of a census's listed stars as the limit map reads it (Design note 4): each star's
/// direction from the observer, and its photopic and scotopic illuminance at the eye after its own
/// reddening.
///
/// A request builds it once from its census, and every job of the band's rows reads it: each star's
/// reddening is resolved here, once. The default holds no star, as a band's limits against its own
/// light alone (the eye cut's pre-pass, R06.T9.d) are.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Glare {
    sources: Vec<GlareSource>,
}

impl Glare {
    /// The glare of the stars `listed` (a [`SkyCensus`](super::census::SkyCensus)'s listed stars,
    /// not its overflow, whose light the band holds) as `observer` sees them.
    ///
    /// Each star's photopic illuminance is its unextinguished light through its own
    /// [`StarColour::reddened`](super::colour::StarColour::reddened) at its extinction, as the
    /// band's overflow points take it, and its scotopic light that times the reddened S/P ratio. A
    /// star at the observer's own position, which has no direction, adds no glare.
    ///
    /// The veil is summed in the stars' order, so `listed` is the census's own, by
    /// [`sky_order`](super::census::sky_order), as [`SkyCensus::listed`](super::census::SkyCensus::listed)
    /// gives it.
    ///
    /// # Panics
    ///
    /// In debug builds, if `listed` is not in that order.
    #[must_use]
    pub fn of_listed(observer: &Observer, listed: &[SkyStar]) -> Self {
        debug_assert!(
            listed
                .windows(2)
                .all(|pair| sky_order(&pair[0], &pair[1]).is_lt()),
            "the listed stars in the census's order"
        );
        let origin = observer.position();
        let sources = listed
            .iter()
            .filter_map(|star| {
                let direction =
                    UnitVector::from_components(origin.displacement_to(star.apparent()).metres())?;
                let reddened = star.colour().reddened(star.a_v());
                let photopic = unextinguished_lux(star.colour(), star.v(), star.a_v())
                    * reddened.photopic_transmission();
                Some(GlareSource {
                    direction,
                    photopic,
                    scotopic: photopic * reddened.sp_ratio(),
                })
            })
            .collect();
        Self { sources }
    }

    /// The number of stars that glare.
    #[must_use]
    pub fn len(&self) -> usize {
        self.sources.len()
    }

    /// Whether no star glares.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }

    /// The photopic and scotopic veiling luminance towards `toward`, cd m⁻²: each source's
    /// illuminance times [`veiling_luminance`]'s veil per lux at its angle, summed in the sources'
    /// order. A source whose cosine is below `reach_cos` lies beyond the reach and is not read.
    #[must_use]
    fn veil(&self, eye: &EyeObserver, toward: &UnitVector, reach_cos: f64) -> (f64, f64) {
        let (mut photopic, mut scotopic) = (0.0, 0.0);
        for source in &self.sources {
            let cos = toward.dot(&source.direction);
            if cos < reach_cos {
                continue;
            }
            let angle = math::acos(cos.clamp(-1.0, 1.0)) / RADIANS_PER_DEGREE;
            let per_lux = veiling_luminance(eye, Lux::new(1.0), Degrees::new(angle))
                .expect("a unit illuminance and an angle of 0–180° are valid")
                .value();
            photopic += source.photopic * per_lux;
            scotopic += source.scotopic * per_lux;
        }
        (photopic, scotopic)
    }
}

#[cfg(test)]
impl Glare {
    /// This glare and one more source: a star in `direction` of photopic illuminance `photopic`,
    /// lux, and S/P ratio `sp_ratio`.
    #[must_use]
    fn with_source(mut self, direction: UnitVector, photopic: f64, sp_ratio: f64) -> Self {
        self.sources.push(GlareSource {
            direction,
            photopic,
            scotopic: photopic * sp_ratio,
        });
        self
    }
}

/// The eye's limit against `texel`'s light and the veil `(photopic, scotopic)` over it, cd m⁻²
/// (the [module](self) documentation). With no veil the background is the texel's own, luminance
/// and ρ bit for bit.
#[must_use]
fn texel_limit(eye: &EyeObserver, texel: &BandTexel, veil: (f64, f64)) -> Magnitudes {
    let (photopic_veil, scotopic_veil) = veil;
    let light = texel.luminance().value();
    debug_assert!(
        light.is_finite() && texel.sp_ratio().is_finite(),
        "a band texel's light {light} and ratio {}",
        texel.sp_ratio()
    );
    debug_assert!(
        photopic_veil.is_finite() && scotopic_veil.is_finite(),
        "a veil of {veil:?}"
    );
    let (luminance, sp_ratio) = if photopic_veil > 0.0 {
        let luminance = light + photopic_veil;
        (
            luminance,
            (light * texel.sp_ratio() + scotopic_veil) / luminance,
        )
    } else {
        (light, texel.sp_ratio())
    };
    let sp_ratio = SpRatio::new(sp_ratio.clamp(SpRatio::MIN.value(), SpRatio::MAX.value()))
        .expect("a finite ratio clamped into the accepted range");
    let background = SkyBackground::new(
        CandelasPerSquareMetre::new(luminance.min(SkyBackground::MAX_LUMINANCE.value())),
        sp_ratio,
    )
    .expect("a non-negative luminance held at the brightest accepted");
    naked_eye_limit(eye, &background)
}

/// Sets the eye's limit of each texel of rows `rows` (from the top) of `face`, `texels` in
/// [`band_rows`](super::band::band_rows)' order (row by row, each from its left): Crumey's
/// threshold for `eye` against the texel's light and the veiling glare `glare` over it (Design note
/// 4; the [module](self) documentation).
///
/// A server runs it on each job's rows of the band; each texel's limit is a function of its own
/// light, its direction and the glare alone, so any split of a face's rows gives the same bits.
/// The texels' light, chroma and ρ are not changed.
///
/// An S/P ratio of the band and the veils together outside 0.01–100, which no starlight reaches, is
/// read at the nearer bound, and a background brighter than 10¹² cd m⁻² at that.
///
/// # Panics
///
/// If `rows` reaches past the face's last row, or `texels` is not those rows' texels in number;
/// and in debug builds, if a texel's light or the veil over it is not finite, which no band or
/// census gives.
pub fn limit_rows(
    eye: &EyeObserver,
    spec: &BandSpec,
    glare: &Glare,
    face: CubeFace,
    rows: Range<u16>,
    texels: &mut [BandTexel],
) {
    let side = spec.face_texels();
    assert!(
        rows.end <= side,
        "rows {rows:?} reach past a face of {side} rows"
    );
    assert_eq!(
        texels.len(),
        rows.len() * usize::from(side),
        "texels for rows {rows:?} of a face of {side}² texels"
    );
    let reach_cos = math::cos((GLARE_MAX_ANGLE_DEG + REACH_MARGIN_DEG) * RADIANS_PER_DEGREE);
    let places = rows.flat_map(|row| (0..side).map(move |column| (row, column)));
    for (texel, (row, column)) in texels.iter_mut().zip(places) {
        let toward = spec.texel_direction(face, row, column);
        let limit = texel_limit(eye, texel, glare.veil(eye, &toward, reach_cos));
        texel.set_eye_limit(limit);
    }
}

/// Sets the eye's limit of every texel of a band, `band` its six faces in [`CubeFace::ALL`]'s order,
/// each face's texels in [`band_rows`](super::band::band_rows)' order: [`limit_rows`] over each
/// face.
///
/// # Panics
///
/// If `band` is not six faces of `spec`'s texels in number, or as [`limit_rows`] panics.
///
/// # Examples
///
/// The eye's limits near the Sun against the band of the stars fainter than V 8.15, about the
/// eye's cut there, with no listed star's glare (`no_run`: the tables take a minute or more to
/// build):
///
/// ```no_run
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
/// use hyperion_sim::galaxy::gas::noise::NoiseCache;
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::observe::Observer;
/// use hyperion_sim::sky::band::{BandSpec, CompleteTo, CubeFace, band_rows};
/// use hyperion_sim::sky::census::{CellOffsets, NoSkyCellCache, SkyCensus, SkyContext, SkyQuery};
/// use hyperion_sim::sky::envelope::BrightnessEnvelope;
/// use hyperion_sim::sky::eye::EyeObserver;
/// use hyperion_sim::sky::limits::{Glare, limit_map};
/// use hyperion_sim::sky::luminosity::LuminosityTables;
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::Magnitudes;
///
/// let galaxy = Galaxy::from_params(Seed::new(7), GalaxyParams::milky_way_like())?;
/// let (tables, envelope) = (LuminosityTables::build(&galaxy), BrightnessEnvelope::build(&galaxy));
/// let offsets = CellOffsets::build(&galaxy);
/// let mut ctx = SkyContext {
///     tables: &tables,
///     envelope: &envelope,
///     offsets: &offsets,
///     noise: NoiseCache::with_capacity(1 << 16),
///     cells: &NoSkyCellCache,
///     sources: &[],
///     modifiers: &NoModifiers,
/// };
/// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 68.0]).ok_or("in the cube")?;
/// let observer = Observer::new(sun, UniverseTime::EPOCH)?;
/// let query = SkyQuery::builder(observer, Magnitudes::new(8.15)).build()?;
/// let spec = BandSpec::new(16, 12).ok_or("a band")?;
/// let mut band = Vec::new();
/// for face in CubeFace::ALL {
///     band_rows(&galaxy, &mut ctx, &query, &SkyCensus::empty(), &CompleteTo::everywhere(),
///         &spec, face, 0..16, &mut band);
/// }
/// limit_map(&EyeObserver::default(), &spec, &Glare::default(), &mut band);
/// let darkest = band.iter().filter_map(|t| t.eye_limit()).fold(f64::MIN, |a, m| a.max(m.value()));
/// assert!(darkest > 7.0, "the darkest sky near the Sun shows stars fainter than V 7");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn limit_map(eye: &EyeObserver, spec: &BandSpec, glare: &Glare, band: &mut [BandTexel]) {
    let side = spec.face_texels();
    let face_len = usize::from(side) * usize::from(side);
    assert_eq!(
        band.len(),
        CubeFace::ALL.len() * face_len,
        "a band of six faces of {side}² texels"
    );
    for (face, texels) in CubeFace::ALL
        .into_iter()
        .zip(band.chunks_exact_mut(face_len))
    {
        limit_rows(eye, spec, glare, face, 0..side, texels);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use core::num::NonZeroU32;

    use hyperion_testkit::float::bits;

    use super::*;
    use crate::coords::GalacticPosition;
    use crate::galaxy::features::centre::testing::milky_way_galaxy;
    use crate::galaxy::gas::modifiers::NoModifiers;
    use crate::galaxy::gas::noise::NoiseCache;
    use crate::sky::band::{CompleteTo, band_rows};
    use crate::sky::census::{
        CensusTallies, MAX_N_MAX, NoSkyCellCache, SkyCensus, SkyContext, SkyQuery, census_cell,
        census_plan, merge_census,
    };
    use crate::sky::colour::{AtmosphereGrid, solar_colour, star_colour};
    use crate::sky::eye::{illuminance_of_magnitude, luminance, mesopic_weight};
    use crate::sky::testing::{milky_way_envelope, milky_way_offsets, milky_way_tables};
    use crate::time::UniverseTime;
    use crate::units::{Kelvin, LightYears, MagnitudesPerArcsec2};

    /// The Sun's place in the fixture, ly, as the sim's other sky tests stand.
    const SUN: [f64; 3] = [0.0, 26_000.0, 68.0];

    /// The eye's cut near the Sun, V: T9.d's reference for the real sky (decided 2026-10-06,
    /// `decision-r06-t9b-band.md`), at which T9.c's medians are ruled.
    const EYE_CUT: f64 = 8.15;

    /// How far out the glare's census looks near the Sun, ly: its brightest stars are the nearest
    /// ones, and a census this deep and wide costs seconds, not minutes.
    const GLARE_RADIUS_LY: f64 = 100.0;

    /// The ruled medians near the Sun, V, and their tolerances (`decision-r06-t9b-band.md`, items 1
    /// and 2): Crumey's limit at Gaia DR3's light fainter than the cut, μ 22.18 in the band and
    /// 24.62 at the poles, and T9.b's 0.5 mag of μ through Crumey's slope there.
    const BAND_MEDIAN: (f64, f64) = (6.5, 0.20);
    const POLES_MEDIAN: (f64, f64) = (7.55, 0.22);

    fn context() -> SkyContext<'static> {
        SkyContext {
            tables: milky_way_tables(),
            envelope: milky_way_envelope(),
            offsets: milky_way_offsets(),
            noise: NoiseCache::with_capacity(1 << 16),
            cells: &NoSkyCellCache,
            sources: &[],
            modifiers: &NoModifiers,
        }
    }

    fn observer() -> Observer {
        let at = GalacticPosition::from_light_years(SUN).expect("in the cube");
        Observer::new(at, UniverseTime::EPOCH).expect("an observer")
    }

    fn query(cut: f64) -> SkyQuery {
        SkyQuery::builder(observer(), Magnitudes::new(cut))
            .build()
            .expect("a valid query")
    }

    fn spec(face_texels: u16) -> BandSpec {
        BandSpec::new(face_texels, BandSpec::STANDARD.nodes_per_decade()).expect("a spec")
    }

    /// Rows `rows` of `face` of the band near the Sun at the eye's cut with no census, complete
    /// everywhere: the light of the stars fainter than the cut, as the final reply's band holds it
    /// to the caps.
    fn band_near_the_sun(spec: BandSpec, face: CubeFace, rows: Range<u16>) -> Vec<BandTexel> {
        let mut out = Vec::new();
        band_rows(
            milky_way_galaxy(),
            &mut context(),
            &query(EYE_CUT),
            &SkyCensus::empty(),
            &CompleteTo::everywhere(),
            &spec,
            face,
            rows,
            &mut out,
        );
        out
    }

    /// The whole band near the Sun at 16² faces ([`band_near_the_sun`]), as T9.b's tests and the
    /// ruling's probe take it: built once for the tests that read it.
    fn near_the_sun() -> &'static [BandTexel] {
        static BAND: OnceLock<Vec<BandTexel>> = OnceLock::new();
        BAND.get_or_init(|| {
            let spec = spec(16);
            CubeFace::ALL
                .into_iter()
                .flat_map(|face| band_near_the_sun(spec, face, 0..16))
                .collect()
        })
    }

    /// The stars a census near the Sun lists brighter than the eye's cut within
    /// [`GLARE_RADIUS_LY`], every cap forced to it, with no eye: built once.
    fn nearby_stars() -> &'static [SkyStar] {
        static STARS: OnceLock<Vec<SkyStar>> = OnceLock::new();
        STARS.get_or_init(|| {
            let galaxy = milky_way_galaxy();
            let query = query(EYE_CUT)
                .with_caps_forced(LightYears::new(GLARE_RADIUS_LY))
                .expect("a forced cap");
            let mut ctx = context();
            let plan = census_plan(galaxy, ctx.tables, ctx.envelope, &query, &mut ctx.noise);
            let mut stars = Vec::new();
            for key in plan.cells() {
                census_cell(galaxy, &mut ctx, key, &query, &mut stars);
            }
            let all = NonZeroU32::new(MAX_N_MAX).expect("not zero");
            merge_census([(stars, CensusTallies::default())], all)
                .listed()
                .to_vec()
        })
    }

    /// The direction `degrees` from `direction`, turned towards the galactic plane's +X or, for a
    /// direction along X, towards +Z.
    fn off(direction: UnitVector, degrees: f64) -> UnitVector {
        let d = direction.components();
        let other = if d[0].abs() > 0.9 {
            UnitVector::NORTH
        } else {
            UnitVector::X
        };
        let across =
            UnitVector::from_components(direction.cross(&other)).expect("not along the other");
        let (sin, cos) = math::sin_cos(degrees * RADIANS_PER_DEGREE);
        let a = across.components();
        UnitVector::from_components([
            cos * d[0] + sin * a[0],
            cos * d[1] + sin * a[1],
            cos * d[2] + sin * a[2],
        ])
        .expect("a direction")
    }

    /// The angle between `a` and `b`, degrees, from the sine and the cosine, independently of the
    /// map's arccosine.
    fn angle_deg(from: &UnitVector, to: &UnitVector) -> f64 {
        let sine = from.cross(to).iter().map(|c| c * c).sum::<f64>().sqrt();
        math::atan2(sine, from.dot(to)) / RADIANS_PER_DEGREE
    }

    /// A star of V `v` and colour (`teff` K, `log_g`) unreddened, as a source: its direction, its
    /// photopic illuminance, lux, and its S/P ratio.
    fn star_at(direction: UnitVector, v: f64, teff: f64, log_g: f64) -> (UnitVector, f64, f64) {
        let colour = star_colour(Kelvin::new(teff), log_g, AtmosphereGrid::MainSequence);
        (
            direction,
            illuminance_of_magnitude(Magnitudes::new(v)).value() * colour.lux_per_v0(),
            colour.sp_ratio(),
        )
    }

    /// The listed stars as sources, read through the public colour and eye functions: each one's
    /// direction from the observer, its photopic illuminance after its own reddening, lux, and its
    /// reddened S/P ratio.
    fn sources_of(stars: &[SkyStar]) -> Vec<(UnitVector, f64, f64)> {
        let origin = *observer().position();
        stars
            .iter()
            .map(|star| {
                let reddened = star.colour().reddened(star.a_v());
                let unextinguished = illuminance_of_magnitude(star.v() - star.a_v()).value()
                    * star.colour().lux_per_v0();
                (
                    UnitVector::from_components(origin.displacement_to(star.apparent()).metres())
                        .expect("a star away from the observer"),
                    unextinguished * reddened.photopic_transmission(),
                    reddened.sp_ratio(),
                )
            })
            .collect()
    }

    fn glare_of(sources: &[(UnitVector, f64, f64)]) -> Glare {
        sources
            .iter()
            .fold(Glare::default(), |glare, &(u, e, rho)| {
                glare.with_source(u, e, rho)
            })
    }

    /// The plan's definition, written out again: `naked_eye_limit` at the texel's luminance plus
    /// each source's photopic veil within 100° (`veiling_luminance` of its illuminance at its
    /// angle), at the S/P ratio of the band's and the veils' light together.
    fn reference_background(
        eye: &EyeObserver,
        texel: &BandTexel,
        toward: &UnitVector,
        sources: &[(UnitVector, f64, f64)],
    ) -> SkyBackground {
        let light = texel.luminance().value();
        let (mut photopic, mut scotopic) = (light, light * texel.sp_ratio());
        for (u, e, rho) in sources {
            let angle = Degrees::new(angle_deg(toward, u));
            photopic += veiling_luminance(eye, Lux::new(*e), angle)
                .expect("a valid glare")
                .value();
            scotopic += veiling_luminance(eye, Lux::new(e * rho), angle)
                .expect("a valid glare")
                .value();
        }
        let rho = if photopic > light {
            scotopic / photopic
        } else {
            texel.sp_ratio()
        };
        SkyBackground::new(
            CandelasPerSquareMetre::new(photopic),
            SpRatio::new(rho).expect("a starlit ratio"),
        )
        .expect("a background")
    }

    /// Each texel's direction and galactic latitude |b|, degrees, in a band's order.
    fn texel_latitudes(spec: BandSpec) -> Vec<(UnitVector, f64)> {
        let n = spec.face_texels();
        let mut out = Vec::new();
        for face in CubeFace::ALL {
            for row in 0..n {
                for column in 0..n {
                    let u = spec.texel_direction(face, row, column);
                    out.push((u, math::asin(u.components()[2]).abs() / RADIANS_PER_DEGREE));
                }
            }
        }
        out
    }

    /// The median of the eye limits of the texels whose |b| lies in `latitudes`, and their number.
    fn median_limit(band: &[BandTexel], spec: BandSpec, latitudes: Range<f64>) -> (f64, usize) {
        let mut limits: Vec<f64> = band
            .iter()
            .zip(texel_latitudes(spec))
            .filter(|(_, (_, b))| latitudes.contains(b))
            .map(|(t, _)| t.eye_limit().expect("a limit").value())
            .collect();
        limits.sort_by(f64::total_cmp);
        let n = limits.len();
        assert!(n > 0, "texels at |b| in {latitudes:?}");
        let median = if n % 2 == 1 {
            limits[n / 2]
        } else {
            f64::midpoint(limits[n / 2 - 1], limits[n / 2])
        };
        (median, n)
    }

    /// The near-Sun band with its limits set against `glare`.
    fn limited(glare: &Glare, eye: &EyeObserver) -> Vec<BandTexel> {
        let mut band = near_the_sun().to_vec();
        limit_map(eye, &spec(16), glare, &mut band);
        band
    }

    #[test]
    fn each_texels_limit_is_crumeys_at_its_light_and_glare() {
        let spec = spec(16);
        let eye = EyeObserver::default();
        let mut sources = sources_of(nearby_stars());
        // A V −1.5 white star 0.05° from a texel's centre (read at 0.1°), and a V −9 one 0.3° from
        // another's, which makes that texel's background mesopic.
        sources.push(star_at(
            off(spec.texel_direction(CubeFace::PosZ, 8, 8), 0.05),
            -1.5,
            9_940.0,
            4.3,
        ));
        sources.push(star_at(
            off(spec.texel_direction(CubeFace::PosX, 7, 7), 0.3),
            -9.0,
            5_772.0,
            4.438,
        ));
        let band = limited(&glare_of(&sources), &eye);
        let (mut worst, mut mesopic) = (0.0_f64, 0);
        for (texel, (toward, _)) in band.iter().zip(texel_latitudes(spec)) {
            let background = reference_background(&eye, texel, &toward, &sources);
            if mesopic_weight(&background).value() > 0.0 {
                mesopic += 1;
            }
            let expected = naked_eye_limit(&eye, &background).value();
            let got = texel.eye_limit().expect("a limit").value();
            worst = worst.max((got - expected).abs());
        }
        eprintln!(
            "{} sources ({} listed within {GLARE_RADIUS_LY} ly): the worst texel differs from \
             the definition by {worst:.3e} mag; {mesopic} mesopic texels",
            sources.len(),
            nearby_stars().len()
        );
        assert!(worst < 1e-9, "a texel differs by {worst} mag");
        assert!(mesopic > 0, "the V −9 star's texel is mesopic");
    }

    /// Near the Sun, with the band at the eye's cut there (8.15), the median texel limit is 6.5 ±
    /// 0.20 in the band (|b| under 5°) and 7.55 ± 0.22 at the poles (|b| over 80°)
    /// (`decision-r06-t9b-band.md`): against the band alone, as the ruling's reference is, and with
    /// the glare of the census's nearest stars.
    #[test]
    fn near_the_sun_the_median_limits_are_crumeys_at_gaias_light_fainter_than_the_cut() {
        let spec = spec(16);
        let eye = EyeObserver::default();
        let (none, nearby) = (
            Glare::default(),
            Glare::of_listed(&observer(), nearby_stars()),
        );
        for (what, glare) in [("the band alone", &none), ("with the glare", &nearby)] {
            let band = limited(glare, &eye);
            let (plane, in_plane) = median_limit(&band, spec, 0.0..5.0);
            let (poles, at_poles) = median_limit(&band, spec, 80.0..90.1);
            eprintln!(
                "near the Sun at cut {EYE_CUT}, {what} ({} stars glare): the median limit is \
                 {plane:.3} over {in_plane} texels in the band and {poles:.3} over {at_poles} at \
                 the poles",
                glare.len()
            );
            assert!(
                (plane - BAND_MEDIAN.0).abs() <= BAND_MEDIAN.1,
                "{what}: the band's median limit {plane}"
            );
            assert!(
                (poles - POLES_MEDIAN.0).abs() <= POLES_MEDIAN.1,
                "{what}: the poles' median limit {poles}"
            );
        }
    }

    /// A texel whose centre lies 0.5° from a V −1.5 star (Sirius's colour) is at least 0.3 mag
    /// shallower than its eight neighbours' mean, towards the north galactic pole and in the plane,
    /// at the server's 64² faces.
    #[test]
    fn a_texel_within_1_degree_of_a_v_minus_1_5_star_is_shallower_than_its_neighbours() {
        let spec = BandSpec::STANDARD;
        let eye = EyeObserver::default();
        let n = usize::from(spec.face_texels());
        for (face, place) in [
            (CubeFace::PosZ, "towards the north galactic pole"),
            (CubeFace::PosX, "in the plane"),
        ] {
            let rows = 31..34;
            let mut texels = band_near_the_sun(spec, face, rows.clone());
            let centre = spec.texel_direction(face, 32, 32);
            let (u, e, rho) = star_at(off(centre, 0.5), -1.5, 9_940.0, 4.3);
            assert!(angle_deg(&centre, &u) < 1.0);
            let glare = Glare::default().with_source(u, e, rho);
            limit_rows(&eye, &spec, &glare, face, rows, &mut texels);
            let limit = |row: usize, column: usize| {
                texels[(row - 31) * n + column]
                    .eye_limit()
                    .expect("a limit")
                    .value()
            };
            let neighbours: Vec<f64> = (31..=33)
                .flat_map(|row| (31..=33).map(move |column| (row, column)))
                .filter(|&place| place != (32, 32))
                .map(|(row, column)| limit(row, column))
                .collect();
            #[expect(clippy::cast_precision_loss, reason = "eight neighbours")]
            let mean = neighbours.iter().sum::<f64>() / neighbours.len() as f64;
            let own = limit(32, 32);
            eprintln!(
                "{place}: the texel 0.5° from a V −1.5 star sees to {own:.3}, its neighbours to \
                 {mean:.3} on average"
            );
            assert!(own <= mean - 0.3, "{place}: {own} against {mean}");
        }
    }

    /// The glare's colour and the band's light are the map's only inputs: a field factor F′ moves
    /// every texel's limit by −2.5 log₁₀(F′ ÷ F), as the client's `fieldFactorOffsetMag` assumes,
    /// since neither the band nor the veil depends on F.
    #[test]
    fn a_field_factor_moves_every_limit_by_its_own_offset() {
        let glare = Glare::of_listed(&observer(), nearby_stars());
        let keen = limited(&glare, &EyeObserver::default());
        let typical = limited(&glare, &EyeObserver::new(2.0, 25.0, 0.5).expect("an eye"));
        let offset = -2.5 * math::log10(2.0 / EyeObserver::DEFAULT_FIELD_FACTOR);
        for (a, b) in keen.iter().zip(&typical) {
            let moved = b.eye_limit().expect("a limit") - a.eye_limit().expect("a limit");
            assert!(
                (moved.value() - offset).abs() < 1e-9,
                "{} against {offset}",
                moved.value()
            );
        }
    }

    /// The map is a function of the band's light and the listed stars alone: any split of a face's
    /// rows gives the same bits; the texels' chroma does not enter; with no glare each limit is
    /// `naked_eye_limit` at the texel's own light, bit for bit; and a texel with no light and no
    /// glare is seen to Crumey's clamp.
    #[test]
    fn the_map_reads_only_the_bands_light_and_the_listed_stars() {
        let spec = spec(16);
        let eye = EyeObserver::default();
        let glare = Glare::of_listed(&observer(), nearby_stars());
        let face = &near_the_sun()[..256];
        let limits = |texels: &[BandTexel]| -> Vec<u64> {
            texels
                .iter()
                .map(|t| bits(t.eye_limit().expect("a limit").value()))
                .collect()
        };
        let mut whole = face.to_vec();
        limit_rows(&eye, &spec, &glare, CubeFace::PosX, 0..16, &mut whole);
        for split in [1_u16, 5, 15] {
            let mut parts = face.to_vec();
            let (top, bottom) = parts.split_at_mut(usize::from(split) * 16);
            limit_rows(&eye, &spec, &glare, CubeFace::PosX, split..16, bottom);
            limit_rows(&eye, &spec, &glare, CubeFace::PosX, 0..split, top);
            assert_eq!(limits(&parts), limits(&whole), "split at row {split}");
        }
        let mut white: Vec<BandTexel> = face
            .iter()
            .map(|t| BandTexel::of_light(t.luminance().value(), t.sp_ratio()))
            .collect();
        limit_rows(&eye, &spec, &glare, CubeFace::PosX, 0..16, &mut white);
        assert_eq!(limits(&white), limits(&whole), "the chroma");
        // No glare: the texel's own background.
        let alone = limited(&Glare::default(), &eye);
        for texel in &alone {
            let own = SkyBackground::new(
                texel.luminance(),
                SpRatio::new(texel.sp_ratio()).expect("a ratio"),
            )
            .expect("a background");
            assert_eq!(
                bits(texel.eye_limit().expect("a limit").value()),
                bits(naked_eye_limit(&eye, &own).value())
            );
        }
        assert_ne!(limits(&alone[..256]), limits(&whole), "the glare moves it");
        // No light, no glare: Crumey's zero background.
        let mut dark = vec![BandTexel::of_light(0.0, solar_colour().sp_ratio()); 64];
        limit_rows(
            &eye,
            &BandSpec::new(8, 12).expect("a spec"),
            &Glare::default(),
            CubeFace::NegZ,
            0..8,
            &mut dark,
        );
        let clamp = naked_eye_limit(
            &eye,
            &SkyBackground::new(
                luminance(MagnitudesPerArcsec2::new(40.0)),
                SpRatio::REFERENCE,
            )
            .expect("a background"),
        );
        for texel in &dark {
            assert_eq!(
                bits(texel.eye_limit().expect("a limit").value()),
                bits(clamp.value())
            );
        }
    }

    /// The glare reads each listed star's light after its own reddening: its photopic illuminance
    /// is its unextinguished light times its photopic transmission, and its scotopic light that
    /// times its reddened ρ, `colour().reddened(a_v()).sp_ratio()`.
    #[test]
    fn the_glare_reads_each_stars_reddened_light() {
        let stars = nearby_stars();
        let glare = Glare::of_listed(&observer(), stars);
        assert_eq!(glare.len(), stars.len());
        assert!(!glare.is_empty());
        let mut reddened_stars = 0;
        for (source, (star, (u, e, rho))) in glare
            .sources
            .iter()
            .zip(stars.iter().zip(sources_of(stars)))
        {
            assert_eq!(bits(source.photopic), bits(e));
            assert_eq!(bits(source.scotopic), bits(e * rho));
            assert!(angle_deg(&source.direction, &u) < 1e-9);
            if star.a_v().value() > 0.0 {
                assert!(rho < star.colour().sp_ratio(), "the dust lowers ρ");
                reddened_stars += 1;
            }
        }
        eprintln!(
            "{} listed stars within {GLARE_RADIUS_LY} ly, {reddened_stars} behind some dust",
            stars.len()
        );
        assert!(reddened_stars > 0, "some stars lie behind dust");
    }

    /// The glare reaches 100°, CIE 146's bound, and no further: a source 99.995° from a texel's
    /// centre veils it by exactly its `veiling_luminance`, one at 100.005° (inside the cosine
    /// test's margin) adds exactly nothing, and those at 100.02° and 120° are not read.
    #[test]
    fn the_glare_reaches_100_degrees_and_no_further() {
        let spec = BandSpec::new(8, 12).expect("a spec");
        let eye = EyeObserver::default();
        let toward = spec.texel_direction(CubeFace::PosX, 4, 4);
        let (e, rho) = (illuminance_of_magnitude(Magnitudes::new(-1.5)).value(), 2.6);
        let texel = BandTexel::of_light(luminance(MagnitudesPerArcsec2::new(24.5)).value(), 2.26);
        let limits = |angles: &[f64]| {
            let glare = angles.iter().fold(Glare::default(), |glare, &angle| {
                glare.with_source(off(toward, angle), e, rho)
            });
            let mut texels = vec![texel; 64];
            limit_rows(&eye, &spec, &glare, CubeFace::PosX, 0..8, &mut texels);
            bits(texels[4 * 8 + 4].eye_limit().expect("a limit").value())
        };
        let near = off(toward, 99.995);
        let angle = Degrees::new(math::acos(toward.dot(&near)) / RADIANS_PER_DEGREE);
        assert!((angle.value() - 99.995).abs() < 1e-6, "{angle:?}");
        let veil = |e: f64| {
            veiling_luminance(&eye, Lux::new(e), angle)
                .expect("a valid glare")
                .value()
        };
        let expected = bits(texel_limit(&eye, &texel, (veil(e), veil(e * rho))).value());
        assert_eq!(limits(&[99.995, 100.005, 100.02, 120.0]), expected);
        assert_eq!(limits(&[99.995]), expected);
        let own = bits(texel_limit(&eye, &texel, (0.0, 0.0)).value());
        assert_eq!(limits(&[100.005, 100.02, 120.0]), own);
        assert_ne!(expected, own, "the source within the reach veils the texel");
    }

    #[test]
    #[should_panic(expected = "texels for rows 0..8 of a face of 8² texels")]
    fn texels_of_another_number_are_refused() {
        let mut texels = vec![BandTexel::of_light(1e-5, 2.26); 63];
        limit_rows(
            &EyeObserver::default(),
            &BandSpec::new(8, 12).expect("a spec"),
            &Glare::default(),
            CubeFace::PosX,
            0..8,
            &mut texels,
        );
    }

    #[test]
    #[should_panic(expected = "reach past a face of 8 rows")]
    fn rows_past_the_face_are_refused() {
        let mut texels = vec![BandTexel::of_light(1e-5, 2.26); 72];
        limit_rows(
            &EyeObserver::default(),
            &BandSpec::new(8, 12).expect("a spec"),
            &Glare::default(),
            CubeFace::PosX,
            0..9,
            &mut texels,
        );
    }
}
