//! The coarse field's header: what a body's field holds once, not per cell (plan R09, Design notes
//! 4, 7, 8, 17 and 18).

use hyperion_base::units::{Gigayears, Kelvin, Metres, Pascals};

use super::cells::{ClimateCell, QuantiseValueError};
use super::{CoarseLevel, boundary_diameter, coarse_level};
use crate::craters::CraterParams;
use crate::spheroid::Spheroid;
use crate::synth::BandSpectrum;

/// A body, as the surface crate names it: the raw 64-bit ID of its system and its index there,
/// the two halves of the sim's `BodyId`, which this crate cannot see.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct BodyRef {
    raw_system_id: u64,
    body_index: u16,
}

impl BodyRef {
    /// Body `body_index` of the system whose raw ID is `raw_system_id`.
    #[must_use]
    pub const fn new(raw_system_id: u64, body_index: u16) -> Self {
        Self {
            raw_system_id,
            body_index,
        }
    }

    /// The system's raw 64-bit ID.
    #[must_use]
    pub const fn raw_system_id(self) -> u64 {
        self.raw_system_id
    }

    /// The body's index in its system.
    #[must_use]
    pub const fn body_index(self) -> u16 {
        self.body_index
    }
}

coded_enum! {
    /// The coarse model a body's climate step ran, by its climate regime (Design note 8, open
    /// question 9's classifier), one byte in the header.
    ClimateModelKind {
        /// Design note 8's seasonal moist energy-balance model, diffusing near-surface moist static
        /// energy in latitude and longitude, for a world with a seasonal water cycle or one like
        /// it.
        EnergyBalance = 0,
        /// The same model in tidally locked coordinates about the substellar axis, with no seasons
        /// (R09.T13.b).
        LockedEnergyBalance = 1,
        /// Radiative equilibrium with thermal inertia, for an airless or thin-aired world
        /// (R09.T13.c).
        RadiativeEquilibrium = 2,
        /// An isothermal surface under a thick atmosphere, as on Venus (R09.T13.c).
        Isothermal = 3,
    }
}

coded_enum! {
    /// Where a field's precipitation came from, one byte in the header, so that a console labels it
    /// (Design note 8).
    PrecipitationSource {
        /// The documented monthly heuristic of Design note 8, after Siler, Roe and Armour (2018)
        /// and Hergarten and Robl (2022), labelled as a heuristic wherever it is shown: simple
        /// climate models cannot solve precipitation. Every field's precipitation is this one.
        Heuristic = 0,
    }
}

/// The parts of a [`FieldHeader`], as the coarse pass or a decoder has them, each in the SI unit
/// its type or name states.
///
/// Plain data with public fields: [`FieldHeader::new`] validates them and derives the field's
/// level and boundary diameter from the radius.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldHeaderParts {
    /// The body the field is of.
    pub body: BodyRef,
    /// The body's volumetric mean radius, metres: plan 14's, from which the field's level and
    /// boundary diameter follow (Design note 4).
    pub radius: Metres,
    /// The body's rotational spheroid, its equatorial radius a and polar radius c in metres, the
    /// datum every height of the field is measured from along its normal (R07's Design note 19;
    /// Design note 17): plan 14's `figure()`. Its volumetric radius ∛(a² c) is `radius`.
    pub figure: Spheroid,
    /// The height of the sea surface above the datum, metres: the area-weighted elevation
    /// quantile that leaves plan 14's ocean fraction below it (Design note 7), so on a world with
    /// no sea the lowest cell's elevation. The climate layer's temperatures are reduced to it.
    pub sea_level: Metres,
    /// The rate at which the surface temperature falls with height, kelvin per metre (positive
    /// for a fall): the climate step reduces each temperature to sea level with it, and the
    /// synthesis re-applies it from a point's elevation (Design note 17). 0.0065 K m⁻¹ in Earth's
    /// standard atmosphere (ICAO 1993), 0 on an airless body.
    pub lapse_rate_k_per_m: f64,
    /// The body's spectrum of relief finer than its coarse cells, which fixes the synthesis's
    /// structural amplitudes (Design note 7).
    pub spectrum: BandSpectrum,
    /// The body's crater contract with plan 14, which fixes the density of every crater narrower
    /// than the boundary diameter that the synthesis draws (Design note 12).
    pub craters: CraterParams,
    /// The coarse model the climate step ran (Design note 8).
    pub climate_model: ClimateModelKind,
    /// Where the precipitation came from: always the labelled heuristic (Design note 8).
    pub precipitation: PrecipitationSource,
    /// The realised hypsometric standard deviation `σ_h` of the reconstructed field, metres: the
    /// area-weighted RMS elevation about its mean, which the pass constrains to plan 14's
    /// (Design note 7). Finite and non-negative.
    pub realised_sigma_h: Metres,
    /// The realised greatest relief, metres: the highest elevation less the lowest, reported
    /// rather than matched, since a greatest relief grows with resolution (Design note 3); about
    /// 7–15 `σ_h` on every body measured. Finite and non-negative.
    pub realised_relief: Metres,
    /// The months of the body's year (Design note 8): 12, the twelve equal spans of the seasonal
    /// orbit's eccentric anomaly from periapsis ([`month_edges`](super::month_edges)), or 1 for a
    /// world with no seasonal forcing, a locked world on a circular orbit or one on a circular
    /// orbit with no obliquity.
    pub months: u8,
    /// The eccentricity e of the seasonal orbit, the one that sets the sun's declination and
    /// distance: the body's own about its star or stars, or its planet's for a moon (Design note
    /// 8; `decision-r09-t2.md` item 1). Its months are what the header's twelve span. At least 0
    /// and below 1, and 0 in a one-month year, whose orbit is circular.
    pub season_eccentricity: f64,
    /// The zero of every climate record's sea-level temperature, kelvin: plan 14's mean surface
    /// temperature, which the climate step imposes on the field (Design note 8). Finite and
    /// positive.
    pub reference_temperature: Kelvin,
    /// The step of every climate record's sea-level temperature, as n in 0.01 K × 2ⁿ, 0 to 15: the
    /// finest step that holds the body's largest departure from `reference_temperature` in an
    /// `i16`, so that none saturates ([`FieldHeader::temperature_step_for`]).
    pub temperature_step: u8,
    /// The step of every climate record's monthly anomaly, as n in 0.25 K × 2ⁿ, 0 to 15: the
    /// finest step that holds the body's largest anomaly in 127 steps (Design note 17;
    /// [`FieldHeader::anomaly_step_for`]), 0.25 K on an Earth, whose extremes are Verkhoyansk's
    /// −30.8 and +30.6 K about its annual mean of −13.9 °C (WMO 1991–2020 normals, station 24266),
    /// within the 31.75 K a 0.25 K step holds, 0.5 K on a Mars and 1 K on a high-obliquity world
    /// with land.
    pub anomaly_step: u8,
    /// The body's surface age, thousands of millions of years: plan 14's, carried for R11's rock
    /// abundance (Design note 18). Finite and non-negative.
    pub surface_age: Gigayears,
    /// The body's mean surface pressure, pascals: plan 14's, carried for R11's atmosphere class
    /// (Design note 18); 0 on an airless body. Finite and non-negative.
    pub surface_pressure: Pascals,
    /// R10's albedo scale, dimensionless and positive, or `None` until R10 fills it (Design note
    /// 18).
    pub albedo_scale: Option<f64>,
}

/// A coarse field's header: [`FieldHeaderParts`], validated, with the level and the boundary
/// diameter that follow from the radius.
///
/// The format and the generator version a field was computed under are not in it: the payload
/// ([`crate::wire`]) carries them in every block's own header and refuses another's, so a header
/// in memory is always this build's.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldHeader {
    parts: FieldHeaderParts,
    level: CoarseLevel,
    boundary_diameter: Metres,
}

/// Why [`FieldHeader::new`] refused its parts.
#[derive(Debug, Clone, PartialEq)]
pub enum BuildFieldHeaderError {
    /// The radius is not finite and positive.
    Radius(f64),
    /// The figure's radii are not finite with 0 < c ≤ a, or its volumetric radius is not the
    /// radius.
    Figure(Spheroid),
    /// The months are neither 1 nor 12.
    Months(u8),
    /// The season eccentricity is not at least +0 and below 1.
    SeasonEccentricity(f64),
    /// A one-month year's season eccentricity is not 0: its orbit is circular.
    EccentricOneMonthYear(f64),
    /// A step's exponent is above 15.
    StepExponent {
        /// The part's name.
        part: &'static str,
        /// The exponent.
        exponent: u8,
    },
    /// A part is not finite.
    NotFinite {
        /// The part's name.
        part: &'static str,
    },
    /// A part that cannot be negative is.
    Negative {
        /// The part's name.
        part: &'static str,
    },
    /// A part that must be positive is not.
    NotPositive {
        /// The part's name.
        part: &'static str,
    },
}

impl std::fmt::Display for BuildFieldHeaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Radius(r) => write!(f, "radius {r} m is not finite and positive"),
            Self::Figure(s) => write!(
                f,
                "figure a = {} m, c = {} m is not a valid datum for the radius",
                s.equatorial_radius_m, s.polar_radius_m
            ),
            Self::Months(m) => write!(f, "a year of {m} months is neither 1 nor 12"),
            Self::SeasonEccentricity(e) => {
                write!(f, "season eccentricity {e} is not at least 0 and below 1")
            }
            Self::EccentricOneMonthYear(e) => {
                write!(f, "a one-month year's orbit has eccentricity {e}, not 0")
            }
            Self::StepExponent { part, exponent } => {
                write!(f, "{part} exponent {exponent} is above {MAX_STEP_EXPONENT}")
            }
            Self::NotFinite { part } => write!(f, "{part} is not finite"),
            Self::Negative { part } => write!(f, "{part} is negative"),
            Self::NotPositive { part } => write!(f, "{part} is not positive"),
        }
    }
}

impl std::error::Error for BuildFieldHeaderError {}

/// The largest exponent of a step, 2¹⁵ times its base: 328 K for a temperature, 8,192 K for an
/// anomaly, beyond any body's range.
const MAX_STEP_EXPONENT: u8 = 15;

/// The base of the sea-level temperature's step, kelvin (Design note 17's 0.01 K).
const TEMPERATURE_STEP_BASE_K: f64 = 0.01;

/// The base of the monthly anomaly's step, kelvin (Design note 17's 0.25 K).
const ANOMALY_STEP_BASE_K: f64 = 0.25;

/// How far the figure's volumetric radius may differ from the radius, relative: the rounding of
/// `Spheroid::from_volumetric`'s cube root, far below any figure of another body.
const FIGURE_TOLERANCE: f64 = 1e-9;

/// `base` × 2^`exponent`, exact.
#[must_use]
fn step(base: f64, exponent: u8) -> f64 {
    base * f64::from(1_u32 << exponent)
}

/// The finest exponent n at most 15 whose step `base` × 2ⁿ holds `largest` in `limit` steps.
#[must_use]
fn finest_step(largest: f64, base: f64, limit: f64) -> Option<u8> {
    if !(largest.is_finite() && largest >= 0.0) {
        return None;
    }
    (0..=MAX_STEP_EXPONENT).find(|&n| (largest / step(base, n)).round() <= limit)
}

/// `value` ÷ `step`, rounded, if it lies within `min` to `max`.
fn quantise(value: f64, step: f64, min: f64, max: f64) -> Result<f64, QuantiseValueError> {
    if !value.is_finite() {
        return Err(QuantiseValueError::NotFinite(value));
    }
    let q = (value / step).round();
    if q < min || q > max {
        return Err(QuantiseValueError::OutOfRange(value));
    }
    Ok(q)
}

impl FieldHeader {
    /// The header of `parts`, with the level of [`coarse_level`] and the boundary diameter of
    /// [`boundary_diameter`] at the radius.
    ///
    /// # Errors
    ///
    /// [`BuildFieldHeaderError`] if the radius is not finite and positive, the figure is not the
    /// radius's spheroid, the months are neither 1 nor 12, the season eccentricity is outside
    /// [0, 1) or not 0 in a one-month year, a step's exponent is above 15, or a part is not finite
    /// or out of its range (each part's documentation states it).
    pub fn new(parts: FieldHeaderParts) -> Result<Self, BuildFieldHeaderError> {
        let r = parts.radius.value();
        if !(r.is_finite() && r > 0.0) {
            return Err(BuildFieldHeaderError::Radius(r));
        }
        let Spheroid {
            equatorial_radius_m: a,
            polar_radius_m: c,
        } = parts.figure;
        let figure_ok = a.is_finite()
            && c.is_finite()
            && c > 0.0
            && c <= a
            && (parts.figure.volumetric_radius_m() / r - 1.0).abs() <= FIGURE_TOLERANCE;
        if !figure_ok {
            return Err(BuildFieldHeaderError::Figure(parts.figure));
        }
        if parts.months != 1 && parts.months != 12 {
            return Err(BuildFieldHeaderError::Months(parts.months));
        }
        let e = parts.season_eccentricity;
        // A −0 is refused too, so that a circular orbit has one wire form.
        if !(0.0..1.0).contains(&e) || e.is_sign_negative() {
            return Err(BuildFieldHeaderError::SeasonEccentricity(e));
        }
        if parts.months == 1 && e > 0.0 {
            return Err(BuildFieldHeaderError::EccentricOneMonthYear(e));
        }
        for (part, exponent) in [
            ("temperature step", parts.temperature_step),
            ("anomaly step", parts.anomaly_step),
        ] {
            if exponent > MAX_STEP_EXPONENT {
                return Err(BuildFieldHeaderError::StepExponent { part, exponent });
            }
        }
        let finite = [
            ("sea level", parts.sea_level.value()),
            ("lapse rate", parts.lapse_rate_k_per_m),
            ("realised sigma_h", parts.realised_sigma_h.value()),
            ("realised relief", parts.realised_relief.value()),
            ("reference temperature", parts.reference_temperature.value()),
            ("surface age", parts.surface_age.value()),
            ("surface pressure", parts.surface_pressure.value()),
            ("albedo scale", parts.albedo_scale.unwrap_or(1.0)),
        ];
        if let Some(&(part, _)) = finite.iter().find(|(_, v)| !v.is_finite()) {
            return Err(BuildFieldHeaderError::NotFinite { part });
        }
        let non_negative = [
            ("realised sigma_h", parts.realised_sigma_h.value()),
            ("realised relief", parts.realised_relief.value()),
            ("surface age", parts.surface_age.value()),
            ("surface pressure", parts.surface_pressure.value()),
        ];
        if let Some(&(part, _)) = non_negative.iter().find(|(_, v)| *v < 0.0) {
            return Err(BuildFieldHeaderError::Negative { part });
        }
        let positive = [
            ("reference temperature", parts.reference_temperature.value()),
            ("albedo scale", parts.albedo_scale.unwrap_or(1.0)),
        ];
        if let Some(&(part, _)) = positive.iter().find(|(_, v)| *v <= 0.0) {
            return Err(BuildFieldHeaderError::NotPositive { part });
        }
        let level = coarse_level(parts.radius);
        let boundary_diameter = boundary_diameter(level, parts.radius);
        Ok(Self {
            parts,
            level,
            boundary_diameter,
        })
    }

    /// The validated parts.
    #[must_use]
    pub fn parts(&self) -> &FieldHeaderParts {
        &self.parts
    }

    /// The validated parts, by value.
    #[must_use]
    pub fn into_parts(self) -> FieldHeaderParts {
        self.parts
    }

    /// The header with its albedo scale set to `albedo_scale` (R10's, Design note 18), every other
    /// part kept.
    ///
    /// # Errors
    ///
    /// [`BuildFieldHeaderError::NotFinite`] or [`BuildFieldHeaderError::NotPositive`] for a scale
    /// that is not finite and positive.
    pub fn with_albedo_scale(
        mut self,
        albedo_scale: Option<f64>,
    ) -> Result<Self, BuildFieldHeaderError> {
        if let Some(scale) = albedo_scale {
            let part = "albedo scale";
            if !scale.is_finite() {
                return Err(BuildFieldHeaderError::NotFinite { part });
            }
            if scale <= 0.0 {
                return Err(BuildFieldHeaderError::NotPositive { part });
            }
        }
        self.parts.albedo_scale = albedo_scale;
        Ok(self)
    }

    /// The body the field is of.
    #[must_use]
    pub fn body(&self) -> BodyRef {
        self.parts.body
    }

    /// The body's volumetric mean radius ([`FieldHeaderParts::radius`]).
    #[must_use]
    pub fn radius(&self) -> Metres {
        self.parts.radius
    }

    /// The datum ([`FieldHeaderParts::figure`]).
    #[must_use]
    pub fn figure(&self) -> Spheroid {
        self.parts.figure
    }

    /// The field's level, [`coarse_level`] of the radius.
    #[must_use]
    pub fn level(&self) -> CoarseLevel {
        self.level
    }

    /// The boundary diameter `D_b`, [`boundary_diameter`] at the level and radius, metres: the
    /// field lists every crater of `D_b` and wider.
    #[must_use]
    pub fn boundary_diameter(&self) -> Metres {
        self.boundary_diameter
    }

    /// The sea surface's height above the datum ([`FieldHeaderParts::sea_level`]).
    #[must_use]
    pub fn sea_level(&self) -> Metres {
        self.parts.sea_level
    }

    /// The lapse rate, kelvin per metre ([`FieldHeaderParts::lapse_rate_k_per_m`]).
    #[must_use]
    pub fn lapse_rate_k_per_m(&self) -> f64 {
        self.parts.lapse_rate_k_per_m
    }

    /// The spectrum of relief finer than the cells ([`FieldHeaderParts::spectrum`]).
    #[must_use]
    pub fn spectrum(&self) -> &BandSpectrum {
        &self.parts.spectrum
    }

    /// The crater contract ([`FieldHeaderParts::craters`]).
    #[must_use]
    pub fn craters(&self) -> &CraterParams {
        &self.parts.craters
    }

    /// The climate model ([`FieldHeaderParts::climate_model`]).
    #[must_use]
    pub fn climate_model(&self) -> ClimateModelKind {
        self.parts.climate_model
    }

    /// The precipitation's source ([`FieldHeaderParts::precipitation`]).
    #[must_use]
    pub fn precipitation(&self) -> PrecipitationSource {
        self.parts.precipitation
    }

    /// The realised `σ_h` ([`FieldHeaderParts::realised_sigma_h`]).
    #[must_use]
    pub fn realised_sigma_h(&self) -> Metres {
        self.parts.realised_sigma_h
    }

    /// The realised greatest relief ([`FieldHeaderParts::realised_relief`]).
    #[must_use]
    pub fn realised_relief(&self) -> Metres {
        self.parts.realised_relief
    }

    /// The months of the year, 1 or 12 ([`FieldHeaderParts::months`]).
    #[must_use]
    pub fn months(&self) -> u8 {
        self.parts.months
    }

    /// The seasonal orbit's eccentricity, 0 to below 1
    /// ([`FieldHeaderParts::season_eccentricity`]).
    #[must_use]
    pub fn season_eccentricity(&self) -> f64 {
        self.parts.season_eccentricity
    }

    /// The zero of the sea-level temperatures ([`FieldHeaderParts::reference_temperature`]).
    #[must_use]
    pub fn reference_temperature(&self) -> Kelvin {
        self.parts.reference_temperature
    }

    /// The sea-level temperature's step exponent ([`FieldHeaderParts::temperature_step`]).
    #[must_use]
    pub fn temperature_step(&self) -> u8 {
        self.parts.temperature_step
    }

    /// The monthly anomaly's step exponent ([`FieldHeaderParts::anomaly_step`]).
    #[must_use]
    pub fn anomaly_step(&self) -> u8 {
        self.parts.anomaly_step
    }

    /// The step of the sea-level temperatures, kelvin: 0.01 K × 2ⁿ.
    #[must_use]
    pub fn temperature_step_k(&self) -> f64 {
        step(TEMPERATURE_STEP_BASE_K, self.parts.temperature_step)
    }

    /// The step of the monthly anomalies, kelvin: 0.25 K × 2ⁿ.
    #[must_use]
    pub fn anomaly_step_k(&self) -> f64 {
        step(ANOMALY_STEP_BASE_K, self.parts.anomaly_step)
    }

    /// The surface age ([`FieldHeaderParts::surface_age`]).
    #[must_use]
    pub fn surface_age(&self) -> Gigayears {
        self.parts.surface_age
    }

    /// The mean surface pressure ([`FieldHeaderParts::surface_pressure`]).
    #[must_use]
    pub fn surface_pressure(&self) -> Pascals {
        self.parts.surface_pressure
    }

    /// R10's albedo scale, `None` until R10 fills it ([`FieldHeaderParts::albedo_scale`]).
    #[must_use]
    pub fn albedo_scale(&self) -> Option<f64> {
        self.parts.albedo_scale
    }

    /// The annual mean surface temperature of `cell`, reduced to sea level, kelvin: its code times
    /// the temperature step, about the reference temperature.
    #[must_use]
    pub fn sea_level_temperature(&self, cell: &ClimateCell) -> Kelvin {
        Kelvin::new(
            self.parts.reference_temperature.value()
                + f64::from(cell.sea_level_temperature) * self.temperature_step_k(),
        )
    }

    /// The mean surface temperature of month `month` (from 0) of `cell`, reduced to sea level,
    /// kelvin: the annual mean plus the month's anomaly, or `None` for a month past the year.
    #[must_use]
    pub fn month_temperature(&self, cell: &ClimateCell, month: u8) -> Option<Kelvin> {
        if month >= self.parts.months {
            return None;
        }
        let anomaly = f64::from(cell.month_anomaly[usize::from(month)]) * self.anomaly_step_k();
        Some(Kelvin::new(
            self.sea_level_temperature(cell).value() + anomaly,
        ))
    }

    /// The code of an annual mean sea-level temperature `temperature`: its departure from the
    /// reference temperature in temperature steps, rounded half away from zero.
    ///
    /// # Errors
    ///
    /// [`QuantiseValueError::NotFinite`] for a temperature that is not finite, and
    /// [`QuantiseValueError::OutOfRange`] for one whose code would not fit an `i16`, which a
    /// step chosen by [`temperature_step_for`](Self::temperature_step_for) rules out.
    pub fn quantise_sea_level_temperature(
        &self,
        temperature: Kelvin,
    ) -> Result<i16, QuantiseValueError> {
        let t = temperature.value();
        if !t.is_finite() {
            return Err(QuantiseValueError::NotFinite(t));
        }
        let q = quantise(
            t - self.parts.reference_temperature.value(),
            self.temperature_step_k(),
            f64::from(i16::MIN),
            f64::from(i16::MAX),
        )
        .map_err(|_| QuantiseValueError::OutOfRange(t))?;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "q is an integer within the i16 range, checked above"
        )]
        Ok(q as i16)
    }

    /// The code of a monthly anomaly of `anomaly_k` kelvin about the annual mean: the anomaly in
    /// anomaly steps, rounded half away from zero.
    ///
    /// # Errors
    ///
    /// [`QuantiseValueError::NotFinite`] for an anomaly that is not finite, and
    /// [`QuantiseValueError::OutOfRange`] for one beyond 127 steps, which a step chosen by
    /// [`anomaly_step_for`](Self::anomaly_step_for) rules out.
    pub fn quantise_month_anomaly(&self, anomaly_k: f64) -> Result<i8, QuantiseValueError> {
        let q = quantise(anomaly_k, self.anomaly_step_k(), -127.0, 127.0)?;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "q is an integer within ±127, checked above"
        )]
        Ok(q as i8)
    }

    /// The exponent of the finest sea-level temperature step, 0.01 K × 2ⁿ, that holds a largest
    /// departure of `largest_k` kelvin from the reference temperature in an `i16`; `None` if
    /// `largest_k` is not finite and non-negative or no step to 2¹⁵ holds it.
    #[must_use]
    pub fn temperature_step_for(largest_k: f64) -> Option<u8> {
        finest_step(largest_k, TEMPERATURE_STEP_BASE_K, f64::from(i16::MAX))
    }

    /// The exponent of the finest anomaly step, 0.25 K × 2ⁿ, that holds a largest anomaly of
    /// `largest_k` kelvin in 127 steps (Design note 17); `None` if `largest_k` is not finite and
    /// non-negative or no step to 2¹⁵ holds it.
    #[must_use]
    pub fn anomaly_step_for(largest_k: f64) -> Option<u8> {
        finest_step(largest_k, ANOMALY_STEP_BASE_K, 127.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::{FieldView, LogPrecipitation, Wind};
    use crate::testing::{SyntheticWorld, synthetic_field};
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    fn parts() -> FieldHeaderParts {
        synthetic_field(SyntheticWorld::CeresLike)
            .header()
            .parts()
            .clone()
    }

    /// The header derives its level and `D_b` from the radius and keeps every part it was given.
    #[test]
    fn a_field_header_derives_its_level_and_boundary_diameter() {
        let parts = parts();
        let header = FieldHeader::new(parts.clone()).unwrap();
        assert_eq!(header.level(), coarse_level(parts.radius));
        assert_eq!(
            header.boundary_diameter(),
            boundary_diameter(header.level(), parts.radius)
        );
        assert_eq!(header.parts(), &parts);
        assert_eq!(header.clone().into_parts(), parts);
        assert_eq!(header.precipitation(), PrecipitationSource::Heuristic);
    }

    /// Each rule of the parts has its own refusal.
    #[test]
    fn a_field_header_refuses_parts_out_of_their_ranges() {
        let refuse = |edit: fn(&mut FieldHeaderParts)| {
            let mut p = parts();
            edit(&mut p);
            FieldHeader::new(p).unwrap_err()
        };
        assert_eq!(
            refuse(|p| p.radius = Metres::new(-1.0)),
            BuildFieldHeaderError::Radius(-1.0)
        );
        assert!(matches!(
            refuse(|p| p.radius = p.radius * 1.01),
            BuildFieldHeaderError::Figure(_)
        ));
        assert!(matches!(
            refuse(|p| p.figure = Spheroid {
                equatorial_radius_m: p.figure.polar_radius_m * 0.9,
                polar_radius_m: p.figure.polar_radius_m,
            }),
            BuildFieldHeaderError::Figure(_)
        ));
        assert_eq!(refuse(|p| p.months = 6), BuildFieldHeaderError::Months(6));
        assert_eq!(
            refuse(|p| p.season_eccentricity = 1.0),
            BuildFieldHeaderError::SeasonEccentricity(1.0)
        );
        assert_eq!(
            refuse(|p| p.season_eccentricity = -0.1),
            BuildFieldHeaderError::SeasonEccentricity(-0.1)
        );
        assert!(matches!(
            refuse(|p| p.season_eccentricity = -0.0),
            BuildFieldHeaderError::SeasonEccentricity(e) if e.is_sign_negative()
        ));
        assert!(matches!(
            refuse(|p| p.season_eccentricity = f64::NAN),
            BuildFieldHeaderError::SeasonEccentricity(e) if e.is_nan()
        ));
        assert_eq!(
            refuse(|p| {
                p.months = 1;
                p.season_eccentricity = 0.05;
            }),
            BuildFieldHeaderError::EccentricOneMonthYear(0.05)
        );
        assert_eq!(
            refuse(|p| p.anomaly_step = 16),
            BuildFieldHeaderError::StepExponent {
                part: "anomaly step",
                exponent: 16
            }
        );
        assert_eq!(
            refuse(|p| p.sea_level = Metres::new(f64::NAN)),
            BuildFieldHeaderError::NotFinite { part: "sea level" }
        );
        assert_eq!(
            refuse(|p| p.surface_pressure = Pascals::new(-1.0)),
            BuildFieldHeaderError::Negative {
                part: "surface pressure"
            }
        );
        assert_eq!(
            refuse(|p| p.albedo_scale = Some(0.0)),
            BuildFieldHeaderError::NotPositive {
                part: "albedo scale"
            }
        );
    }

    /// Design note 17's steps: 0.25 K for an Earth's ±31 K, 0.5 K for a Mars's ±60 K and 1 K for
    /// ±70 K; 0.01 K for a departure of 300 K, 0.02 K for 600 K.
    #[test]
    fn the_finest_step_holds_the_largest_anomaly() {
        assert_eq!(FieldHeader::anomaly_step_for(31.0), Some(0));
        assert_eq!(FieldHeader::anomaly_step_for(60.0), Some(1));
        assert_eq!(FieldHeader::anomaly_step_for(70.0), Some(2));
        assert_eq!(FieldHeader::temperature_step_for(300.0), Some(0));
        assert_eq!(FieldHeader::temperature_step_for(600.0), Some(1));
        assert_eq!(FieldHeader::anomaly_step_for(f64::NAN), None);
        assert_eq!(FieldHeader::anomaly_step_for(1e7), None);
    }

    /// A temperature and an anomaly come back from their codes to within half a step.
    #[test]
    fn field_temperatures_round_trip_to_half_a_step() {
        let mut p = parts();
        p.months = 12;
        p.reference_temperature = Kelvin::new(250.0);
        p.temperature_step = 1;
        p.anomaly_step = 2;
        let header = FieldHeader::new(p).unwrap();
        assert!((header.temperature_step_k() - 0.02).abs() < 1e-15);
        assert!((header.anomaly_step_k() - 1.0).abs() < 1e-15);
        let mut cell = ClimateCell {
            sea_level_temperature: 0,
            month_anomaly: [0; 12],
            month_precipitation: [LogPrecipitation::NONE; 12],
            wind: [Wind::CALM; 12],
        };
        for t in [-400.0, -1.234, 0.0, 17.777, 600.0] {
            let code = header
                .quantise_sea_level_temperature(Kelvin::new(250.0 + t))
                .unwrap();
            cell.sea_level_temperature = code;
            let back = header.sea_level_temperature(&cell).value();
            assert!((back - (250.0 + t)).abs() <= 0.01 + 1e-9, "{t} K → {back}");
        }
        for a in [-127.0, -30.4, 0.6, 126.9] {
            let code = header.quantise_month_anomaly(a).unwrap();
            cell.month_anomaly[5] = code;
            let back = header.month_temperature(&cell, 5).unwrap().value()
                - header.sea_level_temperature(&cell).value();
            assert!((back - a).abs() <= 0.5 + 1e-9, "{a} K → {back}");
        }
        assert_eq!(header.month_temperature(&cell, 12), None);
        assert_eq!(
            header.quantise_month_anomaly(128.0),
            Err(QuantiseValueError::OutOfRange(128.0))
        );
        assert_eq!(
            header.quantise_sea_level_temperature(Kelvin::new(2_000.0)),
            Err(QuantiseValueError::OutOfRange(2_000.0))
        );
    }
}
