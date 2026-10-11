//! A fresh crater's shape: its depth, rim, floor and central peak by morphological zone, and the
//! width of its volume-balanced exterior (plan R09, Design notes 12 and 13; `decision-r09-t2.md`
//! item 3).
//!
//! # The laws
//!
//! Bowls do not depend on gravity, and every complex dimension is self-similar in D ÷ `D_t`, so a
//! body's gravity and target enter only through its [`transition_diameter`]. All are fresh craters:
//! the depth d is rim crest to floor, the rim height h is the rim crest above the surrounding
//! surface, and the floor lies d₀ = d − h below the surroundings.
//!
//! | Zone | Depth d | Rim height h | Floor and peak |
//! | --- | --- | --- | --- |
//! | Bowl, D < 0.8 `D_t` | 0.20 D | 0.20 D ÷ 5.42 = 0.0369 D | none |
//! | Bowl on an airless body | D × 0.125, 0.152, 0.166 and 0.20 at D′ = 63, 141, 283 and 400 m, linear in log D′ between, held below 63 m and from 400 m, D′ = D g ÷ 1.62 m s⁻² | 0.0369 D | none |
//! | Transitional, 0.8–1.5 `D_t` | log-linear in log D from the bowl's at 0.8 `D_t` to the complex law's at 1.5 `D_t` | likewise | floor radius from 0 to its 1.5 `D_t` value, linear in log D; no peak |
//! | Complex, from 1.5 `D_t` | 0.150 `D_t` (D ÷ `D_t`)^0.303 | 0.0402 `D_t` (D ÷ `D_t`)^0.399 | floor diameter 0.389 (D ÷ `D_t`)^0.249 of D, held at 0.778 of D from about 16 `D_t`; below 9 `D_t` a central cone of relief 0.0238 `D_t` (D ÷ `D_t`)^0.900 above the floor and base diameter 0.3 D |
//!
//! Their sources:
//!
//! - _The bowl._ 0.20 lies within 2% of Pike 1980b's lunar 0.196 D^1.010 (USGS Professional Paper
//!   1046-C, Table 6, eq. 1), 4.5% of Stopar et al. 2017's A-class 0.209 (Icarus 298, 34–48,
//!   Table 4) and 0.5% of Mercury's 0.199 D^0.995 (Pike 1988); its rim, 0.0369 D, within 3% of Pike
//!   1980b's 0.036 D^1.014 (eq. 3) and 8% of Stopar et al.'s "∼0.04". The ratio 5.42 is Pike's
//!   lunar depth over rim height, 0.195 (Pike 1980a, Table 2, fresh mare craters) over 0.036
//!   (Pike 1980b, eq. 3); Pike 1980b's own fits give 5.44 at 1 km and 5.42 at 3 km.
//! - _The airless bowl._ Stopar et al.'s A-class d/D in their 40–100, 100–200 and 200–400 m bins
//!   and from 400 m, which they attribute to "the pervasive, poorly cohesive upper regolith". The
//!   scaling D′ = D g ÷ 1.62 (a strength–gravity transition at the Moon's near-surface strength) is
//!   the plan's own, of low confidence. It applies where nothing screens the body (no atmosphere,
//!   or one of no column mass), to rocky and icy airless bodies alike (science-r09-craters.md
//!   item 3: the icy bodies' only fresh simple-crater depths, all from 1 km, lie within what it
//!   predicts); bodies with air keep 0.20 D at every size.
//! - _The complex depth_ is the geometric mean of Kalynn et al. 2013's LOLA mare and highland fits
//!   (Geophysical Research Letters 40, 38–42), 0.870 D^0.352 and 1.558 D^0.254, that is
//!   1.164 D^0.303 km, written at the Moon's `D_t` of 19 km.
//! - _The complex rim, floor and peak_ are Pike 1980b's eq. (4), 0.236 D^0.399, and Table 6's
//!   floor diameter 0.187 D^1.249 and central-peak relief 0.032 D^0.900, written the same way. The
//!   floor is held at 0.778 of D, twice 0.389, which the law reaches at 16.2 `D_t`, so that it
//!   stays continuous (the ruling's script, `min(0.778, 0.389 x^0.249)`).
//! - _The peak's base diameter_, 0.3 D, is Garvin et al. 2003's Mars fit (0.25 D^1.05, Sixth
//!   International Conference on Mars, abstract 3277), of low confidence; it carries little of the
//!   volume. The peak ends at 9 `D_t`, where peak rings begin; neither the ring nor a multi-ring
//!   basin's rings are drawn, only their floor, wall and rim.
//! - _The zone's bounds_: 0.8 and 1.5 `D_t` match Krüger, Hergarten and Kenkmann 2018's lunar
//!   14–17 and 24–28 km (0.74–0.89 and 1.26–1.47 of 19 km); 9 and 16 `D_t` are the plan's own.
//!
//! So at a fixed D in the complex range, d ∝ `D_t`^0.697 ∝ g^−0.697 and h ∝ g^−0.601, the ruling's
//! g^−0.70 and g^−0.60. Out of sample the laws give Mercury's MLA depths (Susorney et al. 2016)
//! within 10% and rims within 14%, Mars's MOLA depths (Robbins and Hynek 2012) within 28% but
//! 30% shallow at 80 km against the deepest (Tornabene et al. 2018, plan R09, Risks), and
//! Ganymede's (Schenk 1991, at `k_target` 0.12) within 22%.
//!
//! # The profile
//!
//! Heights are above the surrounding surface at r rim radii from the centre (Design note 13):
//!
//! - a flat floor at −d₀ to `r_f`, then a wall −d₀ + (h + d₀) u² with u = (r − `r_f`) ÷ (1 − `r_f`)
//!   to the rim crest at r = 1, and a central cone of relief P above the floor to its base radius
//!   `r_p`;
//! - an exterior h (1 − s)³ (1 + 3s) with s = (r − 1) ÷ a, flat at the crest and at its edge
//!   1 + a, the ejecta's reach.
//!
//! The exterior's width a balances the volume, so the crater adds no material: with N = −(`V_int` +
//! `V_peak`) ÷ (π R² h), the exterior's (0.8 a + 0.2 a²) π R² h equals N π R² h, so a = (−0.8 +
//! √(0.64 + 0.8 N)) ÷ 0.4. The bowl's d ÷ h = 5.42 gives a = 1.5426, ejecta to 2.54 rim radii;
//! transitional craters fall from the bowl's 1.5426 at 0.8 `D_t` to 1.33 at 1.5 `D_t`, complex
//! craters to 25 `D_t` give 1.28–1.36 (1.05 at 100 `D_t`), and the airless bowls 0.73–1.54. (Design
//! note 13's "1.28–1.46 for transitional and complex craters" holds for the complex ones; 1.46 is
//! a at `D_t`.) a steps by −0.016 at 1.5 `D_t`, where the central peak enters at full relief, and
//! by +0.040 at 9 `D_t`, where it ends. None exceeds the bowl's, so 2.54 rim radii, 1.27 D, is
//! every crater's reach (Design notes 10 and 15). These widths lie within 15% of the observed
//! continuous ejecta's edge, 1.35 rim radii beyond the rim (Moore et al. 1974, in Pike 1980b,
//! Table 6), but for the airless bowls' smallest and the largest basins, which fall short of it.

use hyperion_base::math;
use hyperion_base::units::Metres;

use super::{CraterParams, MOON_GRAVITY_M_PER_S2, crater_cutoff, transition_diameter};
use crate::field::Morphology;

/// A fresh bowl's depth over its diameter, d ÷ D: 0.20 (the module documentation).
const BOWL_DEPTH: f64 = 0.20;

/// A fresh lunar bowl's depth over its rim height, d ÷ h: 5.42, Pike 1980a's fresh mare depth
/// 0.195 D^1.013 (Table 2) over Pike 1980b's rim 0.036 D^1.014 (eq. 3).
const BOWL_DEPTH_TO_RIM: f64 = 5.42;

/// A fresh bowl's rim height over its diameter, h ÷ D: 0.20 ÷ 5.42 = 0.0369.
const BOWL_RIM: f64 = BOWL_DEPTH / BOWL_DEPTH_TO_RIM;

/// An airless body's bowls, (D′ in metres at the Moon's gravity, d ÷ D): Stopar et al. 2017's
/// A-class craters, Table 4, by bin (40–100, 100–200 and 200–400 m, plotted at 63, 141 and 283 m,
/// and from 400 m), linear in log D′ between and held beyond; applied to rocky and icy airless
/// bodies alike (science-r09-craters.md item 3).
const AIRLESS_BOWLS: [(f64, f64); 4] =
    [(63.0, 0.125), (141.0, 0.152), (283.0, 0.166), (400.0, 0.20)];

/// Where transitional craters begin, in `D_t`: 0.8.
const TRANSITIONAL_FROM: f64 = 0.8;

/// Where central-peak craters begin, in `D_t`: 1.5.
const COMPLEX_FROM: f64 = 1.5;

/// Where peak rings begin and central peaks end, in `D_t`: about 9 (the plan's own).
const PEAK_RING_FROM: f64 = 9.0;

/// Where multi-ring basins begin, in `D_t`: about 16 (the plan's own).
const MULTI_RING_FROM: f64 = 16.0;

/// A complex crater's depth, d = 0.150 `D_t` (D ÷ `D_t`)^0.303: Kalynn et al. 2013's mare and
/// highland LOLA fits, geometric mean 1.164 D^0.303 km, at the Moon's 19 km (the coefficient and
/// the exponent).
const COMPLEX_DEPTH: (f64, f64) = (0.150, 0.303);

/// A complex crater's rim height, h = 0.0402 `D_t` (D ÷ `D_t`)^0.399: Pike 1980b's eq. (4),
/// 0.236 D^0.399 km, at 19 km.
const COMPLEX_RIM: (f64, f64) = (0.0402, 0.399);

/// A complex crater's floor diameter over its diameter, 0.389 (D ÷ `D_t`)^0.249: Pike 1980b's
/// Table 6, 0.187 D^1.249 km, at 19 km.
const COMPLEX_FLOOR: (f64, f64) = (0.389, 0.249);

/// The widest floor over the diameter, 0.778: the law's value at 2^(1 ÷ 0.249) = 16.2 `D_t`, about
/// where multi-ring basins begin, held beyond (`decision-r09-t2.md` item 3).
const FLOOR_HELD: f64 = 0.778;

/// A central peak's relief above the floor, 0.0238 `D_t` (D ÷ `D_t`)^0.900: Pike 1980b's Table 6,
/// 0.032 D^0.900 km, at 19 km.
const PEAK_RELIEF: (f64, f64) = (0.0238, 0.900);

/// A central peak's base radius in rim radii: its base diameter 0.3 D (Garvin et al. 2003's Mars
/// fit, 0.25 D^1.05).
const PEAK_BASE_RADIUS: f64 = 0.3;

/// A fresh crater: its morphology, its relief and the width of its ejecta, from which its profile
/// follows ([`relief`](Self::relief)). Built by [`crater_shape`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CraterShape {
    diameter: Metres,
    morphology: Morphology,
    depth: Metres,
    rim_height: Metres,
    /// The flat floor's radius in rim radii.
    floor_fraction: f64,
    peak_relief: Metres,
    /// The central peak's base radius in rim radii.
    peak_fraction: f64,
    /// The exterior's width a in rim radii.
    exterior_width: f64,
}

impl CraterShape {
    /// The rim-to-rim diameter D.
    #[must_use]
    pub const fn diameter(&self) -> Metres {
        self.diameter
    }

    /// The morphological zone of D against the body's transition diameter.
    #[must_use]
    pub const fn morphology(&self) -> Morphology {
        self.morphology
    }

    /// The depth d, from the rim crest to the floor.
    #[must_use]
    pub const fn depth(&self) -> Metres {
        self.depth
    }

    /// The rim height h, the rim crest above the surrounding surface.
    #[must_use]
    pub const fn rim_height(&self) -> Metres {
        self.rim_height
    }

    /// The floor's depth below the surrounding surface, d₀ = d − h.
    #[must_use]
    pub fn floor_depth(&self) -> Metres {
        self.depth - self.rim_height
    }

    /// The flat floor's radius: zero for a bowl.
    #[must_use]
    pub fn floor_radius(&self) -> Metres {
        self.rim_radius() * self.floor_fraction
    }

    /// The central peak's relief above the floor: zero for a bowl, a transitional crater, and a
    /// crater of 9 `D_t` or more.
    #[must_use]
    pub const fn peak_relief(&self) -> Metres {
        self.peak_relief
    }

    /// The central peak's base radius: zero where it has no peak.
    #[must_use]
    pub fn peak_radius(&self) -> Metres {
        self.rim_radius() * self.peak_fraction
    }

    /// The exterior's width a beyond the rim, in rim radii, at most the bowl's 1.5426: the ejecta
    /// reach 1 + a rim radii from the centre.
    #[must_use]
    pub const fn exterior_width(&self) -> f64 {
        self.exterior_width
    }

    /// The distance from the centre at which the profile ends, (1 + a) rim radii.
    #[must_use]
    pub fn reach(&self) -> Metres {
        self.rim_radius() * (1.0 + self.exterior_width)
    }

    /// The rim radius, D ÷ 2.
    #[must_use]
    fn rim_radius(&self) -> Metres {
        self.diameter * 0.5
    }

    /// The fresh profile's height above the surrounding surface at `r_over_radius` rim radii from
    /// the centre.
    ///
    /// The floor, the wall and the central cone inside the rim, the ejecta outside to
    /// [`reach`](Self::reach), and zero beyond ([the module documentation](self)). The profile is
    /// not band-limited (T7.b's `profile` is), and is continuous, with a sharp rim crest.
    ///
    /// # Panics
    ///
    /// If `r_over_radius` is not finite and non-negative.
    #[must_use]
    pub fn relief(&self, r_over_radius: f64) -> Metres {
        let dist = r_over_radius;
        assert!(
            dist.is_finite() && dist >= 0.0,
            "a distance from a crater's centre must be finite and non-negative, got {dist}"
        );
        let rim_m = self.rim_height.value();
        let floor_m = self.floor_depth().value();
        let width = self.exterior_width;
        let height_m = if dist <= 1.0 {
            let mut inside = if dist <= self.floor_fraction {
                -floor_m
            } else {
                let u = (dist - self.floor_fraction) / (1.0 - self.floor_fraction);
                -floor_m + (rim_m + floor_m) * u * u
            };
            if dist < self.peak_fraction {
                inside += self.peak_relief.value() * (1.0 - dist / self.peak_fraction);
            }
            inside
        } else if dist < 1.0 + width {
            let s = (dist - 1.0) / width;
            let t = 1.0 - s;
            rim_m * t * t * t * (1.0 + 3.0 * s)
        } else {
            0.0
        };
        Metres::new(height_m)
    }
}

/// A fresh bowl's depth over its diameter at `d_m` metres on the body of `p`: 0.20, or Stopar et
/// al.'s shallower small bowls where nothing screens the body.
#[must_use]
fn bowl_depth_ratio(p: &CraterParams, d_m: f64) -> f64 {
    if crater_cutoff(p).is_some() {
        return BOWL_DEPTH;
    }
    let lunar_m = d_m * p.gravity().value() / MOON_GRAVITY_M_PER_S2;
    let (first_m, first_ratio) = AIRLESS_BOWLS[0];
    if lunar_m <= first_m {
        return first_ratio;
    }
    for pair in AIRLESS_BOWLS.windows(2) {
        let [(from_m, from_ratio), (to_m, to_ratio)] = [pair[0], pair[1]];
        if lunar_m < to_m {
            let w = math::ln(lunar_m / from_m) / math::ln(to_m / from_m);
            return from_ratio + w * (to_ratio - from_ratio);
        }
    }
    BOWL_DEPTH
}

/// `coefficient` `D_t` (D ÷ `D_t`)^`exponent`, metres, for the transition `transition_m` metres
/// and `x` = D ÷ `D_t`.
#[must_use]
fn self_similar(law: (f64, f64), transition_m: f64, x: f64) -> f64 {
    law.0 * transition_m * math::powf(x, law.1)
}

/// The complex floor's radius in rim radii at `x` = D ÷ `D_t`, held at [`FLOOR_HELD`].
#[must_use]
fn complex_floor(x: f64) -> f64 {
    let law = COMPLEX_FLOOR.0 * math::powf(x, COMPLEX_FLOOR.1);
    if law < FLOOR_HELD { law } else { FLOOR_HELD }
}

/// The morphological zone of `x` = D ÷ `D_t`.
#[must_use]
fn morphology(x: f64) -> Morphology {
    if x < TRANSITIONAL_FROM {
        Morphology::Simple
    } else if x < COMPLEX_FROM {
        Morphology::Transitional
    } else if x < PEAK_RING_FROM {
        Morphology::CentralPeak
    } else if x < MULTI_RING_FROM {
        Morphology::PeakRing
    } else {
        Morphology::MultiRingBasin
    }
}

/// The exterior width a, in rim radii, that balances an interior of depth `depth_m` and rim height
/// `rim_m`, floor radius `floor` (rim radii), and a cone of relief `peak_m` and base radius `cone`
/// (rim radii): a = (−0.8 + √(0.64 + 0.8 N)) ÷ 0.4 with N = −(`V_int` + `V_peak`) ÷ (π R² h)
/// (Design note 13).
///
/// Zero if the interior holds no net cavity (N ≤ 0), which no crater narrower than about
/// 10⁵ `D_t` reaches.
#[must_use]
fn exterior_width(depth_m: f64, rim_m: f64, floor: f64, peak_m: f64, cone: f64) -> f64 {
    let floor_m = depth_m - rim_m;
    // Over π R², the floor and the wall together hold −d₀ + 2 (h + d₀) (1 − r_f) (r_f ÷ 3 +
    // (1 − r_f) ÷ 4), and the cone P r_p² ÷ 3.
    let wall = 2.0 * (rim_m + floor_m) * (1.0 - floor) * (floor / 3.0 + (1.0 - floor) / 4.0);
    let interior = -floor_m + wall;
    let cone_volume = peak_m * cone * cone / 3.0;
    let n = -(interior + cone_volume) / rim_m;
    if n > 0.0 {
        (-0.8 + (0.64 + 0.8 * n).sqrt()) / 0.4
    } else {
        0.0
    }
}

/// The fresh shape of a crater of diameter `d` on the body of `p`.
///
/// Its morphology is by D ÷ [`transition_diameter`], its depth, rim height, floor and central peak
/// by zone, and its exterior width balances its volume ([the module documentation](self)). The
/// coarse pass's craters (T12.d) and the synthesis's (T7.b, T7.c) are both drawn with it, so that
/// they are one profile.
///
/// # Panics
///
/// If `d` is not finite and positive.
#[must_use]
pub fn crater_shape(p: &CraterParams, d: Metres) -> CraterShape {
    let d_m = d.value();
    assert!(
        d_m.is_finite() && d_m > 0.0,
        "a crater diameter must be finite and positive, got {d_m} m"
    );
    let transition_m = transition_diameter(p).value();
    let x = d_m / transition_m;
    let (depth_m, rim_m, floor) = if x < TRANSITIONAL_FROM {
        let depth_m = bowl_depth_ratio(p, d_m) * d_m;
        (depth_m, BOWL_RIM * d_m, 0.0)
    } else if x < COMPLEX_FROM {
        // Log-linear in log D from the bowl at 0.8 D_t to the complex law at 1.5 D_t.
        let w = math::ln(x / TRANSITIONAL_FROM) / math::ln(COMPLEX_FROM / TRANSITIONAL_FROM);
        let bowl_m = TRANSITIONAL_FROM * transition_m;
        let bowl_depth_m = bowl_depth_ratio(p, bowl_m) * bowl_m;
        let bowl_rim_m = BOWL_RIM * bowl_m;
        let complex_depth_m = self_similar(COMPLEX_DEPTH, transition_m, COMPLEX_FROM);
        let complex_rim_m = self_similar(COMPLEX_RIM, transition_m, COMPLEX_FROM);
        (
            bowl_depth_m * math::powf(complex_depth_m / bowl_depth_m, w),
            bowl_rim_m * math::powf(complex_rim_m / bowl_rim_m, w),
            w * complex_floor(COMPLEX_FROM),
        )
    } else {
        (
            self_similar(COMPLEX_DEPTH, transition_m, x),
            self_similar(COMPLEX_RIM, transition_m, x),
            complex_floor(x),
        )
    };
    let (peak_m, cone) = if (COMPLEX_FROM..PEAK_RING_FROM).contains(&x) {
        (self_similar(PEAK_RELIEF, transition_m, x), PEAK_BASE_RADIUS)
    } else {
        (0.0, 0.0)
    };
    CraterShape {
        diameter: d,
        morphology: morphology(x),
        depth: Metres::new(depth_m),
        rim_height: Metres::new(rim_m),
        floor_fraction: floor,
        peak_relief: Metres::new(peak_m),
        peak_fraction: cone,
        exterior_width: exterior_width(depth_m, rim_m, floor, peak_m, cone),
    }
}

#[cfg(test)]
mod tests;
