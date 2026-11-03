//! The hypervelocity survivors' weight and flights (plan 09, P09.T34.b; plan 08, Design note 25).
//!
//! The donors that double detonations fling out (ruling 128.1) are plan 08's hypervelocity class,
//! registered in layer D at zero weight. From generator version 27 the class has its weight.
//! A survivor launched τ ago at `v` is inside the cube while τ < `path ÷ v`. It is the class's,
//! and not one of the Type Ia class's entries (P09.T34.a's member 1), while τ is at least S, the
//! latest start that class's interval can have: [`SHELL_WINDOW_CAP`] + L + H before the epoch,
//! 4.26 Myr. A galaxy's own start, its gas's cap + L + H (3.85 Myr in the Milky Way fixture), lies
//! within it, so no survivor is both an entry and the class's. Those launched between the two,
//! a few per cent, are neither, so that the class, as every other part of the fields, does not
//! depend on the seed through its gas's drawn pressure floor (lean, 2026-11-01). At the galaxy's
//! own Type Ia rate, steady over the survivors' 10–20 Myr in the cube, population i of
//! [`SURVIVOR_POPULATIONS`] then holds `rate × shareᵢ × (path ÷ v_eff,ᵢ − S)`,
//! with `path` the form table's mean path from launch to the cube's face (P15.T6.f) and `v_eff,ᵢ`
//! the population's `1 ÷ ⟨1 ÷ v⟩`. That is ruling 128.1's weight less the interval. The rate is
//! the whole galaxy's ([`DelayTimeDistribution::from_galaxy`]), whose every Type Ia leaves its
//! share of survivors.
//!
//! A survivor's marks are drawn on plan 08's tags, so the class adds none, each from word 0 and
//! keyed by its ID:
//!
//! - its population, on `displaced.origin_bin`, by the populations' counts;
//! - its launch speed in that population's band, on `runaway.speed`, with density ∝ `path ÷ v − S`,
//!   by bisection;
//! - its time since launch, on `runaway.ejection`, uniform on `[S, path ÷ v]`.
//!
//! From them follow the survivor's star and its line:
//!
//! - **The donor.** A survivor is its double detonation's donor, flung out at its orbital speed at
//!   Roche-lobe contact beside an accretor of [`ACCRETOR_MASS`] ([`contact_speed`], by Eggleton's
//!   lobe and Nauenberg's cold radius), so its mass is its speed's inverse, [`donor_mass`]: about
//!   0.15–0.28 M☉ in the slow band, a helium white dwarf, and 0.87–1.0 M☉ in the fast, a C/O white
//!   dwarf ([`DonorKind`]). Placement gives each its star (`placement::marks`).
//! - **The launch.** It flies in a straight line from its launch site at its speed. The site is a
//!   draw of the galaxy's Type Ia rate density, [`Survivors::launches_per_year_per_cubic_ly`], on the sphere of
//!   radius vτ about its place ([`Survivors::launch_direction`]), on the record's
//!   `system.velocity` stream. P15.T6's production form convolves the same density.
//!
//! The survivors outrun plan 03's pad, so from the same version layer D pads at
//! `UNBOUND_PAD_SPEED` ([`pad_speed`](crate::galaxy::query::pad_speed)).

use super::class_table::{FormTable, SURVIVOR_POPULATIONS, SurvivorPopulation};
use crate::galaxy::catalogue_classes::supernova::SupernovaInterval;
use crate::galaxy::catalogue_classes::type_ia::DelayTimeDistribution;
use crate::galaxy::consts::LIGHT_YEARS_PER_YEAR_PER_KM_S;
use crate::galaxy::imf::MASS_BAND_EDGES;
use crate::galaxy::snr::SHELL_WINDOW_CAP;
use crate::galaxy::{Galaxy, PointLy};
use crate::id::SystemId;
use crate::math;
use crate::rng::{ObjectKey, Seed, Stream, Threshold, tags};
use crate::units::consts::{GM_SUN, SOLAR_RADIUS_M};
use crate::units::{KilometresPerSecond, SolarMasses, Years};

/// Bisection steps of a survivor's speed quantile, 2⁻⁴⁸ of its band, and of its donor's mass.
const SPEED_BISECTIONS: u32 = 48;

/// The accretor whose detonation flings the donor out: a 1.0 M☉ C/O white dwarf, the mass whose
/// bare detonation gives a normal Type Ia's nickel (Shen et al. 2018, ApJ 854, 52) (lean,
/// 2026-11-01).
pub const ACCRETOR_MASS: SolarMasses = SolarMasses::new(1.0);

/// The lightest donor: a 0.15 M☉ helium white dwarf, about the least that degenerate helium cores
/// make (the extremely low-mass white dwarfs' 0.15–0.3 M☉; Brown et al. 2016, ApJ 818, 155). A
/// survivor slower than its contact speed, 1,290 km/s, takes it (lean, 2026-11-01).
pub const LIGHTEST_DONOR: SolarMasses = SolarMasses::new(0.15);

/// The heaviest helium white dwarf the class's donors are: 0.30 M☉, under the lightest naked helium
/// star that burns helium in the stellar stage, 0.31–0.35 M☉ over its metallicities (the core at
/// an `M_HeF` star's helium ignition, `Track::helium_star`), which builds it. Nature's helium
/// white dwarfs reach about 0.45 M☉, under the degenerate core at the helium flash (Hurley, Pols
/// and Tout 2000, MNRAS 315, 543); a heavier donor here is a C/O white dwarf. The bands' donors
/// lie on either side of both, 0.15–0.28 M☉ and 0.87–1.0 M☉.
pub const HEAVIEST_HELIUM_WHITE_DWARF: SolarMasses = SolarMasses::new(0.30);

/// Nauenberg's (1972, ApJ 175, 417) mass M₃ of a cold white dwarf's radius law: 5.816 ÷ μₑ² M☉ at
/// μₑ = 2, 1.454 M☉.
const NAUENBERG_MASS: SolarMasses = SolarMasses::new(5.816 / 4.0);

/// Nauenberg's radius scale, R☉: R = 0.0112 R☉ [(M ÷ M₃)^−2/3 − (M ÷ M₃)^2/3]^1/2.
const NAUENBERG_RADIUS_SOLAR: f64 = 0.0112;

/// The golden angle, rad, π (3 − √5): the step in azimuth of a Fibonacci lattice on the sphere.
const GOLDEN_ANGLE: f64 = 2.399_963_229_728_653;

/// The bound's directions on the sphere of a survivor's launch sites, a Fibonacci lattice.
const BOUND_PROBES: u32 = 256;

/// The bound's margin over the largest ratio of the launch density to the proposal's at its
/// probes, for the ratio between them.
const BOUND_MARGIN: f64 = 3.0;

/// The shares of a launch's proposal, uniform on the sphere, a von Mises–Fisher lobe about the
/// probes' densest direction and a band about the directions whose sites lie in the plane, where
/// the sphere reaches it; and without the band where it does not.
const PROPOSAL_SHARES: [[f64; 3]; 2] = [[0.2, 0.4, 0.4], [0.25, 0.75, 0.0]];

/// The angles from the lobe's axis, rad, at which the density's fall to 1 ÷ e of the axis's is
/// sought, doubling from 10⁻⁴: the lobe's width, so that its concentration κ = 2 ÷ θₑ² matches the
/// peak's, from a cusp's to a disc's.
const WIDTH_STEPS: u32 = 15;

/// The bound's rings of probes about the lobe's axis: at 1/2, 1, 2, 4 and 8 widths.
const RING_WIDTHS: [f64; 5] = [0.5, 1.0, 2.0, 4.0, 8.0];

/// The probes on each ring.
const RING_PROBES: u32 = 8;

/// The bound's probes along the circle of directions whose sites lie in the galactic plane, where
/// a disc's density makes a ridge as narrow as its scale height over the sphere's radius.
const PLANE_PROBES: u32 = 64;

/// The directions a survivor's launch tries, each on four words of its velocity stream, before it
/// takes the radial one.
pub const LAUNCH_TRIES: u32 = 16_384;

/// What a survivor's donor is (module documentation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DonorKind {
    /// A helium white dwarf, up to [`HEAVIEST_HELIUM_WHITE_DWARF`]: the slow band's.
    Helium,
    /// A C/O white dwarf, or an O/Ne one of the heaviest progenitors: the fast band's.
    CarbonOxygen,
}

impl DonorKind {
    /// The kind of a donor of `mass`.
    #[must_use]
    pub fn of(mass: SolarMasses) -> Self {
        if mass.value() <= HEAVIEST_HELIUM_WHITE_DWARF.value() {
            Self::Helium
        } else {
            Self::CarbonOxygen
        }
    }
}

/// A cold white dwarf's radius, m, at `mass`: Nauenberg's (1972) law
/// ([`NAUENBERG_RADIUS_SOLAR`]), for 0 < `mass` < M₃.
#[must_use]
fn cold_white_dwarf_radius_m(mass: SolarMasses) -> f64 {
    let x = math::powf(mass.value() / NAUENBERG_MASS.value(), 2.0 / 3.0);
    NAUENBERG_RADIUS_SOLAR * SOLAR_RADIUS_M * (1.0 / x - x).max(0.0).sqrt()
}

/// The donor's Roche lobe over the separation at mass ratio `q`, donor over accretor: Eggleton's
/// (1983, ApJ 268, 368) 0.49 q^2/3 ÷ (0.6 q^2/3 + ln(1 + q^1/3)).
#[must_use]
fn roche_lobe_share(q: f64) -> f64 {
    let q13 = math::cbrt(q);
    let q23 = q13 * q13;
    0.49 * q23 / (0.6 * q23 + math::ln(1.0 + q13))
}

/// A donor's orbital speed at Roche-lobe contact beside an accretor, a cold white dwarf of
/// `donor` filling its lobe (module documentation).
///
/// It is `M₁ ÷ (M₁ + M₂) × √(G (M₁ + M₂) ÷ a)` with the separation `a = R₂ ÷ r_L(M₂ ÷ M₁)`: the
/// donor's speed about the pair's centre of mass when the accretor detonates, km/s. It holds for
/// donors of 0 < M₂ < Nauenberg's M₃, 1.454 M☉, past which the cold radius vanishes.
#[must_use]
pub fn contact_speed(donor: SolarMasses, accretor: SolarMasses) -> KilometresPerSecond {
    let (m1, m2) = (accretor.value(), donor.value());
    let a = cold_white_dwarf_radius_m(donor) / roche_lobe_share(m2 / m1);
    let orbit = (GM_SUN * (m1 + m2) / a).sqrt();
    KilometresPerSecond::new(m1 / (m1 + m2) * orbit / 1_000.0)
}

/// The donor whose contact speed beside [`ACCRETOR_MASS`] is `speed`: [`contact_speed`]'s inverse,
/// by bisection, held to [`LIGHTEST_DONOR`] and the accretor's own mass, which a donor's cannot
/// pass.
#[must_use]
pub fn donor_mass(speed: KilometresPerSecond) -> SolarMasses {
    let v = |m: f64| contact_speed(SolarMasses::new(m), ACCRETOR_MASS).value();
    let (mut lo, mut hi) = (LIGHTEST_DONOR.value(), ACCRETOR_MASS.value());
    if speed.value() <= v(lo) {
        return LIGHTEST_DONOR;
    }
    if speed.value() >= v(hi) {
        return ACCRETOR_MASS;
    }
    for _ in 0..SPEED_BISECTIONS {
        let mid = f64::midpoint(lo, hi);
        if v(mid) < speed.value() {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    SolarMasses::new(f64::midpoint(lo, hi))
}

/// The initial mass of a C/O donor's progenitor: Kalirai et al.'s (2008, ApJ 676, 594) initial–
/// final relation, `M_wd = 0.109 M_i + 0.394` M☉, inverted, held to layer D's masses, the class's
/// layer, 2.5–8 M☉.
#[must_use]
pub fn donor_progenitor_mass(donor: SolarMasses) -> SolarMasses {
    let initial = (donor.value() - 0.394) / 0.109;
    SolarMasses::new(initial.clamp(MASS_BAND_EDGES[3], MASS_BAND_EDGES[4]))
}

/// A survivor's direction of flight and how it was found ([`Survivors::launch_direction`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LaunchDirection {
    direction: [f64; 3],
    tries: u32,
    radial: bool,
}

impl LaunchDirection {
    /// The unit direction, galactic.
    #[must_use]
    pub const fn direction(&self) -> [f64; 3] {
        self.direction
    }

    /// The tries it read: that of the direction accepted, 1 to [`LAUNCH_TRIES`]; every one where
    /// none was; none on a sphere with no launch density.
    #[must_use]
    pub const fn tries(&self) -> u32 {
        self.tries
    }

    /// Whether no try was accepted, so that the direction is the radial one.
    #[must_use]
    pub const fn is_radial(&self) -> bool {
        self.radial
    }
}

/// Direction `i` of the Fibonacci lattice of `n` on the unit sphere.
#[must_use]
fn fibonacci_direction(i: u32, n: u32) -> [f64; 3] {
    let z = 1.0 - (2.0 * f64::from(i) + 1.0) / f64::from(n);
    let r = (1.0 - z * z).max(0.0).sqrt();
    let (sin, cos) = math::sin_cos(f64::from(i) * GOLDEN_ANGLE);
    [r * cos, r * sin, z]
}

/// One of [`SURVIVOR_POPULATIONS`]: the slow donors or the fast (ruling 128.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SurvivorBand {
    /// 1,000–1,500 km/s.
    Slow,
    /// 2,000–2,500 km/s.
    Fast,
}

impl SurvivorBand {
    /// Both, in [`SURVIVOR_POPULATIONS`]' order.
    pub const ALL: [Self; 2] = [Self::Slow, Self::Fast];

    /// Its index in [`SURVIVOR_POPULATIONS`].
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Slow => 0,
            Self::Fast => 1,
        }
    }

    /// Its share and speeds.
    #[must_use]
    pub const fn population(self) -> SurvivorPopulation {
        SURVIVOR_POPULATIONS[self.index()]
    }
}

/// The hypervelocity survivors of one galaxy (module documentation): their count inside the cube by
/// population, their birth components' odds, and what their flights are drawn from.
#[derive(Debug, Clone, PartialEq)]
pub struct Survivors {
    /// The galaxy's Type Ia rate, per year.
    rate_per_year: f64,
    /// By population of [`SURVIVOR_POPULATIONS`], the survivors inside the cube.
    count: [f64; SURVIVOR_POPULATIONS.len()],
    /// Per field component, its Type Ia rate per year: a survivor's odds of its birth component.
    births_per_year: Box<[f64]>,
    /// Per field component, its Type Ia per system per year, which its density weighs into the
    /// launch density.
    per_system_per_year: Box<[f64]>,
    /// The least time since launch, years: S, the latest start the Type Ia class's interval can
    /// have, before the epoch.
    youngest: Years,
    /// The mean path from launch to the cube's face, ly (P15.T6.f).
    mean_exit_ly: f64,
}

/// A hypervelocity survivor's flight: its population, its launch speed, its time since launch and
/// its donor's mass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurvivorFlight {
    band: SurvivorBand,
    speed: KilometresPerSecond,
    since_launch: Years,
    donor: SolarMasses,
}

impl SurvivorFlight {
    /// Its population.
    #[must_use]
    pub const fn band(&self) -> SurvivorBand {
        self.band
    }

    /// Its speed from its launch site, in its population's band.
    #[must_use]
    pub const fn speed(&self) -> KilometresPerSecond {
        self.speed
    }

    /// The time since its launch, at least [`Survivors::youngest`].
    #[must_use]
    pub const fn since_launch(&self) -> Years {
        self.since_launch
    }

    /// Its donor's mass, the white dwarf it is: its speed's ([`donor_mass`]).
    #[must_use]
    pub const fn donor(&self) -> SolarMasses {
        self.donor
    }

    /// What its donor is.
    #[must_use]
    pub fn donor_kind(&self) -> DonorKind {
        DonorKind::of(self.donor)
    }

    /// How far it has flown since its launch, ly: its speed × its time since launch.
    #[must_use]
    pub fn flown_ly(&self) -> f64 {
        self.speed.value() * LIGHT_YEARS_PER_YEAR_PER_KM_S * self.since_launch.value()
    }
}

impl Survivors {
    /// The survivors of `galaxy` under the form table `forms`: its Type Ia rate, each component's
    /// part of it, and the latest start of the Type Ia class's interval.
    ///
    /// # Panics
    ///
    /// Never: the interval's start lies 4.26 Myr before the epoch, far inside the clock's range.
    #[must_use]
    pub(crate) fn of(galaxy: &Galaxy, forms: &FormTable) -> Self {
        let delays = DelayTimeDistribution::from_galaxy(galaxy);
        let fields = galaxy.fields();
        let per_system_per_year: Box<[f64]> = fields
            .component_ids()
            .map(|id| delays.rate_per_system_per_year(id))
            .collect();
        let births_per_year: Box<[f64]> = fields
            .component_ids()
            .zip(&per_system_per_year)
            .map(|(id, &per_system)| fields.component(id).count_with_unborn() * per_system)
            .collect();
        let rate_per_year = births_per_year.iter().sum();
        let start = SupernovaInterval::of_window(SHELL_WINDOW_CAP).start();
        let youngest = Years::new(-start.since_epoch().as_julian_years_f64());
        let mean_exit_ly = forms.rows().hypervelocity_mean_exit_ly;
        Self {
            rate_per_year,
            count: survivors_launched_before(rate_per_year, mean_exit_ly, youngest),
            births_per_year,
            per_system_per_year,
            youngest,
            mean_exit_ly,
        }
    }

    /// The galaxy's Type Ia rate at the epoch, per year: its components' over their born and
    /// unborn systems.
    #[must_use]
    pub fn rate_per_year(&self) -> f64 {
        self.rate_per_year
    }

    /// The survivors inside the cube, by population of [`SURVIVOR_POPULATIONS`].
    #[must_use]
    pub fn count(&self) -> [f64; SURVIVOR_POPULATIONS.len()] {
        self.count
    }

    /// The survivors inside the cube: the class's count in layer D.
    #[must_use]
    pub fn total(&self) -> f64 {
        self.count[0] + self.count[1]
    }

    /// Per field component, by index, its Type Ia rate per year: a survivor's odds of having been
    /// born in it.
    #[must_use]
    pub fn births_per_year(&self) -> &[f64] {
        &self.births_per_year
    }

    /// The survivors' mean path from launch to the cube's face, ly (P15.T6.f).
    #[must_use]
    pub fn mean_exit_ly(&self) -> f64 {
        self.mean_exit_ly
    }

    /// The least time since launch a member of the class has, S: the latest start the Type Ia
    /// class's interval can have, before the epoch, after which its entries hold the survivors.
    #[must_use]
    pub fn youngest(&self) -> Years {
        self.youngest
    }

    /// The speed at rank `u` in [0, 1] of a survivor of `band` inside the cube.
    ///
    /// Its density is ∝ `path ÷ v − S` over the band, the time a survivor at `v` is the class's,
    /// and its quantile a bisection of the cumulative `(path ÷ c) ln(v ÷ a) − S (v − a)`, which
    /// rises across both bands: `path ÷ v` is at least 9.4 Myr against S's 4.26 Myr.
    #[must_use]
    pub fn speed_at(&self, band: SurvivorBand, u: f64) -> KilometresPerSecond {
        let p = band.population();
        let reach = self.mean_exit_ly / LIGHT_YEARS_PER_YEAR_PER_KM_S;
        let s = self.youngest.value();
        let cumulative = |v: f64| reach * math::ln(v / p.min_km_s) - s * (v - p.min_km_s);
        let target = u * cumulative(p.max_km_s);
        let (mut lo, mut hi) = (p.min_km_s, p.max_km_s);
        for _ in 0..SPEED_BISECTIONS {
            let mid = f64::midpoint(lo, hi);
            if cumulative(mid) < target {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        KilometresPerSecond::new(f64::midpoint(lo, hi))
    }

    /// The time since launch at rank `u` in [0, 1] of a survivor at `speed` inside the cube:
    /// uniform on `[S, path ÷ v]`.
    #[must_use]
    pub fn since_launch(&self, speed: KilometresPerSecond, u: f64) -> Years {
        let s = self.youngest.value();
        let inside = residence_years(self.mean_exit_ly, speed.value());
        Years::new(s + u * (inside - s).max(0.0))
    }

    /// The flight of the survivor `id` under `seed` (module documentation).
    ///
    /// # Panics
    ///
    /// Never: a weighted class has survivors in some population.
    #[must_use]
    pub fn flight(&self, seed: Seed, id: SystemId) -> SurvivorFlight {
        let open = |tag| Stream::open(seed, tag, ObjectKey::from(id));
        let index = open(tags::DISPLACED_ORIGIN_BIN)
            .mark()
            .pick_weighted(&self.count, self.total())
            // Rounding can leave the mark past every threshold: the last population with weight.
            .unwrap_or_else(|| {
                self.count
                    .iter()
                    .rposition(|&count| count > 0.0)
                    .expect("a weighted class has survivors")
            });
        let band = SurvivorBand::ALL[index];
        let launch = self.speed_at(band, open(tags::RUNAWAY_SPEED).uniform());
        SurvivorFlight {
            band,
            speed: launch,
            since_launch: self.since_launch(launch, open(tags::RUNAWAY_EJECTION).uniform()),
            donor: donor_mass(launch),
        }
    }

    /// The galaxy's Type Ia rate density at `p`, per year per cubic light-year, which a survivor's
    /// launch sites follow.
    ///
    /// It is Σ over the galaxy's components of each one's density × its Type Ia per system per
    /// year at the epoch (its delay-time distribution over its ages,
    /// `DelayTimeDistribution::rate_per_system_per_year`). The convolution P15.T6's production
    /// form fits is of the same density.
    #[must_use]
    pub fn launches_per_year_per_cubic_ly(&self, galaxy: &Galaxy, p: &PointLy) -> f64 {
        let fields = galaxy.fields();
        fields
            .component_ids()
            .zip(&self.per_system_per_year)
            .fold(0.0, |sum, (id, &per_system)| {
                sum + per_system * fields.component(id).density(p)
            })
    }

    /// The direction a survivor at `at` with `flight` flies in, from a launch site it left
    /// `flight.flown_ly()` ago, drawn on `stream` (its `system.velocity` stream).
    ///
    /// The site lies on the sphere of that radius about `at`, its direction from `at` the flight's
    /// reversed, with density ∝
    /// [`launches_per_year_per_cubic_ly`](Self::launches_per_year_per_cubic_ly) there, drawn by
    /// rejection (lean, 2026-11-01). The proposal is in part uniform on the sphere, in part a von
    /// Mises–Fisher lobe about the densest of the probes (256 lattice directions, the site nearest
    /// the centre and 64 whose sites lie in the plane), and in part a band about the plane's
    /// circle, where a disc's density makes a ridge; the lobe's concentration and the band's width
    /// are fitted to the density's fall about each. The bound is three times the largest ratio of
    /// the density to the proposal's at the probes and at rings about the lobe and the band. The
    /// probes only sample the sphere, so a try whose density passes the bound raises it to twice
    /// that try's ratio for the tries after it. Try `k` reads words 4k to 4k + 3: the proposal's
    /// part, its direction's two uniforms, and the mark that accepts it. After [`LAUNCH_TRIES`],
    /// or on a sphere with no density, it is the radial direction, away from the centre (along +z
    /// at the centre itself).
    #[must_use]
    pub fn launch_direction(
        &self,
        galaxy: &Galaxy,
        at: &PointLy,
        flight: &SurvivorFlight,
        stream: &mut Stream,
    ) -> LaunchDirection {
        LaunchSampler::new(self, galaxy, at, flight).draw(stream)
    }
}

/// The rejection sampler of one survivor's launch direction ([`Survivors::launch_direction`]): the
/// density on its launch sphere, the proposal and their ratio's bound.
#[derive(Debug, Clone)]
pub(crate) struct LaunchSampler<'a> {
    survivors: &'a Survivors,
    galaxy: &'a Galaxy,
    at: PointLy,
    flown: f64,
    radial: [f64; 3],
    lobe: Lobe,
    band: Option<Band>,
    /// [`PROPOSAL_SHARES`]' row: uniform, the lobe's and the band's.
    shares: [f64; 3],
    bound: f64,
}

impl<'a> LaunchSampler<'a> {
    /// The sampler of the survivor at `at` with `flight`: its probes, its lobe and its bound.
    #[must_use]
    pub(crate) fn new(
        survivors: &'a Survivors,
        galaxy: &'a Galaxy,
        at: &PointLy,
        flight: &SurvivorFlight,
    ) -> Self {
        let flown = flight.flown_ly();
        let from_centre = (at.x * at.x + at.y * at.y + at.z * at.z).sqrt();
        let radial = if from_centre > 0.0 {
            [at.x / from_centre, at.y / from_centre, at.z / from_centre]
        } else {
            [0.0, 0.0, 1.0]
        };
        let mut sampler = Self {
            survivors,
            galaxy,
            at: *at,
            flown,
            radial,
            lobe: Lobe::about(radial, 1.0),
            band: None,
            shares: PROPOSAL_SHARES[1],
            bound: 0.0,
        };
        // The directions whose sites lie in the plane, z = at.z ÷ flown, where the sphere
        // reaches it: the one towards the centre first.
        let plane: Vec<[f64; 3]> = if flown > 0.0 && at.z.abs() <= flown {
            let nz = at.z / flown;
            let across = (1.0 - nz * nz).max(0.0).sqrt();
            let towards = math::atan2(at.y, at.x);
            (0..PLANE_PROBES)
                .map(|i| {
                    let turn = core::f64::consts::TAU * f64::from(i) / f64::from(PLANE_PROBES);
                    let (sin, cos) = math::sin_cos(towards + turn);
                    [across * cos, across * sin, nz]
                })
                .collect()
        } else {
            Vec::new()
        };
        let densest = |probes: &[([f64; 3], f64)]| {
            probes
                .iter()
                .copied()
                .fold((radial, f64::NEG_INFINITY), |best, probe| {
                    if probe.1 > best.1 { probe } else { best }
                })
        };
        let in_plane: Vec<([f64; 3], f64)> =
            plane.iter().map(|&n| (n, sampler.density(n))).collect();
        let probes: Vec<([f64; 3], f64)> = std::iter::once(radial)
            .chain((0..BOUND_PROBES).map(|i| fibonacci_direction(i, BOUND_PROBES)))
            .map(|n| (n, sampler.density(n)))
            .chain(in_plane.iter().copied())
            .collect();
        debug_assert!(
            probes.iter().all(|probe| probe.1.is_finite()),
            "a launch density is finite"
        );
        let (axis, peak) = densest(&probes);
        let lobe_probes = sampler.fit_lobe(axis, peak);
        let band_probes = match densest(&in_plane) {
            (ridge, crest) if crest > 0.0 => sampler.fit_band(ridge, crest),
            _ => Vec::new(),
        };
        sampler.bound = BOUND_MARGIN
            * probes
                .iter()
                .map(|&(n, rho)| rho / sampler.proposal(n))
                .chain(
                    lobe_probes
                        .iter()
                        .chain(&band_probes)
                        .map(|&n| sampler.density(n) / sampler.proposal(n)),
                )
                .fold(0.0, f64::max);
        sampler
    }

    /// Fits the lobe about `axis`, whose density is `peak`: its width the least angle from the
    /// axis at which the density falls under 1 ÷ e of the axis's, along the axis's first tangent,
    /// and its concentration 2 ÷ width². Gives the bound's rings of probes about it.
    fn fit_lobe(&mut self, axis: [f64; 3], peak: f64) -> Vec<[f64; 3]> {
        let tangent = Lobe::about(axis, 1.0);
        let width = (0..WIDTH_STEPS)
            .map(|j| 1e-4 * f64::from(1_u32 << j))
            .find(|&theta| self.density(tangent.at(theta, 0.0)) < peak * math::exp(-1.0))
            .unwrap_or(1.0);
        self.lobe = Lobe::about(axis, (2.0 / (width * width)).clamp(1.0, 1e8));
        RING_WIDTHS
            .iter()
            .flat_map(|&widths| {
                (0..RING_PROBES).map(move |i| (widths, f64::from(i) / f64::from(RING_PROBES)))
            })
            .map(|(widths, turn)| {
                self.lobe
                    .at((widths * width).min(core::f64::consts::PI), turn)
            })
            .collect()
    }

    /// Fits the band about the plane's circle through `ridge`, its densest direction there, whose
    /// density is `crest`: its scale the least step in z from it at which the density falls under
    /// 1 ÷ e of the crest's. Gives the bound's probes on either side of it.
    fn fit_band(&mut self, ridge: [f64; 3], crest: f64) -> Vec<[f64; 3]> {
        let centre = ridge[2];
        let r_xy = (ridge[0] * ridge[0] + ridge[1] * ridge[1]).sqrt();
        let off_plane = |z: f64| {
            let across = (1.0 - z * z).max(0.0).sqrt();
            if r_xy > 0.0 {
                [ridge[0] / r_xy * across, ridge[1] / r_xy * across, z]
            } else {
                [across, 0.0, z]
            }
        };
        let scale = (0..WIDTH_STEPS + 2)
            .map(|j| 1e-5 * f64::from(1_u32 << j))
            .find(|&step| {
                self.density(off_plane((centre + step).min(1.0))) < crest * math::exp(-1.0)
            })
            .unwrap_or(1.0);
        self.band = Some(Band { centre, scale });
        self.shares = PROPOSAL_SHARES[0];
        RING_WIDTHS
            .iter()
            .flat_map(|&widths| {
                [-1.0, 1.0].map(|sign| off_plane((centre + sign * widths * scale).clamp(-1.0, 1.0)))
            })
            .collect()
    }

    /// The launch density at the site in direction `n`'s reverse, per year per cubic light-year.
    #[must_use]
    pub(crate) fn density(&self, n: [f64; 3]) -> f64 {
        let site = PointLy {
            x: self.at.x - self.flown * n[0],
            y: self.at.y - self.flown * n[1],
            z: self.at.z - self.flown * n[2],
        };
        self.survivors
            .launches_per_year_per_cubic_ly(self.galaxy, &site)
    }

    /// The proposal's density at the unit direction `n`, per steradian.
    #[must_use]
    pub(crate) fn proposal(&self, n: [f64; 3]) -> f64 {
        let [uniform, lobe, band] = self.shares;
        uniform / (4.0 * core::f64::consts::PI)
            + lobe * self.lobe.density(n)
            + self.band.map_or(0.0, |b| band * b.density(n))
    }

    /// The bound on the density over the proposal, 0 on a sphere with no launch density.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn bound(&self) -> f64 {
        self.bound
    }

    /// The direction, drawn on `stream` ([`Survivors::launch_direction`]).
    #[must_use]
    pub(crate) fn draw(&self, stream: &mut Stream) -> LaunchDirection {
        if !(self.bound > 0.0 && self.bound.is_finite()) {
            return LaunchDirection {
                direction: self.radial,
                tries: 0,
                radial: true,
            };
        }
        let mut bound = self.bound;
        for k in 0..LAUNCH_TRIES {
            stream.seek(4 * u64::from(k));
            let part = stream
                .mark()
                .pick_weighted(&self.shares, 1.0)
                // Rounding can leave the mark past every threshold: the last part with weight.
                .unwrap_or_else(|| {
                    self.shares
                        .iter()
                        .rposition(|&share| share > 0.0)
                        .expect("a proposal has a part with weight")
                });
            let (rank, turn) = (stream.uniform(), stream.uniform());
            let n = match part {
                0 => Some(uniform_direction(rank, turn)),
                1 => Some(self.lobe.direction(rank, turn)),
                _ => self.band.and_then(|b| b.direction(rank, turn)),
            };
            // A band's direction past a pole is a try refused.
            let Some(n) = n else { continue };
            let (density, cap) = (self.density(n), bound * self.proposal(n));
            // A try past the probed bound raises it, to twice its ratio, for the tries after it.
            if density > cap {
                bound = 2.0 * density / self.proposal(n);
                continue;
            }
            if stream.decide(Threshold::from_ratio(density, cap)) {
                return LaunchDirection {
                    direction: n,
                    tries: k + 1,
                    radial: false,
                };
            }
        }
        LaunchDirection {
            direction: self.radial,
            tries: LAUNCH_TRIES,
            radial: true,
        }
    }
}

/// The direction at ranks `u` and `w` uniform on the unit sphere: z = 2u − 1 and the azimuth 2πw.
#[must_use]
fn uniform_direction(u: f64, w: f64) -> [f64; 3] {
    let z = 2.0 * u - 1.0;
    let (sin, cos) = math::sin_cos(core::f64::consts::TAU * w);
    let across = (1.0 - z * z).max(0.0).sqrt();
    [across * cos, across * sin, z]
}

/// A band of directions about the circle whose sites lie in the galactic plane, the band of a
/// launch's proposal: their z Laplace about `centre` of scale `scale`, their azimuth uniform.
/// Its draws past a pole are refused, so on the sphere it is a density of total under one.
#[derive(Debug, Clone, Copy)]
struct Band {
    centre: f64,
    scale: f64,
}

impl Band {
    /// The density at the unit direction `n`, per steradian: `e^(−|z − centre| ÷ scale) ÷ (2 scale)`
    /// over 2π.
    #[must_use]
    fn density(self, n: [f64; 3]) -> f64 {
        math::exp(-(n[2] - self.centre).abs() / self.scale)
            / (2.0 * self.scale * core::f64::consts::TAU)
    }

    /// The direction at ranks `u` and `w`, its z the Laplace quantile of `u` and its azimuth 2πw,
    /// or `None` past a pole.
    #[must_use]
    fn direction(self, u: f64, w: f64) -> Option<[f64; 3]> {
        let z = if u < 0.5 {
            self.centre + self.scale * math::ln(2.0 * u)
        } else {
            self.centre - self.scale * math::ln(2.0 * (1.0 - u))
        };
        if !(-1.0..=1.0).contains(&z) {
            return None;
        }
        let across = (1.0 - z * z).max(0.0).sqrt();
        let (sin, cos) = math::sin_cos(core::f64::consts::TAU * w);
        Some([across * cos, across * sin, z])
    }
}

/// A von Mises–Fisher density on the unit sphere about `axis` of concentration `kappa`, the lobe
/// of a launch's proposal.
#[derive(Debug, Clone, Copy)]
struct Lobe {
    axis: [f64; 3],
    across: [[f64; 3]; 2],
    kappa: f64,
    /// κ ÷ (2π (1 − e^−2κ)), the density at the axis.
    peak: f64,
}

impl Lobe {
    /// The lobe about the unit `axis` of concentration `kappa`.
    #[must_use]
    fn about(axis: [f64; 3], kappa: f64) -> Self {
        let other = if axis[2].abs() < 0.9 {
            [0.0, 0.0, 1.0]
        } else {
            [1.0, 0.0, 0.0]
        };
        let cross = |a: [f64; 3], b: [f64; 3]| {
            [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ]
        };
        let e1 = cross(axis, other);
        let norm = (e1[0] * e1[0] + e1[1] * e1[1] + e1[2] * e1[2]).sqrt();
        let e1 = e1.map(|c| c / norm);
        let e2 = cross(axis, e1);
        Self {
            axis,
            across: [e1, e2],
            kappa,
            peak: kappa / (core::f64::consts::TAU * (1.0 - math::exp(-2.0 * kappa))),
        }
    }

    /// The direction `theta` rad from the axis, at the azimuth of `turn` turns about it.
    #[must_use]
    fn at(&self, theta: f64, turn: f64) -> [f64; 3] {
        let (sin, cos) = math::sin_cos(theta);
        let (s, c) = math::sin_cos(core::f64::consts::TAU * turn);
        [0, 1, 2]
            .map(|i| cos * self.axis[i] + sin * (c * self.across[0][i] + s * self.across[1][i]))
    }

    /// The density at the unit direction `n`, per steradian.
    #[must_use]
    fn density(&self, n: [f64; 3]) -> f64 {
        let cos = n[0] * self.axis[0] + n[1] * self.axis[1] + n[2] * self.axis[2];
        self.peak * math::exp(self.kappa * (cos - 1.0))
    }

    /// The direction at ranks `u` and `w`: its cosine to the axis `1 + ln(u + (1 − u) e^−2κ) ÷ κ`
    /// (Wood 1994, Communications in Statistics: Simulation and Computation 23, 157) and its
    /// azimuth 2πw.
    #[must_use]
    fn direction(&self, u: f64, w: f64) -> [f64; 3] {
        let cos = 1.0 + math::ln(u + (1.0 - u) * math::exp(-2.0 * self.kappa)) / self.kappa;
        let cos = cos.clamp(-1.0, 1.0);
        let sin = (1.0 - cos * cos).max(0.0).sqrt();
        let (s, c) = math::sin_cos(core::f64::consts::TAU * w);
        [0, 1, 2]
            .map(|i| cos * self.axis[i] + sin * (c * self.across[0][i] + s * self.across[1][i]))
    }
}

/// The years a survivor at `speed_km_s` takes over `path_ly`.
#[must_use]
fn residence_years(path_ly: f64, speed_km_s: f64) -> f64 {
    path_ly / (speed_km_s * LIGHT_YEARS_PER_YEAR_PER_KM_S)
}

/// The survivors of each population inside the cube launched at least `since` before the epoch.
///
/// At the Type Ia rate `rate_per_year`, the mean path to the cube's face being `mean_exit_ly`,
/// they are `rate × shareᵢ × (residenceᵢ − since)`, residence the path over `v_eff,ᵢ` (module
/// documentation); at `since` zero, [`survivors_inside`](super::class_table::survivors_inside).
#[must_use]
fn survivors_launched_before(
    rate_per_year: f64,
    mean_exit_ly: f64,
    since: Years,
) -> [f64; SURVIVOR_POPULATIONS.len()] {
    SURVIVOR_POPULATIONS.map(|p: SurvivorPopulation| {
        let inside = residence_years(mean_exit_ly, p.effective_speed_km_s());
        rate_per_year * p.share * (inside - since.value()).max(0.0)
    })
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::stats::{ALPHA, assert_p_value, ks_one_sample, normal_cdf};

    use super::*;
    use crate::galaxy::displaced::class_table::survivors_inside;
    use crate::tables::displaced_forms::HYPERVELOCITY_MEAN_EXIT_LY;

    /// Survivors at the Milky Way's rate, 0.54 per century, and the latest interval, 4.26 Myr.
    fn milky_way_like() -> Survivors {
        let rate_per_year = 0.0054;
        let youngest = Years::new(4.263_144e6);
        Survivors {
            rate_per_year,
            count: survivors_launched_before(rate_per_year, HYPERVELOCITY_MEAN_EXIT_LY, youngest),
            births_per_year: Box::new([rate_per_year]),
            per_system_per_year: Box::new([1e-12]),
            youngest,
            mean_exit_ly: HYPERVELOCITY_MEAN_EXIT_LY,
        }
    }

    /// With no interval the count is ruling 128.1's, and the interval takes its share of each
    /// population's residence: 4.26 Myr of the slow survivors' 19 Myr and of the fast ones' 10.5.
    #[test]
    fn the_interval_takes_its_share_of_each_population() {
        let all = survivors_inside(0.0054, HYPERVELOCITY_MEAN_EXIT_LY);
        let none = survivors_launched_before(0.0054, HYPERVELOCITY_MEAN_EXIT_LY, Years::ZERO);
        for (a, b) in all.iter().zip(&none) {
            assert!((a - b).abs() <= 1e-12 * a, "{all:?} {none:?}");
        }
        let held = milky_way_like().count();
        for (i, p) in SURVIVOR_POPULATIONS.iter().enumerate() {
            let residence = residence_years(HYPERVELOCITY_MEAN_EXIT_LY, p.effective_speed_km_s());
            let kept = held[i] / all[i];
            assert!(
                (kept - (1.0 - 4.263_144e6 / residence)).abs() < 1e-12,
                "{i}: {kept}"
            );
        }
        eprintln!("all {all:?}, launched before the interval {held:?}");
    }

    /// The speed quantile inverts the density `path ÷ v − S`: its ends are the band's, it rises
    /// with the rank, and at each rank the density's integral to it, by a midpoint sum, is that
    /// share of the band's.
    #[test]
    fn the_speed_quantile_inverts_the_survivors_density() {
        let survivors = milky_way_like();
        let s = survivors.youngest().value();
        for band in SurvivorBand::ALL {
            let p = band.population();
            assert!((survivors.speed_at(band, 0.0).value() - p.min_km_s).abs() < 1e-9);
            assert!((survivors.speed_at(band, 1.0).value() - p.max_km_s).abs() < 1e-9);
            let density = |v: f64| residence_years(HYPERVELOCITY_MEAN_EXIT_LY, v) - s;
            let integral = |to: f64| {
                let n = 20_000;
                let step = (to - p.min_km_s) / f64::from(n);
                (0..n)
                    .map(|k| density(p.min_km_s + (f64::from(k) + 0.5) * step) * step)
                    .sum::<f64>()
            };
            let whole = integral(p.max_km_s);
            let mut last = p.min_km_s;
            for u in [0.1, 0.25, 0.5, 0.75, 0.9] {
                let v = survivors.speed_at(band, u).value();
                assert!(v > last, "{band:?}: {v} at {u}");
                last = v;
                let share = integral(v) / whole;
                assert!((share - u).abs() < 1e-6, "{band:?}: {share} at {u}");
            }
        }
    }

    /// The donor's contact speed beside a 1.0 M☉ accretor against its mass, to 2% of a hand
    /// computation of cold donors filling their lobes by the same two laws with M₃ = 1.44 M☉, which
    /// Nauenberg's own 1.454 M☉ runs 0.2–0.7% under: 1,289 km/s at 0.15 M☉, 1,386 at 0.2, 1,527 at
    /// 0.3, 1,716 at 0.5, 1,869 at 0.7, 2,038 at 0.9 and 2,146 at 1.0; and the speed rises with the
    /// donor's mass, which inverts it.
    #[test]
    fn the_contact_speed_follows_the_donors_mass() {
        let accretor = ACCRETOR_MASS;
        for (donor, km_s) in [
            (0.15, 1_289.0),
            (0.2, 1_386.0),
            (0.3, 1_527.0),
            (0.5, 1_716.0),
            (0.7, 1_869.0),
            (0.9, 2_038.0),
            (1.0, 2_146.0),
        ] {
            let v = contact_speed(SolarMasses::new(donor), accretor).value();
            assert!(
                (v / km_s - 1.0).abs() < 0.02,
                "{donor} M☉: {v} km/s against {km_s}"
            );
            let back = donor_mass(KilometresPerSecond::new(v)).value();
            assert!((back - donor).abs() < 1e-9, "{donor} M☉ back as {back}");
        }
        // The other accretors of the table, 0.8 and 1.1 M☉, at 0.3 M☉.
        for (accretor, km_s) in [(0.8, 1_368.0), (1.1, 1_598.0)] {
            let v = contact_speed(SolarMasses::new(0.3), SolarMasses::new(accretor)).value();
            assert!(
                (v / km_s - 1.0).abs() < 0.02,
                "{accretor} M☉: {v} km/s against {km_s}"
            );
        }
        // The bands' donors: the slow band's helium white dwarfs, held at the lightest below its
        // contact speed, and the fast band's C/O ones, held at the accretor's mass above its.
        let mass = |km_s: f64| donor_mass(KilometresPerSecond::new(km_s));
        assert_same_bits(mass(1_000.0).value(), LIGHTEST_DONOR.value());
        let slowest_top = mass(SurvivorBand::Slow.population().max_km_s);
        assert_eq!(DonorKind::of(slowest_top), DonorKind::Helium);
        assert!(
            slowest_top.value() < HEAVIEST_HELIUM_WHITE_DWARF.value(),
            "{slowest_top:?}"
        );
        let fastest_foot = mass(SurvivorBand::Fast.population().min_km_s);
        assert_eq!(DonorKind::of(fastest_foot), DonorKind::CarbonOxygen);
        assert!(fastest_foot.value() > 0.8, "{fastest_foot:?}");
        assert_same_bits(mass(2_500.0).value(), ACCRETOR_MASS.value());
        // A C/O donor's progenitor is of layer D's masses.
        for donor in [0.5, 0.666, 0.9, 1.0] {
            let initial = donor_progenitor_mass(SolarMasses::new(donor)).value();
            assert!((2.5..=8.0).contains(&initial), "{donor}: {initial}");
        }
        let initial = donor_progenitor_mass(SolarMasses::new(0.9)).value();
        assert!((initial - (0.9 - 0.394) / 0.109).abs() < 1e-12, "{initial}");
    }

    /// The proposal's lobe samples its own density: the cosines to its axis of 20,000 draws follow
    /// its CDF, `(e^{κ(c − 1)} − e^{−2κ}) ÷ (1 − e^{−2κ})`, by a KS test, at a narrow and a wide
    /// concentration; and the density integrates to one over the sphere.
    #[test]
    fn the_lobe_samples_its_own_density() {
        let seed = Seed::new(0x0934_b10b);
        for kappa in [3.0, 400.0] {
            let lobe = Lobe::about([0.6, 0.0, 0.8], kappa);
            let mut stream = Stream::open(seed, tags::SELFTEST_STREAM, ObjectKey::galaxy());
            let mut cosines: Vec<f64> = (0..20_000)
                .map(|_| {
                    let n = lobe.direction(stream.uniform(), stream.uniform());
                    n[0] * 0.6 + n[2] * 0.8
                })
                .collect();
            let floor = math::exp(-2.0 * kappa);
            let ks = ks_one_sample(&mut cosines, |c| {
                ((math::exp(kappa * (c - 1.0)) - floor) / (1.0 - floor)).clamp(0.0, 1.0)
            });
            assert_p_value("the lobe's cosines", ks.p_value, ALPHA);
            // The integral over the sphere, 2π ∫ f dcos, by a midpoint sum.
            let steps = 200_000;
            let integral: f64 = (0..steps)
                .map(|k| {
                    let c = -1.0 + (f64::from(k) + 0.5) * 2.0 / f64::from(steps);
                    lobe.density(lobe.at(math::acos(c), 0.0)) * 2.0 / f64::from(steps)
                })
                .sum::<f64>()
                * core::f64::consts::TAU;
            assert!((integral - 1.0).abs() < 1e-3, "κ {kappa}: {integral}");
        }
    }

    /// A flight's time since launch is never under S, nor over the path at its speed, and its
    /// speed is in its population's band; the populations come in the counts' odds.
    #[test]
    fn a_flight_is_launched_before_the_interval_at_its_populations_speed() {
        let survivors = milky_way_like();
        let seed = Seed::new(0x0934_b000_0000_0001);
        let key = crate::galaxy::placement::CellKey::new(crate::id::Layer::D, [0, 400, 0])
            .expect("a cell of the cube");
        let n = 20_000;
        let mut fast = 0_u32;
        for i in 0..n {
            let id = key.candidate_id(i).expect("under the capacity");
            let flight = survivors.flight(seed, id);
            let band = flight.band().population();
            let v = flight.speed().value();
            assert!((band.min_km_s..=band.max_km_s).contains(&v), "{flight:?}");
            let tau = flight.since_launch().value();
            assert!(tau >= survivors.youngest().value(), "{flight:?}");
            assert!(
                tau <= residence_years(HYPERVELOCITY_MEAN_EXIT_LY, v),
                "{flight:?}"
            );
            fast += u32::from(flight.band() == SurvivorBand::Fast);
        }
        let p = survivors.count()[1] / survivors.total();
        let expected = p * f64::from(n);
        let z = (f64::from(fast) - expected) / (expected * (1.0 - p)).sqrt();
        assert_p_value(
            "the fast survivors' share",
            2.0 * normal_cdf(-z.abs()),
            ALPHA,
        );
    }
}
