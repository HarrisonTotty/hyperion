//! Nurseries: star-forming regions, then young bound clusters or OB associations, and the
//! superbubbles they blow (plan 09, P09.T4.b; Design note 1).
//!
//! Star-forming regions, OB associations and young bound clusters are one Poisson process of
//! nurseries on the young disc. A nursery is a star-forming region while embedded, for a drawn 3–5
//! Myr; then, by an independent mark, a bound open cluster (the bound fraction) or an unbound
//! association that expands at a drawn 1–5 km/s and dissolves at a drawn 30–100 Myr. Its kind is
//! a function of the evaluation time, as a star's state is ([`NurseryStage::at`]).
//!
//! # Figures, each a parameter of the generator version
//!
//! - **Share of star formation in nurseries** `f_n` = 0.9 and **bound fraction** `Γ_b` = 0.125, the
//!   middle of the plan's 10–15% (Design note 2; the brainstorm's "bound fraction of about 0.1").
//! - **Embedded duration** uniform on 3–5 Myr, **dissolution age** uniform on 30–100 Myr and
//!   **expansion speed** uniform on 1–5 km/s (Design note 1 and P09.T4.b), **age spread** uniform
//!   up to 3 Myr.
//! - **The association mass floor** is the nurseries' least mass, 10² M☉, so no association is
//!   cut; it is a named parameter so that a later version can raise it (Design note 1).
//! - **The star-formation efficiency** of an embedded region is drawn uniform on 0.1–0.3 (Lada and
//!   Lada 2003, ARA&A 41, 57, §5: "10–30%"), which sets the gas it holds at birth, `M_* (1 − ε) ÷
//!   ε`. The gas falls linearly to none over the embedded duration (ours), so that nothing jumps
//!   when the region emerges.
//! - **The size** is `max(D₀, v × age)`, held to 10–300 ly, where `D₀` is the diameter of the
//!   birth clump of mass `M_* ÷ ε` by the clouds' mass–radius relation
//!   ([`cloud_radius`]). The 10 ly floor and the
//!   300 ly cap are the brainstorm's range for star-forming regions and associations ("Large
//!   features"); the floor is provisional, since a small clump is smaller. A bound cluster stops
//!   expanding when it emerges: its size is then four half-mass radii, 10–100 ly, and its
//!   half-mass radius and concentration are drawn as an old cluster's are.
//!
//! # The superbubble (Weaver et al. 1977; Mac Low and McCray 1988)
//!
//! A nursery's massive stars blow a bubble of radius `R = 0.76 (L t³ ÷ ρ)^⅕` (Weaver, McCray,
//! Castor, Shapiro and Moore 1977, ApJ 218, 377, the radiative shell of their eq. 51; Mac Low and
//! McCray 1988, ApJ 324, 776, eq. 3, write it `267 pc (L₃₈ ÷ n₀)^⅕ t₇^⅗`). The power has two parts
//! (ruling 118.4):
//!
//! - **Winds:** 10⁵⁰ erg per star of 8 M☉ or more, spread evenly over the nursery's first 4 Myr
//!   (Krause, Fierlinger, Diehl et al. 2013, A&A 550, A49, §1, from Voss et al. 2009's population
//!   synthesis: "averaged over all massive stars (8 M☉ < M < 120 M☉), the energy input due to
//!   winds is of the order of 10⁵⁰ erg/star").
//! - **Supernovae:** `N_SN × 10⁵¹ erg` spread evenly from `t_first` to `t_last`, Mac Low and
//!   McCray's approximation, with `t_first` and `t_last` the lifetimes of 100 and 8 M☉ stars at
//!   solar metallicity.
//!
//! `N_SN` is the nursery's expected stars above 8 M☉. With the power no longer constant, the
//! bubble reads Weaver's form through its injected energy, `R = 0.76 (E(t) t² ÷ ρ)^⅕` with `t` the
//! nursery's age and `E(t)` the energy put in by then (ours: it is Weaver's exactly while one
//! constant power acts, as the winds' alone do for 4 Myr). So an association younger than its first
//! supernova has a bubble. The bubble exists from birth until the last core collapse, and only a
//! nursery that expects at least one massive star blows one. `ρ = 1.4 m_H n` with `n` the
//! smooth gas at the site. The radius is capped at blow-out, 2.5 neutral scale heights, and the
//! interior is log-normal about 0.005 cm⁻³ with 0.3 dex (the scatter is ours), truncated at two
//! standard deviations (1.26 × 10⁻³ to 0.020 cm⁻³; ruling 136.3), at 10^6.2 K.
//!
//! # The merge and the wall (from generator version 29, P09.T5)
//!
//! Grown as Weaver's to blow-out, the bubbles filled 0.96 of the plane at the solar circle and
//! carried no dust, against a hot filling of 0.2–0.5 there (Ferrière 2001, Rev. Mod. Phys. 73,
//! 1031). From [`SUPERBUBBLE_MERGE_VERSION`] ([`Superbubble::merging`]):
//!
//! - **The merge.** A bubble's shell stops growing once it has slowed to the speed at which a
//!   remnant's shell merges with the gas, `v_m = β c_net` (the shell window's: β = 2,
//!   `c_net² = C₀² + σ²`, σ = 8 km/s; Cioffi, McKee and Bertschinger 1988, ApJ 334, 252, p. 264,
//!   ruling 98), with `C₀ = √(P ÷ ρ)` the smooth gas's at the site at the field's thermal pressure
//!   (ours, a lean of 2026-11-05). Weaver's shell moves at `Ṙ = ⅗ R ÷ t` under a constant power
//!   `L`, so it reaches `v_m` at the radius `R_m = α (⅗ α)^(3/2) (L ÷ ρ)^½ v_m^(−3/2)`, α = 0.76.
//!   The power is not constant, so a bubble takes its mean power so far, `L̄ = E(t) ÷ t`, and its
//!   radius is the least of Weaver's, `R_m(L̄)` and the blow-out cap, to its last core collapse.
//! - **The wall.** The gas the bubble sweeps up, the site's mean density over its whole sphere
//!   (Weaver et al. 1977's shell of swept-up gas), lies in a uniform wall at [`WALL_COMPRESSION`]
//!   times that density, β² = 4, an isothermal shock's compression `M²` at the merge speed, whose
//!   Mach number in `c_net` is β (ours): from [`WALL_INNER_SHARE`] = `(1 − 1 ÷ β²)^⅓` = 0.909 of its
//!   radius to its edge. So a bubble moves the plane's gas and dust and does not take them away, and
//!   its hot core is the hole: the cores fill a fifth of the plane at the solar circle, and with the
//!   field's own hot phase half of it is hot.

use crate::GeneratorVersion;
use crate::coords::GalacticPosition;
use crate::galaxy::Galaxy;
use crate::galaxy::consts::YEARS_PER_MEGAYEAR;
use crate::galaxy::snr::MERGE_FACTOR;
use crate::galaxy::snr::window::merge_speed;
use crate::math;
use crate::units::consts::{
    BOLTZMANN_CONSTANT, HYDROGEN_MASS_KG, METRES_PER_LIGHT_YEAR, SECONDS_PER_JULIAN_YEAR,
};
use crate::units::{
    Dex, HydrogenPerCm3, Kelvin, KelvinPerCm3, KilometresPerSecond, LightYears, SolarMasses, Years,
};

use super::cloud::cloud_radius;
use super::open_cluster::{NURSERY_MASS_MIN, dissolution_time, present_mass};
use crate::galaxy::gas::MASS_PER_HYDROGEN_FACTOR;

/// `f_n`: the share of the young disc's star formation that happens in nurseries (Design note 2).
pub const NURSERY_SHARE: f64 = 0.9;

/// `Γ_b`: the share of nurseries that stay bound as open clusters (Design note 2).
pub const BOUND_FRACTION: f64 = 0.125;

/// The embedded duration is drawn uniform on this range, Myr (Design note 1).
pub const EMBEDDED_DURATION_MYR: (f64, f64) = (3.0, 5.0);

/// An association's dissolution age is drawn uniform on this range, Myr (Design note 1).
pub const DISSOLUTION_AGE_MYR: (f64, f64) = (30.0, 100.0);

/// The expansion speed is drawn uniform on this range, km/s (P09.T4.b).
pub const EXPANSION_SPEED_KM_S: (f64, f64) = (1.0, 5.0);

/// The age spread of a nursery's members is drawn uniform up to this, Myr (P09.T4.b).
pub const AGE_SPREAD_MAX_MYR: f64 = 3.0;

/// An unbound nursery lighter than this is no association once it emerges (Design note 1).
pub const ASSOCIATION_MASS_FLOOR: SolarMasses = NURSERY_MASS_MIN;

/// The star-formation efficiency of an embedded region is drawn uniform on this range (Lada and
/// Lada 2003).
pub const EFFICIENCY_RANGE: (f64, f64) = (0.1, 0.3);

/// A nursery's size is held to this range, ly (brainstorm, "Large features").
pub const SIZE_RANGE_LY: (f64, f64) = (10.0, 300.0);

/// A bound cluster's size once it emerges is this many half-mass radii (ours).
pub const BOUND_SIZE_RADII: f64 = 4.0;

/// A bound cluster's size is held to this range, ly: the brainstorm's 10–100 ly for open clusters.
pub const BOUND_SIZE_RANGE_LY: (f64, f64) = (10.0, 100.0);

/// The explosion energy of one supernova in the bubble's power: 10⁴⁴ J (10⁵¹ erg; Mac Low and
/// McCray 1988).
pub const SUPERNOVA_ENERGY_J: f64 = 1e44;

/// The wind energy of one star of 8 M☉ or more: 10⁴³ J (10⁵⁰ erg; Krause et al. 2013).
pub const WIND_ENERGY_J: f64 = 1e43;

/// The winds' energy is put in over this much of a nursery's life, Myr (Krause et al. 2013).
pub const WIND_DURATION_MYR: f64 = 4.0;

/// Weaver et al.'s coefficient of the radiative bubble, 0.76.
pub const BUBBLE_COEFFICIENT: f64 = 0.76;

/// The bubble's radius is capped at this many neutral scale heights: blow-out (P09.T4.b).
pub const BLOW_OUT_HEIGHTS: f64 = 2.5;

/// The median interior density of a superbubble, 0.005 cm⁻³ (P09.T4.b).
pub const BUBBLE_INTERIOR_MEDIAN: HydrogenPerCm3 = HydrogenPerCm3::new(0.005);

/// The log-normal scatter of a bubble's interior density, dex (ours, provisional).
pub const BUBBLE_INTERIOR_SIGMA_DEX: f64 = 0.3;

/// The truncation of a bubble's interior density, in standard deviations either side of the
/// median: 2, the law of the explosion energy (ruling 136.3), so that the shell window's bubble
/// cap is a property of the physics, not of the normal sampler's floating-point limit.
pub const BUBBLE_INTERIOR_TRUNCATION: f64 = 2.0;

/// A superbubble's interior temperature, 10^6.2 K (P09.T4.b).
pub const BUBBLE_TEMPERATURE: Kelvin = Kelvin::new(1_584_893.192_461_114);

/// How many times its site's mean density a superbubble's wall of swept-up gas holds: β² = 4, the
/// compression of an isothermal shock at the merge speed (module documentation; ours). A parameter
/// of the generator version.
pub const WALL_COMPRESSION: f64 = MERGE_FACTOR * MERGE_FACTOR;

/// The share of a superbubble's radius at which its wall begins, `(1 − 1 ÷ β²)^⅓` = `∛¾`, so that
/// the wall from it to the edge holds the gas of the whole sphere at [`WALL_COMPRESSION`].
pub const WALL_INNER_SHARE: f64 = 0.908_560_296_416_069_8;

/// The generator version from which a superbubble stops growing at the remnants' merge speed and
/// keeps its swept-up gas in a wall (P09.T5, module documentation): 29, a lean of 2026-11-05.
///
/// Below it every galaxy's `SuperbubbleRules` is `Weaver`: a bubble grows as Weaver's to
/// blow-out, with no wall, and every figure is version 28's bit for bit. 29's bump deletes this
/// hold, `SuperbubbleRules` with its `Weaver` arm and its field in `GalaxyRules`,
/// [`Superbubble::of`]'s use as an arm of [`Superbubble::in_galaxy`], and the tripwire
/// `the_superbubbles_merge_only_from_29`.
pub const SUPERBUBBLE_MERGE_VERSION: GeneratorVersion = GeneratorVersion::new(29);

/// How a galaxy's superbubbles grow (its hold, [`SUPERBUBBLE_MERGE_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum SuperbubbleRules {
    /// Version 28's and before: as Weaver's to blow-out, with no wall ([`Superbubble::of`]).
    Weaver,
    /// Version 29's: to the merge, with a wall ([`Superbubble::merging`]).
    Merging,
}

impl SuperbubbleRules {
    /// The rules of generator version `version`.
    #[must_use]
    pub(crate) const fn of_version(version: GeneratorVersion) -> Self {
        if version.get() >= SUPERBUBBLE_MERGE_VERSION.get() {
            Self::Merging
        } else {
            Self::Weaver
        }
    }
}

/// The share `G(age)` of associations born `age` ago that have dissolved: uniform dissolution ages
/// on [`DISSOLUTION_AGE_MYR`] (Design note 2).
#[must_use]
pub fn dissolved_share(age: Years) -> f64 {
    let (lo, hi) = DISSOLUTION_AGE_MYR;
    ((age.value() / YEARS_PER_MEGAYEAR - lo) / (hi - lo)).clamp(0.0, 1.0)
}

/// The share of nurseries born `age` ago that are still embedded: uniform embedded durations on
/// [`EMBEDDED_DURATION_MYR`].
#[must_use]
pub fn embedded_share(age: Years) -> f64 {
    let (lo, hi) = EMBEDDED_DURATION_MYR;
    ((hi - age.value() / YEARS_PER_MEGAYEAR) / (hi - lo)).clamp(0.0, 1.0)
}

/// What a nursery is at an age (Design note 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NurseryStage {
    /// Not yet born.
    Unborn,
    /// A star-forming region: an embedded cluster still forming, with its gas.
    Embedded,
    /// A bound open cluster.
    BoundCluster,
    /// An unbound OB association, expanding.
    Association,
    /// Nothing is left: an association past its dissolution age or below the mass floor, or a
    /// cluster past its disruption time.
    Dissolved,
}

/// The marks of a nursery, drawn on its own stream (P09.T4.b).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NurseryMarks {
    pub(crate) age_at_epoch: Years,
    pub(crate) mass: SolarMasses,
    pub(crate) bound: bool,
    pub(crate) embedded_duration: Years,
    pub(crate) dissolution_age: Years,
    pub(crate) expansion_speed: KilometresPerSecond,
    pub(crate) age_spread: Years,
    pub(crate) efficiency: f64,
    pub(crate) fe_h: Dex,
    pub(crate) bubble_interior: HydrogenPerCm3,
    pub(crate) half_mass_radius: LightYears,
    pub(crate) concentration: f64,
}

impl NurseryMarks {
    /// The nursery's age at the epoch: the time since its first stars formed.
    #[must_use]
    pub const fn age_at_epoch(&self) -> Years {
        self.age_at_epoch
    }

    /// Its stellar mass at birth.
    #[must_use]
    pub const fn mass(&self) -> SolarMasses {
        self.mass
    }

    /// Whether it stays bound as an open cluster when it emerges.
    #[must_use]
    pub const fn is_bound(&self) -> bool {
        self.bound
    }

    /// How long it stays embedded.
    #[must_use]
    pub const fn embedded_duration(&self) -> Years {
        self.embedded_duration
    }

    /// The age at which it dissolves if it is an association.
    #[must_use]
    pub const fn dissolution_age(&self) -> Years {
        self.dissolution_age
    }

    /// Its expansion speed once it emerges.
    #[must_use]
    pub const fn expansion_speed(&self) -> KilometresPerSecond {
        self.expansion_speed
    }

    /// The spread of its members' ages.
    #[must_use]
    pub const fn age_spread(&self) -> Years {
        self.age_spread
    }

    /// Its star-formation efficiency, 0.1–0.3.
    #[must_use]
    pub const fn efficiency(&self) -> f64 {
        self.efficiency
    }

    /// Its [Fe/H], one value for every member.
    #[must_use]
    pub const fn fe_h(&self) -> Dex {
        self.fe_h
    }

    /// The age at which its feature ends: the association's dissolution age, the cluster's
    /// disruption time, or the end of the embedded stage for an association below the floor.
    #[must_use]
    pub fn end_age(&self) -> Years {
        if self.bound {
            Years::new(
                dissolution_time(self.mass)
                    .value()
                    .max(self.embedded_duration.value()),
            )
        } else if self.mass.value() < ASSOCIATION_MASS_FLOOR.value() {
            self.embedded_duration
        } else {
            self.dissolution_age
        }
    }

    /// Its stage `t` years after the epoch.
    #[must_use]
    pub fn stage_at(&self, t: Years) -> NurseryStage {
        NurseryStage::at(self, self.age_at_epoch + t)
    }

    /// The gas it holds at `age`: `M_* (1 − ε) ÷ ε` at birth, falling linearly to none when it
    /// emerges, M☉.
    #[must_use]
    pub fn gas_mass_at(&self, age: Years) -> SolarMasses {
        let a = age.value();
        let h = self.embedded_duration.value();
        if a < 0.0 || a >= h {
            return SolarMasses::new(0.0);
        }
        let birth = self.mass.value() * (1.0 - self.efficiency) / self.efficiency;
        SolarMasses::new(birth * (1.0 - a / h))
    }

    /// Its diameter at `age`, light-years: `max(D₀, v × age)` held to [`SIZE_RANGE_LY`] (module
    /// documentation), continuous in age; once a bound cluster emerges, four half-mass radii held
    /// to [`BOUND_SIZE_RANGE_LY`], the cluster's stars without the gas that bore them.
    #[must_use]
    pub fn size_at(&self, age: Years) -> LightYears {
        if self.bound && age.value() >= self.embedded_duration.value() {
            let (floor, cap) = BOUND_SIZE_RANGE_LY;
            return LightYears::new(
                (BOUND_SIZE_RADII * self.half_mass_radius.value()).clamp(floor, cap),
            );
        }
        let (floor, cap) = SIZE_RANGE_LY;
        let clump = SolarMasses::new(self.mass.value() / self.efficiency);
        let birth = 2.0 * cloud_radius(clump).value();
        let expansion = self.expansion_speed.value()
            * crate::galaxy::consts::LIGHT_YEARS_PER_YEAR_PER_KM_S
            * age.value().max(0.0);
        LightYears::new(birth.max(expansion).clamp(floor, cap))
    }

    /// Its stellar mass at `age`: a bound cluster's present mass (L05), an association's or an
    /// embedded region's whole birth mass while it lasts.
    #[must_use]
    pub fn stellar_mass_at(&self, age: Years) -> SolarMasses {
        match NurseryStage::at(self, age) {
            NurseryStage::Unborn | NurseryStage::Dissolved => SolarMasses::new(0.0),
            NurseryStage::BoundCluster => present_mass(self.mass, age),
            NurseryStage::Embedded if self.bound => present_mass(self.mass, age),
            NurseryStage::Embedded | NurseryStage::Association => self.mass,
        }
    }

    /// The median interior density drawn for its bubble.
    #[must_use]
    pub const fn bubble_interior(&self) -> HydrogenPerCm3 {
        self.bubble_interior
    }

    /// Its half-mass radius as a bound cluster, drawn as an old cluster's is (P09.T4.a).
    #[must_use]
    pub const fn half_mass_radius(&self) -> LightYears {
        self.half_mass_radius
    }

    /// Its King concentration as a bound cluster, drawn as an old cluster's is (P09.T4.a).
    #[must_use]
    pub const fn concentration(&self) -> f64 {
        self.concentration
    }
}

impl NurseryStage {
    /// The stage of `marks` at `age`, continuous in the sense that every boundary is a single
    /// age at which the stage changes once.
    #[must_use]
    pub fn at(marks: &NurseryMarks, age: Years) -> Self {
        let a = age.value();
        if a < 0.0 {
            Self::Unborn
        } else if a < marks.embedded_duration.value() {
            Self::Embedded
        } else if a >= marks.end_age().value() {
            Self::Dissolved
        } else if marks.bound {
            Self::BoundCluster
        } else {
            Self::Association
        }
    }
}

/// When a nursery's core collapses happen, and how many it expects per solar mass formed (module
/// documentation, "The superbubble").
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SupernovaClock {
    first: Years,
    last: Years,
    per_solar_mass: f64,
}

impl SupernovaClock {
    /// A clock from the lifetimes of the heaviest and lightest core-collapse progenitors and the
    /// expected core collapses per solar mass formed.
    #[must_use]
    pub const fn new(first: Years, last: Years, per_solar_mass: f64) -> Self {
        Self {
            first,
            last,
            per_solar_mass,
        }
    }

    /// The age of the first core collapse.
    #[must_use]
    pub const fn first(&self) -> Years {
        self.first
    }

    /// The age of the last.
    #[must_use]
    pub const fn last(&self) -> Years {
        self.last
    }

    /// Expected core collapses per solar mass formed.
    #[must_use]
    pub const fn per_solar_mass(&self) -> f64 {
        self.per_solar_mass
    }
}

/// A nursery's superbubble at one age: a hole in the gas (P09.T4.b), lined from generator version
/// 29 by a wall of the gas it swept up (P09.T5).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Superbubble {
    radius: LightYears,
    interior: HydrogenPerCm3,
    wall: Option<HydrogenPerCm3>,
}

/// The energy a nursery expecting `count` core collapses has put into its bubble by age `a`
/// (years), J: its winds' and its supernovae's (module documentation).
fn injected_energy(count: f64, a: f64, clock: &SupernovaClock) -> f64 {
    let (first, last) = (clock.first.value(), clock.last.value());
    let wind_span = WIND_DURATION_MYR * YEARS_PER_MEGAYEAR;
    let winds = count * WIND_ENERGY_J * (a / wind_span).min(1.0);
    let supernovae = count * SUPERNOVA_ENERGY_J * ((a - first) / (last - first)).clamp(0.0, 1.0);
    winds + supernovae
}

impl Superbubble {
    /// The bubble of `marks` at `age`, in gas of smooth density `ambient` whose neutral layer is
    /// `neutral_height` thick, or `None` if it blows none then (module documentation).
    #[must_use]
    pub fn of(
        marks: &NurseryMarks,
        age: Years,
        ambient: HydrogenPerCm3,
        neutral_height: LightYears,
        clock: &SupernovaClock,
    ) -> Option<Self> {
        let count = marks.mass.value() * clock.per_solar_mass;
        let last = clock.last.value();
        let a = age.value();
        if count < 1.0 || a <= 0.0 || a >= last || ambient.value() <= 0.0 {
            return None;
        }
        let energy = injected_energy(count, a, clock);
        let density = MASS_PER_HYDROGEN_FACTOR * HYDROGEN_MASS_KG * ambient.value() * 1e6;
        let t = a * SECONDS_PER_JULIAN_YEAR;
        let radius_m = BUBBLE_COEFFICIENT * math::powf(energy * t * t / density, 0.2);
        let radius =
            (radius_m / METRES_PER_LIGHT_YEAR).min(BLOW_OUT_HEIGHTS * neutral_height.value());
        (radius > 0.0).then_some(Self {
            radius: LightYears::new(radius),
            interior: marks.bubble_interior,
            wall: None,
        })
    }

    /// The bubble of `marks` at `age` as from generator version 29 (module documentation, "The
    /// merge and the wall"): [`of`](Self::of)'s, stopped where its shell slows to the merge speed
    /// in gas of smooth density `ambient` at thermal pressure `pressure`, and lined by a wall of
    /// the gas it swept up. `None` where `of` gives none.
    #[must_use]
    pub fn merging(
        marks: &NurseryMarks,
        age: Years,
        ambient: HydrogenPerCm3,
        pressure: KelvinPerCm3,
        neutral_height: LightYears,
        clock: &SupernovaClock,
    ) -> Option<Self> {
        let weaver = Self::of(marks, age, ambient, neutral_height, clock)?;
        let a = age.value();
        let count = marks.mass.value() * clock.per_solar_mass;
        let power = injected_energy(count, a, clock) / (a * SECONDS_PER_JULIAN_YEAR);
        let density = MASS_PER_HYDROGEN_FACTOR * HYDROGEN_MASS_KG * ambient.value() * 1e6;
        let sound = (pressure.value() * BOLTZMANN_CONSTANT * 1e6 / density).sqrt();
        let merge = merge_speed(sound);
        // R_m = α (⅗ α)^(3/2) (L̄ ÷ ρ)^½ v_m^(−3/2), with square roots alone.
        let speed_factor = 0.6 * BUBBLE_COEFFICIENT;
        let merged_m =
            BUBBLE_COEFFICIENT * speed_factor * speed_factor.sqrt() * (power / density).sqrt()
                / (merge * merge.sqrt());
        let merged = merged_m / METRES_PER_LIGHT_YEAR;
        let radius = if merged < weaver.radius.value() {
            merged
        } else {
            weaver.radius.value()
        };
        // The swept-up gas, `n (4/3) π R³`, in the wall from `WALL_INNER_SHARE R` to `R`.
        let wall = ambient.value() * WALL_COMPRESSION;
        (radius > 0.0).then_some(Self {
            radius: LightYears::new(radius),
            interior: weaver.interior,
            wall: Some(HydrogenPerCm3::new(wall)),
        })
    }

    /// The bubble the nursery of `marks` at `centre` in `galaxy` blows at `age`, by the galaxy's
    /// rules: [`of`](Self::of)'s below generator version 29, [`merging`](Self::merging)'s from it,
    /// each in the smooth gas at `centre`. What the features' gas, the recently dead and a
    /// feature's answers read.
    #[must_use]
    pub fn in_galaxy(
        galaxy: &Galaxy,
        centre: &GalacticPosition,
        marks: &NurseryMarks,
        age: Years,
    ) -> Option<Self> {
        let gas = galaxy.gas();
        Self::by_rules(galaxy, marks, age, gas.mean_density(centre), || {
            gas.pressure(centre)
        })
    }

    /// [`in_galaxy`](Self::in_galaxy)'s bubble from the smooth gas at its centre, read by the
    /// caller: the mean density `ambient`, and the thermal pressure `pressure` gives, which only
    /// 29's rules read.
    #[must_use]
    pub(crate) fn by_rules(
        galaxy: &Galaxy,
        marks: &NurseryMarks,
        age: Years,
        ambient: HydrogenPerCm3,
        pressure: impl FnOnce() -> KelvinPerCm3,
    ) -> Option<Self> {
        let height = galaxy.gas().params().neutral_height();
        let clock = galaxy.feature_shares().supernova_clock();
        match galaxy.rules().superbubbles {
            SuperbubbleRules::Weaver => Self::of(marks, age, ambient, height, clock),
            SuperbubbleRules::Merging => {
                Self::merging(marks, age, ambient, pressure(), height, clock)
            }
        }
    }

    /// The bubble's radius.
    #[must_use]
    pub const fn radius(&self) -> LightYears {
        self.radius
    }

    /// Its interior hydrogen density.
    #[must_use]
    pub const fn interior_density(&self) -> HydrogenPerCm3 {
        self.interior
    }

    /// Its interior temperature, [`BUBBLE_TEMPERATURE`].
    #[must_use]
    pub const fn temperature(&self) -> Kelvin {
        BUBBLE_TEMPERATURE
    }

    /// The density of its wall of swept-up gas, or `None` for a bubble with no wall (one of
    /// [`of`](Self::of)'s).
    #[must_use]
    pub const fn wall_density(&self) -> Option<HydrogenPerCm3> {
        self.wall
    }

    /// Its wall's inner radius, [`WALL_INNER_SHARE`] of its radius.
    #[must_use]
    pub fn wall_inner_radius(&self) -> LightYears {
        LightYears::new(self.radius.value() * WALL_INNER_SHARE)
    }

    /// The radius of its hot interior: its wall's inner radius if it has a wall, its radius if not.
    #[must_use]
    pub fn core_radius(&self) -> LightYears {
        match self.wall {
            Some(_) => self.wall_inner_radius(),
            None => self.radius,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::consts::LIGHT_YEARS_PER_PARSEC;

    fn myr(value: f64) -> Years {
        Years::new(value * YEARS_PER_MEGAYEAR)
    }

    pub(crate) fn marks(mass: f64, bound: bool) -> NurseryMarks {
        NurseryMarks {
            age_at_epoch: myr(10.0),
            mass: SolarMasses::new(mass),
            bound,
            embedded_duration: myr(4.0),
            dissolution_age: myr(60.0),
            expansion_speed: KilometresPerSecond::new(3.0),
            age_spread: myr(1.0),
            efficiency: 0.2,
            fe_h: Dex::new(0.0),
            bubble_interior: BUBBLE_INTERIOR_MEDIAN,
            half_mass_radius: LightYears::new(8.0),
            concentration: 1.0,
        }
    }

    #[test]
    fn stages_run_in_order_with_one_change_at_each_boundary() {
        for (m, bound) in [(1e3, false), (1e3, true), (1e5, true)] {
            let nursery = marks(m, bound);
            let mut last = NurseryStage::Unborn;
            let mut changes = 0;
            for k in -10..20_000 {
                let stage = NurseryStage::at(&nursery, myr(f64::from(k) * 0.5));
                if stage != last {
                    assert!(stage > last, "{last:?} to {stage:?}");
                    changes += 1;
                    last = stage;
                }
            }
            assert_eq!(changes, 3, "{m} {bound}");
            assert_eq!(last, NurseryStage::Dissolved);
        }
    }

    #[test]
    fn size_and_gas_are_continuous_and_inside_their_ranges() {
        let nursery = marks(2e3, false);
        let mut last_size = nursery.size_at(Years::ZERO).value();
        let mut last_gas = nursery.gas_mass_at(Years::ZERO).value();
        for k in 1..200_000 {
            let age = myr(f64::from(k) * 0.0005);
            let size = nursery.size_at(age).value();
            let gas = nursery.gas_mass_at(age).value();
            assert!((10.0..=300.0).contains(&size), "{size}");
            assert!(size >= last_size && size - last_size < 0.01, "{age:?}");
            assert!(
                gas <= last_gas && last_gas - gas < 10.0,
                "{age:?}: {last_gas} to {gas}"
            );
            last_size = size;
            last_gas = gas;
        }
    }

    #[test]
    fn a_bubble_grows_as_weaver_and_stops_at_blow_out() {
        let clock = SupernovaClock::new(myr(3.0), myr(40.0), 0.01);
        let nursery = marks(1e4, false);
        let height = LightYears::new(400.0);
        let n = HydrogenPerCm3::new(1.0);
        // Before the first supernova the winds alone blow a bubble; for their 4 Myr the power is
        // constant, so the radius is Mac Low and McCray's 267 pc (L₃₈ ÷ n)^⅕ t₇^⅗, which rounds
        // Weaver's coefficient and the mass per hydrogen to 2%.
        let early = Superbubble::of(&nursery, myr(2.0), n, height, &clock).unwrap();
        let l38 = 100.0 * 1e50 / ((4.0 * 3.155_76e13) * 1e38);
        let mm88 = 267.0 * LIGHT_YEARS_PER_PARSEC * math::powf(l38, 0.2) * math::powf(0.2, 0.6);
        assert!(
            (early.radius().value() / mm88 - 1.0).abs() < 0.03,
            "{early:?} vs {mm88}"
        );
        let r5 = Superbubble::of(&nursery, myr(5.0), n, height, &clock).unwrap();
        let r10 = Superbubble::of(&nursery, myr(10.0), n, height, &clock).unwrap();
        // Later, E(t) holds the winds' 10⁵¹ erg and 7 ÷ 37 of the supernovae's 10⁵³.
        let energy = 100.0 * 1e43 + 100.0 * 1e44 * 7.0 / 37.0;
        let rho = 1.4 * HYDROGEN_MASS_KG * 1e6;
        let t = 1e7 * SECONDS_PER_JULIAN_YEAR;
        let expected = 0.76 * math::powf(energy * t * t / rho, 0.2) / METRES_PER_LIGHT_YEAR;
        assert!((r10.radius().value() / expected - 1.0).abs() < 1e-12);
        assert!(r10.radius().value() > r5.radius().value());
        let capped = Superbubble::of(
            &nursery,
            myr(39.0),
            HydrogenPerCm3::new(1e-3),
            height,
            &clock,
        )
        .unwrap();
        assert!((capped.radius().value() - 1_000.0).abs() < 1e-9);
        assert!(Superbubble::of(&marks(50.0, false), myr(10.0), n, height, &clock).is_none());
    }

    /// The wall's inner share is `(1 − 1 ÷ β²)^⅓` to the last bit but one.
    #[test]
    fn the_wall_begins_where_it_holds_its_sphere_s_gas() {
        let share = math::cbrt(1.0 - 1.0 / WALL_COMPRESSION);
        assert!(
            (share / WALL_INNER_SHARE - 1.0).abs() < 4.0 * f64::EPSILON,
            "{share}"
        );
        assert!((WALL_COMPRESSION - 4.0).abs() < f64::EPSILON);
    }

    /// The superbubbles' merge is held until the 28 → 29 bump: the version's galaxy grows them as
    /// Weaver's, and 29's rules merge them.
    #[test]
    fn the_superbubbles_merge_only_from_29() {
        use crate::GENERATOR_VERSION;
        use crate::galaxy::rules::GalaxyRules;
        assert!(
            GENERATOR_VERSION < SUPERBUBBLE_MERGE_VERSION,
            "the 28 → 29 bump merges the superbubbles and gives them walls: delete \
             `SUPERBUBBLE_MERGE_VERSION`, `SuperbubbleRules` with its `Weaver` arm, its field in \
             `GalaxyRules` and its match in `Superbubble::in_galaxy`, and this test; restate the \
             tests held below 29 (`nurseries_have_sizes_and_bubbles_in_the_plan_s_ranges`' radii)"
        );
        assert_eq!(
            GalaxyRules::of_version(GENERATOR_VERSION).superbubbles,
            SuperbubbleRules::Weaver
        );
        assert_eq!(
            GalaxyRules::every_29_hold().superbubbles,
            SuperbubbleRules::Merging
        );
    }

    /// Under a constant power (the winds' first 4 Myr) a merging bubble is Weaver's until its shell
    /// slows to the merge speed and keeps that radius after; its wall holds the gas of its whole
    /// sphere, and a nursery that blows no bubble blows none merging.
    #[test]
    fn a_merging_bubble_stops_at_the_merge_speed_and_keeps_its_swept_up_gas() {
        let clock = SupernovaClock::new(myr(4.0), myr(40.0), 0.01);
        let nursery = marks(2e3, false);
        let height = LightYears::new(700.0);
        let (n, p) = (HydrogenPerCm3::new(0.7), KelvinPerCm3::new(4_000.0));
        let rho = MASS_PER_HYDROGEN_FACTOR * HYDROGEN_MASS_KG * 0.7e6;
        let merge = merge_speed((4_000.0 * BOLTZMANN_CONSTANT * 1e6 / rho).sqrt());
        // Weaver's shell speed, ⅗ R ÷ t, at each age, against the merge speed.
        let speed = |t_myr: f64| {
            let weaver = Superbubble::of(&nursery, myr(t_myr), n, height, &clock).unwrap();
            0.6 * weaver.radius().value() * METRES_PER_LIGHT_YEAR
                / (t_myr * YEARS_PER_MEGAYEAR * SECONDS_PER_JULIAN_YEAR)
        };
        let (mut lo, mut hi) = (1e-3, 3.99);
        assert!(
            speed(lo) > merge && speed(hi) < merge,
            "the merge falls in the winds' span"
        );
        for _ in 0..80 {
            let mid = f64::midpoint(lo, hi);
            if speed(mid) > merge {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let at_merge = Superbubble::of(&nursery, myr(hi), n, height, &clock)
            .unwrap()
            .radius()
            .value();
        for t in [0.2 * hi, 0.9 * hi] {
            let weaver = Superbubble::of(&nursery, myr(t), n, height, &clock).unwrap();
            let merging = Superbubble::merging(&nursery, myr(t), n, p, height, &clock).unwrap();
            assert_eq!(
                merging.radius(),
                weaver.radius(),
                "{t} Myr, before the merge"
            );
        }
        for t in [1.1 * hi, 3.9] {
            let merging = Superbubble::merging(&nursery, myr(t), n, p, height, &clock).unwrap();
            assert!(
                (merging.radius().value() / at_merge - 1.0).abs() < 1e-9,
                "{t} Myr: {merging:?} against {at_merge}"
            );
            let wall = merging.wall_density().unwrap().value();
            let r = merging.radius().value();
            let inner = merging.wall_inner_radius().value();
            let shell = r * r * r - inner * inner * inner;
            assert!(
                (wall * shell / (0.7 * r * r * r) - 1.0).abs() < 1e-12,
                "{wall}"
            );
            assert!((merging.core_radius().value() - inner).abs() < 1e-12 * r);
            assert!(
                Superbubble::of(&nursery, myr(t), n, height, &clock)
                    .unwrap()
                    .wall_density()
                    .is_none()
            );
        }
        assert!(
            Superbubble::merging(&marks(50.0, false), myr(10.0), n, p, height, &clock).is_none()
        );
    }

    /// D2 and D3 of the galaxy holds: under the version's rules a galaxy's bubbles are Weaver's bit
    /// for bit; under 29's every bubble near the Sun is at most Weaver's and walled, and some are
    /// smaller.
    #[test]
    fn a_galaxy_s_bubbles_merge_under_29_s_rules_only() {
        use crate::Seed;
        use crate::galaxy::features::catalogue::{FeatureCatalogue, FeatureMarks};
        use crate::galaxy::params::GalaxyParams;
        use crate::galaxy::rules::GalaxyRules;
        use crate::id::FeatureCell;
        let seed = Seed::new(0x0905_b0bb_0000_0001);
        let held = Galaxy::from_params(seed, GalaxyParams::milky_way_like()).unwrap();
        let merging = Galaxy::from_params_with_rules(
            seed,
            GalaxyParams::milky_way_like(),
            GalaxyRules::every_29_hold(),
        )
        .unwrap();
        let gas = held.gas();
        let clock = held.feature_shares().supernova_clock();
        let (mut bubbles, mut smaller) = (0, 0);
        for cell in [[0, 6, 0], [0, 6, -1], [-1, 6, 0]] {
            let cell = FeatureCell::new(cell).unwrap();
            for feature in FeatureCatalogue::cell(&held, cell).features() {
                let FeatureMarks::Nursery(marks) = feature.marks() else {
                    continue;
                };
                let (centre, age) = (feature.position(), marks.age_at_epoch());
                let weaver = Superbubble::of(
                    marks,
                    age,
                    gas.mean_density(centre),
                    gas.params().neutral_height(),
                    clock,
                );
                assert_eq!(Superbubble::in_galaxy(&held, centre, marks, age), weaver);
                let new = Superbubble::in_galaxy(&merging, centre, marks, age);
                assert_eq!(new.is_some(), weaver.is_some());
                let (Some(weaver), Some(new)) = (weaver, new) else {
                    continue;
                };
                bubbles += 1;
                assert!(new.radius() <= weaver.radius(), "{new:?} {weaver:?}");
                assert!(new.radius().value() > 0.0 && new.wall_density().is_some());
                smaller += u32::from(new.radius() < weaver.radius());
            }
        }
        assert!(bubbles > 20, "{bubbles} bubbles");
        assert!(smaller * 2 > bubbles, "{smaller} of {bubbles} smaller");
    }

    #[test]
    fn g_and_the_embedded_share_are_the_uniform_distributions() {
        assert!(dissolved_share(myr(29.0)).abs() < 1e-15);
        assert!((dissolved_share(myr(65.0)) - 0.5).abs() < 1e-12);
        assert!((dissolved_share(myr(101.0)) - 1.0).abs() < 1e-15);
        assert!((embedded_share(myr(2.0)) - 1.0).abs() < 1e-15);
        assert!((embedded_share(myr(4.0)) - 0.5).abs() < 1e-12);
        assert!(embedded_share(myr(5.0)).abs() < 1e-15);
    }
}
