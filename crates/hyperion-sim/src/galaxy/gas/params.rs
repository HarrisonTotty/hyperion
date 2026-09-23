//! The gas parameters of one galaxy: plan 02's gas disc, and the rest drawn here (Design note 3).
//!
//! Plan 02 draws three of them, because its potential needs them: the gas disc's mass, its scale
//! length and its fixed scale height ([`GasDiscParams`](crate::galaxy::params::GasDiscParams)).
//! Those are read, never redrawn, so that nothing plan 02 generates moves. Everything else the
//! field needs is drawn here, on the one domain tag [`tags::GAS_PARAMS`], with the galaxy as the
//! object and one fixed word index per parameter, so that a parameter added later appends an index
//! and moves no other.
//!
//! Two parameters are constants of the generator version rather than draws, because nothing
//! measures them per galaxy: the pressure's scale height [`GasParams::PRESSURE_HEIGHT`] and the
//! velocity dispersion [`GasParams::PRESSURE_SPEED`] that Design note 11's hydrostatic pressure is
//! calibrated with.
//!
//! The ranges are the brainstorm's where it states one ("Between the stars", the bullet "Dust and
//! gas": a warm layer of about 0.03 cm⁻³ with a scale height near 3,000 ly, a corona of about 10⁻³
//! cm⁻³, a log-normal of σ 2–2.5) and Design notes 3 and 5's otherwise. The radial form, the hole
//! inside the bar included, is the one McMillan (2017, MNRAS 465, 76) fits the Milky Way's gas
//! discs with, `exp(−R_m ÷ R − R ÷ R_d)`: his neutral disc has `R_m` = 4 kpc and `R_d` = 7 kpc,
//! both inside the ranges here (`R_m` 2.4–6.6 kpc, `R_g` 3.2–7.1 kpc). His molecular disc,
//! `R_m` = 12 kpc and `R_d` = 1.5 kpc, is not this model's central molecular disc, which is the
//! diffuse part of the central molecular zone (Design note 5) and sits where the nuclear disc is.

use crate::Seed;
use crate::galaxy::params::GalaxyParams;
use crate::math;
use crate::rng::{ObjectKey, Stream, tags};
use crate::units::{HydrogenPerCm3, KelvinPerCm3, KilometresPerSecond, LightYears, SolarMasses};

/// How one gas parameter is drawn from its word, and the range every draw lies in.
///
/// Both laws cost exactly one word, which is what makes a parameter's word index its place in
/// [`Drawn::ALL`].
#[derive(Debug, Clone, Copy, PartialEq)]
enum Law {
    /// Uniform on `[lo, hi]`: [`Stream::uniform_in`].
    Uniform { lo: f64, hi: f64 },
    /// Log-uniform on `[lo, hi]`: `lo × (hi ÷ lo)^u` for one uniform `u`, clamped, as plan 02's
    /// own log-uniform laws are written.
    LogUniform { lo: f64, hi: f64 },
}

impl Law {
    /// One draw from `stream`, which consumes exactly one word.
    fn draw(self, stream: &mut Stream) -> f64 {
        match self {
            Self::Uniform { lo, hi } => stream.uniform_in(lo, hi),
            Self::LogUniform { lo, hi } => {
                (lo * math::exp(stream.uniform() * math::ln(hi / lo))).clamp(lo, hi)
            }
        }
    }

    /// The closed range every draw lies in. Only the tests read it so far; P07.T10.b's parameter
    /// table will report it on the wire.
    #[cfg(test)]
    const fn bounds(self) -> (f64, f64) {
        match self {
            Self::Uniform { lo, hi } | Self::LogUniform { lo, hi } => (lo, hi),
        }
    }
}

/// The number of parameters this plan draws: Design note 3's table less plan 02's three rows and
/// the four rows that are derived or constant.
const DRAWN_COUNT: usize = 11;

/// One parameter drawn here. Its word on [`tags::GAS_PARAMS`] is its place in [`Drawn::ALL`], so
/// the array's order is part of the generator version and a new parameter is appended to it.
///
/// Words 11–15 are reserved for later gas parameters (plan 07, "Generator version"), and a
/// parameter that needs a standard normal, which costs two words, appends two.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Drawn {
    /// The hole scale `R_m` over the bar's half-length.
    HoleRatio,
    /// The warm ionised layer's share `f_w` of the gas mass.
    WarmFraction,
    /// The warm ionised layer's scale height `h_w`, ly.
    WarmHeight,
    /// The molecular disc's share `f_c` of the gas mass.
    MolecularFraction,
    /// The molecular disc's scale height `h_c` over its scale length `R_c`.
    MolecularHeightRatio,
    /// The corona's density `n_cor`, cm⁻³.
    CoronaDensity,
    /// The pressure floor `P_cor` ÷ k, K cm⁻³.
    PressureFloor,
    /// The log-normal's width `σ_ln` in the natural logarithm.
    SigmaLn,
    /// The lanes' inward offset d, ly.
    LaneOffset,
    /// The lanes' width `σ_w`, ly.
    LaneWidth,
    /// The share A of the neutral gas the lanes gather.
    LaneFraction,
}

impl Drawn {
    /// Every drawn parameter, in Design note 3's table order, which is the word order.
    const ALL: [Self; DRAWN_COUNT] = [
        Self::HoleRatio,
        Self::WarmFraction,
        Self::WarmHeight,
        Self::MolecularFraction,
        Self::MolecularHeightRatio,
        Self::CoronaDensity,
        Self::PressureFloor,
        Self::SigmaLn,
        Self::LaneOffset,
        Self::LaneWidth,
        Self::LaneFraction,
    ];

    /// The word this parameter is drawn from, its place in [`Drawn::ALL`].
    const fn word(self) -> u64 {
        match self {
            Self::HoleRatio => 0,
            Self::WarmFraction => 1,
            Self::WarmHeight => 2,
            Self::MolecularFraction => 3,
            Self::MolecularHeightRatio => 4,
            Self::CoronaDensity => 5,
            Self::PressureFloor => 6,
            Self::SigmaLn => 7,
            Self::LaneOffset => 8,
            Self::LaneWidth => 9,
            Self::LaneFraction => 10,
        }
    }

    /// The law and range of Design note 3's table.
    ///
    /// The hole scale follows the bar, whose ends the arms and the gas lanes start from; the warm
    /// layer's height is the brainstorm's "near 3,000 ly"; the molecular share is Design note 5's
    /// 2–3 × 10⁶ M☉ of diffuse central gas as a share of a few 10⁹; the corona's range is the
    /// brainstorm's 10⁻³ cm⁻³, trimmed above so that the corona comes out hot for every seed
    /// (Design note 12 and the plan's Risks); `σ_ln` is the brainstorm's 2–2.5.
    const fn law(self) -> Law {
        match self {
            Self::HoleRatio => Law::Uniform { lo: 0.8, hi: 1.2 },
            Self::WarmFraction => Law::Uniform { lo: 0.20, hi: 0.30 },
            Self::WarmHeight => Law::Uniform {
                lo: 2_500.0,
                hi: 3_500.0,
            },
            Self::MolecularFraction => Law::LogUniform {
                lo: 3e-4,
                hi: 10e-4,
            },
            Self::MolecularHeightRatio => Law::Uniform { lo: 0.15, hi: 0.25 },
            Self::CoronaDensity => Law::LogUniform {
                lo: 0.5e-3,
                hi: 1.2e-3,
            },
            Self::PressureFloor => Law::Uniform {
                lo: 300.0,
                hi: 500.0,
            },
            Self::SigmaLn => Law::Uniform { lo: 2.0, hi: 2.5 },
            Self::LaneOffset => Law::Uniform {
                lo: 300.0,
                hi: 600.0,
            },
            Self::LaneWidth => Law::Uniform {
                lo: 150.0,
                hi: 300.0,
            },
            Self::LaneFraction => Law::Uniform { lo: 0.08, hi: 0.20 },
        }
    }

    /// The parameter's name, as the plan's table and the wire's `gas.*` keys have it. Only the
    /// tests read it so far; P07.T10.b's parameter table will send it.
    #[cfg(test)]
    const fn name(self) -> &'static str {
        match self {
            Self::HoleRatio => "hole_ratio",
            Self::WarmFraction => "warm_fraction",
            Self::WarmHeight => "warm_height",
            Self::MolecularFraction => "molecular_fraction",
            Self::MolecularHeightRatio => "molecular_height_ratio",
            Self::CoronaDensity => "corona_density",
            Self::PressureFloor => "pressure_floor",
            Self::SigmaLn => "sigma_ln",
            Self::LaneOffset => "lane_offset",
            Self::LaneWidth => "lane_width",
            Self::LaneFraction => "lane_fraction",
        }
    }

    /// This parameter's draw from `stream`, whatever the stream's position: it seeks to the
    /// parameter's own word first, so the order of the draws is immaterial.
    fn draw(self, stream: &mut Stream) -> f64 {
        stream.seek(self.word());
        self.law().draw(stream)
    }
}

/// The central molecular disc: the diffuse part of the central molecular zone, where the nuclear
/// disc is (brainstorm, "Between the stars"; Design note 5).
///
/// Its mass is a few thousandths of a per cent of the gas, not the 3–5 × 10⁷ M☉ the real zone
/// holds, because the real mass is in a few dense clouds that a line of sight to the centre mostly
/// misses: the clouds are plan 09's features and carry their own dust. What is left here is what
/// puts the brainstorm's thirty magnitudes in front of the centre.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MolecularDisc {
    fraction: f64,
    mass: SolarMasses,
    length: LightYears,
    height: LightYears,
}

impl MolecularDisc {
    /// Its share `f_c` of the gas mass, 3–10 × 10⁻⁴.
    #[must_use]
    pub fn fraction(&self) -> f64 {
        self.fraction
    }

    /// Its mass: the share times the gas disc's mass.
    #[must_use]
    pub fn mass(&self) -> SolarMasses {
        self.mass
    }

    /// Its radial scale length `R_c`, the nuclear disc's.
    #[must_use]
    pub fn length(&self) -> LightYears {
        self.length
    }

    /// Its scale height `h_c`, 0.15–0.25 of the scale length.
    #[must_use]
    pub fn height(&self) -> LightYears {
        self.height
    }
}

/// The dust lanes: the arm geometry again, at a radius shifted inward (Design note 6).
///
/// The lanes sit on the arms' inner, concave edges, where the gas overtakes the pattern inside
/// corotation and shocks. They gather a share of the neutral gas and move none of it: the factor
/// averages exactly 1 around every circle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LaneParams {
    offset: LightYears,
    width: LightYears,
    fraction: f64,
}

impl LaneParams {
    /// The inward offset d, 300–600 ly, measured perpendicular to the arm.
    #[must_use]
    pub fn offset(&self) -> LightYears {
        self.offset
    }

    /// The lane's Gaussian width `σ_w`, 150–300 ly: narrower than the young stars' arms.
    #[must_use]
    pub fn width(&self) -> LightYears {
        self.width
    }

    /// The share A of the neutral gas gathered into the lanes, 0.08–0.20.
    #[must_use]
    pub fn fraction(&self) -> f64 {
        self.fraction
    }
}

/// Every parameter the gas field needs: plan 02's three, and the eleven drawn here.
///
/// It is immutable and a pure function of the seed, the galaxy's parameters and the generator
/// version. Lengths are light-years, the mass is solar masses, densities are hydrogen nuclei per
/// cubic centimetre and pressures are P ÷ k in K cm⁻³ (Design note 2).
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::gas::params::GasParams;
/// use hyperion_sim::galaxy::imf::MassFunctionKind;
/// use hyperion_sim::galaxy::params::GalaxyParams;
///
/// let seed = Seed::new(0x0700_5eed);
/// let galaxy = GalaxyParams::from_seed(seed, MassFunctionKind::default());
/// let gas = GasParams::from_galaxy(seed, &galaxy);
/// // The mass and the scale length are plan 02's own, never redrawn.
/// assert_eq!(gas.gas_mass(), galaxy.gas_disc().mass());
/// assert_eq!(gas.radial_scale(), galaxy.gas_disc().length());
/// // The three phases share the mass: neutral, warm ionised and molecular.
/// let shares = gas.neutral_fraction() + gas.warm_fraction() + gas.molecular_disc().fraction();
/// assert!((shares - 1.0).abs() < 1e-15);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GasParams {
    gas_mass: SolarMasses,
    radial_scale: LightYears,
    hole_scale: LightYears,
    neutral_height: LightYears,
    warm_fraction: f64,
    warm_height: LightYears,
    molecular: MolecularDisc,
    corona_density: HydrogenPerCm3,
    pressure_floor: KelvinPerCm3,
    sigma_ln: f64,
    lane: LaneParams,
}

impl GasParams {
    /// The scale height `h_P` of the pressure above the plane, a constant of the generator version.
    ///
    /// The thermal pressure falls more slowly with height than the neutral gas does, because the
    /// layers that hold it are thicker; 1,500 ly is between the neutral and the warm layer's
    /// heights (Design note 11).
    pub const PRESSURE_HEIGHT: LightYears = LightYears::new(1_500.0);

    /// The velocity dispersion `σ_P` of Design note 11's hydrostatic pressure, a constant of the
    /// generator version.
    ///
    /// 5.5 km/s is calibrated so that the Milky Way fixture's mid-plane pressure at 26,000 ly is
    /// the measured 3,800 K cm⁻³ (Jenkins and Tripp 2011, ApJ 734, 65), which P07.T5 checks. It is
    /// of the order of the neutral medium's turbulent and thermal speeds, as a hydrostatic layer
    /// requires.
    pub const PRESSURE_SPEED: KilometresPerSecond = KilometresPerSecond::new(5.5);

    /// The gas parameters of the galaxy `params`, with `seed` keying this plan's own draws.
    ///
    /// The draws are on [`tags::GAS_PARAMS`] with [`ObjectKey::galaxy`], one word per parameter at
    /// its fixed index, so nothing plan 02 drew moves and the order of the draws below is
    /// immaterial.
    #[must_use]
    pub fn from_galaxy(seed: Seed, params: &GalaxyParams) -> Self {
        let mut stream = Stream::open(seed, tags::GAS_PARAMS, ObjectKey::galaxy());
        let drawn = core::array::from_fn(|i| Drawn::ALL[i].draw(&mut stream));
        Self::of_galaxy(params, drawn)
    }

    /// The Milky Way fixture, pairing with
    /// [`GalaxyParams::milky_way_like`](crate::galaxy::params::GalaxyParams::milky_way_like).
    ///
    /// Its values are Design note 3's table: the gas disc is plan 02's fixture (5.1 × 10⁹ M☉,
    /// 14,840 ly long, 400 ly tall), the hole scale is the bar's half-length of 16,000 ly, the
    /// molecular disc is the nuclear disc's 290 ly by 58 ly, and the rest are the measured values
    /// where the brainstorm states one and the middle of the range otherwise. P07.T12 tunes them
    /// against the brainstorm's targets.
    #[must_use]
    pub fn milky_way_like() -> Self {
        Self::of_galaxy(&GalaxyParams::milky_way_like(), MILKY_WAY_DRAWN)
    }

    /// The parameters of `params` with `drawn` in Design note 3's table order: what
    /// [`from_galaxy`](Self::from_galaxy) and [`milky_way_like`](Self::milky_way_like) share.
    fn of_galaxy(params: &GalaxyParams, drawn: [f64; DRAWN_COUNT]) -> Self {
        let [
            hole_ratio,
            warm_fraction,
            warm_height,
            molecular_fraction,
            molecular_height_ratio,
            corona_density,
            pressure_floor,
            sigma_ln,
            lane_offset,
            lane_width,
            lane_fraction,
        ] = drawn;
        let disc = params.gas_disc();
        let molecular_length = params.nuclear_disc().length();
        Self {
            gas_mass: disc.mass(),
            radial_scale: disc.length(),
            hole_scale: params.bar().half_length() * hole_ratio,
            neutral_height: disc.height(),
            warm_fraction,
            warm_height: LightYears::new(warm_height),
            molecular: MolecularDisc {
                fraction: molecular_fraction,
                mass: disc.mass() * molecular_fraction,
                length: molecular_length,
                height: molecular_length * molecular_height_ratio,
            },
            corona_density: HydrogenPerCm3::new(corona_density),
            pressure_floor: KelvinPerCm3::new(pressure_floor),
            sigma_ln,
            lane: LaneParams {
                offset: LightYears::new(lane_offset),
                width: LightYears::new(lane_width),
                fraction: lane_fraction,
            },
        }
    }

    /// The gas disc's mass, plan 02's
    /// [`GasDiscParams::mass`](crate::galaxy::params::GasDiscParams::mass): 10–20% of the whole
    /// thin disc's stellar mass, and the budget the three phases share.
    #[must_use]
    pub fn gas_mass(&self) -> SolarMasses {
        self.gas_mass
    }

    /// The radial scale length `R_g`, plan 02's
    /// [`GasDiscParams::length`](crate::galaxy::params::GasDiscParams::length): 1.5–2 times the
    /// thin disc's.
    #[must_use]
    pub fn radial_scale(&self) -> LightYears {
        self.radial_scale
    }

    /// The hole scale `R_m` of `exp(−R_m ÷ R)`, 0.8–1.2 of the bar's half-length.
    ///
    /// The neutral disc peaks at √(`R_m` `R_g`), near the bar's end, which is where the arms and
    /// their lanes start (McMillan 2017; Design note 4).
    #[must_use]
    pub fn hole_scale(&self) -> LightYears {
        self.hole_scale
    }

    /// The neutral layer's scale height `h_n`, plan 02's
    /// [`GasDiscParams::HEIGHT`](crate::galaxy::params::GasDiscParams::HEIGHT) of 400 ly: the
    /// brainstorm's "a thin neutral disc a few hundred light-years tall".
    #[must_use]
    pub fn neutral_height(&self) -> LightYears {
        self.neutral_height
    }

    /// The neutral layer's share of the gas mass, `1 − f_w − f_c`.
    #[must_use]
    pub fn neutral_fraction(&self) -> f64 {
        1.0 - self.warm_fraction - self.molecular.fraction
    }

    /// The warm ionised layer's share `f_w` of the gas mass, 0.20–0.30.
    #[must_use]
    pub fn warm_fraction(&self) -> f64 {
        self.warm_fraction
    }

    /// The warm ionised layer's scale height `h_w`, 2,500–3,500 ly: the brainstorm's "near
    /// 3,000 ly".
    #[must_use]
    pub fn warm_height(&self) -> LightYears {
        self.warm_height
    }

    /// The central molecular disc.
    #[must_use]
    pub fn molecular_disc(&self) -> MolecularDisc {
        self.molecular
    }

    /// The hot corona's density `n_cor`, 0.5–1.2 × 10⁻³ cm⁻³: the brainstorm's "a hot corona of
    /// about 10⁻³".
    ///
    /// It is an additive floor and carries no dust, because grains do not survive in it (Design
    /// note 10). The range stops at 1.2 × 10⁻³ so that the corona is hot for every seed even at
    /// the lowest pressure floor (Design note 12).
    #[must_use]
    pub fn corona_density(&self) -> HydrogenPerCm3 {
        self.corona_density
    }

    /// The pressure floor `P_cor` ÷ k, 300–500 K cm⁻³: what the pressure falls to far from the
    /// plane.
    ///
    /// The floor is what caps a supernova shell's observable window at the brainstorm's 2–4 Myr,
    /// which P07.T12 pins (see the plan's Risks for why it and the corona's density are independent
    /// parameters).
    #[must_use]
    pub fn pressure_floor(&self) -> KelvinPerCm3 {
        self.pressure_floor
    }

    /// The pressure's scale height `h_P`, [`PRESSURE_HEIGHT`](Self::PRESSURE_HEIGHT).
    #[must_use]
    pub fn pressure_height(&self) -> LightYears {
        Self::PRESSURE_HEIGHT
    }

    /// The pressure's velocity dispersion `σ_P`, [`PRESSURE_SPEED`](Self::PRESSURE_SPEED).
    #[must_use]
    pub fn pressure_speed(&self) -> KilometresPerSecond {
        Self::PRESSURE_SPEED
    }

    /// The log-normal's width `σ_ln` in the natural logarithm, 2.0–2.5: the brainstorm's "σ of
    /// 2–2.5 in the logarithm", wide enough that the volume runs from hot rarefied gas to cloud.
    #[must_use]
    pub fn sigma_ln(&self) -> f64 {
        self.sigma_ln
    }

    /// The dust lanes' offset, width and share.
    #[must_use]
    pub fn lane(&self) -> LaneParams {
        self.lane
    }
}

/// The Milky Way fixture's draws, in [`Drawn::ALL`]'s order (Design note 3's `MW` column).
const MILKY_WAY_DRAWN: [f64; DRAWN_COUNT] = [
    // The hole scale is the bar's half-length itself, 16,000 ly, which puts the neutral disc's
    // peak at 15,400 ly.
    1.0, 0.25, 3_000.0,
    // 2.5 × 10⁶ M☉ of diffuse central gas against the fixture's 5.1 × 10⁹ M☉ (Design note 5).
    5e-4,
    // 58 ly against the nuclear disc's 290 ly, half of Sormani et al.'s (2022) 28 pc height.
    0.20, 1e-3, 400.0, 2.3, 450.0, 200.0, 0.12,
];

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::super::IONISED_PARTICLES_PER_HYDROGEN;
    use super::*;
    use crate::galaxy::imf::MassFunctionKind;

    /// Seeds of the sweeps: the same form plan 02's tests use, with this plan's own prefix.
    fn seeds(count: u64) -> impl Iterator<Item = Seed> {
        (0..count).map(|n| Seed::new(0x0700_5eed_0000_0000 | n))
    }

    fn galaxy(seed: Seed) -> GalaxyParams {
        GalaxyParams::from_seed(seed, MassFunctionKind::default())
    }

    /// Every getter against Design note 3's table, written out here independently of [`Drawn`]'s
    /// own laws so that a wrong law or a wrong word fails the test rather than moving the range.
    #[track_caller]
    fn assert_in_ranges(galaxy: &GalaxyParams, gas: &GasParams) {
        let within = |what: &str, value: f64, lo: f64, hi: f64| {
            assert!(
                (lo..=hi).contains(&value),
                "{what} = {value} lies outside [{lo}, {hi}]"
            );
        };
        // Plan 02's three rows, read and never redrawn.
        assert_eq!(gas.gas_mass(), galaxy.gas_disc().mass());
        assert_eq!(gas.radial_scale(), galaxy.gas_disc().length());
        assert_eq!(gas.neutral_height(), galaxy.gas_disc().height());

        within(
            "hole scale over the bar",
            gas.hole_scale() / galaxy.bar().half_length(),
            0.8,
            1.2,
        );
        within("warm fraction", gas.warm_fraction(), 0.20, 0.30);
        within("warm height", gas.warm_height().value(), 2_500.0, 3_500.0);
        let molecular = gas.molecular_disc();
        within("molecular fraction", molecular.fraction(), 3e-4, 10e-4);
        assert_eq!(molecular.length(), galaxy.nuclear_disc().length());
        within(
            "molecular height ratio",
            molecular.height() / molecular.length(),
            0.15,
            0.25,
        );
        within(
            "molecular mass",
            molecular.mass() / gas.gas_mass(),
            3e-4,
            10e-4,
        );
        within(
            "corona density",
            gas.corona_density().value(),
            0.5e-3,
            1.2e-3,
        );
        within("pressure floor", gas.pressure_floor().value(), 300.0, 500.0);
        assert_eq!(gas.pressure_height(), GasParams::PRESSURE_HEIGHT);
        assert_eq!(gas.pressure_speed(), GasParams::PRESSURE_SPEED);
        within("sigma_ln", gas.sigma_ln(), 2.0, 2.5);
        let lane = gas.lane();
        within("lane offset", lane.offset().value(), 300.0, 600.0);
        within("lane width", lane.width().value(), 150.0, 300.0);
        within("lane fraction", lane.fraction(), 0.08, 0.20);
        // The three phases share the whole gas mass.
        let shares = gas.neutral_fraction() + gas.warm_fraction() + molecular.fraction();
        assert!((shares - 1.0).abs() < 1e-15, "shares sum to {shares}");
        within("neutral fraction", gas.neutral_fraction(), 0.69, 0.80);
    }

    /// Design note 3's table over 2,000 seeds. The galaxy's own parameters enter only as the three
    /// rows that are read and the two the draws scale (the bar's half-length and the nuclear
    /// disc's length), so the sweep draws 2,000 seeds against one galaxy and 32 seeds against
    /// their own, which is what plan 02's own fast sweep costs.
    #[test]
    fn every_parameter_lies_in_its_range_over_2000_seeds() {
        let fixture = GalaxyParams::milky_way_like();
        for seed in seeds(2_000) {
            assert_in_ranges(&fixture, &GasParams::from_galaxy(seed, &fixture));
        }
        for seed in seeds(32) {
            let galaxy = galaxy(seed);
            assert_in_ranges(&galaxy, &GasParams::from_galaxy(seed, &galaxy));
        }
    }

    #[test]
    fn the_fixtures_values_lie_in_their_ranges() {
        let galaxy = GalaxyParams::milky_way_like();
        let gas = GasParams::milky_way_like();
        assert_in_ranges(&galaxy, &gas);
        // Design note 3's `MW` column, as far as the fixture's own galaxy fixes it.
        assert_eq!(gas.hole_scale(), LightYears::new(16_000.0));
        assert_eq!(gas.molecular_disc().length(), LightYears::new(290.0));
        assert_eq!(gas.molecular_disc().height(), LightYears::new(58.0));
        assert_eq!(gas.neutral_height(), LightYears::new(400.0));
        assert!((gas.radial_scale().value() - 14_840.0).abs() < 1e-9);
        assert!((gas.gas_mass().value() / 5.1e9 - 1.0).abs() < 0.02);
    }

    #[test]
    fn the_same_seed_gives_the_same_parameters_and_others_differ() {
        let fixture = GalaxyParams::milky_way_like();
        let seed = Seed::new(0x0700_5eed_0000_0007);
        let once = GasParams::from_galaxy(seed, &fixture);
        assert_eq!(once, GasParams::from_galaxy(seed, &fixture));
        let other = GasParams::from_galaxy(Seed::new(seed.get() + 1), &fixture);
        assert_ne!(once, other);
    }

    /// Each parameter is drawn from its own word, so drawing them in any order, or drawing one
    /// alone, gives the same value (Design note 3).
    #[test]
    fn a_parameter_is_the_word_at_its_own_index() {
        let seed = Seed::new(0x0700_5eed_0000_0003);
        let open = || Stream::open(seed, tags::GAS_PARAMS, ObjectKey::galaxy());
        for (i, parameter) in Drawn::ALL.iter().enumerate() {
            assert_eq!(parameter.word(), u64::try_from(i).unwrap());
            let alone = parameter.draw(&mut open());
            let mut backwards = open();
            let mut last = f64::NAN;
            for other in Drawn::ALL.iter().rev() {
                let value = other.draw(&mut backwards);
                if other == parameter {
                    last = value;
                }
            }
            assert_same_bits(alone, last);
            let (lo, hi) = parameter.law().bounds();
            assert!(
                (lo..=hi).contains(&alone),
                "{} = {alone} outside [{lo}, {hi}]",
                parameter.name()
            );
        }
    }

    /// The corona alone must classify as hot, so that the gas far from the disc is hot for every
    /// seed: T = (`P_cor` ÷ k) ÷ (2.3 `n_cor`) above 10⁵ K (Design note 12). The worst case is the
    /// lowest floor against the highest density, which is why the density's range stops at
    /// 1.2 × 10⁻³ cm⁻³.
    #[test]
    fn the_corona_is_hotter_than_a_hundred_thousand_kelvin_for_every_seed() {
        let temperature = |gas: &GasParams| {
            gas.pressure_floor().value()
                / (IONISED_PARTICLES_PER_HYDROGEN * gas.corona_density().value())
        };
        let fixture = GalaxyParams::milky_way_like();
        for seed in seeds(2_000) {
            let gas = GasParams::from_galaxy(seed, &fixture);
            let hot = temperature(&gas);
            assert!(hot > 1e5, "the corona is {hot} K for {seed:?}");
        }
        // The extremes of the two ranges, which no sweep of seeds is sure to reach.
        let worst = 300.0 / (IONISED_PARTICLES_PER_HYDROGEN * 1.2e-3);
        assert!(worst > 1e5, "the worst case is {worst} K");
        assert!(temperature(&GasParams::milky_way_like()) > 1.5e5);
    }
}
