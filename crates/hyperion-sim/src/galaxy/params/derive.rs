//! Deriving everything the seed does not draw (plan 02, P02.T5.b).
//!
//! In order: the populations' shares of systems, their age distributions, the mean present-day
//! mass of a system in each (P02.T4), the system count and the populations' masses (Design note
//! 3), the sizes coupled to those masses (Design note 16), then the gas disc, the nuclear cluster,
//! the dark halo, the halo's components and the accretion history, and last the black hole's
//! mass from the bulge's dispersion in the mass model of everything else (P02.T6.e). Every sum
//! runs in population order, which is part of the generator version.

use super::accretion::{AccretionHistory, Progenitor, ProgenitorKind};
use super::draws;
use super::halo::{HaloBreak, HaloComponentKind, HaloComponentParams, HaloParams};
use super::inputs::{HaloComponentInput, Inputs, Size};
use super::validate::{SizeRange, validate};
use super::{
    ArmParams, BarParams, BlackHoleParams, BuildGalaxyParamsError, BulgeParams, DarkHaloParams,
    DiscParams, GalaxyParams, GasDiscParams, NuclearClusterParams, NuclearDiscParams,
};
use crate::galaxy::ages::{
    AgeDistribution, BULGE_AGES, FeatureShare, LONG_BAR_AGES, THICK_DISC_AGES, THIN_DISC_HISTORY,
    YOUNG_AGE_LIMIT, young_fraction,
};
use crate::galaxy::consts::{G, LIGHT_YEARS_PER_MEGAPARSEC};
use crate::galaxy::fates::{
    ProvisionalFates, mean_formed_mass, mean_present_mass, mean_present_mass_of_mixture,
};
use crate::galaxy::potential::sigma;
use crate::galaxy::{POPULATIONS, Population};
use crate::math;
use crate::units::{
    Degrees, Dex, DexPerKiloparsec, KilometresPerSecond, LightYears, Radians, SolarMasses, Years,
};

/// A size law of Design note 16: the Milky Way value at the Milky Way mass, and the clamp, all
/// from P02.T5.b.
struct SizeLaw {
    range: SizeRange,
    /// Ly.
    reference_size: f64,
    /// M☉.
    reference_mass: f64,
}

impl SizeLaw {
    /// The size for a component of `mass` M☉: fixed, or the reference size times
    /// (mass ÷ reference mass)^⅓ times `10^scatter`, clamped.
    fn size(&self, size: Size, mass: f64) -> f64 {
        match size {
            Size::Fixed(size) => size,
            Size::Coupled { scatter } => (self.reference_size
                * math::cbrt(mass / self.reference_mass)
                * math::exp10(scatter))
            .clamp(self.range.min, self.range.max),
        }
    }
}

/// The thin disc: 8,480 ly (2.6 kpc, Bland-Hawthorn and Gerhard 2016) at 3.4 × 10¹⁰ M☉, clamped
/// to 7,000–11,500 ly.
const THIN_LENGTH: SizeLaw = SizeLaw {
    range: SizeRange {
        parameter: "thin.length",
        scatter_parameter: "thin.length.scatter",
        scatter: draws::THIN_LENGTH_SCATTER,
        min: 7_000.0,
        max: 11_500.0,
    },
    reference_size: 8_480.0,
    reference_mass: 3.4e10,
};

/// The bulge's long axis: 2,300 ly at 1.3 × 10¹⁰ M☉, clamped to 1,700–3,000 ly.
const BULGE_LENGTH: SizeLaw = SizeLaw {
    range: SizeRange {
        parameter: "bulge.length",
        scatter_parameter: "bulge.length.scatter",
        scatter: draws::BULGE_LENGTH_SCATTER,
        min: 1_700.0,
        max: 3_000.0,
    },
    reference_size: 2_300.0,
    reference_mass: 1.3e10,
};

/// The long bar's half-length: 16,000 ly (Wegg, Gerhard and Portail 2015) at 5.6 × 10⁹ M☉,
/// clamped to 10,000–18,000 ly.
const BAR_LENGTH: SizeLaw = SizeLaw {
    range: SizeRange {
        parameter: "bar.length",
        scatter_parameter: "bar.length.scatter",
        scatter: draws::BAR_LENGTH_SCATTER,
        min: 10_000.0,
        max: 18_000.0,
    },
    reference_size: 16_000.0,
    reference_mass: 5.6e9,
};

/// The nuclear disc: 290 ly (Sormani et al. 2022) at 1.05 × 10⁹ M☉, clamped to 200–400 ly.
const NUCLEAR_LENGTH: SizeLaw = SizeLaw {
    range: SizeRange {
        parameter: "nuclear.length",
        scatter_parameter: "nuclear.length.scatter",
        scatter: draws::NUCLEAR_LENGTH_SCATTER,
        min: 200.0,
        max: 400.0,
    },
    reference_size: 290.0,
    reference_mass: 1.05e9,
};

/// The cosmic baryon fraction in `M₂₀₀ = M★ ÷ (0.157 f★)` (brainstorm, "Galaxy parameters"):
/// `Ω_b ÷ Ω_m` of the Planck cosmology.
const BARYON_FRACTION: f64 = 0.157;

/// The Hubble parameter h of Dutton and Macciò's (2014) Planck cosmology, H₀ = 100 h km/s/Mpc.
const HUBBLE_H: f64 = 0.671;

/// Dutton and Macciò (2014, MNRAS 441, 3359, eq. 8), NFW fits at z = 0 in the Planck cosmology:
/// log₁₀ c₂₀₀ = 0.905 − 0.101 log₁₀(M₂₀₀ ÷ 10¹² h⁻¹ M☉), with an intrinsic scatter of 0.11 dex.
/// Re-checked against the paper.
const CONCENTRATION_INTERCEPT: f64 = 0.905;

/// See [`CONCENTRATION_INTERCEPT`].
const CONCENTRATION_SLOPE: f64 = -0.101;

/// The nuclear cluster's mass over the nuclear disc's (plan 02, Design note 15).
const NUCLEAR_CLUSTER_MASS_RATIO: f64 = 0.024;

/// Dark halo mass per globular cluster, M☉ (brainstorm, "What is inside a cluster today").
///
/// Burkert and Forbes (2020, AJ 159, 56) fit `M_vir = 5 × 10⁹ M☉ × N_GC`; the brainstorm divides
/// M₂₀₀ by 6.5 × 10⁹ M☉, which is the value used here (the brainstorm is the specification). For
/// the Milky Way fixture it gives 184 against about 160 known.
const HALO_MASS_PER_GLOBULAR: f64 = 6.5e9;

/// The globular cluster count's clamp.
const GLOBULAR_COUNT: (f64, f64) = (80.0, 800.0);

/// Every lesser progenitor's core radius, ly. The brainstorm gives none and plan 02 draws none, so
/// it is fixed at a provisional value inside the dominant merger's range, 2,000–5,000 ly, and at
/// the lower end of the debris's (plan 02, Risks, R10).
const LESSER_CORE: f64 = 3_000.0;

/// The in-situ halo lies inside about 50,000 ly (brainstorm, "Streams and accreted structure").
const IN_SITU_CUT: f64 = 50_000.0;

/// The halo stops at 65,000 ly so that it fits inside the root cube (brainstorm, "Populations").
const HALO_CUT: f64 = 65_000.0;

/// The mean \[Fe/H\] of the fixed halo components (plan 02, P02.T7.e).
const IN_SITU_FEH: f64 = -0.6;
const DOMINANT_FEH: f64 = -1.2;
const DEBRIS_FEH: f64 = -1.5;

/// The shares of systems by population, in [`POPULATIONS`] order (plan 02, Design note 3).
fn population_shares(i: &Inputs) -> [f64; 7] {
    let thin = 1.0 - i.share_thick - i.share_bulge_bar - i.share_nuclear_disc - i.share_halo;
    let young = thin * young_fraction(Years::new(i.sfh_timescale));
    let bar = i.share_bulge_bar * i.share_bar_of_bulge;
    POPULATIONS.map(|p| match p {
        Population::YoungThinDisc => young,
        Population::OldThinDisc => thin - young,
        Population::ThickDisc => i.share_thick,
        Population::Bulge => i.share_bulge_bar - bar,
        Population::LongBar => bar,
        Population::NuclearDisc => i.share_nuclear_disc,
        Population::Halo => i.share_halo,
    })
}

/// A halo component's age range: the gigayear centred on `centre`.
fn halo_ages(centre: f64) -> AgeDistribution {
    let half = draws::HALO_AGE_HALF_WIDTH;
    AgeDistribution::uniform(Years::new(centre - half), Years::new(centre + half))
        .expect("the centre was checked to leave a range inside 10–13 Gyr")
}

/// The halo's smooth components in their fixed order, shares renormalised to sum to 1.
fn halo_components(i: &Inputs) -> Vec<HaloComponentParams> {
    let lesser_weight = i.halo_lesser.iter().fold(0.0, |sum, l| sum + l.weight);
    let total = i.halo_in_situ.share
        + i.halo_dominant.share
        + i.halo_lesser_share_total
        + i.halo_debris.share;
    let fixed =
        |kind, c: &HaloComponentInput, feh: f64, cut: f64, outer_break| HaloComponentParams {
            kind,
            share: c.share / total,
            slope: c.slope,
            core: LightYears::new(c.core),
            flattening: c.flattening,
            outer_break,
            cut_radius: LightYears::new(cut),
            ages: halo_ages(c.age_centre),
            feh_mean: Dex::new(feh),
        };
    let mut components = Vec::with_capacity(2 + i.halo_lesser.len() + 1);
    components.push(fixed(
        HaloComponentKind::InSitu,
        &i.halo_in_situ,
        IN_SITU_FEH,
        IN_SITU_CUT,
        None,
    ));
    components.push(fixed(
        HaloComponentKind::DominantMerger,
        &i.halo_dominant,
        DOMINANT_FEH,
        HALO_CUT,
        Some(HaloBreak {
            radius: LightYears::new(i.halo_dominant_break_radius),
            steepening: i.halo_dominant_break_steepening,
        }),
    ));
    for (n, lesser) in (1_u8..).zip(&i.halo_lesser) {
        components.push(HaloComponentParams {
            kind: HaloComponentKind::Lesser(n),
            share: i.halo_lesser_share_total * lesser.weight / lesser_weight / total,
            slope: lesser.slope,
            core: LightYears::new(LESSER_CORE),
            flattening: lesser.flattening,
            outer_break: None,
            cut_radius: LightYears::new(HALO_CUT),
            ages: halo_ages(lesser.age_centre),
            feh_mean: Dex::new(lesser.feh_mean),
        });
    }
    components.push(fixed(
        HaloComponentKind::GlobularDebris,
        &i.halo_debris,
        DEBRIS_FEH,
        HALO_CUT,
        None,
    ));
    components
}

/// The mean present-day mass of a system of each population, in [`POPULATIONS`] order.
fn mean_masses(i: &Inputs, halo: &[HaloComponentParams]) -> [SolarMasses; 7] {
    let f = i.mass_function.to_mass_function();
    let fates = ProvisionalFates;
    let tau = Years::new(i.sfh_timescale);
    let uniform = |[lo, hi]: [Years; 2]| {
        AgeDistribution::uniform(lo, hi).expect("the populations' age ranges are ordered")
    };
    POPULATIONS.map(|p| {
        let ages = match p {
            Population::YoungThinDisc => AgeDistribution::young_disc(tau, FeatureShare::None),
            Population::OldThinDisc => {
                AgeDistribution::exponential_history(tau, YOUNG_AGE_LIMIT, THIN_DISC_HISTORY)
            }
            Population::ThickDisc => Ok(uniform(THICK_DISC_AGES)),
            Population::Bulge => Ok(uniform(BULGE_AGES)),
            Population::LongBar => Ok(uniform(LONG_BAR_AGES)),
            Population::NuclearDisc => Ok(AgeDistribution::nuclear_disc()),
            Population::Halo => {
                let parts: Vec<(f64, &AgeDistribution)> =
                    halo.iter().map(|c| (c.share, &c.ages)).collect();
                return mean_present_mass_of_mixture(f.as_ref(), &fates, &parts);
            }
        }
        .expect("the timescale was checked to be positive");
        mean_present_mass(f.as_ref(), &fates, &ages)
    })
}

/// The NFW halo from the stellar mass, f★ and the concentration's scatter.
fn dark_halo(stellar_mass: f64, f_star: f64, scatter: f64) -> DarkHaloParams {
    let m200 = stellar_mass / (BARYON_FRACTION * f_star);
    let log_c = CONCENTRATION_INTERCEPT
        + CONCENTRATION_SLOPE * math::log10(m200 * HUBBLE_H / 1e12)
        + scatter;
    // ρ_crit = 3 H₀² ÷ 8πG in M☉ ly⁻³, with H₀ in km/s per light-year.
    let hubble = 100.0 * HUBBLE_H / LIGHT_YEARS_PER_MEGAPARSEC;
    let critical_density = 3.0 * hubble * hubble / (8.0 * core::f64::consts::PI * G);
    let r200 = math::cbrt(3.0 * m200 / (4.0 * core::f64::consts::PI * 200.0 * critical_density));
    DarkHaloParams {
        f_star,
        m200: SolarMasses::new(m200),
        concentration: math::exp10(log_c),
        r200: LightYears::new(r200),
    }
}

/// The globular cluster count: M₂₀₀ ÷ 6.5 × 10⁹ M☉ times `10^scatter`, rounded and clamped.
fn globular_count(m200: f64, scatter: f64) -> u32 {
    let count = (m200 / HALO_MASS_PER_GLOBULAR * math::exp10(scatter))
        .round()
        .clamp(GLOBULAR_COUNT.0, GLOBULAR_COUNT.1);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a whole number clamped to 80–800"
    )]
    let count = count as u32;
    count
}

/// The populations' shares, mean masses, masses and the system count.
struct Budget {
    shares: [f64; 7],
    mean_masses: [SolarMasses; 7],
    masses: [SolarMasses; 7],
    system_count: f64,
}

impl Budget {
    /// Shares from the inputs, mean masses by quadrature, then `N = M★ ÷ Σ share × mean` and
    /// each population's mass `N × share × mean` (plan 02, Design note 3).
    fn new(i: &Inputs, halo: &[HaloComponentParams]) -> Self {
        let shares = population_shares(i);
        let mean_masses = mean_masses(i, halo);
        let per_system = POPULATIONS.iter().fold(0.0, |sum, p| {
            sum + shares[p.index()] * mean_masses[p.index()].value()
        });
        let system_count = i.stellar_mass / per_system;
        let masses = POPULATIONS.map(|p| {
            SolarMasses::new(system_count * shares[p.index()] * mean_masses[p.index()].value())
        });
        Self {
            shares,
            mean_masses,
            masses,
            system_count,
        }
    }

    fn mass(&self, population: Population) -> f64 {
        self.masses[population.index()].value()
    }

    /// The thin disc's stellar mass, young and old together.
    fn thin_mass(&self) -> f64 {
        self.mass(Population::YoungThinDisc) + self.mass(Population::OldThinDisc)
    }
}

/// The accretion history: the dominant merger and the lesser progenitors with the masses of their
/// halo components, then the recent ones.
fn accretion(
    i: &Inputs,
    halo: &[HaloComponentParams],
    halo_mass: f64,
    m200: f64,
) -> AccretionHistory {
    let component_mass = |kind| {
        let share = halo
            .iter()
            .find(|c| c.kind == kind)
            .map_or(0.0, |c| c.share);
        SolarMasses::new(share * halo_mass)
    };
    let mut progenitors = Vec::with_capacity(1 + i.halo_lesser.len() + i.recent.len());
    progenitors.push(Progenitor {
        kind: ProgenitorKind::DominantMerger,
        mass: component_mass(HaloComponentKind::DominantMerger),
        accreted: Years::new(i.last_major_merger),
        orbit: i.dominant_orbit,
    });
    for (n, lesser) in (1_u8..).zip(&i.halo_lesser) {
        progenitors.push(Progenitor {
            kind: ProgenitorKind::Lesser(n),
            mass: component_mass(HaloComponentKind::Lesser(n)),
            accreted: Years::new(lesser.accreted),
            orbit: lesser.orbit,
        });
    }
    for (j, recent) in (0_u32..).zip(&i.recent) {
        progenitors.push(Progenitor {
            kind: ProgenitorKind::Recent(j),
            mass: SolarMasses::new(recent.mass),
            accreted: Years::new(recent.accreted),
            orbit: recent.orbit,
        });
    }
    AccretionHistory {
        last_major_merger: Years::new(i.last_major_merger),
        progenitors,
        globular_count: globular_count(m200, i.globular_count_scatter),
    }
}

/// Validates `i` and derives the rest.
pub(super) fn build(i: &Inputs) -> Result<GalaxyParams, BuildGalaxyParamsError> {
    validate(
        i,
        &[
            THIN_LENGTH.range,
            BULGE_LENGTH.range,
            BAR_LENGTH.range,
            NUCLEAR_LENGTH.range,
        ],
    )?;
    let halo_components = halo_components(i);
    let budget = Budget::new(i, &halo_components);
    let thin_mass = budget.thin_mass();
    let thin_length = THIN_LENGTH.size(i.thin_length, thin_mass);
    let bulge_a = BULGE_LENGTH.size(i.bulge_length, budget.mass(Population::Bulge));
    let bar_half = BAR_LENGTH.size(i.bar_length, budget.mass(Population::LongBar));
    let nuclear_mass = budget.mass(Population::NuclearDisc);
    let nuclear_length = NUCLEAR_LENGTH.size(i.nuclear_length, nuclear_mass);
    let dark_halo = dark_halo(i.stellar_mass, i.dark_f_star, i.dark_concentration_scatter);
    let accretion = accretion(
        i,
        &halo_components,
        budget.mass(Population::Halo),
        dark_halo.m200.value(),
    );
    let mass_function = i.mass_function.to_mass_function();
    let mut params = GalaxyParams {
        mass_function: i.mass_function,
        stellar_mass: SolarMasses::new(i.stellar_mass),
        sfh_timescale: Years::new(i.sfh_timescale),
        bar_of_bulge: i.share_bar_of_bulge,
        shares: budget.shares,
        mean_masses: budget.mean_masses,
        masses: budget.masses,
        system_count: budget.system_count,
        mean_formed_mass: mean_formed_mass(mass_function.as_ref(), &ProvisionalFates),
        thin_disc: DiscParams {
            length: LightYears::new(thin_length),
            height: LightYears::new(i.thin_mean_height),
        },
        young_disc: DiscParams {
            length: LightYears::new(thin_length),
            height: LightYears::new(i.young_height),
        },
        thick_disc: DiscParams {
            length: LightYears::new(i.thick_length_ratio * thin_length),
            height: LightYears::new(i.thick_height_ratio * i.thin_mean_height),
        },
        bulge: BulgeParams {
            scale_x: LightYears::new(bulge_a),
            scale_y: LightYears::new(i.bulge_b_over_a * bulge_a),
            scale_z: LightYears::new(i.bulge_c_over_a * bulge_a),
            boxiness: i.bulge_boxiness,
        },
        bar: BarParams {
            half_length: LightYears::new(bar_half),
            width: LightYears::new(i.bar_width_ratio * bar_half),
            height: LightYears::new(i.bar_height),
            corotation_ratio: i.bar_corotation_ratio,
        },
        nuclear_disc: NuclearDiscParams {
            length: LightYears::new(nuclear_length),
            height: LightYears::new(i.nuclear_height_ratio * nuclear_length),
        },
        nuclear_cluster: NuclearClusterParams {
            mass: SolarMasses::new(
                NUCLEAR_CLUSTER_MASS_RATIO
                    * nuclear_mass
                    * math::exp10(i.nuclear_cluster_mass_scatter),
            ),
        },
        arms: ArmParams {
            count: i.arm_count,
            pitch: Radians::from(Degrees::new(i.arm_pitch)),
            young_width: LightYears::new(i.arm_young_width),
            young_fraction: i.arm_young_fraction,
            old_amplitude: i.arm_old_amplitude,
        },
        gas_disc: GasDiscParams {
            mass: SolarMasses::new(i.gas_mass_fraction * thin_mass),
            length: LightYears::new(i.gas_length_ratio * thin_length),
        },
        dark_halo,
        // Filled in by the second phase below.
        black_hole: BlackHoleParams {
            scatter: Dex::new(i.bh_scatter),
            bulge_dispersion: KilometresPerSecond::ZERO,
            mass: SolarMasses::ZERO,
        },
        metallicity_gradient: DexPerKiloparsec::new(i.metallicity_gradient),
        halo: HaloParams {
            components: halo_components,
            discrete_share: i.halo_discrete_share,
        },
        accretion,
    };
    params.black_hole = black_hole(&params);
    Ok(params)
}

/// The second phase of the build (plan 02, P02.T6.e): the bulge's dispersion in the mass model of
/// `params` without the black hole and the nuclear cluster, then the black hole's mass from it
/// with the drawn scatter.
fn black_hole(params: &GalaxyParams) -> BlackHoleParams {
    let scatter = params.black_hole.scatter;
    let dispersion = sigma::bulge_dispersion(params);
    BlackHoleParams {
        scatter,
        bulge_dispersion: dispersion,
        mass: sigma::black_hole_mass(dispersion, scatter),
    }
}
