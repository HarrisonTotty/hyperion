//! Impacts on a body: `body.impact` (plan 14, P14.T31.a; design note 15), the events of the
//! projectiles that strike it during play.
//!
//! # The rate
//!
//! The rate of impacts of projectiles of diameter at least D is the belts' number flux at the body
//! times its gravitationally focused cross-section. P14.T48.f's closed form gives that product per
//! unit area over the Moon's, `R_b` ([`Impactors::rate`]): the projectiles' abundance over the
//! Moon's times the encounter factor v∞ (1 + `v_esc`² ÷ v∞²), the focusing included, over the
//! Moon's. So, over the whole body of radius R,
//!
//! > Ṅ(> D) = Φ☾ × B(A) × `R_b` × 4πR² × S(D),
//!
//! - Φ☾ = [`LUNAR_FLUX_PER_KM2_YEAR`], 2.3 × 10⁻¹⁵ km⁻² yr⁻¹, the Moon's present rate of impacts
//!   of projectiles over 1 km, the anchor `R_b` was calibrated against (science-p14-t24b-laws
//!   §2.2: Earth's interval of about 0.6 Myr between impacts of 1 km bodies, Stuart and Binzel
//!   2004, Icarus 170, 295, over Earth's `R_b` of 1.42);
//! - B(A) = [`bombardment`], the lunar chronology's rate at the system's age over the Solar
//!   System's present rate, 1 at 4.567 Gyr: `R_b` holds the belts at a fixed age, and the
//!   chronology carries the time profile, as it does for the crater contract (P14.T48.c);
//! - S(D) = [`SizeLaw::share_above`], the projectiles' size law by their composition
//!   (decision-p14-t31a-events, amendments (a) and (b); [`size_law_of`]):
//!   - rocky ones, from a rocky belt: [`ROCKY_IMPACTOR_SLOPE`] = 2.25 normalised at 1 km, the slope
//!     through Φ☾'s own anchor and Earth's rate above 1 Mt (Brown et al. 2002, Nature 420, 294),
//!     up to the collisional cascade's top, 450 km ([`rocky_projectiles`]);
//!   - icy ones, from the comets or an icy belt: [`NUCLEUS_SIZE_SLOPE`] = 1.92 normalised at
//!     [`ICY_REFERENCE_DIAMETER`] = 1.5 km, the size `R_b`'s comet calibration counts
//!     (science-p14-t24b-laws §2.7: Jupiter's and Pluto's rates over 1.5 km against the Moon's
//!     over 1 km), up to [`LARGEST_NUCLEUS`] ([`icy_projectiles`]).
//!
//!   P14.T21.c's size slope of each belt, 2.5–3.5, describes its cascade above a kilometre, and
//!   carried down to the stated energy's tens of metres gave an Earth a 1 Mt impact every 1.2–52
//!   years against about 135 observed: impacts do not read it.
//!
//! # Energy
//!
//! A projectile of diameter D and density ρ (P14.T48.f's
//! [`Impactors::projectile_density`]) striking at the body's mean impact velocity v
//! ([`Impactors::velocity`], the escape speed included) brings E = ½ m v² = π ρ D³ v² ÷ 12. The
//! rate above an energy is the rate above that energy's diameter, so each event's energy comes
//! from the size law through its diameter. The velocity's spread is not drawn (P14.T48.f takes the
//! mean).
//!
//! # The floor
//!
//! An impact is an event above the body's floor, [`ImpactModel::floor_joules`]: the larger of
//! [`FIXED_FLOOR_JOULES`], the plan's stated energy (1 Mt of TNT), and the energy at which Ṅ is
//! [`PoissonBins::MAX_MEAN_PER_BIN`] a bin, so that a giant struck often lists its largest impacts
//! and no bin overflows (plan 06's flares' rule). The rate is normalised at the law's reference,
//! never at the floor, so a raised floor removes small impacts and leaves the large ones as they
//! were.
//!
//! # Time
//!
//! Poisson bins of [`BIN_SECONDS`] = one Julian year, looking back [`LOOK_BACK_BINS`] = none: an
//! impact is an instant. The rate is constant: its inputs are read once, at the epoch, so a bin's
//! bound is the rate and nothing is thinned.
//!
//! # Marks
//!
//! From each impact's own stream, [`BinEvent::marks`], at word 0: its projectile's diameter, one
//! word ([`SizeLaw`]'s density from the floor up); then the place it strikes, uniform over the
//! sphere, words 1 and 2 ([`SurfacePoint`]); words 3–7 are reserved (an angle, a velocity's
//! spread). Marks never move a time.

use core::f64::consts::PI;
use std::error::Error;
use std::fmt;

use hyperion_surface::craters::{SOLAR_SYSTEM_AGE, lunar_chronology};

use super::comets::{LARGEST_NUCLEUS, NUCLEUS_SIZE_SLOPE};
use super::{SizeLaw, SurfacePoint, expected_count};
use crate::Seed;
use crate::events::{
    BinEvent, EventId, EventSeries, EventsPerSecond, PoissonBins, RateModel, TimeWindow,
};
use crate::id::BodyId;
use crate::id::event_tags::BODY_IMPACT;
use crate::math;
use crate::planetary::belts::{BeltComposition, CASCADE_TOP_DIAMETER};
use crate::planetary::hooks::figures::{ImpactorSource, Impactors};
use crate::rng::PowerLaw;
use crate::time::UniverseTime;
use crate::units::{Gigayears, KilogramsPerCubicMetre, Metres, MetresPerSecond};

/// The length of an impact bin, s: one Julian year, 31,557,600 s.
pub const BIN_SECONDS: i64 = 31_557_600;

/// How many whole bins an impact's `active_at` would look back: none, since an impact is an
/// instant.
pub const LOOK_BACK_BINS: u32 = 0;

/// The plan's stated energy, J: 4.184 × 10¹⁵, one megaton of TNT (the megaton's definition,
/// 4.184 × 10¹⁵ J). A builder's choice, recorded in plan 14's Risks: about the smallest impact a
/// ship would report, a little above the airburst over Chelyabinsk in 2013, about 0.5 Mt (Brown et
/// al. 2013, Nature 503, 238). Part of the generator version.
pub const FIXED_FLOOR_JOULES: f64 = 4.184e15;

/// Φ☾, the Moon's present rate of impacts of projectiles over 1 km per square kilometre of its
/// surface, per Julian year: 2.3 × 10⁻¹⁵ (science-p14-t24b-laws §2.2, the anchor of P14.T48.f's
/// `R_b`).
///
/// It is Earth's interval of about 0.6 Myr between impacts of bodies over 1 km (Stuart and Binzel
/// 2004, Icarus 170, 295), 3.3 × 10⁻¹⁵ km⁻² yr⁻¹ over Earth's 5.1 × 10⁸ km², carried to the Moon by
/// Earth's `R_b` of 1.42 (P14.T48.f). The ruling records it as low in confidence, by a factor of
/// about 2. Part of the generator version.
pub const LUNAR_FLUX_PER_KM2_YEAR: f64 = 2.3e-15;

/// The span over which [`bombardment`] takes the lunar chronology's mean rate: 1 Myr (10⁻³ Gyr),
/// over which the chronology's exponential term changes by e^(6.93 × 10⁻³), 0.7%. Part of the
/// generator version.
pub const BOMBARDMENT_SPAN: Gigayears = Gigayears::new(1e-3);

/// The slope q of rocky projectiles' size law, N(> D) ∝ D^(−q): 2.25 (decision-p14-t31a-events,
/// amendment (a)). Part of the generator version.
///
/// It is the slope through two rates at an Earth struck at 19.87 km/s by projectiles of 3,000 kg
/// m⁻³ (P14.T48.f's), whose 1 Mt brings a 23.8 m body: Φ☾'s own anchor, one impact over 1 km in
/// 0.6 Myr (Stuart and Binzel 2004, Icarus 170, 295), and Brown et al.'s (2002, Nature 420, 294)
/// N(> E) = 3.7 E^(−0.90) a year with E in kt, once in 135 years above 1 Mt (its abstract has
/// Tunguska's 10 Mt about once in 1,000 years), which give 2.246. The near-Earth asteroids' real
/// distribution is wavy (Harris and D'Abramo 2015, Icarus 257, 302), and Brown et al. (2013,
/// Nature 503, 238) find the flux of bodies tens of metres across may be up to an order of
/// magnitude higher than other estimates.
pub const ROCKY_IMPACTOR_SLOPE: f64 = 2.25;

/// The diameter rocky projectiles' size law is normalised at: 1 km, the size Φ☾ counts.
pub const ROCKY_REFERENCE_DIAMETER: Metres = Metres::new(1e3);

/// The diameter icy projectiles' size law is normalised at: 1.5 km (decision-p14-t31a-events,
/// amendment (b)). `R_b`'s comet sources are calibrated on comets over 1.5 km against the Moon's
/// impactors over 1 km (science-p14-t24b-laws §2.7: Jupiter's 0.005 a year over 1.5 km, Zahnle et
/// al. 2003, Icarus 163, 263, and Pluto's), so Φ☾ × `R_b` is an icy source's rate over 1.5 km.
/// Part of the generator version.
pub const ICY_REFERENCE_DIAMETER: Metres = Metres::new(1.5e3);

/// [`BIN_SECONDS`] as a float, exactly.
const BIN_SECONDS_F64: f64 = 31_557_600.0;

/// Square metres in a square kilometre.
const SQUARE_METRES_PER_KM2: f64 = 1e6;

/// The impact bins: a Julian year long, looking back none.
#[must_use]
fn bins() -> PoissonBins {
    PoissonBins::new(BIN_SECONDS, LOOK_BACK_BINS).expect("a year lies between 16 s and 2^44 s")
}

/// B(A), how much more often a system of age `system_age` is struck now than the Solar System:
/// the lunar chronology's mean rate over the [`BOMBARDMENT_SPAN`] after the age, over the same
/// after the Solar System's 4.567 Gyr ([`SOLAR_SYSTEM_AGE`]), so exactly 1 there.
///
/// The chronology is Neukum, Ivanov and Hartmann's (2001) counted from each system's own
/// formation ([`lunar_chronology`]): its exponential term, the early bombardment, makes a young
/// system's rate far higher (about 1.2 × 10⁴ at 0.1 Gyr and 25 at 1 Gyr), and its linear term
/// holds an older one at about 1.
///
/// # Panics
///
/// If the age is not finite and non-negative.
///
/// # Examples
///
/// ```
/// use hyperion_sim::planetary::events::impacts::bombardment;
/// use hyperion_sim::units::Gigayears;
///
/// assert!((bombardment(Gigayears::new(4.567)) - 1.0).abs() < 1e-15);
/// assert!(bombardment(Gigayears::new(1.0)) > 20.0);
/// assert!((bombardment(Gigayears::new(10.0)) - 1.0).abs() < 1e-3);
/// ```
#[must_use]
pub fn bombardment(system_age: Gigayears) -> f64 {
    let age = system_age.value();
    assert!(
        age.is_finite() && age >= 0.0,
        "a system's age must be finite and non-negative, got {age} Gyr"
    );
    let span = BOMBARDMENT_SPAN;
    let after = |age: f64| lunar_chronology(span, Gigayears::new(age + span.value())).value();
    after(age) / after(SOLAR_SYSTEM_AGE.value())
}

/// Rocky projectiles' size law: [`ROCKY_IMPACTOR_SLOPE`] normalised at
/// [`ROCKY_REFERENCE_DIAMETER`], up to the top of a belt's collisional cascade,
/// [`CASCADE_TOP_DIAMETER`] (450 km; P14.T48.f's sources leave the named members out too).
///
/// # Panics
///
/// Never: the constants are in range.
#[must_use]
pub fn rocky_projectiles() -> SizeLaw {
    SizeLaw::new(
        ROCKY_IMPACTOR_SLOPE,
        ROCKY_REFERENCE_DIAMETER,
        Metres::new(CASCADE_TOP_DIAMETER * 1e3),
    )
    .expect("the rocky law's constants are in range")
}

/// Icy projectiles' size law: the nuclei's slope, [`NUCLEUS_SIZE_SLOPE`] (Snodgrass et al. 2011),
/// normalised at [`ICY_REFERENCE_DIAMETER`], up to [`LARGEST_NUCLEUS`].
///
/// # Panics
///
/// Never: the constants are in range.
#[must_use]
pub fn icy_projectiles() -> SizeLaw {
    SizeLaw::new(NUCLEUS_SIZE_SLOPE, ICY_REFERENCE_DIAMETER, LARGEST_NUCLEUS)
        .expect("the icy law's constants are in range")
}

/// The size law of the projectiles `source` sends, by their composition: rocky from a rocky belt
/// ([`rocky_projectiles`]); icy from an icy belt, the giants' comets or the inner comets
/// ([`icy_projectiles`]).
#[must_use]
pub fn size_law_of(source: ImpactorSource) -> SizeLaw {
    match source {
        ImpactorSource::InBelt { composition, .. }
        | ImpactorSource::NearestBelt { composition, .. } => match composition {
            BeltComposition::Rocky => rocky_projectiles(),
            BeltComposition::Icy => icy_projectiles(),
        },
        ImpactorSource::Comets | ImpactorSource::InnerComets { .. } => icy_projectiles(),
    }
}

/// What an [`ImpactModel`] reads of a body: its radius, its impactors' rate, velocity and density
/// (P14.T48.f), their size law and its system's age.
///
/// Plain data with public fields: [`ImpactModel::new`] validates them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImpactInputs {
    /// The body's mean radius, m, finite and positive: its bulk section's.
    pub radius: Metres,
    /// `R_b`, its impact rate per unit area over the Moon's, finite and non-negative
    /// ([`Impactors::rate`]).
    pub rate_over_moon: f64,
    /// Its impactors' mean impact velocity, m/s, finite and positive ([`Impactors::velocity`]).
    pub velocity: MetresPerSecond,
    /// Its projectiles' density, kg m⁻³, finite and positive
    /// ([`Impactors::projectile_density`]).
    pub projectile_density: KilogramsPerCubicMetre,
    /// Its projectiles' size law, by their composition ([`size_law_of`]).
    pub size_law: SizeLaw,
    /// Its system's age at the epoch, Gyr, finite and non-negative ([`bombardment`]).
    pub system_age: Gigayears,
}

impl ImpactInputs {
    /// The inputs of a body of mean radius `radius` struck by `impactors`, in a system
    /// `system_age` old at the epoch: the projectiles' size law by their source's composition
    /// ([`size_law_of`]).
    #[must_use]
    pub fn new(radius: Metres, impactors: &Impactors, system_age: Gigayears) -> Self {
        Self {
            radius,
            rate_over_moon: impactors.rate(),
            velocity: impactors.velocity(),
            projectile_density: impactors.projectile_density(),
            size_law: size_law_of(impactors.source()),
            system_age,
        }
    }
}

/// An [`ImpactModel`] could not be built, with the value refused.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildImpactModelError {
    /// The radius is not finite and positive.
    Radius(Metres),
    /// `R_b` is not finite and non-negative.
    RateOverMoon(f64),
    /// The impact velocity is not finite and positive.
    Velocity(MetresPerSecond),
    /// The projectiles' density is not finite and positive.
    ProjectileDensity(KilogramsPerCubicMetre),
    /// The system's age is not finite and non-negative.
    SystemAge(Gigayears),
}

impl fmt::Display for BuildImpactModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Radius(r) => write!(f, "radius {} m is not finite and positive", r.value()),
            Self::RateOverMoon(rate) => {
                write!(
                    f,
                    "impact rate {rate} over the moon's is not finite and non-negative"
                )
            }
            Self::Velocity(v) => {
                write!(
                    f,
                    "impact velocity {} m/s is not finite and positive",
                    v.value()
                )
            }
            Self::ProjectileDensity(rho) => write!(
                f,
                "projectile density {} kg m⁻³ is not finite and positive",
                rho.value()
            ),
            Self::SystemAge(age) => write!(
                f,
                "system age {} Gyr is not finite and non-negative",
                age.value()
            ),
        }
    }
}

impl Error for BuildImpactModelError {}

/// The impacts on one body: its series, its inputs, its floor and its constant rate above the
/// floor (see the [module documentation](self)).
///
/// It is the [`RateModel`] of the body's `body.impact` series, whose bound and rate are both
/// [`listed_rate`](Self::listed_rate) at every instant.
///
/// # Examples
///
/// A Jupiter-like giant struck by comets, its rate per unit area 42 times the Moon's:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::id::{BodyId, SystemId};
/// use hyperion_sim::planetary::events::impacts::{FIXED_FLOOR_JOULES, icy_projectiles};
/// use hyperion_sim::planetary::events::{ImpactInputs, ImpactModel};
/// use hyperion_sim::units::{Gigayears, KilogramsPerCubicMetre, Metres, MetresPerSecond};
///
/// let jupiter = BodyId::new(SystemId::from_raw(0x0200_0800_2000_0000)?, 0x0105);
/// let inputs = ImpactInputs {
///     radius: Metres::new(6.9911e7),
///     rate_over_moon: 42.0,
///     velocity: MetresPerSecond::new(60_200.0),
///     projectile_density: KilogramsPerCubicMetre::new(500.0),
///     size_law: icy_projectiles(),
///     system_age: Gigayears::new(4.567),
/// };
/// let model = ImpactModel::new(Seed::new(7), jupiter, &inputs)?;
/// let per_year = |rate: f64| rate * 31_557_600.0;
/// // A comet over 1.5 km, a Shoemaker–Levy 9, every century or two,
/// let sl9 = per_year(model.rate_above(Metres::new(1_500.0)).value());
/// assert!((1.0 / 500.0..1.0 / 50.0).contains(&sl9));
/// // and some twenty impacts a year above a megaton.
/// let megaton = per_year(model.rate_above_joules(FIXED_FLOOR_JOULES).value());
/// assert!((10.0..65.0).contains(&megaton));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImpactModel {
    series: EventSeries,
    inputs: ImpactInputs,
    bombardment: f64,
    reference_rate: EventsPerSecond,
    floor: Metres,
    draws: Option<PowerLaw>,
    rate: EventsPerSecond,
}

impl ImpactModel {
    /// The impacts on `body` in the universe of `seed`, from `inputs`: the series of
    /// [`BODY_IMPACT`] with the body's ID as subject.
    ///
    /// # Errors
    ///
    /// The [`BuildImpactModelError`] of the first input out of range, in the field order.
    pub fn new(
        seed: Seed,
        body: BodyId,
        inputs: &ImpactInputs,
    ) -> Result<Self, BuildImpactModelError> {
        let positive = |x: f64| x.is_finite() && x > 0.0;
        let non_negative = |x: f64| x.is_finite() && x >= 0.0;
        if !positive(inputs.radius.value()) {
            return Err(BuildImpactModelError::Radius(inputs.radius));
        }
        if !non_negative(inputs.rate_over_moon) {
            return Err(BuildImpactModelError::RateOverMoon(inputs.rate_over_moon));
        }
        if !positive(inputs.velocity.value()) {
            return Err(BuildImpactModelError::Velocity(inputs.velocity));
        }
        if !positive(inputs.projectile_density.value()) {
            return Err(BuildImpactModelError::ProjectileDensity(
                inputs.projectile_density,
            ));
        }
        if !non_negative(inputs.system_age.value()) {
            return Err(BuildImpactModelError::SystemAge(inputs.system_age));
        }
        let bombardment = bombardment(inputs.system_age);
        let radius = inputs.radius.value();
        let area_km2 = 4.0 * PI * radius * radius / SQUARE_METRES_PER_KM2;
        let per_year = LUNAR_FLUX_PER_KM2_YEAR * bombardment * inputs.rate_over_moon * area_km2;
        let reference_rate = EventsPerSecond::per_julian_year(per_year);
        let mut model = Self {
            series: EventSeries::new(seed, BODY_IMPACT, body.into()),
            inputs: *inputs,
            bombardment,
            reference_rate,
            floor: inputs.size_law.largest(),
            draws: None,
            rate: EventsPerSecond::ZERO,
        };
        // The diameter at which a bin's mean reaches the bins' limit; an infinite share (no
        // impacts at all) puts it at zero, under the fixed floor.
        let capped = inputs.size_law.diameter_at_share(
            PoissonBins::MAX_MEAN_PER_BIN / (reference_rate.value() * BIN_SECONDS_F64),
        );
        let fixed = model.diameter_of(FIXED_FLOOR_JOULES);
        let floor = if capped > fixed { capped } else { fixed };
        // The one error a positive floor can give is a floor at or above the law's largest
        // diameter: then no projectile brings the stated energy, and the model, as built above,
        // lists none.
        if let Ok(draws) = inputs.size_law.draws_above(floor) {
            model.floor = floor;
            model.draws = Some(draws);
            // Clamped at the bins' limit, which the floor already meets to rounding.
            model.rate = EventsPerSecond::new(
                (reference_rate.value() * inputs.size_law.share_above(floor))
                    .min(PoissonBins::MAX_MEAN_PER_BIN / BIN_SECONDS_F64),
            );
        }
        Ok(model)
    }

    /// The series: the body's subject, [`BODY_IMPACT`] and their event key.
    #[must_use]
    pub const fn series(&self) -> &EventSeries {
        &self.series
    }

    /// The inputs it was built from.
    #[must_use]
    pub const fn inputs(&self) -> &ImpactInputs {
        &self.inputs
    }

    /// B(A), the system's [`bombardment`] factor.
    #[must_use]
    pub const fn bombardment(&self) -> f64 {
        self.bombardment
    }

    /// Ṅ(> `D_ref`), the rate of impacts of projectiles over the size law's reference diameter
    /// (1 km rocky, 1.5 km icy), events per second: Φ☾ × B × `R_b` × 4πR².
    #[must_use]
    pub const fn reference_rate(&self) -> EventsPerSecond {
        self.reference_rate
    }

    /// The smallest projectile listed, m: the diameter of the floor's energy, or the size law's
    /// largest diameter for a body with no impact above the floor.
    #[must_use]
    pub const fn floor_diameter(&self) -> Metres {
        self.floor
    }

    /// The smallest energy listed, J: the larger of [`FIXED_FLOOR_JOULES`] and the energy at which
    /// a bin's mean reaches [`PoissonBins::MAX_MEAN_PER_BIN`].
    #[must_use]
    pub fn floor_joules(&self) -> f64 {
        self.energy_of(self.floor)
    }

    /// The rate of the impacts listed, those above the floor, events per second: constant.
    #[must_use]
    pub const fn listed_rate(&self) -> EventsPerSecond {
        self.rate
    }

    /// Ṅ(> D), the rate of impacts of projectiles over `diameter`, events per second, whatever the
    /// floor: the law the listing follows above it.
    ///
    /// # Panics
    ///
    /// In debug builds, unless the diameter is positive ([`SizeLaw::share_above`]).
    #[must_use]
    pub fn rate_above(&self, diameter: Metres) -> EventsPerSecond {
        EventsPerSecond::new(
            self.reference_rate.value() * self.inputs.size_law.share_above(diameter),
        )
    }

    /// The rate of impacts bringing at least `joules`, events per second:
    /// [`rate_above`](Self::rate_above) at that energy's diameter.
    ///
    /// # Panics
    ///
    /// In debug builds, unless the energy is positive.
    #[must_use]
    pub fn rate_above_joules(&self, joules: f64) -> EventsPerSecond {
        self.rate_above(self.diameter_of(joules))
    }

    /// The energy a projectile of `diameter` brings at the body's impact velocity, J:
    /// π ρ D³ v² ÷ 12.
    #[must_use]
    pub fn energy_of(&self, diameter: Metres) -> f64 {
        let (d, v) = (diameter.value(), self.inputs.velocity.value());
        PI * self.inputs.projectile_density.value() * d * d * d * v * v / 12.0
    }

    /// The diameter of the projectile that brings `joules` at the body's impact velocity, m: the
    /// inverse of [`energy_of`](Self::energy_of), ∛(12 E ÷ (π ρ v²)).
    #[must_use]
    pub fn diameter_of(&self, joules: f64) -> Metres {
        let v = self.inputs.velocity.value();
        Metres::new(math::cbrt(
            12.0 * joules / (PI * self.inputs.projectile_density.value() * v * v),
        ))
    }

    /// The number of impacts expected in `window` above the floor: the rate times the window's
    /// span, not a draw.
    #[must_use]
    pub fn expected_count(&self, window: TimeWindow) -> f64 {
        expected_count(self.rate, window)
    }
}

impl RateModel for ImpactModel {
    fn bound(&self, _bin: TimeWindow) -> EventsPerSecond {
        self.rate
    }

    fn rate(&self, _t: UniverseTime) -> EventsPerSecond {
        self.rate
    }
}

/// One impact on a body: its ID, its instant, its projectile's diameter, its energy, its velocity
/// and where it strikes (see the [module documentation](self)).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Impact {
    id: EventId,
    time: UniverseTime,
    diameter: Metres,
    energy_joules: f64,
    velocity: MetresPerSecond,
    site: SurfacePoint,
}

impl Impact {
    /// The impact's ID, from which [`impact`] regenerates it.
    #[must_use]
    pub const fn id(&self) -> EventId {
        self.id
    }

    /// The instant of the impact.
    #[must_use]
    pub const fn time(&self) -> UniverseTime {
        self.time
    }

    /// The projectile's diameter, m, from the floor's up to the size law's largest.
    #[must_use]
    pub const fn diameter(&self) -> Metres {
        self.diameter
    }

    /// The energy it brings, J: at least the model's floor.
    #[must_use]
    pub const fn energy_joules(&self) -> f64 {
        self.energy_joules
    }

    /// The impact velocity, m/s: the body's mean ([`ImpactInputs::velocity`]).
    #[must_use]
    pub const fn velocity(&self) -> MetresPerSecond {
        self.velocity
    }

    /// Where it strikes, uniform over the body's sphere.
    #[must_use]
    pub const fn site(&self) -> SurfacePoint {
        self.site
    }
}

/// The impact of `event`, its marks drawn from its own stream.
#[must_use]
fn impact_of(model: &ImpactModel, event: BinEvent) -> Impact {
    let draws = model
        .draws
        .as_ref()
        .expect("a model lists impacts only when it has a size law above its floor");
    let (id, time) = (event.id(), event.time());
    let mut marks = event.into_marks();
    let diameter = Metres::new(marks.power_law(draws));
    let site = SurfacePoint::uniform(&mut marks);
    Impact {
        id,
        time,
        diameter,
        energy_joules: model.energy_of(diameter),
        velocity: model.inputs.velocity,
        site,
    }
}

/// Appends to `out` the body's impacts in `window`, sorted by time and then by ID.
///
/// Every bin the window touches is generated whole, so a listing over a window is the union of the
/// listings over any partition of it, in any order of asking. `out` is only appended to.
pub fn impacts(model: &ImpactModel, window: TimeWindow, out: &mut Vec<Impact>) {
    let mut events = Vec::new();
    bins().events_in(model.series(), window, model, &mut events);
    out.extend(events.into_iter().map(|event| impact_of(model, event)));
}

/// The impact `id` names, regenerated on its own.
///
/// `None` if `id` is not of the model's series (another event tag or another subject) or names a
/// number beyond its bin's count.
#[must_use]
pub fn impact(model: &ImpactModel, id: EventId) -> Option<Impact> {
    bins()
        .event(model.series(), id, model)
        .map(|event| impact_of(model, event))
}

#[cfg(test)]
mod tests;
