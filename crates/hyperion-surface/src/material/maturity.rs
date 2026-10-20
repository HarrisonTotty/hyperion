//! The space-weathering maturity: its time law, shared by the sim and the client
//! (decision-p14-t51d-weathering §1.4 and §2.1; plan 14, P14.T51.d), and the ground's exposure, the
//! maturity at a point and the organic cover that R10.T1.c applies to each material's albedo
//! (decision-r10-t1c-maturity, 2026-10-20).
//!
//! The maturity m is the background regolith's nanophase-iron (npFe⁰) content as a fraction of
//! the Moon's mature background's, per unit of `FeO_T`: 0 for fresh ground and 1 for the saturation
//! the Moon's mature background regolith reaches, the optical-maturity floor of an old lunar
//! surface. Ground exposed for a time t at a weathering rate k, the body's surface-reaching dose
//! rate relative to the Moon's present one (k = 1 for the Moon), matures as
//!
//! m(t) = 1 − exp(−k t ÷ `τ_L`),
//!
//! a first-order saturation, labelled: lunar optical-maturity profiles may fade otherwise, and the
//! Tycho and Copernicus bounds below are what it is held to (the ruling's Risks). `τ_L` is
//! [`LUNAR_MATURATION_TIME`]. Saturation is why both the rate and the maturity are needed: in
//! `f64`, m is exactly 1 past k t ÷ `τ_L` ≈ 37, so k cannot be recovered from m on an old surface.
//!
//! - [`body_maturity`] is a body's background maturity at its surface age, P14.T24.b's, which
//!   already carries its resurfacing (P14.T51.d's `weathering_rate` and `maturity`).
//! - [`maturity_after`] is ground exposed for a given time, such as a crater's ejecta.
//!
//! The darkening the maturity drives is R10.T1.d's (`material/weathering.rs`), not this module's.
//!
//! # The exposed albedo
//!
//! [`exposed_properties`] is [`properties()`](super::properties()) with the albedo a ground's
//! exposure ([`GroundExposure`]: a maturity and an organic cover, either absent) gives it, the one
//! function the server's albedo scale ([`exposed_albedo_scale`](super::exposed_albedo_scale)), the
//! worker's photometry and class map (at the header's `m_body`) and its point query (at the point's
//! `m_local`) all call (decision-r10-t1c-maturity R2). In each band it
//!
//! 1. weathers the entry's `A_N` to the exposure's maturity by R10.T1.d's law
//!    ([`weathered_entry_bvr`]), the identity for an entry of `FeO_T` `None` (an observed albedo,
//!    weathered already: nothing darkens twice, decision-p14-t51d-weathering §1.5) or 0;
//! 2. multiplies it by the form's factor, as `properties` does;
//! 3. blends in the organic cover where the entry takes one (below).
//!
//! `Melt` skips the first step: a molten surface holds no npFe⁰-bearing regolith. Every other form
//! weathers at the exposure's maturity, `Bedrock` included, as the solve has it; that steep slopes
//! stay immature by mass wasting ("Mass wasting has exposed less weathered surfaces on steep
//! slopes", Lucey et al. 2014, JGR Planets 119, 1665, Fig. 6's caption, p. 1670; and Lemelin et al.
//! 2020, Icarus 351, 113926, as the ruling cites it, not read: its subject is Jackson's and Tycho's
//! impact melts) waits for R10.T1.d's whole-soil fresh rows (D2), under whose stand-in rows a fresh
//! cliff would draw five times its background. With neither a maturity nor a cover the result is
//! `properties`' to the bit.
//!
//! # The organic cover
//!
//! The header's cover c is "the covered fraction of the ground that can take one" (P14.T51.d's
//! `planetary/hooks/modifiers.rs`), and the ground that can take one is its `organic_ground`: the
//! solid, non-seasonal condensates whose registry row carries carbon, CO₂ excepted, and N₂'s where
//! one of those is present. On the palette's keys ([`takes_organic_cover`]) that is an `Ice`-role
//! entry whose key is a formula holding carbon (a `C` not followed by a lowercase letter, so that
//! `Ca`, `Cl`, `Co` and `Cs` are not carbon), `CO2` excepted, and an `Ice`-role `N2` beside one.
//! The cover itself is the palette's first organic `Cover` entry, the classifier's
//! `OrganicSediment` substance. The blend is areal, per band: (1 − c) A + c `A_cover`, A the
//! material's albedo after the first two steps and `A_cover` the cover entry's own `A_N` weathered
//! to the same maturity (the identity at its `FeO_T` of 0), without a form factor; the material
//! keeps its law and its origin, on every form of an eligible entry. Recorded, not built: one law
//! for the mix on a consolidated form, where Design note 8 would sum each law's radiance; young
//! craters on covered ice keep the cover; and the seasonal ices, which `organic_ground` drops,
//! cannot be told on the palette.
//!
//! # The maturity at a point
//!
//! A young crater's ejecta are fresher than the ground about it. Mixed with the local ground in a
//! fresh share s, both deficits from the background decaying as e^(−k t ÷ `τ_L`) under the
//! first-order law, a crater of age `t_c` leaves the maturity
//!
//! `m_local` = `m_body` − s × max(0, `m_body` − `maturity_after`(`t_c`, k))
//!
//! ([`crater_maturity`]; with s = 1 it is min(`m_body`, `maturity_after`(`t_c`, k))), and where
//! several reach a point the largest deficit holds, `m_local` = `m_body` − maxᵢ sᵢ × max(0,
//! `m_body` − `m_c,i`), which is order-free, so deterministic without a sort; exact compositing is
//! recorded, not built ([`maturity_at`]). The ages are the synthesis's own ([`SmallCrater::age`],
//! [`CoarseCrater::age`](crate::field::CoarseCrater::age)), so nothing new is drawn. A crater's
//! fresh share is s = g `f_ex` p(x), in [0, 1]:
//!
//! - **The halo p(x)** ([`halo_share`]), x the chord from the crater's centre in rim radii, as the
//!   synthesis measures it. Grier, McEwen, Milazzo, Hester and Lucey 2000 (LPSC XXXI, abstract
//!   1950, Fig. 3, whose axis is in crater diameters) radially average the optical maturity (OMAT,
//!   by Lucey et al.'s technique, from Clementine's images) about North Ray and South Ray, 1 and
//!   0.8 km across and 50 and 2 Ma old. The excess over the background, as a share of the rim's, is
//!   North Ray's 0.60–0.62 at 2 R, 0.25–0.30 at 4 R, 0.14–0.15 at 6 R and 0.085–0.095 at 8 R (the
//!   ruling's ranges; South Ray's, read by this lane's science check, 0.68, 0.24, 0.13 and 0.08),
//!   reaching the background by about 10–14 R. λ = 3.0 R is the ruling's value, which it gives as a
//!   least-squares fit of exp(−(x − 1) ÷ λ); the science check's fits to those points and to both
//!   profiles give 2.3–2.8 R (recorded, plan R10's Risks). p is 1 inside the rim, x ≤ 1, and for a
//!   small crater (e^(−(x − 1) ÷ 3) − e^(−3)) ÷ (1 − e^(−3)) out to 10 R
//!   ([`HALO_LENGTH_RIM_RADII`], [`HALO_EXTENT_RIM_RADII`]), 0 beyond. A coarse crater's is shifted
//!   to 0 at its reach (1 + a) R instead, since the field carries a coarse crater only on the cells
//!   its reach touches, and an answer must not depend on which blocks a client holds. Labelled:
//!   λ = 3 R and 10 R are two lunar 1 km craters', taken for every size and body, and the OMAT
//!   excess stands in for the fresh share. For context, the continuous ejecta reach `R_ce` =
//!   2.348 R^1.006 km (Moore, Hodges and Scott 1974, Proc. LSC 5th, 71, eq. 1, p. 83, fitted to
//!   craters of 0.65–218 km), the model's reach, and new 18–70 m craters' proximal zones "fall
//!   within about five crater radii of the impact" (Speyerer et al. 2026, PSJ 7, 73, p. 7; the
//!   ruling's p. 9). Rays are not modelled.
//! - **The excavated share `f_ex`** ([`excavated_share`]): only ejecta dug from below the mature
//!   layer is fresh. `f_ex` = (1 − q)² for q < 1, else 0, with q = `d_m` ÷ (0.084 D) and `d_m` the
//!   mature layer the surface had when the crater formed ([`reworked_depth`]): Morris 1978's
//!   in-situ reworking depth, 2.2 cm × t^0.45 with t in Myr, at a 50% probability (Proc. LPSC 9,
//!   1801, eq. 1, p. 1807, fitted over about 1 to 500 Myr and extrapolated beyond it here), over
//!   the surface's exposure then, `t_s` − `t_c`. 0.084 D is a simple crater's maximum excavation
//!   depth (Melosh 1989, as Speyerer et al. 2026 take it, p. 15; [`EXCAVATION_DEPTH_RATIO`]), taken
//!   for the complex coarse craters too, where q is tiny and `f_ex` about 1; and (1 − q)² the share
//!   of a paraboloid excavation cavity's volume below `d_m` (derived in the ruling, labelled). The
//!   regolith is mature "down to several tens of cm and immature below one meter" (Robinson et al.
//!   2015, Icarus 252, 229, as the ruling quotes it, not read). On 3.5 Gyr ground, where `d_m` is
//!   0.87 m, `f_ex` is 0 below 10.3 m, 0.18 at 18 m, 0.63 at 50 m, 0.80 at 100 m and 0.98 at 1 km;
//!   on 4.4 Gyr ground (0.96 m) 0 below 11.4 m, then 0.13, 0.60, 0.78 and 0.98. It is 1 for a
//!   crater as old as its surface.
//! - **The wear g**: a crater's ejecta fade as its relief does, by the same gardening (Grier et al.
//!   2000, p. 1: "the ejecta of small craters appears to mature faster than that of larger
//!   craters"). g is the crater's kept relief exactly as the synthesis computes it: a small
//!   crater's [`relief_factor`] of its rim radius and diffusion length, a coarse crater's
//!   [`kept_relief`](crate::synth::craters::rims::kept_relief), one less its degradation. Tycho
//!   keeps g ≈ 1 (κt ÷ R² ≈ 3 × 10⁻⁷), so `τ_L`'s calibration is untouched; a 25 m crater keeps 21%
//!   of its relief after 5 Myr.
//!
//! The halo, the excavation and the wear together keep old, small craters from leaving most of
//! the ground immature: on a 3.5 Gyr Moon-like surface, the ruling's estimates of the ground whose
//! deficit `m_body` − `m_local` is above 0.2 and above 0.5 are 79% and 57% for every crater from
//! 2 m with the 2.54 R profile, 67% and 7.6% with the halo and `f_ex`, and 16% and 2.1% with g too,
//! the rule built.
//!
//! # The search
//!
//! A coarse crater reaches a point only from the coarse cell that holds it, whose index lists every
//! crater whose reach touches the cell ([`FieldView::craters_reaching`]), so the coarse cell is
//! read, and refused where the view does not hold it, as the height refuses it. A small crater's
//! halo reaches 10 of its rim radii, beyond its reach of at most 2.543
//! ([`MAX_EXTERIOR_WIDTH`](crate::synth::craters::MAX_EXTERIOR_WIDTH)), so an octave's search
//! takes the 3 × 3 neighbourhood, across face edges and without repeats, of the point's cell two
//! levels above the octave's, and reads each of its cells' 16 descendants at the octave's level
//! ([`SmallCraters::craters_in`]): the octave's own level holds 2.543 of its largest rim radii
//! within 0.98 of its narrowest width, and the width doubles a level, so the cell two levels up
//! holds 10 of them. An octave on level 0 or 1 is searched whole. An octave none of whose craters
//! can expose fresh ground is not searched: one that draws none, and one where `d_m`(`t_s` − τ) ≥
//! 0.084 times its largest diameter, τ its window of ages, since `d_m` is least for its oldest
//! crater. Nor is a cell drawn that provably lies beyond the halo's reach from the point (by its
//! corners' angles, `is_beyond`): a cold query's craters are most of its cost, about 400 drawn an
//! octave for some 8 within reach, and the pass-by cut a lone query on the weathered Moon-like
//! world from about 44 ms to 5 ms (provisional, a shared laptop). Neither moves a value. The small
//! craters are read through the caller's cache, which draws a cell from its own stream as the
//! height does, and no answer depends on what it holds.
//!
//! # Determinism
//!
//! Pure `f64` arithmetic and `hyperion_base::math` ([`math::exp_m1`], [`math::powf`]); no draw, so
//! no domain tag. The result at zero exposure or zero rate is +0, never −0, as the field header's
//! reserved `maturity` part requires. Every comparison is explicit, never `f64::min` or `max`, and
//! the deficits are compared, never summed.
//!
//! # Not on the record yet
//!
//! Nothing on the record calls this module's time law at generator version 22: P14.T54.b (23 → 24)
//! fills the header's `maturity` from P14.T51.d's `hooks/modifiers.rs`, which calls it, and deletes
//! the sim's tripwire (`modifiers_nothing_on_the_record_reads_the_modifiers_yet`) that holds the
//! line. No header outside the `testing` builder carries a maturity, a rate or a cover before it,
//! so every exposure the server and the worker take is [`GroundExposure::NONE`]'s, and every albedo
//! and answer is as it was. The point maturity is presentation (plan R10's Generator version); the
//! solve's per-form rule (`Melt`, the cover) reaches the header's generator-versioned
//! `albedo_scale` at 24.

use hyperion_base::math;
use hyperion_base::units::{Gigayears, Metres};

use super::classify::organic_cover_entry;
use super::weathering::check_unit;
use super::{
    Material, MaterialClass, MaterialProperties, ResolvePropertiesError, properties,
    weathered_entry_bvr,
};
use crate::craters::crater_shape;
use crate::cube::{Edge, FaceUv, PatchKey, face_uv_to_xyz, st_to_uv, unit_dir};
use crate::field::{
    FieldHeader, FieldView, MaterialPalette, PaletteEntry, PaletteRole, cell_at_index,
};
use crate::substance_key::{SubstanceKey, SubstanceKeyForm};
use crate::synth::craters::rims::kept_relief;
use crate::synth::craters::{CraterCache, CraterOctave, SmallCrater, relief_factor};
use crate::synth::{QueryHeightError, SmallCraters, SynthCache, Synthesiser};

/// The lunar maturation time `τ_L`: 0.3277 Gyr, the time over which the Moon's ground matures by a
/// factor of e towards its mature background at the Moon's weathering rate (k = 1).
///
/// **The fit** (decision-p14-t51d-weathering §2.1). Grier et al. 2001 (JGR 106, 32847, abstract
/// read) class lunar rays by Clementine's optical-maturity parameter into craters as young as or
/// younger than Tycho, "inferred age of ~109 Myr", whose ejecta are immature, and craters older
/// than Copernicus, "~810 Myr", whose ejecta are mature. In m's terms, with X the lunar mature
/// background's Morris index `I_s/FeO` and Morris's classes, immature 0–29.0 and mature from 60.0
/// (Morris 1976, Proc. LPSC 7, 315, abstract read, which defines the index; Morris 1978, Proc.
/// LPSC 9, 2287, pp. 2289–2290, read):
///
/// - m(109 Myr) < 30 ÷ X, so `τ_L` > 0.109 Gyr ÷ −ln(1 − 30 ÷ X);
/// - m(810 Myr) > 60 ÷ X, so `τ_L` < 0.810 Gyr ÷ −ln(1 − 60 ÷ X).
///
/// At X = 75, a stand-in inside the mature class's 60 to above 100 (Morris 1978, pp. 2289–2290,
/// read), the bounds are 0.4 and 0.8 and give `τ_L` in
/// (0.2134, 0.5033) Gyr. `τ_L` is their geometric centre, √(0.2134 × 0.5033) = 0.3277 Gyr, which
/// keeps the same ratio margin, 1.536, on both sides: m(109 Myr) = 0.283 and m(810 Myr) = 0.916.
/// The fit holds both bounds for any X from about 66 to 106, outside which the Copernicus bound
/// (below) or Tycho's (above) passes it; the module's tests hold it at X = 75.
pub const LUNAR_MATURATION_TIME: Gigayears = Gigayears::new(0.3277);

/// The maturity, dimensionless in [+0, 1], of ground exposed for `exposure` at the weathering rate
/// `weathering_rate` (k, dimensionless, the Moon's 1): 1 − exp(−k t ÷ `τ_L`) (the `maturity` module
/// documentation), with `τ_L` [`LUNAR_MATURATION_TIME`].
///
/// It is +0 at zero exposure or zero rate (never −0, whatever the sign of a zero exposure), never
/// falls as either grows, is below 1 while k t ÷ `τ_L` is under about 36.7, where `f64` can hold
/// 1 − exp(−x) apart from 1, and is exactly 1 beyond. It is written as −expm1(−x), which keeps a
/// young surface's small maturity to full precision.
///
/// # Panics
///
/// If the exposure or the rate is not finite and non-negative: the sim validates both before the
/// call, so a NaN or a negative value here is a bug.
///
/// # Examples
///
/// Tycho's ejecta, 109 Myr old, are immature, and Copernicus's, 810 Myr old, near the background;
/// a body weathered twice as fast matures in half the time.
///
/// ```
/// use hyperion_base::units::Gigayears;
/// use hyperion_surface::material::maturity_after;
///
/// assert!(maturity_after(Gigayears::new(0.109), 1.0) < 0.4);
/// assert!(maturity_after(Gigayears::new(0.810), 1.0) > 0.8);
/// let twice = maturity_after(Gigayears::new(0.05), 2.0);
/// assert!((twice - maturity_after(Gigayears::new(0.1), 1.0)).abs() < 1e-15);
/// assert!(maturity_after(Gigayears::ZERO, 1.0).is_sign_positive());
/// ```
#[must_use]
pub fn maturity_after(exposure: Gigayears, weathering_rate: f64) -> f64 {
    let t = exposure.value();
    assert!(
        t.is_finite() && t >= 0.0,
        "an exposure must be finite and non-negative, got {t} Gyr"
    );
    assert!(
        weathering_rate.is_finite() && weathering_rate >= 0.0,
        "a weathering rate must be finite and non-negative, got {weathering_rate}"
    );
    let x = weathering_rate * t / LUNAR_MATURATION_TIME.value();
    // A zero exposure of either sign, or a zero rate, is fresh ground: +0, which −expm1(−0) would
    // give as −0 for a negative zero.
    if x > 0.0 { -math::exp_m1(-x) } else { 0.0 }
}

/// A body's background maturity, dimensionless in [+0, 1], at its surface age `surface_age`
/// (P14.T24.b's, which already carries its resurfacing) and weathering rate `weathering_rate`:
/// [`maturity_after`] of the surface age (decision-p14-t51d-weathering §2.1).
///
/// A later steady-state factor, the balance of weathering against gardening, would multiply it;
/// that is a value change, and R10 never recomputes it.
///
/// # Panics
///
/// As [`maturity_after`].
#[must_use]
pub fn body_maturity(surface_age: Gigayears, weathering_rate: f64) -> f64 {
    maturity_after(surface_age, weathering_rate)
}

/// The e-folding length of a young crater's fresh halo, rim radii: 3, the ruling's value
/// (decision-r10-t1c-maturity R4), given there as a least-squares fit of exp(−(x − 1) ÷ λ) to
/// Grier et al. 2000's radially averaged optical maturity about North Ray (LPSC XXXI, abstract
/// 1950, Fig. 3).
///
/// Labelled: two lunar 1 km craters' figure, taken for every size and body. This lane's science
/// check fits 2.3–2.8 R to the same points and to both craters' profiles, not 3 (recorded, plan
/// R10's Risks); the ruled value is kept.
pub const HALO_LENGTH_RIM_RADII: f64 = 3.0;

/// How far a small crater's fresh halo reaches, rim radii: 10, where Grier et al. 2000's profiles
/// reach the background, "by about 10–14 R" (Fig. 3, as the ruling reads it; North Ray's excess is
/// about 0.025 of its rim's at 10 R), [`halo_share`]'s 0.
///
/// Labelled, as [`HALO_LENGTH_RIM_RADII`] is. A coarse crater's halo ends at its own reach
/// instead, (1 + a) rim radii, the farthest the field carries it.
pub const HALO_EXTENT_RIM_RADII: f64 = 10.0;

/// A simple crater's maximum excavation depth over its diameter: 0.084 (Melosh 1989, *Impact
/// Cratering*, as Speyerer et al. 2026, PSJ 7, 73, p. 15, take it: "excavate and eject material
/// from a maximum depth around 0.084D"; decision-r10-t1c-maturity R4). Taken for every crater: on
/// a complex one q is tiny and the excavated share about 1.
pub const EXCAVATION_DEPTH_RATIO: f64 = 0.084;

/// The depth the lunar surface's in-situ reworking matures in 1 Myr: 2.2 cm (Morris 1978, Proc.
/// LPSC 9, 1801, eq. 1, p. 1807: the reworking depth, at a 50% probability, 2.2 cm × t^0.45, t in
/// Myr, "only over the ~1.0 to 500 m.y. range of the data", extrapolated beyond it here;
/// decision-r10-t1c-maturity R4).
pub const REWORKING_DEPTH_AT_ONE_MYR: Metres = Metres::new(0.022);

/// The exponent of the reworking depth's growth with the surface's exposure: 0.45 (Morris 1978, as
/// [`REWORKING_DEPTH_AT_ONE_MYR`]).
pub const REWORKING_EXPONENT: f64 = 0.45;

/// The levels above an octave's own at which its halo search's neighbourhood is taken: 2, whose
/// cells are four times as wide (the module documentation's search).
const SEARCH_LEVELS_UP: u8 = 2;

/// The `N2` key, the nitrogen ice that takes the organic cover beside a carbon ice.
const NITROGEN: SubstanceKey = SubstanceKey::new_const("N2");

/// The `CO2` key, the carbon-bearing ice that takes no organic cover (P14.T51.d's
/// `organic_ground`).
const CARBON_DIOXIDE: SubstanceKey = SubstanceKey::new_const("CO2");

/// What the ground at a place is exposed to, as R10.T1.c weathers and covers its albedo
/// ([`exposed_properties`]): a space-weathering maturity and an organic cover, each a fraction in
/// [0, 1] or absent (decision-r10-t1c-maturity R2 and R3).
///
/// [`NONE`](Self::NONE), with neither, is the identity: the materials' albedos as resolved, fresh
/// and uncovered. A header's ([`of_header`](Self::of_header)) carries its `m_body` and cover; a
/// point's ([`with_maturity`](Self::with_maturity)) its `m_local` ([`maturity_at`]). Each part is
/// set by name ([`with_maturity`](Self::with_maturity),
/// [`with_organic_cover`](Self::with_organic_cover)), so that the two fractions cannot be swapped.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct GroundExposure {
    maturity: Option<f64>,
    organic_cover: Option<f64>,
}

impl GroundExposure {
    /// No maturity and no cover: [`exposed_properties`] is [`properties()`](super::properties())
    /// to the bit.
    pub const NONE: Self = Self {
        maturity: None,
        organic_cover: None,
    };

    /// The exposure of a body's ground as its field header gives it: the background maturity
    /// `m_body` ([`FieldHeader::maturity`]) and the organic cover
    /// ([`FieldHeader::organic_cover`]), each absent until P14.T54.b fills it.
    ///
    /// # Panics
    ///
    /// Never for a header, which validates both parts as fractions in [0, 1].
    #[must_use]
    pub fn of_header(header: &FieldHeader) -> Self {
        let exposure = header
            .maturity()
            .map_or(Self::NONE, |m| Self::NONE.with_maturity(m));
        header
            .organic_cover()
            .map_or(exposure, |c| exposure.with_organic_cover(c))
    }

    /// The same exposure at the maturity `maturity`, dimensionless: a point's `m_local`
    /// ([`maturity_at`]) or a body's `m_body`, the cover kept.
    ///
    /// # Panics
    ///
    /// If `maturity` is outside [0, 1] or NaN: the header validates its maturity first, so such a
    /// value here is a bug, as it is for the law's inputs.
    #[must_use]
    pub fn with_maturity(self, maturity: f64) -> Self {
        check_unit("an exposure's maturity", maturity);
        Self {
            maturity: Some(maturity),
            ..self
        }
    }

    /// The same exposure under the organic cover `organic_cover`, the covered fraction of the
    /// ground that can take one, dimensionless, the maturity kept.
    ///
    /// # Panics
    ///
    /// If `organic_cover` is outside [0, 1] or NaN, as [`with_maturity`](Self::with_maturity).
    #[must_use]
    pub fn with_organic_cover(self, organic_cover: f64) -> Self {
        check_unit("an exposure's organic cover", organic_cover);
        Self {
            organic_cover: Some(organic_cover),
            ..self
        }
    }

    /// The maturity, dimensionless in [0, 1], or `None` where the ground's is not modelled.
    #[must_use]
    pub const fn maturity(&self) -> Option<f64> {
        self.maturity
    }

    /// The organic cover, the covered fraction of the ground that can take one, dimensionless in
    /// [0, 1], or `None` where it is not modelled.
    #[must_use]
    pub const fn organic_cover(&self) -> Option<f64> {
        self.organic_cover
    }
}

/// Whether the palette entry `entry` of `palette` takes the body's organic cover: the ground that
/// can take one, P14.T51.d's `organic_ground` on the palette's keys (decision-r10-t1c-maturity R3;
/// the module documentation).
///
/// It does where `entry` is an [`Ice`](PaletteRole::Ice)-role entry whose key is a formula
/// ([`SubstanceKeyForm::Formula`]) holding carbon, `CO2` excepted (`CH4`, `CO`, `C2H6`, `CH3OH`,
/// `HCN`; a `C` followed by a lowercase letter is another element, as `CaSO4`'s), or the `Ice`-role
/// `N2` where `palette` holds an entry of the first kind. The seasonal ices, which
/// `organic_ground` drops, cannot be told on the palette, so they take it too (recorded).
#[must_use]
pub fn takes_organic_cover(palette: &MaterialPalette, entry: &PaletteEntry) -> bool {
    is_carbon_ice(entry)
        || (entry.role == PaletteRole::Ice
            && entry.substance == NITROGEN
            && palette.entries().iter().any(is_carbon_ice))
}

/// Whether `entry` is an `Ice`-role entry whose formula holds carbon, `CO2` excepted.
#[must_use]
fn is_carbon_ice(entry: &PaletteEntry) -> bool {
    entry.role == PaletteRole::Ice
        && entry.substance.form() == SubstanceKeyForm::Formula
        && entry.substance != CARBON_DIOXIDE
        && holds_carbon(entry.substance.as_str())
}

/// Whether the formula `formula` names carbon: an element symbol `C` not followed by a lowercase
/// letter, which would make it `Ca`, `Cl`, `Co`, `Cs` or another.
#[must_use]
fn holds_carbon(formula: &str) -> bool {
    let bytes = formula.as_bytes();
    bytes
        .iter()
        .enumerate()
        .any(|(at, &byte)| byte == b'C' && !bytes.get(at + 1).is_some_and(u8::is_ascii_lowercase))
}

/// The properties of `material` on a body of palette `palette` with its albedo as ground of
/// exposure `exposure` shows it: [`properties()`](super::properties()) with each band's `A_N`
/// weathered to the exposure's maturity ([`weathered_entry_bvr`]), times the form's factor, and
/// blended with the organic cover where the entry takes one (decision-r10-t1c-maturity R2 and R3;
/// the module documentation).
///
/// The law, its origin and every other property are `properties`' own. `Melt` keeps its fresh
/// albedo, a molten surface holding no npFe⁰-bearing regolith; every other form weathers. The
/// cover blend, (1 − c) A + c `A_cover`, takes the palette's first organic `Cover` entry's `A_N`
/// weathered to the same maturity, without a form factor; it is skipped where the cover is absent
/// or 0, where `material`'s entry takes none ([`takes_organic_cover`]) and where the palette has no
/// cover entry. With neither a maturity nor a cover blended the result is `properties`' to the bit.
///
/// # Errors
///
/// [`ResolvePropertiesError::EntryNotInPalette`] for an entry past the palette, as
/// [`properties()`](super::properties()).
///
/// # Panics
///
/// As [`weathered_entry_bvr`], for an entry whose values a [`MaterialPalette`] refuses (a bug).
pub fn exposed_properties(
    material: Material,
    palette: &MaterialPalette,
    exposure: GroundExposure,
) -> Result<MaterialProperties, ResolvePropertiesError> {
    let fresh = properties(material, palette)?;
    let maturity = if material.class() == MaterialClass::Melt {
        None
    } else {
        exposure.maturity
    };
    let entry = palette
        .get(material.entry())
        .expect("`properties` resolved the entry");
    let cover = match exposure.organic_cover {
        Some(c) if c > 0.0 && takes_organic_cover(palette, entry) => organic_cover_entry(palette)
            .map(|k| {
                let cover = palette.get(k).expect("the cover's index is the palette's");
                (c, cover)
            }),
        Some(_) | None => None,
    };
    if maturity.is_none() && cover.is_none() {
        return Ok(fresh);
    }
    let exposed =
        |e: &PaletteEntry| maturity.map_or(e.normal_albedo_bvr, |m| weathered_entry_bvr(e, m));
    let factor = fresh.form().albedo_factor().unwrap_or(1.0);
    let mut bvr = exposed(entry).map(|a| a * factor);
    if let Some((c, cover)) = cover {
        for (a, covering) in bvr.iter_mut().zip(exposed(cover)) {
            *a = (1.0 - c) * *a + c * covering;
        }
    }
    Ok(fresh.with_normal_albedo_bvr(bvr))
}

/// The reworked depth `d_m` of a surface exposed for `exposure`, metres: Morris 1978's in-situ
/// reworking, 2.2 cm × t^0.45 with t in Myr ([`REWORKING_DEPTH_AT_ONE_MYR`],
/// [`REWORKING_EXPONENT`]), the mature layer it holds (decision-r10-t1c-maturity R4). Zero for no
/// exposure, or a negative one.
///
/// Labelled: the depth by surface age alone, the Moon's, not by the body's own impact flux.
///
/// # Panics
///
/// If `exposure` is not finite.
#[must_use]
pub fn reworked_depth(exposure: Gigayears) -> Metres {
    let t = exposure.value();
    assert!(t.is_finite(), "an exposure must be finite, got {t} Gyr");
    let myr = t * 1e3;
    if myr > 0.0 {
        REWORKING_DEPTH_AT_ONE_MYR * math::powf(myr, REWORKING_EXPONENT)
    } else {
        Metres::ZERO
    }
}

/// The share `f_ex` of a crater's ejecta dug from below the mature layer, dimensionless in [0, 1]:
/// (1 − q)² for q < 1, else 0, with q = `d_m` ÷ (0.084 D), `d_m` the [`reworked_depth`] of the
/// surface's exposure when the crater formed, `surface_age` − `crater_age`, and D the crater's
/// `diameter` (decision-r10-t1c-maturity R4; the module documentation).
///
/// (1 − q)² is the share of a paraboloid excavation cavity's volume below `d_m` (derived in the
/// ruling, labelled). It is 1 for a crater as old as its surface, or older (no mature layer), and
/// 0 where the excavation depth, [`EXCAVATION_DEPTH_RATIO`] D, is no deeper than `d_m`.
///
/// # Panics
///
/// If either age is not finite and non-negative or the diameter is not finite and positive.
#[must_use]
pub fn excavated_share(surface_age: Gigayears, crater_age: Gigayears, diameter: Metres) -> f64 {
    let (t_s, t_c, d) = (surface_age.value(), crater_age.value(), diameter.value());
    assert!(
        t_s.is_finite() && t_s >= 0.0 && t_c.is_finite() && t_c >= 0.0,
        "a surface's and a crater's ages must be finite and non-negative, got {t_s} and {t_c} Gyr"
    );
    assert!(
        d.is_finite() && d > 0.0,
        "a crater's diameter must be finite and positive, got {d} m"
    );
    let q = reworked_depth(Gigayears::new(t_s - t_c)).value() / (EXCAVATION_DEPTH_RATIO * d);
    if q < 1.0 {
        let deeper = 1.0 - q;
        deeper * deeper
    } else {
        0.0
    }
}

/// The share p of a crater's fresh ejecta at `distance_rim_radii` from its centre, dimensionless
/// in [0, 1], for a halo that ends at `outer_rim_radii`: 1 inside the rim, x ≤ 1, then
/// (e^(−(x − 1) ÷ λ) − e^(−(x₁ − 1) ÷ λ)) ÷ (1 − e^(−(x₁ − 1) ÷ λ)) with λ
/// [`HALO_LENGTH_RIM_RADII`] and x₁ the halo's end, and 0 from x₁ (decision-r10-t1c-maturity R4).
///
/// A small crater's halo ends at [`HALO_EXTENT_RIM_RADII`], 10, and a coarse crater's at its reach,
/// 1 + a. The exponential is Grier et al. 2000's OMAT profile shifted to reach 0 at the end, so
/// that the share is continuous at both ends; labelled, as the constants are.
///
/// # Panics
///
/// If the distance is negative or NaN, or the end is not finite or is below 1.
#[must_use]
pub fn halo_share(distance_rim_radii: f64, outer_rim_radii: f64) -> f64 {
    let (x, end) = (distance_rim_radii, outer_rim_radii);
    assert!(
        x >= 0.0,
        "a distance must be non-negative, got {x} rim radii"
    );
    assert!(
        end.is_finite() && end >= 1.0,
        "a halo must end finite and at or beyond the rim, got {end} rim radii"
    );
    if x <= 1.0 {
        return 1.0;
    }
    if x >= end {
        return 0.0;
    }
    // Both terms less one, by expm1, so that a halo ending near the rim keeps its precision; the
    // quotient's rounding may not carry it past either end of [0, 1].
    let tail = math::exp_m1(-(end - 1.0) / HALO_LENGTH_RIM_RADII);
    let here = math::exp_m1(-(x - 1.0) / HALO_LENGTH_RIM_RADII);
    ((here - tail) / -tail).clamp(0.0, 1.0)
}

/// The maturity a crater of age `crater_age` leaves at a point where its fresh share is
/// `fresh_share`, on ground of background maturity `body_maturity` weathered at the rate
/// `weathering_rate` (k): `m_body` − s × max(0, `m_body` − [`maturity_after`](`t_c`, k)),
/// dimensionless in [0, `m_body`] (decision-r10-t1c-maturity R4: one crater).
///
/// Exact under the first-order time law for ejecta of share s mixed with local ground, since both
/// deficits from the background decay as e^(−k t ÷ `τ_L`). With s = 1 it is min(`m_body`,
/// `maturity_after`(`t_c`, k)); with s = 0, or a crater no fresher than the ground, `m_body`
/// itself. The share is g `f_ex` p(x) ([`relief_factor`] or
/// [`kept_relief`](crate::synth::craters::rims::kept_relief), [`excavated_share`],
/// [`halo_share`]); [`maturity_at`] takes the largest deficit over the craters that reach a point.
///
/// # Panics
///
/// If `body_maturity` or `fresh_share` is outside [0, 1] or NaN, or as [`maturity_after`].
#[must_use]
pub fn crater_maturity(
    body_maturity: f64,
    crater_age: Gigayears,
    weathering_rate: f64,
    fresh_share: f64,
) -> f64 {
    check_unit("a body's maturity", body_maturity);
    check_unit("a crater's fresh share", fresh_share);
    let deficit = ejecta_deficit(
        body_maturity,
        maturity_after(crater_age, weathering_rate),
        fresh_share,
    );
    less(body_maturity, deficit)
}

/// The deficit a crater's ejecta of maturity `ejecta` leave below the background `body` where its
/// fresh share is `share`: s × max(0, `m_body` − `m_c`), by an explicit comparison.
#[must_use]
fn ejecta_deficit(body: f64, ejecta: f64, share: f64) -> f64 {
    if body > ejecta {
        share * (body - ejecta)
    } else {
        0.0
    }
}

/// `body` less the largest deficit `deficit`: `body` itself, to the bit, where there is none.
#[must_use]
fn less(body: f64, deficit: f64) -> f64 {
    if deficit > 0.0 { body - deficit } else { body }
}

/// The space-weathering maturity at `at`, `m_local`, dimensionless in [0, `m_body`]: the header's
/// `m_body` less the largest deficit the young craters that reach the point leave there, each
/// [`crater_maturity`]'s at its fresh share g `f_ex` p(x), or `None` where the header carries no
/// maturity (decision-r10-t1c-maturity R4; the module documentation).
///
/// The craters are the coarse ones the field's index lists for the coarse cell that holds the
/// point, each with its halo ending at its reach (1 + a) R and g its kept relief, and the small
/// craters of every octave that can expose fresh ground, each with its halo ending at 10 R and g
/// its [`relief_factor`], all at their synthesis ages; where none reaches the point it is `m_body`
/// to the bit. `cache` is the caller's, and the answer does not depend on what it holds. It is
/// presentation (plan R10's Generator version): the client half bakes it per texel (R10.T1.c's
/// split), and the worker's point query reads its material's albedo at it.
///
/// # Errors
///
/// [`QueryHeightError::NotSurveyed`] naming the coarse cell holding the point where the view does
/// not hold it, as the height refuses it.
///
/// # Panics
///
/// If `at` names no direction (a (u, v) that is not finite); and as [`SmallCraters::craters_in`],
/// if a crater drawn has an exterior wider than its search's bound (a bug).
pub fn maturity_at<F: FieldView>(
    synth: &Synthesiser<'_, F>,
    cache: &mut SynthCache,
    at: FaceUv,
) -> Result<Option<f64>, QueryHeightError> {
    let field = synth.field();
    let header = field.header();
    let (Some(body), Some(rate)) = (header.maturity(), header.weathering_rate()) else {
        return Ok(None);
    };
    let dir = unit_dir(face_uv_to_xyz(at));
    let figure = header.figure();
    let point = figure.point(dir);
    let surface_age = header.surface_age();
    let cell = PatchKey::containing(header.level().get(), dir)
        .expect("a coarse level is a quadtree level");
    if field.cell(cell).is_none() {
        return Err(QueryHeightError::NotSurveyed(cell));
    }
    let mut largest = 0.0;
    let mut take = |deficit: f64| {
        if deficit > largest {
            largest = deficit;
        }
    };
    for crater in field.craters_reaching(cell) {
        let shape = crater_shape(header.craters(), crater.diameter);
        let rim_m = 0.5 * shape.diameter().value();
        let x = chord(point, figure.point(crater.centre)) / rim_m;
        let halo = halo_share(x, 1.0 + shape.exterior_width());
        if halo > 0.0 {
            let fresh = kept_relief(crater)
                * excavated_share(surface_age, crater.age, crater.diameter)
                * halo;
            take(deficit_of(body, crater.age, rate, fresh));
        }
    }
    let craters = synth.craters();
    let smallest_radius_m = if figure.polar_radius_m < figure.equatorial_radius_m {
        figure.polar_radius_m
    } else {
        figure.equatorial_radius_m
    };
    for octave in craters.octaves().octaves() {
        if !can_expose(octave, surface_age) {
            continue;
        }
        each_small_crater_near(
            craters,
            octave,
            (dir, point),
            smallest_radius_m,
            cache.craters_mut(),
            |crater, x| {
                let share = relief_factor(crater.diameter() * 0.5, crater.diffusion())
                    * excavated_share(surface_age, crater.age(), crater.diameter())
                    * halo_share(x, HALO_EXTENT_RIM_RADII);
                take(deficit_of(body, crater.age(), rate, share));
            },
        );
    }
    Ok(Some(less(body, largest)))
}

/// The deficit a crater of age `age` leaves below `body` at fresh share `share` and rate `rate`,
/// the ejecta's maturity computed only where the share is positive.
#[must_use]
fn deficit_of(body: f64, age: Gigayears, rate: f64, share: f64) -> f64 {
    if share > 0.0 {
        ejecta_deficit(body, maturity_after(age, rate), share)
    } else {
        0.0
    }
}

/// Whether some crater of `octave` can expose fresh ground on a surface `surface_age` old: it draws
/// craters at all, and its oldest, at its window's age, has the least reworked depth and its
/// largest diameter the deepest excavation, so where their [`excavated_share`] is 0 every crater's
/// is. Where it does not, its search would find no deficit, so passing it by moves no value.
#[must_use]
fn can_expose(octave: &CraterOctave, surface_age: Gigayears) -> bool {
    octave.drawn_density().value() > 0.0
        && excavated_share(surface_age, octave.window(), octave.diameters().upper()) > 0.0
}

/// The chord between the spheroid points `a` and `b`, metres.
#[must_use]
fn chord(a: [f64; 3], b: [f64; 3]) -> f64 {
    squared_chord(a, b).sqrt()
}

/// The square of the chord between `a` and `b`, square metres.
#[must_use]
fn squared_chord(a: [f64; 3], b: [f64; 3]) -> f64 {
    let d = [0, 1, 2].map(|k| a[k] - b[k]);
    d[0] * d[0] + d[1] * d[1] + d[2] * d[2]
}

/// Calls `emit` with each crater of `octave` drawn by `craters` whose halo, 10 rim radii, holds the
/// spheroid point `point` of the unit direction `dir`, and its distance from it in rim radii (the
/// module documentation's search), in the search's order, on a figure whose smallest radius is
/// `smallest_radius_m`: the cells two levels up's 3 × 3 neighbourhood's descendants, each read
/// through `cache` unless it lies beyond the octave's largest halo ([`is_beyond`]).
fn each_small_crater_near(
    craters: &SmallCraters,
    octave: &CraterOctave,
    (dir, point): ([f64; 3], [f64; 3]),
    smallest_radius_m: f64,
    cache: &mut CraterCache,
    mut emit: impl FnMut(&SmallCrater, f64),
) {
    let (index, level) = (octave.index(), octave.level());
    let reach_rad = reach_angle(
        HALO_EXTENT_RIM_RADII * 0.5 * octave.diameters().upper().value(),
        smallest_radius_m,
    );
    let mut visit = |cell: PatchKey, cache: &mut CraterCache| {
        if is_beyond(cell, dir, reach_rad) {
            return;
        }
        for crater in craters.craters_in(index, cell, cache) {
            let rim_m = 0.5 * crater.diameter().value();
            let halo_m = HALO_EXTENT_RIM_RADII * rim_m;
            let d2 = squared_chord(point, crater.point_m());
            if d2 < halo_m * halo_m {
                emit(crater, d2.sqrt() / rim_m);
            }
        }
    };
    if level < SEARCH_LEVELS_UP {
        let count = 6_u32 << (2 * u32::from(level));
        for n in 0..count {
            visit(
                cell_at_index(level, n).expect("an index below the level's count is a cell"),
                cache,
            );
        }
        return;
    }
    let home = PatchKey::containing(level - SEARCH_LEVELS_UP, dir)
        .expect("a level above an octave's is a quadtree level");
    let (cells, n) = neighbourhood(home);
    let side = 1_u32 << SEARCH_LEVELS_UP;
    for parent in &cells[..n] {
        for j in 0..side {
            for i in 0..side {
                let cell = PatchKey::new(
                    parent.face(),
                    level,
                    side * parent.i() + i,
                    side * parent.j() + j,
                )
                .expect("a descendant of a cell is a cell");
                visit(cell, cache);
            }
        }
    }
}

/// The relative margin by which a cell must lie beyond a halo's reach before the search passes it
/// by ([`is_beyond`]): 10⁻⁹, far above the rounding of the bound's few operations, so that no
/// crater within the reach is ever passed by.
const BEYOND_MARGIN: f64 = 1e-9;

/// The absolute margin on the reach's angle, radians: 10⁻¹², a thousand times the rounding of the
/// angles the bound compares (about 10⁻¹⁵ radians each), which on a large body exceeds the relative
/// margin's share of a small halo; a cell at the finest level is some 10⁻⁷ radians wide, so it
/// passes nothing more by.
const BEYOND_FLOOR_RAD: f64 = 1e-12;

/// A bound on the angle from a cell's centre to any point of it, radians, times 2ⁿ at level n:
/// 1.75, above S2's `kMaxEdge` derivative for the quadratic projection, 1.704897
/// (`s2metrics.cc`), the most a unit direction turns per unit of s or of t, since a point of the
/// cell is at most half its side away in each ([`is_beyond`]'s shortcut).
const CELL_SPREAD_PER_LEVEL: f64 = 1.75;

/// The angle from the unit direction `dir` beyond which a crater centre's spheroid point lies more
/// than `reach_m` metres, as a chord, from the query's, on a figure whose smallest radius is
/// `smallest_radius_m`, by [`BEYOND_MARGIN`] and [`BEYOND_FLOOR_RAD`]: 2 asin(reach ÷ 2 `r_min`),
/// or π where no angle is, since the map from the unit sphere to the spheroid stretches no length
/// by less than its smallest radius.
#[must_use]
fn reach_angle(reach_m: f64, smallest_radius_m: f64) -> f64 {
    let half = reach_m * (1.0 + BEYOND_MARGIN) / (2.0 * smallest_radius_m);
    if half < 1.0 {
        2.0 * math::asin(half) + BEYOND_FLOOR_RAD
    } else {
        core::f64::consts::PI
    }
}

/// Whether no crater centred in `cell` can lie within the angle `reach_rad`, radians
/// ([`reach_angle`]), of the unit direction `dir`: then the search need not draw the cell, which
/// moves no value.
///
/// A cell's edges are great circles (lines of constant u or v on the cube's faces), so it is a
/// convex spherical quadrilateral, held within the cap about its centre's direction through its
/// farthest corner, of angular radius ρ. A centre in it is then at least the angle θ − ρ from
/// `dir`, θ the angle to the cell's centre. A cell whose centre is within the reach is drawn, one
/// beyond the reach by more than [`CELL_SPREAD_PER_LEVEL`]'s bound on ρ is passed by, and only
/// those between take their corners.
#[must_use]
fn is_beyond(cell: PatchKey, dir: [f64; 3], reach_rad: f64) -> bool {
    let side = f64::from(1_u32 << cell.level());
    let (i, j) = (f64::from(cell.i()), f64::from(cell.j()));
    let at = |s: f64, t: f64| {
        unit_dir(face_uv_to_xyz(FaceUv {
            face: cell.face(),
            u: st_to_uv(s / side),
            v: st_to_uv(t / side),
        }))
    };
    let centre = at(i + 0.5, j + 0.5);
    let apart = angle_between(dir, centre);
    if apart <= reach_rad {
        return false;
    }
    if apart > reach_rad + CELL_SPREAD_PER_LEVEL / side {
        return true;
    }
    let spread = [(i, j), (i + 1.0, j), (i, j + 1.0), (i + 1.0, j + 1.0)]
        .into_iter()
        .map(|(s, t)| angle_between(centre, at(s, t)))
        .fold(0.0, |widest, a| if a > widest { a } else { widest });
    apart - spread > reach_rad
}

/// The angle between the unit vectors `a` and `b`, radians, from their chord: 2 asin(|a − b| ÷ 2),
/// accurate at small angles, π where rounding carries the half-chord past 1.
#[must_use]
fn angle_between(a: [f64; 3], b: [f64; 3]) -> f64 {
    let half = 0.5 * chord(a, b);
    if half < 1.0 {
        2.0 * math::asin(half)
    } else {
        core::f64::consts::PI
    }
}

/// The 3 × 3 neighbourhood of `cell`: itself, its four edge neighbours and its corner neighbours
/// (three at a cube corner), across face edges by the cube's rule, sorted by key without repeats;
/// the array and how many of it are the neighbourhood. The small craters' own search's
/// (`synth/craters.rs`), whose function is private to it.
#[must_use]
fn neighbourhood(cell: PatchKey) -> ([PatchKey; 9], usize) {
    let mut cells = [cell; 9];
    let mut n = 1;
    for edge in Edge::ALL {
        cells[n] = cell.edge_neighbour(edge);
        n += 1;
    }
    for corner in cell.corner_neighbours().into_iter().flatten() {
        cells[n] = corner;
        n += 1;
    }
    cells[..n].sort_unstable();
    let mut kept = 1;
    for k in 1..n {
        let next = cells[k];
        if next != cells[kept - 1] {
            cells[kept] = next;
            kept += 1;
        }
    }
    (cells, kept)
}

#[cfg(test)]
mod tests {
    //! The maturity's tests: the time law (P14.T51.d), and R10.T1.c's exposure, cover and maturity
    //! at a point (decision-r10-t1c-maturity).

    use std::collections::BTreeSet;
    use std::fmt::Write as _;
    use std::sync::OnceLock;

    use hyperion_base::units::{Kelvin, Pascals, PerSquareKilometre};
    use hyperion_testkit::float::{assert_same_bits, bits};
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;
    use hyperion_testkit::lcg::Lcg;
    use hyperion_testkit::order::assert_order_independent;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;
    use crate::craters::{CraterParams, cumulative_density, lunar_chronology};
    use crate::cube::{Face, st_to_uv, xyz_to_face_uv};
    use crate::field::{BVR_EFFECTIVE_WAVELENGTHS_NM, CoarseCrater, CoarseField, MechanicsFamily};
    use crate::geometry::min_rate;
    use crate::material::{
        PhaseLaw, albedo_scale, class_fractions, exposed_albedo_scale, material_at,
        scaled_normal_albedo, weathered_normal_albedo,
    };
    use crate::synth::craters::diffusion_length;
    use crate::synth::test_support::{SEED, along_chord, king_distances, world};
    use crate::testing::{
        FieldBuilder, SyntheticWorld, moon_like_builder, moon_like_crater_params,
        one_crater_builder, one_crater_centre, pluto_like,
    };

    /// The lunar mature background's `I_s/FeO`, X, at which the fit is held: 75, a stand-in inside
    /// Morris 1978's mature range (see [`LUNAR_MATURATION_TIME`]).
    const MATURE_BACKGROUND_IS_FEO: f64 = 75.0;

    /// Morris's upper edge of the immature class and lower edge of the mature one, `I_s/FeO`
    /// (Morris 1978, pp. 2289–2290, read).
    const IMMATURE_BELOW: f64 = 30.0;
    const MATURE_ABOVE: f64 = 60.0;

    /// Tycho's and Copernicus's ages, Gyr (Grier et al. 2001, abstract read).
    const TYCHO: f64 = 0.109;
    const COPERNICUS: f64 = 0.810;

    #[test]
    fn is_fresh_at_zero_exposure_or_rate() {
        for t in [0.0, -0.0] {
            for k in [0.0, 1.0, 1e3] {
                assert_same_bits(maturity_after(Gigayears::new(t), k), 0.0);
            }
        }
        for t in [0.0, 1e-9, 1.0, 13.8] {
            assert_same_bits(maturity_after(Gigayears::new(t), 0.0), 0.0);
            assert_same_bits(maturity_after(Gigayears::new(t), -0.0), 0.0);
        }
    }

    #[test]
    fn never_falls_and_stays_in_the_unit_interval() {
        for k in [1e-6, 0.01, 0.5, 1.0, 8.6, 1e3] {
            let mut previous = 0.0;
            for i in 0..=2_000 {
                let t = 14.0 * f64::from(i) / 2_000.0;
                let m = maturity_after(Gigayears::new(t), k);
                assert!(
                    m.is_finite() && (0.0..=1.0).contains(&m),
                    "k {k}, t {t}: {m}"
                );
                assert!(m >= previous, "k {k}: {m} at {t} Gyr after {previous}");
                previous = m;
            }
        }
        // And in the rate, at a fixed exposure.
        let mut previous = 0.0;
        for i in 0..=1_000 {
            let k = 50.0 * f64::from(i) / 1_000.0;
            let m = maturity_after(Gigayears::new(0.1), k);
            assert!(m >= previous, "{m} at k {k} after {previous}");
            previous = m;
        }
    }

    #[test]
    fn is_below_one_while_f64_can_hold_it() {
        // x = k t ÷ τ_L; 1 − e^(−x) is distinct from 1 up to x ≈ 36.7.
        for x in [1e-300, 1e-12, 1e-3, 0.5, 1.0, 10.0, 30.0, 36.0] {
            let t = x * LUNAR_MATURATION_TIME.value();
            let m = maturity_after(Gigayears::new(t), 1.0);
            assert!(m < 1.0, "x {x}: {m}");
            assert!(m > 0.0, "x {x}: {m}");
        }
        // A tiny exposure keeps its precision: 1 − e^(−x) ≈ x.
        let x = 1e-12;
        let m = maturity_after(Gigayears::new(x * LUNAR_MATURATION_TIME.value()), 1.0);
        assert!((m / x - 1.0).abs() < 1e-11, "{m}");
        // Saturated: the Moon's 4 Gyr is 12 τ_L, Mercury's k ≈ 9 about 100.
        assert!(maturity_after(Gigayears::new(4.0), 1.0) > 0.99999);
        assert_same_bits(maturity_after(Gigayears::new(4.0), 9.0), 1.0);
    }

    #[test]
    fn holds_tycho_immature_and_copernicus_mature() {
        let x = MATURE_BACKGROUND_IS_FEO;
        let tycho = maturity_after(Gigayears::new(TYCHO), 1.0);
        let copernicus = maturity_after(Gigayears::new(COPERNICUS), 1.0);
        assert!(tycho < IMMATURE_BELOW / x, "Tycho {tycho}");
        assert!(copernicus > MATURE_ABOVE / x, "Copernicus {copernicus}");
        // The fit: τ_L the geometric centre of the bounds, to the stated four figures.
        let low = TYCHO / -math::ln_1p(-IMMATURE_BELOW / x);
        let high = COPERNICUS / -math::ln_1p(-MATURE_ABOVE / x);
        let centre = (low * high).sqrt();
        let tau = LUNAR_MATURATION_TIME.value();
        assert!((low..high).contains(&tau), "{low} < {tau} < {high}");
        assert!((tau - centre).abs() < 5e-5, "{tau} against {centre}");
        eprintln!(
            "τ_L {tau} Gyr in ({low:.4}, {high:.4}), centre {centre:.6}: \
             m(Tycho) {tycho:.4}, m(Copernicus) {copernicus:.4}"
        );
    }

    #[test]
    fn a_body_s_maturity_is_its_surface_s_exposure() {
        for (t, k) in [(0.0, 1.0), (0.1, 0.5), (3.9, 1.0), (4.0, 8.6), (1e-6, 1e3)] {
            assert_same_bits(
                body_maturity(Gigayears::new(t), k),
                maturity_after(Gigayears::new(t), k),
            );
        }
    }

    // The refusals of a negative or NaN exposure and of an infinite rate, and of R10.T1.c's
    // exposure and fresh share outside [0, 1], are in the crate's `tests/panics.rs` (`mod
    // maturity`), where every `should_panic` test lives.

    // ---- R1: the fresh/mature contrast, stated against Dollfus and Bowell 1971 ----

    /// One of R10.T1.d's fit rows, as `tables/space_weathering.rs`'s acceptance line gives it: its
    /// `FeO_T`, its grain density, kg m⁻³, and its fresh `A_N` in B, V and R under the regolith
    /// law.
    struct FitRow {
        name: &'static str,
        feo: f64,
        density_kg_m3: f64,
        fresh: [f64; 3],
    }

    /// The highland row, `feldspathic`: `FeO_T` 0.055, 2,900 kg m⁻³, fresh 0.613/0.667/0.698.
    const HIGHLAND: FitRow = FitRow {
        name: "highland",
        feo: 0.055,
        density_kg_m3: 2_900.0,
        fresh: [0.613, 0.667, 0.698],
    };

    /// The mare row, `basalt_fe`: `FeO_T` 0.16, 3,310 kg m⁻³, fresh 0.367/0.410/0.425.
    const MARE: FitRow = FitRow {
        name: "mare",
        feo: 0.16,
        density_kg_m3: 3_310.0,
        fresh: [0.367, 0.410, 0.425],
    };

    /// The wavelength Dollfus and Bowell 1971's normal albedos are at, nm: 600, their "orange light
    /// of wavelength λ 0.60 µ" (A&A 10, 29, p. 50; Table 4, pp. 43–45, gives the albedos).
    const DOLLFUS_BOWELL_NM: f64 = 600.0;

    /// Dollfus and Bowell 1971's (Table 4) fresh craters' contrasts over R10.T1.d's medians of the
    /// table's spots (0.114 highland, 0.072 mare), as the ruling gives them (R1): Tycho B42 0.169,
    /// Proclus B8 0.191 and Kepler B57 0.121, each a spot under 5″ (9 km or less) inside its
    /// crater; Tycho's low end, 1.34, is region D3's 0.153, a 65″ (about 121 km) average of Tycho
    /// and its ejecta (p. 45, and pp. 46–47 for the regions' size), and Proclus's D28's 0.180.
    const TYCHO_CONTRAST: (f64, f64) = (1.34, 1.48);
    const PROCLUS_CONTRAST: (f64, f64) = (1.58, 1.68);
    const KEPLER_CONTRAST: f64 = 1.68;

    /// Kepler's ages, Gyr: "no more than 50–100 m.y." (Basilevsky et al. 1977, LPS VIII, 70, p. 71;
    /// an upper bound, and a weak, morphological estimate). Tycho's is [`TYCHO`], 109 ± 4 Myr
    /// (Drozd et al. 1977, Proc. LSC 8, 3027); Proclus has no published age, only "Copernican".
    const KEPLER: [f64; 2] = [0.050, 0.100];

    /// The contrast of `row`, its `A_N` at maturity `m` over its mature background's, at m = 1, in
    /// band `band` (0 B, 1 V, 2 R).
    fn contrast(row: &FitRow, m: f64, band: usize) -> f64 {
        let at = |maturity| {
            weathered_normal_albedo(
                row.fresh[band],
                BVR_EFFECTIVE_WAVELENGTHS_NM[band],
                PhaseLaw::Regolith,
                row.density_kg_m3,
                row.feo,
                maturity,
            )
        };
        at(m) / at(1.0)
    }

    /// The contrast of `row` at maturity `m` taken linearly in wavelength from V's to R's, to 600
    /// nm.
    fn contrast_600(row: &FitRow, m: f64) -> f64 {
        let [_, v_nm, r_nm] = BVR_EFFECTIVE_WAVELENGTHS_NM;
        let (v, r) = (contrast(row, m, 1), contrast(row, m, 2));
        v + (DOLLFUS_BOWELL_NM - v_nm) / (r_nm - v_nm) * (r - v)
    }

    /// The exposure at which `row`'s contrast at 600 nm falls to `target`, Gyr, by bisection over
    /// 0–4 Gyr at the Moon's rate (the contrast falls with age).
    fn age_at_contrast(row: &FitRow, target: f64) -> f64 {
        let (mut lo, mut hi) = (0.0, 4.0);
        for _ in 0..60 {
            let mid = f64::midpoint(lo, hi);
            if contrast_600(row, maturity_after(Gigayears::new(mid), 1.0)) > target {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        f64::midpoint(lo, hi)
    }

    /// The ruling's replica of the law on the committed table (R1's table): the contrasts at m 0
    /// against 1 and at Tycho's 109 Myr, B, V and R, highland then mare.
    const REPLICA_FRESH: [[f64; 3]; 2] = [[7.00, 5.44, 5.10], [7.07, 7.19, 7.06]];
    const REPLICA_TYCHO: [[f64; 3]; 2] = [[2.46, 2.14, 2.05], [1.68, 2.12, 2.23]];

    /// The code's contrasts at m 0 and at 109 Myr, pinned to 1% (R1), each within 0.25% of the
    /// replica when pinned (R10.T1.c's record): a broken pin is a refit, never a wider bound.
    const PIN_FRESH: [[f64; 3]; 2] = [[6.999, 5.442, 5.104], [7.065, 7.189, 7.059]];
    const PIN_TYCHO: [[f64; 3]; 2] = [[2.456, 2.143, 2.045], [1.677, 2.116, 2.226]];

    /// R1: the contrast's invariants are asserted, its as-built values pinned, and its comparison
    /// with Dollfus and Bowell 1971's fresh craters at each crater's own age stated, a known miss
    /// (D2) until T49.i's whole-soil fresh rows and a refit of Λ, after which it becomes an
    /// assertion within the measured range.
    #[test]
    fn maturity_contrast_holds_its_invariants_and_states_the_known_miss() {
        let rows = [&HIGHLAND, &MARE];
        for row in rows {
            let mut previous = [f64::INFINITY; 3];
            for i in 0..=4_000_u32 {
                let t = f64::from(i) / 1_000.0;
                let m = maturity_after(Gigayears::new(t), 1.0);
                for (band, last) in previous.iter_mut().enumerate() {
                    let c = contrast(row, m, band);
                    assert!(c >= 1.0, "{} band {band} at {t} Gyr: {c}", row.name);
                    assert!(c <= *last, "{} band {band} rises at {t} Gyr: {c}", row.name);
                    if t < 1.0 {
                        assert!(c > 1.0, "{} band {band} at {t} Gyr: {c}", row.name);
                    }
                    *last = c;
                }
            }
            let copernicus = contrast(row, maturity_after(Gigayears::new(COPERNICUS), 1.0), 1);
            assert!(
                copernicus < 1.10,
                "{} at Copernicus's age: {copernicus}",
                row.name
            );
        }
        let tycho_m = maturity_after(Gigayears::new(TYCHO), 1.0);
        for (k, row) in rows.into_iter().enumerate() {
            for band in 0..3 {
                let fresh = contrast(row, 0.0, band);
                let tycho = contrast(row, tycho_m, band);
                for (what, code, replica, pin) in [
                    ("m 0", fresh, REPLICA_FRESH[k][band], PIN_FRESH[k][band]),
                    ("109 Myr", tycho, REPLICA_TYCHO[k][band], PIN_TYCHO[k][band]),
                ] {
                    let gap = code / replica - 1.0;
                    eprintln!(
                        "{} band {band} {what}: code {code:.4}, replica {replica}, gap {:+.2}%",
                        row.name,
                        100.0 * gap
                    );
                    assert!(
                        gap.abs() < 0.02,
                        "{} band {band} {what}: {code} is more than 2% from the replica's \
                         {replica}",
                        row.name
                    );
                    assert!(
                        (code / pin - 1.0).abs() < 0.01,
                        "{} band {band} {what}: {code} against its pin {pin}",
                        row.name
                    );
                }
            }
        }
        // D2, the known miss, printed.
        let [low, high] = [TYCHO_CONTRAST.0, TYCHO_CONTRAST.1];
        let (v, r) = (
            contrast(&HIGHLAND, tycho_m, 1),
            contrast(&HIGHLAND, tycho_m, 2),
        );
        let at_600 = contrast_600(&HIGHLAND, tycho_m);
        eprintln!(
            "D2: Tycho (highland, {TYCHO} Gyr, m {tycho_m:.3}): V {v:.3}, R {r:.3}, 600 nm \
             {at_600:.3} against Dollfus and Bowell's {low}–{high}, {:.2}–{:.2} times; m 0 \
             limits V {:.2}, R {:.2}",
            at_600 / high,
            at_600 / low,
            contrast(&HIGHLAND, 0.0, 1),
            contrast(&HIGHLAND, 0.0, 2)
        );
        for age in KEPLER {
            let m = maturity_after(Gigayears::new(age), 1.0);
            let (v, r, at_600) = (
                contrast(&MARE, m, 1),
                contrast(&MARE, m, 2),
                contrast_600(&MARE, m),
            );
            eprintln!(
                "D2: Kepler (mare, {age} Gyr, m {m:.3}): V {v:.3}, R {r:.3}, 600 nm {at_600:.3} \
                 against {KEPLER_CONTRAST}, {:.2} times; m 0 limits V {:.2}, R {:.2}",
                at_600 / KEPLER_CONTRAST,
                contrast(&MARE, 0.0, 1),
                contrast(&MARE, 0.0, 2)
            );
        }
        let (from, to) = (
            age_at_contrast(&HIGHLAND, PROCLUS_CONTRAST.1),
            age_at_contrast(&HIGHLAND, PROCLUS_CONTRAST.0),
        );
        eprintln!(
            "D2: Proclus (highland, no published age): the model reaches {}–{} at 600 nm at \
             {:.0}–{:.0} Myr; Tycho's {low}–{high} at {:.0}–{:.0} Myr, Kepler's \
             {KEPLER_CONTRAST} at {:.0} Myr",
            PROCLUS_CONTRAST.0,
            PROCLUS_CONTRAST.1,
            1e3 * from,
            1e3 * to,
            1e3 * age_at_contrast(&HIGHLAND, high),
            1e3 * age_at_contrast(&HIGHLAND, low),
            1e3 * age_at_contrast(&MARE, KEPLER_CONTRAST)
        );
    }

    // ---- The fields R4's tests read ----

    /// The Moon-like world weathered as the Moon is, at maturity 1 and the Moon's rate, on its own
    /// 4.4 Gyr surface: built once per test process.
    fn weathered_moon() -> &'static CoarseField {
        static FIELD: OnceLock<CoarseField> = OnceLock::new();
        FIELD.get_or_init(|| {
            moon_like_builder()
                .maturity(1.0)
                .weathering_rate(1.0)
                .build()
        })
    }

    /// The weathered Moon-like world on a younger surface, `age` Gyr old, whose crater contract's
    /// N(>1 km) is the chronology's there, `n_1km` per km²: 4.8 × 10⁻³ at 3.5 Gyr and 8.4 × 10⁻⁴ at
    /// 1 Gyr (R4's estimates' surfaces).
    fn younger_moon(age: f64, n_1km: f64) -> CoarseField {
        moon_like_builder()
            .surface_age(Gigayears::new(age))
            .crater_params(moon_like_crater_params(PerSquareKilometre::new(n_1km)))
            .maturity(1.0)
            .weathering_rate(1.0)
            .build()
    }

    /// The single crater's age in its young variant, Gyr.
    const YOUNG_CRATER_AGE: f64 = 0.1;

    /// The single crater's degradation code in its young variant: 51, a fifth of its relief worn,
    /// so that g is 0.8.
    const YOUNG_CRATER_DEGRADATION: u8 = 51;

    /// The single crater's world, its crater `YOUNG_CRATER_AGE` old and of degradation
    /// `degradation`, at its 4 Gyr surface's body maturity and the Moon's rate. Its crater density
    /// is zero, so that no small crater is drawn and the coarse crater alone reaches its ground.
    fn young_crater_with(degradation: u8) -> CoarseField {
        one_crater_builder(Gigayears::new(YOUNG_CRATER_AGE), degradation)
            .maturity(body_maturity(Gigayears::new(4.0), 1.0))
            .weathering_rate(1.0)
            .build()
    }

    /// [`young_crater_with`] at [`YOUNG_CRATER_DEGRADATION`], built once per test process.
    fn young_crater() -> &'static CoarseField {
        static FIELD: OnceLock<CoarseField> = OnceLock::new();
        FIELD.get_or_init(|| young_crater_with(YOUNG_CRATER_DEGRADATION))
    }

    /// The distance of `dir` from coarse crater `crater` of the field of `header`, rim radii, as
    /// the synthesis measures it, with the crater's exterior width a.
    fn rim_radii_from(header: &FieldHeader, crater: &CoarseCrater, dir: [f64; 3]) -> (f64, f64) {
        let figure = header.figure();
        let shape = crater_shape(header.craters(), crater.diameter);
        let rim_m = 0.5 * shape.diameter().value();
        let x = chord(figure.point(dir), figure.point(crater.centre)) / rim_m;
        (x, shape.exterior_width())
    }

    /// The smallest radius of `field`'s figure, metres.
    fn smallest_radius(field: &CoarseField) -> f64 {
        let figure = field.header().figure();
        if figure.polar_radius_m < figure.equatorial_radius_m {
            figure.polar_radius_m
        } else {
            figure.equatorial_radius_m
        }
    }

    /// The direction a query at `dir` reads, through its face coordinates, as [`local`] asks it.
    fn queried(dir: [f64; 3]) -> [f64; 3] {
        unit_dir(face_uv_to_xyz(xyz_to_face_uv(dir)))
    }

    /// The maturity at `dir` of `field`, through a fresh cache.
    fn local<F: FieldView>(field: &F, dir: [f64; 3]) -> f64 {
        let synth = Synthesiser::new(field, SEED);
        maturity_at(&synth, &mut SynthCache::new(), xyz_to_face_uv(dir))
            .expect("the field holds the cell")
            .expect("the field carries a maturity")
    }

    // ---- R4: one crater ----

    /// One young coarse crater: inside its rim the maturity is `m_body` − g `f_ex` (`m_body` −
    /// `maturity_after`(`t_c`, k)), and at every distance it is the one-crater formula at g `f_ex`
    /// p(x), continuous across the rim and the reach and `m_body` itself from the reach (1 + a) R
    /// on; and p(2), p(4) and p(6) are R4's formulas.
    #[test]
    fn maturity_one_crater_s_profile_is_its_fresh_share_s() {
        let field = young_crater();
        let header = field.header();
        let crater = &field.craters()[0];
        let (body, rate) = (
            header.maturity().unwrap(),
            header.weathering_rate().unwrap(),
        );
        let t_s = header.surface_age();
        let g = kept_relief(crater);
        assert_same_bits(g, 1.0 - 51.0 / 255.0);
        let f_ex = excavated_share(t_s, crater.age, crater.diameter);
        assert!(
            f_ex > 0.99,
            "a 300 km crater digs below a 4 Gyr mature layer: {f_ex}"
        );
        let m_c = maturity_after(crater.age, rate);
        let inside = body - g * f_ex * (body - m_c);
        let radius_m = header.figure().equatorial_radius_m;
        let rim_m = 0.5 * crater.diameter.value();
        let (_, a) = rim_radii_from(header, crater, crater.centre);
        let at = |x: f64| along_chord(crater.centre, x * rim_m, radius_m);
        for x in [0.0, 0.3, 0.7, 0.99] {
            assert_same_bits(local(field, at(x)), inside);
        }
        let mut previous = inside;
        for k in 0..=60_u8 {
            let target = 0.95 + f64::from(k) * (a + 0.6) / 60.0;
            let dir = at(target);
            let (x, _) = rim_radii_from(header, crater, queried(dir));
            let share = g * f_ex * halo_share(x, 1.0 + a);
            let want = crater_maturity(body, crater.age, rate, share);
            let got = local(field, dir);
            assert_same_bits(got, want);
            assert!(
                got >= previous,
                "the maturity falls outwards at {x}: {got} after {previous}"
            );
            previous = got;
            if x >= 1.0 + a {
                assert_same_bits(got, body);
            }
        }
        // Continuous across the rim and the reach.
        for edge in [1.0, 1.0 + a] {
            let (low, high) = (local(field, at(edge - 1e-6)), local(field, at(edge + 1e-6)));
            assert!(
                (high - low).abs() < 1e-5,
                "a step at {edge} rim radii: {low} to {high}"
            );
        }
        assert_same_bits(local(field, at(1.0 + a + 0.01)), body);
        // The halo's shape: R4's formulas, a small crater's to 10 R and a coarse one's to its
        // reach.
        let e3 = math::exp(-3.0);
        let end = math::exp(-a / 3.0);
        for x in [2.0, 4.0, 6.0] {
            let want = (math::exp(-(x - 1.0) / 3.0) - e3) / (1.0 - e3);
            let got = halo_share(x, HALO_EXTENT_RIM_RADII);
            assert!(
                (got / want - 1.0).abs() < 1e-14,
                "p({x}): {got} against {want}"
            );
            let xc = 1.0 + a * (x - 1.0) / 6.0;
            let want = (math::exp(-(xc - 1.0) / 3.0) - end) / (1.0 - end);
            let coarse = halo_share(xc, 1.0 + a);
            assert!(
                (coarse / want - 1.0).abs() < 1e-12,
                "coarse p({xc}): {coarse} against {want}"
            );
        }
        assert_same_bits(halo_share(1.0, HALO_EXTENT_RIM_RADII), 1.0);
        assert_same_bits(halo_share(10.0, HALO_EXTENT_RIM_RADII), 0.0);
        assert_same_bits(halo_share(0.0, 1.0), 1.0);
        assert_same_bits(halo_share(1.5, 1.0), 0.0);
        // For the record: the halo against Grier et al. 2000's profiles, the excess as a share of
        // the rim's (Fig. 3): North Ray's ranges as the ruling reads them, and South Ray's as this
        // lane's science check read them.
        for (x, low, high, south) in [
            (2.0, 0.60, 0.62, 0.68),
            (4.0, 0.25, 0.30, 0.24),
            (6.0, 0.14, 0.15, 0.13),
            (8.0, 0.085, 0.095, 0.08),
        ] {
            eprintln!(
                "halo p({x}) {:.3} against North Ray's {low}–{high} and South Ray's {south}",
                halo_share(x, HALO_EXTENT_RIM_RADII)
            );
        }
    }

    /// A small crater's halo reaches 10 of its rim radii, beyond its reach: the search through the
    /// 3 × 3 neighbourhood two levels up finds every crater a direct search over a wide
    /// neighbourhood at the octave's own level finds, and no other, at points of the weathered
    /// Moon-like world; and the cell two levels up holds 10 of each octave's largest rim radii, on
    /// every world.
    #[test]
    fn maturity_small_craters_halo_search_finds_every_crater_within_ten_rim_radii() {
        let field = weathered_moon();
        let figure = field.header().figure();
        let smallest_radius_m = smallest_radius(field);
        let synth = Synthesiser::new(field, SEED);
        let craters = synth.craters();
        let mut cache = CraterCache::new();
        let mut rng = Lcg::new(0x6861_6c6f_0000_0001);
        let surface_age = field.header().surface_age();
        let (mut found_any, mut skipped) = (0, 0);
        for _ in 0..6 {
            let dir = unit_dir([0, 1, 2].map(|_| 2.0 * rng.next_f64() - 1.0));
            let point = figure.point(dir);
            for octave in craters.octaves().octaves() {
                let mut searched = BTreeSet::new();
                // An octave the search passes by can expose nothing: every crater's `f_ex` is 0.
                let exposes = can_expose(octave, surface_age);
                skipped += usize::from(!exposes);
                each_small_crater_near(
                    craters,
                    octave,
                    (dir, point),
                    smallest_radius_m,
                    &mut cache,
                    |crater, x| {
                        assert!(x < HALO_EXTENT_RIM_RADII, "{x}");
                        searched.insert(crater.point_m().map(bits));
                        if !exposes {
                            let share =
                                excavated_share(surface_age, crater.age(), crater.diameter());
                            assert_same_bits(share, 0.0);
                        }
                    },
                );
                let home = PatchKey::containing(octave.level(), dir).unwrap();
                let mut direct = BTreeSet::new();
                for cell in king_distances(home, 6).into_keys() {
                    for crater in craters.craters_in(octave.index(), cell, &mut cache) {
                        let halo_m = HALO_EXTENT_RIM_RADII * 0.5 * crater.diameter().value();
                        if squared_chord(point, crater.point_m()) < halo_m * halo_m {
                            direct.insert(crater.point_m().map(bits));
                        }
                    }
                }
                assert_eq!(searched, direct, "octave {} at {dir:?}", octave.index());
                found_any += searched.len();
            }
        }
        assert!(found_any > 0, "the points lie in some crater's halo");
        assert!(
            skipped > 0,
            "the 4.4 Gyr ground's finest octaves expose nothing"
        );
        for field in SyntheticWorld::ALL
            .iter()
            .map(|&w| world(w))
            .chain([weathered_moon()])
        {
            let smallest_radius_m = smallest_radius(field);
            let synth = Synthesiser::new(field, SEED);
            for octave in synth.craters().octaves().octaves() {
                let Some(up) = octave.level().checked_sub(SEARCH_LEVELS_UP) else {
                    continue;
                };
                let width = min_rate() / f64::from(1_u32 << up);
                let held = 2.0 * smallest_radius_m * math::sin(0.5 * width);
                let largest_rim_m = 0.5 * octave.diameters().upper().value();
                assert!(
                    HALO_EXTENT_RIM_RADII * largest_rim_m <= held,
                    "octave {}: {} m against {held} m",
                    octave.index(),
                    HALO_EXTENT_RIM_RADII * largest_rim_m
                );
            }
        }
    }

    /// The maturity at a point does not depend on the order of queries or on what the caller's
    /// cache holds: through one cache, warmed by the classifier and the height at other points
    /// first, every order of queries gives each point the bits it has alone, on the weathered
    /// Moon-like world (at points spread over its octaves' cells, some in a small crater's halo)
    /// and across the young single crater (sim-determinism, order independence).
    #[test]
    fn maturity_at_does_not_depend_on_the_order_of_queries_or_the_cache() {
        let mut rng = Lcg::new(0x6f72_6465_7200_0002);
        let moon = weathered_moon();
        let mut dirs: Vec<[f64; 3]> = (0..8)
            .map(|_| unit_dir([0, 1, 2].map(|_| 2.0 * rng.next_f64() - 1.0)))
            .collect();
        // Two clusters, so that later queries read cells the earlier ones drew.
        for k in 0..4_u8 {
            let offset = 30.0 * f64::from(k);
            dirs.push(along_chord(dirs[0], offset, smallest_radius(moon)));
            dirs.push(along_chord(dirs[1], 1e3 + offset, smallest_radius(moon)));
        }
        let crater = young_crater();
        let centre = crater.craters()[0].centre;
        let rim_m = 0.5 * crater.craters()[0].diameter.value();
        let across: Vec<[f64; 3]> = (0..6_u8)
            .map(|k| along_chord(centre, rim_m * f64::from(k) * 0.4, smallest_radius(crater)))
            .collect();
        for (field, points) in [(moon, &dirs), (crater, &across)] {
            let synth = Synthesiser::new(field, SEED);
            let mut warm = SynthCache::new();
            for &d in points.iter().rev().take(3) {
                let at = xyz_to_face_uv(d);
                material_at(&synth, &mut warm, at).expect("whole");
                let other = along_chord(d, 500.0, smallest_radius(field));
                maturity_at(&synth, &mut warm, xyz_to_face_uv(other)).expect("whole");
            }
            let shared = std::cell::RefCell::new(warm);
            assert_order_independent(points, |&d| {
                maturity_at(&synth, &mut shared.borrow_mut(), xyz_to_face_uv(d))
                    .expect("whole")
                    .map(bits)
            });
            let alone: Vec<Option<u64>> = points
                .iter()
                .map(|&d| Some(bits(local(field, d))))
                .collect();
            let shared_again: Vec<Option<u64>> = points
                .iter()
                .map(|&d| {
                    maturity_at(&synth, &mut shared.borrow_mut(), xyz_to_face_uv(d))
                        .expect("whole")
                        .map(bits)
                })
                .collect();
            assert_eq!(shared_again, alone);
        }
    }

    // ---- R4: k scaling, f_ex and g, overlaps ----

    /// The one-crater formula at a given share gives the same maturity for age t at rate k as for
    /// age k t at rate 1, to 10⁻¹²: the law is a function of k t alone.
    #[test]
    fn maturity_one_crater_formula_scales_with_k_t() {
        for body in [0.5, 0.95, 1.0] {
            for share in [0.0, 0.3, 1.0] {
                for t in [0.001, 0.01, 0.1, 0.5, 2.0] {
                    for k in [0.1, 0.5, 2.0, 8.6, 1e3] {
                        let scaled = crater_maturity(body, Gigayears::new(t), k, share);
                        let moon = crater_maturity(body, Gigayears::new(k * t), 1.0, share);
                        assert!(
                            (scaled - moon).abs() < 1e-12,
                            "m_body {body}, s {share}, t {t}, k {k}: {scaled} against {moon}"
                        );
                    }
                }
            }
        }
        // With s = 1 it is min(m_body, maturity_after(t_c, k)), to rounding; with s = 0, m_body.
        for t in [0.0, 0.05, 0.3, 3.0] {
            let m_c = maturity_after(Gigayears::new(t), 1.0);
            let full = crater_maturity(0.9, Gigayears::new(t), 1.0, 1.0);
            let least = if m_c < 0.9 { m_c } else { 0.9 };
            assert!((full - least).abs() < 1e-15, "{t}: {full} against {least}");
            assert_same_bits(crater_maturity(0.9, Gigayears::new(t), 1.0, 0.0), 0.9);
        }
    }

    /// `f_ex` is 0 where the excavation depth 0.084 D is no deeper than the mature layer, (1 − q)²
    /// otherwise, near the ruling's figures on 3.5 Gyr ground, and 1 for a crater as old as its
    /// surface; a crater worn to g = 0 never lowers the maturity.
    #[test]
    fn maturity_excavated_share_and_wear_bound_the_fresh_share() {
        let surface = Gigayears::new(3.5);
        let fresh = Gigayears::ZERO;
        let d_m = reworked_depth(surface).value();
        assert!(
            (d_m - 0.865).abs() < 0.005,
            "the mature layer at 3.5 Gyr: {d_m} m"
        );
        assert_same_bits(reworked_depth(Gigayears::new(1e-3)).value(), 0.022);
        assert_same_bits(reworked_depth(Gigayears::ZERO).value(), 0.0);
        assert_same_bits(reworked_depth(Gigayears::new(-1.0)).value(), 0.0);
        let edge_m = d_m / EXCAVATION_DEPTH_RATIO;
        for d in [2.0, 5.0, 0.999 * edge_m] {
            assert_same_bits(excavated_share(surface, fresh, Metres::new(d)), 0.0);
        }
        for (d, ruling) in [(18.0, 0.18), (50.0, 0.62), (100.0, 0.80), (1_000.0, 0.98)] {
            let q = d_m / (EXCAVATION_DEPTH_RATIO * d);
            let got = excavated_share(surface, fresh, Metres::new(d));
            assert_same_bits(got, (1.0 - q) * (1.0 - q));
            assert!(
                (got - ruling).abs() < 0.015,
                "{d} m: {got} against the ruling's {ruling}"
            );
        }
        for d in [2.0, 18.0, 1e5] {
            assert_same_bits(excavated_share(surface, surface, Metres::new(d)), 1.0);
            assert_same_bits(
                excavated_share(surface, Gigayears::new(4.0), Metres::new(d)),
                1.0,
            );
        }
        // g = 0: a coarse crater worn away leaves the background, inside its rim too.
        let worn = young_crater_with(255);
        let crater = &worn.craters()[0];
        assert_same_bits(kept_relief(crater), 0.0);
        let body = worn.header().maturity().unwrap();
        let radius_m = worn.header().figure().equatorial_radius_m;
        for x in [0.0, 0.5, 1.5] {
            let dir = along_chord(crater.centre, x * 0.5 * crater.diameter.value(), radius_m);
            assert_same_bits(local(&worn, dir), body);
        }
    }

    /// A view of a field whose index lists each cell's coarse craters in the reverse of the field's
    /// order.
    struct Reversed<'a>(&'a CoarseField);

    impl FieldView for Reversed<'_> {
        fn header(&self) -> &FieldHeader {
            self.0.header()
        }

        fn cell(&self, cell: PatchKey) -> Option<&crate::field::SynthesisCell> {
            self.0.cell(cell)
        }

        fn climate(&self, cell: PatchKey) -> Option<&crate::field::ClimateCell> {
            self.0.climate(cell)
        }

        fn craters_reaching(&self, cell: PatchKey) -> impl Iterator<Item = &CoarseCrater> + '_ {
            let mut listed: Vec<&CoarseCrater> = self.0.craters_reaching(cell).collect();
            listed.reverse();
            listed.into_iter()
        }
    }

    /// Two overlapping young craters give the larger deficit, in either order of enumeration.
    #[test]
    fn maturity_overlapping_craters_give_the_larger_deficit() {
        let radius_m = 1.7374e6;
        let first = one_crater_centre();
        let second = along_chord(first, 200e3, radius_m);
        let field = FieldBuilder::new(Metres::new(radius_m))
            .crater(first, Metres::new(300e3), Gigayears::new(0.05), 0)
            .crater(second, Metres::new(250e3), Gigayears::new(0.2), 30)
            .maturity(1.0)
            .weathering_rate(1.0)
            .build();
        let header = field.header();
        assert_eq!(field.craters().len(), 2);
        let (body, rate, t_s) = (1.0, 1.0, header.surface_age());
        let mut both = 0;
        for k in 0..=40_u8 {
            let dir = along_chord(first, 5e3 * f64::from(k) + 1.0, radius_m);
            let deficits: Vec<f64> = field
                .craters()
                .iter()
                .map(|crater| {
                    let (x, a) = rim_radii_from(header, crater, queried(dir));
                    let share = kept_relief(crater)
                        * excavated_share(t_s, crater.age, crater.diameter)
                        * halo_share(x, 1.0 + a);
                    deficit_of(body, crater.age, rate, share)
                })
                .collect();
            let (d0, d1) = (deficits[0], deficits[1]);
            if d0 > 0.0 && d1 > 0.0 && bits(d0) != bits(d1) {
                both += 1;
            }
            let want = less(body, if d0 > d1 { d0 } else { d1 });
            let got = local(&field, dir);
            assert_same_bits(got, want);
            assert_same_bits(local(&Reversed(&field), dir), got);
            // The larger deficit is the lesser of the two craters' maturities.
            let each = [d0, d1].map(|d| less(body, d));
            assert_same_bits(got, if each[0] < each[1] { each[0] } else { each[1] });
        }
        assert!(both > 5, "the craters' haloes overlap at {both} points");
    }

    // ---- R2 and R3: the identity, `Melt`, the cover ----

    /// With no exposure, with a header that carries neither part, with no cover and at maturity 0,
    /// `exposed_properties` is `properties` to the bit, for every form of every entry of every
    /// synthetic world; and an entry past the palette is refused alike.
    #[test]
    fn maturity_exposed_properties_are_properties_to_the_bit_without_an_exposure() {
        for &w in SyntheticWorld::ALL {
            let header = world(w).header();
            assert_eq!(GroundExposure::of_header(header), GroundExposure::NONE);
            let palette = header.palette();
            let entries = u8::try_from(palette.len()).unwrap();
            for entry in 0..entries {
                for &class in MaterialClass::ALL {
                    let m = Material::new(class, entry).unwrap();
                    let fresh = properties(m, palette).unwrap();
                    for exposure in [
                        GroundExposure::NONE,
                        GroundExposure::of_header(header),
                        GroundExposure::NONE.with_organic_cover(0.0),
                        GroundExposure::NONE.with_maturity(0.0),
                        GroundExposure::NONE
                            .with_maturity(0.0)
                            .with_organic_cover(0.0),
                    ] {
                        let exposed = exposed_properties(m, palette, exposure).unwrap();
                        assert_eq!(exposed, fresh, "{w:?} {m:?} at {exposure:?}");
                        assert_eq!(
                            exposed.normal_albedo_bvr().map(bits),
                            fresh.normal_albedo_bvr().map(bits),
                            "{w:?} {m:?} at {exposure:?}"
                        );
                    }
                }
            }
            let past = Material::new(MaterialClass::Regolith, entries).unwrap();
            assert_eq!(
                exposed_properties(
                    past,
                    palette,
                    GroundExposure::NONE
                        .with_maturity(1.0)
                        .with_organic_cover(1.0)
                ),
                Err(properties(past, palette).unwrap_err())
            );
        }
    }

    /// A header's exposure is its maturity and its cover, each where it carries one.
    #[test]
    fn maturity_a_header_s_exposure_is_its_maturity_and_cover() {
        let field = FieldBuilder::new(Metres::new(1.7374e6))
            .maturity(0.4)
            .weathering_rate(1.0)
            .organic_cover(0.3)
            .build();
        let exposure = GroundExposure::of_header(field.header());
        assert_eq!(exposure.maturity().map(bits), Some(bits(0.4)));
        assert_eq!(exposure.organic_cover().map(bits), Some(bits(0.3)));
        assert_eq!(
            exposure,
            GroundExposure::NONE
                .with_maturity(0.4)
                .with_organic_cover(0.3)
        );
        let covered = FieldBuilder::new(Metres::new(1.7374e6))
            .organic_cover(0.0)
            .build();
        let exposure = GroundExposure::of_header(covered.header());
        assert_eq!(exposure.maturity(), None);
        assert_eq!(exposure.organic_cover().map(bits), Some(bits(0.0)));
    }

    /// `Melt` keeps its fresh albedo at maturity 1, while the regolith of the same iron-bearing
    /// entry darkens in every band, its law and every other figure kept.
    #[test]
    fn maturity_melt_stays_fresh_while_its_regolith_weathers() {
        let palette = weathered_moon().header().palette();
        let mature = GroundExposure::NONE.with_maturity(1.0);
        let mut seen = 0;
        for (entry, e) in (0_u8..).zip(palette.entries()) {
            if !e.feo_mass_fraction.is_some_and(|f| f > 0.0) {
                continue;
            }
            seen += 1;
            let melt = Material::new(MaterialClass::Melt, entry).unwrap();
            let fresh = properties(melt, palette).unwrap();
            let exposed = exposed_properties(melt, palette, mature).unwrap();
            assert_eq!(
                exposed.normal_albedo_bvr().map(bits),
                fresh.normal_albedo_bvr().map(bits)
            );
            let regolith = Material::new(MaterialClass::Regolith, entry).unwrap();
            let fresh = properties(regolith, palette).unwrap();
            let weathered = exposed_properties(regolith, palette, mature).unwrap();
            for band in 0..3 {
                assert!(
                    weathered.normal_albedo_bvr()[band] < fresh.normal_albedo_bvr()[band],
                    "{e:?} band {band}"
                );
            }
            assert_eq!(weathered.phase_law(), fresh.phase_law());
            assert_eq!(weathered.phase_law_origin(), fresh.phase_law_origin());
            assert_eq!(
                weathered.with_normal_albedo_bvr(fresh.normal_albedo_bvr()),
                fresh
            );
        }
        assert_eq!(seen, 2, "the Moon-like palette's two iron-bearing crusts");
    }

    /// A palette entry of `key` in `role`, a volatile ice's mechanics, its normal albedo `albedo`
    /// in every band and no iron.
    fn ice(key: &str, role: PaletteRole, albedo: f64) -> PaletteEntry {
        PaletteEntry {
            substance: SubstanceKey::new(key).expect("a test key is in the grammar"),
            role,
            normal_albedo_bvr: [albedo; 3],
            phase_row: 0,
            grain_density_kg_m3: 900.0,
            transition: Kelvin::new(90.0),
            mechanics: MechanicsFamily::VolatileIce,
            liquid: None,
            feo_mass_fraction: Some(0.0),
        }
    }

    /// A tholin cover of a dark red albedo, illustrative.
    fn tholin() -> PaletteEntry {
        PaletteEntry {
            normal_albedo_bvr: [0.06, 0.09, 0.14],
            mechanics: MechanicsFamily::Organic,
            ..ice("tholin", PaletteRole::Cover, 0.0)
        }
    }

    /// A formula holds carbon where a `C` is not followed by a lowercase letter.
    #[test]
    fn maturity_a_formula_holds_carbon_by_its_symbols() {
        for key in [
            "CH4", "CO", "C2H6", "CH3OH", "HCN", "CS2", "C", "CO2", "NaHCO3",
        ] {
            assert!(holds_carbon(key), "{key}");
        }
        for key in [
            "Ca", "Cl", "Co", "Cs", "CaSO4", "HCl", "NaCl", "H2O", "N2", "NH3", "Cr2O3",
        ] {
            assert!(!holds_carbon(key), "{key}");
        }
    }

    /// The cover: zero or absent gives the substance's own albedo to the bit, and cover 1 the cover
    /// entry's, on every form of an eligible entry; an ineligible entry (`H2O`, `CO2`, a `Ca` or
    /// `Cl` formula, a name, a carbon formula that is not an ice) is unchanged; `N2` takes it only
    /// beside a carbon ice; and without a cover entry nothing is blended. On the Pluto-like world
    /// and on hand-built palettes.
    #[test]
    fn maturity_cover_blends_only_where_the_ground_takes_one() {
        let pluto = pluto_like(Pascals::new(1.0));
        let palette = pluto.header().palette();
        let keys: Vec<&str> = palette
            .entries()
            .iter()
            .map(|e| e.substance.as_str())
            .collect();
        assert_eq!(keys, ["H2O", "CH4", "N2", "tholin"]);
        let takes: Vec<bool> = palette
            .entries()
            .iter()
            .map(|e| takes_organic_cover(palette, e))
            .collect();
        assert_eq!(takes, [false, true, true, false]);
        check_cover(palette);
        let hand = MaterialPalette::new(vec![
            ice("CH4", PaletteRole::Ice, 0.8),
            ice("CO2", PaletteRole::Ice, 0.7),
            ice("CaSO4", PaletteRole::Ice, 0.6),
            ice("H2O", PaletteRole::Ice, 0.5),
            ice("HCl", PaletteRole::Ice, 0.4),
            ice("N2", PaletteRole::Ice, 0.9),
            ice("methane_ice", PaletteRole::Ice, 0.3),
            tholin(),
            ice("C2H6", PaletteRole::Deposit, 0.35),
        ])
        .unwrap();
        let takes: Vec<bool> = hand
            .entries()
            .iter()
            .map(|e| takes_organic_cover(&hand, e))
            .collect();
        assert_eq!(
            takes,
            [true, false, false, false, false, true, false, false, false]
        );
        check_cover(&hand);
        // N2 beside CO2 alone takes none, and beside no carbon ice none.
        for alone in [
            vec![
                ice("CO2", PaletteRole::Ice, 0.7),
                ice("N2", PaletteRole::Ice, 0.9),
                tholin(),
            ],
            vec![ice("N2", PaletteRole::Ice, 0.9), tholin()],
        ] {
            let alone = MaterialPalette::new(alone).unwrap();
            assert!(
                alone
                    .entries()
                    .iter()
                    .all(|e| !takes_organic_cover(&alone, e))
            );
            check_cover(&alone);
        }
        // No cover entry: nothing to blend.
        let bare = MaterialPalette::new(vec![ice("CH4", PaletteRole::Ice, 0.8)]).unwrap();
        assert!(takes_organic_cover(&bare, &bare.entries()[0]));
        let m = Material::new(MaterialClass::Frost, 0).unwrap();
        let exposed =
            exposed_properties(m, &bare, GroundExposure::NONE.with_organic_cover(1.0)).unwrap();
        assert_eq!(exposed, properties(m, &bare).unwrap());
    }

    /// A maturity and a cover together: the entry's albedo is weathered, times its form's factor,
    /// and then blended with the cover entry's own albedo weathered to the same maturity, without a
    /// factor; `Melt` blends the fresh albedos. On a palette whose carbon ice and cover both carry
    /// iron, so that both weather.
    #[test]
    fn maturity_and_cover_weather_both_then_blend() {
        let with_iron = |e: PaletteEntry, feo: f64| PaletteEntry {
            feo_mass_fraction: Some(feo),
            ..e
        };
        let palette = MaterialPalette::new(vec![
            with_iron(ice("CH4", PaletteRole::Ice, 0.8), 0.1),
            with_iron(tholin(), 0.05),
        ])
        .unwrap();
        let (entry, cover) = (&palette.entries()[0], &palette.entries()[1]);
        let (m, c) = (0.7, 0.5);
        let exposure = GroundExposure::NONE.with_maturity(m).with_organic_cover(c);
        assert_ne!(
            weathered_entry_bvr(entry, m).map(bits),
            entry.normal_albedo_bvr.map(bits),
            "the ice weathers"
        );
        assert_ne!(
            weathered_entry_bvr(cover, m).map(bits),
            cover.normal_albedo_bvr.map(bits),
            "the cover weathers"
        );
        for class in [
            MaterialClass::Regolith,
            MaterialClass::Frost,
            MaterialClass::Glacier,
            MaterialClass::Snow,
            MaterialClass::Melt,
        ] {
            let material = Material::new(class, 0).unwrap();
            let fresh = properties(material, &palette).unwrap();
            let factor = fresh.form().albedo_factor().unwrap_or(1.0);
            let (own, covering) = if class == MaterialClass::Melt {
                (entry.normal_albedo_bvr, cover.normal_albedo_bvr)
            } else {
                (weathered_entry_bvr(entry, m), weathered_entry_bvr(cover, m))
            };
            let want = [0, 1, 2].map(|b| (1.0 - c) * (own[b] * factor) + c * covering[b]);
            let got = exposed_properties(material, &palette, exposure).unwrap();
            assert_eq!(
                got.normal_albedo_bvr().map(bits),
                want.map(bits),
                "{material:?}"
            );
            assert_eq!(got.with_normal_albedo_bvr(fresh.normal_albedo_bvr()), fresh);
        }
    }

    /// Every form of every entry of `palette` under covers of none, 0, −0, ½ and 1: the identity
    /// where the cover is none or zero or the entry takes none, the cover entry's own albedo at 1,
    /// and the areal blend at ½, its law and origin kept.
    fn check_cover(palette: &MaterialPalette) {
        let cover = organic_cover_entry(palette).map(|k| palette.get(k).unwrap().normal_albedo_bvr);
        let entries = u8::try_from(palette.len()).unwrap();
        for entry in 0..entries {
            let takes = takes_organic_cover(palette, palette.get(entry).unwrap());
            for &class in MaterialClass::ALL {
                let m = Material::new(class, entry).unwrap();
                let fresh = properties(m, palette).unwrap();
                let own = fresh.normal_albedo_bvr();
                for c in [None, Some(0.0), Some(-0.0)] {
                    let exposed = exposed_properties(
                        m,
                        palette,
                        c.map_or(GroundExposure::NONE, |c| {
                            GroundExposure::NONE.with_organic_cover(c)
                        }),
                    )
                    .unwrap();
                    assert_eq!(
                        exposed.normal_albedo_bvr().map(bits),
                        own.map(bits),
                        "{m:?} {c:?}"
                    );
                }
                let full =
                    exposed_properties(m, palette, GroundExposure::NONE.with_organic_cover(1.0))
                        .unwrap();
                let half =
                    exposed_properties(m, palette, GroundExposure::NONE.with_organic_cover(0.5))
                        .unwrap();
                for p in [&full, &half] {
                    assert_eq!(p.phase_law(), fresh.phase_law());
                    assert_eq!(p.phase_law_origin(), fresh.phase_law_origin());
                    assert_eq!(p.with_normal_albedo_bvr(own), fresh);
                }
                let covering = cover.filter(|_| takes);
                if let Some(covering) = covering {
                    assert_eq!(
                        full.normal_albedo_bvr().map(bits),
                        covering.map(bits),
                        "{m:?}"
                    );
                    let blend = [0, 1, 2].map(|b| 0.5 * own[b] + 0.5 * covering[b]);
                    assert_eq!(half.normal_albedo_bvr().map(bits), blend.map(bits), "{m:?}");
                } else {
                    assert_eq!(full.normal_albedo_bvr().map(bits), own.map(bits), "{m:?}");
                    assert_eq!(half.normal_albedo_bvr().map(bits), own.map(bits), "{m:?}");
                }
            }
        }
    }

    // ---- R2: the solve ----

    /// On the weathered Moon-like world the scaled exposed materials' geometric albedo in V is p,
    /// by the solve's own figures and by the materials' own; and the solve is the as-built
    /// server's, on a palette weathered entry by entry, to the bit (the world has no `Melt` and no
    /// cover), and not the fresh solve.
    #[test]
    fn maturity_the_solve_on_the_exposed_albedos_gives_p() {
        let field = weathered_moon();
        let header = field.header();
        let palette = header.palette();
        let synth = Synthesiser::new(field, SEED);
        let fractions = class_fractions(&synth, &mut SynthCache::new()).unwrap();
        let exposure = GroundExposure::of_header(header);
        let target = [0.10, 0.12, 0.14];
        let solved = exposed_albedo_scale(&fractions, palette, exposure, target).unwrap();
        assert!(solved.relative_residual()[1].abs() < 1e-9, "{solved:?}");
        let (mut sum, mut named) = (0.0, 0.0);
        for &(ground, share) in fractions.shares() {
            let Some(m) = ground.material() else {
                continue;
            };
            named += share;
            let p = exposed_properties(m, palette, exposure).unwrap();
            let v = scaled_normal_albedo(&p, solved.scale())[1];
            sum += share * v * (2.0 + p.phase_law().limb_parameter(v)) / 3.0;
        }
        let p_v = sum / named;
        assert!(
            (p_v / target[1] - 1.0).abs() < 1e-12,
            "{p_v} against {}",
            target[1]
        );
        let weathered = MaterialPalette::new(
            palette
                .entries()
                .iter()
                .map(|e| PaletteEntry {
                    normal_albedo_bvr: weathered_entry_bvr(e, 1.0),
                    ..e.clone()
                })
                .collect(),
        )
        .unwrap();
        let as_built = albedo_scale(&fractions, &weathered, target).unwrap();
        assert_eq!(solved, as_built);
        assert_same_bits(solved.scale(), as_built.scale());
        let fresh = albedo_scale(&fractions, palette, target).unwrap();
        assert!(
            solved.scale() > fresh.scale(),
            "{solved:?} against {fresh:?}"
        );
    }

    // ---- R4: the shares, Grier et al. 2000's count and the mean V excess ----

    /// The deficits `m_body` − `m_local` the shares are counted above.
    const DEFICITS: [f64; 3] = [0.05, 0.2, 0.5];

    /// The points each field is sampled at, uniform in area.
    const SAMPLES: u32 = 10_000;

    /// A field's sampled maturities: the shares of its ground above each of [`DEFICITS`], and the
    /// mean V excess of its named ground's exposed albedo at `m_local` over its albedo at `m_body`.
    #[derive(Debug)]
    struct Sampled {
        above: [f64; 3],
        mean_v_excess: f64,
    }

    /// [`SAMPLES`] points of `field`, uniform in area on its near-spherical figure.
    fn sample(field: &CoarseField) -> Sampled {
        let header = field.header();
        let palette = header.palette();
        let synth = Synthesiser::new(field, SEED);
        let mut cache = SynthCache::new();
        let mut rng = Lcg::new(0x6d61_7475_7269_7479);
        let body = header.maturity().unwrap();
        let exposure = GroundExposure::of_header(header);
        let mut above = [0_u32; 3];
        let (mut excess, mut named) = (0.0, 0_u32);
        for _ in 0..SAMPLES {
            let z = 2.0 * rng.next_f64() - 1.0;
            let (s, c) = math::sin_cos(core::f64::consts::TAU * rng.next_f64());
            let across = (1.0 - z * z).sqrt();
            let at = xyz_to_face_uv([across * c, across * s, z]);
            let local = maturity_at(&synth, &mut cache, at).unwrap().unwrap();
            let deficit = body - local;
            for (n, &d) in above.iter_mut().zip(&DEFICITS) {
                if deficit > d {
                    *n += 1;
                }
            }
            if let Some(m) = material_at(&synth, &mut cache, at).unwrap().material() {
                let v = |e| {
                    exposed_properties(m, palette, e)
                        .unwrap()
                        .normal_albedo_bvr()[1]
                };
                excess += v(exposure.with_maturity(local)) / v(exposure) - 1.0;
                named += 1;
            }
        }
        Sampled {
            above: above.map(|n| f64::from(n) / f64::from(SAMPLES)),
            mean_v_excess: excess / f64::from(named),
        }
    }

    /// The 1–20 km craters per km² whose rim deficit, g `f_ex` (1 − `maturity_after`(t, 1)) on
    /// mature ground, is above `threshold` on a surface `surface_age` old, from the production and
    /// the chronology: over 400 bins of D uniform in its logarithm, each bin's share of N(>1 km)
    /// times the chronology's count of craters formed in the age below which a crater's rim deficit
    /// is above the threshold, at the present flux.
    fn immature_rims_per_km2(params: &CraterParams, surface_age: Gigayears, threshold: f64) -> f64 {
        const BINS: u32 = 400;
        let above = |d_m: f64| cumulative_density(params, Metres::new(d_m)).value();
        let per_1km = above(1e3);
        let mut total = 0.0;
        for k in 0..BINS {
            let edge = |n: u32| 1e3 * math::powf(20.0, f64::from(n) / f64::from(BINS));
            let (d0, d1) = (edge(k), edge(k + 1));
            let d = (d0 * d1).sqrt();
            let deficit = |t: f64| {
                let age = Gigayears::new(t);
                relief_factor(Metres::new(0.5 * d), diffusion_length(age))
                    * excavated_share(surface_age, age, Metres::new(d))
                    * (1.0 - maturity_after(age, 1.0))
            };
            if deficit(0.0) <= threshold {
                continue;
            }
            let (mut lo, mut hi) = (0.0, surface_age.value());
            for _ in 0..60 {
                let mid = f64::midpoint(lo, hi);
                if deficit(mid) > threshold {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            let formed = lunar_chronology(Gigayears::new(lo), params.system_age()).value();
            total += (above(d0) - above(d1)) / per_1km * formed;
        }
        total
    }

    /// Grier et al. 2000's count: 327 craters with "some discernable degree of immaturity" in
    /// 2.60 × 10⁶ km², down to about 1 km (LPSC XXXI, abstract 1950, p. 1), per km².
    const GRIER_IMMATURE_PER_KM2: f64 = 327.0 / 2.60e6;

    /// R4's shares on the weathered Moon-like world, on its 3.5 Gyr variant, where the share above
    /// a deficit of 0.5 is under 10% (without `f_ex` and g it would be 57%), and on a 1 Gyr
    /// variant, printed beside the ruling's estimates, with Grier et al. 2000's count in closed
    /// form and R5b's mean V excess.
    #[test]
    #[ignore = "slow: the maturity and the material at 10^4 points of three Moon-like worlds"]
    fn maturity_immature_shares_are_held_and_grier_s_count_stated() {
        let middle = younger_moon(3.5, 4.8e-3);
        let young = younger_moon(1.0, 8.4e-4);
        for (name, field, estimates, excess) in [
            ("4.4 Gyr", weathered_moon(), "0.3% above 0.2", 0.004),
            ("3.5 Gyr", &middle, "16% above 0.2, 2.1% above 0.5", 0.18),
            ("1 Gyr", &young, "none stated", 0.20),
        ] {
            let s = sample(field);
            eprintln!(
                "{name}: shares above {DEFICITS:?}: {:.4} {:.4} {:.4} (R4's estimates: \
                 {estimates}); mean V excess {:.4} (R5b's estimate, an upper bound, {excess})",
                s.above[0], s.above[1], s.above[2], s.mean_v_excess
            );
            let params = field.header().craters();
            let surface = field.header().surface_age();
            let (above_02, above_05) = (
                immature_rims_per_km2(params, surface, 0.2),
                immature_rims_per_km2(params, surface, 0.5),
            );
            eprintln!(
                "{name}: 1–20 km craters with a rim deficit above 0.2 {above_02:.2e} km⁻², above \
                 0.5 {above_05:.2e} km⁻² (the ruling's 4.2e-4 and 1.7e-4), against Grier et al. \
                 2000's {GRIER_IMMATURE_PER_KM2:.2e}"
            );
            if name == "3.5 Gyr" {
                assert!(s.above[2] < 0.10, "{s:?}");
            }
        }
    }

    // ---- The golden: `tests/golden/material/maturity.golden` (`material/golden.rs` counts it)
    // ----

    /// `value`'s IEEE 754 bits, as `0x` and 16 hexadecimal digits.
    fn hex(value: f64) -> String {
        format!("0x{:016x}", bits(value))
    }

    /// The face coordinates of the centre of cell (`i`, `j`) of an `n` × `n` grid on face `face`.
    fn grid_point(face: Face, i: u32, j: u32, n: u32) -> FaceUv {
        let at = |k: u32| st_to_uv((f64::from(k) + 0.5) / f64::from(n));
        FaceUv {
            face,
            u: at(i),
            v: at(j),
        }
    }

    /// A line of `label` and three albedos' bits.
    fn bvr_line(out: &mut GoldenWriter, label: &str, bvr: [f64; 3]) {
        out.line(&format!(
            "{label}: {} {} {}",
            hex(bvr[0]),
            hex(bvr[1]),
            hex(bvr[2])
        ));
    }

    /// R10.T1.c's golden text: the time law and the fresh share's parts, the maturity at a point on
    /// the weathered Moon-like world and the young single crater's, every material's exposed albedo
    /// at `m_body` and two `m_local`, and the organic cover's blend on the Pluto-like world (the
    /// `material::golden` module documentation). It is written here, not in `material/golden.rs`,
    /// because the sim's tripwire (`modifiers_nothing_on_the_record_reads_the_modifiers_yet`, until
    /// P14.T54.b) reads every file of the crate but this one for the time law's names.
    fn maturity_text() -> String {
        let mut out = GoldenWriter::new();
        out.header(crate::generator_version());
        law_lines(&mut out);
        let moon = moon_like_builder()
            .maturity(1.0)
            .weathering_rate(1.0)
            .build();
        let crater = one_crater_builder(Gigayears::new(0.1), 51)
            .maturity(body_maturity(Gigayears::new(4.0), 1.0))
            .weathering_rate(1.0)
            .build();
        for (name, field) in [("moon_like", &moon), ("one_crater", &crater)] {
            point_lines(&mut out, name, field);
        }
        out.line("cover pluto_like: <class> <entry> <cover>: A_N in B, V and R");
        let pluto = pluto_like(Pascals::new(1.0));
        let palette = pluto.header().palette();
        for entry in 0..u8::try_from(palette.len()).expect("a palette holds at most 15 entries") {
            for &class in MaterialClass::ALL {
                let m = Material::new(class, entry).expect("entry is a palette index");
                for c in [0.0, 0.5, 1.0] {
                    let p =
                        exposed_properties(m, palette, GroundExposure::NONE.with_organic_cover(c))
                            .expect("in the palette");
                    bvr_line(
                        &mut out,
                        &format!("{class:?} {entry} {c}"),
                        p.normal_albedo_bvr(),
                    );
                }
            }
        }
        out.finish()
    }

    /// The maturity golden's laws: `maturity_after` on a grid of exposures and rates, and the fresh
    /// share's parts on grids of their inputs.
    fn law_lines(out: &mut GoldenWriter) {
        out.line("maturity_after: t <exposure, Gyr> k <rate>");
        for t in [
            0.0, 1e-6, 0.001, 0.05, 0.109, 0.3277, 0.81, 1.0, 3.5, 4.4, 13.8,
        ] {
            for k in [0.0, 0.5, 1.0, 8.6, 1e3] {
                out.f64(
                    &format!("t {t} k {k}"),
                    maturity_after(Gigayears::new(t), k),
                );
            }
        }
        out.line("fresh share: reworked_depth (m), excavated_share, halo_share");
        for t in [0.0, 0.001, 0.1, 1.0, 3.5, 4.4] {
            out.f64(
                &format!("reworked_depth {t}"),
                reworked_depth(Gigayears::new(t)).value(),
            );
        }
        for (t_s, t_c) in [(3.5, 0.0), (4.4, 0.1), (1.0, 0.5), (3.5, 3.5)] {
            for d in [5.0, 18.0, 50.0, 100.0, 1_000.0, 3e5] {
                out.f64(
                    &format!("excavated_share t_s {t_s} t_c {t_c} D {d}"),
                    excavated_share(Gigayears::new(t_s), Gigayears::new(t_c), Metres::new(d)),
                );
            }
        }
        for x in [
            0.0, 0.5, 1.0, 1.2, 2.0, 2.5, 4.0, 6.0, 8.0, 9.99, 10.0, 12.0,
        ] {
            for end in [1.0, 1.3, 2.543, 10.0] {
                out.f64(&format!("halo_share x {x} end {end}"), halo_share(x, end));
            }
        }
    }

    /// The maturity golden's lines for the field `field`, named `name`: its maturity at the grid's
    /// points (and the single crater's transect), then every material's exposed albedo at `m_body`
    /// and at two `m_local`.
    fn point_lines(out: &mut GoldenWriter, name: &str, field: &CoarseField) {
        out.line(&format!("maturity_at {name}: <face> <u> <v> m_local"));
        let synth = Synthesiser::new(field, SEED);
        let mut cache = SynthCache::new();
        let mut points: Vec<FaceUv> = Face::ALL
            .iter()
            .flat_map(|&face| {
                (0..8).flat_map(move |j| (0..8).map(move |i| grid_point(face, i, j, 8)))
            })
            .collect();
        if name == "one_crater" {
            // A transect from the crater's centre out past its reach, 0 to 3 rim radii.
            let radius_m = field.header().figure().equatorial_radius_m;
            let rim_m = 0.5 * field.craters()[0].diameter.value();
            points.extend((0..=24_u32).map(|k| {
                xyz_to_face_uv(along_chord(
                    one_crater_centre(),
                    rim_m * f64::from(k) / 8.0,
                    radius_m,
                ))
            }));
        }
        let mut line = String::new();
        for at in points {
            let m = maturity_at(&synth, &mut cache, at)
                .expect("a whole field holds every cell")
                .expect("the field carries a maturity");
            line.clear();
            write!(
                line,
                "{} {} {} {}",
                at.face.index(),
                hex(at.u),
                hex(at.v),
                hex(m)
            )
            .expect("a string takes any write");
            out.line(&line);
        }
        out.line(&format!(
            "exposed {name}: <class> <entry> <maturity>: A_N in B, V and R"
        ));
        let header = field.header();
        let palette = header.palette();
        if palette.is_empty() {
            out.line("  no palette entries");
        }
        let body = GroundExposure::of_header(header);
        for entry in 0..u8::try_from(palette.len()).expect("a palette holds at most 15 entries") {
            for &class in MaterialClass::ALL {
                let m = Material::new(class, entry).expect("entry is a palette index");
                for (label, exposure) in [
                    ("m_body", body),
                    ("0.3", body.with_maturity(0.3)),
                    ("0.7", body.with_maturity(0.7)),
                ] {
                    let p = exposed_properties(m, palette, exposure).expect("in the palette");
                    bvr_line(
                        out,
                        &format!("{class:?} {entry} {label}"),
                        p.normal_albedo_bvr(),
                    );
                }
            }
        }
    }

    #[test]
    fn maturity_matches_its_golden() {
        golden!("material/maturity", &maturity_text());
    }

    /// The tests that read the clock, which the browser target cannot.
    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    mod native_only {
        use std::time::Instant;

        use super::*;

        /// The point query's time: `maturity_at` per point on the three Moon-like worlds, from a
        /// fresh cache each point (a lone query) and through one cache over a level-14 patch's 65 ×
        /// 65 vertices (a bake's), printed.
        #[test]
        #[ignore = "measurement, run by hand with --nocapture: the point query's time"]
        fn maturity_at_s_time_per_point() {
            let middle = younger_moon(3.5, 4.8e-3);
            let young = younger_moon(1.0, 8.4e-4);
            for (name, field) in [
                ("4.4 Gyr", weathered_moon()),
                ("3.5 Gyr", &middle),
                ("1 Gyr", &young),
            ] {
                let synth = Synthesiser::new(field, SEED);
                let mut rng = Lcg::new(0x7469_6d65_0000_0001);
                let points: Vec<FaceUv> = (0..400)
                    .map(|_| {
                        xyz_to_face_uv(unit_dir([0, 1, 2].map(|_| 2.0 * rng.next_f64() - 1.0)))
                    })
                    .collect();
                let start = Instant::now();
                for &at in &points {
                    assert!(
                        maturity_at(&synth, &mut SynthCache::new(), at)
                            .unwrap()
                            .is_some()
                    );
                }
                let lone = start.elapsed().as_secs_f64() / 400.0;
                let patch = PatchKey::containing(14, unit_dir([0.3, 1.0, 0.2])).unwrap();
                let side = f64::from(1_u32 << patch.level());
                let mut cache = SynthCache::new();
                let start = Instant::now();
                for y in 0..=64_u32 {
                    for x in 0..=64_u32 {
                        let at = FaceUv {
                            face: patch.face(),
                            u: st_to_uv((f64::from(patch.i()) + f64::from(x) / 64.0) / side),
                            v: st_to_uv((f64::from(patch.j()) + f64::from(y) / 64.0) / side),
                        };
                        assert!(maturity_at(&synth, &mut cache, at).unwrap().is_some());
                    }
                }
                let baked = start.elapsed().as_secs_f64() / 4_225.0;
                eprintln!(
                    "{name}: maturity_at {:.1} µs a lone point, {:.1} µs a vertex of a level-14 \
                     patch",
                    1e6 * lone,
                    1e6 * baked
                );
            }
        }
    }
}
