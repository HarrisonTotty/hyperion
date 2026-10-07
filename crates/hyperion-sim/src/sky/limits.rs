//! The naked eye's limit in every direction of the band, and each listed star's own: the limit
//! map and the eye offsets (rendering plan R06, R06.T9.c and R06.T9.h; Design notes 4, 5 and 17).
//!
//! Each texel's limit is Crumey's threshold ([`naked_eye_limit`]) against the background the eye
//! sees in the direction of the texel's centre, the direction its ray takes: the band's light there,
//! the stars the census did not list ([`BandTexel`]), plus the veiling glare of every listed star
//! within 90° of it, CIE 146:2002's general disability glare ([`veiling_luminance`]; Vos 2003,
//! Lighting Res. Technol. 35, 163), which Design note 4 takes for the "standard way" (Adrian 1989,
//! Lighting Res. Technol. 21, 181) to which Crumey (2014, MNRAS 442, 2600, §1.6.3) leaves glare.
//!
//! The glare's illuminance is taken in the plane of the eye, perpendicular to the line of sight,
//! as CIE 146:2002 defines it (and IJspeert et al. 1990, Vision Res. 30, 699; Stiles and Crawford
//! 1937, Proc. R. Soc. B 122, 255; see [`veiling_luminance`]): a star of illuminance E at θ from a
//! texel's centre gives E cos θ, and a star at or beyond 90°, behind the eye's plane, gives none.
//! That nothing beyond 90° is taken, where CIE's range runs to 100°, is the ruling's inference from
//! that definition (decided 2026-10-06, `decision-r06-t9c-glare.md`); it also makes the veil
//! continuous at its reach.
//!
//! The glare is taken as the stars' own light scattered in the eye, in each star's colour (Design
//! note 4). Intraocular straylight is not spectrally neutral (near λ⁻⁴ in young, well-pigmented
//! eyes, with a red component added in light ones; Coppens, Franssen and van den Berg 2006, Exp.
//! Eye Res. 82, 688), so the veil's rod weight is uncertain by some tens of per cent: up to about
//! 0.1 mag where the veil dominates a texel. With E★ a listed star's photopic illuminance at the
//! eye after its own reddening, ρ★ its reddened S/P ratio
//! (`star.colour().reddened(star.a_v()).sp_ratio()`, R06.T9.e) and K(θ) the veil per lux in the
//! eye's plane at θ from the texel's centre (θ clamped at 0.1°, cos θ the plane's factor, nothing
//! at or beyond 90°), a texel of band luminance B and ratio ρ is seen against
//!
//! - a photopic luminance B′ = B + Σ E★ K(θ★) cos θ★, and
//! - an S/P ratio ρ′ = (ρ B + Σ ρ★ E★ K(θ★) cos θ★) ÷ B′, the band's and the veils' light together.
//!
//! In a scotopic background the threshold then reads (ρ′ ÷ 1.408) B′ of Blackwell's light: each
//! star's veil weighed by ρ★ ÷ 1.408 as the band's light is by ρ ÷ 1.408, Design note 4's rod
//! weighting. In a mesopic one each light is weighed by its MES2 mesopic luminance at the photopic
//! weight of the whole background, band and veils together, since the eye adapts to all it sees
//! ([`blackwell_equivalent_factor`]'s construction). F stays the observer's: the glare is
//! modelled, not folded into F (R06's Risks, "Glare double count").
//!
//! **A star's own veil is not its own background** (R06.T9.h). Crumey's eq. 34 fits Blackwell's
//! thresholds for small targets seen by real eyes, so the target's own light scattered in the eye
//! is already in its threshold, and CIE 146's veil is one source's light over another target. The
//! map's texels are each a function of every listed star, as the other stars see them; a listed
//! star's own limit is taken against its texel's background less the veil the map added for it,
//! at its angle from its texel's centre, and [`eye_offsets`] gives each star that limit less its
//! texel's: its self-exclusion and its colour offset ([`star_colour_offset`]) together. The wire
//! carries it on the star's eye offset (Design note 17), and a view keeps a star whose V is
//! brighter than its texel's limit plus it, its own limit.
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
    EyeObserver, SkyBackground, SpRatio, naked_eye_limit, star_colour_offset, veiling_luminance,
};

/// One listed star as the glare reads it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct GlareSource {
    /// Its direction from the observer, on the galactic axes: `None` for a star at the observer's
    /// own position, which has none and glares nothing.
    direction: Option<UnitVector>,
    /// Its photopic illuminance at the eye after its own reddening, normal to its direction, lux.
    photopic: f64,
    /// Its scotopic light at the eye, its photopic illuminance times its reddened S/P ratio, lux.
    scotopic: f64,
    /// Its reddened S/P ratio, which sets its colour offset.
    sp_ratio: f64,
}

impl GlareSource {
    /// The veil per lux of this source's illuminance over a texel whose centre lies in direction
    /// `toward`, cd m⁻² lx⁻¹: [`veiling_luminance`]'s per lux at its angle θ, times cos θ for the
    /// illuminance in the plane of the eye. `None` for a source with no direction, and for one at
    /// or beyond 90°, behind the eye's plane, which gives no illuminance there.
    ///
    /// The map's veil and each star's own term ([`eye_offsets`]) are both this, so the term a star
    /// is excluded by is, bit for bit, the term the map added for it.
    #[must_use]
    fn veil_per_lux(&self, eye: &EyeObserver, toward: &UnitVector) -> Option<f64> {
        let cos = toward.dot(&self.direction?);
        if cos <= 0.0 {
            return None;
        }
        let angle = math::acos(cos.min(1.0)) / RADIANS_PER_DEGREE;
        let per_lux = veiling_luminance(eye, Lux::new(1.0), Degrees::new(angle))
            .expect("a unit illuminance and an angle of 0–90° are valid")
            .value();
        Some(per_lux * cos)
    }

    /// The photopic and scotopic veil this source adds over a texel in direction `toward`, cd m⁻²:
    /// none where [`veil_per_lux`](Self::veil_per_lux) is `None`.
    #[must_use]
    fn veil(&self, eye: &EyeObserver, toward: &UnitVector) -> Option<[f64; 2]> {
        self.veil_per_lux(eye, toward)
            .map(|per_lux| [self.photopic * per_lux, self.scotopic * per_lux])
    }
}

/// The glare of a census's listed stars as the limit map reads it (Design note 4): each star's
/// direction from the observer, and its photopic and scotopic illuminance at the eye after its own
/// reddening.
///
/// A request builds it once from its census, and every job of the band's rows reads it: each star's
/// reddening is resolved here, once. It holds one entry a listed star, in the census's order, which
/// [`eye_offsets`] keeps. The default holds no star, as a band's limits against its own light alone
/// (the eye cut's pre-pass, R06.T9.d) are.
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
            .map(|star| {
                let direction =
                    UnitVector::from_components(origin.displacement_to(star.apparent()).metres());
                let reddened = star.colour().reddened(star.a_v());
                let photopic = unextinguished_lux(star.colour(), star.v(), star.a_v())
                    * reddened.photopic_transmission();
                GlareSource {
                    direction,
                    photopic,
                    scotopic: photopic * reddened.sp_ratio(),
                    sp_ratio: reddened.sp_ratio(),
                }
            })
            .collect();
        Self { sources }
    }

    /// The number of listed stars it holds, one a star, whether or not each glares.
    #[must_use]
    pub fn len(&self) -> usize {
        self.sources.len()
    }

    /// Whether it holds no star.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }

    /// The photopic and scotopic veiling luminance towards `toward`, cd m⁻²: each source's
    /// [`GlareSource::veil`], summed in the sources' order.
    #[must_use]
    fn veil(&self, eye: &EyeObserver, toward: &UnitVector) -> [f64; 2] {
        let (mut photopic, mut scotopic) = (0.0, 0.0);
        for [p, s] in self
            .sources
            .iter()
            .filter_map(|source| source.veil(eye, toward))
        {
            photopic += p;
            scotopic += s;
        }
        [photopic, scotopic]
    }
}

#[cfg(test)]
impl Glare {
    /// This glare and one more source: a star in `direction` of photopic illuminance `photopic`,
    /// lux, and S/P ratio `sp_ratio`.
    #[must_use]
    fn with_source(mut self, direction: UnitVector, photopic: f64, sp_ratio: f64) -> Self {
        self.sources.push(GlareSource {
            direction: Some(direction),
            photopic,
            scotopic: photopic * sp_ratio,
            sp_ratio,
        });
        self
    }
}

/// The background the eye sees over `texel` with the veil `[photopic, scotopic]` over it, cd m⁻²
/// (the [module](self) documentation). With no veil it is the texel's own, luminance and ρ bit for
/// bit.
#[must_use]
fn background(texel: &BandTexel, veil: [f64; 2]) -> SkyBackground {
    let [photopic_veil, scotopic_veil] = veil;
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
    SkyBackground::new(
        CandelasPerSquareMetre::new(luminance.min(SkyBackground::MAX_LUMINANCE.value())),
        held_ratio(sp_ratio),
    )
    .expect("a non-negative luminance held at the brightest accepted")
}

/// The ratio `sp_ratio` held within [`SpRatio`]'s 0.01–100, which no starlight leaves.
#[must_use]
fn held_ratio(sp_ratio: f64) -> SpRatio {
    SpRatio::new(sp_ratio.clamp(SpRatio::MIN.value(), SpRatio::MAX.value()))
        .expect("a finite ratio clamped into the accepted range")
}

/// The eye's limit against `texel`'s light and the veil `[photopic, scotopic]` over it, cd m⁻²
/// (the [module](self) documentation).
#[must_use]
fn texel_limit(eye: &EyeObserver, texel: &BandTexel, veil: [f64; 2]) -> Magnitudes {
    naked_eye_limit(eye, &background(texel, veil))
}

/// Sets the eye's limit of each texel of rows `rows` (from the top) of `face`, `texels` in
/// [`band_rows`](super::band::band_rows)' order (row by row, each from its left): Crumey's
/// threshold for `eye` against the texel's light and the veiling glare `glare` over it (Design note
/// 4; the [module](self) documentation). Each texel keeps the veil, photopic and scotopic, beside
/// its limit, for [`eye_offsets`].
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
    let places = rows.flat_map(|row| (0..side).map(move |column| (row, column)));
    for (texel, (row, column)) in texels.iter_mut().zip(places) {
        let toward = spec.texel_direction(face, row, column);
        let veil = glare.veil(eye, &toward);
        let limit = texel_limit(eye, texel, veil);
        texel.set_eye_limit(limit, veil);
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

/// One listed star's eye offset in its two parts, mag (R06.T9.h).
#[derive(Debug, Clone, Copy, PartialEq)]
struct EyeOffsetParts {
    /// Its own limit before its colour offset, `naked_eye_limit` at its texel's background less its
    /// own veil, less its texel's limit.
    self_exclusion: Magnitudes,
    /// Its colour offset against its texel's background less its own veil.
    colour: Magnitudes,
}

impl EyeOffsetParts {
    /// The eye offset, the two parts' sum.
    #[must_use]
    fn total(self) -> Magnitudes {
        self.self_exclusion + self.colour
    }
}

/// A background of no light, against which a colour offset is the scotopic 2.5 log₁₀(ρ★ ÷ 2.297).
#[must_use]
fn scotopic_sky() -> SkyBackground {
    SkyBackground::new(CandelasPerSquareMetre::ZERO, SpRatio::REFERENCE)
        .expect("no light is a valid background")
}

/// The eye offset of `source`, in its two parts, against the limits [`limit_map`] set in `band`
/// (the [`eye_offsets`] documentation).
///
/// # Panics
///
/// If the source's texel has no eye limit; in debug builds, if its own veil exceeds the veil its
/// texel's limit was taken against.
#[must_use]
fn eye_offset_parts(
    eye: &EyeObserver,
    spec: BandSpec,
    source: &GlareSource,
    band: &[BandTexel],
) -> EyeOffsetParts {
    let rho = held_ratio(source.sp_ratio);
    let Some(direction) = source.direction else {
        return EyeOffsetParts {
            self_exclusion: Magnitudes::ZERO,
            colour: star_colour_offset(rho, &scotopic_sky()),
        };
    };
    let (face, row, column) = spec
        .texel_of(direction.components())
        .expect("a unit direction falls in a texel");
    let side = usize::from(spec.face_texels());
    let texel =
        &band[(usize::from(face.layer()) * side + usize::from(row)) * side + usize::from(column)];
    let (Some(limit), Some(veil)) = (texel.eye_limit(), texel.eye_veil()) else {
        panic!("the limit map sets every texel's eye limit before the eye offsets are read");
    };
    let own = source
        .veil(eye, &spec.texel_direction(face, row, column))
        .unwrap_or([0.0, 0.0]);
    // The texel's veil is a sum of non-negative terms that holds this one, so in floating point it
    // is at least this term, and each difference is at least zero.
    debug_assert!(
        own[0] <= veil[0] && own[1] <= veil[1],
        "a star's own veil {own:?} within its texel's {veil:?}: the band's limits are this glare's"
    );
    let background = background(texel, [veil[0] - own[0], veil[1] - own[1]]);
    EyeOffsetParts {
        self_exclusion: naked_eye_limit(eye, &background) - limit,
        colour: star_colour_offset(rho, &background),
    }
}

/// Each listed star's eye offset, its own eye limit less its texel's, mag: one a star of `glare`,
/// in the census's order (R06.T9.h; Design notes 4 and 17). `band` is the band whose limits
/// [`limit_map`] set for `eye`, `spec` and `glare`, its six faces in [`CubeFace::ALL`]'s order.
///
/// A star's own veil is not its own background (the [module](self) documentation). A star's texel
/// is the one its direction falls in ([`BandSpec::texel_of`]), and its own limit is
/// [`naked_eye_limit`] against that texel's background less the veil the map added for it, at its
/// angle from the texel's centre (the same term, bit for bit), plus its [`star_colour_offset`] at
/// its reddened ρ against that background. Its offset is that less the texel's limit, the sum of:
///
/// - its self-exclusion, its own limit before the colour offset less the texel's limit, which is
///   never negative in a scotopic texel;
/// - its colour offset, Design note 3's 2.5 log₁₀(ρ★ ÷ 2.297) in a scotopic texel, fading by MES2's
///   photopic weight in a mesopic one.
///
/// A star that adds no veil takes its colour offset alone; a star at the observer's own position,
/// which has no direction, takes it against a scotopic sky. The texels' limits do not change.
///
/// The field factor cancels: it moves both limits by −2.5 log₁₀ F, so a view's own factor, applied
/// as that offset to every texel, leaves each star's own limit exact. The wire quantises the offset
/// once, to `i8` centimagnitudes, saturating at −1.28 and +1.27 (Design note 17). A star's V less
/// its texel's limit rises with its V, so the +1.27 culls a star the offset would keep only when a
/// star at its own limit lies more than 1.27 fainter than its texel's limit. At the default eye (F
/// 1.4, 25 years) none does: at Crumey's darkest-sky clamp, where the self-exclusion is largest,
/// the colour table's hottest star close to its texel's centre has an offset of up to 1.45, but a
/// star that bright lies only its colour offset fainter than its texel's limit, and a fainter one
/// at its own limit 1.10 fainter. From F ≈ 2.2 (2.0 at 80 years) the saturation takes up to 0.05
/// mag of such a star's window at Crumey's F 2.4, and 0.12 at 80 years with light eyes, within the
/// 0.16 ruled (R06's Risks, "Deviations in T9.h, as built").
///
/// # Panics
///
/// If `band` is not six faces of `spec`'s texels in number, or a star's texel has no eye limit
/// (the limit map was not run on `band`); and in debug builds, if a star's own veil exceeds its
/// texel's, as a band whose limits were set against another glare can give.
///
/// # Examples
///
/// The eye offsets of the stars within 30 ly of the Sun brighter than V 8.15, about the eye's cut
/// there, against the band of the rest: a view sees a star whose V is brighter than its
/// texel's limit plus its offset, its own limit (`no_run`: the tables take a minute or more to
/// build):
///
/// ```no_run
/// use core::num::NonZeroU32;
///
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::{GalacticPosition, UnitVector};
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::gas::modifiers::NoModifiers;
/// use hyperion_sim::galaxy::gas::noise::NoiseCache;
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::observe::Observer;
/// use hyperion_sim::sky::band::{BandSpec, CompleteTo, CubeFace, band_rows};
/// use hyperion_sim::sky::census::{
///     CellOffsets, CensusTallies, MAX_N_MAX, NoSkyCellCache, SkyContext, SkyQuery, census_cell,
///     census_plan, merge_census,
/// };
/// use hyperion_sim::sky::envelope::BrightnessEnvelope;
/// use hyperion_sim::sky::eye::EyeObserver;
/// use hyperion_sim::sky::limits::{Glare, eye_offsets, limit_map};
/// use hyperion_sim::sky::luminosity::LuminosityTables;
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::{LightYears, Magnitudes};
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
/// let query = SkyQuery::builder(observer, Magnitudes::new(8.15))
///     .build()?
///     .with_caps_forced(LightYears::new(30.0))?;
/// let plan = census_plan(&galaxy, ctx.tables, ctx.envelope, &query, &mut ctx.noise);
/// let mut stars = Vec::new();
/// for key in plan.cells() {
///     census_cell(&galaxy, &mut ctx, key, &query, &mut stars);
/// }
/// let all = NonZeroU32::new(MAX_N_MAX).ok_or("not zero")?;
/// let census = merge_census([(stars, CensusTallies::default())], all);
/// let listed = census.listed();
/// let spec = BandSpec::new(16, 12).ok_or("a band")?;
/// let complete_to = CompleteTo::of_caps(plan.caps());
/// let mut band = Vec::new();
/// for face in CubeFace::ALL {
///     band_rows(&galaxy, &mut ctx, &query, &census, &complete_to, &spec, face, 0..16, &mut band);
/// }
/// let (eye, glare) = (EyeObserver::default(), Glare::of_listed(&observer, listed));
/// limit_map(&eye, &spec, &glare, &mut band);
/// let n = usize::from(spec.face_texels());
/// for (star, offset) in listed.iter().zip(eye_offsets(&eye, &spec, &glare, &band)) {
///     // The texel `eye_offsets` reads: the one the star's unit direction falls in.
///     let toward = observer.position().displacement_to(star.apparent()).metres();
///     let toward = UnitVector::from_components(toward).ok_or("a star away from the observer")?;
///     let (face, row, column) = spec.texel_of(toward.components()).ok_or("a direction")?;
///     let index = (usize::from(face.layer()) * n + usize::from(row)) * n + usize::from(column);
///     let own = band[index].eye_limit().ok_or("the map's limit")? + offset;
///     println!("V {:.2}, seen to {:.2}: {}", star.v().value(), own.value(), star.v() < own);
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn eye_offsets(
    eye: &EyeObserver,
    spec: &BandSpec,
    glare: &Glare,
    band: &[BandTexel],
) -> Vec<Magnitudes> {
    let side = spec.face_texels();
    assert_eq!(
        band.len(),
        CubeFace::ALL.len() * usize::from(side) * usize::from(side),
        "a band of six faces of {side}² texels"
    );
    glare
        .sources
        .iter()
        .map(|source| eye_offset_parts(eye, *spec, source, band).total())
        .collect()
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
    use crate::sky::eye::{
        BLACKWELL_SP_RATIO, DARKEST_BACKGROUND, PhotopicWeight, REFERENCE_SP_RATIO,
        illuminance_of_magnitude, luminance, mesopic_weight,
    };
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

    /// The eye offsets the wire holds, mag: `i8` centimagnitudes, saturating (Design note 17).
    const WIRE_MIN_EYE_OFFSET_MAG: f64 = -1.28;
    const WIRE_MAX_EYE_OFFSET_MAG: f64 = 1.27;

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

    /// The plan's definition, written out again: the background over a texel in direction
    /// `toward`, its luminance plus each source's veil, `veiling_luminance` of its illuminance in
    /// the plane of the eye, E cos θ, at its angle θ (from `atan2`) for every source within 90°,
    /// and none beyond; at the S/P ratio of the band's and the veils' light together. Source
    /// `skip`, a star's own, is left out.
    fn reference_background(
        eye: &EyeObserver,
        texel: &BandTexel,
        toward: &UnitVector,
        sources: &[(UnitVector, f64, f64)],
        skip: Option<usize>,
    ) -> SkyBackground {
        let light = texel.luminance().value();
        let (mut photopic, mut scotopic) = (light, light * texel.sp_ratio());
        for (i, (u, e, rho)) in sources.iter().enumerate() {
            let theta = angle_deg(toward, u);
            if skip == Some(i) || theta >= 90.0 {
                continue;
            }
            let (angle, in_plane) = (
                Degrees::new(theta),
                e * math::cos(theta * RADIANS_PER_DEGREE),
            );
            photopic += veiling_luminance(eye, Lux::new(in_plane), angle)
                .expect("a valid glare")
                .value();
            scotopic += veiling_luminance(eye, Lux::new(in_plane * rho), angle)
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

    /// The texel of a band of `spec` that `direction` falls in: its index in the band's order and
    /// the direction of its centre.
    fn texel_of(spec: BandSpec, direction: &UnitVector) -> (usize, UnitVector) {
        let (face, row, column) = spec
            .texel_of(direction.components())
            .expect("a unit direction falls in a texel");
        let n = usize::from(spec.face_texels());
        (
            (usize::from(face.layer()) * n + usize::from(row)) * n + usize::from(column),
            spec.texel_direction(face, row, column),
        )
    }

    /// The fixture's sources: the stars the census lists within [`GLARE_RADIUS_LY`], then a V −1.5
    /// white star 0.05° from a texel's centre (read at 0.1°), and a V −9 one 0.3° from another's,
    /// which makes that texel's background mesopic.
    fn fixture_sources(spec: BandSpec) -> Vec<(UnitVector, f64, f64)> {
        let mut sources = sources_of(nearby_stars());
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
        sources
    }

    /// The identity: every texel's limit is `naked_eye_limit` at the background written again from
    /// the definition, and every listed star's own limit, its texel's plus its eye offset, is
    /// `naked_eye_limit` plus `star_colour_offset` at that background without its own veil, to
    /// 10⁻⁹ mag, on the near-Sun fixture and its two placed sources.
    #[test]
    fn each_texels_limit_and_each_stars_own_are_crumeys_at_their_light_and_glare() {
        let spec = spec(16);
        let eye = EyeObserver::default();
        let sources = fixture_sources(spec);
        let glare = glare_of(&sources);
        let band = limited(&glare, &eye);
        let (mut worst, mut mesopic) = (0.0_f64, 0);
        for (texel, (toward, _)) in band.iter().zip(texel_latitudes(spec)) {
            let background = reference_background(&eye, texel, &toward, &sources, None);
            if mesopic_weight(&background).value() > 0.0 {
                mesopic += 1;
            }
            let expected = naked_eye_limit(&eye, &background).value();
            let got = texel.eye_limit().expect("a limit").value();
            worst = worst.max((got - expected).abs());
        }
        let offsets = eye_offsets(&eye, &spec, &glare, &band);
        assert_eq!(offsets.len(), sources.len());
        let mut worst_own = 0.0_f64;
        for (i, ((u, _, rho), offset)) in sources.iter().zip(&offsets).enumerate() {
            let (index, toward) = texel_of(spec, u);
            let texel = &band[index];
            let background = reference_background(&eye, texel, &toward, &sources, Some(i));
            let expected = naked_eye_limit(&eye, &background)
                + star_colour_offset(SpRatio::new(*rho).expect("a ratio"), &background);
            let got = texel.eye_limit().expect("a limit") + *offset;
            worst_own = worst_own.max((got - expected).value().abs());
        }
        eprintln!(
            "{} sources ({} listed within {GLARE_RADIUS_LY} ly): the worst texel differs from \
             the definition by {worst:.3e} mag, the worst star's own limit by {worst_own:.3e}; \
             {mesopic} mesopic texels",
            sources.len(),
            nearby_stars().len()
        );
        assert!(worst < 1e-9, "a texel differs by {worst} mag");
        assert!(
            worst_own < 1e-9,
            "a star's own limit differs by {worst_own} mag"
        );
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
    /// since neither the band nor the veil depends on F; and F cancels in every eye offset, which
    /// is a difference of two limits at one F (R06.T9.h).
    #[test]
    fn a_field_factor_moves_every_limit_by_its_own_offset_and_no_eye_offset() {
        let spec = spec(16);
        let glare = glare_of(&fixture_sources(spec));
        let (keen_eye, typical_eye) = (
            EyeObserver::default(),
            EyeObserver::new(2.0, 25.0, 0.5).expect("an eye"),
        );
        let keen = limited(&glare, &keen_eye);
        let typical = limited(&glare, &typical_eye);
        let offset = -2.5 * math::log10(2.0 / EyeObserver::DEFAULT_FIELD_FACTOR);
        for (a, b) in keen.iter().zip(&typical) {
            let moved = b.eye_limit().expect("a limit") - a.eye_limit().expect("a limit");
            assert!(
                (moved.value() - offset).abs() < 1e-9,
                "{} against {offset}",
                moved.value()
            );
        }
        let keen_offsets = eye_offsets(&keen_eye, &spec, &glare, &keen);
        let typical_offsets = eye_offsets(&typical_eye, &spec, &glare, &typical);
        let mut worst = 0.0_f64;
        for (a, b) in keen_offsets.iter().zip(&typical_offsets) {
            worst = worst.max((*b - *a).value().abs());
        }
        eprintln!("F = 2 against 1.4: the eye offsets differ by {worst:.3e} mag at most");
        assert!(worst < 1e-9, "an eye offset moves by {worst} mag");
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
        // Each texel's limit and the veil it keeps, which the eye offsets read.
        let limits = |texels: &[BandTexel]| -> Vec<(u64, [u64; 2])> {
            texels
                .iter()
                .map(|t| {
                    (
                        bits(t.eye_limit().expect("a limit").value()),
                        t.eye_veil().expect("a veil").map(bits),
                    )
                })
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
        // The eye offsets of a band set whole and of one set in split rows, the last rows first.
        let offsets = |band: &[BandTexel]| -> Vec<u64> {
            eye_offsets(&eye, &spec, &glare, band)
                .iter()
                .map(|o| bits(o.value()))
                .collect()
        };
        let side = usize::from(spec.face_texels());
        let mut split = near_the_sun().to_vec();
        for (face, texels) in CubeFace::ALL
            .into_iter()
            .zip(split.chunks_exact_mut(side * side))
        {
            let (top, bottom) = texels.split_at_mut(7 * side);
            limit_rows(&eye, &spec, &glare, face, 7..16, bottom);
            limit_rows(&eye, &spec, &glare, face, 0..7, top);
        }
        assert_eq!(
            offsets(&split),
            offsets(&limited(&glare, &eye)),
            "the eye offsets"
        );
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
            assert_eq!(bits(source.sp_ratio), bits(rho));
            assert!(angle_deg(&source.direction.expect("a direction"), &u) < 1e-9);
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

    /// The glare reaches 90°, the plane of the eye, and no further (R06.T9.h): a source 89.995°
    /// from a texel's centre veils it by exactly its `veiling_luminance` per lux times cos θ times
    /// its illuminance, and one at 90.005°, behind the eye's plane, adds exactly nothing, as those
    /// at 99.995° and 120° do not. The texel keeps that veil beside its limit.
    #[test]
    fn the_glare_reaches_90_degrees_in_the_plane_of_the_eye_and_no_further() {
        let spec = BandSpec::new(8, 12).expect("a spec");
        let eye = EyeObserver::default();
        let toward = spec.texel_direction(CubeFace::PosX, 4, 4);
        let (e, rho) = (illuminance_of_magnitude(Magnitudes::new(-1.5)).value(), 2.6);
        let texel = BandTexel::of_light(luminance(MagnitudesPerArcsec2::new(24.5)).value(), 2.26);
        let lit = |angles: &[f64]| {
            let glare = angles.iter().fold(Glare::default(), |glare, &angle| {
                glare.with_source(off(toward, angle), e, rho)
            });
            let mut texels = vec![texel; 64];
            limit_rows(&eye, &spec, &glare, CubeFace::PosX, 0..8, &mut texels);
            texels[4 * 8 + 4]
        };
        let limit_bits = |texel: BandTexel| bits(texel.eye_limit().expect("a limit").value());
        let near = off(toward, 89.995);
        let cos = toward.dot(&near);
        let angle = Degrees::new(math::acos(cos) / RADIANS_PER_DEGREE);
        assert!((angle.value() - 89.995).abs() < 1e-6, "{angle:?}");
        let per_lux = veiling_luminance(&eye, Lux::new(1.0), angle)
            .expect("a valid glare")
            .value()
            * cos;
        let veil = [e * per_lux, e * rho * per_lux];
        let expected = bits(texel_limit(&eye, &texel, veil).value());
        let within = lit(&[89.995]);
        assert_eq!(limit_bits(within), expected);
        assert_eq!(within.eye_veil().map(|v| v.map(bits)), Some(veil.map(bits)));
        assert_eq!(limit_bits(lit(&[89.995, 90.005, 99.995, 120.0])), expected);
        let own = bits(texel_limit(&eye, &texel, [0.0, 0.0]).value());
        let behind = lit(&[90.005, 99.995, 120.0]);
        assert_eq!(limit_bits(behind), own);
        assert_eq!(behind.eye_veil(), Some([0.0, 0.0]));
        assert_ne!(expected, own, "the source within the reach veils the texel");
    }

    /// A star of the reference colour 0.05° from a 64² texel's centre towards the north galactic
    /// pole, 0.05 mag brighter than its own limit (its texel's background less its own veil), is
    /// culled at its texel's limit, which its own veil makes shallower, and kept at its own, as a
    /// view's cull (`starIsSeen`: V < limit + eye offset) keeps it (R06.T9.h).
    #[test]
    fn a_star_near_its_texels_centre_is_culled_at_its_texels_limit_and_kept_at_its_own() {
        let spec = BandSpec::STANDARD;
        let eye = EyeObserver::default();
        let n = usize::from(spec.face_texels());
        let centre = spec.texel_direction(CubeFace::PosZ, 32, 32);
        let pole = band_near_the_sun(spec, CubeFace::PosZ, 32..33)[32];
        let own_limit = texel_limit(&eye, &pole, [0.0, 0.0]);
        let v = own_limit - Magnitudes::new(0.05);
        let u = off(centre, 0.05);
        let glare = Glare::default().with_source(
            u,
            illuminance_of_magnitude(v).value(),
            REFERENCE_SP_RATIO,
        );
        let mut band = vec![pole; CubeFace::ALL.len() * n * n];
        limit_map(&eye, &spec, &glare, &mut band);
        let (index, _) = texel_of(spec, &u);
        assert_eq!(
            index,
            (4 * n + 32) * n + 32,
            "the star lies in the pole's texel"
        );
        let limit = band[index].eye_limit().expect("a limit");
        let offsets = eye_offsets(&eye, &spec, &glare, &band);
        assert_eq!(offsets.len(), 1, "one offset for one star");
        let offset = offsets[0];
        eprintln!(
            "towards the pole, a reference star of V {:.3} 0.05° from a 64² texel's centre: the \
             texel sees to {:.3}, the star's own limit is {:.3} (offset {:+.3})",
            v.value(),
            limit.value(),
            (limit + offset).value(),
            offset.value()
        );
        // One source: the texel's veil is its term alone, so the term subtracted leaves exactly
        // none, and the star's own limit is its texel's with no glare, bit for bit.
        assert_eq!(bits(offset.value()), bits((own_limit - limit).value()));
        assert!(
            v >= limit,
            "culled at its texel's limit: V {v:?} against {limit:?}"
        );
        assert!(
            v < limit + offset,
            "kept at its own: V {v:?} against {:?}",
            limit + offset
        );
    }

    /// In the fixture's scotopic texels no listed star's self-exclusion is negative: its own light
    /// only brightens its texel's background there; and the wire's saturation of the eye offset
    /// changes no star's verdict. Also prints T9.h's records for the fixture:
    /// the largest self-exclusion that decides a star (its V between its limit before the
    /// self-exclusion and its own), the largest eye offset such a star and any star has, and the
    /// largest self-exclusion of a star within 0.5 mag of its own limit and 0.6° or more from its
    /// texel's centre.
    #[test]
    fn every_self_exclusion_is_non_negative_in_a_scotopic_texel() {
        let spec = spec(16);
        let eye = EyeObserver::default();
        let stars = nearby_stars();
        let glare = Glare::of_listed(&observer(), stars);
        let band = limited(&glare, &eye);
        let (mut scotopic, mut deciding, mut saturated) = (0, 0, 0);
        let (mut decides, mut decided_offset, mut largest_offset, mut far) =
            (0.0_f64, f64::NEG_INFINITY, f64::NEG_INFINITY, 0.0_f64);
        for (star, source) in stars.iter().zip(&glare.sources) {
            let direction = source.direction.expect("a star away from the observer");
            let (index, toward) = texel_of(spec, &direction);
            let texel = &band[index];
            let parts = eye_offset_parts(&eye, spec, source, &band);
            let (self_exclusion, total) = (parts.self_exclusion.value(), parts.total().value());
            let glared = background(texel, texel.eye_veil().expect("a veil"));
            if mesopic_weight(&glared) == PhotopicWeight::SCOTOPIC {
                scotopic += 1;
                assert!(
                    self_exclusion >= 0.0,
                    "a self-exclusion of {self_exclusion}"
                );
            }
            let limit = texel.eye_limit().expect("a limit").value();
            let own = limit + total;
            let v = star.v().value();
            largest_offset = largest_offset.max(total);
            let carried = total.clamp(WIRE_MIN_EYE_OFFSET_MAG, WIRE_MAX_EYE_OFFSET_MAG);
            if (v < limit + carried) != (v < own) {
                saturated += 1;
            }
            if own - self_exclusion <= v && v < own {
                deciding += 1;
                decides = decides.max(self_exclusion);
                decided_offset = decided_offset.max(total);
            }
            if (v - own).abs() <= 0.5 && angle_deg(&toward, &direction) >= 0.6 {
                far = far.max(self_exclusion);
            }
        }
        eprintln!(
            "{} listed stars at 16², {scotopic} in scotopic texels: {deciding} decided by their \
             self-exclusion, the largest {decides:.4} mag (their largest eye offset \
             {decided_offset:+.4}); the largest eye offset of any star {largest_offset:+.4}; the \
             largest self-exclusion within 0.5 mag of a star's own limit and 0.6° or more from its \
             texel's centre {far:.2e}",
            stars.len()
        );
        assert!(scotopic > 0, "stars in scotopic texels");
        assert_eq!(
            saturated, 0,
            "the wire's saturation changes a star's verdict"
        );
    }

    /// The wire's saturation at Crumey's clamp, for one eye ([`saturation_at_the_clamp`]).
    #[derive(Debug)]
    struct Saturation {
        /// The star's own limit.
        own: f64,
        /// The V of the brightest star its self-exclusion decides, and that star's texel's limit
        /// and eye offset.
        first: (f64, f64, EyeOffsetParts),
        /// How much fainter than its texel's limit a star at its own limit lies.
        at_own: f64,
        /// The window the saturation takes: V from its texel's limit + 1.27 to its own limit.
        lost: f64,
    }

    /// A star of S/P ratio `rho`, 0.05° from a 64² texel's centre (read at 0.1°), alone over a
    /// texel whose colour-corrected background lies exactly at Crumey's clamp, 10⁻⁵ cd m⁻², where
    /// the self-exclusion is largest, seen by `eye`: it is seen while V < its own limit, and the
    /// wire's +1.27 culls it where V ≥ its texel's limit + 1.27.
    fn saturation_at_the_clamp(eye: &EyeObserver, rho: f64) -> Saturation {
        let spec = BandSpec::STANDARD;
        let n = usize::from(spec.face_texels());
        let u = off(spec.texel_direction(CubeFace::PosZ, 32, 32), 0.05);
        let (index, _) = texel_of(spec, &u);
        let dark = BandTexel::of_light(DARKEST_BACKGROUND.value(), BLACKWELL_SP_RATIO);
        // The star of V `v`: its texel's limit, and its eye offset in its two parts.
        let at = |v: f64| {
            let illuminance = illuminance_of_magnitude(Magnitudes::new(v)).value();
            let glare = Glare::default().with_source(u, illuminance, rho);
            let mut band = vec![dark; CubeFace::ALL.len() * n * n];
            limit_map(eye, &spec, &glare, &mut band);
            let parts = eye_offset_parts(eye, spec, &glare.sources[0], &band);
            (band[index].eye_limit().expect("a limit").value(), parts)
        };
        // The faintest V at which `fainter` holds, by bisection over [lo, hi], where it holds at
        // hi and not at lo.
        let bisect = |fainter: &dyn Fn(f64) -> bool, (mut lo, mut hi): (f64, f64)| {
            for _ in 0..80 {
                let mid = f64::midpoint(lo, hi);
                if fainter(mid) {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            hi
        };
        let (_, faint) = at(20.0);
        let own = texel_limit(eye, &dark, [0.0, 0.0]).value() + faint.colour.value();
        // The brightest star its self-exclusion decides: its V is its limit before the
        // self-exclusion, its texel's limit plus its colour offset. V less its texel's limit
        // rises with V, so the faintest star the wire could cull lies at its own limit.
        let first = bisect(
            &|v| {
                let (limit, parts) = at(v);
                v >= limit + parts.colour.value()
            },
            (own - 3.0, own),
        );
        let (limit, parts) = at(first);
        let at_own = own - at(own).0;
        let lost = if at_own <= WIRE_MAX_EYE_OFFSET_MAG {
            0.0
        } else {
            own - bisect(&|v| v - at(v).0 >= WIRE_MAX_EYE_OFFSET_MAG, (first, own))
        };
        Saturation {
            own,
            first: (first, limit, parts),
            at_own,
            lost,
        }
    }

    /// At Crumey's darkest-sky clamp the wire's saturation of the eye offset at +1.27 (Design note
    /// 17) costs a star near its texel's centre at most 0.16 mag of its window (the ruling's
    /// bound, `decision-r06-t9c-glare.md`, item 1), for the colour table's hottest star, over
    /// Crumey's field factors to 2.4 and CIE's ages to 80 years; and nothing at the default eye,
    /// whose largest offset passes +1.27 only for stars no fainter than their texel's limit plus
    /// their colour offset.
    #[test]
    fn at_crumeys_clamp_the_wires_saturation_costs_a_star_at_most_0_16_mag_of_its_window() {
        use crate::tables::star_colour::{NORMAL, WHITE_DWARF};
        let hottest = NORMAL
            .iter()
            .chain(&WHITE_DWARF)
            .map(|row| row[3])
            .fold(f64::NEG_INFINITY, f64::max);
        for (f, age, p) in [(1.4, 25.0, 0.5), (2.4, 25.0, 0.5), (2.4, 80.0, 1.2)] {
            let eye = EyeObserver::new(f, age, p).expect("an eye");
            let Saturation {
                own,
                first: (first, limit, parts),
                at_own,
                lost,
            } = saturation_at_the_clamp(&eye, hottest);
            let largest = parts.total().value();
            eprintln!(
                "at Crumey's clamp, F {f}, age {age}, p {p}, the hottest row (ρ {hottest:.4}) \
                 0.05° from a 64² texel's centre: own limit {own:.4}; the brightest star its \
                 self-exclusion decides, V {first:.4} (texel's limit {limit:.4}), has the largest \
                 eye offset, {largest:+.4} ({:.4} self-exclusion, {:+.4} colour), {:+.4} past the \
                 wire's +1.27; a star at its own limit lies {at_own:.4} fainter than its texel's \
                 limit; the saturation takes {lost:.4} mag of the window",
                parts.self_exclusion.value(),
                parts.colour.value(),
                largest - WIRE_MAX_EYE_OFFSET_MAG,
            );
            assert!(
                (limit + largest - own).abs() < 1e-9,
                "the star's own limit is fixed"
            );
            assert!(
                lost <= 0.16,
                "F {f}, age {age}: the saturation takes {lost} mag of the window"
            );
            if eye == EyeObserver::default() {
                assert!(largest > WIRE_MAX_EYE_OFFSET_MAG, "the offset saturates");
                assert!(
                    at_own < WIRE_MAX_EYE_OFFSET_MAG,
                    "but the saturation culls no star: {at_own}"
                );
            }
        }
    }

    /// A star that adds no veil takes its colour offset alone: one with no illuminance takes it
    /// against its texel's background, and one at the observer's own position, which has no
    /// direction, against a scotopic sky.
    #[test]
    fn a_star_that_adds_no_veil_takes_its_colour_offset_alone() {
        let spec = BandSpec::new(8, 12).expect("a spec");
        let eye = EyeObserver::default();
        let mut glare = Glare::default().with_source(UnitVector::NORTH, 0.0, 3.0);
        glare.sources.push(GlareSource {
            direction: None,
            photopic: 1e-6,
            scotopic: 3e-6,
            sp_ratio: 3.0,
        });
        let texel = BandTexel::of_light(luminance(MagnitudesPerArcsec2::new(17.0)).value(), 2.26);
        let mut band = vec![texel; CubeFace::ALL.len() * 64];
        limit_map(&eye, &spec, &glare, &mut band);
        let rho = SpRatio::new(3.0).expect("a ratio");
        let mesopic = background(&texel, [0.0, 0.0]);
        assert!(mesopic_weight(&mesopic).value() > 0.0, "a mesopic texel");
        let offsets = eye_offsets(&eye, &spec, &glare, &band);
        assert_eq!(
            offsets.iter().map(|o| bits(o.value())).collect::<Vec<_>>(),
            [
                bits(star_colour_offset(rho, &mesopic).value()),
                bits(star_colour_offset(rho, &scotopic_sky()).value()),
            ]
        );
        let scotopic = 2.5 * math::log10(3.0 / REFERENCE_SP_RATIO);
        assert!((offsets[1].value() - scotopic).abs() < 1e-12, "{offsets:?}");
        assert!(
            (offsets[0].value() - scotopic).abs() > 1e-3,
            "the mesopic offset fades"
        );
    }

    #[test]
    #[should_panic(expected = "a band of six faces of 8² texels")]
    fn a_band_of_another_size_is_refused_by_the_eye_offsets() {
        let band = vec![BandTexel::of_light(1e-5, 2.26); 6 * 64 - 1];
        let _ = eye_offsets(
            &EyeObserver::default(),
            &BandSpec::new(8, 12).expect("a spec"),
            &Glare::default(),
            &band,
        );
    }

    #[test]
    #[should_panic(expected = "the limit map sets every texel's eye limit")]
    fn a_band_without_its_limits_is_refused_by_the_eye_offsets() {
        let band = vec![BandTexel::of_light(1e-5, 2.26); 6 * 64];
        let glare = Glare::default().with_source(UnitVector::X, 1e-6, 2.26);
        let _ = eye_offsets(
            &EyeObserver::default(),
            &BandSpec::new(8, 12).expect("a spec"),
            &glare,
            &band,
        );
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
