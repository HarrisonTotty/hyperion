//! The isothermal surface (R09.T13.c, Design note 8): the climate of a Venus, which
//! [`ClimateModelKind::Isothermal`](hyperion_surface::field::ClimateModelKind::Isothermal) names,
//! a sea-level temperature uniform at plan 14's mean, the same in every month (research-c §6).
//!
//! # Why uniform
//!
//! Plan 14's classifier names the model only where the climate's range is narrow, (`T_warm` ÷
//! `T_cold`)⁴ − 1 at most `ISOTHERMAL_FLUX_SPREAD` (`hooks::surface::regime`, P14.T48.d). Venus's deep
//! atmosphere is so: a GCM finds that "below 55 km, the temperature contrasts between equator and
//! poles are very small, less than 1 to 2 K" (Lebonnois et al. 2010, JGR 115, E06006, ¶42, read),
//! and the Pioneer Venus probes that "thermal contrasts below 45 km are a few K" (Seiff et al.
//! 1980, JGR 85, 7903, abstract, as research-c read it).
//!
//! # Why the months are equal
//!
//! The air's column holds the heat of years. Its heat capacity per area, `c_p` `p_s` ÷ g, about
//! 1,130 J kg⁻¹ K⁻¹ × 9.2 MPa ÷ 8.87 m s⁻² ≈ 1.2 × 10⁹ J m⁻² K⁻¹ on Venus, over its radiative
//! restoring 4σ`T_e`³ ≈ 2.8 W m⁻² K⁻¹ at an emission temperature near 230 K, is a time constant of
//! about 4 × 10⁸ s, 13 years (research-c §6; Venus's 92 bar, 8.87 m s⁻², e 0.0068 and 224.701-day
//! year from NASA's NSSDCA Venus fact sheet, its `c_p` and emission temperature from memory).
//! Venus's e = 0.0068 moves its light by ±1.4% over a 224.7-day year, ±2.5 K at equilibrium,
//! which that constant damps by ωτ ≈ 130 to ±0.02 K: every month's anomaly is zero.
//!
//! # What T13.d adds
//!
//! The lapse term, which carries the relief here (decision-r09-t13a-relief §2.2), and plan 14's
//! contrasts as corrections of fixed shape (decision-r09-t13d-normalisation §2.4: on this model's
//! uniform field a = −⅔ of the equator–pole contrast and b = ¾ of the day–night). This model's own
//! field varies by nothing (the plan's test reads it before them). It lies on the radiative
//! model's grid ([`frame`](super::airless::frame)): the zonal grid, or the tidally locked one on a
//! world locked 1:1 to its host, so that T13.d's P₁(cos γ) has its coordinates there.

use super::grid::{GridClimate, ModelGrid};
use crate::planetary::surface::inputs::CoarseInputs;

/// The isothermal model's means on `grid` for the body of `inputs`: every point at its mean
/// surface temperature in the annual mean and in each of its months.
#[must_use]
pub(super) fn solve(inputs: &CoarseInputs, grid: &ModelGrid) -> GridClimate {
    let field = vec![inputs.mean_surface_temperature().value(); grid.points()];
    GridClimate {
        annual_k: field.clone(),
        months_k: vec![field; usize::from(inputs.months())],
        orbits: 0,
        converged: true,
    }
}
