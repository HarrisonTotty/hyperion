//! The precipitation heuristic's zonal moisture budget (R09.T13.e; Design note 8,
//! decision-r09-t13ge §2.5, §2.10, §2.11): Siler, Roe and Armour's partition of the energy-balance
//! model's own meridional flux of moist static energy into a Hadley cell and eddies, each month,
//! centred on the month's energy-flux equator held at Guendelman and Kaspi's ascending-branch cap.
//!
//! The source is Siler, N., Roe, G. H., and Armour, K. C. 2018, "Insights into the zonal-mean
//! response of the hydrologic cycle to global warming from a diffusive energy balance model", J.
//! Climate 31, 7481–7493, §2, eqs. 1–9 (read). With x the sine of latitude, the model's northward
//! flux F of moist static energy is split into the Hadley cell's `F_HC` = w F and the eddies'
//! (1 − w) F (their eqs. 4–5). The cell's mass flux, southward in its lower branch and northward
//! in its upper, is ψ = `F_HC` ÷ (`h_T` − h) (eqs. 7–8, after Held 2001), `h_T` the upper
//! troposphere's moist static energy; its lower branch carries the latent heat −ψ L q (eq. 9), the
//! eddies (1 − w) times the diffusive flux of L q, and E − P is the divergence of their sum, over L.
//!
//! # The generalisation (decision-r09-t13ge §2.5, §2.10, §2.11)
//!
//! - **The flux** is the model's own, from its own monthly records before T13.d's correction,
//!   which balances its radiation: each band's mean h over its columns, and at each band edge the
//!   model's flux form, F = −Λ (cos φ ÷ Δφ)(`h_k` − `h_k−1`) per 2πR² ([`ModelGrid::edge_conductance`],
//!   the discretisation of the model's own convergence). Since that form is linear, the band means
//!   give the geographic model's zonal-mean flux exactly.
//! - **The energy-flux equator** is the (−, +) zero of F in the band of greatest h, ties to the
//!   lower band index, interpolated linearly in x across the band ([`energy_flux_equator`]): the
//!   greatest h makes F ≤ 0 at the band's southern edge and ≥ 0 at its northern, so the band always
//!   holds one; where F vanishes at both edges the band's middle. The annual one is the same zero
//!   of the annual-mean F in the band of greatest annual-mean h.
//! - **The centre**, on a rotating world (decision-r09-t13ge §2.10, e10), is the energy-flux
//!   equator held at the ascending branch's latitude Φ₁ of Guendelman and Kaspi 2019, ApJ 881, 67,
//!   doi:10.3847/1538-4357/ab2a06 (arXiv:1906.05748), §4.4, Eq. 20, c = sign(`x_EFE`)
//!   min(|`φ_EFE`|, Φ₁) ([`Circulation::cap_deg`]). A world locked 1:1 to its host takes no cap:
//!   its circulation converges on the substellar point, in tidally locked coordinates (Design note
//!   8), which the energy-flux equator there finds.
//! - **The weighting** is 1 between the equator and the centre. From the equator into the other
//!   hemisphere it is Siler et al.'s Gaussian of width `σ_x`. Beyond the centre in its own, the
//!   summer, hemisphere it is a Gaussian of the narrower width `σ_s` = `σ_x` max(0, 1 − |c| ÷
//!   `x_H`), `x_H` = sin `φ_H`, and zero where that width is zero ([`hadley_share`];
//!   decision-r09-t13i §3.1). The summer cell weakens and narrows as the ascending branch moves
//!   poleward, and vanishes at the cell's edge: in solstice seasons the winter cell is strong and
//!   spans the equator, the summer cell weak and confined (Lindzen, R. S., and Hou, A. Y. 1988,
//!   "Hadley Circulations for Zonally Averaged Heating Centered off the Equator", J. Atmos. Sci.
//!   45, 2416–2427; Dima, I. M., and Wallace, J. M. 2003, "On the Seasonality of the Hadley Cell",
//!   J. Atmos. Sci. 60, 1522–1527; both from memory, as the ruling reads them). Without it the
//!   summer midlatitudes took a Hadley share Siler et al.'s annual calibration never has, 0.14 at
//!   40° N in July against 0.01, and their summer convergence collapsed even on observed
//!   temperatures (0.18 mm/day at 40–60° N in JJA against 1.09; decision-r09-t13i §1.2). At c = 0
//!   it is their eq. 6 to the bit; a locked world's centre stands at its substellar pole, beyond
//!   which no edge lies, so its share is as before.
//! - **The width** scales with rotation after Kaspi and Showman 2015 (decision-r09-t13ge §2.5): the
//!   Hadley edge `φ_H` = 25.1° (Ω⊕ ÷ Ω)^½, held at 90°, and `σ_x` = 0.3 sin `φ_H` ÷ sin 25.1°.
//! - **`h_T`** is 1.06 times the month's greatest band h (Siler et al. take 1.06 h(0), their
//!   equator being their greatest h), which keeps `h_T` − h positive at every edge.
//!
//! Every loop runs over the bands from the south, in `f64`, every transcendental through
//! [`crate::math`].

use core::f64::consts::PI;

use super::ebm::EnergyBalance;
use super::grid::{LATITUDE_BANDS, ModelGrid};
use super::transport::{REFERENCE_PRESSURE_PA, rotation_ratio};
use crate::math;
use crate::planetary::surface::inputs::CoarseInputs;
use crate::units::consts::SECONDS_PER_JULIAN_YEAR;

/// The Hadley cell's edge at Earth's rotation, degrees: 25.1°, Kaspi and Showman 2015's (ApJ 804,
/// 60) Fig. 6a width at Ω⊕, as the lane read it (decision-r09-t13ge §2.5, e5). With the ruled
/// (Ω⊕ ÷ Ω)^½ it matches their widths from Ω⊕ ÷ 8 to 12 Ω⊕ within 6.6° (at Ω⊕ ÷ 6); the lane's
/// free fit gives 24.6° (Ω ÷ Ω⊕)^−0.477, 2.9° at most from them. Medium confidence at Earth-like
/// rotation, low beyond their sweep.
pub(super) const HADLEY_EDGE_AT_EARTH_DEG: f64 = 25.1;

/// Siler, Roe and Armour's Hadley width in sine latitude, `σ_x` of their eq. 6: 0.3, with which
/// "eddies account for essentially all energy transport poleward of 45° latitude, while the
/// Hadley cell accounts for most energy transport inside of 15°" (2018, §2, read), at Earth's
/// rotation; scaled with the edge ([`Circulation::for_inputs`]).
pub(super) const HADLEY_WIDTH_AT_EARTH: f64 = 0.3;

/// The upper troposphere's moist static energy over the month's greatest band h, `h_T` ÷ h: 1.06,
/// Siler, Roe and Armour's `h_T` = 1.06 h(0), "which results in the best agreement with observed
/// moisture transport" (2018, §2, eq. 8 and the text after it, read).
pub(super) const UPPER_BRANCH_ENERGY_RATIO: f64 = 1.06;

/// The prefactor C of the ascending branch's latitude Φ₁ = C (γ ÷ 1°)^0.71 (`p_s` ÷ p⊕)^−0.13
/// (Ω ÷ Ω⊕)^−0.62 (P ÷ 1 yr)^0.28, degrees: 2.045 483°, the geometric mean of φ ÷ γ^0.71 over the
/// lane's reading of Fig. 13b of Guendelman and Kaspi 2019, ApJ 881, 67,
/// doi:10.3847/1538-4357/ab2a06 (arXiv:1906.05748), at Ω⊕, 1 bar and a one-year orbit (the
/// median ascending branch at nine obliquities from 20° to 90°, `e2/gk19-fig13b-read.txt`; 2.045 483
/// by the decision's `e_checks.py`, recomputed by decision-r09-t13e-asbuilt §2), which reproduces
/// the reading within 2.6° from 20° to 80° and by 4.9° at 90°. GK19 print their §4.4, Eq. 20 as a
/// proportionality with no prefactor, so C is this fit's. decision-r09-t13ge §2.10 prints it
/// rounded as 2.0°, but every figure it gives is this value's (Earth 19.2°, the 60° aquaplanet
/// 37.4°, 90° 49.9°); 2.0° would give 18.8°, 36.6° and 48.8°. Low-to-medium confidence;
/// extrapolated outside GK19's sweep (Ω⊕ ÷ 16 to Ω⊕, ½ to 30 bar, orbits of ⅛ to 4 years, 10° to
/// 90°).
pub(super) const ASCENDING_BRANCH_PREFACTOR_DEG: f64 = 2.045_483;

/// Eq. 20's exponent of the obliquity γ, 0.71 (Guendelman, I., and Kaspi, Y. 2019, "Atmospheric
/// Dynamics on Terrestrial Planets: The Seasonal Response to Changes in Orbital, Rotational, and
/// Radiative Timescales", ApJ 881, 67, doi:10.3847/1538-4357/ab2a06 (arXiv:1906.05748), §4.4,
/// Eq. 20, read; R² 0.87).
pub(super) const ASCENDING_BRANCH_OBLIQUITY_EXPONENT: f64 = 0.71;

/// Eq. 20's exponent of the surface pressure over p⊕, −0.13 (as
/// [`ASCENDING_BRANCH_OBLIQUITY_EXPONENT`]).
pub(super) const ASCENDING_BRANCH_PRESSURE_EXPONENT: f64 = -0.13;

/// Eq. 20's exponent of the rotation rate over Ω⊕, −0.62 (as
/// [`ASCENDING_BRANCH_OBLIQUITY_EXPONENT`]).
pub(super) const ASCENDING_BRANCH_ROTATION_EXPONENT: f64 = -0.62;

/// Eq. 20's exponent of the orbital period over a year, 0.28 (as
/// [`ASCENDING_BRANCH_OBLIQUITY_EXPONENT`]). The year is the Julian year, the ruling's "P ÷ 1 yr";
/// GK19 take their ω = 1 as 360 days (§2, read), 1.5% shorter, which moves Φ₁ by 0.4%.
pub(super) const ASCENDING_BRANCH_PERIOD_EXPONENT: f64 = 0.28;

/// One degree, radians.
pub(super) const DEGREE_RAD: f64 = PI / 180.0;

/// A body's Hadley circulation: its edge and width in sine latitude, and the cap on its centre.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Circulation {
    /// `φ_H`, degrees: 25.1° (Ω⊕ ÷ Ω)^½, held at 90°.
    edge_deg: f64,
    /// sin `φ_H`.
    edge_sine: f64,
    /// `σ_x` = 0.3 sin `φ_H` ÷ sin 25.1°.
    width: f64,
    /// Φ₁, degrees, held to [0°, 90°]; `None` on a world locked 1:1 to its host, which takes none.
    cap_deg: Option<f64>,
    /// sin Φ₁, or `None` where there is no cap.
    cap_sine: Option<f64>,
}

/// Whether a world's circulation is a rotating one, its rain band capped at the ascending branch,
/// or a locked one's, converging on the substellar point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Turning {
    /// A world that turns under its sun: geographic coordinates, the cap.
    Rotating,
    /// A world locked 1:1 to its host: tidally locked coordinates, no cap.
    Locked,
}

impl Circulation {
    /// The circulation of the body of `inputs`, which turns as `turning` says.
    ///
    /// The cap is Guendelman and Kaspi 2019's §4.4, Eq. 20 (ApJ 881, 67;
    /// [`ASCENDING_BRANCH_PREFACTOR_DEG`]): γ the obliquity to the seasonal orbit, min(ε,
    /// 180° − ε); `p_s` over p⊕ = [`REFERENCE_PRESSURE_PA`], the transport's 1 bar, GK19's
    /// Earth-like atmospheric mass; Ω over Ω⊕ as the transport takes it ([`rotation_ratio`]); P the
    /// seasonal orbit's period over the Julian year. A world with no seasonal orbit has no seasons,
    /// and its cap is 0°, Eq. 20's limit at no obliquity; a cap that is not a number, as at no
    /// pressure, is held at 90°.
    #[must_use]
    pub(super) fn for_inputs(inputs: &CoarseInputs, turning: Turning) -> Self {
        let rate = rotation_ratio(inputs.spin().rotation_period.value());
        let edge = HADLEY_EDGE_AT_EARTH_DEG / rate.sqrt();
        let edge_deg = if edge < 90.0 { edge } else { 90.0 };
        let edge_sine = math::sin(edge_deg * DEGREE_RAD);
        let width =
            HADLEY_WIDTH_AT_EARTH * edge_sine / math::sin(HADLEY_EDGE_AT_EARTH_DEG * DEGREE_RAD);
        let cap_deg = match turning {
            Turning::Locked => None,
            Turning::Rotating => Some(ascending_branch_deg(inputs, rate)),
        };
        Self {
            edge_deg,
            edge_sine,
            width,
            cap_deg,
            cap_sine: cap_deg.map(|cap| math::sin(cap * DEGREE_RAD)),
        }
    }

    /// The Hadley edge `φ_H`, degrees.
    #[must_use]
    pub(super) const fn edge_deg(&self) -> f64 {
        self.edge_deg
    }

    /// sin `φ_H`.
    #[must_use]
    pub(super) const fn edge_sine(&self) -> f64 {
        self.edge_sine
    }

    /// The Hadley width `σ_x` in sine latitude.
    #[cfg(test)]
    #[must_use]
    pub(super) const fn width(&self) -> f64 {
        self.width
    }

    /// The Hadley cell's share w at sine latitude `x` about a centre of sine `centre`, under this
    /// circulation's width and edge ([`hadley_share`]). A world locked 1:1 to its host has no
    /// summer hemisphere: its centre is the substellar convergence, and its share keeps Siler et
    /// al.'s width on both sides, as built (decision-r09-t13i §3.1: "a locked world's centre
    /// stands at the substellar pole, so no x lies beyond it: unchanged"), by construction rather
    /// than by where its greatest h lies.
    #[must_use]
    pub(super) fn share(&self, x: f64, centre: f64) -> f64 {
        // An edge at no finite sine leaves the summer width at `σ_x` to the bit (1 − |c| ÷ ∞ = 1).
        let edge_sine = match self.cap_sine {
            Some(_) => self.edge_sine,
            None => f64::INFINITY,
        };
        hadley_share(x, centre, self.width, edge_sine)
    }

    /// The ascending branch's latitude Φ₁ that caps the centre, degrees, or `None` on a locked
    /// world.
    #[cfg(test)]
    #[must_use]
    pub(super) const fn cap_deg(&self) -> Option<f64> {
        self.cap_deg
    }

    /// The centre's sine for an energy-flux equator of sine `efe_sine`: c = sign(`x_EFE`)
    /// min(|`φ_EFE`|, Φ₁), compared in sines, which asin keeps in order; the equator itself
    /// uncapped on a locked world.
    #[must_use]
    pub(super) fn centre_sine(&self, efe_sine: f64) -> f64 {
        match self.cap_sine {
            Some(cap) if efe_sine > cap => cap,
            Some(cap) if efe_sine < -cap => -cap,
            Some(_) | None => efe_sine,
        }
    }
}

/// Guendelman and Kaspi's Φ₁ for the body of `inputs` at the rotation rate `rate` × Ω⊕, degrees,
/// held to [0°, 90°] ([`Circulation::for_inputs`]).
#[must_use]
fn ascending_branch_deg(inputs: &CoarseInputs, rate: f64) -> f64 {
    let Some(orbit) = inputs.seasonal_orbit() else {
        return 0.0;
    };
    let obliquity = inputs.spin().obliquity.value() / DEGREE_RAD;
    let gamma = if obliquity > 90.0 {
        180.0 - obliquity
    } else {
        obliquity
    };
    if gamma <= 0.0 {
        return 0.0;
    }
    let cap = ASCENDING_BRANCH_PREFACTOR_DEG
        * math::powf(gamma, ASCENDING_BRANCH_OBLIQUITY_EXPONENT)
        * math::powf(
            inputs.surface_pressure().value() / REFERENCE_PRESSURE_PA,
            ASCENDING_BRANCH_PRESSURE_EXPONENT,
        )
        * math::powf(rate, ASCENDING_BRANCH_ROTATION_EXPONENT)
        * math::powf(
            orbit.period.value() / SECONDS_PER_JULIAN_YEAR,
            ASCENDING_BRANCH_PERIOD_EXPONENT,
        );
    // By comparison, so that a NaN (no pressure) holds at 90°.
    if (0.0..90.0).contains(&cap) {
        cap
    } else {
        90.0
    }
}

/// The Hadley cell's share w of the flux at sine latitude `x` for a centre of sine `centre`, a
/// width `width` (`σ_x`) and an edge of sine `edge_sine` (`x_H`; decision-r09-t13ge §2.10, as
/// decision-r09-t13i §3.1 narrows its summer side): 1 from the equator to the centre; from the
/// equator into the other hemisphere Siler et al.'s Gaussian exp(−(d ÷ `σ_x`)²), d the distance in
/// sine latitude past the equator; beyond the centre in its own hemisphere exp(−((x − c) ÷
/// `σ_s`)²) with the summer width `σ_s` ([`summer_width`]), and zero where `σ_s` is zero. At a
/// centre on the equator it is their eq. 6 to the bit, `σ_s` being `σ_x` there.
#[must_use]
fn hadley_share(x: f64, centre: f64, width: f64, edge_sine: f64) -> f64 {
    let (low, high) = if centre < 0.0 {
        (centre, 0.0)
    } else {
        (0.0, centre)
    };
    // Past the centre in its own hemisphere: the summer side. At a centre of 0 neither side is the
    // summer's; the summer width is then `σ_x` itself, so either reading gives eq. 6's bits.
    let (past, scale) = if x > high {
        (
            x - high,
            if centre > 0.0 {
                summer_width(centre, width, edge_sine)
            } else {
                width
            },
        )
    } else if x < low {
        (
            low - x,
            if centre < 0.0 {
                summer_width(centre, width, edge_sine)
            } else {
                width
            },
        )
    } else {
        return 1.0;
    };
    // By comparison, so that no width at all gives no share rather than a NaN.
    if scale > 0.0 {
        let d = past / scale;
        math::exp(-d * d)
    } else {
        0.0
    }
}

/// The summer side's width `σ_s` = `σ_x` max(0, 1 − |c| ÷ `x_H`) of a centre of sine `centre` under
/// the width `width` (`σ_x`) and the edge's sine `edge_sine` (`x_H`; decision-r09-t13i §3.1): the
/// summer cell narrows as its ascending branch moves poleward and has no width once the centre
/// reaches the cell's edge. It scales with rotation through `φ_H`. At c = 0 it is `σ_x` to the bit.
#[must_use]
fn summer_width(centre: f64, width: f64, edge_sine: f64) -> f64 {
    let narrowing = 1.0 - centre.abs() / edge_sine;
    // By comparison, so that a centre at or past the edge (and a NaN) gives no width.
    if narrowing > 0.0 {
        width * narrowing
    } else {
        0.0
    }
}

/// The as-built share of decision-r09-t13ge §2.10, before decision-r09-t13i §3.1 narrowed its
/// summer side: Siler et al.'s Gaussian of width `width` beyond each end, from the centre in its
/// hemisphere and from the equator into the other. For the tests that hold the new share to it.
#[cfg(test)]
#[must_use]
pub(super) fn as_built_share(x: f64, centre: f64, width: f64) -> f64 {
    let (low, high) = if centre < 0.0 {
        (centre, 0.0)
    } else {
        (0.0, centre)
    };
    let past = if x > high {
        x - high
    } else if x < low {
        low - x
    } else {
        return 1.0;
    };
    let d = past / width;
    math::exp(-d * d)
}

/// The bands' moist static energy, as the budget reads it: each band's mean over its columns of h
/// and of its latent part L q, J kg⁻¹, from the south.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Bands {
    /// Each band's mean h, J kg⁻¹.
    pub(super) energy_j_per_kg: Vec<f64>,
    /// Each band's mean L q, J kg⁻¹; zero on a dry world.
    pub(super) latent_j_per_kg: Vec<f64>,
}

impl Bands {
    /// The bands of `model` on `grid` at the points' temperatures `t_k`, K, one a point: h and L q
    /// at each point ([`EnergyBalance::energy`]), then each band's mean over its columns, summed
    /// column by column ([`ModelGrid::band_means`]).
    #[must_use]
    pub(super) fn of(model: &EnergyBalance, grid: &ModelGrid, t_k: &[f64]) -> Self {
        let energy: Vec<f64> = t_k.iter().map(|&t| model.energy(t)).collect();
        let latent: Vec<f64> = match &model.moisture {
            Some(moisture) => t_k
                .iter()
                .map(|&t| moisture.latent_heat_j_per_kg * moisture.specific_humidity(t))
                .collect(),
            None => vec![0.0; t_k.len()],
        };
        Self {
            energy_j_per_kg: grid.band_means(&energy),
            latent_j_per_kg: grid.band_means(&latent),
        }
    }

    /// The mean of `months`' bands weighted by `shares`, one a month, summed in month order.
    #[must_use]
    pub(super) fn mean(months: &[Self], shares: &[f64]) -> Self {
        let weighted = |part: fn(&Self) -> &Vec<f64>| -> Vec<f64> {
            (0..LATITUDE_BANDS)
                .map(|k| {
                    months
                        .iter()
                        .zip(shares)
                        .fold(0.0, |sum, (month, &share)| sum + share * part(month)[k])
                })
                .collect()
        };
        Self {
            energy_j_per_kg: weighted(|b| &b.energy_j_per_kg),
            latent_j_per_kg: weighted(|b| &b.latent_j_per_kg),
        }
    }
}

/// The northward flux of `values` (one a band) at each of `grid`'s band edges, per 2πR², in the
/// model's flux form −Λ (cos φ ÷ Δφ)(`v_k` − `v_k−1`) with Λ = `transport_kg_m2_s`: W m⁻² of
/// sphere-area-normalised flux for h, zero at the poles ([`LATITUDE_BANDS`] + 1 values). The band's
/// divergence (`F_k+1` − `F_k`) ÷ `w_k` is the negative of the model's convergence.
#[must_use]
pub(super) fn edge_fluxes(grid: &ModelGrid, transport_kg_m2_s: f64, values: &[f64]) -> Vec<f64> {
    let mut flux = vec![0.0; LATITUDE_BANDS + 1];
    edge_fluxes_into(grid, transport_kg_m2_s, values, &mut flux);
    flux
}

/// [`edge_fluxes`] into `out` ([`LATITUDE_BANDS`] + 1 values), with no allocation: the same
/// arithmetic.
pub(super) fn edge_fluxes_into(
    grid: &ModelGrid,
    transport_kg_m2_s: f64,
    values: &[f64],
    out: &mut [f64],
) {
    for (edge, flux) in out.iter_mut().enumerate() {
        *flux = if edge == 0 || edge == LATITUDE_BANDS {
            0.0
        } else {
            -transport_kg_m2_s * grid.edge_conductance(edge) * (values[edge] - values[edge - 1])
        };
    }
}

/// The energy-flux equator's sine latitude of the flux `flux_w_m2` (one an edge, W m⁻² per 2πR²)
/// and the bands' h `energy_j_per_kg`, J kg⁻¹: the (−, +) zero of F in the band of greatest h, the
/// first of equals, interpolated linearly in sine latitude across the band, or the band's middle
/// where F vanishes at both of its edges (decision-r09-t13ge §2.11).
///
/// # Panics
///
/// If a band's h is not finite.
#[must_use]
pub(super) fn energy_flux_equator(
    grid: &ModelGrid,
    flux_w_m2: &[f64],
    energy_j_per_kg: &[f64],
) -> f64 {
    let band = energy_j_per_kg
        .iter()
        .enumerate()
        .fold((0, f64::NEG_INFINITY), |(best, most), (k, &h)| {
            assert!(h.is_finite(), "a band's h must be finite, got {h}");
            if h > most { (k, h) } else { (best, most) }
        })
        .0;
    let (south, north) = (grid.edge_sine(band), grid.edge_sine(band + 1));
    let (below, above) = (flux_w_m2[band], flux_w_m2[band + 1]);
    let rise = above - below;
    if rise > 0.0 {
        south + (north - south) * (-below / rise)
    } else {
        f64::midpoint(south, north)
    }
}

/// One month's zonal budget on the model's bands: its energy-flux equator, the rain band's centre,
/// the Hadley cell's mass flux at each edge and each band's E − P.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Budget {
    /// The energy-flux equator's sine latitude.
    pub(super) efe_sine: f64,
    /// The rain band's centre's sine latitude, c (capped on a rotating world).
    pub(super) centre_sine: f64,
    /// The Hadley cell's mass flux ψ at each band edge over 2πR², kg m⁻² s⁻¹: positive where its
    /// lower branch flows towards the model's south pole (Siler et al.'s ψ, southward), zero at
    /// the poles ([`LATITUDE_BANDS`] + 1 values).
    pub(super) mass_flux_kg_m2_s: Vec<f64>,
    /// Each band's E − P, kg m⁻² s⁻¹: the divergence of the latent heat's flux over L; zero on a
    /// dry world.
    pub(super) evaporation_less_precipitation_kg_m2_s: Vec<f64>,
    /// The northward flux of h at each edge, W m⁻² per 2πR² ([`edge_fluxes`]).
    pub(super) flux_w_m2: Vec<f64>,
}

/// The month's budget on `grid` of `model`'s bands `bands` under `circulation` (the [module
/// documentation](self)): F, its energy-flux equator and the centre; w at each edge; ψ =
/// w F ÷ (`h_T` − h), h the edge's mean of its two bands; the latent flux (1 − w) times the
/// diffusive flux of L q, less ψ L q at the edge's mean L q; E − P each band's divergence of it,
/// (`F_q,k+1` − `F_q,k`) ÷ (`w_k` L), with L the condensable's latent heat.
#[must_use]
pub(super) fn budget(
    grid: &ModelGrid,
    model: &EnergyBalance,
    circulation: &Circulation,
    bands: &Bands,
) -> Budget {
    let lambda = model.transport_kg_m2_s;
    let flux = edge_fluxes(grid, lambda, &bands.energy_j_per_kg);
    let diffusive_latent = edge_fluxes(grid, lambda, &bands.latent_j_per_kg);
    let efe_sine = energy_flux_equator(grid, &flux, &bands.energy_j_per_kg);
    let centre_sine = circulation.centre_sine(efe_sine);
    let greatest =
        bands.energy_j_per_kg.iter().fold(
            f64::NEG_INFINITY,
            |most, &h| {
                if h > most { h } else { most }
            },
        );
    let upper = UPPER_BRANCH_ENERGY_RATIO * greatest;
    let mut mass_flux = vec![0.0; LATITUDE_BANDS + 1];
    let mut latent_flux = vec![0.0; LATITUDE_BANDS + 1];
    for edge in 1..LATITUDE_BANDS {
        let share = circulation.share(grid.edge_sine(edge), centre_sine);
        let energy = f64::midpoint(bands.energy_j_per_kg[edge], bands.energy_j_per_kg[edge - 1]);
        let latent = f64::midpoint(bands.latent_j_per_kg[edge], bands.latent_j_per_kg[edge - 1]);
        let psi = share * flux[edge] / (upper - energy);
        mass_flux[edge] = psi;
        latent_flux[edge] = (1.0 - share) * diffusive_latent[edge] - psi * latent;
    }
    let latent_heat = model
        .moisture
        .as_ref()
        .map_or(0.0, |moisture| moisture.latent_heat_j_per_kg);
    let divergence = (0..LATITUDE_BANDS)
        .map(|k| {
            if latent_heat > 0.0 {
                (latent_flux[k + 1] - latent_flux[k]) / (grid.weight(k) * latent_heat)
            } else {
                0.0
            }
        })
        .collect();
    Budget {
        efe_sine,
        centre_sine,
        mass_flux_kg_m2_s: mass_flux,
        evaporation_less_precipitation_kg_m2_s: divergence,
        flux_w_m2: flux,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyperion_testkit::float::assert_same_bits;

    /// Earth's Hadley edge in sine latitude, sin 25.1° = 0.424 (decision-r09-t13i §3.1).
    const EARTH_EDGE_SINE: f64 = 0.424;

    /// decision-r09-t13i §3.5, 1: at a centre on the equator the share is the as-built function,
    /// Siler et al.'s eq. 6, to the bit, at every edge and under any edge sine.
    #[test]
    fn at_a_centre_on_the_equator_the_share_is_the_as_built_one_to_the_bit() {
        let grid = ModelGrid::zonal();
        for edge_sine in [EARTH_EDGE_SINE, 0.2, 1.0] {
            for edge in 0..=LATITUDE_BANDS {
                let x = grid.edge_sine(edge);
                assert_same_bits(
                    hadley_share(x, 0.0, 0.3, edge_sine),
                    as_built_share(x, 0.0, 0.3),
                );
                let siler = math::exp(-(x / 0.3) * (x / 0.3));
                assert!((hadley_share(x, 0.0, 0.3, edge_sine) - siler).abs() < 1e-15);
            }
            assert_same_bits(summer_width(0.0, 0.3, edge_sine), 0.3);
        }
    }

    /// decision-r09-t13i §3.5, 1: at c = 0.2 and `x_H` = 0.424 the summer side's width is 0.3 (1 −
    /// 0.2 ÷ 0.424) to 10⁻¹², and its share that width's Gaussian about the centre; at c ≥ `x_H`
    /// the summer side has no share at all; 1 between the equator and the centre; the winter side
    /// as built to the bit; in each hemisphere.
    #[test]
    fn the_summer_side_narrows_with_the_centre_and_the_winter_side_is_as_built() {
        let expected = 0.3 * (1.0 - 0.2 / EARTH_EDGE_SINE);
        for sign in [1.0_f64, -1.0] {
            let centre = 0.2 * sign;
            let width = summer_width(centre, 0.3, EARTH_EDGE_SINE);
            assert!(
                (width - expected).abs() < 1e-12,
                "{width} against {expected}"
            );
            for past in [0.01, 0.05, 0.1, 0.3] {
                let x = centre + sign * past;
                let w = hadley_share(x, centre, 0.3, EARTH_EDGE_SINE);
                let gaussian = math::exp(-(past / expected) * (past / expected));
                assert!(
                    (w - gaussian).abs() < 1e-12,
                    "x {x}: {w} against {gaussian}"
                );
                // Narrower than the as-built tail.
                assert!(w < as_built_share(x, centre, 0.3), "x {x}");
            }
            for share in [0.0, 0.1, 0.5, 0.99, 1.0] {
                assert_same_bits(
                    hadley_share(centre * share, centre, 0.3, EARTH_EDGE_SINE),
                    1.0,
                );
            }
            for x in [-0.9, -0.3, -0.05, -1e-9].map(|x| x * sign) {
                assert_same_bits(
                    hadley_share(x, centre, 0.3, EARTH_EDGE_SINE),
                    as_built_share(x, centre, 0.3),
                );
            }
            for at in [EARTH_EDGE_SINE, 0.5, 0.9] {
                let centre = at * sign;
                assert_same_bits(summer_width(centre, 0.3, EARTH_EDGE_SINE), 0.0);
                for past in [1e-12, 0.01, 0.05] {
                    assert_same_bits(
                        hadley_share(centre + sign * past, centre, 0.3, EARTH_EDGE_SINE),
                        0.0,
                    );
                }
                assert_same_bits(hadley_share(centre, centre, 0.3, EARTH_EDGE_SINE), 1.0);
            }
        }
    }

    /// A rotating world's circulation narrows its summer side ([`Circulation::share`] reads its
    /// own edge): the reference Earth's share at 0.3 about a centre of 0.2 is the narrowed one's
    /// to the bit, under the as-built tail's.
    #[test]
    fn a_rotating_world_s_circulation_narrows_its_summer_side() {
        let earth = Circulation::for_inputs(
            &crate::planetary::surface::reference::earth_like(),
            Turning::Rotating,
        );
        assert!(earth.cap_deg().is_some());
        for (x, centre) in [(0.3, 0.2), (-0.3, -0.2), (0.25, 0.1)] {
            let narrowed = hadley_share(x, centre, earth.width(), earth.edge_sine());
            assert_same_bits(earth.share(x, centre), narrowed);
            assert!(
                narrowed < as_built_share(x, centre, earth.width()),
                "{x}, {centre}"
            );
        }
    }

    /// decision-r09-t13i §3.5, 1: a world locked 1:1 to its host takes no cap, its centre the
    /// energy-flux equator, at its substellar pole (the north pole of its tidally locked frame) in
    /// the pole's band, beyond which no inner edge lies. Its share at every edge is the as-built one
    /// to the bit, and so it is about a centre away from the pole (a greatest h off the substellar
    /// band), the locked share keeping Siler et al.'s width on both sides by construction; for a
    /// slow rotation's edge (79.4°, a 10-day rotation) and for Earth's.
    #[test]
    fn a_locked_world_s_share_is_as_built() {
        let grid = ModelGrid::locked();
        // The pole band's energy-flux equator, F < 0 at its southern edge and 0 at the pole.
        let south = grid.edge_sine(LATITUDE_BANDS - 1);
        let north = grid.edge_sine(LATITUDE_BANDS);
        let flux = [-3.0, 0.0];
        let efe = south + (north - south) * (-flux[0] / (flux[1] - flux[0]));
        for edge_deg in [79.4_f64, HADLEY_EDGE_AT_EARTH_DEG] {
            let edge_sine = math::sin(edge_deg * DEGREE_RAD);
            let circulation = Circulation {
                edge_deg,
                edge_sine,
                width: HADLEY_WIDTH_AT_EARTH * edge_sine
                    / math::sin(HADLEY_EDGE_AT_EARTH_DEG * DEGREE_RAD),
                cap_deg: None,
                cap_sine: None,
            };
            assert_same_bits(circulation.centre_sine(efe), efe);
            for centre in [efe, 0.5, 0.2, -0.3] {
                assert_same_bits(circulation.centre_sine(centre), centre);
                for edge in 1..LATITUDE_BANDS {
                    let x = grid.edge_sine(edge);
                    assert_same_bits(
                        circulation.share(x, centre),
                        as_built_share(x, centre, circulation.width()),
                    );
                }
            }
        }
    }

    /// The energy-flux equator is the (−, +) zero in the band of greatest h, interpolated in sine
    /// latitude, the first of equal bands taken; a local maximum elsewhere (another (−, +) zero) is
    /// not taken.
    #[test]
    fn the_energy_flux_equator_lies_in_the_band_of_greatest_h() {
        let grid = ModelGrid::zonal();
        // A peak at band 40 and a lower one at band 5.
        let energy: Vec<f64> = (0..LATITUDE_BANDS)
            .map(|k| {
                let x = grid.centre_sine(k) - grid.centre_sine(40);
                let y = grid.centre_sine(k) - grid.centre_sine(5);
                3.0e5 - 1.0e5 * x * x + 2.0e4 * math::exp(-(y / 0.01) * (y / 0.01))
            })
            .collect();
        let flux = edge_fluxes(&grid, 3.0e-4, &energy);
        let efe = energy_flux_equator(&grid, &flux, &energy);
        assert!(
            efe > grid.edge_sine(40) && efe < grid.edge_sine(41),
            "{efe}"
        );
        let expected = grid.edge_sine(40)
            + (grid.edge_sine(41) - grid.edge_sine(40)) * (-flux[40] / (flux[41] - flux[40]));
        assert_same_bits(efe, expected);
        // Two equal bands: the first's northern edge, where F vanishes between them.
        let flat: Vec<f64> = (0..LATITUDE_BANDS)
            .map(|k| if (35..=36).contains(&k) { 1.0 } else { 0.5 })
            .collect();
        let flux = edge_fluxes(&grid, 1.0, &flat);
        let efe = energy_flux_equator(&grid, &flux, &flat);
        assert!((efe - grid.edge_sine(36)).abs() < 1e-15, "{efe}");
    }
}
