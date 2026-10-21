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
//!   0.8 km across and 50 and 2 Ma old. The excess over the background as a share of the rim's,
//!   e(x), at 2, 4, 6, 8 and 10 R is North Ray's 0.620, 0.301, 0.145, 0.084 and 0.023 and South
//!   Ray's 0.687, 0.235, 0.132, 0.078 and 0.040 (main's re-reading of the figure from its vector
//!   form, decision-r10-t1c-halo R2, North Ray's against a background of 0.143 and South Ray's
//!   against its own floor, 0.1468; the other choice moves them by up to 0.02), reaching the
//!   background by about 10–14 R. λ = 3.0 R keeps the halo's area moment ∫₁¹⁰ e 2x dx, the
//!   integrated fresh share that a body's mean brightness counts, within 7% of both craters' (the
//!   ruling's "within 6%" is 6.5% at the top of the range): the model's 15.98 against the
//!   measured 15.5–17.1 over both backgrounds, whose own fits give 2.90–3.25 R. Labelled
//!   alternatives: least squares on e gives 2.42–2.63 R and on ln e 3.35–3.86 R. The shape's cost
//!   is a share within 0.10 of the measured one at 2–8 R. p is 1 inside the rim, x ≤ 1, and for a
//!   small crater (e^(−(x − 1) ÷ 3) − e^(−3)) ÷ (1 − e^(−3)) out to 10 R
//!   ([`HALO_LENGTH_RIM_RADII`], [`HALO_EXTENT_RIM_RADII`]), 0 beyond, which drops the profiles'
//!   10–15 R tail, 4–20% of their moment. A coarse crater's is shifted to 0 at its reach (1 + a) R
//!   instead, since the field carries a coarse crater only on the cells its reach touches, and an
//!   answer must not depend on which blocks a client holds. Labelled: λ = 3 R and 10 R are two
//!   lunar 1 km craters', taken for every size and body, and the OMAT excess stands in for the
//!   fresh share. For context, the continuous ejecta reach `R_ce` =
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
//!   crater's [`relief_factor`] of its rim radius and its wear ([`SmallCrater::wear`]: the larger
//!   of its diffusion and its octave's equilibrium closure, R09.T7.j), a coarse crater's
//!   [`kept_relief`](crate::synth::craters::rims::kept_relief), one less its degradation, which the
//!   coarse pass sets by the same law. Tycho keeps g ≈ 1 (κt ÷ R² ≈ 3 × 10⁻⁷, and its octave's
//!   closure `K_v` q of order 10⁻⁴ on a saturated lunar surface), so `τ_L`'s calibration is
//!   untouched; a 25 m crater keeps 21% of its relief after 5 Myr by κ.
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
//! # A patch's layer
//!
//! The client draws the maturity a patch's vertices carry (R10.T1.c's client half,
//! decision-r10-t1c-client-scope §1.3 and §1.4). Band level n resolves the coarse craters and each
//! small-crater octave whose smallest kept diameter is at least ½ `λ_n` ([`maturity_cut`],
//! [`maturity_keeps_octave`], [`MATURITY_CUT_SHARE`]), so the halo of a crater too small to show as
//! relief is drawn, and the client takes the finer octaves at their expectation; from the band
//! limit's level every octave is kept. [`maturity_at_level`] is the point rule over the octaves a
//! level keeps, [`maturity_at`]'s with every octave, and [`patch_maturity`] gives it at every vertex
//! of a patch, to the bit, at the patch's level and its parent's, which keeps the same octaves less
//! the finest, so that the client morphs it as the heights morph. It gathers once for the patch
//! what each vertex's search would repeat, about 2,000 cells and up to 1,300 craters at level 14,
//! some 4,000 times: each octave's cells under the patch once, each crater's terms that do not
//! depend on the point once, and then each vertex's largest deficit over the craters listed for its
//! block of the patch, in falling rim deficit to an exact exit. That costs about 1 ms a warm patch
//! where the point rule at each vertex costs 0.7–0.8 s (provisional).
//!
//! # The body's maturity mix
//!
//! A body drawn at each point's `m_local` is brighter than one drawn at `m_body`, so the server's
//! albedo scale counts the haloes (decision-r10-t1c-halo R1): [`maturity_mix`] gives the
//! distribution of the point deficit D = `m_body` − `m_local` over the body's ground in closed
//! form, the exact expectation of [`maturity_at`]'s rule, and cuts it at nodes for the solve
//! ([`haloed_albedo_scale`](super::haloed_albedo_scale)).
//!
//! - **The small craters.** Each octave's craters are a Poisson field (`synth/craters.rs`'s draw),
//!   and under the largest-deficit rule a point's deficit is at most d exactly where no crater's
//!   exceeds d, so over the body P(D ≤ d) = exp(−`Λ_s`(d)), `Λ_s`(d) the expected number of small
//!   craters whose deficit at a point exceeds d. A crater of rim deficit δ₀ = g `f_ex` max(0,
//!   `m_body` − `maturity_after`(`t_c`, k)) exceeds d < δ₀ within x*(d ÷ δ₀) rim radii
//!   ([`halo_reach`]): an area of π (R x*)², exact on a sphere, where a cap within a chord c has
//!   the area π c² (Archimedes: the cap's area is 2π R h, and c² = 2 R h), and nearly so on a
//!   spheroid of flattening 10⁻³ (labelled). So `Λ_s`(d) sums, over the octaves that can expose
//!   fresh ground from the largest, the kept density per m² times the mean of π (R x*)² over the
//!   octave's diameters and ages where d < δ₀. The diameters are the eight Gauss–Legendre nodes
//!   on the octave's kept share through its own inversion, as its closed forms take them; the ages
//!   are the draw's, `chronology_age`(u × `N_w`, A) of a uniform u at the window's density `N_w`
//!   ([`OctaveWear::window_density`](crate::synth::craters::OctaveWear::window_density)), by
//!   eight-point Gauss–Legendre on 24 panels of u graded towards 0, [0, 2⁻²³] and then halvings up
//!   to [½, 1], which resolve the young end, where g falls on the scale b R² ÷ κ (27 kyr for a 2 m
//!   crater). Each age node's g reads its wear in the octave ([`CraterWear::of_age`]: the share q
//!   = u at the node on a saturated octave and u P ÷ S on an unsaturated one, R09.T7.j), as the
//!   draw's craters do.
//! - **The coarse craters.** `F_c`(d) = max(0, 1 − Σ π (R x*)² ÷ A) over the field's list in order,
//!   each crater with its own shape, its halo ending at its reach (1 + a) and g its kept relief,
//!   and A the body's area, its cells' summed in index order as
//!   [`class_fractions`](super::class_fractions) weights the materials. 1 − Σ is exact where the
//!   haloes whose deficit exceeds d do not overlap, and coarse craters are few (labelled).
//! - **F.** The small craters are drawn independently of the coarse list, so F(d) =
//!   `F_c`(d) exp(−`Λ_s`(d)) below `m_body`, and 1 from it. At d = 0 it is its limit from above,
//!   every halo of a positive δ₀ whole: the ground in none, an atom at `m_body`.
//! - **The nodes.** The deficit is cut at 52 edges, d = `m_body` e for e = 0, then 2⁻²⁴, 2⁻²³, …,
//!   2⁻⁵ by halvings, then 2/32, 3/32, …, 32/32: on old ground nearly all the probability lies
//!   below 1/64, where 64 uniform bins miss the excess by 53–69% (main's check). The atom goes to
//!   the first node, and each bin (a, b]'s mass P = F(b) − F(a) is shared between its edges so
//!   that its mean deficit, (b F(b) − a F(a) − ∫ₐᵇ F) ÷ P, is kept: exact for an albedo linear on
//!   the bin with the bin's true mean, and to Simpson's rule for the integral here, which a jump
//!   of F inside a bin (a crater's floor, an atom at its δ₀) makes approximate; the convergence
//!   test bounds it. That is 103 evaluations of F. Each node's
//!   maturity is `m_body` less its edge, and the zero weights are dropped. Main's replica of the
//!   scheme was within 0.36% of a 4,000-bin reference's excess; the convergence test holds it to
//!   1%, and its doublings of the diameters, the ages and the bins likewise.
//!
//! Labelled, and deferred (plan R10's Risks): the mix leaves out the material–crater correlation
//! (crater interiors' `Bedrock`, a young coarse crater's own terrain); the realised small craters'
//! shot noise about the expectation, by main's estimate at most 0.15% of p on the testing rows at
//! 1–3.5 Gyr and 0.26% on R10.T1.d's highland row (Campbell's theorem, overlaps ignored); the
//! coarse haloes' overlaps; and the spheroid's area within a chord, taken as π c². It costs some
//! tens of milliseconds a body, where 10⁴ sampled points of [`maturity_at`] would take 30–50 s.
//!
//! # Determinism
//!
//! Pure `f64` arithmetic and `hyperion_base::math` ([`math::exp`], [`math::exp_m1`],
//! [`math::ln_1p`], [`math::powf`]); no draw, so no domain tag. The result at zero exposure or zero
//! rate is +0, never −0, as the field header's reserved `maturity` part requires. Every comparison
//! is explicit, never `f64::min` or `max`, and the deficits are compared, never summed. The mix's
//! sums run in a fixed order from +0 (the octaves from the largest, each one's diameter nodes and
//! then its age nodes in order, the coarse list in order, the bins in order), and it holds no
//! cache: it is a function of the header and the coarse list alone.
//!
//! # Not on the record yet
//!
//! Nothing on the record calls this module's time law at generator version 22: P14.T54.b (23 → 24)
//! fills the header's `maturity` from P14.T51.d's `hooks/modifiers.rs`, which calls it, and deletes
//! the sim's tripwire (`modifiers_nothing_on_the_record_reads_the_modifiers_yet`) that holds the
//! line. No header outside the `testing` builder carries a maturity, a rate or a cover before it,
//! so every exposure the server and the worker take is [`GroundExposure::NONE`]'s, every mix the
//! server asks for is `None`, and every albedo and answer is as it was. The point maturity is
//! presentation (plan R10's Generator version); the solve's per-form rule (`Melt`, the cover) and
//! its haloes reach the header's generator-versioned `albedo_scale` at 24.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use hyperion_base::math;
use hyperion_base::units::{Gigayears, Metres};

use super::classify::organic_cover_entry;
use super::weathering::check_unit;
use super::{
    Material, MaterialClass, MaterialProperties, ResolvePropertiesError, properties,
    weathered_entry_bvr,
};
use crate::craters::density::GAUSS_LEGENDRE_8;
use crate::craters::{UnitUniform, chronology_age, crater_shape, diameter_in_octave};
use crate::cube::{
    Edge, Face, FaceUv, PatchKey, face_uv_to_xyz, st_to_uv, unit_dir, xyz_to_face_uv,
};
use crate::field::{
    CoarseField, FieldHeader, FieldView, MaterialPalette, PaletteEntry, PaletteRole, cell_at_index,
};
use crate::patch::PATCH_VERTICES;
use crate::spheroid::Spheroid;
use crate::substance_key::{SubstanceKey, SubstanceKeyForm};
use crate::synth::craters::rims::kept_relief;
use crate::synth::craters::{
    CraterCache, CraterOctave, CraterWear, SmallCrater, cell_area, relief_factor,
};
use crate::synth::levels::shortest_wavelength;
use crate::synth::{BandLevel, QueryHeightError, SmallCraters, SynthCache, Synthesiser};

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

/// The e-folding length of a young crater's fresh halo, rim radii: 3, the length that keeps the
/// halo's area moment, the integrated fresh share a body's mean brightness counts, within 7% of
/// both craters' that Grier et al. 2000 profile (LPSC XXXI, abstract 1950, Fig. 3: the radially
/// averaged optical maturity about North Ray and South Ray; decision-r10-t1c-halo R2, from main's
/// re-reading of the figure, whose "within 6%" is 6.5% at the top of the measured range).
///
/// The moment ∫₁¹⁰ e(x) 2x dx of the excess over the background as a share of the rim's, e(x), is
/// 15.5–17.1 measured, over both craters and both backgrounds, and the code's shifted form
/// ([`halo_share`]) gives 15.98 at λ = 3.0, the moment's fits giving 2.90–3.25 R. Labelled
/// alternatives: least squares on e gives 2.42–2.63 R, which fits the near-rim points but loses 18%
/// of the moment at 2.45 R, and least squares on ln e 3.35–3.86 R. The cost is the shape: within
/// 0.10 of the measured share at 2–8 R (p(2) 0.702, p(4) 0.335, p(8) 0.050). Labelled too: two
/// lunar 1 km craters' figure, taken for every size and body.
pub const HALO_LENGTH_RIM_RADII: f64 = 3.0;

/// How far a small crater's fresh halo reaches, rim radii: 10, where Grier et al. 2000's profiles
/// reach the background, "by about 10–14 R" (Fig. 3, as the ruling reads it; North Ray's excess is
/// about 0.025 of its rim's at 10 R), [`halo_share`]'s 0.
///
/// Labelled, as [`HALO_LENGTH_RIM_RADII`] is: the 10 R end drops the profiles' 10–15 R tail, 4–20%
/// of their measured moment (decision-r10-t1c-halo R2). A coarse crater's halo ends at its own
/// reach instead, (1 + a) rim radii, the farthest the field carries it.
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
    halo_with_tail(x, end, || halo_tail(end))
}

/// The halo's tail for a halo that ends at `end` rim radii, e^(−(x₁ − 1) ÷ λ) − 1 by expm1: the
/// term [`halo_share`] shifts its exponential by, the same for every point of one halo, so that a
/// patch's gather ([`patch_maturity`]) takes it once a crater.
#[must_use]
fn halo_tail(end: f64) -> f64 {
    math::exp_m1(-(end - 1.0) / HALO_LENGTH_RIM_RADII)
}

/// [`halo_share`] of `x` rim radii for a halo that ends at `end`, unchecked, its tail
/// ([`halo_tail`] of `end`) from `tail` where the point lies between the rim and the end: the one
/// expression that [`halo_share`] and a patch's gather both evaluate, so that the two agree to the
/// bit.
#[must_use]
fn halo_with_tail(x: f64, end: f64, tail: impl FnOnce() -> f64) -> f64 {
    if x <= 1.0 {
        return 1.0;
    }
    if x >= end {
        return 0.0;
    }
    // Both terms less one, by expm1, so that a halo ending near the rim keeps its precision; the
    // quotient's rounding may not carry it past either end of [0, 1].
    let tail = tail();
    let here = math::exp_m1(-(x - 1.0) / HALO_LENGTH_RIM_RADII);
    ((here - tail) / -tail).clamp(0.0, 1.0)
}

/// The distance from a crater's centre at which its fresh share falls to `share`, rim radii, for a
/// halo that ends at `outer_rim_radii`: [`halo_share`]'s inverse, x* = 1 − λ ln(1 − (1 − s)(1 − T))
/// with T = e^(−(x₁ − 1) ÷ λ), by `ln_1p` (decision-r10-t1c-halo R1).
///
/// It is 1 at a share of 1, the rim, and tends to the halo's end as the share falls to 0. The
/// area within it, π (R x*)² on a sphere (Archimedes: a cap within a chord c has the area π c²),
/// is the ground where the crater's deficit exceeds `share` times its rim's, which the body's
/// maturity mix ([`maturity_mix`]) counts.
///
/// # Panics
///
/// If the share is outside (0, 1] or NaN, or the end is not finite or is below 1.
#[must_use]
pub fn halo_reach(share: f64, outer_rim_radii: f64) -> f64 {
    let end = outer_rim_radii;
    assert!(
        share > 0.0 && share <= 1.0,
        "a halo's share must be in (0, 1], got {share}"
    );
    assert!(
        end.is_finite() && end >= 1.0,
        "a halo must end finite and at or beyond the rim, got {end} rim radii"
    );
    reach_at(share, halo_opening(end))
}

/// 1 − T = −expm1(−(x₁ − 1) ÷ λ) for a halo that ends at `end` rim radii: the share of the rim's
/// excess the halo's exponential loses by its end, before [`halo_share`]'s shift.
#[must_use]
fn halo_opening(end: f64) -> f64 {
    -math::exp_m1(-(end - 1.0) / HALO_LENGTH_RIM_RADII)
}

/// [`halo_reach`] of `share` in [0, 1] for a halo of opening `opening` ([`halo_opening`]),
/// unchecked: its end at a share of 0, where every halo of a positive rim deficit counts whole.
#[must_use]
fn reach_at(share: f64, opening: f64) -> f64 {
    1.0 - HALO_LENGTH_RIM_RADII * math::ln_1p(-(1.0 - share) * opening)
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
    point_maturity(synth, cache, || unit_dir(face_uv_to_xyz(at)), Kept::Every)
}

/// The share of a band level's shortest wavelength `λ_n` at which its maturity is cut: 0.5, so
/// that band level n draws one by one the fresh haloes of the small craters whose smallest kept
/// diameter is at least ½ `λ_n` ([`maturity_cut`]; decision-r10-t1c-client-scope §1.4).
///
/// The height keeps an octave from `λ_n` itself (R09's [`LevelSet`](crate::synth::LevelSet)), and
/// a halo is ten times its crater's diameter across, so the cut draws the bright halo of a crater
/// too small to resolve as relief, as fresh small craters show. Above the band limit, where `λ_n`
/// is 4 s with s the level's largest vertex spacing, a kept crater's rim radius is at least s: its
/// halo of [`HALO_EXTENT_RIM_RADII`] spans at least 20 vertex spacings, its rim's interior at least
/// two and its e-fold of [`HALO_LENGTH_RIM_RADII`] at least three. The ruling allows ¼ to 1;
/// presentation alone, so a change moves no generator-versioned output.
pub const MATURITY_CUT_SHARE: f64 = 0.5;

/// The smallest small-crater diameter whose fresh halo band level `level` draws crater by crater
/// on the figure `figure`, metres: [`MATURITY_CUT_SHARE`] of the level's shortest wavelength
/// ([`shortest_wavelength`]), ½ `λ_n` (decision-r10-t1c-client-scope §1.4).
///
/// It is 1 m from the band limit's level on, where every octave, its craters from 2 m, is kept, and
/// doubles with each level coarser: on the Moon-like world it keeps 13 octaves at level 14, 9 at
/// level 10, 5 at level 6, 3 at level 4 and none at level 0. It never rises as the level deepens,
/// so every octave a level keeps ([`maturity_keeps_octave`]) its children keep.
/// The octaves a level keeps are drawn exactly at each point ([`maturity_at_level`],
/// [`patch_maturity`]); the client draws the rest at their expectation, and the coarse craters are
/// always drawn.
#[must_use]
pub fn maturity_cut(figure: &Spheroid, level: BandLevel) -> Metres {
    shortest_wavelength(figure, level) * MATURITY_CUT_SHARE
}

/// Whether a level of maturity cut `cut` ([`maturity_cut`], metres) draws the small craters of
/// `octave` one by one: its smallest kept diameter ([`CraterOctave::smallest`]) is at least the
/// cut. The octaves fall in diameter from the largest, so a level keeps the first of them.
#[must_use]
pub fn maturity_keeps_octave(octave: &CraterOctave, cut: Metres) -> bool {
    octave.smallest().value() >= cut.value()
}

/// The maturity at `at` as band level `level` draws its resolved craters, dimensionless in [0,
/// `m_body`], or `None` where the header carries no maturity or no rate: [`maturity_at`]'s rule over
/// every coarse crater and the small craters of the octaves the level's cut keeps
/// ([`maturity_cut`], [`maturity_keeps_octave`]; decision-r10-t1c-client-scope §1.4).
///
/// From the band limit's level on, where the cut keeps every octave, it is [`maturity_at`] to the
/// bit. A patch's maturity layer is its value at the patch's vertices ([`patch_maturity`]), and
/// the octaves it leaves out the client draws at their expectation. It is presentation, as
/// [`maturity_at`] is.
///
/// # Errors
///
/// As [`maturity_at`]: [`QueryHeightError::NotSurveyed`] naming the coarse cell holding the point
/// where the view does not hold it.
///
/// # Panics
///
/// As [`maturity_at`].
pub fn maturity_at_level<F: FieldView>(
    synth: &Synthesiser<'_, F>,
    cache: &mut SynthCache,
    at: FaceUv,
    level: BandLevel,
) -> Result<Option<f64>, QueryHeightError> {
    let cut = maturity_cut(&synth.field().header().figure(), level);
    point_maturity(
        synth,
        cache,
        || unit_dir(face_uv_to_xyz(at)),
        Kept::From(cut),
    )
}

/// Which small-crater octaves a maturity draws crater by crater.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Kept {
    /// Every octave: [`maturity_at`]'s rule.
    Every,
    /// The octaves a level of this cut keeps ([`maturity_keeps_octave`]).
    From(Metres),
}

impl Kept {
    /// Whether `octave` is drawn.
    #[must_use]
    fn keeps(self, octave: &CraterOctave) -> bool {
        match self {
            Self::Every => true,
            Self::From(cut) => maturity_keeps_octave(octave, cut),
        }
    }
}

/// [`maturity_at`]'s rule at the unit direction `dir` gives, over the octaves `kept` keeps: the one
/// implementation of [`maturity_at`] and [`maturity_at_level`], and the reference a patch's gather
/// ([`patch_maturity`]) is held to the bit against. The direction is made only where the header
/// carries a maturity, as [`maturity_at`] always made it.
///
/// # Errors
///
/// As [`maturity_at`].
fn point_maturity<F: FieldView>(
    synth: &Synthesiser<'_, F>,
    cache: &mut SynthCache,
    dir: impl FnOnce() -> [f64; 3],
    kept: Kept,
) -> Result<Option<f64>, QueryHeightError> {
    let field = synth.field();
    let header = field.header();
    let (Some(body), Some(rate)) = (header.maturity(), header.weathering_rate()) else {
        return Ok(None);
    };
    let dir = dir();
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
        if !can_expose(octave, surface_age) || !kept.keeps(octave) {
            continue;
        }
        each_small_crater_near(
            craters,
            octave,
            (dir, point),
            smallest_radius_m,
            cache.craters_mut(),
            |crater, x| {
                let share = small_crater_wear(crater.diameter() * 0.5, crater.wear())
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

/// The wear g of a small crater of rim radius `rim_radius` and wear `wear`, in [0, 1]: its kept
/// relief as the synthesis computes it, [`relief_factor`] of its rim radius and its wear, the
/// larger of its diffusion and its octave's equilibrium closure (R09.T7.j). [`maturity_at`] gives
/// it each crater's own [`SmallCrater::wear`], and the mix each age node's,
/// [`CraterWear::of_age`] in the node's octave, which is q = u at the node's draw share u on a
/// saturated octave and u P ÷ S on an unsaturated one, as the draw's are.
///
/// The one helper that [`maturity_at`] and the body's maturity mix ([`maturity_mix`]) both take a
/// small crater's g from, so that the two cannot disagree and one change moves both
/// (decision-r10-t1c-halo R3).
#[must_use]
fn small_crater_wear(rim_radius: Metres, wear: CraterWear) -> f64 {
    relief_factor(rim_radius, wear)
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

/// The bins a side into which [`patch_maturity`] sorts a patch's vertices: 8, each of 8 quads a
/// side (the last row and column of bins 9 vertices wide), each listing the small craters whose
/// halo can reach one of its vertices, so that a vertex tests some 50–100 craters rather than the
/// patch's 400–600 (decision-r10-t1c-client-scope §1.1: the probe's best of 4, 8 and 16 a side).
const MATURITY_BINS: usize = 8;

/// The relative margin by which a bin's reach is widened before a crater is listed for it: 10⁻⁹,
/// far above the few roundings of a chord, each relative to the chord itself, so that no crater
/// whose halo holds a vertex of the bin is left out of its list.
const BIN_MARGIN: f64 = 1e-9;

/// Each vertex's maturity on the patch `key` at its band level n and at its parent's, n − 1 (level
/// 0's at level 0 for both), dimensionless in [0, `m_body`], row by row (y outer, x inner): the
/// point rule over the octaves each level keeps ([`maturity_at_level`]) to the bit at the
/// direction `vertices` gives a vertex, `m_body` for both at a vertex it gives none, or `None`
/// where the header carries no maturity or no rate (decision-r10-t1c-client-scope §1.3).
///
/// `vertices` holds the patch's 65 × 65 vertices' unit directions as the bake makes them
/// ([`PatchKey::vertex_dir`]), each where the caller takes the layer: the bake's vertices with
/// weights. The values are the point rule's at those directions; the patch gives the level and
/// the bins the vertices share.
///
/// It is the point rule gathered once a patch, where each vertex's own search would repeat nearly
/// the same 2,000 cells and 600–1,300 craters (decision-r10-t1c-client-scope §1.1):
///
/// 1. each octave the level keeps and that can expose fresh ground searches the cells two levels
///    above its own that hold the vertices, a rectangle of indices on each face, and their 3 × 3
///    neighbourhoods across the face edges, drawing each of their descendants at its level through
///    `cache` unless it lies beyond the octave's largest halo from every vertex, the reach widened
///    by the vertices' spread about the patch's centre (`is_beyond`);
/// 2. each crater's terms that do not depend on the point, g × `f_ex` and `m_body` − `m_c`, are
///    taken once, and a crater that leaves no deficit anywhere, g × `f_ex` of 0 or ejecta no
///    fresher than `m_body`, is dropped;
/// 3. each coarse cell's craters are resolved once;
/// 4. each vertex takes the largest deficit over its coarse cell's craters and over the small
///    craters listed for its bin of 8 × 8 quads, in falling rim deficit g `f_ex` (`m_body` −
///    `m_c`), which bounds a crater's deficit anywhere, up to the first that cannot pass the
///    largest so far: an exact exit.
///
/// Level n − 1 keeps the octaves level n keeps less its finest, so its value is a second largest
/// over the same craters. Every comparison is explicit, the deficits are compared and never
/// summed, and each term is grouped as [`maturity_at`] groups it, so no order of the search moves
/// a bit. It draws nothing of its own and no value depends on what `cache` holds.
///
/// # Errors
///
/// [`QueryHeightError::NotSurveyed`] naming the coarse cell holding the first vertex given a
/// direction, in row order, where the view does not hold it, as [`maturity_at`] refuses it.
///
/// # Panics
///
/// If `vertices` does not hold a patch's 4,225 vertices; if a direction is zero or not finite, as
/// [`PatchKey::containing`]; and as [`SmallCraters::craters_in`], if a crater drawn has an
/// exterior wider than its search's bound (a bug).
pub fn patch_maturity<F: FieldView>(
    synth: &Synthesiser<'_, F>,
    cache: &mut SynthCache,
    key: PatchKey,
    vertices: &[Option<[f64; 3]>],
) -> Result<Option<Vec<[f64; 2]>>, QueryHeightError> {
    let figure = synth.field().header().figure();
    let cut = |level: u8| {
        let level = BandLevel::new(level).expect("a patch's level is a band level");
        Kept::From(maturity_cut(&figure, level))
    };
    gather_patch(
        synth,
        cache,
        key,
        vertices,
        (cut(key.level()), cut(key.level().saturating_sub(1))),
    )
}

/// [`patch_maturity`] over the octaves `kept` keeps, each vertex's second value over those
/// `parent` keeps, which keeps none that `kept` does not.
///
/// # Errors
///
/// As [`patch_maturity`].
fn gather_patch<F: FieldView>(
    synth: &Synthesiser<'_, F>,
    cache: &mut SynthCache,
    key: PatchKey,
    vertices: &[Option<[f64; 3]>],
    (kept, parent): (Kept, Kept),
) -> Result<Option<Vec<[f64; 2]>>, QueryHeightError> {
    let count = PATCH_VERTICES * PATCH_VERTICES;
    assert!(
        vertices.len() == count,
        "a patch's maturity takes its {count} vertices, got {}",
        vertices.len()
    );
    let field = synth.field();
    let header = field.header();
    let (Some(body), Some(rate)) = (header.maturity(), header.weathering_rate()) else {
        return Ok(None);
    };
    let figure = header.figure();
    let middle = u8::try_from(PATCH_VERTICES / 2).expect("a patch's middle vertex index fits a u8");
    let centre = key.vertex_dir(middle, middle);
    let mut points = vec![[0.0; 3]; count];
    let mut coarse_of = vec![0_usize; count];
    let mut coarse = CoarseLists::default();
    let mut spans = FaceSpans::default();
    let mut farthest = 0.0;
    for (n, vertex) in vertices.iter().enumerate() {
        let Some(dir) = *vertex else {
            continue;
        };
        points[n] = figure.point(dir);
        coarse_of[n] = coarse.slot(field, dir, (body, rate))?;
        spans.include(dir);
        let apart = squared_chord(centre, dir);
        if apart > farthest {
            farthest = apart;
        }
    }
    let mut out = vec![[body, body]; count];
    if spans.is_empty() {
        return Ok(Some(out));
    }
    // The angle from the centre beyond which no vertex lies, widened as a halo's reach is.
    let spread = angle_of_chord(farthest.sqrt()) * (1.0 + BEYOND_MARGIN) + BEYOND_FLOOR_RAD;
    let ground = PatchGround {
        body,
        rate,
        surface_age: header.surface_age(),
        smallest_radius_m: if figure.polar_radius_m < figure.equatorial_radius_m {
            figure.polar_radius_m
        } else {
            figure.equatorial_radius_m
        },
    };
    let small = gather_small(
        synth.craters(),
        cache.craters_mut(),
        &spans,
        (centre, spread),
        &ground,
        (kept, parent),
    );
    let bins = bin_lists(key, &figure, vertices, &points, &small);
    let small_tail = halo_tail(HALO_EXTENT_RIM_RADII);
    for (n, vertex) in vertices.iter().enumerate() {
        if vertex.is_some() {
            let listed = bins[bin_of(n)].iter().map(|&k| &small[k]);
            let (largest, largest_parent) =
                largest_deficits(points[n], &coarse.lists[coarse_of[n]], listed, small_tail);
            out[n] = [less(body, largest), less(body, largest_parent)];
        }
    }
    Ok(Some(out))
}

/// The largest deficits at the spheroid point `point` of a patch's vertex, at the patch's level
/// and at its parent's: over the coarse craters `coarse` of its cell, drawn at both, and over the
/// small craters `listed` for its bin, in falling rim deficit, the parent's over those it keeps
/// alone. `small_tail` is the small craters' halo's tail ([`halo_tail`] of
/// [`HALO_EXTENT_RIM_RADII`]).
#[must_use]
fn largest_deficits<'c>(
    point: [f64; 3],
    coarse: &[ResolvedCrater],
    listed: impl Iterator<Item = &'c GatheredCrater>,
    small_tail: f64,
) -> (f64, f64) {
    let (mut largest, mut largest_parent) = (0.0, 0.0);
    for crater in coarse {
        let x = chord(point, crater.point_m) / crater.rim_m;
        let halo = halo_with_tail(x, crater.end, || crater.tail);
        if halo > 0.0 {
            let fresh = crater.fresh * halo;
            if fresh > 0.0 {
                let deficit = fresh * crater.gap;
                if deficit > largest {
                    largest = deficit;
                }
                if deficit > largest_parent {
                    largest_parent = deficit;
                }
            }
        }
    }
    for crater in listed {
        // No crater after this one can leave more than its rim deficit, at most this one's, and a
        // deficit that does not pass a largest moves neither.
        if largest_parent >= crater.rim_deficit {
            break;
        }
        if !crater.in_parent && largest >= crater.rim_deficit {
            continue;
        }
        let d2 = squared_chord(point, crater.point_m);
        if d2 < crater.halo2_m2 {
            let halo = halo_with_tail(d2.sqrt() / crater.rim_m, HALO_EXTENT_RIM_RADII, || {
                small_tail
            });
            let share = crater.fresh * halo;
            if share > 0.0 {
                let deficit = share * crater.gap;
                if deficit > largest {
                    largest = deficit;
                }
                if crater.in_parent && deficit > largest_parent {
                    largest_parent = deficit;
                }
            }
        }
    }
    (largest, largest_parent)
}

/// The angle between two unit vectors a chord `chord` apart, radians, as [`angle_between`] takes
/// it: 2 asin(`chord` ÷ 2), π where rounding carries the half-chord past 1.
#[must_use]
fn angle_of_chord(chord: f64) -> f64 {
    let half = 0.5 * chord;
    if half < 1.0 {
        2.0 * math::asin(half)
    } else {
        core::f64::consts::PI
    }
}

/// What a patch's gather reads of its body: the header's `m_body` and rate, its surface age, and
/// its figure's smallest radius, metres.
#[derive(Debug, Clone, Copy)]
struct PatchGround {
    body: f64,
    rate: f64,
    surface_age: Gigayears,
    smallest_radius_m: f64,
}

/// A small crater gathered once for a patch, with the terms of its deficit that do not depend on
/// the point, each as [`maturity_at`] computes it.
#[derive(Debug, Clone, Copy)]
struct GatheredCrater {
    /// Its centre's spheroid point, metres.
    point_m: [f64; 3],
    /// Its rim radius, metres.
    rim_m: f64,
    /// Its halo's reach, [`HALO_EXTENT_RIM_RADII`] rim radii, metres.
    halo_m: f64,
    /// The reach's square, square metres.
    halo2_m2: f64,
    /// Its wear g times its excavated share `f_ex`, in that order, positive, so that g × `f_ex` × p
    /// groups as [`maturity_at`]'s product does.
    fresh: f64,
    /// `m_body` less its ejecta's maturity, positive.
    gap: f64,
    /// `fresh` × `gap`: its deficit at its rim, which no share p of at most 1 rounds above.
    rim_deficit: f64,
    /// Whether the parent level keeps its octave.
    in_parent: bool,
}

/// A coarse crater that reaches a patch's coarse cell, resolved once for the patch, its terms as
/// [`maturity_at`] computes them.
#[derive(Debug, Clone, Copy)]
struct ResolvedCrater {
    /// Its centre's spheroid point, metres.
    point_m: [f64; 3],
    /// Its rim radius, metres.
    rim_m: f64,
    /// Its halo's end, its reach 1 + a, rim radii.
    end: f64,
    /// The halo's tail at its end ([`halo_tail`]).
    tail: f64,
    /// Its kept relief times its excavated share, positive.
    fresh: f64,
    /// `m_body` less its ejecta's maturity, positive.
    gap: f64,
}

/// The coarse craters that leave a deficit on a patch's coarse cells, resolved once a cell.
#[derive(Debug, Default)]
struct CoarseLists {
    slots: BTreeMap<PatchKey, usize>,
    lists: Vec<Vec<ResolvedCrater>>,
}

impl CoarseLists {
    /// The slot of the coarse cell holding the unit direction `dir`, its craters resolved on the
    /// cell's first vertex, on ground of `m_body` and rate (`body`, `rate`).
    ///
    /// # Errors
    ///
    /// [`QueryHeightError::NotSurveyed`] where `field` does not hold the cell.
    fn slot<F: FieldView>(
        &mut self,
        field: &F,
        dir: [f64; 3],
        (body, rate): (f64, f64),
    ) -> Result<usize, QueryHeightError> {
        let header = field.header();
        let cell = PatchKey::containing(header.level().get(), dir)
            .expect("a coarse level is a quadtree level");
        match self.slots.entry(cell) {
            Entry::Occupied(slot) => Ok(*slot.get()),
            Entry::Vacant(slot) => {
                if field.cell(cell).is_none() {
                    return Err(QueryHeightError::NotSurveyed(cell));
                }
                let figure = header.figure();
                let surface_age = header.surface_age();
                let mut list = Vec::new();
                for crater in field.craters_reaching(cell) {
                    let fresh = kept_relief(crater)
                        * excavated_share(surface_age, crater.age, crater.diameter);
                    if fresh > 0.0 {
                        let ejecta = maturity_after(crater.age, rate);
                        if body > ejecta {
                            let shape = crater_shape(header.craters(), crater.diameter);
                            let end = 1.0 + shape.exterior_width();
                            list.push(ResolvedCrater {
                                point_m: figure.point(crater.centre),
                                rim_m: 0.5 * shape.diameter().value(),
                                end,
                                tail: halo_tail(end),
                                fresh,
                                gap: body - ejecta,
                            });
                        }
                    }
                }
                self.lists.push(list);
                Ok(*slot.insert(self.lists.len() - 1))
            }
        }
    }
}

/// The small craters of the octaves `kept` keeps that can leave a deficit at one of a patch's
/// vertices, spread at most the angle `spread` from the unit direction `centre`, whose face
/// coordinates `spans` bounds, in falling rim deficit (ties in the search's order), each marked by
/// whether `parent` keeps its octave.
fn gather_small(
    craters: &SmallCraters,
    cache: &mut CraterCache,
    spans: &FaceSpans,
    (centre, spread): ([f64; 3], f64),
    ground: &PatchGround,
    (kept, parent): (Kept, Kept),
) -> Vec<GatheredCrater> {
    let mut gathered = Vec::new();
    for octave in craters.octaves().octaves() {
        if !can_expose(octave, ground.surface_age) || !kept.keeps(octave) {
            continue;
        }
        let in_parent = parent.keeps(octave);
        let reach_rad = reach_angle(
            HALO_EXTENT_RIM_RADII * 0.5 * octave.diameters().upper().value(),
            ground.smallest_radius_m,
        ) + spread;
        for cell in octave_cells(octave.level(), spans).cells() {
            if is_beyond(cell, centre, reach_rad) {
                continue;
            }
            for crater in craters.craters_in(octave.index(), cell, cache) {
                let fresh = small_crater_wear(crater.diameter() * 0.5, crater.wear())
                    * excavated_share(ground.surface_age, crater.age(), crater.diameter());
                if fresh > 0.0 {
                    let ejecta = maturity_after(crater.age(), ground.rate);
                    if ground.body > ejecta {
                        let rim_m = 0.5 * crater.diameter().value();
                        let halo_m = HALO_EXTENT_RIM_RADII * rim_m;
                        let gap = ground.body - ejecta;
                        gathered.push(GatheredCrater {
                            point_m: crater.point_m(),
                            rim_m,
                            halo_m,
                            halo2_m2: halo_m * halo_m,
                            fresh,
                            gap,
                            rim_deficit: fresh * gap,
                            in_parent,
                        });
                    }
                }
            }
        }
    }
    // A stable sort: the search's order breaks ties, though no order moves a value.
    gathered.sort_by(|a, b| b.rim_deficit.total_cmp(&a.rim_deficit));
    gathered
}

/// The cells of an octave at quadtree level `level` that a search from any vertex `spans` bounds
/// reads (`each_small_crater_near`): the level's every cell below [`SEARCH_LEVELS_UP`], and
/// otherwise the descendants at `level` of the 3 × 3 neighbourhoods of the cells two levels up
/// that hold a vertex, as rectangles on each face that hold them all.
#[must_use]
fn octave_cells(level: u8, spans: &FaceSpans) -> FaceRects {
    let Some(up) = level.checked_sub(SEARCH_LEVELS_UP) else {
        let last = (1_u32 << level) - 1;
        let mut every = FaceRects::new(level);
        for face in Face::ALL {
            every.include_rect(face, [0, last, 0, last]);
        }
        return every;
    };
    let homes = spans.cells_at(up);
    let last = (1_u32 << up) - 1;
    let mut near = FaceRects::new(up);
    for (face, [i0, i1, j0, j1]) in homes.rects() {
        near.include_rect(
            face,
            [
                i0.saturating_sub(1),
                (i1 + 1).min(last),
                j0.saturating_sub(1),
                (j1 + 1).min(last),
            ],
        );
        // Across the face's edges, the neighbourhoods of the cells on them.
        if i0 == 0 || j0 == 0 || i1 == last || j1 == last {
            for j in j0..=j1 {
                for i in i0..=i1 {
                    if i == 0 || j == 0 || i == last || j == last {
                        let home = PatchKey::new(face, up, i, j).expect("a held cell is a cell");
                        let (cells, n) = neighbourhood(home);
                        for &cell in &cells[..n] {
                            near.include(cell);
                        }
                    }
                }
            }
        }
    }
    near.descendants(SEARCH_LEVELS_UP)
}

/// The extremes of a patch's taken vertices' face coordinates on each face ([`xyz_to_face_uv`]):
/// the directions of least and greatest u and of least and greatest v, from which the cells of
/// any level that hold the vertices lie in a rectangle of indices on each face, since a cell's
/// index rises with u and with v ([`PatchKey::containing`]).
#[derive(Debug, Default)]
struct FaceSpans([Option<FaceSpan>; 6]);

/// One face's [`FaceSpans`]: (coordinate, direction) at the least u, the greatest u, the least v
/// and the greatest v.
#[derive(Debug, Clone, Copy)]
struct FaceSpan([(f64, [f64; 3]); 4]);

impl FaceSpans {
    /// Takes in the unit direction `dir`.
    fn include(&mut self, dir: [f64; 3]) {
        let FaceUv { face, u, v } = xyz_to_face_uv(dir);
        let slot = &mut self.0[usize::from(face.index())];
        let Some(FaceSpan([least_u, most_u, least_v, most_v])) = slot.as_mut() else {
            *slot = Some(FaceSpan([(u, dir), (u, dir), (v, dir), (v, dir)]));
            return;
        };
        if u < least_u.0 {
            *least_u = (u, dir);
        }
        if u > most_u.0 {
            *most_u = (u, dir);
        }
        if v < least_v.0 {
            *least_v = (v, dir);
        }
        if v > most_v.0 {
            *most_v = (v, dir);
        }
    }

    /// Whether no direction was taken in.
    #[must_use]
    fn is_empty(&self) -> bool {
        self.0.iter().all(Option::is_none)
    }

    /// The cells of quadtree level `level` holding the directions taken in: on each face, the
    /// rectangle from the cell of the least u and v to that of the greatest.
    #[must_use]
    fn cells_at(&self, level: u8) -> FaceRects {
        let mut rects = FaceRects::new(level);
        for (face, span) in Face::ALL.into_iter().zip(&self.0) {
            let Some(FaceSpan([least_u, most_u, least_v, most_v])) = span else {
                continue;
            };
            let at = |dir: [f64; 3]| {
                let cell =
                    PatchKey::containing(level, dir).expect("a level of a search is a level");
                debug_assert_eq!(cell.face(), face, "a direction's cell is on its face");
                cell
            };
            rects.include_rect(
                face,
                [
                    at(least_u.1).i(),
                    at(most_u.1).i(),
                    at(least_v.1).j(),
                    at(most_v.1).j(),
                ],
            );
        }
        rects
    }
}

/// A set of cells of one quadtree level held as a rectangle of indices on each face, (least i,
/// greatest i, least j, greatest j), or none.
#[derive(Debug, Clone, Copy)]
struct FaceRects {
    level: u8,
    rects: [Option<[u32; 4]>; 6],
}

impl FaceRects {
    /// No cell of `level`.
    #[must_use]
    const fn new(level: u8) -> Self {
        Self {
            level,
            rects: [None; 6],
        }
    }

    /// Widens `face`'s rectangle to hold the rectangle `rect`.
    fn include_rect(&mut self, face: Face, [i0, i1, j0, j1]: [u32; 4]) {
        let slot = &mut self.rects[usize::from(face.index())];
        *slot = Some(match *slot {
            None => [i0, i1, j0, j1],
            Some([a0, a1, b0, b1]) => [a0.min(i0), a1.max(i1), b0.min(j0), b1.max(j1)],
        });
    }

    /// Widens its face's rectangle to hold `cell`, a cell of its level.
    fn include(&mut self, cell: PatchKey) {
        debug_assert_eq!(cell.level(), self.level, "a cell of the set's level");
        let (i, j) = (cell.i(), cell.j());
        self.include_rect(cell.face(), [i, i, j, j]);
    }

    /// Each face's rectangle, in face order.
    fn rects(&self) -> impl Iterator<Item = (Face, [u32; 4])> {
        Face::ALL
            .into_iter()
            .zip(self.rects)
            .filter_map(|(face, rect)| rect.map(|r| (face, r)))
    }

    /// The descendants `down` levels below of every cell held.
    #[must_use]
    fn descendants(&self, down: u8) -> Self {
        let side = 1_u32 << down;
        let mut below = Self::new(self.level + down);
        for (face, [i0, i1, j0, j1]) in self.rects() {
            below.include_rect(
                face,
                [
                    side * i0,
                    side * i1 + side - 1,
                    side * j0,
                    side * j1 + side - 1,
                ],
            );
        }
        below
    }

    /// Every cell held, face by face, row by row.
    fn cells(&self) -> impl Iterator<Item = PatchKey> {
        let level = self.level;
        self.rects().flat_map(move |(face, [i0, i1, j0, j1])| {
            (j0..=j1).flat_map(move |j| {
                (i0..=i1)
                    .map(move |i| PatchKey::new(face, level, i, j).expect("a held cell is a cell"))
            })
        })
    }
}

/// The bin ([`MATURITY_BINS`]) of a patch's vertex `n`, row by row, its index row by row.
#[must_use]
fn bin_of(n: usize) -> usize {
    let per = (PATCH_VERTICES - 1) / MATURITY_BINS;
    let along = |k: usize| (k / per).min(MATURITY_BINS - 1);
    along(n / PATCH_VERTICES) * MATURITY_BINS + along(n % PATCH_VERTICES)
}

/// Each bin's list of the craters of `small` (by index, in their order) whose halo can hold one of
/// its vertices that `vertices` gives a direction, of spheroid points `points`: those within their
/// halo's reach plus the bin's radius, the farthest of its vertices from its middle vertex
/// (`key`'s, on `figure`), widened by [`BIN_MARGIN`]; empty for a bin with none.
#[must_use]
fn bin_lists(
    key: PatchKey,
    figure: &Spheroid,
    vertices: &[Option<[f64; 3]>],
    points: &[[f64; 3]],
    small: &[GatheredCrater],
) -> Vec<Vec<usize>> {
    let bins = MATURITY_BINS * MATURITY_BINS;
    let per = (PATCH_VERTICES - 1) / MATURITY_BINS;
    let middle = |b: usize| {
        u8::try_from(per * b + per / 2).expect("a vertex index along a side is at most 64")
    };
    let middles: Vec<[f64; 3]> = (0..bins)
        .map(|b| figure.point(key.vertex_dir(middle(b % MATURITY_BINS), middle(b / MATURITY_BINS))))
        .collect();
    let mut radii2: Vec<Option<f64>> = vec![None; bins];
    for (n, vertex) in vertices.iter().enumerate() {
        if vertex.is_some() {
            let b = bin_of(n);
            let d2 = squared_chord(middles[b], points[n]);
            radii2[b] = Some(match radii2[b] {
                Some(r2) if r2 >= d2 => r2,
                Some(_) | None => d2,
            });
        }
    }
    middles
        .iter()
        .zip(radii2)
        .map(|(&middle, r2)| {
            let Some(r2) = r2 else {
                return Vec::new();
            };
            let radius_m = r2.sqrt();
            small
                .iter()
                .enumerate()
                .filter(|(_, crater)| {
                    let reach_m = (crater.halo_m + radius_m) * (1.0 + BIN_MARGIN);
                    squared_chord(middle, crater.point_m) < reach_m * reach_m
                })
                .map(|(k, _)| k)
                .collect()
        })
        .collect()
}

/// The halvings below `m_body` that the mix's finest deficit edge lies at: 24, so that the edges
/// run 2⁻²⁴ `m_body`, 2⁻²³ `m_body`, … up to 2⁻⁵ `m_body` (decision-r10-t1c-halo R1).
const FINEST_DEFICIT_HALVINGS: u32 = 24;

/// The halvings below `m_body` of the last halved deficit edge, 2⁻⁵ `m_body`, from which the edges
/// step by 1/32 of `m_body` to `m_body` itself.
const COARSEST_DEFICIT_HALVINGS: u32 = 5;

/// The steps of `m_body` the mix's edges take from 2⁻⁵ `m_body` on: 32, edges at 2/32 to 32/32.
const DEFICIT_STEPS: u32 = 32;

/// The panels of the mix's mean over a crater's age share u, graded towards u = 0: 24, [0, 2⁻²³],
/// then [2⁻ᵏ, 2⁻ᵏ⁺¹] for k from 23 down to 1 (decision-r10-t1c-halo R1).
const AGE_PANELS: u32 = 24;

/// One node of a body's maturity mix ([`MaturityMix`]): a maturity, dimensionless in
/// [0, `m_body`], and the share of the body's ground the mix gives it, in (0, 1].
///
/// A mix holds no node of zero weight, so the type has no default.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct MaturityNode {
    maturity: f64,
    weight: f64,
}

impl MaturityNode {
    /// The node's maturity, `m_body` less one of the mix's deficit edges, dimensionless in
    /// [0, `m_body`].
    #[must_use]
    pub const fn maturity(&self) -> f64 {
        self.maturity
    }

    /// The node's share of the body's ground, in (0, 1].
    #[must_use]
    pub const fn weight(&self) -> f64 {
        self.weight
    }
}

/// A body's maturity mix (decision-r10-t1c-halo R1; the module documentation): the distribution
/// over its ground of the maturity its young craters' haloes leave, as the point query
/// ([`maturity_at`]) draws it, cut at nodes of falling maturity whose weights sum to 1, so that a
/// material's albedo averaged over the nodes is its albedo averaged over the body wherever the
/// albedo is linear between two nodes.
///
/// It is built in closed form ([`maturity_mix`]), the exact expectation of the drawn rule, and
/// holds no cache: the nodes and the law that gave them, from which
/// [`share_above`](Self::share_above) reads the share of the ground at any deficit.
#[derive(Debug, Clone, PartialEq)]
pub struct MaturityMix {
    nodes: Vec<MaturityNode>,
    law: HaloLaw,
}

impl MaturityMix {
    /// The body's background maturity, `m_body`, the header's: the first node's maturity, and the
    /// one an albedo solve over the mix must be exposed at.
    #[must_use]
    pub fn body(&self) -> f64 {
        self.law.body
    }

    /// The nodes, in falling maturity from `m_body`, each of a positive weight, the weights summing
    /// to 1 to rounding.
    #[must_use]
    pub fn nodes(&self) -> &[MaturityNode] {
        &self.nodes
    }

    /// The share of the body's ground whose deficit `m_body` − `m_local` exceeds `deficit`,
    /// 1 − F(d), dimensionless in [0, 1]: 0 from `m_body` on, and never rising with the deficit.
    ///
    /// # Panics
    ///
    /// If `deficit` is negative or NaN.
    #[must_use]
    pub fn share_above(&self, deficit: f64) -> f64 {
        assert!(
            deficit >= 0.0,
            "a deficit must be non-negative, got {deficit}"
        );
        1.0 - self.law.cumulative(deficit)
    }
}

/// The distribution F(d) = P(D ≤ d) of a body's point deficit D over its ground, from which its
/// mix is cut: F(d) = `F_c`(d) exp(−`Λ_s`(d)) below `m_body` and 1 from it (the module
/// documentation).
#[derive(Debug, Clone, PartialEq)]
struct HaloLaw {
    /// The background maturity, `m_body`.
    body: f64,
    /// A small crater's [`halo_opening`], for its end at [`HALO_EXTENT_RIM_RADII`].
    small_opening: f64,
    /// The octaves that can expose fresh ground, from the largest.
    octaves: Vec<OctaveHaloes>,
    /// The coarse craters of a positive rim deficit, in the field's list order, and the body's
    /// area; `None` where there are none.
    coarse: Option<CoarseHaloes>,
}

/// An octave's part of `Λ_s`: its kept density and its nodes in diameter and age.
#[derive(Debug, Clone, PartialEq)]
struct OctaveHaloes {
    /// The craters it keeps per square metre ([`CraterOctave::density`]).
    density_per_m2: f64,
    /// Its (diameter, age) nodes of a positive rim deficit, diameters outer and ages inner.
    haloes: Vec<SmallHalo>,
}

/// One (diameter, age) node of an octave: its crater's rim deficit and its weight.
#[derive(Debug, Clone, Copy, PartialEq)]
struct SmallHalo {
    /// The rim deficit δ₀ = g `f_ex` max(0, `m_body` − `maturity_after`(`t_c`, k)), as
    /// [`maturity_at`] takes it.
    deficit: f64,
    /// The node's weight in the octave's mean over diameters and ages, ½ ω times the age node's,
    /// times its crater's disc π R², square metres.
    disc_m2: f64,
}

/// The coarse craters' part of F: each one's halo, and the body's area.
#[derive(Debug, Clone, PartialEq)]
struct CoarseHaloes {
    haloes: Vec<CoarseHalo>,
    /// The body's area, square metres: its cells' [`cell_area`]s summed in index order, as
    /// [`class_fractions`](super::class_fractions) weights its materials.
    area_m2: f64,
}

/// One coarse crater's halo.
#[derive(Debug, Clone, Copy, PartialEq)]
struct CoarseHalo {
    /// Its rim deficit δ₀, as [`maturity_at`] takes it.
    deficit: f64,
    /// Its disc π R², square metres.
    disc_m2: f64,
    /// Its [`halo_opening`], for its end at its reach, (1 + a) rim radii.
    opening: f64,
}

impl HaloLaw {
    /// F(`deficit`), the share of the ground whose deficit is at most `deficit`: 1 from `m_body`
    /// on, and below it `F_c` exp(−`Λ_s`), each crater counted where its rim deficit is above
    /// `deficit` (strictly), out to where its share falls to `deficit` over its rim deficit. At
    /// `deficit` 0 it is F's limit from above, every halo of a positive rim deficit counted whole.
    /// Every sum runs in a fixed order from +0, of terms that never rise with `deficit`, so that F
    /// never falls as `deficit` rises.
    #[must_use]
    fn cumulative(&self, deficit: f64) -> f64 {
        if deficit >= self.body {
            return 1.0;
        }
        let mut rate = 0.0;
        for octave in &self.octaves {
            let mut mean = 0.0;
            for halo in &octave.haloes {
                if deficit < halo.deficit {
                    let x = reach_at(deficit / halo.deficit, self.small_opening);
                    mean += halo.disc_m2 * x * x;
                }
            }
            rate += octave.density_per_m2 * mean;
        }
        let free = self.coarse.as_ref().map_or(1.0, |coarse| {
            let mut covered = 0.0;
            for halo in &coarse.haloes {
                if deficit < halo.deficit {
                    let x = reach_at(deficit / halo.deficit, halo.opening);
                    covered += halo.disc_m2 * x * x;
                }
            }
            let share = covered / coarse.area_m2;
            if share < 1.0 { 1.0 - share } else { 0.0 }
        });
        free * math::exp(-rate)
    }
}

/// How finely the mix is taken: R1's scheme, or a finer one that the convergence tests take.
#[derive(Debug, Clone, Copy)]
struct MixScheme<'a> {
    /// The Gauss–Legendre rule on [−1, 1] in each octave's kept share of diameters.
    diameters: &'a [(f64, f64)],
    /// The parts each graded age panel is split into, each taking the eight-point rule.
    age_parts: u32,
    /// The parts each of R1's 51 deficit bins is split into.
    bin_parts: u32,
}

impl MixScheme<'static> {
    /// R1's scheme: eight diameter nodes, the 24 graded age panels whole, the 51 bins whole.
    const RULED: Self = Self {
        diameters: &GAUSS_LEGENDRE_8,
        age_parts: 1,
        bin_parts: 1,
    };
}

/// The maturity mix of the body whose coarse field `synth` reads, or `None` where its header
/// carries no maturity or no weathering rate (decision-r10-t1c-halo R1; the module
/// documentation).
///
/// The young craters' haloes, the coarse field's list and the small craters' octaves alike, are
/// counted in closed form at their synthesis's own laws, so the mix is the exact expectation of
/// [`maturity_at`]'s rule over the body, to its quadratures and its labelled limits. Where no
/// crater leaves a deficit (no maturity above any crater's, no crater density, no young coarse
/// crater) it is one node, `m_body`, of weight 1. Nothing is drawn: it is a function of the
/// header (crater contract, figure, boundary diameter, surface age, maturity, rate) and the coarse
/// list, the same on every target.
///
/// # Panics
///
/// As [`maturity_after`], [`excavated_share`] and the octaves' laws, for a header whose values the
/// field validated (a bug).
#[must_use]
pub fn maturity_mix(synth: &Synthesiser<'_, CoarseField>) -> Option<MaturityMix> {
    mix_with(synth, &MixScheme::RULED)
}

/// [`maturity_mix`] under `scheme`.
#[must_use]
fn mix_with(synth: &Synthesiser<'_, CoarseField>, scheme: &MixScheme<'_>) -> Option<MaturityMix> {
    let law = halo_law(synth, scheme)?;
    let nodes = cut(&law, scheme.bin_parts);
    Some(MaturityMix { nodes, law })
}

/// The deficit law of the body `synth` reads, under `scheme`, or `None` without a maturity and a
/// rate.
#[must_use]
fn halo_law(synth: &Synthesiser<'_, CoarseField>, scheme: &MixScheme<'_>) -> Option<HaloLaw> {
    let field = synth.field();
    let header = field.header();
    let (Some(body), Some(rate)) = (header.maturity(), header.weathering_rate()) else {
        return None;
    };
    let params = header.craters();
    let system = params.system_age();
    let surface_age = header.surface_age();
    let shares = age_nodes(scheme.age_parts);
    let mut octaves = Vec::new();
    for octave in synth.craters().octaves().octaves() {
        if !can_expose(octave, surface_age) {
            continue;
        }
        // The draw's ages: a uniform u of the window's count, through the chronology's inverse, at
        // the draw's own window density, and each age's wear in the octave (R09.T7.j).
        let window_density = octave.wear().window_density();
        let dated: Vec<(Gigayears, CraterWear, f64, f64)> = shares
            .iter()
            .map(|&(u, weight)| {
                let age = chronology_age(window_density * u, system);
                (
                    age,
                    CraterWear::of_age(age, octave.wear()),
                    ejecta_deficit(body, maturity_after(age, rate), 1.0),
                    weight,
                )
            })
            .collect();
        // The last octave keeps the share of its draw from 2 m up.
        let kept = octave.density().value() / octave.drawn_density().value();
        let dropped = 1.0 - kept;
        let mut haloes = Vec::with_capacity(scheme.diameters.len() * dated.len());
        for &(node, weight) in scheme.diameters {
            let size = UnitUniform::new(dropped + kept * f64::midpoint(1.0, node))
                .expect("a Gauss–Legendre node's share lies in [0, 1)");
            let diameter = diameter_in_octave(params, octave.diameters(), size);
            let rim = diameter * 0.5;
            let disc_m2 = 0.5 * weight * core::f64::consts::PI * rim.value() * rim.value();
            for &(age, wear, fresher, age_weight) in &dated {
                if fresher > 0.0 {
                    let deficit = small_crater_wear(rim, wear)
                        * excavated_share(surface_age, age, diameter)
                        * fresher;
                    if deficit > 0.0 {
                        haloes.push(SmallHalo {
                            deficit,
                            disc_m2: disc_m2 * age_weight,
                        });
                    }
                }
            }
        }
        octaves.push(OctaveHaloes {
            density_per_m2: octave.density().value() * 1e-6,
            haloes,
        });
    }
    let mut coarse = Vec::new();
    for crater in field.craters() {
        let fresh = kept_relief(crater) * excavated_share(surface_age, crater.age, crater.diameter);
        let deficit = deficit_of(body, crater.age, rate, fresh);
        if deficit > 0.0 {
            let shape = crater_shape(params, crater.diameter);
            let rim_m = 0.5 * shape.diameter().value();
            coarse.push(CoarseHalo {
                deficit,
                disc_m2: core::f64::consts::PI * rim_m * rim_m,
                opening: halo_opening(1.0 + shape.exterior_width()),
            });
        }
    }
    let coarse = (!coarse.is_empty()).then(|| {
        let level = header.level();
        let figure = header.figure();
        let area_m2 = (0..level.cell_count()).fold(0.0, |sum, index| {
            let cell =
                cell_at_index(level.get(), index).expect("every index below the count is a cell");
            sum + cell_area(cell, &figure).value()
        });
        CoarseHaloes {
            haloes: coarse,
            area_m2,
        }
    });
    Some(HaloLaw {
        body,
        small_opening: halo_opening(HALO_EXTENT_RIM_RADII),
        octaves,
        coarse,
    })
}

/// The age shares u and their weights of the mix's mean over a crater's age: eight-point
/// Gauss–Legendre on each of the [`AGE_PANELS`] graded panels split into `parts`, in order, the
/// weights summing to 1.
#[must_use]
fn age_nodes(parts: u32) -> Vec<(f64, f64)> {
    let edge = |k: u32| {
        if k == 0 {
            0.0
        } else {
            1.0 / f64::from(1_u32 << (AGE_PANELS - k))
        }
    };
    // A capacity hint alone: a count past usize's range leaves it at none.
    let panels = usize::try_from(AGE_PANELS.saturating_mul(parts)).unwrap_or(0);
    let mut nodes = Vec::with_capacity(panels.saturating_mul(GAUSS_LEGENDRE_8.len()));
    for panel in 0..AGE_PANELS {
        let (low, high) = (edge(panel), edge(panel + 1));
        let width = (high - low) / f64::from(parts);
        for part in 0..parts {
            let half = 0.5 * width;
            let middle = low + width * f64::from(part) + half;
            for (node, weight) in GAUSS_LEGENDRE_8 {
                nodes.push((middle + half * node, half * weight));
            }
        }
    }
    nodes
}

/// The mix's deficit edges as shares of `m_body`, ascending from 0 to 1: R1's 52, 0, then 2⁻²⁴ to
/// 2⁻⁵ by halvings, then 2/32 to 32/32, each of their 51 bins split into `parts` equal parts.
#[must_use]
fn deficit_edges(parts: u32) -> Vec<f64> {
    let halved = (COARSEST_DEFICIT_HALVINGS..=FINEST_DEFICIT_HALVINGS)
        .rev()
        .map(|k| 1.0 / f64::from(1_u32 << k));
    let stepped = (2..=DEFICIT_STEPS).map(|j| f64::from(j) / f64::from(DEFICIT_STEPS));
    let ruled: Vec<f64> = core::iter::once(0.0).chain(halved).chain(stepped).collect();
    // A capacity hint alone, as `age_nodes`' is.
    let bins = usize::try_from(parts)
        .unwrap_or(0)
        .saturating_mul(ruled.len() - 1);
    let mut edges = Vec::with_capacity(bins.saturating_add(1));
    edges.push(0.0);
    for pair in ruled.windows(2) {
        let (low, high) = (pair[0], pair[1]);
        for part in 1..parts {
            edges.push(low + (high - low) * f64::from(part) / f64::from(parts));
        }
        edges.push(high);
    }
    edges
}

/// The nodes `law`'s distribution is cut at, its bins split into `parts` (R1): `m_body` less each
/// deficit edge d, with the atom F(0) at the first and each bin's mass P = F(b) − F(a) shared
/// between its two edges so that its mean deficit is kept, the zero weights dropped.
///
/// A bin's mean deficit is d̄ = (b F(b) − a F(a) − ∫ₐᵇ F) ÷ P, the integral by Simpson's rule on
/// F(a), F at the midpoint m and F(b); its share towards b, t = (d̄ − a) ÷ (b − a), is then
/// exactly (P + 4 (F(b) − F(m))) ÷ 6P, which is how it is computed, since the first form cancels
/// digits where F is near 1 and P small. For an F that never falls, t lies in [1/6, 5/6], so the
/// clamp to [0, 1] guards only rounding; the bin gives P (1 − t) to the node at a and P t to the
/// node at b.
#[must_use]
fn cut(law: &HaloLaw, parts: u32) -> Vec<MaturityNode> {
    let body = law.body;
    let deficits: Vec<f64> = deficit_edges(parts).into_iter().map(|e| body * e).collect();
    let below: Vec<f64> = deficits.iter().map(|&d| law.cumulative(d)).collect();
    let mut weights = vec![0.0; deficits.len()];
    weights[0] = below[0];
    for k in 0..deficits.len() - 1 {
        let mass = below[k + 1] - below[k];
        if mass > 0.0 {
            let middle = law.cumulative(f64::midpoint(deficits[k], deficits[k + 1]));
            // Never −0: the numerator is a positive mass plus a difference of at least −mass to
            // rounding, which rounds to +0 where it cancels.
            let towards = ((mass + 4.0 * (below[k + 1] - middle)) / (6.0 * mass)).clamp(0.0, 1.0);
            weights[k] += mass * (1.0 - towards);
            weights[k + 1] += mass * towards;
        }
    }
    deficits
        .iter()
        .zip(&weights)
        .filter(|&(_, &weight)| weight > 0.0)
        .map(|(&d, &weight)| MaturityNode {
            maturity: body - d,
            weight,
        })
        .collect()
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
    use crate::craters::{cumulative_density, lunar_chronology};
    use crate::cube::{Face, MAX_LEVEL, st_to_uv, xyz_to_face_uv};
    use crate::field::{BVR_EFFECTIVE_WAVELENGTHS_NM, CoarseCrater, CoarseField, MechanicsFamily};
    use crate::geometry::{BAND_LIMIT_M, min_rate};
    use crate::material::{
        AlbedoScale, ClassFractions, GroundMaterial, PhaseLaw, albedo_scale, class_fractions,
        exposed_albedo_scale, haloed_albedo_scale, material_at, scaled_normal_albedo,
        weathered_normal_albedo,
    };
    use crate::synth::craters::{CraterOctaves, CraterWear, diffusion_length};
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

    /// A field's sampled maturities: the shares of its ground above each of [`DEFICITS`], the mean
    /// V excess of its named ground's exposed albedo at `m_local` over its albedo at `m_body`, and
    /// the named points' drawn geometric albedo in V ([`drawn_g`]).
    #[derive(Debug)]
    struct Sampled {
        above: [f64; 3],
        mean_v_excess: f64,
        /// Σ G(`m_local`) over the named points at each of the scales sampled.
        drawn: [f64; 2],
        /// Σ G(`m_body`) over the named points at the first scale.
        at_body: f64,
        /// The named points.
        named: u32,
    }

    /// [`SAMPLES`] points of `field`, uniform in area on its near-spherical figure, its named
    /// ground drawn at the two scales `scales`.
    fn sample(field: &CoarseField, scales: [f64; 2]) -> Sampled {
        let header = field.header();
        let palette = header.palette();
        let synth = Synthesiser::new(field, SEED);
        let mut cache = SynthCache::new();
        let mut rng = Lcg::new(0x6d61_7475_7269_7479);
        let body = header.maturity().unwrap();
        let exposure = GroundExposure::of_header(header);
        let mut above = [0_u32; 3];
        let (mut excess, mut named) = (0.0, 0_u32);
        let (mut drawn, mut at_body) = ([0.0; 2], 0.0);
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
                for (sum, &scale) in drawn.iter_mut().zip(&scales) {
                    *sum += drawn_g(m, palette, exposure, local, scale);
                }
                at_body += drawn_g(m, palette, exposure, body, scales[0]);
            }
        }
        Sampled {
            above: above.map(|n| f64::from(n) / f64::from(SAMPLES)),
            mean_v_excess: excess / f64::from(named),
            drawn,
            at_body,
            named,
        }
    }

    /// Which wear a rim deficit's g reads in [`immature_rims_per_km2`].
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum RimWear {
        /// R09.T7.j's, the synthesis's: the larger of κ's diffusion and the octave's equilibrium
        /// closure ([`CraterWear::of_age`] in the crater's octave).
        Equilibrium,
        /// κ's diffusion alone, as the ruling (decision-r10-t1c-maturity) computed its figures.
        DiffusionAlone,
    }

    /// The 1–20 km craters per km² whose rim deficit, g `f_ex` (1 − `maturity_after`(t, 1)) on
    /// mature ground, is above `threshold` on the surface of `header`, from the production and the
    /// chronology: over 400 bins of D uniform in its logarithm, each bin's share of N(>1 km) times
    /// the chronology's count of craters formed in the age below which a crater's rim deficit is
    /// above the threshold, at the present flux. g is [`relief_factor`] of the crater's `wear`:
    /// the closed form of the synthesis's rim deficit, its octave's ([`CraterOctaves::new`]) wear
    /// at the crater's age, or κ's alone.
    fn immature_rims_per_km2(header: &FieldHeader, threshold: f64, wear: RimWear) -> f64 {
        const BINS: u32 = 400;
        let params = header.craters();
        let surface_age = header.surface_age();
        let octaves = CraterOctaves::new(header);
        let above = |d_m: f64| cumulative_density(params, Metres::new(d_m)).value();
        let per_1km = above(1e3);
        let mut total = 0.0;
        for k in 0..BINS {
            let edge = |n: u32| 1e3 * math::powf(20.0, f64::from(n) / f64::from(BINS));
            let (d0, d1) = (edge(k), edge(k + 1));
            let d = (d0 * d1).sqrt();
            let octave = octaves
                .octaves()
                .iter()
                .find(|o| d >= o.diameters().lower().value() && d < o.diameters().upper().value())
                .expect("a 1–20 km crater is a small crater on the Moon-like worlds");
            let deficit = |t: f64| {
                let age = Gigayears::new(t);
                let g = match wear {
                    RimWear::Equilibrium => {
                        relief_factor(Metres::new(0.5 * d), CraterWear::of_age(age, octave.wear()))
                    }
                    RimWear::DiffusionAlone => relief_factor(
                        Metres::new(0.5 * d),
                        CraterWear::new(diffusion_length(age), 0.0),
                    ),
                };
                g * excavated_share(surface_age, age, Metres::new(d))
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
    /// a deficit of 0.5 is under 10% (without `f_ex` and g it would be 57%), and on its 1 and
    /// 0.1 Gyr variants, printed beside the ruling's estimates, with Grier et al. 2000's count in
    /// closed form, under R09.T7.j's wear and, beside it, κ's alone (the ruling's), and R5b's mean
    /// V excess.
    ///
    /// And the haloed solve against the drawn ground (decision-r10-t1c-halo's brute force): at each
    /// point its material's G(m) at the haloed scale for [`TARGET`], the stated excess within
    /// max(10%, 0.002) of the drawn one, Σ G(`m_local`) ÷ Σ G(`m_body`) − 1; the drawn mean
    /// Σ G(`m_local`) ÷ N within 1% of p, printed beside the same at the halo-free scale; and the
    /// mix's shares above 0.05, 0.2 and 0.5 within 10% of the sampled share plus three binomial
    /// standard errors.
    #[test]
    #[ignore = "slow: the maturity and the material at 10^4 points of four Moon-like worlds"]
    fn maturity_immature_shares_are_held_and_grier_s_count_stated() {
        let worlds = halo_worlds();
        for (world, estimates, excess) in [
            (&worlds[0], "0.3% above 0.2", "0.004"),
            (&worlds[1], "16% above 0.2, 2.1% above 0.5", "0.18"),
            (&worlds[2], "none stated", "0.20"),
            (&worlds[3], "none stated", "none stated"),
        ] {
            let (name, field) = (world.name, world.field);
            let s = sample(field, [world.haloed.scale(), world.halo_free.scale()]);
            eprintln!(
                "{name}: shares above {DEFICITS:?}: {:.4} {:.4} {:.4} (R4's estimates: \
                 {estimates}); mean V excess {:.4} (R5b's estimate, an upper bound, {excess})",
                s.above[0], s.above[1], s.above[2], s.mean_v_excess
            );
            check_the_haloed_solve(world, &s);
            let header = field.header();
            for (label, wear) in [
                ("R09.T7.j's wear", RimWear::Equilibrium),
                ("κ's alone, as the ruling's", RimWear::DiffusionAlone),
            ] {
                let (above_02, above_05) = (
                    immature_rims_per_km2(header, 0.2, wear),
                    immature_rims_per_km2(header, 0.5, wear),
                );
                eprintln!(
                    "{name}, {label}: 1–20 km craters with a rim deficit above 0.2 \
                     {above_02:.2e} km⁻², above 0.5 {above_05:.2e} km⁻² (the ruling's 4.2e-4 and \
                     1.7e-4, κ's alone), against Grier et al. 2000's {GRIER_IMMATURE_PER_KM2:.2e}"
                );
            }
            if name == "moon_like 3.5 Gyr" {
                assert!(s.above[2] < 0.10, "{s:?}");
            }
        }
    }

    /// The brute force's checks of `world`'s haloed solve against its sample `s`, printed and
    /// asserted (the bounds are the ruling's, never widened).
    fn check_the_haloed_solve(world: &HaloWorld, s: &Sampled) {
        let name = world.name;
        let n = f64::from(s.named);
        let drawn = s.drawn[0] / s.at_body - 1.0;
        let stated = world.haloed.halo_excess_v();
        let (mean, free_mean) = (s.drawn[0] / n, s.drawn[1] / n);
        eprintln!(
            "{name}: the stated excess {:.4}% against the drawn {:.4}%; the drawn mean over p \
             {:.4} at the haloed scale {:.6}, {:.4} at the halo-free {:.6}; {} named points",
            100.0 * stated,
            100.0 * drawn,
            mean / TARGET[1],
            world.haloed.scale(),
            free_mean / TARGET[1],
            world.halo_free.scale(),
            s.named
        );
        let bound = if 0.1 * drawn > 0.002 {
            0.1 * drawn
        } else {
            0.002
        };
        assert!(
            (stated - drawn).abs() <= bound,
            "{name}: the stated excess {stated} against the drawn {drawn}"
        );
        assert!(
            (mean / TARGET[1] - 1.0).abs() < 0.01,
            "{name}: the drawn mean {mean} against p {}",
            TARGET[1]
        );
        for (&d, &sampled) in DEFICITS.iter().zip(&s.above) {
            let closed = world.mix.share_above(d);
            let error = (sampled * (1.0 - sampled) / f64::from(SAMPLES)).sqrt();
            eprintln!(
                "{name}: the share above {d}: the mix's {closed:.5} against the sampled \
                 {sampled:.5} (± {error:.5})"
            );
            assert!(
                (closed - sampled).abs() <= 0.1 * sampled + 3.0 * error,
                "{name}: the share above {d}, {closed}, against the sampled {sampled}"
            );
        }
    }

    /// The `n`-point Gauss–Legendre rule on [−1, 1], ascending: Newton's method on Pₙ from the
    /// usual cosine guesses, to rounding.
    fn gauss_legendre(n: u32) -> Vec<(f64, f64)> {
        let order = f64::from(n);
        (0..n)
            .map(|i| {
                let mut x =
                    -math::cos(core::f64::consts::PI * (f64::from(i) + 0.75) / (order + 0.5));
                let mut slope = 1.0;
                for _ in 0..100 {
                    let (mut before, mut here) = (1.0, x);
                    for k in 2..=n {
                        let k = f64::from(k);
                        let next = ((2.0 * k - 1.0) * x * here - (k - 1.0) * before) / k;
                        before = here;
                        here = next;
                    }
                    slope = order * (x * here - before) / (x * x - 1.0);
                    let step = here / slope;
                    x -= step;
                    if step.abs() < 1e-16 {
                        break;
                    }
                }
                (x, 2.0 / ((1.0 - x * x) * slope * slope))
            })
            .collect()
    }

    /// The figures the convergence test holds: the stated excess of `mix`'s haloed solve on
    /// `world`, and its shares above [`DEFICITS`].
    fn figures(world: &HaloWorld, mix: &MaturityMix) -> [f64; 4] {
        let header = world.field.header();
        let haloed = haloed_albedo_scale(
            &world.fractions,
            header.palette(),
            GroundExposure::of_header(header),
            mix,
            TARGET,
        )
        .unwrap();
        let [a, b, c] = DEFICITS.map(|d| mix.share_above(d));
        [haloed.halo_excess_v(), a, b, c]
    }

    /// The excess in V of `world`'s named ground over `mix`'s nodes at the scale `scale`, against
    /// its ground at `m_body`: Σ a Σ w G(m) ÷ Σ a G(`m_body`) − 1.
    fn excess_at(world: &HaloWorld, mix: &MaturityMix, scale: f64) -> f64 {
        let header = world.field.header();
        let (palette, exposure) = (header.palette(), GroundExposure::of_header(header));
        let (mut over, mut under) = (0.0, 0.0);
        for &(ground, a) in world.fractions.shares() {
            let Some(m) = ground.material() else {
                continue;
            };
            let g = |maturity| drawn_g(m, palette, exposure, maturity, scale);
            over += a * mix
                .nodes()
                .iter()
                .fold(0.0, |sum, n| sum + n.weight() * g(n.maturity()));
            under += a * g(mix.body());
        }
        over / under - 1.0
    }

    /// Convergence (decision-r10-t1c-halo's test 5): doubling the diameter nodes (16
    /// Gauss–Legendre), the age panels (each split in two) or the bins (each split in two), one at
    /// a time, moves the stated excess and the shares above 0.05, 0.2 and 0.5 by under 1% of
    /// themselves on the four Moon-like worlds; and the excess over R1's nodes is within 1% of a
    /// 4,080-bin reference's on the same law (each bin split into 80), both at the haloed scale.
    #[test]
    #[ignore = "slow: the mix under four schemes and the haloed solve on four Moon-like worlds"]
    fn maturity_mix_converges_in_its_diameters_ages_and_bins() {
        let rule = gauss_legendre(8);
        for (got, want) in rule.iter().zip(&GAUSS_LEGENDRE_8) {
            assert!((got.0 - want.0).abs() < 1e-15 && (got.1 - want.1).abs() < 1e-15);
        }
        let sixteen = gauss_legendre(16);
        let total = sixteen.iter().fold(0.0, |sum, &(_, w)| sum + w);
        assert!((total - 2.0).abs() < 1e-14, "{total}");
        let ruled = MixScheme::RULED;
        let schemes = [
            (
                "16 diameter nodes",
                MixScheme {
                    diameters: &sixteen,
                    ..ruled
                },
            ),
            (
                "the age panels split in two",
                MixScheme {
                    age_parts: 2,
                    ..ruled
                },
            ),
            (
                "the bins split in two",
                MixScheme {
                    bin_parts: 2,
                    ..ruled
                },
            ),
        ];
        let names = ["the stated excess", "above 0.05", "above 0.2", "above 0.5"];
        for world in &halo_worlds()[..4] {
            let synth = Synthesiser::new(world.field, SEED);
            let base = figures(world, &world.mix);
            for (what, scheme) in &schemes {
                let mix = mix_with(&synth, scheme).unwrap();
                let moved = figures(world, &mix);
                for ((name, &b), &m) in names.iter().zip(&base).zip(&moved) {
                    let change = m / b - 1.0;
                    eprintln!(
                        "{}: {what}: {name} {b:.6e} to {m:.6e}, {:+.4}%",
                        world.name,
                        100.0 * change
                    );
                    assert!(change.abs() < 0.01, "{}: {what}: {name}", world.name);
                }
            }
            let reference = MaturityMix {
                nodes: cut(&world.mix.law, 80),
                law: world.mix.law.clone(),
            };
            let scale = world.haloed.scale();
            let (nodes, fine) = (
                excess_at(world, &world.mix, scale),
                excess_at(world, &reference, scale),
            );
            let gap = nodes / fine - 1.0;
            eprintln!(
                "{}: the excess at the haloed scale over R1's nodes {nodes:.6e}, over 4,080 bins \
                 {fine:.6e}, {:+.4}%",
                world.name,
                100.0 * gap
            );
            assert!(gap.abs() < 0.01, "{}: {nodes} against {fine}", world.name);
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

    // ---- decision-r10-t1c-halo: the inverse halo, the body's maturity mix and the haloed solve
    // ----

    /// The weathered Moon-like world on its 3.5 Gyr surface (N(>1 km) 4.8 × 10⁻³), built once per
    /// test process.
    fn middle_moon() -> &'static CoarseField {
        static FIELD: OnceLock<CoarseField> = OnceLock::new();
        FIELD.get_or_init(|| younger_moon(3.5, 4.8e-3))
    }

    /// The weathered Moon-like world on its 1 Gyr surface (N(>1 km) 8.4 × 10⁻⁴), built once per
    /// test process.
    fn young_moon() -> &'static CoarseField {
        static FIELD: OnceLock<CoarseField> = OnceLock::new();
        FIELD.get_or_init(|| younger_moon(1.0, 8.4e-4))
    }

    /// The weathered Moon-like world on its 0.1 Gyr surface (N(>1 km) 8.4 × 10⁻⁵), built once per
    /// test process.
    fn youngest_moon() -> &'static CoarseField {
        static FIELD: OnceLock<CoarseField> = OnceLock::new();
        FIELD.get_or_init(|| younger_moon(0.1, 8.4e-5))
    }

    /// An iron-bearing basalt regolith (`FeO_T` 0.16), the server test's `weathering_field`'s
    /// crust, so that the ground it covers weathers.
    fn basalt_fe() -> PaletteEntry {
        PaletteEntry {
            substance: SubstanceKey::new("basalt_fe").expect("a test key is in the grammar"),
            role: PaletteRole::PrimaryCrust,
            normal_albedo_bvr: [0.1, 0.11, 0.12],
            phase_row: 0,
            grain_density_kg_m3: 3_310.0,
            transition: Kelvin::new(1_323.15),
            mechanics: MechanicsFamily::Silicate,
            liquid: None,
            feo_mass_fraction: Some(0.16),
        }
    }

    /// `builder` on a one-entry palette of [`basalt_fe`], every crust of it.
    fn on_basalt(builder: FieldBuilder) -> FieldBuilder {
        builder
            .palette(MaterialPalette::new(vec![basalt_fe()]).expect("one entry is a palette"))
            .crust_palette([Some(0); 4])
    }

    /// The young single crater's world ([`young_crater_with`] at [`YOUNG_CRATER_DEGRADATION`], g
    /// 0.8) on [`basalt_fe`], so that its ground weathers: built once per test process.
    fn basalt_crater() -> &'static CoarseField {
        static FIELD: OnceLock<CoarseField> = OnceLock::new();
        FIELD.get_or_init(|| {
            on_basalt(one_crater_builder(
                Gigayears::new(YOUNG_CRATER_AGE),
                YOUNG_CRATER_DEGRADATION,
            ))
            .maturity(body_maturity(Gigayears::new(4.0), 1.0))
            .weathering_rate(1.0)
            .build()
        })
    }

    /// The worlds the mix is held on: the weathered Moon-like world and its younger variants, and
    /// the young single crater on basalt.
    fn halo_fields() -> [(&'static str, &'static CoarseField); 5] {
        [
            ("moon_like 4.4 Gyr", weathered_moon()),
            ("moon_like 3.5 Gyr", middle_moon()),
            ("moon_like 1 Gyr", young_moon()),
            ("moon_like 0.1 Gyr", youngest_moon()),
            ("one_crater", basalt_crater()),
        ]
    }

    /// The geometric albedo in B, V and R the solves are taken for, as
    /// `maturity_the_solve_on_the_exposed_albedos_gives_p` takes it.
    const TARGET: [f64; 3] = [0.10, 0.12, 0.14];

    /// What the solve's tests and the golden read of a world: its class fractions, its mix, and its
    /// halo-free and haloed scales for [`TARGET`].
    struct HaloWorld {
        name: &'static str,
        field: &'static CoarseField,
        fractions: ClassFractions,
        mix: MaturityMix,
        halo_free: AlbedoScale,
        haloed: AlbedoScale,
    }

    /// The [`halo_fields`]' [`HaloWorld`]s, computed once per test process: the class fractions
    /// are the costly part.
    fn halo_worlds() -> &'static [HaloWorld] {
        static WORLDS: OnceLock<Vec<HaloWorld>> = OnceLock::new();
        WORLDS.get_or_init(|| {
            halo_fields()
                .into_iter()
                .map(|(name, field)| {
                    let header = field.header();
                    let synth = Synthesiser::new(field, SEED);
                    let fractions = class_fractions(&synth, &mut SynthCache::new())
                        .expect("a coarse field holds every cell");
                    let mix = maturity_mix(&synth).expect("the field carries a maturity");
                    let exposure = GroundExposure::of_header(header);
                    let palette = header.palette();
                    let halo_free = exposed_albedo_scale(&fractions, palette, exposure, TARGET)
                        .expect("the world names its ground");
                    let haloed = haloed_albedo_scale(&fractions, palette, exposure, &mix, TARGET)
                        .expect("the world names its ground");
                    HaloWorld {
                        name,
                        field,
                        fractions,
                        mix,
                        halo_free,
                        haloed,
                    }
                })
                .collect()
        })
    }

    /// The inverse halo is [`halo_share`]'s inverse, to 10⁻¹², across (0, 1] for a small crater's
    /// end and coarse craters' (1 + a), and 1 at a share of 1.
    #[test]
    fn maturity_halo_reach_inverts_the_halo() {
        let (_, a) = rim_radii_from(
            young_crater().header(),
            &young_crater().craters()[0],
            one_crater_centre(),
        );
        for end in [HALO_EXTENT_RIM_RADII, 1.0 + a, 1.3, 2.543, 1.0] {
            assert_same_bits(halo_reach(1.0, end), 1.0);
            let mut previous = 1.0;
            for k in 0..=2_000_u32 {
                let s = if k == 0 {
                    1e-300
                } else {
                    f64::from(k) / 2_000.0
                };
                let x = halo_reach(s, end);
                assert!((1.0..=end).contains(&x), "end {end}, s {s}: {x}");
                let back = halo_share(x, end);
                if end > 1.0 {
                    assert!((back - s).abs() < 1e-12, "end {end}, s {s}: {back}");
                }
                if k > 0 {
                    assert!(x <= previous, "end {end}: {x} at {s} after {previous}");
                }
                previous = x;
            }
            // The end at a share falling to 0.
            assert!((halo_reach(1e-300, end) - end).abs() < 1e-12 * end, "{end}");
        }
    }

    /// The scheme's counts: 192 age nodes on 24 graded panels whose weights sum to 1, and 52
    /// deficit edges, 0, the 20 halvings from 2⁻²⁴ and the 31 steps of 1/32 to 1, each split in
    /// two when the bins are.
    #[test]
    fn maturity_mix_scheme_has_the_ruled_nodes() {
        let ages = age_nodes(1);
        assert_eq!(ages.len(), 192);
        let total = ages.iter().fold(0.0, |sum, &(_, w)| sum + w);
        assert!((total - 1.0).abs() < 1e-15, "{total}");
        assert!(ages.windows(2).all(|pair| pair[0].0 < pair[1].0));
        assert!(ages[0].0 > 0.0 && ages[0].0 < 1.0 / f64::from(1_u32 << 23));
        assert!(ages[191].0 < 1.0);
        assert_eq!(age_nodes(2).len(), 384);
        let edges = deficit_edges(1);
        assert_eq!(edges.len(), 52);
        assert_same_bits(edges[0], 0.0);
        for (k, &e) in edges[1..=20].iter().enumerate() {
            let halvings = 24 - u32::try_from(k).unwrap();
            assert_same_bits(e, 1.0 / f64::from(1_u32 << halvings));
        }
        for (j, &e) in (2..=32_u32).zip(&edges[21..]) {
            assert_same_bits(e, f64::from(j) / 32.0);
        }
        let split = deficit_edges(2);
        assert_eq!(split.len(), 103);
        for (k, &e) in edges.iter().enumerate() {
            assert_same_bits(split[2 * k], e);
        }
        assert!(split.windows(2).all(|pair| pair[0] < pair[1]));
    }

    /// The mix is a distribution over the body's maturity: positive weights summing to 1 within
    /// 10⁻¹², nodes in [0, `m_body`] in falling maturity, the first `m_body` where the atom holds,
    /// and a share above a deficit that is 0 from `m_body` on and never rises, on every world.
    #[test]
    fn maturity_mix_is_a_distribution_over_the_body_s_maturity() {
        for world in halo_worlds() {
            let (name, mix) = (world.name, &world.mix);
            let body = world.field.header().maturity().unwrap();
            assert_same_bits(mix.body(), body);
            let nodes = mix.nodes();
            assert!(
                !nodes.is_empty() && nodes.len() <= 52,
                "{name}: {}",
                nodes.len()
            );
            let total = nodes.iter().fold(0.0, |sum, n| sum + n.weight());
            assert!((total - 1.0).abs() < 1e-12, "{name}: {total}");
            for node in nodes {
                assert!(
                    node.weight() > 0.0 && node.weight() <= 1.0,
                    "{name}: {node:?}"
                );
                assert!((0.0..=body).contains(&node.maturity()), "{name}: {node:?}");
            }
            assert!(
                nodes
                    .windows(2)
                    .all(|pair| pair[0].maturity() > pair[1].maturity()),
                "{name}"
            );
            assert!(nodes.len() > 1, "{name}: its young craters leave deficits");
            assert_same_bits(mix.share_above(body), 0.0);
            assert_same_bits(mix.share_above(2.0), 0.0);
            let mut previous = mix.share_above(0.0);
            assert!((0.0..=1.0).contains(&previous), "{name}: {previous}");
            for k in 1..=4_000_u32 {
                let d = body * f64::from(k) / 4_000.0;
                let share = mix.share_above(d);
                assert!(
                    share <= previous && share >= 0.0,
                    "{name}: {share} at {d} after {previous}"
                );
                previous = share;
            }
            for &e in &deficit_edges(1) {
                let share = mix.share_above(body * e);
                assert!((0.0..=1.0).contains(&share), "{name}: {share}");
            }
        }
    }

    /// The mix is the same twice, and the same under another detail seed, to the bit: a function
    /// of the header and the coarse list alone, on the weathered Moon-like world (the small
    /// craters' octaves) and the young single crater (the coarse list).
    #[test]
    fn maturity_mix_is_the_same_twice_and_under_any_seed() {
        let node_bits = |mix: &MaturityMix| -> Vec<(u64, u64)> {
            mix.nodes()
                .iter()
                .map(|n| (bits(n.maturity()), bits(n.weight())))
                .collect()
        };
        for field in [weathered_moon(), basalt_crater()] {
            let once = maturity_mix(&Synthesiser::new(field, SEED)).unwrap();
            let twice = maturity_mix(&Synthesiser::new(field, SEED)).unwrap();
            let other = maturity_mix(&Synthesiser::new(
                field,
                hyperion_base::rng::DetailSeed::new(0x5eed_0000_0000_0051),
            ))
            .unwrap();
            assert_eq!(node_bits(&once), node_bits(&twice));
            assert_eq!(node_bits(&once), node_bits(&other));
            assert_eq!(once, other);
            for d in DEFICITS {
                assert_same_bits(once.share_above(d), other.share_above(d));
            }
        }
    }

    /// A share of the ground in each of `classes` of entry 0, and in an unnamed form, the shares
    /// summing to 1.
    fn hand_fractions(classes: &[MaterialClass]) -> ClassFractions {
        ClassFractions::new(
            classes
                .iter()
                .enumerate()
                .map(|(k, &class)| {
                    let k = u32::try_from(k).unwrap();
                    (GroundMaterial::of(class, Some(0)), f64::from(k + 2))
                })
                .chain([(GroundMaterial::of(MaterialClass::Regolith, None), 1.0)]),
        )
        .unwrap()
    }

    /// The identity, to the bit: no maturity gives no mix; a maturity where no crater leaves a
    /// deficit (no crater density and no coarse crater, an old coarse crater no fresher than the
    /// ground, or a maturity of 0 on cratered ground) gives one node, `m_body`, of weight 1, and
    /// the haloed solve is `exposed_albedo_scale`'s; and a palette whose every material is
    /// maturity-independent (an entry of `FeO_T` `None` or 0, `Melt`) solves as
    /// `exposed_albedo_scale` on a cratered, weathered world.
    #[test]
    fn maturity_mix_is_the_identity_where_no_crater_leaves_a_deficit() {
        for &w in SyntheticWorld::ALL {
            let field = world(w);
            assert_eq!(field.header().maturity(), None);
            assert!(
                maturity_mix(&Synthesiser::new(field, SEED)).is_none(),
                "{w:?}"
            );
        }
        let radius = Metres::new(1.7374e6);
        let fields = [
            on_basalt(FieldBuilder::new(radius))
                .maturity(1.0)
                .weathering_rate(1.0)
                .build(),
            on_basalt(one_crater_builder(Gigayears::new(3.9), 0))
                .maturity(0.6)
                .weathering_rate(1.0)
                .build(),
            on_basalt(moon_like_builder())
                .maturity(0.0)
                .weathering_rate(1.0)
                .build(),
        ];
        let forms = [
            MaterialClass::Regolith,
            MaterialClass::Bedrock,
            MaterialClass::Sand,
            MaterialClass::Melt,
        ];
        let fractions = hand_fractions(&forms);
        for field in &fields {
            let header = field.header();
            let mix = maturity_mix(&Synthesiser::new(field, SEED)).unwrap();
            let body = header.maturity().unwrap();
            assert_eq!(
                mix.nodes()
                    .iter()
                    .map(|n| (bits(n.maturity()), bits(n.weight())))
                    .collect::<Vec<_>>(),
                [(bits(body), bits(1.0))]
            );
            let exposure = GroundExposure::of_header(header);
            let palette = header.palette();
            let haloed = haloed_albedo_scale(&fractions, palette, exposure, &mix, TARGET).unwrap();
            let halo_free = exposed_albedo_scale(&fractions, palette, exposure, TARGET).unwrap();
            assert_eq!(haloed, halo_free);
            assert_same_bits(haloed.scale(), halo_free.scale());
            assert_same_bits(haloed.halo_excess_v(), 0.0);
        }
        // Maturity-independent ground on the cratered, weathered Moon-like world.
        let mix = &halo_worlds()[0].mix;
        assert!(mix.nodes().len() > 1);
        let without_iron = |key: &str, feo: Option<f64>| PaletteEntry {
            substance: SubstanceKey::new(key).unwrap(),
            feo_mass_fraction: feo,
            ..basalt_fe()
        };
        let palette = MaterialPalette::new(vec![
            without_iron("basalt_a", None),
            without_iron("basalt_b", Some(0.0)),
            ice("H2O", PaletteRole::Ice, 0.8),
        ])
        .unwrap();
        let fractions = ClassFractions::new([
            (GroundMaterial::of(MaterialClass::Regolith, Some(0)), 3.0),
            (GroundMaterial::of(MaterialClass::Bedrock, Some(1)), 2.0),
            (GroundMaterial::of(MaterialClass::Glacier, Some(2)), 1.0),
            (GroundMaterial::of(MaterialClass::Frost, Some(2)), 1.0),
            (GroundMaterial::of(MaterialClass::Melt, Some(0)), 0.5),
        ])
        .unwrap();
        let exposure = GroundExposure::NONE.with_maturity(mix.body());
        let haloed = haloed_albedo_scale(&fractions, &palette, exposure, mix, TARGET).unwrap();
        let halo_free = exposed_albedo_scale(&fractions, &palette, exposure, TARGET).unwrap();
        assert_eq!(haloed, halo_free);
        assert_same_bits(haloed.halo_excess_v(), 0.0);
        // And `Melt` of an iron-bearing entry beside them.
        let palette = MaterialPalette::new(vec![basalt_fe()]).unwrap();
        let fractions = hand_fractions(&[MaterialClass::Melt]);
        let haloed = haloed_albedo_scale(&fractions, &palette, exposure, mix, TARGET).unwrap();
        let halo_free = exposed_albedo_scale(&fractions, &palette, exposure, TARGET).unwrap();
        assert_eq!(haloed, halo_free);
    }

    /// The rim deficit of `field`'s one coarse crater, δ₀ = g `f_ex` (`m_body` −
    /// `maturity_after`(`t_c`, k)), its rim radius, metres, and its halo's end, 1 + a.
    fn coarse_rim(field: &CoarseField) -> (f64, f64, f64) {
        let header = field.header();
        let crater = &field.craters()[0];
        let (body, rate) = (
            header.maturity().unwrap(),
            header.weathering_rate().unwrap(),
        );
        let fresh = kept_relief(crater)
            * excavated_share(header.surface_age(), crater.age, crater.diameter);
        let deficit = deficit_of(body, crater.age, rate, fresh);
        let (_, a) = rim_radii_from(header, crater, crater.centre);
        (deficit, 0.5 * crater.diameter.value(), 1.0 + a)
    }

    /// The body's area as the mix takes it: its cells' areas summed in index order.
    fn body_area(field: &CoarseField) -> f64 {
        let level = field.header().level();
        let figure = field.header().figure();
        (0..level.cell_count()).fold(0.0, |sum, index| {
            sum + cell_area(cell_at_index(level.get(), index).unwrap(), &figure).value()
        })
    }

    /// The geometric albedo in V of `material` drawn at maturity `m` under the scale `scale`,
    /// G(m) = x (2 + L(x)) ÷ 3, x its scaled V and L its law's limb parameter (the brute force's).
    fn drawn_g(
        material: Material,
        palette: &MaterialPalette,
        exposure: GroundExposure,
        m: f64,
        scale: f64,
    ) -> f64 {
        let p = exposed_properties(material, palette, exposure.with_maturity(m)).unwrap();
        let x = scaled_normal_albedo(&p, scale)[1];
        x * (2.0 + p.phase_law().limb_parameter(x)) / 3.0
    }

    /// One coarse crater, the small craters' density 0: the share above d is the halo's disc out
    /// to x*(d ÷ δ₀) over the body's area, to 10⁻¹², below the rim deficit δ₀, and 0 beyond; and
    /// each form's excess over the mix at a fixed scale, Σ w G(m) ÷ G(`m_body`) − 1, is the direct
    /// radial integral of G(`m_local`(x)) ÷ G(`m_body`) − 1 over the halo's area to 0.5%.
    #[test]
    fn maturity_mix_of_one_coarse_crater_is_its_halo_s_closed_form() {
        /// The midpoints of the direct radial integral.
        const STEPS: u32 = 10_000;
        let field = basalt_crater();
        let header = field.header();
        let mix = maturity_mix(&Synthesiser::new(field, SEED)).unwrap();
        let (deficit, rim_m, end) = coarse_rim(field);
        let area = body_area(field);
        let body = header.maturity().unwrap();
        assert!(deficit > 0.5 && deficit < body, "{deficit}");
        for k in 0..=1_000_u32 {
            let d = deficit * f64::from(k) / 1_000.0;
            let got = mix.share_above(d);
            if k < 1_000 {
                let share = if k == 0 { 1e-300 } else { d / deficit };
                let reach = halo_reach(share, end) * rim_m;
                let want = core::f64::consts::PI * reach * reach / area;
                assert!(
                    (got / want - 1.0).abs() < 1e-12,
                    "{d}: {got} against {want}"
                );
            } else {
                assert_same_bits(got, 0.0);
            }
        }
        for d in [deficit * (1.0 + 1e-12), 0.7, body] {
            assert_same_bits(mix.share_above(d), 0.0);
        }
        // The excess, form by form, at the entry's own albedo (c = 1).
        let palette = header.palette();
        let exposure = GroundExposure::of_header(header);
        let crater = &field.craters()[0];
        let rate = header.weathering_rate().unwrap();
        let fresh = kept_relief(crater)
            * excavated_share(header.surface_age(), crater.age, crater.diameter);
        let mut checked = 0;
        for &class in MaterialClass::ALL {
            let material = Material::new(class, 0).unwrap();
            let g = |m: f64| drawn_g(material, palette, exposure, m, 1.0);
            let at_body = g(body);
            let mixed = mix
                .nodes()
                .iter()
                .fold(0.0, |sum, n| sum + n.weight() * g(n.maturity()))
                / at_body
                - 1.0;
            let dx = end / f64::from(STEPS);
            let mut direct = 0.0;
            for i in 0..STEPS {
                let x = dx * (f64::from(i) + 0.5);
                let share = fresh * halo_share(x, end);
                let local = less(body, deficit_of(body, crater.age, rate, share));
                direct += (g(local) / at_body - 1.0) * 2.0 * x * dx;
            }
            let direct = direct * core::f64::consts::PI * rim_m * rim_m / area;
            eprintln!("one crater, {class:?}: the mix's excess {mixed:.6e}, direct {direct:.6e}");
            if class == MaterialClass::Melt {
                assert_same_bits(mixed, 0.0);
                continue;
            }
            assert!(direct > 0.0, "{class:?}: {direct}");
            assert!(
                (mixed / direct - 1.0).abs() < 0.005,
                "{class:?}: the mix's {mixed} against the direct {direct}"
            );
            checked += 1;
        }
        assert!(checked > 5, "{checked}");
    }

    /// The haloed solve: on every world the haloed scale is at most the halo-free one, and below it
    /// where the stated excess is positive, the V residual is under 10⁻⁹ relative, and the
    /// halo-free solve states no excess.
    #[test]
    fn maturity_the_haloed_scale_is_below_the_halo_free_one() {
        for world in halo_worlds() {
            let (name, haloed, free) = (world.name, &world.haloed, &world.halo_free);
            eprintln!(
                "{name}: halo-free scale {:.6}, haloed {:.6}, stated excess {:.4}%; {} nodes",
                free.scale(),
                haloed.scale(),
                100.0 * haloed.halo_excess_v(),
                world.mix.nodes().len()
            );
            assert_same_bits(free.halo_excess_v(), 0.0);
            assert!(haloed.halo_excess_v() >= 0.0, "{name}: {haloed:?}");
            assert!(haloed.scale() <= free.scale(), "{name}");
            if haloed.halo_excess_v() > 0.0 {
                assert!(haloed.scale() < free.scale(), "{name}");
            }
            assert!(
                haloed.relative_residual()[1].abs() < 1e-9,
                "{name}: {haloed:?}"
            );
            assert!(free.relative_residual()[1].abs() < 1e-9, "{name}: {free:?}");
        }
        assert!(
            halo_worlds()[1].haloed.halo_excess_v() > 0.01,
            "the 3.5 Gyr ground's haloes brighten it"
        );
    }

    // ---- The golden: `tests/golden/material/halo.golden` (`material/golden.rs` counts it) ----

    /// The halo golden's text: the inverse halo on a grid, and on each of [`halo_worlds`] the mix's
    /// nodes and weights, the shares above 0.05, 0.2 and 0.5, and the halo-free and haloed scales
    /// with the stated excess.
    fn halo_text() -> String {
        let mut out = GoldenWriter::new();
        out.header(crate::generator_version());
        out.line("halo_reach: s <share> end <end>");
        for s in [1e-12, 0.01, 0.1, 0.3, 0.5, 0.9, 0.999, 1.0] {
            for end in [1.0, 1.3, 2.543, 10.0] {
                out.f64(&format!("s {s} end {end}"), halo_reach(s, end));
            }
        }
        for world in halo_worlds() {
            let mix = &world.mix;
            out.line(&format!(
                "mix {}: m_body {}, {} nodes: <maturity> <weight>",
                world.name,
                hex(mix.body()),
                mix.nodes().len()
            ));
            for node in mix.nodes() {
                out.line(&format!(
                    "  {} {}",
                    hex(node.maturity()),
                    hex(node.weight())
                ));
            }
            for d in DEFICITS {
                out.f64(&format!("  share_above {d}"), mix.share_above(d));
            }
            out.f64("  scale halo-free", world.halo_free.scale());
            out.f64("  scale haloed", world.haloed.scale());
            out.f64("  halo_excess_v", world.haloed.halo_excess_v());
        }
        out.finish()
    }

    #[test]
    fn maturity_halo_matches_its_golden() {
        golden!("material/halo", &halo_text());
    }

    // ---- R10.T1.c's client half: a level's cut and a patch's gather
    // (decision-r10-t1c-client-scope §1.3, §1.4 and §2.5's tests 1 and 13) ----

    /// The scope probe's place, at which its level-14 patch lay.
    const PROBE_DIR: [f64; 3] = [0.3, 1.0, 0.2];

    /// The three weathered Moon-like worlds the gather is held on, by surface age.
    fn weathered_worlds() -> [(&'static str, &'static CoarseField); 3] {
        [
            ("4.4 Gyr", weathered_moon()),
            ("3.5 Gyr", middle_moon()),
            ("1 Gyr", young_moon()),
        ]
    }

    /// The shallowest band level whose shortest wavelength on `field`'s figure is the band limit.
    fn band_limit_level(field: &CoarseField) -> u8 {
        let figure = field.header().figure();
        (0..=MAX_LEVEL)
            .find(|&n| {
                shortest_wavelength(&figure, BandLevel::new(n).unwrap()).value() <= BAND_LIMIT_M
            })
            .expect("a body's finest levels reach the band limit")
    }

    /// How many of `synth`'s octaves can expose fresh ground and are kept by `kept`.
    fn kept_octaves(synth: &Synthesiser<'_, CoarseField>, kept: Kept) -> usize {
        let age = synth.field().header().surface_age();
        synth
            .craters()
            .octaves()
            .octaves()
            .iter()
            .filter(|octave| can_expose(octave, age) && kept.keeps(octave))
            .count()
    }

    /// Every vertex of `key`, row by row, each given its direction.
    fn every_vertex(key: PatchKey) -> Vec<Option<[f64; 3]>> {
        (0..=64_u8)
            .flat_map(|y| (0..=64_u8).map(move |x| Some(key.vertex_dir(x, y))))
            .collect()
    }

    /// The vertices a gather is compared at: every one natively where `every` asks it, and
    /// otherwise, and on the wasm targets, the two diagonals', so that the browser's run stays
    /// short.
    fn compared_vertices(every: bool) -> Vec<(u8, u8)> {
        if every && !cfg!(target_arch = "wasm32") {
            (0..=64_u8)
                .flat_map(|y| (0..=64_u8).map(move |x| (x, y)))
                .collect()
        } else {
            (0..=64_u8).flat_map(|k| [(k, k), (64 - k, k)]).collect()
        }
    }

    /// Holds `got`, the gather of `key` with `vertices` over the octaves `kept` and `parent` keep,
    /// to the bit against the point rule over each at each vertex of `at` given a direction, and
    /// to `m_body` for both at one given none; how many of them lie below `m_body`.
    fn assert_is_the_point_rule(
        synth: &Synthesiser<'_, CoarseField>,
        (key, vertices): (PatchKey, &[Option<[f64; 3]>]),
        got: &[[f64; 2]],
        (kept, parent): (Kept, Kept),
        at: &[(u8, u8)],
    ) -> usize {
        let body = synth.field().header().maturity().unwrap();
        // The parent keeps no octave the level does not, so the same count is the same set.
        let same = kept_octaves(synth, kept) == kept_octaves(synth, parent);
        let mut cache = SynthCache::new();
        let mut below = 0;
        for &(x, y) in at {
            let n = usize::from(y) * 65 + usize::from(x);
            let Some(dir) = vertices[n] else {
                assert_same_bits(got[n][0], body);
                assert_same_bits(got[n][1], body);
                continue;
            };
            let mut rule = |kept: Kept| {
                point_maturity(synth, &mut cache, || dir, kept)
                    .unwrap()
                    .unwrap()
            };
            let here = rule(kept);
            let there = if same { here } else { rule(parent) };
            assert_eq!(
                (bits(got[n][0]), bits(got[n][1])),
                (bits(here), bits(there)),
                "{key:?} ({x}, {y}): {:?} against the point rule's ({here}, {there})",
                got[n]
            );
            below += usize::from(here < body);
        }
        below
    }

    /// Test 1 where every octave is kept (decision-r10-t1c-client-scope §2.5): a patch's gather is
    /// [`maturity_at`]'s rule to the bit at every vertex natively, and at the two diagonals' on
    /// the wasm targets, both values: at the band limit's level at the probe's place on the three
    /// weathered worlds, and at a cube's corner, where the search crosses three faces, on the
    /// 1 Gyr one; across the young single crater's rim and reach, where no small crater is drawn;
    /// and, through the gather's own form, at the probe's level-14 patch with every octave kept and
    /// the second value over the level's own cut.
    #[test]
    fn maturity_patch_gather_is_maturity_at_where_every_octave_is_kept() {
        for (name, field) in weathered_worlds() {
            let synth = Synthesiser::new(field, SEED);
            let figure = field.header().figure();
            let limit = band_limit_level(field);
            let band = |level: u8| BandLevel::new(level).unwrap();
            let cut = maturity_cut(&figure, band(limit));
            assert_same_bits(cut.value(), 0.5 * BAND_LIMIT_M);
            assert_eq!(
                kept_octaves(&synth, Kept::From(cut)),
                kept_octaves(&synth, Kept::Every),
                "{name}: the band limit's level keeps every octave"
            );
            let parent = Kept::From(maturity_cut(&figure, band(limit - 1)));
            let mut keys = vec![PatchKey::containing(limit, unit_dir(PROBE_DIR)).unwrap()];
            if name == "1 Gyr" {
                let last = (1_u32 << limit) - 1;
                keys.push(PatchKey::new(Face::PosX, limit, last, last).unwrap());
            }
            for key in keys {
                let vertices = every_vertex(key);
                let got = patch_maturity(&synth, &mut SynthCache::new(), key, &vertices)
                    .unwrap()
                    .unwrap();
                let below = assert_is_the_point_rule(
                    &synth,
                    (key, &vertices),
                    &got,
                    (Kept::Every, parent),
                    &compared_vertices(true),
                );
                eprintln!("{name}: {key:?}, {below} compared vertices below m_body");
            }
            let key = PatchKey::containing(14, unit_dir(PROBE_DIR)).unwrap();
            let vertices = every_vertex(key);
            let own = Kept::From(maturity_cut(&figure, band(14)));
            let got = gather_patch(
                &synth,
                &mut SynthCache::new(),
                key,
                &vertices,
                (Kept::Every, own),
            )
            .unwrap()
            .unwrap();
            let below = assert_is_the_point_rule(
                &synth,
                (key, &vertices),
                &got,
                (Kept::Every, own),
                &compared_vertices(name == "1 Gyr"),
            );
            eprintln!("{name}: level 14 with every octave, {below} compared vertices below m_body");
        }
        let field = young_crater();
        let synth = Synthesiser::new(field, SEED);
        assert_eq!(kept_octaves(&synth, Kept::Every), 0);
        let crater = &field.craters()[0];
        let rim_m = 0.5 * crater.diameter.value();
        let across = along_chord(crater.centre, rim_m, smallest_radius(field));
        let key = PatchKey::containing(4, across).unwrap();
        let vertices = every_vertex(key);
        let got = patch_maturity(&synth, &mut SynthCache::new(), key, &vertices)
            .unwrap()
            .unwrap();
        let below = assert_is_the_point_rule(
            &synth,
            (key, &vertices),
            &got,
            (Kept::Every, Kept::Every),
            &compared_vertices(true),
        );
        assert!(below > 0, "the patch lies in the crater's halo");
    }

    /// Test 1 at each level (decision-r10-t1c-client-scope §2.5): at levels 4 to 14, and 0, a
    /// patch's gather at the probe's place is the point rule over the octaves the level keeps
    /// ([`maturity_at_level`]'s) at the two diagonals' vertices natively (at every eighth of them,
    /// at levels 0, 4, 9 and 14, on the wasm targets), on the three weathered worlds; its second
    /// value is the parent level's, level 0's its own; and a vertex given no direction takes
    /// `m_body` for both, the others unmoved.
    #[test]
    fn maturity_patch_gather_at_each_level_is_that_level_s_point_rule() {
        let wasm = cfg!(target_arch = "wasm32");
        let levels: Vec<u8> = if wasm {
            vec![0, 4, 9, 14]
        } else {
            [0].into_iter().chain(4..=14).collect()
        };
        let step = if wasm { 8 } else { 1 };
        let at: Vec<(u8, u8)> = (0..=64_u8)
            .step_by(step)
            .flat_map(|k| [(k, k), (64 - k, k)])
            .collect();
        for (name, field) in weathered_worlds() {
            let synth = Synthesiser::new(field, SEED);
            let figure = field.header().figure();
            let cut = |level: u8| Kept::From(maturity_cut(&figure, BandLevel::new(level).unwrap()));
            for &level in &levels {
                let key = PatchKey::containing(level, unit_dir(PROBE_DIR)).unwrap();
                // Every fifth vertex given none, the diagonals' every fifth among them.
                let vertices: Vec<Option<[f64; 3]>> = every_vertex(key)
                    .into_iter()
                    .enumerate()
                    .map(|(n, vertex)| vertex.filter(|_| n % 5 != 0))
                    .collect();
                let got = patch_maturity(&synth, &mut SynthCache::new(), key, &vertices)
                    .unwrap()
                    .unwrap();
                let parent = cut(level.saturating_sub(1));
                let below = assert_is_the_point_rule(
                    &synth,
                    (key, &vertices),
                    &got,
                    (cut(level), parent),
                    &at,
                );
                if level == 0 {
                    assert!(got.iter().all(|pair| pair[0].total_cmp(&pair[1]).is_eq()));
                }
                eprintln!(
                    "{name}: level {level}, {} octaves kept, {below} compared vertices below m_body",
                    kept_octaves(&synth, cut(level))
                );
            }
        }
    }

    /// The level's cut is half its shortest wavelength and never rises as the level deepens, so
    /// that the octaves a level keeps nest; from the band limit's level it is 1 m and keeps every
    /// octave, where [`maturity_at_level`] is [`maturity_at`] to the bit; and the octaves each
    /// level keeps of those that can expose fresh ground are printed, on the three weathered
    /// worlds.
    #[test]
    fn maturity_cut_is_half_the_shortest_wavelength_and_its_octaves_nest() {
        let mut rng = Lcg::new(0x6375_7400_0000_0001);
        for (name, field) in weathered_worlds() {
            let synth = Synthesiser::new(field, SEED);
            let figure = field.header().figure();
            let limit = band_limit_level(field);
            let every = kept_octaves(&synth, Kept::Every);
            let mut above: Option<(f64, usize)> = None;
            let mut counts = Vec::new();
            for level in 0..=MAX_LEVEL {
                let band = BandLevel::new(level).unwrap();
                let cut = maturity_cut(&figure, band);
                assert_same_bits(
                    cut.value(),
                    MATURITY_CUT_SHARE * shortest_wavelength(&figure, band).value(),
                );
                let kept = kept_octaves(&synth, Kept::From(cut));
                if let Some((cut_above, kept_above)) = above {
                    assert!(cut.value() <= cut_above, "{name}: level {level}");
                    assert!(kept >= kept_above, "{name}: level {level}");
                }
                above = Some((cut.value(), kept));
                if level >= limit {
                    assert_same_bits(cut.value(), 1.0);
                    assert_eq!(kept, every, "{name}: level {level}");
                }
                counts.push(format!("{level}:{kept}"));
            }
            eprintln!(
                "{name}: {every} octaves can expose; kept by level {}; the band limit's level \
                 {limit}",
                counts.join(" ")
            );
            let mut cache = SynthCache::new();
            for _ in 0..3 {
                let at = xyz_to_face_uv(unit_dir([0, 1, 2].map(|_| 2.0 * rng.next_f64() - 1.0)));
                let limit = BandLevel::new(limit).unwrap();
                assert_same_bits(
                    maturity_at_level(&synth, &mut cache, at, limit)
                        .unwrap()
                        .unwrap(),
                    maturity_at(&synth, &mut cache, at).unwrap().unwrap(),
                );
            }
        }
    }

    /// The tests that read the clock, which the browser target cannot.
    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    mod native_only {
        use std::time::Instant;

        use super::*;
        use crate::bake::{HorizonLayer, TerrainBakeOptions, WeightsLayer, bake_terrain};
        use crate::cube::SampleGrid;
        use crate::field::{Cover, CoverRange, PartialField, ResolutionCode, cell_index};
        use crate::patch::{BakeOptions, NormalScale, VertexPath};
        use crate::wire::{decode_payload, encode_payload};

        /// The bench's `bake/patch_65` cases' bake: the low setting, the weights, no horizon map.
        const LOW: TerrainBakeOptions = TerrainBakeOptions {
            patch: BakeOptions {
                vertex_path: VertexPath::FaceDifferences,
                normals: NormalScale::Mesh,
                skirt_m: 0.0,
            },
            weights: WeightsLayer::Baked,
            horizon: HorizonLayer::Skipped,
        };

        /// The least of `runs` timings of `run`, seconds.
        fn best_of(runs: usize, mut run: impl FnMut()) -> f64 {
            (0..runs)
                .map(|_| {
                    let start = Instant::now();
                    run();
                    start.elapsed().as_secs_f64()
                })
                .fold(f64::INFINITY, |a, b| if b < a { b } else { a })
        }

        /// A client's field of `field` holding the coarse cells under each of `keys`' samples,
        /// surveyed at 10 m, with their margin, as the bench's `client` makes it.
        fn view_under(field: &CoarseField, keys: &[PatchKey]) -> PartialField {
            let level = field.header().level().get();
            let cells: BTreeSet<u32> = keys
                .iter()
                .flat_map(|&key| {
                    (0..=128)
                        .flat_map(|y| (0..=128).map(move |x| (x, y)))
                        .map(move |(x, y)| {
                            let dir = key.sample_dir(x, y, SampleGrid::Double);
                            cell_index(PatchKey::containing(level, dir).unwrap())
                        })
                })
                .collect();
            let code = ResolutionCode::from_resolution(Metres::new(10.0)).unwrap();
            let cover = Cover::from_ranges(
                cells
                    .iter()
                    .map(|&i| CoverRange::new(i, i + 1, code).unwrap()),
            )
            .unwrap();
            let blocks = decode_payload(&encode_payload(field, &cover, None)).unwrap();
            let mut view = PartialField::new(blocks[0].header().unwrap().clone());
            for block in &blocks {
                view.insert(block).unwrap();
            }
            view
        }

        /// The Moon-like world on a surface `age` Gyr old of N(>1 km) `n_1km` per km², as
        /// [`younger_moon`] but with no maturity: the twin whose bake has no layer.
        fn plain_moon(age: f64, n_1km: f64) -> CoarseField {
            moon_like_builder()
                .surface_age(Gigayears::new(age))
                .crater_params(moon_like_crater_params(PerSquareKilometre::new(n_1km)))
                .build()
        }

        /// Test 13, the layer's cost (decision-r10-t1c-client-scope §1.3): at the probe's place on
        /// each weathered world, at levels 6, 10 and 14 and the band limit's, the octaves the
        /// level keeps; the gather alone, warm (through the cache of a first gather) and cold
        /// (from an empty cache); and the whole bake with the layer and without it (the same world
        /// without the header's maturity), at the bench's `bake/patch_65` settings, warm (from a
        /// cache a first bake of the patch filled) and cold, each the best of several, printed
        /// with the added cost. Then the crater cache's clears over a run of bakes with the layer
        /// and without (`print_crater_cache_clears`), which show whether the halo search's cells
        /// make the heights redraw theirs.
        #[test]
        #[ignore = "measurement, run by hand with --nocapture: the maturity layer's cost"]
        fn maturity_layer_s_time_per_patch() {
            let twins = [
                plain_moon(4.4, 0.05),
                plain_moon(3.5, 4.8e-3),
                plain_moon(1.0, 8.4e-4),
            ];
            for ((name, weathered), plain) in weathered_worlds().into_iter().zip(&twins) {
                assert_eq!(
                    weathered.header().surface_age(),
                    plain.header().surface_age()
                );
                for level in [6, 10, 14, band_limit_level(weathered)] {
                    let key = PatchKey::containing(level, unit_dir(PROBE_DIR)).unwrap();
                    let (seen, unseen) = (view_under(weathered, &[key]), view_under(plain, &[key]));
                    let (with, without) = (
                        Synthesiser::new(&seen, SEED),
                        Synthesiser::new(&unseen, SEED),
                    );
                    let vertices = every_vertex(key);
                    let mut cache = SynthCache::new();
                    let kept = kept_octaves(
                        &Synthesiser::new(weathered, SEED),
                        Kept::From(maturity_cut(
                            &weathered.header().figure(),
                            BandLevel::new(level).unwrap(),
                        )),
                    );
                    let gather_cold = best_of(3, || {
                        let got = patch_maturity(&with, &mut SynthCache::new(), key, &vertices);
                        assert!(got.unwrap().is_some());
                    });
                    let gather_warm = best_of(5, || {
                        let got = patch_maturity(&with, &mut cache, key, &vertices);
                        assert!(got.unwrap().is_some());
                    });
                    let bake = |synth: &Synthesiser<'_, PartialField>, cache: &mut SynthCache| {
                        let bake = bake_terrain(synth, cache, key, &LOW, &[]).unwrap();
                        assert!(bake.layers().is_complete());
                        bake.layers().maturity().len()
                    };
                    let (mut warm_with, mut warm_without) = (SynthCache::new(), SynthCache::new());
                    assert_eq!(bake(&with, &mut warm_with), 65 * 65);
                    assert_eq!(bake(&without, &mut warm_without), 0);
                    let baked_with = best_of(5, || {
                        bake(&with, &mut warm_with);
                    });
                    let baked_without = best_of(5, || {
                        bake(&without, &mut warm_without);
                    });
                    let cold_with = best_of(3, || {
                        bake(&with, &mut SynthCache::new());
                    });
                    let cold_without = best_of(3, || {
                        bake(&without, &mut SynthCache::new());
                    });
                    eprintln!(
                        "{name} level {level}: {kept} octaves kept; gather {:.2} ms warm, {:.2} ms \
                         cold; bake {:.2} ms with the layer and {:.2} ms without, warm (added \
                         {:.2} ms), {:.2} and {:.2} ms cold (added {:.2} ms)",
                        1e3 * gather_warm,
                        1e3 * gather_cold,
                        1e3 * baked_with,
                        1e3 * baked_without,
                        1e3 * (baked_with - baked_without),
                        1e3 * cold_with,
                        1e3 * cold_without,
                        1e3 * (cold_with - cold_without),
                    );
                }
            }
            print_crater_cache_clears(&[
                ("1 Gyr", young_moon()),
                ("1 Gyr, no maturity", &twins[2]),
            ]);
        }

        /// Prints how often the crater cache starts afresh over a run of 80 bakes on each of
        /// `worlds`, 64 neighbouring level-14 patches at the probe's place and their 16 parents,
        /// through one cache, and the most cells it holds: its cells fall only where it has.
        fn print_crater_cache_clears(worlds: &[(&str, &CoarseField)]) {
            let base = PatchKey::containing(14, unit_dir(PROBE_DIR)).unwrap();
            let mut keys = Vec::new();
            for dj in 0..8 {
                for di in 0..8 {
                    keys.push(
                        PatchKey::new(base.face(), 14, base.i() + di, base.j() + dj).unwrap(),
                    );
                }
            }
            let parents: BTreeSet<PatchKey> = keys.iter().map(|k| k.parent().unwrap()).collect();
            keys.extend(parents);
            for &(name, field) in worlds {
                let view = view_under(field, &keys);
                let synth = Synthesiser::new(&view, SEED);
                let mut cache = SynthCache::new();
                let (mut clears, mut most) = (0, 0);
                let start = Instant::now();
                for &key in &keys {
                    let before = cache.craters_mut().len();
                    assert!(bake_terrain(&synth, &mut cache, key, &LOW, &[]).is_ok());
                    let after = cache.craters_mut().len();
                    clears += usize::from(after < before);
                    if after > most {
                        most = after;
                    }
                }
                eprintln!(
                    "{name}: {} bakes in {:.0} ms, {clears} crater-cache clears, at most {most} \
                     cells held",
                    keys.len(),
                    1e3 * start.elapsed().as_secs_f64()
                );
            }
        }

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

        /// The mix's and the haloed solve's cost (decision-r10-t1c-halo's test 11):
        /// `maturity_mix` and `haloed_albedo_scale` on each world, beside `class_fractions` and
        /// `exposed_albedo_scale` on the same world, each the best of three, printed.
        #[test]
        #[ignore = "measurement, run by hand with --nocapture: the mix's and the solve's cost"]
        fn maturity_mix_s_and_the_haloed_solve_s_time() {
            let best = |mut run: Box<dyn FnMut() + '_>| {
                (0..3)
                    .map(|_| {
                        let start = Instant::now();
                        run();
                        start.elapsed().as_secs_f64()
                    })
                    .fold(f64::INFINITY, |a, b| if b < a { b } else { a })
            };
            for (name, field) in halo_fields() {
                let header = field.header();
                let (palette, exposure) = (header.palette(), GroundExposure::of_header(header));
                let synth = Synthesiser::new(field, SEED);
                let fractions = class_fractions(&synth, &mut SynthCache::new()).unwrap();
                let mix = maturity_mix(&synth).unwrap();
                let classes = best(Box::new(|| {
                    assert!(class_fractions(&synth, &mut SynthCache::new()).is_ok());
                }));
                let mixing = best(Box::new(|| assert!(maturity_mix(&synth).is_some())));
                let haloed = best(Box::new(|| {
                    assert!(
                        haloed_albedo_scale(&fractions, palette, exposure, &mix, TARGET).is_ok()
                    );
                }));
                let alone = best(Box::new(|| {
                    assert!(exposed_albedo_scale(&fractions, palette, exposure, TARGET).is_ok());
                }));
                let small: usize = mix.law.octaves.iter().map(|o| o.haloes.len()).sum();
                eprintln!(
                    "{name}: class_fractions {:.1} ms, maturity_mix {:.1} ms ({small} small \
                     haloes in {} octaves, {} nodes), haloed_albedo_scale {:.2} ms, \
                     exposed_albedo_scale {:.3} ms ({} materials)",
                    1e3 * classes,
                    1e3 * mixing,
                    mix.law.octaves.len(),
                    mix.nodes().len(),
                    1e3 * haloed,
                    1e3 * alone,
                    fractions.shares().len()
                );
            }
        }
    }
}
