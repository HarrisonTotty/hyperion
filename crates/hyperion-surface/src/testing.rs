//! Synthetic coarse fields for tests: the worlds R09's synthesis (T3 to T9) and R10 test on,
//! built directly rather than by the coarse pass, which only the sim runs (plan R09, T2; feature
//! `testing`, and this crate's own tests).
//!
//! [`synthetic_field`] builds one of six [`SyntheticWorld`]s: an Earth-, a Mars-, a Moon- and a
//! Ceres-like field, a flat field and a single crater. [`FieldBuilder`] builds any other: the caller
//! gives the body, the plates or the crust, and the elevation, ice and climate as functions of
//! position, and the builder derives what depends on them (the plates' boundaries, the water, the
//! flow and drainage, the craters' morphology, relief and reach, the header's realised figures and
//! steps), so that every field it returns is valid by construction.
//!
//! These are test inputs, not generated worlds: nothing here draws from a stream, the values are
//! illustrative ones near the real bodies', and the coarse pass's physics (R09.T10–T16) is
//! replaced by closed forms. Every value is a pure function of the builder's inputs, computed in
//! cell-index order, so a world built twice is the same to the bit. Where the pass would do more,
//! the builder says so: the header's realised `σ_h` and relief are the cells' (their area-weighted
//! RMS about the mean, and their range), not the reconstructed field's, which needs R09.T4's
//! interpolant; the reference temperature is the climate layer's area-weighted mean; flow follows
//! steepest descent with no depression filling; and a crater's relief is its profile sampled at
//! each cell's centre, not smoothed to the cell.

mod worlds;

pub use worlds::{SyntheticWorld, synthetic_field};

use hyperion_base::math;
use hyperion_base::units::{
    Gigayears, Kelvin, Metres, MetresPerSecond, MetresPerSecondSquared, Pascals,
    PerSquareKilometre, Radians, SquareMetres,
};

use crate::craters::{CraterParams, CraterParamsParts, Screening};
use crate::cube::{Edge, PatchKey, st_to_uv};
use crate::field::{
    BodyRef, BoundaryKind, ClimateCell, ClimateModelKind, CoarseCrater, CoarseField, CoarseLevel,
    Cover, Crust, FieldHeader, FieldHeaderParts, FlowDirection, LogArea, LogPrecipitation,
    LogSteepness, Morphology, PrecipitationSource, SurfaceClass, SynthesisCell, Wind,
    boundary_diameter, cell_at_index, cell_index, coarse_level,
};
use crate::spheroid::Spheroid;
use crate::synth::BandSpectrum;

/// A plate of a mobile-lid world: its seed, its crust and its motion.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlateSpec {
    /// The seed, a unit vector in the body-fixed frame: each cell takes the plate whose seed is
    /// nearest its centre, ties to the lower plate.
    pub seed: [f64; 3],
    /// The plate's crust, [`Crust::Continental`] or [`Crust::Oceanic`].
    pub crust: Crust,
    /// The plate's angular velocity in the body-fixed frame, radians per million years: its
    /// surface moves at ω × p.
    pub rotation_rad_per_myr: [f64; 3],
}

/// What the builder's functions know of a cell: its centre and its tectonic setting.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellSite {
    /// The cell's centre, a unit vector in the body-fixed frame.
    pub dir: [f64; 3],
    /// The cell's plate, 0 on a stagnant lid.
    pub plate: u8,
    /// The crust under the cell.
    pub crust: Crust,
    /// The kind of the nearest plate boundary, [`BoundaryKind::Absent`] on a stagnant lid.
    pub boundary: BoundaryKind,
    /// The signed great-circle distance to the nearest boundary, negative on a subducting plate,
    /// or `None` on a stagnant lid.
    pub boundary_distance: Option<Metres>,
    /// The obliquity of the relative motion at the nearest boundary, 0 to π ÷ 2.
    pub boundary_obliquity: Radians,
}

/// A climate cell's climate, in SI units, which the builder quantises.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClimateSample {
    /// The annual mean surface temperature reduced to sea level.
    pub sea_level_temperature: Kelvin,
    /// Each month's mean temperature less the annual mean, kelvin (none is read in a one-month
    /// year, which has no anomaly).
    pub month_anomaly_k: [f64; 12],
    /// Each month's precipitation rate, kg m⁻² s⁻¹ (only the first is read in a one-month year).
    pub month_precipitation_kg_m2_s: [f64; 12],
    /// Each month's 10 m wind: its mean speed and the direction its resultant blows towards,
    /// clockwise from local north (only the first is read in a one-month year, whose eleven later
    /// winds are calm).
    pub wind: [(MetresPerSecond, Radians); 12],
}

impl ClimateSample {
    /// A climate with no seasons, no precipitation and no wind, at `temperature`.
    #[must_use]
    pub const fn still(temperature: Kelvin) -> Self {
        Self {
            sea_level_temperature: temperature,
            month_anomaly_k: [0.0; 12],
            month_precipitation_kg_m2_s: [0.0; 12],
            wind: [(MetresPerSecond::ZERO, Radians::ZERO); 12],
        }
    }
}

/// How the builder routes water.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Routing {
    /// Not at all: every cell is terminal, with no drainage, as on a world with no fluvial step.
    None,
    /// Each dry cell to its edge neighbour of steepest descent, if any is lower, accumulating
    /// drainage area downstream; a cell under water is terminal.
    SteepestDescent,
}

/// A crater the builder places: its centre, diameter, age and degradation code.
#[derive(Debug, Clone, Copy, PartialEq)]
struct CraterSpec {
    centre: [f64; 3],
    diameter: Metres,
    age: Gigayears,
    degradation: u8,
}

/// A function of a cell's site.
type SiteFn<T> = Box<dyn Fn(&CellSite) -> T>;

/// A function of a direction.
type DirFn<T> = Box<dyn Fn([f64; 3]) -> T>;

/// The share of a cell under ice, from its site and its elevation.
type IceFn = Box<dyn Fn(&CellSite, Metres) -> f64>;

/// Builds a [`CoarseField`] that is valid by construction (see the module documentation).
///
/// [`FieldBuilder::new`] starts from a dry, airless, flat sphere with a one-month year on a
/// circular orbit, a still climate at 250 K, no fine relief and no craters, coarse or small (a
/// crater density of zero): every other part is optional.
pub struct FieldBuilder {
    body: BodyRef,
    radius: Metres,
    figure: Option<Spheroid>,
    sea: Option<Metres>,
    lapse_rate_k_per_m: f64,
    spectrum: BandSpectrum,
    crater_params: CraterParams,
    climate_model: ClimateModelKind,
    months: u8,
    season_eccentricity: f64,
    surface_age: Gigayears,
    surface_pressure: Pascals,
    plates: Vec<PlateSpec>,
    crust: DirFn<Crust>,
    elevation: SiteFn<Metres>,
    ice: IceFn,
    climate: DirFn<ClimateSample>,
    routing: Routing,
    craters: Vec<CraterSpec>,
}

impl std::fmt::Debug for FieldBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FieldBuilder")
            .field("body", &self.body)
            .field("radius", &self.radius)
            .field("plates", &self.plates.len())
            .field("craters", &self.craters.len())
            .finish_non_exhaustive()
    }
}

/// The rim radii a crater's reach extends to, its ejecta's edge: 1 + a with a = 1.54, the exterior
/// width that balances a lunar depth-to-rim-height ratio of 5.42 (Design note 13): 0.195 ÷ 0.036,
/// Pike 1980's Table 2 depth of fresh mare simple craters, d = 0.195 D^1.013, over the rim height
/// h = 0.036 D^1.014 usually credited to Pike 1977, which is not yet checked (R09.T7.b checks it).
const REACH_RIM_RADII: f64 = 2.54;

/// The width of the crater profile's exterior, rim radii (Design note 13's a).
const EJECTA_WIDTH: f64 = REACH_RIM_RADII - 1.0;

impl FieldBuilder {
    /// A dry, airless, flat sphere of volumetric radius `radius`, with a one-month year on a
    /// circular orbit, a still climate at 250 K, no fine relief and no craters: no coarse crater,
    /// and a crater density of zero, so that the synthesis draws no small ones either (at the
    /// Moon's gravity, `k_target` 1). The radius is checked by [`build`](Self::build).
    ///
    /// # Panics
    ///
    /// Never: its default spectrum and crater contract are valid.
    #[must_use]
    pub fn new(radius: Metres) -> Self {
        Self {
            body: BodyRef::default(),
            radius,
            figure: None,
            sea: None,
            lapse_rate_k_per_m: 0.0,
            spectrum: BandSpectrum::new(BandSpectrum::DEFAULT_EXPONENT, SquareMetres::ZERO)
                .expect("the default spectrum is valid"),
            crater_params: CraterParams::new(CraterParamsParts {
                n_1km: PerSquareKilometre::ZERO,
                screening: Screening::None,
                gravity: MetresPerSecondSquared::new(1.62),
                k_target: 1.0,
                impact_velocity: None,
            })
            .expect("the default crater contract is valid"),
            climate_model: ClimateModelKind::RadiativeEquilibrium,
            months: 1,
            season_eccentricity: 0.0,
            surface_age: Gigayears::new(4.0),
            surface_pressure: Pascals::ZERO,
            plates: Vec::new(),
            crust: Box::new(|_| Crust::Lid),
            elevation: Box::new(|_| Metres::ZERO),
            ice: Box::new(|_, _| 0.0),
            climate: Box::new(|_| ClimateSample::still(Kelvin::new(250.0))),
            routing: Routing::None,
            craters: Vec::new(),
        }
    }

    /// The body the field is of.
    #[must_use]
    pub fn body(mut self, body: BodyRef) -> Self {
        self.body = body;
        self
    }

    /// The datum, whose volumetric radius must be the builder's radius; a sphere otherwise.
    #[must_use]
    pub fn figure(mut self, figure: Spheroid) -> Self {
        self.figure = Some(figure);
        self
    }

    /// A sea whose surface lies at `level` above the datum: every cell below it is under water.
    /// Without one the world is dry, and the header's sea level is its lowest elevation.
    #[must_use]
    pub fn sea(mut self, level: Metres) -> Self {
        self.sea = Some(level);
        self
    }

    /// The lapse rate, kelvin per metre.
    #[must_use]
    pub fn lapse_rate(mut self, k_per_m: f64) -> Self {
        self.lapse_rate_k_per_m = k_per_m;
        self
    }

    /// The spectrum of relief finer than the cells.
    #[must_use]
    pub fn spectrum(mut self, spectrum: BandSpectrum) -> Self {
        self.spectrum = spectrum;
        self
    }

    /// The crater contract.
    #[must_use]
    pub fn crater_params(mut self, params: CraterParams) -> Self {
        self.crater_params = params;
        self
    }

    /// The climate model the header names.
    #[must_use]
    pub fn climate_model(mut self, model: ClimateModelKind) -> Self {
        self.climate_model = model;
        self
    }

    /// The months of the year, 1 or 12.
    #[must_use]
    pub fn months(mut self, months: u8) -> Self {
        self.months = months;
        self
    }

    /// The eccentricity of the seasonal orbit, whose twelve equal spans of eccentric anomaly are
    /// the months (0 by default; 0 in a one-month year).
    #[must_use]
    pub fn season_eccentricity(mut self, eccentricity: f64) -> Self {
        self.season_eccentricity = eccentricity;
        self
    }

    /// The surface age.
    #[must_use]
    pub fn surface_age(mut self, age: Gigayears) -> Self {
        self.surface_age = age;
        self
    }

    /// The mean surface pressure.
    #[must_use]
    pub fn surface_pressure(mut self, pressure: Pascals) -> Self {
        self.surface_pressure = pressure;
        self
    }

    /// Mobile-lid plates, at most 256: their crusts replace [`crust`](Self::crust)'s, and their
    /// boundaries are classed by the plates' relative motion.
    #[must_use]
    pub fn plates(mut self, plates: Vec<PlateSpec>) -> Self {
        self.plates = plates;
        self
    }

    /// A stagnant lid's crust at each cell's centre ([`Crust::Lid`] by default).
    #[must_use]
    pub fn crust(mut self, crust: impl Fn([f64; 3]) -> Crust + 'static) -> Self {
        self.crust = Box::new(crust);
        self
    }

    /// The elevation at each cell's centre, before the craters' relief is added.
    #[must_use]
    pub fn elevation(mut self, elevation: impl Fn(&CellSite) -> Metres + 'static) -> Self {
        self.elevation = Box::new(elevation);
        self
    }

    /// The share of each cell under ice, 0 to 1 (clamped), from its site and its elevation.
    #[must_use]
    pub fn ice(mut self, ice: impl Fn(&CellSite, Metres) -> f64 + 'static) -> Self {
        self.ice = Box::new(ice);
        self
    }

    /// The climate at each climate cell's centre.
    #[must_use]
    pub fn climate(mut self, climate: impl Fn([f64; 3]) -> ClimateSample + 'static) -> Self {
        self.climate = Box::new(climate);
        self
    }

    /// How water is routed.
    #[must_use]
    pub fn routing(mut self, routing: Routing) -> Self {
        self.routing = routing;
        self
    }

    /// A crater of `diameter` (at least the field's boundary diameter) centred on the unit vector
    /// `centre`, formed `age` ago, of degradation code `degradation` (255ths of its rim lost). Its
    /// morphology, depth and rim follow from its diameter, and its relief, Design note 13's
    /// volume-balanced profile sampled at each cell's centre, is added to the elevation.
    #[must_use]
    pub fn crater(
        mut self,
        centre: [f64; 3],
        diameter: Metres,
        age: Gigayears,
        degradation: u8,
    ) -> Self {
        self.craters.push(CraterSpec {
            centre,
            diameter,
            age,
            degradation,
        });
        self
    }

    /// The field.
    ///
    /// # Panics
    ///
    /// If an input is out of its range:
    /// - a radius that is not finite and positive, a figure of another radius, months other than
    ///   1 or 12, or a season eccentricity outside [0, 1) or not 0 in a one-month year;
    /// - exactly one plate or more than 256, two plates with one seed, or a plate whose crust is
    ///   neither continental nor oceanic;
    /// - a crater whose centre is not a finite unit vector, whose age is not finite and
    ///   non-negative, or which is narrower than the boundary diameter, or two craters of one
    ///   centre cell and diameter;
    /// - an elevation beyond ±2,147 km, or a climate whose temperatures, anomalies, rates or winds
    ///   no code can hold.
    #[must_use]
    pub fn build(self) -> CoarseField {
        self.check_plates();
        for spec in &self.craters {
            let norm = norm(spec.centre);
            assert!(
                spec.centre.iter().all(|c| c.is_finite()) && (norm - 1.0).abs() < 1e-12,
                "a crater's centre must be a finite unit vector, got {:?}",
                spec.centre
            );
            let age = spec.age.value();
            assert!(
                age.is_finite() && age >= 0.0,
                "a crater's age must be finite and non-negative, got {age} Gyr"
            );
        }
        let level = coarse_level(self.radius);
        let r = self.radius.value();
        let d_b = boundary_diameter(level, self.radius);
        let grid = Grid::new(level.get());
        let sites: Vec<CellSite> = grid.centres.iter().map(|&dir| self.site(dir, r)).collect();
        let craters: Vec<PreparedCrater> = self
            .craters
            .iter()
            .map(|spec| self.prepare(spec, d_b))
            .collect();
        let elevation_mm: Vec<i32> = sites
            .iter()
            .map(|site| {
                let mut h = (self.elevation)(site).value();
                for crater in &craters {
                    h += crater.relief(site.dir, r);
                }
                SynthesisCell::quantise_height(Metres::new(h))
                    .expect("a synthetic elevation is within ±2,147 km")
            })
            .collect();
        let lowest_mm = elevation_mm.iter().copied().min().unwrap_or(0);
        let (sea_level, water_mm): (Metres, Vec<i32>) = match self.sea {
            Some(level) => {
                let sea_mm =
                    SynthesisCell::quantise_height(level).expect("a sea level is within range");
                (level, elevation_mm.iter().map(|&e| e.max(sea_mm)).collect())
            }
            None => (
                Metres::new(f64::from(lowest_mm) / 1e3),
                elevation_mm.clone(),
            ),
        };
        let routes = match self.routing {
            Routing::None => vec![Route::TERMINAL; elevation_mm.len()],
            Routing::SteepestDescent => grid.route(&elevation_mm, &water_mm, r),
        };
        let synthesis: Vec<SynthesisCell> = (0..sites.len())
            .map(|n| self.record(&sites[n], elevation_mm[n], water_mm[n], routes[n]))
            .collect();
        let (sigma_h, relief) = grid.hypsometry(&elevation_mm);
        let climate_grid = Grid::new(level.climate_level());
        let samples: Vec<ClimateSample> = climate_grid
            .centres
            .iter()
            .map(|&dir| (self.climate)(dir))
            .collect();
        let (reference, temperature_step, anomaly_step) =
            climate_steps(&climate_grid, &samples, self.months);
        let header = FieldHeader::new(FieldHeaderParts {
            body: self.body,
            radius: self.radius,
            figure: self.figure.unwrap_or_else(|| Spheroid::sphere(r)),
            sea_level,
            lapse_rate_k_per_m: self.lapse_rate_k_per_m,
            spectrum: self.spectrum,
            craters: self.crater_params,
            climate_model: self.climate_model,
            precipitation: PrecipitationSource::Heuristic,
            realised_sigma_h: sigma_h,
            realised_relief: relief,
            months: self.months,
            season_eccentricity: self.season_eccentricity,
            reference_temperature: reference,
            temperature_step,
            anomaly_step,
            surface_age: self.surface_age,
            surface_pressure: self.surface_pressure,
            albedo_scale: None,
        })
        .expect("the builder's header parts are valid");
        let climate: Vec<ClimateCell> = samples
            .iter()
            .map(|sample| quantise_climate(&header, sample))
            .collect();
        let mut listed: Vec<(u32, CoarseCrater)> = craters
            .iter()
            .map(|crater| crater.listed(&grid, level, r))
            .collect();
        listed.sort_by(|(a, x), (b, y)| a.cmp(b).then_with(|| x.diameter.total_cmp(&y.diameter)));
        let craters = listed.into_iter().map(|(_, crater)| crater).collect();
        CoarseField::new(header, synthesis, climate, craters)
            .expect("a built field is valid by construction")
    }

    /// The synthesis record of the cell at `site`, with its quantised elevation and water surface
    /// and its route.
    fn record(
        &self,
        site: &CellSite,
        elevation_mm: i32,
        water_mm: i32,
        route: Route,
    ) -> SynthesisCell {
        let elevation = Metres::new(f64::from(elevation_mm) / 1e3);
        let ice = (self.ice)(site, elevation).clamp(0.0, 1.0);
        SynthesisCell {
            elevation_mm,
            boundary_distance_km: SynthesisCell::quantise_boundary_distance(site.boundary_distance)
                .expect("a synthetic boundary distance is finite"),
            plate: site.plate,
            crust: site.crust,
            boundary: site.boundary,
            boundary_obliquity: SynthesisCell::quantise_obliquity(site.boundary_obliquity)
                .expect("an obliquity is 0 to π ÷ 2"),
            flow: route.flow,
            drainage: route.drainage,
            steepness: route.steepness,
            water_surface_mm: water_mm,
            ice: SynthesisCell::quantise_ice(ice).expect("an ice share is 0 to 1"),
            class: SurfaceClass::UNCLASSIFIED,
            crater_state: 0,
        }
    }

    /// Asserts the plates are none or between two and 256, with distinct seeds and mobile-lid
    /// crusts.
    fn check_plates(&self) {
        let count = self.plates.len();
        assert!(
            count != 1 && count <= 256,
            "a field has no plates or two to 256, not {count}"
        );
        for (k, plate) in self.plates.iter().enumerate() {
            assert!(
                matches!(plate.crust, Crust::Continental | Crust::Oceanic),
                "plate {k}'s crust is {:?}, not continental or oceanic",
                plate.crust
            );
            for other in &self.plates[..k] {
                assert!(
                    norm(sub(plate.seed, other.seed)) > 0.0,
                    "plate {k} shares its seed with another"
                );
            }
        }
    }

    /// The site of the cell centred on `dir`, on a body of radius `r` metres.
    fn site(&self, dir: [f64; 3], r: f64) -> CellSite {
        if self.plates.is_empty() {
            return CellSite {
                dir,
                plate: 0,
                crust: (self.crust)(dir),
                boundary: BoundaryKind::Absent,
                boundary_distance: None,
                boundary_obliquity: Radians::ZERO,
            };
        }
        // The nearest seed, ties to the lower plate.
        let mut own = 0;
        for (k, plate) in self.plates.iter().enumerate().skip(1) {
            if dot(dir, plate.seed) > dot(dir, self.plates[own].seed) {
                own = k;
            }
        }
        let a = self.plates[own];
        // The nearest boundary: the bisecting great circle closest to the centre, whose
        // half-spaces' intersection is the plate's cell.
        let mut nearest: Option<(usize, f64, [f64; 3])> = None;
        for (k, b) in self.plates.iter().enumerate() {
            if k == own {
                continue;
            }
            let normal = unit(sub(a.seed, b.seed));
            let angle = math::asin(dot(dir, normal).clamp(-1.0, 1.0));
            if nearest.is_none_or(|(_, best, _)| angle < best) {
                nearest = Some((k, angle, normal));
            }
        }
        let (other, angle, normal) =
            nearest.expect("check_plates leaves no world with exactly one plate");
        let b = self.plates[other];
        // The boundary's normal at the centre, pointing from this plate into the other.
        let towards = unit(scale(sub(normal, scale(dir, dot(normal, dir))), -1.0));
        let relative = cross(sub(a.rotation_rad_per_myr, b.rotation_rad_per_myr), dir);
        let closing = dot(relative, towards);
        let sliding = norm(sub(relative, scale(towards, closing)));
        let direction = math::atan2(sliding, closing);
        let (boundary, subducts) = if direction < core::f64::consts::FRAC_PI_3 {
            // Two continents collide; otherwise the oceanic plate subducts beneath the continental
            // one, and of two oceanic plates the higher-numbered. `check_plates` admits no other
            // crust on a plate.
            match (a.crust, b.crust) {
                (Crust::Continental, Crust::Continental) => (BoundaryKind::Collision, false),
                (Crust::Oceanic, Crust::Continental) => (BoundaryKind::Subduction, true),
                (Crust::Continental, Crust::Oceanic) => (BoundaryKind::Subduction, false),
                (Crust::Oceanic, Crust::Oceanic) => (BoundaryKind::Subduction, own > other),
                (Crust::Lid | Crust::Province, _) | (_, Crust::Lid | Crust::Province) => {
                    unreachable!("check_plates admits only continental and oceanic plates")
                }
            }
        } else if direction > 2.0 * core::f64::consts::FRAC_PI_3 {
            (BoundaryKind::Divergent, false)
        } else {
            (BoundaryKind::Transform, false)
        };
        let distance = angle * r;
        CellSite {
            dir,
            plate: u8::try_from(own).expect("at most 256 plates"),
            crust: a.crust,
            boundary,
            boundary_distance: Some(Metres::new(if subducts { -distance } else { distance })),
            boundary_obliquity: Radians::new(math::atan2(sliding, closing.abs())),
        }
    }

    /// `spec`'s morphology and profile, from its diameter and the body's transition diameter.
    fn prepare(&self, spec: &CraterSpec, d_b: Metres) -> PreparedCrater {
        let d = spec.diameter.value();
        assert!(
            d.is_finite() && d >= d_b.value(),
            "a coarse crater of {d} m is narrower than the boundary diameter {} m",
            d_b.value()
        );
        // D_t = 19 km × (1.62 ÷ g) × k_target (Design note 12, after Pike 1980), a zone: bowls
        // below 0.8 D_t, transitional to 1.5 D_t, central peaks above, peak rings from about 9 D_t
        // and multi-ring basins above about 16 D_t.
        let params = &self.crater_params;
        let d_t = 19e3 * (1.62 / params.gravity().value()) * params.k_target();
        let morphology = match d / d_t {
            x if x < 0.8 => Morphology::Simple,
            x if x < 1.5 => Morphology::Transitional,
            x if x < 9.0 => Morphology::CentralPeak,
            x if x < 16.0 => Morphology::PeakRing,
            _ => Morphology::MultiRingBasin,
        };
        // Depth, rim to floor: 0.2 D for a bowl, 0.84 (D_t ÷ 19 km) D^0.33 km for a complex crater
        // (Design note 12, Pike 1980's Table 2 lunar mare fits 0.195 D^1.013 and 0.841 D^0.332,
        // rounded); the rim height H balances the volume, d ÷ H = 2 + 1.6a + 0.4a² (Design note 13).
        let depth = if d < d_t {
            0.2 * d
        } else {
            840.0 * (d_t / 19e3) * math::powf(d / 1e3, 0.33)
        };
        let rim_m = depth / (2.0 + 1.6 * EJECTA_WIDTH + 0.4 * EJECTA_WIDTH * EJECTA_WIDTH);
        PreparedCrater {
            spec: *spec,
            morphology,
            rim_m,
            floor_m: depth - rim_m,
        }
    }
}

/// A crater with its morphology and its profile's rim height and floor depth.
struct PreparedCrater {
    spec: CraterSpec,
    morphology: Morphology,
    rim_m: f64,
    floor_m: f64,
}

impl PreparedCrater {
    /// The crater's relief at the unit vector `dir` on a body of radius `r` metres: Design note
    /// 13's interior −d₀ + (H + d₀) x² and exterior H (1 − s)³ (1 + 3s), s = (x − 1) ÷ a, with x
    /// the distance in rim radii.
    fn relief(&self, dir: [f64; 3], r: f64) -> f64 {
        let x = arc(self.spec.centre, dir) * r / (self.spec.diameter.value() / 2.0);
        if x <= 1.0 {
            -self.floor_m + (self.rim_m + self.floor_m) * x * x
        } else {
            let s = (x - 1.0) / EJECTA_WIDTH;
            if s >= 1.0 {
                0.0
            } else {
                let t = 1.0 - s;
                self.rim_m * t * t * t * (1.0 + 3.0 * s)
            }
        }
    }

    /// The crater as the field lists it, with the index of its centre's cell: its reach is every
    /// cell whose nearest point may lie within [`REACH_RIM_RADII`] of the centre, by the angle to
    /// the cell's centre less the cell's circumradius, with the centre's own cell.
    fn listed(&self, grid: &Grid, level: CoarseLevel, r: f64) -> (u32, CoarseCrater) {
        let spec = &self.spec;
        let reach_angle = REACH_RIM_RADII * spec.diameter.value() / 2.0 / r;
        let centre_cell = cell_index(
            PatchKey::containing(level.get(), spec.centre).expect("a coarse level is valid"),
        );
        let touched = (0_u32..).zip(&grid.centres).filter_map(|(n, &c)| {
            let slack =
                grid.circumradii_rad[usize::try_from(n).expect("a cell index fits a usize")];
            (arc(spec.centre, c) - slack <= reach_angle || n == centre_cell).then_some(n)
        });
        let crater = CoarseCrater {
            centre: spec.centre,
            diameter: spec.diameter,
            morphology: self.morphology,
            age: spec.age,
            degradation: spec.degradation,
            reach: Cover::from_cells(touched),
        };
        (centre_cell, crater)
    }
}

/// One cell's flow, drainage and steepness.
#[derive(Debug, Clone, Copy)]
struct Route {
    flow: FlowDirection,
    drainage: LogArea,
    steepness: LogSteepness,
}

impl Route {
    const TERMINAL: Self = Self {
        flow: FlowDirection::Terminal,
        drainage: LogArea::ZERO,
        steepness: LogSteepness::ZERO,
    };
}

/// The cells of one level in cell-index order, with their centres, solid angles and
/// circumradii.
struct Grid {
    cells: Vec<PatchKey>,
    centres: Vec<[f64; 3]>,
    solid_angles_sr: Vec<f64>,
    circumradii_rad: Vec<f64>,
}

impl Grid {
    fn new(level: u8) -> Self {
        let count = 6_u32 << (2 * level);
        let cells: Vec<PatchKey> = (0..count)
            .map(|n| cell_at_index(level, n).expect("every index below the count is a cell"))
            .collect();
        // Vertex (32, 32) of a cell's patch is its centre, s = (i + ½) ÷ 2ᴸ.
        let centres: Vec<[f64; 3]> = cells.iter().map(|c| c.vertex_dir(32, 32)).collect();
        let solid_angles_sr = cells.iter().map(|&c| solid_angle(c)).collect();
        let circumradii_rad = cells
            .iter()
            .zip(&centres)
            .map(|(c, &centre)| {
                [(0, 0), (64, 0), (0, 64), (64, 64)]
                    .into_iter()
                    .map(|(x, y)| arc(centre, c.vertex_dir(x, y)))
                    .fold(0.0, crate::num::max)
            })
            .collect();
        Self {
            cells,
            centres,
            solid_angles_sr,
            circumradii_rad,
        }
    }

    /// Steepest descent to an edge neighbour from each dry cell, ties to the first edge of
    /// [`Edge::ALL`], and the drainage area accumulated downstream in order of falling elevation.
    fn route(&self, elevation_mm: &[i32], water_mm: &[i32], r: f64) -> Vec<Route> {
        let n = self.cells.len();
        let mut receivers: Vec<Option<(usize, Edge, f64)>> = vec![None; n];
        for (k, &cell) in self.cells.iter().enumerate() {
            if water_mm[k] > elevation_mm[k] {
                continue;
            }
            for edge in Edge::ALL {
                let next = usize::try_from(cell_index(cell.edge_neighbour(edge)))
                    .expect("a cell index fits a usize");
                // Exact: any two i32 values and their difference are f64 integers.
                let drop_mm = f64::from(elevation_mm[k]) - f64::from(elevation_mm[next]);
                if drop_mm <= 0.0 {
                    continue;
                }
                let slope = drop_mm / 1e3 / (arc(self.centres[k], self.centres[next]) * r);
                if receivers[k].is_none_or(|(_, _, best)| slope > best) {
                    receivers[k] = Some((next, edge, slope));
                }
            }
        }
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by_key(|&k| (std::cmp::Reverse(elevation_mm[k]), k));
        let mut area: Vec<f64> = self.solid_angles_sr.iter().map(|w| w * r * r).collect();
        for &k in &order {
            if let Some((next, _, _)) = receivers[k] {
                area[next] += area[k];
            }
        }
        (0..n)
            .map(|k| {
                let drainage = LogArea::from_area(SquareMetres::new(area[k]))
                    .expect("a drainage area has a code");
                match receivers[k] {
                    None => Route {
                        flow: FlowDirection::Terminal,
                        drainage,
                        steepness: LogSteepness::ZERO,
                    },
                    Some((_, edge, slope)) => Route {
                        flow: FlowDirection::from(edge),
                        drainage,
                        steepness: LogSteepness::from_index_m0_9(
                            slope * math::powf(area[k], LogSteepness::THETA),
                        )
                        .expect("a steepness index has a code"),
                    },
                }
            })
            .collect()
    }

    /// The area-weighted standard deviation of the elevations and their range, metres.
    fn hypsometry(&self, elevation_mm: &[i32]) -> (Metres, Metres) {
        let heights: Vec<f64> = elevation_mm.iter().map(|&e| f64::from(e) / 1e3).collect();
        let total: f64 = self.solid_angles_sr.iter().sum();
        let mean = heights
            .iter()
            .zip(&self.solid_angles_sr)
            .map(|(h, w)| h * w)
            .sum::<f64>()
            / total;
        let variance = heights
            .iter()
            .zip(&self.solid_angles_sr)
            .map(|(h, w)| (h - mean) * (h - mean) * w)
            .sum::<f64>()
            / total;
        let lowest = elevation_mm.iter().copied().min().unwrap_or(0);
        let highest = elevation_mm.iter().copied().max().unwrap_or(0);
        (
            Metres::new(variance.sqrt()),
            Metres::new((f64::from(highest) - f64::from(lowest)) / 1e3),
        )
    }
}

/// The header's reference temperature, the area-weighted mean of the sea-level temperatures, and
/// the finest temperature and anomaly steps that hold the samples.
fn climate_steps(grid: &Grid, samples: &[ClimateSample], months: u8) -> (Kelvin, u8, u8) {
    let total: f64 = grid.solid_angles_sr.iter().sum();
    let mean = samples
        .iter()
        .zip(&grid.solid_angles_sr)
        .map(|(s, w)| s.sea_level_temperature.value() * w)
        .sum::<f64>()
        / total;
    let largest_departure = samples
        .iter()
        .map(|s| (s.sea_level_temperature.value() - mean).abs())
        .fold(0.0, crate::num::max);
    let largest_anomaly = if months == 1 {
        0.0
    } else {
        samples
            .iter()
            .flat_map(|s| s.month_anomaly_k)
            .map(f64::abs)
            .fold(0.0, crate::num::max)
    };
    (
        Kelvin::new(mean),
        FieldHeader::temperature_step_for(largest_departure)
            .expect("a synthetic climate's temperatures have a step"),
        FieldHeader::anomaly_step_for(largest_anomaly)
            .expect("a synthetic climate's anomalies have a step"),
    )
}

/// `sample` quantised in `header`'s steps; past the header's months, and every anomaly of a
/// one-month year, zero, and every wind past them calm.
fn quantise_climate(header: &FieldHeader, sample: &ClimateSample) -> ClimateCell {
    let months = usize::from(header.months());
    let mut cell = ClimateCell {
        sea_level_temperature: header
            .quantise_sea_level_temperature(sample.sea_level_temperature)
            .expect("the step holds every temperature"),
        month_anomaly: [0; 12],
        month_precipitation: [LogPrecipitation::NONE; 12],
        wind: [Wind::CALM; 12],
    };
    for month in 0..months {
        if months > 1 {
            cell.month_anomaly[month] = header
                .quantise_month_anomaly(sample.month_anomaly_k[month])
                .expect("the step holds every anomaly");
        }
        cell.month_precipitation[month] =
            LogPrecipitation::from_rate(sample.month_precipitation_kg_m2_s[month])
                .expect("a synthetic rate has a code");
        let (speed, azimuth) = sample.wind[month];
        cell.wind[month] =
            Wind::from_velocity(speed, azimuth).expect("a synthetic wind has a code");
    }
    cell
}

/// The solid angle of `cell`, steradians, in closed form: F(u, v) = atan2(uv, √(1 + u² + v²))
/// differenced over the cell's corners in face coordinates (the gnomonic projection's rectangle).
fn solid_angle(cell: PatchKey) -> f64 {
    let side = f64::from(1_u32 << cell.level());
    let (u0, u1) = (
        st_to_uv(f64::from(cell.i()) / side),
        st_to_uv(f64::from(cell.i() + 1) / side),
    );
    let (v0, v1) = (
        st_to_uv(f64::from(cell.j()) / side),
        st_to_uv(f64::from(cell.j() + 1) / side),
    );
    let f = |u: f64, v: f64| math::atan2(u * v, (1.0 + u * u + v * v).sqrt());
    f(u1, v1) - f(u0, v1) - f(u1, v0) + f(u0, v0)
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

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale(a: [f64; 3], k: f64) -> [f64; 3] {
    [a[0] * k, a[1] * k, a[2] * k]
}

fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

fn unit(a: [f64; 3]) -> [f64; 3] {
    scale(a, 1.0 / norm(a))
}

/// The angle between two unit vectors, radians, by atan2 of the cross and dot products, accurate
/// at every angle.
fn arc(a: [f64; 3], b: [f64; 3]) -> f64 {
    math::atan2(norm(cross(a, b)), dot(a, b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::FieldView;
    use hyperion_testkit::float::assert_same_bits;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    /// The cells' closed-form solid angles sum to 4π, 2π ÷ 3 a face.
    #[test]
    fn field_cells_tile_the_sphere() {
        let grid = Grid::new(5);
        let total: f64 = grid.solid_angles_sr.iter().sum();
        assert!(
            (total - 4.0 * core::f64::consts::PI).abs() < 1e-12,
            "4π against {total}"
        );
    }

    /// A field built twice is the same to the bit: its header, every record and every crater.
    fn assert_fields_identical(a: &CoarseField, b: &CoarseField) {
        assert_eq!(a.header(), b.header());
        if let Some(n) = (0..a.synthesis().len()).find(|&n| a.synthesis()[n] != b.synthesis()[n]) {
            panic!("synthesis cell {n} differs");
        }
        assert_eq!(a.climate_layer(), b.climate_layer());
        assert_eq!(a.craters(), b.craters());
        for (x, y) in a.craters().iter().zip(b.craters()) {
            for (p, q) in x.centre.into_iter().zip(y.centre) {
                assert_same_bits(p, q);
            }
            assert_same_bits(x.diameter.value(), y.diameter.value());
            assert_same_bits(x.age.value(), y.age.value());
        }
        let (h, k) = (a.header().parts(), b.header().parts());
        for (p, q) in [
            (h.realised_sigma_h.value(), k.realised_sigma_h.value()),
            (h.realised_relief.value(), k.realised_relief.value()),
            (h.sea_level.value(), k.sea_level.value()),
            (
                h.reference_temperature.value(),
                k.reference_temperature.value(),
            ),
        ] {
            assert_same_bits(p, q);
        }
        assert_eq!(a, b);
    }

    /// Every synthetic world is built twice with identical values, and each is the world its name
    /// says: its level, its sea, its plates or lid, its craters.
    #[test]
    fn every_synthetic_field_is_built_twice_alike() {
        for &world in SyntheticWorld::ALL {
            let first = synthetic_field(world);
            let second = synthetic_field(world);
            assert_fields_identical(&first, &second);
        }
    }

    fn ocean_fraction(field: &CoarseField) -> f64 {
        let grid = Grid::new(field.header().level().get());
        let total: f64 = grid.solid_angles_sr.iter().sum();
        let wet: f64 = field
            .synthesis()
            .iter()
            .zip(&grid.solid_angles_sr)
            .filter(|(c, _)| c.is_under_water())
            .map(|(_, w)| w)
            .sum();
        wet / total
    }

    #[test]
    fn the_synthetic_fields_are_the_worlds_they_name() {
        let earth = synthetic_field(SyntheticWorld::EarthLike);
        assert_eq!(earth.header().level().get(), 8);
        let ocean = ocean_fraction(&earth);
        assert!((0.4..0.85).contains(&ocean), "Earth-like ocean {ocean}");
        for kind in [
            BoundaryKind::Subduction,
            BoundaryKind::Collision,
            BoundaryKind::Divergent,
            BoundaryKind::Transform,
        ] {
            assert!(
                earth.synthesis().iter().any(|c| c.boundary == kind),
                "the Earth-like world has no {kind:?} boundary"
            );
        }
        let sigma = earth.header().realised_sigma_h().value();
        assert!(
            (1_500.0..4_000.0).contains(&sigma),
            "Earth-like σ_h {sigma} m"
        );
        assert!(earth.synthesis().iter().any(|c| c.ice == 255));
        assert_eq!(earth.header().months(), 12);
        assert_same_bits(earth.header().season_eccentricity(), 0.016_711_23);
        // Its winds turn with the rain band: the trades between the band's places in the year's
        // first and seventh months blow from opposite hemispheres.
        assert!(earth.climate_layer().iter().any(|c| c.wind[0] != c.wind[6]));

        let mars = synthetic_field(SyntheticWorld::MarsLike);
        assert_eq!(mars.header().level().get(), 7);
        assert!((ocean_fraction(&mars)).abs() < 1e-12);
        assert!(
            mars.synthesis()
                .iter()
                .all(|c| c.boundary == BoundaryKind::Absent)
        );
        assert!(mars.synthesis().iter().any(|c| c.crust == Crust::Province));
        assert!(mars.craters().len() >= 20);

        let moon = synthetic_field(SyntheticWorld::MoonLike);
        assert_eq!(moon.header().level().get(), 6);
        assert!(moon.craters().len() >= 40);
        assert!(
            moon.synthesis()
                .iter()
                .all(|c| c.flow == FlowDirection::Terminal)
        );
        assert!(
            moon.craters()
                .iter()
                .any(|c| c.morphology == Morphology::MultiRingBasin)
        );

        let ceres = synthetic_field(SyntheticWorld::CeresLike);
        assert_eq!(ceres.header().level().get(), 5);
        assert!(ceres.craters().len() >= 25);

        let flat = synthetic_field(SyntheticWorld::Flat);
        assert!(flat.synthesis().iter().all(|c| c.elevation_mm == 0));
        assert!(flat.craters().is_empty());
        assert_eq!(flat.header().months(), 1);

        let one = synthetic_field(SyntheticWorld::OneCrater);
        assert_eq!(one.craters().len(), 1);
        let faces: std::collections::BTreeSet<u8> = one.craters()[0]
            .reach
            .cells()
            .map(|n| cell_at_index(6, n).unwrap().face().index())
            .collect();
        assert!(faces.len() >= 2, "the one crater's reach spans {faces:?}");
    }
}
