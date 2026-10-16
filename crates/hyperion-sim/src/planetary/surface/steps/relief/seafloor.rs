//! The sea floor (R09.T12.a; `decision-r09-t5.md` item 5 and `decision-r09-t12-coarse.md` item
//! 1): its steady-state age, its depth by GDH1 under its sediment's load, and its roughness inputs,
//! the draped hill relief and the ponded sediment that the `seafloor` byte carries.
//!
//! # The age
//!
//! R09.T11's τ = `d_r` ÷ `u_h` is the kinematic age: right near a ridge, wrong in aggregate, since
//! the pass has no consumption history and ages every plate's far side, every slow or oblique
//! ridge's plate and every ridgeless plate as if no trench had consumed floor (on four reference
//! Earths its area mean is 110–125 Myr against Earth's 64.3). It is a rank key. The oceanic cells
//! are sorted by κ = `d_r` ÷ `u_h` uncapped (then by cell index), each run of equal κ a block and
//! the ridgeless cells the last, the oldest; walking the blocks in order, each spans [q₀, q₁] of
//! the oceanic area, and each of its cells takes the block's mean age under the steady state's
//! triangular area–age distribution dA ∝ (1 − t ÷ `t_m`) dt (Parsons 1982, JGR 87, 289; not
//! re-read), whose area mean is `t_m` ÷ 3:
//!
//! > `τ_ss` = `t_m` [1 − (2/3) ((1 − q₀)^(3/2) − (1 − q₁)^(3/2)) ÷ (q₁ − q₀)].
//!
//! The sum telescopes, so the area mean is `t_m` ÷ 3 exactly, and equal keys take equal ages.
//! The oldest floor `t_m` = 192.9 Myr × (`a_o` ÷ 0.595) × (Ω⊕ ÷ Ω), at most the system's age
//! ([`oldest_floor_age`]).
//!
//! # The depth
//!
//! z = −`d_GDH1`(`τ_ss`) + L(S) + `n_o`, S = `S_p` + `S_t`: GDH1's basement ([`gdh1_depth`]), its
//! sediment's isostatic load ([`sediment_load`]) and the oceanic noise, which the step adds.

use hyperion_surface::num;

use super::super::SeafloorOrigin;
use super::super::plates::{EARTH_ANGULAR_SPEED_RAD_PER_S, angular_speed_scale_rad_per_s};
use crate::math;
use crate::planetary::hooks::figures::relief::EARTH_CONTINENTAL_FRACTION;
use crate::units::consts::{
    RADIANS_PER_DEGREE, SECONDS_PER_GIGAYEAR, SECONDS_PER_JULIAN_YEAR, SECONDS_PER_MEGAYEAR,
};
use crate::units::{
    Gigayears, KilogramsPerCubicMetre, Metres, MetresPerSecond, Radians, Seconds,
    WattsPerSquareMetre,
};

/// Earth's area-weighted mean sea-floor age, 64.3 Myr: Müller et al. 2008's age grid (G³ 9,
/// Q04006) as R09.T11's science check weighted it (Seton et al. 2020, G³ 21, e2020GC009214, quote
/// 63.8 Myr for that grid and give 64.2 Myr for their own); `decision-r09-t12-coarse.md` item 1.
/// High confidence on Earth.
pub(crate) const EARTH_MEAN_FLOOR_AGE: Seconds = Seconds::new(64.3 * SECONDS_PER_MEGAYEAR);

/// Earth's oldest floor under the triangular law, `t_m` = 3 × [`EARTH_MEAN_FLOOR_AGE`],
/// 192.9 Myr: it leaves 14.2%, 4.9% and 0.45% of the floor older than 120, 150 and 180 Myr,
/// against Müller et al. 2008's grid's 15.2%, 4.7% and 0.22% (`decision-r09-t12-coarse.md`
/// item 1).
pub(crate) const EARTH_OLDEST_FLOOR: Seconds = Seconds::new(3.0 * EARTH_MEAN_FLOOR_AGE.value());

/// Earth's oceanic share of the sphere, 0.595: one less plan 14's
/// [`EARTH_CONTINENTAL_FRACTION`], the reference Earth's `f_c` of 0.405.
pub(crate) const EARTH_OCEANIC_SHARE: f64 = 1.0 - EARTH_CONTINENTAL_FRACTION;

/// The mantle's density beneath the sediment's and a province's load, 3,300 kg m⁻³: Earth's upper
/// mantle at the surface, the figure Turcotte and Schubert 2014 take in §3.16–3.17's problems (from
/// memory); the same as [`MANTLE_DENSITY`](crate::planetary::moons::impact::MANTLE_DENSITY)
/// (Canup 2004). Medium confidence on Earth (`decision-r09-t12-coarse.md` item 3).
pub(crate) const MANTLE_DENSITY: KilogramsPerCubicMetre = KilogramsPerCubicMetre::new(3_300.0);

/// Seawater's density over the floor, 1,030 kg m⁻³, the load under which GDH1's floor subsides
/// (from memory; `decision-r09-t12-coarse.md` item 1).
pub(crate) const SEAWATER_DENSITY: KilogramsPerCubicMetre = KilogramsPerCubicMetre::new(1_030.0);

/// The surface porosity φ₀ of Athy's law φ = φ₀ e^(−cz), 0.63: Sclater and Christie 1980's shale
/// (JGR 85, 3711, Table 2), one law for the whole column (labelled).
pub(crate) const SEDIMENT_SURFACE_POROSITY: f64 = 0.63;

/// The porosity's decay constant c of Athy's law, 0.51 km⁻¹, as metres⁻¹ (Sclater and Christie
/// 1980's shale, Table 2).
pub(crate) const SEDIMENT_POROSITY_DECAY_PER_M: f64 = 0.51e-3;

/// The sediment's grain density, 2,720 kg m⁻³ (Sclater and Christie 1980's shale, Table 2).
pub(crate) const SEDIMENT_GRAIN_DENSITY: KilogramsPerCubicMetre =
    KilogramsPerCubicMetre::new(2_720.0);

/// Below this cS, Athy's mean porosity takes its series, φ₀ (1 − cS ÷ 2), never 0 ÷ 0.
const SERIES_BELOW: f64 = 1e-6;

/// GDH1's shallow branch, d = 2,600 + 365 √t m below 20 Myr: its ridge depth, metres.
const GDH1_RIDGE_M: f64 = 2_600.0;

/// GDH1's shallow branch's coefficient, 365 m Myr^−½.
const GDH1_ROOT_M: f64 = 365.0;

/// GDH1's asymptote, 5,651 m, of d = 5,651 − 2,473 e^(−0.0278 t) m at and above 20 Myr.
const GDH1_ASYMPTOTE_M: f64 = 5_651.0;

/// GDH1's deep branch's amplitude, 2,473 m.
const GDH1_DEEP_M: f64 = 2_473.0;

/// GDH1's deep branch's rate, 0.0278 Myr⁻¹.
const GDH1_RATE_PER_MYR: f64 = 0.0278;

/// The age at which GDH1 changes branch, 20 Myr.
const GDH1_BREAK_MYR: f64 = 20.0;

/// The basement depth of floor `age` old, GDH1's (Stein and Stein 1992, Nature 359, 123–129,
/// after Renkin and Sclater 1988's sediment-corrected depths): d = 2,600 + 365 √t m below 20 Myr,
/// 5,651 − 2,473 e^(−0.0278 t) m above, t in Myr. It is the floor unloaded of sediment; the two
/// branches meet at 20 Myr within 0.41 m, and at 200 Myr it is 9.5 m short of its asymptote.
///
/// # Panics
///
/// If `age` is negative or not finite.
#[must_use]
pub(crate) fn gdh1_depth(age: Seconds) -> Metres {
    let t = age.value() / SECONDS_PER_MEGAYEAR;
    assert!(
        t.is_finite() && t >= 0.0,
        "a floor's age must be finite and non-negative, got {t} Myr"
    );
    Metres::new(if t < GDH1_BREAK_MYR {
        GDH1_RIDGE_M + GDH1_ROOT_M * t.sqrt()
    } else {
        GDH1_ASYMPTOTE_M - GDH1_DEEP_M * math::exp(-GDH1_RATE_PER_MYR * t)
    })
}

/// The rise of the sediment's top over the unloaded basement under sediment `thickness` thick,
/// L(S) = S (`ρ_m` − `ρ̄_s`) ÷ (`ρ_m` − `ρ_w`), Airy and local (labelled): `ρ̄_s` = `ρ_g` −
/// (`ρ_g` − `ρ_w`) φ̄ the water-filled column's mean density, φ̄ = φ₀ (1 − e^(−cS)) ÷ (cS) Athy's porosity
/// averaged over it (Sclater and Christie 1980's shale; [`MANTLE_DENSITY`], [`SEAWATER_DENSITY`]).
/// L ÷ S is 0.713 at 100 m, 0.623 at 1 km and 0.526 at 2.4 km; L(0) = 0.
///
/// # Panics
///
/// If `thickness` is negative or not finite.
#[must_use]
pub(crate) fn sediment_load(thickness: Metres) -> Metres {
    let s = thickness.value();
    assert!(
        s.is_finite() && s >= 0.0,
        "a sediment's thickness must be finite and non-negative, got {s} m"
    );
    let x = SEDIMENT_POROSITY_DECAY_PER_M * s;
    let mean_porosity = if x < SERIES_BELOW {
        SEDIMENT_SURFACE_POROSITY * (1.0 - 0.5 * x)
    } else {
        SEDIMENT_SURFACE_POROSITY * -math::exp_m1(-x) / x
    };
    let (grain, water, mantle) = (
        SEDIMENT_GRAIN_DENSITY.value(),
        SEAWATER_DENSITY.value(),
        MANTLE_DENSITY.value(),
    );
    let column = grain - (grain - water) * mean_porosity;
    Metres::new(s * (mantle - column) / (mantle - water))
}

/// The half-rate in mm a⁻¹ of `rate`.
#[must_use]
fn mm_per_year(rate: MetresPerSecond) -> f64 {
    rate.value() * SECONDS_PER_JULIAN_YEAR * 1e3
}

/// The basement's abyssal-hill relief H₀ of floor that formed at the half-rate `half_rate`
/// (`decision-r09-t5.md` item 5): 55 m + 175 m ÷ (1 + e^((u − 25) ÷ 4)) + 70 m ÷ (1 +
/// e^((u − 8) ÷ 2)), u in mm a⁻¹, fitted to Goff, Smith and Marks 2004's Table 2 (Oceanography
/// 17(1), 24–37): 223 m at a half-rate of 14 mm a⁻¹ (their northern MAR's 235.8 m), 191 m at 20
/// (140–201 m), 62 m at 37.5 (the SEIR's axial high, 61.7 m) and 55 m above 55 (56.1 and
/// 58.7 m); the ultraslow term gives 286 m at 5 mm a⁻¹ (Sloan et al. 2012's 280–320 m). Medium
/// confidence.
#[must_use]
pub(crate) fn basement_hill_relief(half_rate: MetresPerSecond) -> Metres {
    let u = mm_per_year(half_rate);
    Metres::new(
        55.0 + 175.0 / (1.0 + math::exp((u - 25.0) / 4.0))
            + 70.0 / (1.0 + math::exp((u - 8.0) / 2.0)),
    )
}

/// The latitude, degrees, beyond which the pelagic drape's law is held: 72°, GlobSed's eq. 2a's
/// range (Straume et al. 2019).
const DRAPE_LATITUDE_LIMIT_DEG: f64 = 72.0;

/// The pelagic drape `S_p` over floor `age` old at `latitude`: √(t ÷ 1 Myr) × (52 − 2.46 |φ| +
/// 0.045 φ²) m, φ in degrees, GlobSed's eq. 2a with its ocean factor 1 (Straume et al. 2019, G³ 20,
/// 1756–1772), valid to 82 Myr and |φ| ≤ 72°, extended beyond both by the same √t and the
/// clamped latitude (labelled). It is Earth's biogenic rain; a lifeless ocean's is lower (for the
/// biosphere plan). 468 m at 81 Myr on the equator.
///
/// # Panics
///
/// If `age` is negative or not finite.
#[must_use]
pub(crate) fn pelagic_drape(age: Seconds, latitude: Radians) -> Metres {
    let t = age.value() / SECONDS_PER_MEGAYEAR;
    assert!(
        t.is_finite() && t >= 0.0,
        "a floor's age must be finite and non-negative, got {t} Myr"
    );
    let phi = num::min(
        latitude.value().abs() / RADIANS_PER_DEGREE,
        DRAPE_LATITUDE_LIMIT_DEG,
    );
    Metres::new(t.sqrt() * (52.0 - 2.46 * phi + 0.045 * phi * phi))
}

/// The draped hill relief H of hills of basement relief `basement` under a pelagic drape `drape`:
/// max(H₀ − `S_p` ÷ 2, H₀ ÷ 2), Goff 2010's eq. 5 (JGR 115, B12104, after Webb and Jordan 2001),
/// frozen at H₀ ÷ 2 as his ¶36 does (¶43: hills keep their relief under up to 500 m of pelagic
/// sediment).
#[must_use]
pub(crate) fn draped_hill_relief(basement: Metres, drape: Metres) -> Metres {
    let h0 = basement.value();
    Metres::new(hyperion_surface::num::max(
        h0 - 0.5 * drape.value(),
        0.5 * h0,
    ))
}

/// The continental rise's ponded sediment at a passive margin, 2.4 km: Harris et al. 2014's map
/// data with GlobSed (Marine Geology 352, 4–24; `decision-r09-t5.md` item 5).
pub(crate) const PONDED_AT_MARGIN: Metres = Metres::new(2_400.0);

/// The ponded sediment's e-folding length away from its passive margin, 200 km: the low end of
/// ordinary margins' 200–500 km (`decision-r09-t5.md` item 5's 350 km lean), calibrated by
/// R09.T12.a's test on the reference Earth, where it gives the plains 32.8% of the floor below
/// 3 km (Harris et al. 2014's 32.9% of the abyss) and 370 m of sediment beyond 200 km of a
/// margin (GlobSed's 404 m); 225 km gives 36.3% and 399 m, and the lean's 350 km 50.6% and 548 m
/// (`calibrate_the_ponded_sediment_s_e_folding_length`, by hand).
pub(crate) const PONDED_DECAY: Metres = Metres::new(200e3);

/// The ponded sediment's reach, three e-folding lengths: beyond it the wedge is zero.
const PONDED_REACH_LENGTHS: f64 = 3.0;

/// The ponded (turbidite) sediment `S_t` at `distance` from the plate's nearest passive margin
/// over its own oceanic cells, `None` where no margin of the plate reaches: 2.4 km × e^(−x ÷ λ),
/// λ [`PONDED_DECAY`], zero beyond 3λ and without a margin (`decision-r09-t5.md` item 5). The path
/// crosses no plate boundary, since a trench traps the supply and a ridge's crest stops it.
#[must_use]
pub(crate) fn ponded_sediment(distance: Option<Metres>) -> Metres {
    let Some(x) = distance else {
        return Metres::ZERO;
    };
    let lambda = PONDED_DECAY.value();
    if x.value() > PONDED_REACH_LENGTHS * lambda {
        return Metres::ZERO;
    }
    Metres::new(PONDED_AT_MARGIN.value() * math::exp(-x.value() / lambda))
}

/// The oldest floor `t_m` of a world whose realised oceanic share of the sphere is `oceanic_share`
/// and whose heat flow is `heat_flow`, in a system `system_age` old (`decision-r09-t12-coarse.md`
/// item 1): 192.9 Myr × (`a_o` ÷ 0.595) × (Ω⊕ ÷ Ω), with Ω R09.T11's speed scale
/// ([`angular_speed_scale_rad_per_s`]) and Ω⊕ Earth's, at most the system's age, and the system's
/// age where Ω is zero.
///
/// Under R09.T11's model the plates' divergence goes as Ω R², and the steady state's `t_m` is
/// 2 `A_o` ÷ P, so `t_m` goes as `a_o` ÷ Ω (medium-low confidence away from Earth). It is not taken
/// from the raster's ridge production, since R09.T11's continents ignore the plate boundaries and
/// its oceanic ridges make only some 55–60% of Earth's floor (estimated): the ages near ridges are
/// then compressed by `P_o` ÷ P, which makes ridge flanks look faster-spreading (recorded).
#[must_use]
pub(crate) fn oldest_floor_age(
    oceanic_share: f64,
    heat_flow: WattsPerSquareMetre,
    system_age: Gigayears,
) -> Seconds {
    let cap = system_age.value() * SECONDS_PER_GIGAYEAR;
    let omega = angular_speed_scale_rad_per_s(heat_flow);
    if omega <= 0.0 || omega.is_nan() {
        return Seconds::new(cap);
    }
    let t = EARTH_OLDEST_FLOOR.value()
        * (oceanic_share / EARTH_OCEANIC_SHARE)
        * (EARTH_ANGULAR_SPEED_RAD_PER_S / omega);
    Seconds::new(hyperion_surface::num::min(t, cap))
}

/// Each oceanic cell's steady-state age `τ_ss` from its sea floor's origin, `None` off oceanic
/// crust: R09.T11's kinematic ages ranked by area onto the triangular area–age distribution of
/// oldest age `oldest` (the module documentation), each cell weighted by its solid angle in
/// `solid_angles`.
///
/// # Panics
///
/// If a key is not finite, or the oceanic cells have no area.
#[must_use]
pub(crate) fn steady_state_ages(
    origins: &[Option<SeafloorOrigin>],
    solid_angles: &[f64],
    oldest: Seconds,
) -> Vec<Option<Seconds>> {
    let mut keyed: Vec<(f64, usize)> = Vec::new();
    let mut ridgeless: Vec<usize> = Vec::new();
    let mut total = 0.0;
    for (n, (origin, &omega)) in origins.iter().zip(solid_angles).enumerate() {
        let Some(origin) = origin else { continue };
        total += omega;
        match origin.ridge_distance {
            // κ = d_r ÷ u_h uncapped, as R09.T11 divides them before its cap.
            Some(d) => {
                let kappa =
                    hyperion_surface::num::assert_finite(d.value() / origin.half_rate.value());
                keyed.push((kappa, n));
            }
            None => ridgeless.push(n),
        }
    }
    let mut ages = vec![None; origins.len()];
    if keyed.is_empty() && ridgeless.is_empty() {
        return ages;
    }
    assert!(total > 0.0, "the oceanic cells have an area");
    keyed.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let t_m = oldest.value();
    let mut below = 0.0;
    let mut start = 0;
    while start < keyed.len() {
        let key = keyed[start].0;
        let end = keyed[start..]
            .iter()
            .position(|&(k, _)| k.total_cmp(&key).is_ne())
            .map_or(keyed.len(), |p| start + p);
        let area: f64 = keyed[start..end]
            .iter()
            .map(|&(_, n)| solid_angles[n])
            .sum();
        let last = end == keyed.len() && ridgeless.is_empty();
        let q0 = below / total;
        let q1 = if last { 1.0 } else { (below + area) / total };
        let age = Seconds::new(block_mean_age(q0, q1, t_m));
        for &(_, n) in &keyed[start..end] {
            ages[n] = Some(age);
        }
        below += area;
        start = end;
    }
    if !ridgeless.is_empty() {
        let age = Seconds::new(block_mean_age(below / total, 1.0, t_m));
        for &n in &ridgeless {
            ages[n] = Some(age);
        }
    }
    ages
}

/// The mean age over the area fractions [`q0`, `q1`] under the triangular law of oldest age `t_m`
/// seconds: `t_m` [1 − (2/3) ((1 − q₀)^(3/2) − (1 − q₁)^(3/2)) ÷ (q₁ − q₀)].
#[must_use]
fn block_mean_age(q0: f64, q1: f64, t_m: f64) -> f64 {
    let three_halves = |q: f64| {
        let r = hyperion_surface::num::max(1.0 - q, 0.0);
        r * r.sqrt()
    };
    if q1 <= q0 {
        // A block of no area, never met (every cell has one): the age at its quantile.
        let r = hyperion_surface::num::max(1.0 - q0, 0.0);
        return t_m * (1.0 - r.sqrt());
    }
    t_m * (1.0 - (2.0 / 3.0) * (three_halves(q0) - three_halves(q1)) / (q1 - q0))
}
