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
//! The warm ionised layer and the central molecular disc are drawn in absolute terms — the warm
//! layer by its mid-plane density at the Sun's radius, the molecular disc by its mass — because the
//! brainstorm and Design note 5 state them so, and the neutral disc takes the rest of plan 02's gas
//! mass (plan 07, ruling 19 of 2026-09-22). Drawn as shares of that mass, as they first were, the
//! two layers moved whenever plan 02's gas mass did, which is a quantity their measurements have
//! nothing to do with, and plan 02's own spread of gas masses multiplied into the warm layer's
//! density.
//!
//! The ranges are the brainstorm's where it states one ("Between the stars", the bullet "Dust and
//! gas": a warm layer of about 0.03 cm⁻³ with a scale height near 3,000 ly, a corona of about 10⁻³
//! cm⁻³, a log-normal of σ 2–2.5) and Design notes 3 and 5's otherwise. The radial form, the hole
//! inside the bar included, is the one McMillan (2017, MNRAS 465, 76) fits the Milky Way's gas
//! discs with, `exp(−R_m ÷ R − R ÷ R_d)`: his neutral disc has `R_m` = 4 kpc and `R_d` = 7 kpc,
//! both inside the ranges here (`R_m` 2.4–6.6 kpc, `R_g` 3.2–7.1 kpc). His molecular disc,
//! `R_m` = 12 kpc and `R_d` = 1.5 kpc, is not this model's central molecular disc, which is the
//! diffuse part of the central molecular zone (Design note 5) and sits where the nuclear disc is.

use std::error::Error;
use std::fmt;

use crate::Seed;
use crate::galaxy::gas::smooth;
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
    /// The warm ionised layer's mid-plane density at the Sun's radius, cm⁻³.
    WarmDensity,
    /// The warm ionised layer's scale height `h_w`, ly.
    WarmHeight,
    /// The molecular disc's mass, M☉.
    MolecularMass,
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
        Self::WarmDensity,
        Self::WarmHeight,
        Self::MolecularMass,
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
            Self::WarmDensity => 1,
            Self::WarmHeight => 2,
            Self::MolecularMass => 3,
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
    /// The hole scale follows the bar, whose ends the arms and the gas lanes start from. The warm
    /// layer is the brainstorm's "about 0.03 atoms per cubic centimetre with a scale height near
    /// 3,000 ly", drawn independently in density and height: the two ranges together give a column
    /// of 19–38 cm⁻³ pc from the plane, around the pulsars' measured 24.4 cm⁻³ pc (Schnitzeler
    /// 2012, as McKee, Parravano and Hollenbach 2015, ApJ 814, 13, Table 2 adopt it: 0.0154 cm⁻³
    /// over 1,590 pc, the same column in a taller, thinner layer). The molecular disc's mass is
    /// Design note 5's 2–3 × 10⁶ M☉ of diffuse central gas. The corona's range is the brainstorm's
    /// "about 10⁻³ cm⁻³", trimmed to 0.5–0.8 × 10⁻³ so that the corona comes out hot 20,000 ly
    /// above every radius from 8,000 to 40,000 ly for every draw (ruling 91 of 2026-09-22; Design
    /// note 12); Miller and Bregman's (2015, ApJ 800, 14) β-model of the hot halo, `n₀ r_c^(3β)` of
    /// 1.35 × 10⁻² cm⁻³ kpc^(3β) with β = 0.50, gives 4–7 × 10⁻⁴ cm⁻³ at 7–10 kpc from the centre.
    /// `σ_ln` is the brainstorm's 2–2.5.
    const fn law(self) -> Law {
        match self {
            Self::HoleRatio => Law::Uniform { lo: 0.8, hi: 1.2 },
            Self::WarmDensity => Law::Uniform {
                lo: 0.025,
                hi: 0.035,
            },
            Self::WarmHeight => Law::Uniform {
                lo: 2_500.0,
                hi: 3_500.0,
            },
            Self::MolecularMass => Law::LogUniform { lo: 2e6, hi: 3e6 },
            Self::MolecularHeightRatio => Law::Uniform { lo: 0.15, hi: 0.25 },
            Self::CoronaDensity => Law::LogUniform {
                lo: 0.5e-3,
                hi: 0.8e-3,
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
            Self::WarmDensity => "warm_density",
            Self::WarmHeight => "warm_height",
            Self::MolecularMass => "molecular_mass",
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
/// Its mass is 2–3 × 10⁶ M☉, a few hundredths of a per cent of the gas, not the 3–5 × 10⁷ M☉ the
/// real zone holds, because the real mass is in a few dense clouds that a line of sight to the centre mostly
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
    /// Its share `f_c` of the gas mass, derived: its mass over plan 02's gas mass, a few 10⁻⁴.
    #[must_use]
    pub fn fraction(&self) -> f64 {
        self.fraction
    }

    /// Its mass, 2–3 × 10⁶ M☉, drawn (Design note 5).
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

/// A galaxy's gas parameters could not be built (plan 07, ruling 22 of 2026-09-22).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildGasParamsError {
    /// The warm ionised layer and the central molecular disc drawn for the galaxy weigh at least as
    /// much as its whole gas disc, which leaves the neutral layer nothing.
    ///
    /// Since ruling 91 of 2026-09-22 the warm layer is clamped to half the gas less the molecular
    /// disc, so this is refused only where the molecular disc alone weighs half the gas: a gas
    /// disc under 6 × 10⁶ M☉, four hundred times lighter than the lightest plan 02's ranges
    /// allow (`the_neutral_share_is_at_least_half_at_every_corner_of_the_draws`). No galaxy the
    /// public API builds reaches it; the variant stays so that a later range cannot turn it into a
    /// panic (ruling 22).
    NoNeutralGas {
        /// Plan 02's gas disc mass.
        gas_mass: SolarMasses,
        /// The warm ionised layer's mass, from its drawn density and height.
        warm_mass: SolarMasses,
        /// The molecular disc's drawn mass.
        molecular_mass: SolarMasses,
    },
}

impl fmt::Display for BuildGasParamsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoNeutralGas {
                gas_mass,
                warm_mass,
                molecular_mass,
            } => write!(
                f,
                "a warm ionised layer of {:.3e} M☉ and a molecular disc of {:.3e} M☉ leave nothing \
                 of a {:.3e} M☉ gas disc for the neutral layer",
                warm_mass.value(),
                molecular_mass.value(),
                gas_mass.value()
            ),
        }
    }
}

impl Error for BuildGasParamsError {}

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
/// let gas = GasParams::from_galaxy(seed, &galaxy)?;
/// // The mass and the scale length are plan 02's own, never redrawn.
/// assert_eq!(gas.gas_mass(), galaxy.gas_disc().mass());
/// assert_eq!(gas.radial_scale(), galaxy.gas_disc().length());
/// // The warm ionised layer is drawn by its density at the Sun's radius, and the neutral disc
/// // takes what the warm layer and the molecular disc leave of the gas mass.
/// assert!((0.025..=0.035).contains(&gas.warm_density().value()));
/// let shares = gas.neutral_fraction() + gas.warm_fraction() + gas.molecular_disc().fraction();
/// assert!((shares - 1.0).abs() < 1e-15);
/// assert!(gas.neutral_fraction() > 0.5);
/// # Ok::<(), hyperion_sim::galaxy::gas::params::BuildGasParamsError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GasParams {
    gas_mass: SolarMasses,
    radial_scale: LightYears,
    hole_scale: LightYears,
    neutral_height: LightYears,
    warm_density: HydrogenPerCm3,
    warm_mass: SolarMasses,
    warm_height: LightYears,
    molecular: MolecularDisc,
    corona_density: HydrogenPerCm3,
    pressure_floor: KelvinPerCm3,
    sigma_ln: f64,
    lane: LaneParams,
}

impl GasParams {
    /// The Sun's galactocentric radius, 26,000 ly: where the brainstorm's figures for the solar
    /// neighbourhood are read, and so where the warm ionised layer's drawn density applies.
    ///
    /// It is the same for every galaxy, as the rest of the generator takes it (plan 02's and plan
    /// 03's checks at the Sun-like point, and this plan's brackets). Scaling it with a galaxy's gas
    /// disc was estimated and rejected: over the same 2,000 galaxies the least neutral share falls
    /// from about 0.5 to about 0.27, because a short disc then reads the solar density well inside
    /// 26,000 ly, where the layer is denser and its mass larger (plan 07, Risks).
    pub const REFERENCE_RADIUS: LightYears = LightYears::new(26_000.0);

    /// The scale height `h_P` of the pressure above the plane, a constant of the generator version.
    ///
    /// The thermal pressure falls more slowly with height than the neutral gas does, because the
    /// layers that hold it are thicker; 1,500 ly is between the neutral and the warm layer's
    /// heights (Design note 11).
    pub const PRESSURE_HEIGHT: LightYears = LightYears::new(1_500.0);

    /// The velocity dispersion `σ_P` of Design note 11's hydrostatic pressure, a constant of the
    /// generator version: 5.15 km/s.
    ///
    /// It is of the order of the neutral medium's turbulent and thermal speeds, as a hydrostatic
    /// layer requires, and it is calibrated at the Milky Way fixture against two measurements
    /// together (P07.T5). The mid-plane thermal pressure at 26,000 ly is 3,800 K cm⁻³ (Jenkins and
    /// Tripp 2011, ApJ 734, 65: log(P ÷ k) of 3.58 with a dispersion of at least 0.175 dex among
    /// the cold neutral medium's sight lines), held to 3,400–4,200; and the hot phase fills a fifth
    /// to two fifths of the plane's volume for `σ_ln` of 2–2.5 (brainstorm, "Between the stars"),
    /// held to 0.17–0.41, which a denser plane needs a higher pressure for. Design note 11's 5.5
    /// km/s was set when the fixture's disc gas in the plane was 0.70 cm⁻³; ruling 19 raised it to
    /// 0.83, which at 5.5 km/s gives 4,670 K cm⁻³, and 4.9 km/s, which gives 3,790, leaves the hot
    /// phase 16% of the plane at `σ_ln` 2.0. Both hold for 5.10–5.19 km/s; 5.15 gives 4,145 K cm⁻³, 0.04 dex
    /// above the measured mean, and a hot share of 0.17–0.38 (plan 07, Risks). P07.T12 tunes it
    /// again with the rest of the fixture.
    pub const PRESSURE_SPEED: KilometresPerSecond = KilometresPerSecond::new(5.15);

    /// The gas parameters of the galaxy `params`, with `seed` keying this plan's own draws.
    ///
    /// The draws are on [`tags::GAS_PARAMS`] with [`ObjectKey::galaxy`], one word per parameter at
    /// its fixed index, so nothing plan 02 drew moves and the order of the draws below is
    /// immaterial.
    ///
    /// # Errors
    ///
    /// [`BuildGasParamsError::NoNeutralGas`] if the molecular disc and the warm ionised layer drawn
    /// here outweigh plan 02's gas mass, so that nothing is left for the neutral disc (ruling 22 of
    /// 2026-09-22). The warm layer's clamp (ruling 91; [`warm_density`](Self::warm_density)) keeps
    /// half the gas neutral wherever the molecular disc weighs less than half of it, which holds at
    /// every corner of plan 02's ranges by a factor of some four hundred, so no galaxy the public
    /// API builds is refused.
    pub fn from_galaxy(seed: Seed, params: &GalaxyParams) -> Result<Self, BuildGasParamsError> {
        let mut stream = Stream::open(seed, tags::GAS_PARAMS, ObjectKey::galaxy());
        let drawn = core::array::from_fn(|i| Drawn::ALL[i].draw(&mut stream));
        Self::of_galaxy(params, drawn)
    }

    /// The Milky Way fixture, pairing with
    /// [`GalaxyParams::milky_way_like`](crate::galaxy::params::GalaxyParams::milky_way_like).
    ///
    /// Its values are Design note 3's table: the gas disc is plan 02's fixture (8.2 × 10⁹ M☉,
    /// 12,250 ly long, 700 ly tall), the hole scale is the bar's half-length of 16,000 ly, the
    /// molecular disc is the nuclear disc's 290 ly by 58 ly, and the rest are the measured values
    /// where the brainstorm states one and the middle of the range otherwise. P07.T12 tuned them
    /// against the brainstorm's targets (`tests/gas_statistics.rs`): only the corona moved, from
    /// 10⁻³ to 6 × 10⁻⁴ cm⁻³, when ruling 91 narrowed its range.
    ///
    /// # Panics
    ///
    /// Never: the fixture's neutral disc holds 85% of its gas.
    #[must_use]
    pub fn milky_way_like() -> Self {
        Self::of_galaxy(&GalaxyParams::milky_way_like(), MILKY_WAY_DRAWN)
            .expect("the fixture's neutral disc holds 85% of its gas")
    }

    /// The parameters of `params` with `drawn` in Design note 3's table order: what
    /// [`from_galaxy`](Self::from_galaxy) and [`milky_way_like`](Self::milky_way_like) share.
    fn of_galaxy(
        params: &GalaxyParams,
        drawn: [f64; DRAWN_COUNT],
    ) -> Result<Self, BuildGasParamsError> {
        let [
            hole_ratio,
            warm_density,
            warm_height,
            molecular_mass,
            molecular_height_ratio,
            corona_density,
            pressure_floor,
            sigma_ln,
            lane_offset,
            lane_width,
            lane_fraction,
        ] = drawn;
        let disc = params.gas_disc();
        let gas_mass = disc.mass();
        let hole_scale = params.bar().half_length() * hole_ratio;
        let drawn_warm_mass = smooth::warm_mass(
            warm_density,
            hole_scale.value(),
            disc.length().value(),
            warm_height,
        );
        // Ruling 91: the warm layer weighs at most half the gas less the molecular disc, so the
        // neutral layer keeps at least half. The mass is linear in the density, so the density is
        // scaled by the same factor; the comparison, not `f64::min`, keeps the drawn bits exactly
        // where the clamp does not bind, which is every one of 2,000 drawn galaxies.
        let warm_cap = 0.5 * gas_mass.value() - molecular_mass;
        let (warm_density, warm_mass) = if drawn_warm_mass > warm_cap && warm_cap > 0.0 {
            (warm_density * (warm_cap / drawn_warm_mass), warm_cap)
        } else {
            (warm_density, drawn_warm_mass)
        };
        let neutral_mass = gas_mass.value() - warm_mass - molecular_mass;
        if neutral_mass.is_nan() || neutral_mass <= 0.0 {
            return Err(BuildGasParamsError::NoNeutralGas {
                gas_mass,
                warm_mass: SolarMasses::new(warm_mass),
                molecular_mass: SolarMasses::new(molecular_mass),
            });
        }
        let molecular_length = params.nuclear_disc().length();
        Ok(Self {
            gas_mass,
            radial_scale: disc.length(),
            hole_scale,
            neutral_height: disc.height(),
            warm_density: HydrogenPerCm3::new(warm_density),
            warm_mass: SolarMasses::new(warm_mass),
            warm_height: LightYears::new(warm_height),
            molecular: MolecularDisc {
                fraction: molecular_mass / gas_mass.value(),
                mass: SolarMasses::new(molecular_mass),
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
        })
    }

    /// The gas disc's mass, plan 02's
    /// [`GasDiscParams::mass`](crate::galaxy::params::GasDiscParams::mass): 17.5–35% of the whole
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
    /// [`GasDiscParams::HEIGHT`](crate::galaxy::params::GasDiscParams::HEIGHT) of 700 ly: the
    /// brainstorm's "a thin neutral disc a few hundred light-years tall", made tall enough to carry
    /// the measured column without densifying the plane (plan 02, ruling 1 of 2026-09-22).
    #[must_use]
    pub fn neutral_height(&self) -> LightYears {
        self.neutral_height
    }

    /// The neutral layer's mass: what the warm ionised layer and the molecular disc leave of the gas
    /// mass (ruling 19).
    #[must_use]
    pub fn neutral_mass(&self) -> SolarMasses {
        self.gas_mass - self.warm_mass - self.molecular.mass
    }

    /// The neutral layer's share of the gas mass, `1 − f_w − f_c`.
    ///
    /// It is most of the gas: 0.85 for the Milky Way fixture, against the 0.87 of McKee, Parravano
    /// and Hollenbach's (2015) local column that is atomic or molecular, at least 0.53 over 2,000
    /// seeds (plan 07, Risks), and never below 0.5 for any galaxy the public API builds, by the
    /// warm layer's clamp (ruling 91).
    #[must_use]
    pub fn neutral_fraction(&self) -> f64 {
        1.0 - self.warm_fraction() - self.molecular.fraction
    }

    /// The warm ionised layer's mid-plane density at [`REFERENCE_RADIUS`](Self::REFERENCE_RADIUS),
    /// 0.025–0.035 cm⁻³: the brainstorm's "about 0.03 atoms per cubic centimetre", drawn.
    ///
    /// It is clamped where the layer would weigh more than half the gas less the molecular disc,
    /// `½ G − M_c`, so that the neutral share is never below 0.5 (ruling 91 of 2026-09-22). The
    /// layer's mass is linear in its density, so the clamp scales the density by `(½ G − M_c) ÷
    /// W`. It binds on none of 2,000 drawn galaxies, whose least neutral share is 0.529, and only
    /// on galaxies built by hand at the corner of plan 02's ranges.
    #[must_use]
    pub fn warm_density(&self) -> HydrogenPerCm3 {
        self.warm_density
    }

    /// The warm ionised layer's mass, derived from its density at the Sun's radius, its height and
    /// the radial form it shares with the neutral disc.
    #[must_use]
    pub fn warm_mass(&self) -> SolarMasses {
        self.warm_mass
    }

    /// The warm ionised layer's share `f_w` of the gas mass, derived: its mass over plan 02's.
    #[must_use]
    pub fn warm_fraction(&self) -> f64 {
        self.warm_mass / self.gas_mass
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

    /// The hot corona's density `n_cor`, 0.5–0.8 × 10⁻³ cm⁻³: the brainstorm's "a hot corona of
    /// about 10⁻³".
    ///
    /// It is an additive floor and carries no dust, because grains do not survive in it (Design
    /// note 10). The range stops at 0.8 × 10⁻³ so that the corona, with the warm layer's tail, is
    /// hot 20,000 ly above every radius from 8,000 to 40,000 ly even at the lowest pressure floor:
    /// the worst corner of the draws gives about 104,000 K (ruling 91 of 2026-09-22; Design note
    /// 12). Miller and Bregman's (2015, ApJ 800, 14) hot halo is 4–7 × 10⁻⁴ cm⁻³ at 7–10 kpc.
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
    // peak at 14,000 ly; the warm layer is the brainstorm's 0.03 cm⁻³ and 3,000 ly.
    1.0, 0.030, 3_000.0,
    // 2.5 × 10⁶ M☉ of diffuse central gas, the middle of Design note 5's 2–3 × 10⁶.
    2.5e6,
    // 58 ly against the nuclear disc's 290 ly, half of Sormani et al.'s (2022) 28 pc height.
    0.20,
    // The corona at the Sun's radius as Miller and Bregman's (2015) β-model gives it,
    // 1.35 × 10⁻² × 8.2^(−1.5) = 5.7 × 10⁻⁴ cm⁻³ at 8.2 kpc, to one figure (P07.T12, ruling 91).
    0.6e-3, 400.0, 2.3, 450.0, 200.0, 0.12,
];

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::super::IONISED_PARTICLES_PER_HYDROGEN;
    use super::*;
    use crate::galaxy::imf::MassFunctionKind;
    use crate::galaxy::params::{GalaxyParamsBuilder, GasDiscParams};
    use crate::units::{Dex, Years};

    /// Seeds of the sweeps: the same form plan 02's tests use, with this plan's own prefix.
    fn seeds(count: u64) -> impl Iterator<Item = Seed> {
        (0..count).map(|n| Seed::new(0x0700_5eed_0000_0000 | n))
    }

    fn galaxy(seed: Seed) -> GalaxyParams {
        GalaxyParams::from_seed(seed, MassFunctionKind::default())
    }

    /// The least share of the gas the neutral disc may hold: half. A galaxy whose neutral gas is not
    /// most of its gas is not a Milky Way-like spiral (ruling 19); the Milky Way's local column is
    /// 87% atomic or molecular (McKee, Parravano and Hollenbach 2015, Table 2: 11.9 of 13.7 M☉
    /// pc⁻²). A seed below it is a finding to report, not a share to clamp.
    const NEUTRAL_SHARE_FLOOR: f64 = 0.5;

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
        within("warm density", gas.warm_density().value(), 0.025, 0.035);
        within("warm height", gas.warm_height().value(), 2_500.0, 3_500.0);
        assert_same_bits(gas.warm_fraction(), gas.warm_mass() / gas.gas_mass());
        let molecular = gas.molecular_disc();
        within("molecular mass", molecular.mass().value(), 2e6, 3e6);
        assert_same_bits(molecular.fraction(), molecular.mass() / gas.gas_mass());
        assert_eq!(molecular.length(), galaxy.nuclear_disc().length());
        within(
            "molecular height ratio",
            molecular.height() / molecular.length(),
            0.15,
            0.25,
        );
        within(
            "corona density",
            gas.corona_density().value(),
            0.5e-3,
            0.8e-3,
        );
        within("pressure floor", gas.pressure_floor().value(), 300.0, 500.0);
        assert_eq!(gas.pressure_height(), GasParams::PRESSURE_HEIGHT);
        assert_eq!(gas.pressure_speed(), GasParams::PRESSURE_SPEED);
        within("sigma_ln", gas.sigma_ln(), 2.0, 2.5);
        let lane = gas.lane();
        within("lane offset", lane.offset().value(), 300.0, 600.0);
        within("lane width", lane.width().value(), 150.0, 300.0);
        within("lane fraction", lane.fraction(), 0.08, 0.20);
        // The three phases share the whole gas mass, and the neutral disc holds most of it.
        let shares = gas.neutral_fraction() + gas.warm_fraction() + molecular.fraction();
        assert!((shares - 1.0).abs() < 1e-15, "shares sum to {shares}");
        let masses = gas.neutral_mass() + gas.warm_mass() + molecular.mass();
        assert!(
            (masses / gas.gas_mass() - 1.0).abs() < 1e-15,
            "masses sum to {masses:?}"
        );
        within(
            "neutral fraction",
            gas.neutral_fraction(),
            NEUTRAL_SHARE_FLOOR,
            1.0,
        );
    }

    /// Design note 3's table over 2,000 seeds. The galaxy's own parameters enter as the three rows
    /// that are read, the two the draws scale (the bar's half-length and the nuclear disc's length)
    /// and, through the neutral share, the gas mass and scale length. A galaxy costs some 17 ms to
    /// build even optimised, so the sweep draws 2,000 seeds against one galaxy and 32 seeds against
    /// their own, which is what plan 02's own fast sweep costs; `tests/gas_statistics.rs` holds the
    /// neutral share over 2,000 seeds' own galaxies under `just test-slow`.
    #[test]
    fn every_parameter_lies_in_its_range_over_2000_seeds() {
        let fixture = GalaxyParams::milky_way_like();
        for seed in seeds(2_000) {
            assert_in_ranges(&fixture, &GasParams::from_galaxy(seed, &fixture).unwrap());
        }
        for seed in seeds(32) {
            let galaxy = galaxy(seed);
            assert_in_ranges(&galaxy, &GasParams::from_galaxy(seed, &galaxy).unwrap());
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
        assert_eq!(gas.neutral_height(), GasDiscParams::HEIGHT);
        assert_eq!(gas.warm_density(), HydrogenPerCm3::new(0.030));
        assert_eq!(gas.molecular_disc().mass(), SolarMasses::new(2.5e6));
        // Plan 02's fixture: 24% of a 3.43 × 10¹⁰ M☉ thin disc, 1.75 of its 7,000 ly.
        assert!((gas.radial_scale().value() - 12_250.0).abs() < 1e-9);
        assert!((gas.gas_mass().value() / 8.24e9 - 1.0).abs() < 0.01);
        // The warm layer at 0.03 cm⁻³ and 3,000 ly weighs 1.2 × 10⁹ M☉, a seventh of the gas.
        assert!((gas.warm_mass().value() / 1.2e9 - 1.0).abs() < 0.01);
        assert!((0.84..=0.86).contains(&gas.neutral_fraction()));
    }

    #[test]
    fn the_same_seed_gives_the_same_parameters_and_others_differ() {
        let fixture = GalaxyParams::milky_way_like();
        let seed = Seed::new(0x0700_5eed_0000_0007);
        let once = GasParams::from_galaxy(seed, &fixture).unwrap();
        assert_eq!(once, GasParams::from_galaxy(seed, &fixture).unwrap());
        let other = GasParams::from_galaxy(Seed::new(seed.get() + 1), &fixture).unwrap();
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
    /// lowest floor against the highest density, 163,000 K; the warm layer's tail brings it to
    /// about 104,000 K over the inner disc (`gas::phase`'s corner proof, ruling 91).
    #[test]
    fn the_corona_is_hotter_than_a_hundred_thousand_kelvin_for_every_seed() {
        let temperature = |gas: &GasParams| {
            gas.pressure_floor().value()
                / (IONISED_PARTICLES_PER_HYDROGEN * gas.corona_density().value())
        };
        let fixture = GalaxyParams::milky_way_like();
        for seed in seeds(2_000) {
            let gas = GasParams::from_galaxy(seed, &fixture).unwrap();
            let hot = temperature(&gas);
            assert!(hot > 1e5, "the corona is {hot} K for {seed:?}");
        }
        // The extremes of the two ranges, which no sweep of seeds is sure to reach.
        let worst = 300.0 / (IONISED_PARTICLES_PER_HYDROGEN * 0.8e-3);
        assert!(worst > 1.6e5, "the worst case is {worst} K");
        assert!(temperature(&GasParams::milky_way_like()) > 1.5e5);
    }

    /// The least neutral share of the gas the *drawn* warm layer would leave, over the corners of
    /// every draw that feeds it (plan 07, ruling 31 of 2026-09-22): below zero, which is why ruling
    /// 91 clamps the layer.
    ///
    /// The share is `1 − (W + M_c) ÷ G`, with `G` plan 02's gas mass, `M_c` the molecular disc's
    /// drawn mass and `W` the warm ionised layer's mass. `W` is linear in the warm layer's drawn
    /// density and height, and `ln W` is convex in the hole scale and in the inverse of the gas
    /// disc's scale length (the log of an integral of exponentials linear in both, plus a linear
    /// term), so over any box of those draws `W` is greatest at a corner. It is **not** monotone in
    /// either length: at a short disc a larger hole lightens the layer and at a long one it weighs
    /// it down, which is why both ends are evaluated rather than assumed. `G` is linear in the
    /// stellar mass and the gas fraction, and the thin disc's part of the stellar mass falls as any
    /// other population's share rises. The formation timescale, the bar's part of the bulge and the
    /// mass function move the mean masses, and are evaluated at their ends; the thin disc's and the
    /// bar's lengths are taken at their clamps, which their scatters reach at any mass.
    ///
    /// The least corner is Kroupa's mass function, 3 × 10¹⁰ M☉ of stars with every other
    /// population at its largest share, a 5 Gyr formation timescale, the bar 40% of the bulge, a
    /// gas fraction of 0.175, the thin disc at its 11,500 ly clamp with the gas disc twice as long,
    /// the bar at its 18,000 ly clamp with a hole 1.2 times it, and the warm layer at 0.035 cm⁻³
    /// and 3,500 ly with a 3 × 10⁶ M☉ molecular disc, which together weigh 1.059 times the gas. A
    /// drawn galaxy reaches it only with the thin disc's length scatter at least
    /// [`LEAST_FAILING_THIN_SCATTER`] above its mass's length, 4.7 times its 0.05 dex σ, while every
    /// uniform draw above sits at its end; with no scatter the same corner keeps 0.32 of its gas
    /// neutral, and over 2,000 drawn galaxies the least is 0.529 (`tests/gas_statistics.rs`).
    const LEAST_DRAWN_CORNER_SHARE: f64 = -0.0586;

    /// The least thin-disc length scatter, dex, at which a drawn galaxy at the corner of
    /// [`LEAST_DRAWN_CORNER_SHARE`] would leave its neutral layer nothing without the clamp.
    const LEAST_FAILING_THIN_SCATTER: f64 = 0.236;

    /// The least gas mass over the corners of plan 02's ranges, M☉, to two figures: some 420 times
    /// the 6 × 10⁶ M☉ below which the molecular disc's largest draw would weigh half of it, the one
    /// case the clamp cannot help.
    const LEAST_CORNER_GAS_MASS: f64 = 2.5e9;

    /// The neutral share `1 − (W + M_c) ÷ G` the drawn warm layer would leave in `galaxy`, with it
    /// at `warm` cm⁻³ and `height` ly, the hole at `hole` times the bar and `molecular` M☉ of
    /// molecular gas; and the share [`GasParams::of_galaxy`] builds with the clamp, which must be
    /// the drawn share where that is at least half and half otherwise (ruling 91).
    fn corner_share(
        galaxy: &GalaxyParams,
        [hole, warm, height, molecular]: [f64; 4],
    ) -> (f64, f64) {
        let warm_mass = smooth::warm_mass(
            warm,
            galaxy.bar().half_length().value() * hole,
            galaxy.gas_disc().length().value(),
            height,
        );
        let drawn = 1.0 - (warm_mass + molecular) / galaxy.gas_disc().mass().value();
        let mut values = MILKY_WAY_DRAWN;
        values[..4].copy_from_slice(&[hole, warm, height, molecular]);
        let gas = GasParams::of_galaxy(galaxy, values).expect("the clamp leaves half the gas");
        let built = gas.neutral_fraction();
        if drawn >= NEUTRAL_SHARE_FLOOR {
            assert!((built - drawn).abs() < 1e-12, "{built} built at {drawn}");
            assert_same_bits(gas.warm_density().value(), warm);
        } else {
            assert!(
                (built - NEUTRAL_SHARE_FLOOR).abs() < 1e-12,
                "{built} at {drawn}"
            );
            assert!(gas.warm_density().value() < warm);
            // The clamped density is what the smooth layer reads, so the mass and the density
            // still agree.
            let clamped = smooth::warm_mass(
                gas.warm_density().value(),
                gas.hole_scale().value(),
                gas.radial_scale().value(),
                height,
            );
            assert!((clamped / gas.warm_mass().value() - 1.0).abs() < 1e-12);
        }
        (drawn, built)
    }

    /// A galaxy with the least gas plan 02's draws allow: the least stellar mass and gas fraction,
    /// and every population other than the thin disc at the top of its share.
    fn least_gas(kind: MassFunctionKind, sfh: f64, bar_of_bulge: f64) -> GalaxyParamsBuilder {
        GalaxyParamsBuilder::new()
            .mass_function(kind)
            .stellar_mass(SolarMasses::new(3e10))
            .thick_share(0.14)
            .bulge_bar_share(0.35)
            .bar_share_of_bulge(bar_of_bulge)
            .nuclear_disc_share(0.025)
            .halo_share(0.014)
            .sfh_timescale(Years::new(sfh))
            .gas_mass_fraction(0.175)
    }

    /// Ruling 91's proof that [`Galaxy::new`](crate::galaxy::Galaxy::new)'s `expect` cannot fire,
    /// replacing ruling 31's corner search: the neutral share at every corner of the draws that
    /// feed it, which is where its minimum over the drawn ranges lies
    /// ([`LEAST_DRAWN_CORNER_SHARE`] gives the argument).
    ///
    /// With the clamp the share is `1 − (min(W, ½ G − M_c) + M_c) ÷ G ≥ ½` wherever `½ G > M_c`,
    /// whatever `W` is; so the corners need only show that the least gas mass is far above twice
    /// the largest molecular disc, and the sweep checks the algebra as built, to 10⁻¹². The drawn
    /// share the clamp replaces is still found at its documented corner, and a drawn galaxy's
    /// thin-disc scatter either side of the least that would fail builds at 0.5.
    #[test]
    fn the_neutral_share_is_at_least_half_at_every_corner_of_the_draws() {
        let ends = |lo: f64, hi: f64| [lo, hi];
        let mut gas_corners = Vec::new();
        for hole in ends(0.8, 1.2) {
            for warm in ends(0.025, 0.035) {
                for height in ends(2_500.0, 3_500.0) {
                    for molecular in ends(2e6, 3e6) {
                        gas_corners.push([hole, warm, height, molecular]);
                    }
                }
            }
        }
        let (mut least, mut least_at) = (f64::INFINITY, String::new());
        let (mut least_built, mut least_gas_mass) = (f64::INFINITY, f64::INFINITY);
        for kind in [MassFunctionKind::Kroupa, MassFunctionKind::Chabrier] {
            for sfh in ends(5e9, 9e9) {
                for bar_of_bulge in ends(0.30, 0.40) {
                    for thin in ends(7_000.0, 11_500.0) {
                        for ratio in ends(1.5, 2.0) {
                            for bar in ends(10_000.0, 18_000.0) {
                                let galaxy = least_gas(kind, sfh, bar_of_bulge)
                                    .thin_length(LightYears::new(thin))
                                    .gas_length_ratio(ratio)
                                    .bar_half_length(LightYears::new(bar))
                                    .build()
                                    .expect("every value is inside plan 02's drawn ranges");
                                least_gas_mass =
                                    least_gas_mass.min(galaxy.gas_disc().mass().value());
                                for corner in &gas_corners {
                                    let (drawn, built) = corner_share(&galaxy, *corner);
                                    least_built = least_built.min(built);
                                    if drawn < least {
                                        least = drawn;
                                        least_at = format!(
                                            "{kind:?}, {sfh:e} yr, {bar_of_bulge}, {thin} ly × \
                                             {ratio}, {bar} ly, {corner:?}"
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(
            (least - LEAST_DRAWN_CORNER_SHARE).abs() < 5e-4,
            "the least drawn corner share is {least:.4} ({least_at}), not the documented \
             {LEAST_DRAWN_CORNER_SHARE}"
        );
        assert!(
            (least_built - NEUTRAL_SHARE_FLOOR).abs() < 1e-12,
            "the least built share is {least_built}"
        );
        assert!(
            (least_gas_mass / LEAST_CORNER_GAS_MASS - 1.0).abs() < 0.05,
            "the least gas mass is {least_gas_mass:e} M☉"
        );
        assert!(0.5 * least_gas_mass > 400.0 * 3e6);
        // The least corner as a drawn galaxy reaches it: its lengths coupled to their masses, the
        // bar's scatter at 0.1 dex (which puts it at its clamp) and the thin disc's either side of
        // the least that fails.
        let worst = [1.2, 0.035, 3_500.0, 3e6];
        let drawn = |scatter: f64| {
            least_gas(MassFunctionKind::Kroupa, 5e9, 0.40)
                .gas_length_ratio(2.0)
                .thin_length_scatter(Dex::new(scatter))
                .bar_length_scatter(Dex::new(0.1))
                .build()
                .expect("every value is inside plan 02's drawn ranges")
        };
        let below = corner_share(&drawn(LEAST_FAILING_THIN_SCATTER - 0.001), worst);
        let above = corner_share(&drawn(LEAST_FAILING_THIN_SCATTER + 0.001), worst);
        assert!(below.0 > 0.0 && above.0 < 0.0, "{below:?} and {above:?}");
        assert!((below.1 - 0.5).abs() < 1e-12 && (above.1 - 0.5).abs() < 1e-12);
        assert!(corner_share(&drawn(0.0), worst).0 > 0.3);
        // Why the corners suffice for the two radial scales: the warm layer's mass is log-convex in
        // the hole scale and in the inverse radial scale, so on a line between two drawn ends it
        // never rises above the chord. Checked on a grid over the drawn ranges.
        let log_mass = |hole: f64, inverse: f64| {
            math::ln(smooth::warm_mass(0.035, hole, 1.0 / inverse, 3_500.0))
        };
        let (holes, inverses) = ((8_000.0, 21_600.0), (1.0 / 23_000.0, 1.0 / 10_500.0));
        let at = |(lo, hi): (f64, f64), t: f64| lo + t * (hi - lo);
        for i in 0..=16_u32 {
            for j in 0..=16_u32 {
                let (s, t) = (f64::from(i) / 16.0, f64::from(j) / 16.0);
                let (hole, inverse) = (at(holes, s), at(inverses, t));
                let here = log_mass(hole, inverse);
                let across =
                    (1.0 - s) * log_mass(holes.0, inverse) + s * log_mass(holes.1, inverse);
                let along = (1.0 - t) * log_mass(hole, inverses.0) + t * log_mass(hole, inverses.1);
                assert!(
                    here <= across + 1e-9 && here <= along + 1e-9,
                    "not convex at {s}, {t}"
                );
            }
        }
    }
}
