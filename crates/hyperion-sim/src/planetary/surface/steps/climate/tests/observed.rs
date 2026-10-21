//! The partition on observed temperatures (R09.T13.i; decision-r09-t13i §3.4, 1, and §3.5, 3): the
//! zonal budget of [`partition`](super::super::partition) under the reference Earth's air, its
//! transport coefficient Λ⊕ and its circulation, run on NCEP/NCAR R1 1991–2020's monthly sea-level
//! 2 m air temperatures ([`ncep_air`](super::ncep_air)), h of each band's mean temperature (the
//! ruling's "zonal" variant).

use super::super::ebm::EnergyBalance;
use super::super::grid::{LATITUDE_BANDS, ModelGrid};
use super::super::lapse;
use super::super::partition::{self, Bands, Budget, Circulation, Turning};
use super::ncep_air::{
    MONTH_DAYS, REDUCTION_LAPSE_RATE_K_PER_M, REDUCTION_LAPSE_RATE_TOLERANCE_K_PER_M,
    SEA_LEVEL_AIR_K,
};
use crate::math;
use crate::planetary::surface::reference;

/// One day, s.
const DAY_S: f64 = 86_400.0;

/// The calendar's winter months in the north, December, January and February (indices from
/// January).
const DJF: [usize; 3] = [11, 0, 1];

/// The calendar's summer months in the north, June, July and August.
const JJA: [usize; 3] = [5, 6, 7];

/// The least JJA convergence over 40–60° N, mm/day: 0.6 (decision-r09-t13i §3.4, 1; the observed
/// 0.6–1.1 of GPCP less NCEP's evaporation, whose upper end rests on a land evaporation from
/// memory, decision-r09-t13e-revision §2; NCEP's own P − E 0.35).
const LEAST_SUMMER_CONVERGENCE_MM_PER_DAY: f64 = 0.6;

/// The greatest annual E − P over 40–60° N, mm/day: −0.6 (decision-r09-t13i §3.4, 1; GPCP − NCEP
/// −0.73, and −0.83 with NCEP's evaporation scaled by 0.944 to close its global budget on GPCP's,
/// decision-r09-t13e-revision §1.2).
const GREATEST_ANNUAL_STORM_TRACK_BUDGET_MM_PER_DAY: f64 = -0.6;

/// The solid-angle mean of `bands` over the bands whose centres lie within `low` to `high`
/// degrees of latitude.
fn window(grid: &ModelGrid, bands: &[f64], low: f64, high: f64) -> f64 {
    let (mut sum, mut area) = (0.0, 0.0);
    for (band, &value) in bands.iter().enumerate() {
        let latitude = math::asin(grid.centre_sine(band)).to_degrees();
        if (low..=high).contains(&latitude) {
            sum += grid.weight(band) * value;
            area += grid.weight(band);
        }
    }
    sum / area
}

/// The northward latent heat flux at each band edge, W m⁻² per 2πR², of a budget whose E − P is
/// `budget`'s under the latent heat `latent_heat_j_per_kg`: the integral from the south pole of
/// each band's E − P times its weight and L, the inverse of the budget's divergence.
fn latent_flux(grid: &ModelGrid, budget: &Budget, latent_heat_j_per_kg: f64) -> Vec<f64> {
    let mut flux = vec![0.0; LATITUDE_BANDS + 1];
    for k in 0..LATITUDE_BANDS {
        flux[k + 1] = flux[k]
            + budget.evaporation_less_precipitation_kg_m2_s[k]
                * grid.weight(k)
                * latent_heat_j_per_kg;
    }
    flux
}

/// The band edge at `latitude_deg`, a multiple of 2.5° (edge e at −90° + 2.5° e).
///
/// # Panics
///
/// If no edge lies there.
fn edge_at(latitude_deg: f64) -> usize {
    (0..=LATITUDE_BANDS)
        .zip(0_u32..)
        .find(|&(_, e)| (-90.0 + 2.5 * f64::from(e) - latitude_deg).abs() < 1e-9)
        .map_or_else(
            || panic!("no band edge at {latitude_deg}°"),
            |(edge, _)| edge,
        )
}

/// The partition's budgets on the observed temperatures, one a calendar month, on the zonal grid.
struct Observed {
    grid: ModelGrid,
    budgets: Vec<Budget>,
    /// The annual E − P at each band, kg m⁻² s⁻¹, months weighted by their days.
    annual: Vec<f64>,
}

impl Observed {
    /// The mean over `months` of each month's convergence −(E − P) over `low` to `high` degrees,
    /// mm/day, the months weighted equally.
    fn season(&self, months: [usize; 3], low: f64, high: f64) -> f64 {
        months.iter().fold(0.0, |sum, &m| {
            sum - window(
                &self.grid,
                &self.budgets[m].evaporation_less_precipitation_kg_m2_s,
                low,
                high,
            ) * DAY_S
        }) / 3.0
    }

    /// Prints the windows beside the observations and the ruling's prototype, each month's
    /// energy-flux equator and centre, and the annual latent heat transport at ±40° and ±15°, PW, of
    /// a body of radius `radius_m` whose condensable's latent heat is `latent_heat_j_per_kg`.
    fn report(&self, circulation: &Circulation, latent_heat_j_per_kg: f64, radius_m: f64) {
        let (grid, year) = (&self.grid, MONTH_DAYS.iter().sum::<f64>());
        eprintln!(
            "the reference Earth's circulation on NCEP R1's temperatures: Hadley edge {:.2}°, \
             width {:.4}",
            circulation.edge_deg(),
            circulation.width()
        );
        let by_month: Vec<String> = self
            .budgets
            .iter()
            .map(|b| {
                let c = -window(grid, &b.evaporation_less_precipitation_kg_m2_s, 40.0, 60.0);
                format!("{:.2}", c * DAY_S)
            })
            .collect();
        eprintln!(
            "  40–60° N convergence by month, mm/day: {}",
            by_month.join(" ")
        );
        eprintln!(
            "  40–60° N: JJA {:.2} (ruled prototype 1.09, as built 0.18; observed 0.6–1.1, NCEP's \
             own 0.35), DJF {:.2} (0.76; observed 1.1–1.3, NCEP's own 0.84), annual E − P {:+.2} \
             (−0.83, as built −0.38; GPCP − NCEP −0.73, GPCP − 0.944 NCEP −0.83)",
            self.season(JJA, 40.0, 60.0),
            self.season(DJF, 40.0, 60.0),
            window(grid, &self.annual, 40.0, 60.0) * DAY_S
        );
        for (low, high, observed) in [
            (15.0, 35.0, "+1.42 (+1.23)"),
            (-60.0, -40.0, "−1.41 (−1.50)"),
            (-35.0, -15.0, "+1.65 (+1.44)"),
        ] {
            eprintln!(
                "  {low:+.0}° to {high:+.0}°: annual E − P {:+.2} (GPCP − NCEP, GPCP − 0.944 NCEP: \
                 {observed}), JJA convergence {:.2}, DJF {:.2}",
                window(grid, &self.annual, low, high) * DAY_S,
                self.season(JJA, low, high),
                self.season(DJF, low, high)
            );
        }
        for (m, b) in self.budgets.iter().enumerate() {
            eprintln!(
                "  month {:2}: energy-flux equator {:+6.2}°, centre {:+6.2}°",
                m + 1,
                math::asin(b.efe_sine).to_degrees(),
                math::asin(b.centre_sine).to_degrees()
            );
        }
        let to_pw = 2.0 * core::f64::consts::PI * radius_m * radius_m / 1e15;
        let fluxes: Vec<Vec<f64>> = self
            .budgets
            .iter()
            .map(|b| latent_flux(grid, b, latent_heat_j_per_kg))
            .collect();
        for (latitude, prototype, siler) in [
            (40.0, "+2.22", "about +2.1"),
            (15.0, "−1.66", "—"),
            (-15.0, "—", "—"),
            (-40.0, "−2.15", "about −2.6"),
        ] {
            let edge = edge_at(latitude);
            let mean = fluxes
                .iter()
                .zip(&MONTH_DAYS)
                .fold(0.0, |sum, (f, &days)| sum + days / year * f[edge])
                * to_pw;
            eprintln!(
                "  the annual latent heat transport at {latitude:+.0}°: {mean:+.2} PW (the ruling's \
                 prototype {prototype}; Siler et al.'s ERA-Interim {siler})"
            );
        }
    }
}

/// decision-r09-t13i §3.4, 1 (its unit test §3.5, 3): the partition on NCEP/NCAR R1 1991–2020's
/// monthly zonal-mean 2 m air temperature, reduced to the sea level over land at the reference
/// Earth's Γ on the model's 72 bands ([`ncep_air`](super::ncep_air)), under the reference Earth's
/// air, Λ⊕ and circulation (its cap and its Hadley edge), h of each band's mean temperature. Its
/// 40–60° N convergence −(E − P) is positive in every month, its JJA mean is at least 0.6 mm/day,
/// and its annual 40–60° N E − P, months weighted by their days, is at most −0.6 mm/day. The
/// ruled form's prototype gave 1.09 and −0.83, as built 0.18 and −0.38; observed 0.35–1.1 and
/// −0.73 (−0.83 with NCEP's evaporation scaled by 0.944). The prototype's figures printed beside
/// the model are its `runs/obs_weighting3.txt` (the "zonal monthly narrow0.424" line, in
/// `rm45-scratch/decision-r09-t13i/`), which the test reproduced to the printed digits at version
/// 22's air. From P14.T54.a's 23 air (T49.b's `c_p(T)`, and the fixture's water at 0.77 of its
/// saturation) the model gives a JJA mean of 1.15 and an annual E − P of −0.89 mm/day.
///
/// It prints the other windows (on observed temperatures the energy-flux equator stays north all
/// year, so 15–35° N is no fair window), each month's energy-flux equator and centre, and the
/// latent heat transport at ±40° and ±15° beside the ruling's prototype and Siler, Roe and Armour
/// 2018's ERA-Interim, about +2.1 PW at 40° N and −2.6 near 35–40° S (their Fig. 2e, read roughly
/// off the page by the ruling; plan 09's deferred finding I5).
#[test]
fn the_partition_on_observed_temperatures_converges_on_the_storm_tracks_every_month() {
    let inputs = reference::earth_like();
    let lapse = lapse::lapse_rate_k_per_m(&inputs);
    eprintln!(
        "the reference Earth's Γ {:.6} K/km, the data's {:.3}",
        lapse * 1e3,
        REDUCTION_LAPSE_RATE_K_PER_M * 1e3
    );
    assert!(
        (lapse - REDUCTION_LAPSE_RATE_K_PER_M).abs() <= REDUCTION_LAPSE_RATE_TOLERANCE_K_PER_M,
        "the reference Earth's lapse rate {lapse} K/m is no longer the data's"
    );
    let model = EnergyBalance::for_inputs(&inputs);
    let latent_heat = model
        .moisture
        .expect("the reference Earth is wet")
        .latent_heat_j_per_kg;
    let circulation = Circulation::for_inputs(&inputs, Turning::Rotating);
    let grid = ModelGrid::zonal();
    let budgets: Vec<Budget> = SEA_LEVEL_AIR_K
        .iter()
        .map(|t_k| partition::budget(&grid, &model, &circulation, &Bands::of(&model, &grid, t_k)))
        .collect();
    let year: f64 = MONTH_DAYS.iter().sum();
    let annual: Vec<f64> = (0..LATITUDE_BANDS)
        .map(|k| {
            budgets
                .iter()
                .zip(&MONTH_DAYS)
                .fold(0.0, |sum, (b, &days)| {
                    sum + days / year * b.evaporation_less_precipitation_kg_m2_s[k]
                })
        })
        .collect();
    let observed = Observed {
        grid,
        budgets,
        annual,
    };
    observed.report(&circulation, latent_heat, inputs.radius().value());
    let convergence: Vec<f64> = observed
        .budgets
        .iter()
        .map(|b| {
            -window(
                &observed.grid,
                &b.evaporation_less_precipitation_kg_m2_s,
                40.0,
                60.0,
            )
        })
        .collect();
    let summer = observed.season(JJA, 40.0, 60.0);
    let storm_tracks = window(&observed.grid, &observed.annual, 40.0, 60.0) * DAY_S;
    for (m, c) in convergence.iter().enumerate() {
        assert!(
            *c > 0.0,
            "month {}: the 40–60° N convergence {c} kg m⁻² s⁻¹",
            m + 1
        );
    }
    assert!(
        summer >= LEAST_SUMMER_CONVERGENCE_MM_PER_DAY,
        "the 40–60° N JJA convergence {summer} mm/day"
    );
    assert!(
        storm_tracks <= GREATEST_ANNUAL_STORM_TRACK_BUDGET_MM_PER_DAY,
        "the 40–60° N annual E − P {storm_tracks} mm/day"
    );
}
