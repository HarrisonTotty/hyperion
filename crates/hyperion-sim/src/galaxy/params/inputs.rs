//! The primary parameters: everything the seed draws, before anything is derived.
//!
//! [`Inputs`] is what [`draws`](super::draws) fills from a seed, what the Milky Way fixture fills
//! by hand, and what [`GalaxyParamsBuilder`](super::GalaxyParamsBuilder) edits. Values are held
//! in the working units of [`galaxy`](crate::galaxy): M☉, light-years, years, degrees for the
//! pitch, dex for scatters. [`derive`](super::derive) validates them and derives the rest.

use super::accretion::Orbit;
use crate::galaxy::imf::MassFunctionKind;
use crate::units::{Dex, LightYears, SolarMasses, Years};

/// A size coupled to the mass it holds, or fixed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Size {
    /// Its Milky Way value times (mass ÷ Milky Way mass)^⅓ times `10^scatter`, clamped
    /// (plan 02, Design note 16). The scatter is in dex.
    Coupled { scatter: f64 },
    /// This many light-years, as a fixture sets it.
    Fixed(f64),
}

/// The number of spiral arms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ArmCount {
    /// Two arms, each leaving the x axis at an end of the bar.
    Two,
    /// Four arms.
    Four,
}

impl ArmCount {
    /// The number, 2 or 4.
    #[must_use]
    pub const fn get(self) -> u32 {
        match self {
            Self::Two => 2,
            Self::Four => 4,
        }
    }
}

/// The primary values of one smooth halo component other than the lesser progenitors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct HaloComponentInput {
    /// Share of the halo before renormalising.
    pub share: f64,
    /// Vertical axis ratio.
    pub flattening: f64,
    /// Core radius, ly.
    pub core: f64,
    /// Power-law slope.
    pub slope: f64,
    /// Centre of the one-gigayear age range, years.
    pub age_centre: f64,
}

/// A lesser old progenitor, as the builder takes it: its halo component and its accretion.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LesserProgenitorInput {
    pub(super) weight: f64,
    pub(super) flattening: f64,
    pub(super) slope: f64,
    pub(super) age_centre: f64,
    pub(super) feh_mean: f64,
    pub(super) accreted: f64,
    pub(super) orbit: Orbit,
}

impl LesserProgenitorInput {
    /// A lesser progenitor whose halo component takes `weight` of the lesser progenitors'
    /// combined share (the weights are normalised over all of them), with axis ratio
    /// `flattening`, slope `slope`, ages uniform over the gigayear centred on `age_centre` and
    /// mean \[Fe/H\] `feh_mean`, accreted `accreted` before the epoch on `orbit`. The builder
    /// checks every range, and that the accretion is 6–12 Gyr ago and no earlier than the
    /// component's youngest stars, half a gigayear younger than `age_centre`.
    #[must_use]
    pub const fn new(
        weight: f64,
        flattening: f64,
        slope: f64,
        age_centre: Years,
        feh_mean: Dex,
        accreted: Years,
        orbit: Orbit,
    ) -> Self {
        Self {
            weight,
            flattening,
            slope,
            age_centre: age_centre.value(),
            feh_mean: feh_mean.value(),
            accreted: accreted.value(),
            orbit,
        }
    }
}

/// A recent progenitor, as the builder takes it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RecentProgenitorInput {
    pub(super) mass: f64,
    pub(super) accreted: f64,
    pub(super) orbit: Orbit,
}

impl RecentProgenitorInput {
    /// A recent progenitor of stellar mass `mass`, accreted `accreted` before the epoch on
    /// `orbit`. The builder checks every range.
    #[must_use]
    pub const fn new(mass: SolarMasses, accreted: Years, orbit: Orbit) -> Self {
        Self {
            mass: mass.value(),
            accreted: accreted.value(),
            orbit,
        }
    }
}

/// Every primary parameter of a galaxy.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Inputs {
    pub mass_function: MassFunctionKind,
    /// M☉.
    pub stellar_mass: f64,
    pub share_thick: f64,
    pub share_bulge_bar: f64,
    pub share_bar_of_bulge: f64,
    pub share_nuclear_disc: f64,
    pub share_halo: f64,
    /// Years.
    pub sfh_timescale: f64,
    pub thin_length: Size,
    /// Ly.
    pub thin_mean_height: f64,
    /// Ly.
    pub young_height: f64,
    pub thick_length_ratio: f64,
    pub thick_height_ratio: f64,
    pub bulge_length: Size,
    pub bulge_b_over_a: f64,
    pub bulge_c_over_a: f64,
    pub bulge_boxiness: f64,
    pub bar_length: Size,
    pub bar_width_ratio: f64,
    /// Ly.
    pub bar_height: f64,
    pub bar_corotation_ratio: f64,
    pub nuclear_length: Size,
    pub nuclear_height_ratio: f64,
    /// Dex.
    pub nuclear_cluster_mass_scatter: f64,
    pub arm_count: ArmCount,
    /// Degrees.
    pub arm_pitch: f64,
    /// Ly.
    pub arm_young_width: f64,
    pub arm_young_fraction: f64,
    pub arm_old_amplitude: f64,
    pub gas_mass_fraction: f64,
    pub gas_length_ratio: f64,
    pub dark_f_star: f64,
    /// Dex.
    pub dark_concentration_scatter: f64,
    /// Dex.
    pub bh_scatter: f64,
    /// Dex per kpc.
    pub metallicity_gradient: f64,
    pub halo_in_situ: HaloComponentInput,
    pub halo_dominant: HaloComponentInput,
    /// Ly.
    pub halo_dominant_break_radius: f64,
    pub halo_dominant_break_steepening: f64,
    pub halo_lesser_share_total: f64,
    pub halo_lesser: Vec<LesserProgenitorInput>,
    pub halo_debris: HaloComponentInput,
    pub halo_discrete_share: f64,
    /// Years before the epoch.
    pub last_major_merger: f64,
    pub dominant_orbit: Orbit,
    pub recent: Vec<RecentProgenitorInput>,
    /// Dex.
    pub globular_count_scatter: f64,
}

impl Inputs {
    /// The coupled sizes' scatters, for a builder that sets `Size::Coupled` from a value in dex.
    pub(super) fn coupled(scatter: Dex) -> Size {
        Size::Coupled {
            scatter: scatter.value(),
        }
    }

    /// A fixed size.
    pub(super) fn fixed(size: LightYears) -> Size {
        Size::Fixed(size.value())
    }
}
