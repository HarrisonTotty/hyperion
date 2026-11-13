//! The climate models' sun (R09.T13.a–b, Design note 8): the step clock over the seasonal orbit,
//! the month each step falls in, and every grid point's insolation at every step: the diurnal
//! mean of its band on a world that turns under its sun, and the beam itself on a world locked
//! 1:1 to it.
//!
//! The steps are uniform in time over one seasonal orbit from periapsis, at most
//! [`NOMINAL_STEP`] long. Each is evaluated at its midpoint: its mean anomaly M gives the
//! eccentric anomaly E by Kepler's equation ([`solve_kepler`]'s fixed starter and iterations), the
//! distance and the true anomaly ν from E (Vladilo et al. 2013, ApJ 767, 65, eq. A27, read), the
//! flux at the body `host_flux` √(1 − e²) (a ÷ r)², `host_flux` being the orbit's time mean, and
//! the sun's declination sin δ = sin ε sin(ν − `ν_eq`) from the obliquity ε and the equinox's true
//! anomaly `ν_eq` ([`Spin`]; Vladilo et al. 2013, eqs. A25–A26, read). Each step's month is the
//! one whose mean-anomaly edges ([`month_edges`]: twelve equal spans of E from periapsis) hold its
//! midpoint, so each month's record is the mean of its span's steps.
//!
//! A world that is not locked sees the sun's diurnal mean at every latitude (the sun's body-fixed
//! longitude averaged), so a month's body-fixed record is a climatology (Design note 8; Risks,
//! "Slow and resonant rotators' climate", settled so by R09.T13.b). A world with one month (no
//! seasonal orbit, or a circular one with no obliquity to it) has constant insolation, at equinox.
//!
//! A world locked 1:1 to its seasonal host takes no diurnal mean: each step's insolation at a
//! point of unit normal n̂ is F max(0, n̂ · ŝ), ŝ the direction to the host in the body frame
//! ([`HostTrack`]). There the prime meridian turns uniformly at the mean motion, W = `W_p` + M
//! (P14.T14.b's locked rotation law, `derive::rotation`), so the host moves in the body frame by
//! the true anomaly less the mean anomaly in longitude, the optical libration, and by the
//! declination in latitude; at periapsis it stands over the prime meridian, +x.

use core::f64::consts::{PI, TAU};

use hyperion_surface::field::month_edges;

use super::coordinates::Coordinates;
use super::grid::{LATITUDE_BANDS, ModelGrid};
use crate::math;
use crate::orbit::{Eccentricity, solve_kepler};
use crate::planetary::surface::inputs::{CoarseInputs, Spin};
use crate::units::{Radians, Seconds};

/// The step of the seasonal integration: six hours, 21,600 s, the brainstorm's step (Design note
/// 8), and the longest a step is but on the longest orbits ([`MAX_STEPS_PER_ORBIT`]).
pub(super) const NOMINAL_STEP: Seconds = Seconds::new(21_600.0);

/// The fewest steps an orbit takes: 1,200, so that every month holds at least four steps at any
/// eccentricity below 1 (`Adopted`).
///
/// The shortest month, at periapsis, spans the mean anomalies 0 to π ÷ 6 − e ÷ 2 (Kepler's
/// equation at E = 30°), so at least (π ÷ 6 − ½) ÷ 2π = 0.003 76 of the orbit as e tends to 1,
/// 4.5 steps of 1,200; at e = 0.6 it is 0.0356 of the orbit, 42.7 steps. Orbits shorter than 300
/// days take steps shorter than six hours.
pub(super) const MIN_STEPS_PER_ORBIT: u32 = 1_200;

/// The most steps an orbit takes: 5,844, four Julian years of six-hour steps, so that a long
/// year's cost stays bounded (`Adopted`): an orbit longer than four years takes longer steps,
/// 41.7 days on a 1,000-year orbit, which the implicit scheme holds (Design note 8) and the
/// seasons, P ÷ 5,844 apart, still resolve.
pub(super) const MAX_STEPS_PER_ORBIT: u32 = 5_844;

/// The steps over an orbit of period `period` at a nominal step of `step`: ⌈P ÷ step⌉, held
/// between [`MIN_STEPS_PER_ORBIT`] and [`MAX_STEPS_PER_ORBIT`].
#[must_use]
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the count is clamped to [1,200, 5,844] in f64 first, so the cast holds it exactly"
)]
pub(super) fn steps_per_orbit(period: Seconds, step: Seconds) -> u32 {
    let wanted = (period / step).ceil();
    let (low, high) = (
        f64::from(MIN_STEPS_PER_ORBIT),
        f64::from(MAX_STEPS_PER_ORBIT),
    );
    // By comparison, so that a NaN takes the fewest.
    let held = if wanted > low { wanted.min(high) } else { low };
    held as u32
}

/// The diurnal-mean insolation, W m⁻², of a latitude of sine `sine_latitude` and cosine
/// `cosine_latitude` (positive), under a flux `flux_w_m2` at the body when the sun's declination
/// has sine `sine_declination`:
/// (F ÷ π) (H₀ sin φ sin δ + cos φ cos δ sin H₀), cos H₀ = −tan φ tan δ, with H₀ = π in a polar
/// day and 0 in a polar night (Vladilo et al. 2013, ApJ 767, 65, eqs. A19 and A22, read; Berger
/// 1978, J. Atmos. Sci. 35, 2362, not read; Hartmann 2016, Global Physical Climatology, 2nd ed.,
/// eq. 2.15, from memory). A test holds its annual mean to plan 14's closed form
/// ([`annual_insolation_at`]).
///
/// [`annual_insolation_at`]: crate::planetary::hooks::surface::annual_insolation_at
#[must_use]
pub(super) fn diurnal_insolation(
    flux_w_m2: f64,
    sine_latitude: f64,
    cosine_latitude: f64,
    sine_declination: f64,
) -> f64 {
    let cosine_declination = (1.0 - sine_declination * sine_declination).max(0.0).sqrt();
    // cos H₀ = −n ÷ d, written so that no tangent of a pole or a declination of 90° is taken.
    let n = sine_latitude * sine_declination;
    let d = cosine_latitude * cosine_declination;
    let (hour_angle, sine_hour_angle) = if n >= d {
        (PI, 0.0)
    } else if n <= -d {
        (0.0, 0.0)
    } else {
        let c = -n / d;
        (math::acos(c), (1.0 - c * c).sqrt())
    };
    flux_w_m2 / PI * (hour_angle * n + d * sine_hour_angle)
}

/// The path of the host across the body frame of a world locked 1:1 to it, on an orbit whose
/// northern spring equinox falls at the true anomaly `ν_eq` ([`Spin::equinox_true_anomaly`]), at
/// obliquity ε ([`Spin::obliquity`]).
///
/// In the equator's frame whose first axis Ê points to the host at the spring equinox (where the
/// host crosses the equator northward) and whose third is the pole p̂, the host at the ecliptic
/// longitude L = ν − `ν_eq` lies along (cos L, cos ε sin L, sin ε sin L): its declination's sine
/// is sin ε sin L, as the diurnal mean's, and its right ascension α(L) = atan2(cos ε sin L, cos L)
/// (the standard rotation by the obliquity; a test checks it against P14.T14's frame). The prime
/// meridian stands at the right ascension α₀ = α(−`ν_eq`) of the host at periapsis plus the mean
/// anomaly M (the locked law), so the host's body-frame longitude, east of +x towards +y, is
/// α(ν − `ν_eq`) − α₀ − M: on an untilted orbit, ν − M, the equation of the centre, 2e sin M to
/// first order in e.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct HostTrack {
    sine_obliquity: f64,
    cosine_obliquity: f64,
    equinox_rad: f64,
    pericentre_right_ascension_rad: f64,
}

impl HostTrack {
    /// The track of a body of spin `spin`, locked 1:1 to its seasonal host.
    #[must_use]
    pub(super) fn of(spin: &Spin) -> Self {
        let (sine_obliquity, cosine_obliquity) = math::sin_cos(spin.obliquity.value());
        let equinox_rad = spin.equinox_true_anomaly.value();
        let (s, c) = math::sin_cos(-equinox_rad);
        Self {
            sine_obliquity,
            cosine_obliquity,
            equinox_rad,
            pericentre_right_ascension_rad: math::atan2(cosine_obliquity * s, c),
        }
    }

    /// The unit vector towards the host in the body frame at the true anomaly `true_anomaly_rad`
    /// and the mean anomaly `mean_anomaly_rad`.
    #[must_use]
    pub(super) fn direction(&self, true_anomaly_rad: f64, mean_anomaly_rad: f64) -> [f64; 3] {
        let (sl, cl) = math::sin_cos(true_anomaly_rad - self.equinox_rad);
        let (along_equinox, along_quarter, along_pole) =
            (cl, self.cosine_obliquity * sl, self.sine_obliquity * sl);
        let (s, c) = math::sin_cos(self.pericentre_right_ascension_rad + mean_anomaly_rad);
        [
            c * along_equinox + s * along_quarter,
            -s * along_equinox + c * along_quarter,
            along_pole,
        ]
    }
}

/// How the sunlight varies over its grid and its steps.
#[derive(Debug, Clone, PartialEq)]
enum Light {
    /// Constant: every step's is the annual mean.
    Constant,
    /// Each band's diurnal mean at each step, the same in every column.
    Bands {
        /// The insolation, W m⁻², at `step * LATITUDE_BANDS + band`.
        insolation_w_m2: Vec<f64>,
    },
    /// A beam: each step's flux at the body, W m⁻², and the unit vector towards its source in the
    /// model frame.
    Beam {
        flux_w_m2: Vec<f64>,
        direction: Vec<[f64; 3]>,
    },
}

/// The sunlight a climate model integrates over (the [module documentation](self)): constant, or
/// one seasonal orbit of uniform steps with each step's month and every point's insolation.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Sunlight {
    steps: u32,
    step_s: f64,
    months: u8,
    month_of_step: Vec<u8>,
    light: Light,
    annual_w_m2: Vec<f64>,
}

/// A place on a seasonal orbit: its anomalies and its distance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct OrbitPlace {
    /// The mean anomaly M, radians from periapsis.
    pub(super) mean_anomaly_rad: f64,
    /// The true anomaly ν, radians from periapsis.
    pub(super) true_anomaly_rad: f64,
    /// The distance over the semi-major axis, r ÷ a, dimensionless.
    pub(super) distance_over_a: f64,
}

/// One step of a seasonal orbit of eccentricity `eccentricity` in `steps` steps from periapsis,
/// at the midpoint of step `step` ([`orbit_at`]).
#[must_use]
fn orbit_step(step: u32, steps: u32, eccentricity: Eccentricity) -> OrbitPlace {
    orbit_at(
        TAU * (f64::from(step) + 0.5) / f64::from(steps),
        eccentricity,
    )
}

/// The place on a seasonal orbit of eccentricity `eccentricity` at the mean anomaly
/// `mean_anomaly_rad`, by Kepler's equation ([`solve_kepler`]): the true anomaly from the eccentric
/// anomaly E, tan(ν ÷ 2) = √((1 + e) ÷ (1 − e)) tan(E ÷ 2), and r ÷ a = 1 − e cos E (Vladilo et al.
/// 2013, ApJ 767, 65, eq. A27, read).
#[must_use]
pub(super) fn orbit_at(mean_anomaly_rad: f64, eccentricity: Eccentricity) -> OrbitPlace {
    let e = eccentricity.value();
    let big_e = solve_kepler(Radians::new(mean_anomaly_rad), eccentricity).value();
    let (sine_half, cosine_half) = math::sin_cos(0.5 * big_e);
    OrbitPlace {
        mean_anomaly_rad,
        true_anomaly_rad: 2.0
            * math::atan2((1.0 + e).sqrt() * sine_half, (1.0 - e).sqrt() * cosine_half),
        distance_over_a: 1.0 - e * math::cos(big_e),
    }
}

/// The sun's height over one day at a latitude φ under a declination δ: cos z = a + b cos h, with
/// a = sin φ sin δ, b = cos φ cos δ and h the hour angle from local noon, and the half-day H₀ over
/// which it is up, cos H₀ = −a ÷ b, π in a polar day and 0 in a polar night (Vladilo et al. 2013,
/// ApJ 767, 65, eqs. A19 and A22, read; Berger 1978, J. Atmos. Sci. 35, 2362, not read; as
/// [`diurnal_insolation`] takes it).
///
/// The radiative model reads the sun's mean height over each step's span of hour angles, the day
/// resolved ([`mean_over`](Self::mean_over)), where the energy-balance model reads its whole day's
/// mean ([`daily_mean`](Self::daily_mean), which is [`diurnal_insolation`]'s over the flux).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Daylight {
    a: f64,
    b: f64,
    half_day_rad: f64,
    sine_half_day: f64,
}

impl Daylight {
    /// The day at a latitude of sine `sine_latitude` and cosine `cosine_latitude` (not negative)
    /// under a declination of sine `sine_declination` and cosine `cosine_declination` (not
    /// negative).
    #[must_use]
    pub(super) fn new(
        sine_latitude: f64,
        cosine_latitude: f64,
        sine_declination: f64,
        cosine_declination: f64,
    ) -> Self {
        let (a, b) = (
            sine_latitude * sine_declination,
            cosine_latitude * cosine_declination,
        );
        // Written so that no tangent of a pole or of a declination of 90° is taken: where b is
        // zero the sun's height is a all day, a polar day if a is positive.
        let (half_day_rad, sine_half_day) = if a >= b {
            (PI, 0.0)
        } else if a <= -b {
            (0.0, 0.0)
        } else {
            let c = -a / b;
            (math::acos(c), (1.0 - c * c).sqrt())
        };
        Self {
            a,
            b,
            half_day_rad,
            sine_half_day,
        }
    }

    /// The day's mean of max(0, cos z): (H₀ a + b sin H₀) ÷ π.
    #[must_use]
    pub(super) fn daily_mean(&self) -> f64 {
        (self.half_day_rad * self.a + self.b * self.sine_half_day) / PI
    }

    /// The mean of max(0, cos z) over the hour angles from `start_rad`, in [−π, π), to `end_rad`,
    /// above it and at most 2π on, whose sines are `sine_start` and `sine_end`: the integral of
    /// a + b cos h over each span of daylight inside them, [−H₀, H₀] and [2π − H₀, 2π + H₀], over
    /// their length.
    #[must_use]
    pub(super) fn mean_over(
        &self,
        start_rad: f64,
        sine_start: f64,
        end_rad: f64,
        sine_end: f64,
    ) -> f64 {
        let mut total = 0.0;
        for noon in [0.0, TAU] {
            let (rise, sine_rise) = (noon - self.half_day_rad, -self.sine_half_day);
            let (set, sine_set) = (noon + self.half_day_rad, self.sine_half_day);
            let (from, sine_from) = if rise > start_rad {
                (rise, sine_rise)
            } else {
                (start_rad, sine_start)
            };
            let (to, sine_to) = if set < end_rad {
                (set, sine_set)
            } else {
                (end_rad, sine_end)
            };
            if to > from {
                total += self.a * (to - from) + self.b * (sine_to - sine_from);
            }
        }
        total / (end_rad - start_rad)
    }
}

/// `angle_rad` reduced to [−π, π), by whole turns.
#[must_use]
pub(super) fn reduced(angle_rad: f64) -> f64 {
    angle_rad - TAU * ((angle_rad + PI) / TAU).floor()
}

/// The month of each of `steps` steps of an orbit of eccentricity `eccentricity`, by the
/// midpoint's mean anomaly against [`month_edges`].
#[must_use]
fn months_of_steps(steps: u32, eccentricity: Eccentricity) -> Vec<u8> {
    let edges = month_edges(eccentricity.value()).expect("an `Eccentricity` is in [0, 1)");
    let mut month = 0_u8;
    (0..steps)
        .map(|step| {
            let mean_anomaly = TAU * (f64::from(step) + 0.5) / f64::from(steps);
            while month < 11 && mean_anomaly >= edges[usize::from(month) + 1].value() {
                month += 1;
            }
            month
        })
        .collect()
}

/// The insolation, W m⁻², of every point of `grid` under a beam of flux `flux_w_m2` from the
/// model-frame direction `source`, F max(0, n̂ · ŝ), written to `out`.
fn beam_insolation(grid: &ModelGrid, flux_w_m2: f64, source: [f64; 3], out: &mut [f64]) {
    for (band, row) in out.chunks_exact_mut(grid.columns()).enumerate() {
        for (column, value) in row.iter_mut().enumerate() {
            let n = grid.centre_direction_of(band, column);
            let cosine = n[0] * source[0] + n[1] * source[1] + n[2] * source[2];
            *value = if cosine > 0.0 {
                flux_w_m2 * cosine
            } else {
                0.0
            };
        }
    }
}

impl Sunlight {
    /// The diurnal-mean sunlight of the body of `inputs` over the bands of `grid`, the same in
    /// every column, at a nominal step of `step` ([`NOMINAL_STEP`] in the pass): seasonal in a
    /// twelve-month year ([`CoarseInputs::months`]) and constant otherwise.
    ///
    /// # Panics
    ///
    /// If the seasonal orbit's eccentricity is not at least 0 and below 1, which the inputs'
    /// builder refuses.
    #[must_use]
    pub(super) fn of(inputs: &CoarseInputs, grid: &ModelGrid, step: Seconds) -> Self {
        let flux = inputs.host_flux().value();
        match inputs.seasonal_orbit() {
            Some(orbit) if inputs.months() == 12 => Self::seasonal(
                grid,
                flux,
                eccentricity_of(orbit.eccentricity),
                orbit.period,
                inputs.spin(),
                step,
            ),
            _ => Self::constant(
                grid.broadcast(
                    &(0..LATITUDE_BANDS)
                        .map(|k| {
                            diurnal_insolation(
                                flux,
                                grid.centre_sine(k),
                                grid.centre_cosine(k),
                                0.0,
                            )
                        })
                        .collect::<Vec<f64>>(),
                ),
            ),
        }
    }

    /// The sunlight of the body of `inputs`, locked 1:1 to its seasonal host, over the points of
    /// `grid` in `coordinates`, at a nominal step of `step`: the beam from the host
    /// ([`HostTrack`]) over a seasonal orbit in a twelve-month year, and in a one-month year the
    /// beam constant from +x, the substellar point.
    ///
    /// # Panics
    ///
    /// If the seasonal orbit's eccentricity is not at least 0 and below 1, which the inputs'
    /// builder refuses.
    #[must_use]
    pub(super) fn locked(
        inputs: &CoarseInputs,
        grid: &ModelGrid,
        coordinates: Coordinates,
        step: Seconds,
    ) -> Self {
        let flux = inputs.host_flux().value();
        match inputs.seasonal_orbit() {
            Some(orbit) if inputs.months() == 12 => Self::beam(
                grid,
                coordinates,
                flux,
                eccentricity_of(orbit.eccentricity),
                orbit.period,
                inputs.spin(),
                step,
            ),
            _ => {
                let mut insolation = vec![0.0; grid.points()];
                beam_insolation(
                    grid,
                    flux,
                    coordinates.to_model([1.0, 0.0, 0.0]),
                    &mut insolation,
                );
                Self::constant(insolation)
            }
        }
    }

    /// Constant sunlight of insolation `insolation_w_m2`, one value per grid point: a one-month
    /// year.
    #[must_use]
    pub(super) fn constant(insolation_w_m2: Vec<f64>) -> Self {
        Self {
            steps: 0,
            step_s: 0.0,
            months: 1,
            month_of_step: Vec::new(),
            light: Light::Constant,
            annual_w_m2: insolation_w_m2,
        }
    }

    /// One seasonal orbit of eccentricity `eccentricity` and period `period`, under a mean flux of
    /// `mean_flux_w_m2`, for a body of spin `spin` that turns under its sun, over the bands of
    /// `grid`, in steps of at most `nominal_step` ([`steps_per_orbit`]).
    #[must_use]
    pub(super) fn seasonal(
        grid: &ModelGrid,
        mean_flux_w_m2: f64,
        eccentricity: Eccentricity,
        period: Seconds,
        spin: &Spin,
        nominal_step: Seconds,
    ) -> Self {
        let steps = steps_per_orbit(period, nominal_step);
        let e = eccentricity.value();
        let sine_obliquity = math::sin(spin.obliquity.value());
        let equinox = spin.equinox_true_anomaly.value();
        let distance_scale = (1.0 - e * e).sqrt();
        let count = usize::try_from(steps).expect("an orbit's steps fit a usize");
        let mut insolation_w_m2 = Vec::with_capacity(count * LATITUDE_BANDS);
        let mut annual_w_m2 = vec![0.0; LATITUDE_BANDS];
        for step in 0..steps {
            let place = orbit_step(step, steps, eccentricity);
            let distance = place.distance_over_a;
            let flux = mean_flux_w_m2 * distance_scale / (distance * distance);
            let sine_declination = sine_obliquity * math::sin(place.true_anomaly_rad - equinox);
            for (band, annual) in annual_w_m2.iter_mut().enumerate() {
                let q = diurnal_insolation(
                    flux,
                    grid.centre_sine(band),
                    grid.centre_cosine(band),
                    sine_declination,
                );
                insolation_w_m2.push(q);
                *annual += q;
            }
        }
        let n = f64::from(steps);
        for annual in &mut annual_w_m2 {
            *annual /= n;
        }
        Self {
            steps,
            step_s: period.value() / n,
            months: 12,
            month_of_step: months_of_steps(steps, eccentricity),
            light: Light::Bands { insolation_w_m2 },
            annual_w_m2: grid.broadcast(&annual_w_m2),
        }
    }

    /// One seasonal orbit of eccentricity `eccentricity` and period `period`, under a mean flux of
    /// `mean_flux_w_m2`, for a body of spin `spin` locked 1:1 to its host, over the points of
    /// `grid` in `coordinates`, in steps of at most `nominal_step`: the beam from the host at each
    /// step ([`HostTrack`]).
    #[must_use]
    pub(super) fn beam(
        grid: &ModelGrid,
        coordinates: Coordinates,
        mean_flux_w_m2: f64,
        eccentricity: Eccentricity,
        period: Seconds,
        spin: &Spin,
        nominal_step: Seconds,
    ) -> Self {
        let steps = steps_per_orbit(period, nominal_step);
        let e = eccentricity.value();
        let track = HostTrack::of(spin);
        let distance_scale = (1.0 - e * e).sqrt();
        let count = usize::try_from(steps).expect("an orbit's steps fit a usize");
        let (mut flux_w_m2, mut direction) = (Vec::with_capacity(count), Vec::with_capacity(count));
        let mut annual_w_m2 = vec![0.0; grid.points()];
        let mut insolation = vec![0.0; grid.points()];
        for step in 0..steps {
            let place = orbit_step(step, steps, eccentricity);
            let distance = place.distance_over_a;
            let flux = mean_flux_w_m2 * distance_scale / (distance * distance);
            let source = coordinates
                .to_model(track.direction(place.true_anomaly_rad, place.mean_anomaly_rad));
            beam_insolation(grid, flux, source, &mut insolation);
            for (annual, q) in annual_w_m2.iter_mut().zip(&insolation) {
                *annual += q;
            }
            flux_w_m2.push(flux);
            direction.push(source);
        }
        let n = f64::from(steps);
        for annual in &mut annual_w_m2 {
            *annual /= n;
        }
        Self {
            steps,
            step_s: period.value() / n,
            months: 12,
            month_of_step: months_of_steps(steps, eccentricity),
            light: Light::Beam {
                flux_w_m2,
                direction,
            },
            annual_w_m2,
        }
    }

    /// This sunlight over `grid` as the zonal model reads it: each step's band means over `grid`'s
    /// columns, summed column by column (decision-r09-t13i's §2.4 follow-up 4 §4, the zonal search).
    /// A turning world's diurnal mean is the same in every column, so its bands are its own; a
    /// beam's are averaged step by step.
    #[must_use]
    pub(super) fn band_means(&self, grid: &ModelGrid) -> Self {
        let light = match &self.light {
            Light::Constant => Light::Constant,
            Light::Bands { insolation_w_m2 } => Light::Bands {
                insolation_w_m2: insolation_w_m2.clone(),
            },
            Light::Beam { .. } => {
                let mut insolation = vec![0.0; grid.points()];
                let mut bands = Vec::with_capacity(slot(self.steps) * LATITUDE_BANDS);
                for step in 0..self.steps {
                    self.fill(step, grid, &mut insolation);
                    bands.extend(grid.band_means(&insolation));
                }
                Light::Bands {
                    insolation_w_m2: bands,
                }
            }
        };
        Self {
            steps: self.steps,
            step_s: self.step_s,
            months: self.months,
            month_of_step: self.month_of_step.clone(),
            light,
            annual_w_m2: grid.band_means(&self.annual_w_m2),
        }
    }

    /// Whether a step's insolation varies along a band, which only a beam's does: the diurnal
    /// mean is the same in every column, and constant sunlight runs no orbit.
    #[must_use]
    pub(super) fn varies_in_longitude(&self) -> bool {
        match self.light {
            Light::Beam { .. } => true,
            Light::Constant | Light::Bands { .. } => false,
        }
    }

    /// The steps of its orbit: zero for constant sunlight.
    #[must_use]
    pub(super) fn steps(&self) -> u32 {
        self.steps
    }

    /// The length of each step, s: the orbit's period over its steps; zero for constant sunlight.
    #[must_use]
    pub(super) fn step_s(&self) -> f64 {
        self.step_s
    }

    /// The months of its year, 1 or 12.
    #[must_use]
    pub(super) fn months(&self) -> u8 {
        self.months
    }

    /// The month, from 0 at periapsis, of step `step`.
    ///
    /// # Panics
    ///
    /// If `step` is not one of the orbit's steps (it is at least [`steps`](Self::steps)), as on
    /// constant sunlight, which has none.
    #[must_use]
    pub(super) fn month_of(&self, step: u32) -> u8 {
        self.month_of_step[slot(step)]
    }

    /// Writes every point's insolation at step `step` on `grid`, W m⁻², to `out`, one value per
    /// point; the constant insolation for constant sunlight.
    ///
    /// # Panics
    ///
    /// On seasonal sunlight, if `step` is not one of the orbit's steps (it is at least
    /// [`steps`](Self::steps)); on constant sunlight, if `out` is not one value per point.
    pub(super) fn fill(&self, step: u32, grid: &ModelGrid, out: &mut [f64]) {
        match &self.light {
            Light::Constant => out.copy_from_slice(&self.annual_w_m2),
            Light::Bands { insolation_w_m2 } => {
                let start = slot(step) * LATITUDE_BANDS;
                let bands = &insolation_w_m2[start..start + LATITUDE_BANDS];
                for (row, &q) in out.chunks_exact_mut(grid.columns()).zip(bands) {
                    row.fill(q);
                }
            }
            Light::Beam {
                flux_w_m2,
                direction,
            } => beam_insolation(grid, flux_w_m2[slot(step)], direction[slot(step)], out),
        }
    }

    /// Every point's annual-mean insolation, W m⁻²: the mean of its steps', or the constant.
    #[must_use]
    pub(super) fn annual(&self) -> &[f64] {
        &self.annual_w_m2
    }

    /// Every point's mean insolation in each month on `grid`, W m⁻²: the mean over the month's
    /// steps of their insolation, the steps whose temperatures the month's record averages
    /// ([`EnergyBalance::orbits`](super::ebm::EnergyBalance::orbits)), summed in step order; one
    /// month, the constant, for constant sunlight. The precipitation heuristic's absorbed sunlight
    /// reads it (R09.T13.e, decision-r09-t13ge §2.11).
    ///
    /// # Panics
    ///
    /// If a month of a seasonal orbit holds no step, which [`MIN_STEPS_PER_ORBIT`] rules out.
    #[must_use]
    pub(super) fn month_means(&self, grid: &ModelGrid) -> Vec<Vec<f64>> {
        if self.steps == 0 {
            return vec![self.annual_w_m2.clone()];
        }
        let points = grid.points();
        let mut months = vec![vec![0.0; points]; usize::from(self.months)];
        let mut counts = vec![0_u32; months.len()];
        let mut insolation = vec![0.0; points];
        for step in 0..self.steps {
            self.fill(step, grid, &mut insolation);
            let month = usize::from(self.month_of(step));
            counts[month] += 1;
            for (sum, &q) in months[month].iter_mut().zip(&insolation) {
                *sum += q;
            }
        }
        for (month, &count) in months.iter_mut().zip(&counts) {
            assert!(count > 0, "every month holds a step");
            let n = f64::from(count);
            for sum in month.iter_mut() {
                *sum /= n;
            }
        }
        months
    }

    /// The global mean of the annual insolation on `grid`, W m⁻².
    #[cfg(test)]
    #[must_use]
    pub(super) fn global_mean(&self, grid: &ModelGrid) -> f64 {
        grid.mean(&self.annual_w_m2)
    }
}

/// The index of step `step` into the sunlight's vectors.
#[must_use]
fn slot(step: u32) -> usize {
    usize::try_from(step).expect("an orbit's steps fit a usize")
}

/// A seasonal orbit's eccentricity `e`, validated once for the sun's functions.
///
/// # Panics
///
/// If `e` is not at least 0 and below 1, which the inputs' builder refuses.
#[must_use]
fn eccentricity_of(e: f64) -> Eccentricity {
    Eccentricity::new(e)
        .expect("the inputs' builder holds a seasonal orbit's eccentricity in [0, 1)")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planetary::hooks::surface::annual_insolation_at;

    fn spin(obliquity_deg: f64, equinox_deg: f64) -> Spin {
        Spin {
            obliquity: Radians::new(obliquity_deg.to_radians()),
            rotation_period: Seconds::new(86_164.0),
            solar_day: Some(Seconds::new(86_400.0)),
            equinox_true_anomaly: Radians::new(equinox_deg.to_radians()),
        }
    }

    /// The months are twelve equal spans of eccentric anomaly from periapsis: every step's E lies
    /// in its month's 30°, a circular orbit's months are twelfths of its steps, and at e = 0.6 the
    /// periapsis month holds (π ÷ 6 − 0.3) ÷ 2π of them, 0.427 twelfths, the plan's "0.4".
    #[test]
    fn the_months_are_equal_spans_of_eccentric_anomaly_from_periapsis() {
        let grid = ModelGrid::zonal();
        let year = 365.256 * 86_400.0;
        for e in [0.0, 0.0167, 0.6, 0.95] {
            let sun = Sunlight::seasonal(
                &grid,
                1_361.0,
                Eccentricity::new(e).unwrap(),
                Seconds::new(year),
                &spin(23.44, 77.0),
                NOMINAL_STEP,
            );
            let n = sun.steps();
            assert_eq!(n, 1_462);
            let mut counts = [0_u32; 12];
            for step in 0..n {
                let month = sun.month_of(step);
                counts[usize::from(month)] += 1;
                let m = TAU * (f64::from(step) + 0.5) / f64::from(n);
                // `solve_kepler` answers in [−π, π]; the months count E from periapsis to 2π.
                let half_turn =
                    solve_kepler(Radians::new(m), Eccentricity::new(e).unwrap()).value();
                let big_e = if half_turn < 0.0 {
                    half_turn + TAU
                } else {
                    half_turn
                };
                let span = (f64::from(month) * PI / 6.0, f64::from(month + 1) * PI / 6.0);
                assert!(
                    big_e >= span.0 - 1e-12 && big_e < span.1 + 1e-12,
                    "e {e}: step {step} at E {big_e} in month {month}"
                );
            }
            assert!(counts.iter().all(|&c| c >= 4), "e {e}: {counts:?}");
            for (month, &count) in counts.iter().enumerate() {
                let edges = month_edges(e).unwrap();
                let share = (edges[month + 1].value() - edges[month].value()) / TAU;
                let held = f64::from(count) / f64::from(n);
                assert!(
                    (held - share).abs() <= 1.0 / f64::from(n),
                    "e {e}, month {month}: {held} of the steps against {share}"
                );
                if e < 1e-12 {
                    assert!((held - 1.0 / 12.0).abs() <= 1.0 / f64::from(n));
                }
            }
            if (e - 0.6).abs() < 1e-12 {
                let twelfths = 12.0 * f64::from(counts[0]) / f64::from(n);
                let expected = 12.0 * (PI / 6.0 - 0.3) / TAU;
                assert!(
                    (twelfths - expected).abs() < 12.0 / f64::from(n),
                    "{twelfths}"
                );
                assert!((twelfths - 0.427).abs() < 0.01, "{twelfths}");
            }
        }
    }

    /// The annual mean of the diurnal-mean insolation at every band is plan 14's closed form,
    /// (S ÷ 4) times `annual_insolation_at`, on a circular orbit and an eccentric one, whose
    /// (a ÷ r)² dt is uniform in the sun's longitude (Kepler's second law), and at three
    /// obliquities; the global mean is S ÷ 4.
    #[test]
    fn the_annual_mean_insolation_is_the_closed_form() {
        let grid = ModelGrid::zonal();
        let year = 365.256 * 86_400.0;
        for (e, obliquity, equinox) in [
            (0.0, 23.44, 77.0),
            (0.3, 23.44, 40.0),
            (0.0167, 60.0, 77.0),
            (0.1, 90.0, 200.0),
        ] {
            let flux = 1_361.0;
            let sun = Sunlight::seasonal(
                &grid,
                flux,
                Eccentricity::new(e).unwrap(),
                Seconds::new(year),
                &spin(obliquity, equinox),
                Seconds::new(3_600.0),
            );
            for band in 0..LATITUDE_BANDS {
                let expected = flux / 4.0
                    * annual_insolation_at(
                        Radians::new(f64::to_radians(obliquity)),
                        grid.centre_sine(band),
                    );
                let got = sun.annual()[band];
                assert!(
                    (got - expected).abs() < 2e-3 * flux / 4.0,
                    "e {e}, obliquity {obliquity}, band {band}: {got} against {expected}"
                );
            }
            let mean = sun.global_mean(&grid);
            assert!((mean / (flux / 4.0) - 1.0).abs() < 2e-4, "{mean}");
        }
    }

    /// A polar day sees the sun all day and a polar night none; at equinox the equator receives
    /// F ÷ π and the pole nothing.
    #[test]
    fn the_diurnal_insolation_has_polar_days_and_nights() {
        let flux = 1_000.0;
        let (s, c) = math::sin_cos(80_f64.to_radians());
        let declination = math::sin(20_f64.to_radians());
        let day = diurnal_insolation(flux, s, c, declination);
        // All day: F sin φ sin δ.
        assert!((day - flux * s * declination).abs() < 1e-9, "{day}");
        assert!(diurnal_insolation(flux, -s, c, declination).abs() < 1e-12);
        assert!((diurnal_insolation(flux, 0.0, 1.0, 0.0) - flux / PI).abs() < 1e-12);
        assert!(diurnal_insolation(flux, 1.0, 0.0, 0.0).abs() < 1e-12);
        // A declination of 90°: the lit pole has the sun overhead all day.
        assert!((diurnal_insolation(flux, 1.0, 0.0, 1.0) - flux).abs() < 1e-9);
    }

    /// The resolved day: its whole day's mean is [`diurnal_insolation`]'s, the mean of its 48
    /// steps of hour angle is the whole day's, a step's mean is a direct quadrature's of
    /// max(0, cos z), and a span crossing midnight, a polar day and a polar night read as they
    /// should.
    #[test]
    fn the_resolved_day_averages_to_the_diurnal_mean() {
        let flux = 1_000.0;
        let quadrature = |day: &Daylight, from: f64, to: f64| {
            let n = 20_000;
            let width = (to - from) / f64::from(n);
            (0..n).fold(0.0, |sum, k| {
                let h = from + (f64::from(k) + 0.5) * width;
                sum + (day.a + day.b * math::cos(h)).max(0.0)
            }) / f64::from(n)
        };
        for (latitude, declination) in [
            (0.0, 0.0),
            (35.0, 20.0),
            (-60.0, 23.44),
            (80.0, 20.0),
            (-80.0, 20.0),
            (90.0, 0.0),
            (89.0, -1.0),
        ] {
            let (s, c) = math::sin_cos(f64::to_radians(latitude));
            let (sd, cd) = math::sin_cos(f64::to_radians(declination));
            let day = Daylight::new(s, c, sd, cd);
            let expected = diurnal_insolation(flux, s, c, sd);
            assert!(
                (flux * day.daily_mean() - expected).abs() < 1e-9,
                "{latitude}"
            );
            let steps = 48;
            let edge = |k: u32| -PI + TAU * f64::from(k) / f64::from(steps);
            let mean = (0..steps).fold(0.0, |sum, k| {
                let (from, to) = (edge(k), edge(k + 1));
                sum + day.mean_over(from, math::sin(from), to, math::sin(to))
            }) / f64::from(steps);
            assert!(
                (mean - day.daily_mean()).abs() < 1e-12,
                "{latitude}: {mean}"
            );
            for (from, to) in [
                (-0.3, 0.2),
                (1.2, 1.5),
                (3.0, 3.4),
                (-3.1, -2.0),
                (2.9, 6.0),
            ] {
                let direct = quadrature(&day, from, to);
                let ours = day.mean_over(from, math::sin(from), to, math::sin(to));
                assert!(
                    (ours - direct).abs() < 1e-6,
                    "{latitude}, {from}–{to}: {ours} against {direct}"
                );
            }
        }
        for angle in [-7.0, -PI, 0.0, 3.0, 10.0] {
            let r = reduced(angle);
            assert!((-PI..PI).contains(&r), "{angle}: {r}");
            assert!((math::sin(r) - math::sin(angle)).abs() < 1e-12);
        }
    }

    /// Six-hour steps but on short and long orbits, where the bounds hold.
    #[test]
    fn an_orbit_takes_six_hour_steps_within_its_bounds() {
        let days = |d: f64| Seconds::new(d * 86_400.0);
        assert_eq!(steps_per_orbit(days(365.256), NOMINAL_STEP), 1_462);
        assert_eq!(steps_per_orbit(days(686.98), NOMINAL_STEP), 2_748);
        assert_eq!(
            steps_per_orbit(days(10.0), NOMINAL_STEP),
            MIN_STEPS_PER_ORBIT
        );
        assert_eq!(
            steps_per_orbit(Seconds::new(3.16e10), NOMINAL_STEP),
            MAX_STEPS_PER_ORBIT
        );
        assert_eq!(
            steps_per_orbit(Seconds::new(f64::NAN), NOMINAL_STEP),
            MIN_STEPS_PER_ORBIT
        );
    }

    fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
        a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
    }

    fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    }

    /// The sign conventions, end to end against P14.T14's frame: a planet locked 1:1 to its star
    /// on an eccentric, tilted orbit, whose star's direction from it, turned into its body-fixed
    /// frame by `frames::body_fixed_at`, is [`HostTrack::direction`]'s at the orbit's own true and
    /// mean anomalies at every sampled time, with the equinox found from the frame: the true
    /// anomaly at which the star crosses the frame's equator northward. The eccentricities are
    /// at most P14.T14.b's 0.1, above which a lock is 3:2 (`SPIN_ORBIT_RESONANCE_ECCENTRICITY`).
    #[test]
    fn the_host_s_direction_is_p14_t14_s_body_frame_s() {
        use crate::orbit::{KeplerElements, Orientation};
        use crate::planetary::frames::{BodyFixedFrame, FrameSpin, body_fixed_at};
        use crate::time::{Span, UniverseTime};
        use crate::units::consts::{METRES_PER_AU, SECONDS_PER_JULIAN_YEAR};
        use crate::units::{GravitationalParameter, Metres, SolarMasses, Years};

        for (e, obliquity, azimuth) in [(0.08, 0.4, 1.3), (0.05, 0.0, 0.0), (0.1, 2.0, -0.7)] {
            let orientation =
                Orientation::new(Radians::new(0.7), Radians::new(2.1), Radians::new(4.0)).unwrap();
            let orbit = KeplerElements::from_semi_major_axis(
                Metres::new(0.05 * METRES_PER_AU),
                GravitationalParameter::from_solar_masses(SolarMasses::new(0.3)),
                Eccentricity::new(e).unwrap(),
                orientation,
                Radians::new(0.9),
            )
            .unwrap();
            let frame = BodyFixedFrame::new(
                &orbit,
                &FrameSpin {
                    obliquity: Radians::new(obliquity),
                    pole_azimuth: Radians::new(azimuth),
                    primordial_period: Seconds::new(54_000.0),
                    locking_time: Seconds::new(1e6 * SECONDS_PER_JULIAN_YEAR),
                    phase_at_epoch: Radians::new(0.25),
                },
                Years::new(4e9),
            )
            .unwrap();
            let p = orbit.orientation().periapsis_direction();
            let q = cross(orbit.orientation().normal(), p);
            let true_anomaly_of = |r: [f64; 3]| math::atan2(dot(r, q), dot(r, p));
            // The star's direction at true anomaly ν is −(cos ν P + sin ν Q); it crosses the
            // equator where it lies along the frame's node, northward at one of the two.
            let star = |nu: f64| {
                let (s, c) = math::sin_cos(nu);
                [0, 1, 2].map(|i| -(c * p[i] + s * q[i]))
            };
            let node = frame.equator_node();
            let crossing = true_anomaly_of(node);
            let equinox = [crossing, crossing + PI]
                .into_iter()
                .find(|&nu| dot(star(nu + 1e-4), frame.pole()) > 0.0)
                .unwrap();
            let spin = Spin {
                obliquity: Radians::new(obliquity),
                rotation_period: orbit.period(),
                solar_day: None,
                equinox_true_anomaly: Radians::new(equinox.rem_euclid(TAU)),
            };
            let track = HostTrack::of(&spin);
            let period = orbit.period().value();
            for k in 0..37 {
                let seconds = 1e5 + period * f64::from(k) / 12.3;
                let t = UniverseTime::EPOCH
                    .checked_add(Span::from_seconds_f64(seconds).unwrap())
                    .unwrap();
                let r = orbit.relative_state_at(t).0.metres();
                let length = dot(r, r).sqrt();
                let towards = r.map(|c| -c / length);
                let fixed = body_fixed_at(&frame, t)
                    .matrix()
                    .map(|row| dot(row, towards));
                let ours = track.direction(true_anomaly_of(r), orbit.mean_anomaly_at(t).value());
                for (a, b) in ours.iter().zip(fixed) {
                    assert!(
                        (a - b).abs() < 1e-9,
                        "e {e}, ε {obliquity}, time {k}: {ours:?} against {fixed:?}"
                    );
                }
            }
        }
    }

    /// On an untilted eccentric orbit the substellar point stays on the equator and librates in
    /// longitude by the equation of the centre, ν − M: over +x at periapsis and apoapsis, east
    /// (towards +y) as the planet leaves periapsis, by 2e to first order; with obliquity it moves
    /// in latitude by the declination, and at periapsis it stands over the prime meridian.
    #[test]
    fn the_substellar_point_librates_by_the_equation_of_the_centre() {
        for e in [0.05, 0.1] {
            let track = HostTrack::of(&spin(0.0, 0.0));
            let eccentricity = Eccentricity::new(e).unwrap();
            let steps = 1_200;
            let mut largest: f64 = 0.0;
            for step in 0..steps {
                let place = orbit_step(step, steps, eccentricity);
                let (m, nu) = (place.mean_anomaly_rad, place.true_anomaly_rad);
                let s = track.direction(nu, m);
                assert!(s[2].abs() < 1e-15);
                let longitude = math::atan2(s[1], s[0]);
                let centre = math::atan2(math::sin(nu - m), math::cos(nu - m));
                assert!((longitude - centre).abs() < 1e-12, "e {e}, step {step}");
                if m < PI {
                    assert!(longitude > 0.0, "east after periapsis: {longitude}");
                }
                largest = largest.max(longitude);
            }
            assert!(
                (largest - 2.0 * e).abs() < 0.02 * 2.0 * e + 2.0 * e * e * e,
                "e {e}: {largest}"
            );
            for m in [0.0, PI] {
                let s = track.direction(m, m);
                assert!((s[0] - 1.0).abs() < 1e-12 && s[1].abs() < 1e-12, "{s:?}");
            }
        }
        let tilted = HostTrack::of(&spin(23.44, 77.0));
        let at_periapsis = tilted.direction(0.0, 0.0);
        assert!(at_periapsis[1].abs() < 1e-15 && at_periapsis[0] > 0.9);
        let solstice = tilted.direction(77_f64.to_radians() + PI / 2.0, 1.0);
        assert!((solstice[2] - math::sin(23.44_f64.to_radians())).abs() < 1e-12);
    }

    /// A locked world's beam delivers a quarter of the mean flux over the year on either grid,
    /// on an eccentric and tilted orbit; in a one-month year it stands over +x, the tidally locked
    /// grid's pole, and lights each band as its centre's sine.
    #[test]
    fn a_locked_world_s_beam_delivers_a_quarter_of_its_flux() {
        let flux = 1_361.0;
        for (coordinates, grid) in [
            (Coordinates::TidallyLocked, ModelGrid::locked()),
            (Coordinates::Geographic, ModelGrid::geographic()),
        ] {
            let sun = Sunlight::beam(
                &grid,
                coordinates,
                flux,
                Eccentricity::new(0.1).unwrap(),
                Seconds::new(20.0 * 86_400.0),
                &spin(30.0, 40.0),
                NOMINAL_STEP,
            );
            assert_eq!(sun.steps(), MIN_STEPS_PER_ORBIT);
            let mean = sun.global_mean(&grid);
            assert!(
                (mean / (flux / 4.0) - 1.0).abs() < 1e-3,
                "{coordinates:?}: {mean}"
            );
            let mut out = vec![0.0; grid.points()];
            sun.fill(17, &grid, &mut out);
            assert!(out.iter().all(|q| *q >= 0.0 && q.is_finite()));
        }
        let grid = ModelGrid::locked();
        let constant = Sunlight::constant({
            let mut q = vec![0.0; grid.points()];
            beam_insolation(&grid, flux, [0.0, 0.0, 1.0], &mut q);
            q
        });
        for band in 0..LATITUDE_BANDS {
            let expected = flux * grid.centre_sine(band).max(0.0);
            for column in 0..grid.columns() {
                let got = constant.annual()[grid.point(band, column)];
                assert!((got - expected).abs() < 1e-9 * flux);
            }
        }
    }
}
