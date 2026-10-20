//! Aerosol extinction at 550 nm: a cloud's or a haze's mass extinction and single-scattering albedo
//! from its material's optical constants and its size distribution (plan 14, P14.T49.e).
//!
//! # What it gives
//!
//! [`aerosol_optics`] takes a registry row that is an aerosol material
//! ([`ClientOptics::aerosol_material`](super::ClientOptics::aerosol_material)), the phase and the
//! variant of one of rendering plan R08's committed index files
//! (`apps/hyperion/src/renderer/src/view/atmosphere/materials/`), a [`SizeDistribution`] and a
//! temperature, and returns the mode's [`AerosolOptics`] at 550 nm: its mass extinction `κ_ext` in
//! m² kg⁻¹, its single-scattering albedo ω, whether its index is a stand-in, and what its value
//! rests on ([`ExtinctionBasis`]). A mode's τ(550) is `κ_ext` times its column mass (P14.T24.c,
//! P14.T52.c).
//!
//! # The model
//!
//! - **The particles are the client's** (science-r08-nonspherical.md §6.4: "`κ_ext` for
//!   `nonSphericalMineral` and `crystal` modes is read from R08.T5.c's tables' `C_ext`, not from
//!   sphere Mie, so that the sim's mass extinction and the client's optics agree"):
//!   - a `sphere` file's are Mie spheres at its index ([`ExtinctionBasis::Sphere`]);
//!   - a `nonSphericalMineral` or `crystal` file with a phase table of its own (R08.T5.c's and
//!     T5.d's, 17 of the 21 on file) takes that table's `C_ext` and `C_sca`
//!     ([`ExtinctionBasis::PhaseTable`]):
//!     `TAMUdust2020`'s irregular hexahedra at sphericity 0.71 (M. Saito, P. Yang, J. Ding and X.
//!     Liu, J. Atmos. Sci. 78, 2089–2111, 2021) for the minerals, and Yang et al. 2013's roughened
//!     aggregate of eight columns for water ice (J. Atmos. Sci. 70, 330–347), read as the client's
//!     `tableModeOptics` reads them, the floor's absorption scaled by k ÷ `k_floor`. Outside a
//!     table's sizes spheres serve, as the client's do below them, and above them spheres of the
//!     particle's volume-to-area radius (T. C. Grenfell and S. G. Warren, J. Geophys. Res. 104,
//!     31697–31709, 1999), where the client has none ([`ExtinctionBasis::SpheresOutsidePhaseTable`]
//!     labels a mode whose `r_eff` lies there);
//!   - a `nonSphericalMineral` or `crystal` file with none (every ice but water's, whose extinction
//!     the client also takes from Mie), or whose index at 550 nm lies outside the hexahedra's
//!     kernels (aluminium oxide, sulphur, titanium dioxide and zinc sulphide, transparent above n
//!     1.70, whose tables are the longwave kernel's floor's and leave a large lossless grain's
//!     `Q̄_ext` near 1.2, and whose larger modes the client takes from Mie too), is the sphere of
//!     equal volume to projected area, r = 3V ÷ 4A, a labelled fallback
//!     ([`ExtinctionBasis::SphereFallback`]).
//!
//!   Every `r_eff` is of r = 3V ÷ 4A (P14.T24.c), A the orientation-averaged projected area, and
//!   `Q̄_ext` = ⟨`C_ext`⟩ ÷ ⟨A⟩. Equal-V/A spheres are not the hexahedra: for `mars_dust` at 550 nm
//!   the spheres' mass extinction is 0.38–1.49 times the hexahedra's over the grid (0.41–1.29 for
//!   gamma distributions of `v_eff` 0.22–0.37), and 832 against 704 m² kg⁻¹ at R08's Mars mode
//!   (gamma, `r_eff` 0.8974 µm, `v_eff` 0.3; science-r08-mmf.md §3); per particle the hexahedra's C ÷ V exceeds the sphere's for 0.2 ≲ x
//!   ≲ 1.3, up to Ψ⁻³ = 2.8 times in the Rayleigh limit, the equal-V/A sphere holding Ψ³ = 0.36 of
//!   the grain's volume (plan 14's record of P14.T49.e).
//! - **The size distribution** is Hansen and Travis 1974's standard gamma distribution or the
//!   log-normal, each by its effective radius `r_eff` = ⟨r³⟩ ÷ ⟨r²⟩ and effective variance `v_eff` =
//!   ⟨(r − `r_eff`)² r²⟩ ÷ (`r_eff²` ⟨r²⟩) (J. E. Hansen and L. D. Travis, "Light scattering in
//!   planetary atmospheres", Space Sci. Rev. 16, 527–610, 1974, (2.53)–(2.61)).
//! - **The table is density-free.** `hyperion-fit`'s `condensate_extinction` holds, for each index
//!   file at 550 nm (found by wavelength on the file's own grid; a phase table's two wavelength
//!   nodes about it blended linearly in wavelength, as the client blends them), the area-weighted
//!   extinction efficiency `Q̄_ext` = ⟨`C_ext`⟩ ÷ ⟨A⟩ and the albedo's logarithm ln ω =
//!   ln(⟨`C_sca`⟩ ÷ ⟨`C_ext`⟩), which keeps both ω and 1 − ω to its five digits, on an
//!   (`r_eff`, `v_eff`) grid for each distribution (`tables::condensate_extinction` and its six other
//!   parts). This module applies the row's grain density ρ at the mode's temperature
//!   ([`Substance::grain_density`](super::Substance::grain_density)): `κ_ext` = 3 `Q̄_ext` ÷ (4 ρ
//!   `r_eff`), since ⟨V⟩ ÷ ⟨A⟩ = 4 `r_eff` ÷ 3 for every distribution, the relation P14.T24.c's τ(550)
//!   = (3 ÷ 4) `Q̄_ext` M ÷ (ρ `r_eff`) writes. A compact particle's density is its row's grain density
//!   (science-r09-drainage.md §3), so a row whose density depends on temperature, or is not read
//!   at all, is served the same way: a phase whose density was not read at that temperature (solid
//!   `Ar`, solid `H2S`, β-`CO`, β-`O2`, liquid `S8`) is refused, labelled
//!   ([`ResolveAerosolError::NoDensity`]).
//! - **A variant's density.** The row's grain density is its reference composition's. A file of
//!   another variant (the 84.5 wt% acid, seawater at S = 35) has a table entry but no density yet;
//!   the 75 wt% acid's `75wt%` is `H2SO4`'s reference ([`density_variant`]).
//! - **Interpolation** is Lagrange's cubic on the four nearest nodes in ln `r_eff` and in ln(`v_eff` +
//!   0.05), of ln `Q̄_ext` and of ln ω ([`cubic_weights`]). The fit holds the table to its
//!   half-step check: halving either step moves no value of `Q̄_ext` or ω by more than 10⁻³ of
//!   itself, for every file (the fit refuses a table that breaks it).
//!
//! # What is not modelled
//!
//! - **Aggregates** (soot's and tholin's fractal clusters, P14.T24.c's fourth shape class): no
//!   index file is an aggregate's, so none has an entry, and no [`SizeDistribution`] describes one
//!   (an aggregate's sizes are its monomers' radius, their count and its fractal dimension). A
//!   closed set, for the composition audit.
//! - **Other wavelengths.** The table is 550 nm's alone, the wavelength τ(550) is quoted at.
//! - **Outside the grid** (`r_eff` 1 nm to about 101 µm for the gamma distribution and 33 µm for the
//!   log-normal; `v_eff` 0.04 to 0.45 and to 0.65), a mode is refused
//!   ([`ResolveAerosolError::OffGrid`]): below, the Rayleigh limit; above, the geometric limit,
//!   which `Q̄_ext` approaches as 2 + 2 x^(−2/3).
//!
//! Generator version 22 publishes none of it: nothing on the record reads it until P14.T54.b
//! (decision-bump-plan-after-22 §1.3).

use std::error::Error;
use std::fmt;

use super::{CondensedPhase, SubstanceId, SubstanceKey};
use crate::math;
use crate::tables::{
    condensate_extinction, condensate_extinction_2, condensate_extinction_3,
    condensate_extinction_4, condensate_extinction_5, condensate_extinction_6,
    condensate_extinction_7,
};
use crate::units::{Kelvin, Metres};

/// The wavelength of the table, nm: the visible wavelength τ(550) is quoted at.
pub const EXTINCTION_WAVELENGTH_NM: f64 = 550.0;

/// The number of the gamma distribution's effective-variance nodes in the table, which
/// `hyperion-fit`'s `condensate_extinction` fits on: 0.04 to 0.45, even in ln(`v_eff` + 0.05).
pub const GAMMA_VARIANCE_NODES: usize = 12;

/// The number of the log-normal's effective-variance nodes: 0.04 to 0.65, even in
/// ln(`v_eff` + 0.05).
pub const LOG_NORMAL_VARIANCE_NODES: usize = 14;

/// The rows whose grain density is that of one of their files' variants, by key, with the variant:
/// `H2SO4`'s is the 75.0 wt% acid's at 300 K, its reference composition (`rows/condensed.rs`'s
/// `SULPHURIC_ACID_75_DENSITY`). Every other row's density is the substance's own (a file with no
/// variant).
const DENSITY_VARIANTS: [(&str, &str); 1] = [("H2SO4", "75wt%")];

/// Which of a size distribution's two families it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SizeDistributionKind {
    /// Hansen and Travis 1974's standard gamma distribution (their (2.56)), n(r) ∝ r^((1 − 3b) ÷ b)
    /// e^(−r ÷ ab) with a = `r_eff` and b = `v_eff`, which can be normalised for 0 < `v_eff` < 0.5.
    Gamma,
    /// The log-normal (their (2.60)), n(r) ∝ r⁻¹ exp(−(ln r − ln `r_g`)² ÷ 2σ²), with σ² = ln(1 +
    /// `v_eff`) and `r_g` = `r_eff` (1 + `v_eff`)^(−5/2) (their Fig. 14's footnote).
    LogNormal,
}

impl SizeDistributionKind {
    /// The family's name, as messages write it: `gamma` or `log-normal`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Gamma => "gamma",
            Self::LogNormal => "log-normal",
        }
    }
}

/// A size distribution of particles by its effective radius and effective variance (Hansen and
/// Travis 1974, (2.53) and (2.54)), each of the volume-to-area radius r = 3V ÷ 4A (P14.T24.c).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct SizeDistribution {
    kind: SizeDistributionKind,
    effective_radius: Metres,
    effective_variance: f64,
}

impl SizeDistribution {
    /// Hansen and Travis's gamma distribution of effective radius `effective_radius` and effective
    /// variance `effective_variance`.
    ///
    /// # Errors
    ///
    /// [`BuildSizeDistributionError::Radius`] unless the radius is finite and positive, and
    /// [`BuildSizeDistributionError::Variance`] unless 0 < `v_eff` < 0.5, the range in which the
    /// distribution can be normalised.
    pub fn gamma(
        effective_radius: Metres,
        effective_variance: f64,
    ) -> Result<Self, BuildSizeDistributionError> {
        Self::new(
            SizeDistributionKind::Gamma,
            effective_radius,
            effective_variance,
        )
    }

    /// The log-normal of effective radius `effective_radius` and effective variance
    /// `effective_variance`.
    ///
    /// # Errors
    ///
    /// [`BuildSizeDistributionError::Radius`] unless the radius is finite and positive, and
    /// [`BuildSizeDistributionError::Variance`] unless `v_eff` is finite and positive.
    pub fn log_normal(
        effective_radius: Metres,
        effective_variance: f64,
    ) -> Result<Self, BuildSizeDistributionError> {
        Self::new(
            SizeDistributionKind::LogNormal,
            effective_radius,
            effective_variance,
        )
    }

    fn new(
        kind: SizeDistributionKind,
        effective_radius: Metres,
        effective_variance: f64,
    ) -> Result<Self, BuildSizeDistributionError> {
        let r = effective_radius.value();
        if !(r.is_finite() && r > 0.0) {
            return Err(BuildSizeDistributionError::Radius { radius_m: r });
        }
        let top = match kind {
            SizeDistributionKind::Gamma => 0.5,
            SizeDistributionKind::LogNormal => f64::INFINITY,
        };
        if !(effective_variance > 0.0 && effective_variance < top) {
            return Err(BuildSizeDistributionError::Variance {
                kind,
                variance: effective_variance,
            });
        }
        Ok(Self {
            kind,
            effective_radius,
            effective_variance,
        })
    }

    /// Its family.
    #[must_use]
    pub const fn kind(&self) -> SizeDistributionKind {
        self.kind
    }

    /// Its effective radius `r_eff` = ⟨r³⟩ ÷ ⟨r²⟩: finite and positive.
    #[must_use]
    pub const fn effective_radius(&self) -> Metres {
        self.effective_radius
    }

    /// Its effective variance `v_eff`, dimensionless: in (0, 0.5) for the gamma distribution and
    /// positive for the log-normal.
    #[must_use]
    pub const fn effective_variance(&self) -> f64 {
        self.effective_variance
    }
}

/// A size distribution could not be built.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildSizeDistributionError {
    /// The effective radius is not finite and positive.
    Radius {
        /// The radius asked for, m.
        radius_m: f64,
    },
    /// The effective variance is outside its family's range.
    Variance {
        /// The family.
        kind: SizeDistributionKind,
        /// The variance asked for.
        variance: f64,
    },
}

impl fmt::Display for BuildSizeDistributionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Radius { radius_m } => {
                write!(
                    f,
                    "effective radius {radius_m} m is not finite and positive"
                )
            }
            Self::Variance { kind, variance } => write!(
                f,
                "effective variance {variance} is outside the {} distribution's range",
                kind.name()
            ),
        }
    }
}

impl Error for BuildSizeDistributionError {}

/// What an index file's extinction, or a mode's, rests on: the particles `hyperion-fit`'s
/// `condensate_extinction` drew (the [module's](self) model).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ExtinctionBasis {
    /// Mie spheres at the file's index: a `sphere` file, its own shape class.
    Sphere,
    /// The file's own phase table, rendering plan R08.T5.c's or T5.d's: its particles' `C_ext` and
    /// `C_sca` as the client reads them (science-r08-nonspherical.md §6.4), for a mode whose
    /// `r_eff` lies within the table's sizes.
    PhaseTable,
    /// A phase-table file's mode whose `r_eff` lies outside its table's sizes: there the particles
    /// are spheres, below the table as the client draws them and above it of the particle's
    /// volume-to-area radius (Grenfell and Warren 1999), so the value rests mostly on them.
    SpheresOutsidePhaseTable,
    /// A `nonSphericalMineral` or `crystal` file with no phase table of its own, or whose index at
    /// 550 nm lies past the hexahedra's kernels: the spheres of its volume-to-area radius, a
    /// labelled fallback.
    SphereFallback,
}

impl ExtinctionBasis {
    /// The basis's name, as messages and the golden write it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Sphere => "sphere",
            Self::PhaseTable => "phase table",
            Self::SpheresOutsidePhaseTable => "spheres outside the phase table",
            Self::SphereFallback => "sphere fallback",
        }
    }
}

/// The particles `hyperion-fit`'s `condensate_extinction` drew for an index file's grids, a phase
/// table with its sizes ([`ExtinctionEntry::basis`]). A mode's [`ExtinctionBasis`] is its file's,
/// a phase table's split by where the mode's `r_eff` lies against the table's sizes.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum EntryBasis {
    /// Mie spheres at the file's index: a `sphere` file, its own shape class.
    Sphere,
    /// The file's own phase table, rendering plan R08.T5.c's or T5.d's: its particles' `C_ext` and
    /// `C_sca` as the client reads them (science-r08-nonspherical.md §6.4), and spheres outside its
    /// sizes.
    PhaseTable {
        /// The table's sizes, the volume-to-area radii r = 3V ÷ 4A from its smallest to its largest
        /// particle at 550 nm, m, where the two wavelength nodes drawn both hold particles.
        radii_m: (f64, f64),
    },
    /// A `nonSphericalMineral` or `crystal` file with no phase table of its own, or whose index at
    /// 550 nm lies past the hexahedra's kernels: the spheres of its volume-to-area radius, a
    /// labelled fallback.
    SphereFallback,
}

/// A mode's optics at 550 nm ([`aerosol_optics`]).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct AerosolOptics {
    mass_extinction_m2_per_kg: f64,
    extinction_efficiency: f64,
    single_scattering_albedo: f64,
    stand_in: bool,
    extinction_basis: ExtinctionBasis,
}

impl AerosolOptics {
    /// The mass extinction `κ_ext` = 3 `Q̄_ext` ÷ (4 ρ `r_eff`), m² kg⁻¹: the extinction cross-section
    /// of the mode's particles per unit of their mass, so that its τ(550) is `κ_ext` times its column
    /// mass in kg m⁻².
    #[must_use]
    pub const fn mass_extinction_m2_per_kg(&self) -> f64 {
        self.mass_extinction_m2_per_kg
    }

    /// The area-weighted extinction efficiency `Q̄_ext` = ⟨`C_ext`⟩ ÷ ⟨A⟩, A the projected area
    /// (πr² for a sphere), dimensionless: about 2 for large particles, falling as `r_eff`
    /// (absorbing) or `r_eff⁴` (scattering) for small ones.
    #[must_use]
    pub const fn extinction_efficiency(&self) -> f64 {
        self.extinction_efficiency
    }

    /// The single-scattering albedo ω = ⟨`C_sca`⟩ ÷ ⟨`C_ext`⟩, in (0, 1]: 1 for a file whose k is 0
    /// at 550 nm.
    #[must_use]
    pub const fn single_scattering_albedo(&self) -> f64 {
        self.single_scattering_albedo
    }

    /// Whether the index file is a stated stand-in (its `provenance` is `standIn`), not a
    /// measurement or a derivation from one.
    #[must_use]
    pub const fn is_stand_in(&self) -> bool {
        self.stand_in
    }

    /// What the value rests on: the file's spheres, its phase table, the spheres outside the
    /// table's sizes where the mode's `r_eff` lies, or the sphere fallback.
    #[must_use]
    pub const fn extinction_basis(&self) -> ExtinctionBasis {
        self.extinction_basis
    }
}

/// A mode's optics could not be resolved.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResolveAerosolError {
    /// The row is not an aerosol material: no index file or stand-in serves it as a cloud's or a
    /// haze's particles.
    NotModelled {
        /// The row.
        key: SubstanceKey,
    },
    /// No index file is the row's in that phase and variant.
    NoFile {
        /// The row.
        key: SubstanceKey,
        /// The phase asked for.
        phase: CondensedPhase,
    },
    /// The row has no grain density in that phase and variant at that temperature.
    NoDensity {
        /// The row.
        key: SubstanceKey,
        /// The phase asked for.
        phase: CondensedPhase,
        /// The temperature asked for.
        temperature: Kelvin,
    },
    /// The size distribution lies outside the table's grid.
    OffGrid {
        /// The family.
        kind: SizeDistributionKind,
        /// The effective radius asked for.
        effective_radius: Metres,
        /// The effective variance asked for.
        effective_variance: f64,
    },
    /// The temperature is not finite.
    NonFiniteTemperature {
        /// The temperature asked for.
        temperature: Kelvin,
    },
}

impl fmt::Display for ResolveAerosolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotModelled { key } => write!(f, "{key} is not modelled as an aerosol material"),
            Self::NoFile { key, phase } => {
                write!(
                    f,
                    "{key} has no index file as a {} of that variant",
                    phase_name(*phase)
                )
            }
            Self::NoDensity {
                key,
                phase,
                temperature,
            } => write!(
                f,
                "{key}'s {} grain density of that variant at {} K is not modelled",
                phase_name(*phase),
                temperature.value()
            ),
            Self::OffGrid {
                kind,
                effective_radius,
                effective_variance,
            } => write!(
                f,
                "a {} distribution of r_eff {} m and v_eff {effective_variance} lies outside the \
                 extinction table's grid",
                kind.name(),
                effective_radius.value()
            ),
            Self::NonFiniteTemperature { temperature } => {
                write!(f, "temperature {} K is not finite", temperature.value())
            }
        }
    }
}

impl Error for ResolveAerosolError {}

/// `solid` or `liquid`.
#[must_use]
const fn phase_name(phase: CondensedPhase) -> &'static str {
    match phase {
        CondensedPhase::Solid => "solid",
        CondensedPhase::Liquid => "liquid",
    }
}

/// One index file's entry in the extinction table, as `hyperion-fit`'s `condensate_extinction`
/// writes it: the file's identity, its stand-in flag, its basis with a phase table's sizes, and
/// its grids of `Q̄_ext` and ln ω at 550 nm.
///
/// Each grid's row i is the table's effective radius i
/// (`tables::condensate_extinction::EFFECTIVE_RADII_M`, the log-normal's rows the first of them),
/// and its column j the family's effective variance j.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct ExtinctionEntry {
    /// The file's name, in `apps/hyperion/src/renderer/src/view/atmosphere/materials/`.
    pub(crate) file: &'static str,
    /// The row's key.
    pub(crate) key: &'static str,
    /// The file's phase.
    pub(crate) phase: CondensedPhase,
    /// The file's variant, `None` for the substance itself.
    pub(crate) variant: Option<&'static str>,
    /// Whether the file's provenance is `standIn`.
    pub(crate) stand_in: bool,
    /// The particles drawn, with a phase table's sizes.
    pub(crate) basis: EntryBasis,
    /// `Q̄_ext` for the gamma distribution.
    pub(crate) gamma_extinction: &'static [[f64; GAMMA_VARIANCE_NODES]],
    /// ln ω for the gamma distribution.
    pub(crate) gamma_ln_albedo: &'static [[f64; GAMMA_VARIANCE_NODES]],
    /// `Q̄_ext` for the log-normal.
    pub(crate) log_normal_extinction: &'static [[f64; LOG_NORMAL_VARIANCE_NODES]],
    /// ln ω for the log-normal.
    pub(crate) log_normal_ln_albedo: &'static [[f64; LOG_NORMAL_VARIANCE_NODES]],
}

impl ExtinctionEntry {
    /// The index file's name, as `sulphuric-acid-75.json`.
    #[must_use]
    pub const fn file(&self) -> &'static str {
        self.file
    }

    /// The row's key, as `H2SO4`.
    #[must_use]
    pub const fn key(&self) -> &'static str {
        self.key
    }

    /// The file's phase.
    #[must_use]
    pub const fn phase(&self) -> CondensedPhase {
        self.phase
    }

    /// The file's variant, as `75wt%`, or `None` for the substance itself.
    #[must_use]
    pub const fn variant(&self) -> Option<&'static str> {
        self.variant
    }

    /// Whether the file is a stated stand-in.
    #[must_use]
    pub const fn is_stand_in(&self) -> bool {
        self.stand_in
    }

    /// The particles its grids were computed from, with a phase table's sizes.
    #[must_use]
    pub const fn basis(&self) -> EntryBasis {
        self.basis
    }

    /// A phase-table file's sizes at 550 nm, from its smallest particle's volume-to-area radius to
    /// its largest's, or `None` for a file with no phase table.
    #[must_use]
    pub const fn phase_table_radii(&self) -> Option<(Metres, Metres)> {
        match self.basis {
            EntryBasis::PhaseTable {
                radii_m: (low, high),
            } => Some((Metres::new(low), Metres::new(high))),
            EntryBasis::Sphere | EntryBasis::SphereFallback => None,
        }
    }
}

/// Every index file's entry, in the table's order: its seven parts in turn, each in file-name
/// order.
pub fn extinction_entries() -> impl Iterator<Item = &'static ExtinctionEntry> {
    condensate_extinction::ENTRIES
        .iter()
        .chain(&condensate_extinction_2::ENTRIES)
        .chain(&condensate_extinction_3::ENTRIES)
        .chain(&condensate_extinction_4::ENTRIES)
        .chain(&condensate_extinction_5::ENTRIES)
        .chain(&condensate_extinction_6::ENTRIES)
        .chain(&condensate_extinction_7::ENTRIES)
}

/// Which of a row's index files in a phase: the substance's own (a file whose `variant` is null,
/// as `water.json`), or the variant a file names (`75wt%` for `sulphuric-acid-75.json`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum FileVariant<'a> {
    /// The substance itself.
    #[default]
    Substance,
    /// A named variant, as the file writes it.
    Named(&'a str),
}

impl<'a> From<Option<&'a str>> for FileVariant<'a> {
    /// A file's `variant` field: `None` is the substance itself.
    fn from(variant: Option<&'a str>) -> Self {
        variant.map_or(Self::Substance, Self::Named)
    }
}

/// The variant whose density is the row's grain density: `75wt%` for `H2SO4`, and the substance
/// itself for every other row (`DENSITY_VARIANTS`).
#[must_use]
pub fn density_variant(key: &str) -> FileVariant<'static> {
    DENSITY_VARIANTS
        .iter()
        .find(|(row, _)| *row == key)
        .map_or(FileVariant::Substance, |&(_, variant)| {
            FileVariant::Named(variant)
        })
}

/// The four nodes of `nodes` nearest `at` and their weights in Lagrange's cubic through them, in
/// the coordinate `coordinate` (ln `r_eff` for the radii, ln(`v_eff` + 0.05) for the variances), or
/// `None` if `at` lies outside the nodes or there are fewer than four.
///
/// `nodes` increase, and `coordinate` increases with them. The nodes are the two either side of
/// `at`'s interval and one beyond each, shifted inwards at the ends, so the cubic is local; its
/// weights sum to 1 and reproduce any cubic in the coordinate exactly. It is the table's
/// interpolation, which `hyperion-fit` holds to its half-step check.
///
/// # Examples
///
/// ```
/// use hyperion_sim::substance::aerosol::cubic_weights;
///
/// let nodes = [1.0, 2.0, 3.0, 4.0, 5.0];
/// let (at, weights) = cubic_weights(&nodes, 2.5, |x| x).expect("2.5 lies inside the nodes");
/// let cube = |x: f64| x * x * x - x;
/// let interpolated: f64 = at.iter().zip(weights).map(|(&i, w)| w * cube(nodes[i])).sum();
/// assert!((interpolated - cube(2.5)).abs() < 1e-12);
/// ```
#[must_use]
pub fn cubic_weights(
    nodes: &[f64],
    at: f64,
    coordinate: impl Fn(f64) -> f64,
) -> Option<([usize; 4], [f64; 4])> {
    let n = nodes.len();
    if n < 4 || !(at >= nodes[0] && at <= nodes[n - 1]) {
        return None;
    }
    let interval = nodes.partition_point(|&node| node <= at).clamp(1, n - 1) - 1;
    let start = interval.saturating_sub(1).min(n - 4);
    let indices = [start, start + 1, start + 2, start + 3];
    let x = coordinate(at);
    let xs = indices.map(|i| coordinate(nodes[i]));
    let mut weights = [1.0; 4];
    for (a, weight) in weights.iter_mut().enumerate() {
        for (b, &xb) in xs.iter().enumerate() {
            if a != b {
                *weight *= (x - xb) / (xs[a] - xb);
            }
        }
    }
    Some((indices, weights))
}

/// How a grid is interpolated: of ln value (`Q̄_ext`, positive) or of the value itself (ln ω).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GridSpace {
    /// Lagrange's cubic of ln value.
    Logarithmic,
    /// Lagrange's cubic of the value.
    Linear,
}

/// A table grid's value at radius `radius_m`, m, and variance `variance`: Lagrange's cubic over
/// the sixteen nearest nodes ([`cubic_weights`]) in ln r and in ln(v + `offset`), of ln value or
/// of the value (`space`); `None` off the grid, or if `rows` and the axes disagree.
///
/// `rows` holds one row per radius of `radii` (its first `rows.len()`), one value per variance of
/// `variances`. It is the table's interpolation, which [`aerosol_optics`] reads the committed
/// grids by and `hyperion-fit`'s half-step check holds its fitted grids to.
///
/// # Examples
///
/// A grid that is a cubic in both coordinates is reproduced between its nodes:
///
/// ```
/// use hyperion_sim::math;
/// use hyperion_sim::substance::aerosol::{GridSpace, interpolate_grid};
///
/// let radii = [1e-7, 2e-7, 4e-7, 8e-7, 1.6e-6];
/// let variances = [0.05, 0.1, 0.2, 0.3];
/// let f = |r: f64, v: f64| math::ln(r) * 2.0 + math::ln(v + 0.05);
/// let rows: Vec<Vec<f64>> = radii
///     .iter()
///     .map(|&r| variances.iter().map(|&v| f(r, v)).collect())
///     .collect();
/// let value = interpolate_grid(&rows, &radii, &variances, 0.05, 3e-7, 0.15, GridSpace::Linear)
///     .expect("the point lies inside the grid");
/// assert!((value - f(3e-7, 0.15)).abs() < 1e-9);
/// ```
#[must_use]
pub fn interpolate_grid<R: AsRef<[f64]>>(
    rows: &[R],
    radii: &[f64],
    variances: &[f64],
    offset: f64,
    radius_m: f64,
    variance: f64,
    space: GridSpace,
) -> Option<f64> {
    let radii = radii.get(..rows.len())?;
    if rows.iter().any(|row| row.as_ref().len() != variances.len()) {
        return None;
    }
    let (ri, rw) = cubic_weights(radii, radius_m, math::ln)?;
    let (vi, vw) = cubic_weights(variances, variance, |v| math::ln(v + offset))?;
    let mut sum = 0.0;
    for (a, wr) in rw.iter().enumerate() {
        for (b, wv) in vw.iter().enumerate() {
            let value = rows[ri[a]].as_ref()[vi[b]];
            let term = match space {
                GridSpace::Logarithmic => math::ln(value),
                GridSpace::Linear => value,
            };
            sum += wr * wv * term;
        }
    }
    Some(match space {
        GridSpace::Logarithmic => math::exp(sum),
        GridSpace::Linear => sum,
    })
}

/// The committed grid `rows` at `radius_m` and `variance`, on the table's axes
/// ([`interpolate_grid`]).
#[must_use]
fn interpolate<const N: usize>(
    rows: &[[f64; N]],
    variances: &[f64],
    radius_m: f64,
    variance: f64,
    space: GridSpace,
) -> Option<f64> {
    interpolate_grid(
        rows,
        &condensate_extinction::EFFECTIVE_RADII_M,
        variances,
        condensate_extinction::EFFECTIVE_VARIANCE_OFFSET,
        radius_m,
        variance,
        space,
    )
}

/// The optics at 550 nm of a mode of row `id`'s particles in `phase`, of its index file of
/// variant `variant`, sized by `distribution`, at `temperature`, which sets the grain density (the
/// [module's](self) model).
///
/// # Errors
///
/// - [`ResolveAerosolError::NonFiniteTemperature`] if `temperature` is not finite;
/// - [`ResolveAerosolError::NotModelled`] if the row is not an aerosol material;
/// - [`ResolveAerosolError::NoFile`] if no index file is the row's in that phase and variant;
/// - [`ResolveAerosolError::NoDensity`] if the row has no grain density there: the phase is not
///   read, the temperature lies outside its phase diagram's range, or the variant is not the one
///   whose density the row holds ([`density_variant`]);
/// - [`ResolveAerosolError::OffGrid`] if the distribution lies outside the table's grid.
///
/// # Examples
///
/// Venus's mode 2 (modal radius 1.0 µm and ln σ 0.25, so `r_eff` 1.17 µm and `v_eff` 0.065: J. B.
/// Pollack et al., Icarus 103, 1–42, 1993) of the 75 wt% acid extinguishes some 900 m² per
/// kilogram of droplets:
///
/// ```
/// use hyperion_sim::substance::aerosol::{FileVariant, SizeDistribution, aerosol_optics};
/// use hyperion_sim::substance::{CondensedPhase, by_key};
/// use hyperion_sim::units::{Kelvin, Metres};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let acid = by_key("H2SO4").ok_or("H2SO4 is a registry row")?;
/// let mode = SizeDistribution::log_normal(Metres::new(1.17e-6), 0.065)?;
/// let liquid = CondensedPhase::Liquid;
/// let optics = aerosol_optics(acid, liquid, FileVariant::Named("75wt%"), &mode, Kelvin::new(250.0))?;
/// assert!((850.0..1_000.0).contains(&optics.mass_extinction_m2_per_kg()));
/// assert!(optics.single_scattering_albedo() > 0.999_99);
/// # Ok(())
/// # }
/// ```
pub fn aerosol_optics(
    id: SubstanceId,
    phase: CondensedPhase,
    variant: FileVariant<'_>,
    distribution: &SizeDistribution,
    temperature: Kelvin,
) -> Result<AerosolOptics, ResolveAerosolError> {
    if !temperature.value().is_finite() {
        return Err(ResolveAerosolError::NonFiniteTemperature { temperature });
    }
    let row = id.substance();
    let key = row.key();
    if !row.optics().aerosol_material() {
        return Err(ResolveAerosolError::NotModelled { key });
    }
    let entry = extinction_entries()
        .find(|e| {
            e.key == key.as_str() && e.phase == phase && FileVariant::from(e.variant) == variant
        })
        .ok_or(ResolveAerosolError::NoFile { key, phase })?;
    let no_density = ResolveAerosolError::NoDensity {
        key,
        phase,
        temperature,
    };
    if variant != density_variant(key.as_str()) {
        return Err(no_density);
    }
    let density = row.grain_density(phase, temperature).ok_or(no_density)?;
    let radius = distribution.effective_radius().value();
    let variance = distribution.effective_variance();
    let off_grid = ResolveAerosolError::OffGrid {
        kind: distribution.kind(),
        effective_radius: distribution.effective_radius(),
        effective_variance: variance,
    };
    let (extinction, ln_albedo) = match distribution.kind() {
        SizeDistributionKind::Gamma => {
            let variances = &condensate_extinction::GAMMA_EFFECTIVE_VARIANCES;
            (
                interpolate(
                    entry.gamma_extinction,
                    variances,
                    radius,
                    variance,
                    GridSpace::Logarithmic,
                ),
                interpolate(
                    entry.gamma_ln_albedo,
                    variances,
                    radius,
                    variance,
                    GridSpace::Linear,
                ),
            )
        }
        SizeDistributionKind::LogNormal => {
            let variances = &condensate_extinction::LOG_NORMAL_EFFECTIVE_VARIANCES;
            (
                interpolate(
                    entry.log_normal_extinction,
                    variances,
                    radius,
                    variance,
                    GridSpace::Logarithmic,
                ),
                interpolate(
                    entry.log_normal_ln_albedo,
                    variances,
                    radius,
                    variance,
                    GridSpace::Linear,
                ),
            )
        }
    };
    let (Some(extinction), Some(ln_albedo)) = (extinction, ln_albedo) else {
        return Err(off_grid);
    };
    let extinction_basis = match entry.basis {
        EntryBasis::Sphere => ExtinctionBasis::Sphere,
        EntryBasis::PhaseTable {
            radii_m: (low, high),
        } => {
            if (low..=high).contains(&radius) {
                ExtinctionBasis::PhaseTable
            } else {
                ExtinctionBasis::SpheresOutsidePhaseTable
            }
        }
        EntryBasis::SphereFallback => ExtinctionBasis::SphereFallback,
    };
    Ok(AerosolOptics {
        mass_extinction_m2_per_kg: 3.0 * extinction / (4.0 * density.value() * radius),
        extinction_efficiency: extinction,
        // ln ω is at most 0, which a cubic through nodes of 0 (a lossless file's) keeps exactly;
        // a comparison, not `min`, so that neither zero's sign is left to the platform's `min`.
        single_scattering_albedo: if ln_albedo >= 0.0 {
            1.0
        } else {
            math::exp(ln_albedo)
        },
        stand_in: entry.stand_in,
        extinction_basis,
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use hyperion_testkit::float::assert_same_bits;

    use super::super::thermal::tests::{
        before_inline_tests, names, out_of_line_test_modules, rust_files,
    };
    use super::*;
    use crate::substance::{by_key, ids};

    /// The files outside this module and its tables whose reads of its API sit behind a named
    /// hold, each with it: none yet. **P14.T54.b deletes this list** with the tripwire below.
    const HELD: [(&str, &str); 0] = [];

    /// The forms by which other code would read this module's extinction or its tables: its calls,
    /// its paths and its types, and the tables' name.
    const API_PATTERNS: [&str; 6] = [
        "aerosol_optics(",
        "substance::aerosol",
        "AerosolOptics",
        "extinction_entries(",
        "interpolate_grid(",
        "condensate_extinction",
    ];

    /// The lines that declare this module and its tables and re-export the module, each by its
    /// file and the start of its line: `substance/mod.rs`'s module line and its `pub use
    /// aerosol::{…};` statement, whatever lines that takes, and `tables/mod.rs`'s module lines and
    /// the entries of the tables' `MANIFEST`, which `hyperion-fit` writes.
    const DECLARATIONS: [(&str, &str); 4] = [
        ("substance/mod.rs", "pub mod aerosol;"),
        ("substance/mod.rs", "pub use aerosol::"),
        ("tables/mod.rs", "pub mod condensate_extinction"),
        ("tables/mod.rs", "name: \"condensate_extinction"),
    ];

    /// Whether the file at `relative` (under the crate's `src/`) is this module's or one of its
    /// seven tables.
    fn own(relative: &str) -> bool {
        relative == "substance/aerosol.rs"
            || (relative.starts_with("tables/condensate_extinction")
                && Path::new(relative)
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("rs")))
    }

    /// The `(line, pattern)` pairs, 1-based, by which the code of the file at `relative`, of text
    /// `text`, names [`API_PATTERNS`] before its inline test module: its comment lines and its
    /// [`DECLARATIONS`] (a statement to its `;`) left out, each pattern as a whole name (`names`).
    fn readers(relative: &str, text: &str) -> Vec<(usize, &'static str)> {
        let mut found = Vec::new();
        let mut in_declaration = false;
        for (n, line) in before_inline_tests(text).iter().enumerate() {
            let code = line.trim_start();
            if in_declaration {
                in_declaration = !code.trim_end().ends_with(';');
                continue;
            }
            if DECLARATIONS
                .iter()
                .any(|(file, start)| *file == relative && code.starts_with(start))
            {
                in_declaration = !code.trim_end().ends_with(';') && !code.ends_with(',');
                continue;
            }
            if code.starts_with("//") {
                continue;
            }
            for pattern in API_PATTERNS {
                if names(line, pattern) {
                    found.push((n + 1, pattern));
                }
            }
        }
        found
    }

    /// The scan reads the code alone: it finds a call, a path and a type, leaves out comments, the
    /// declarations (a re-export over several lines whole) and the inline test module, and reads
    /// whole names only.
    #[test]
    fn substance_aerosol_the_tripwire_s_scan_reads_the_code_alone() {
        let text = "// aerosol_optics(id, phase, variant, &mode, t)\n\
                    let optics = substance::aerosol::aerosol_optics(id, p, v, &m, t)?;\n\
                    let my_condensate_extinction = 1;\n\
                    #[cfg(test)]\nmod tests {\n    fn f() { extinction_entries(); }\n}\n";
        assert_eq!(
            readers("planetary/x.rs", text),
            [(2, "aerosol_optics("), (2, "substance::aerosol")]
        );
        let declarations = "pub mod aerosol;\npub use aerosol::{\n    AerosolOptics, aerosol_optics,\n};\n\
                            pub use condensed::CondensedPhase;\nfn f(o: AerosolOptics) {}\n";
        assert_eq!(
            readers("substance/mod.rs", declarations),
            [(6, "AerosolOptics")]
        );
        let tables = "pub mod condensate_extinction_2;\n    TableInfo {\n        name: \
                      \"condensate_extinction_2\",\n";
        assert_eq!(readers("tables/mod.rs", tables), Vec::new());
        assert!(own("tables/condensate_extinction_7.rs") && !own("tables/mod.rs"));
    }

    /// Nothing on the record reads the aerosol extinction before P14.T54.b, its bump
    /// (decision-bump-plan-after-22 §1.3): no code of the crate's `src/` outside this module and
    /// its seven tables names [`API_PATTERNS`], every file read in sorted order by
    /// `substance::thermal`'s tripwire's walk, but each inline test module, every file an
    /// out-of-line test module declares, [`DECLARATIONS`] and [`HELD`]'s files (none). **P14.T54.b
    /// deletes this tripwire** when P14.T52.c and T24.c first read it.
    #[test]
    fn substance_aerosol_nothing_on_the_record_reads_the_extinction_yet() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        rust_files(&src, &src, &mut files);
        assert!(files.len() > 100, "the walk found the crate's sources");
        let read = |path: &Path| {
            std::fs::read_to_string(path)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
        };
        assert_eq!(
            files.iter().filter(|(relative, _)| own(relative)).count(),
            8,
            "the module and its seven tables"
        );
        for (held, hold) in HELD {
            assert!(!hold.is_empty(), "{held} names its hold");
            assert!(
                files.iter().any(|(relative, _)| relative == held),
                "{held} is a source file"
            );
        }
        let mut tests = Vec::new();
        for (relative, path) in &files {
            tests.extend(out_of_line_test_modules(relative, &read(path)));
        }
        let mut found = Vec::new();
        for (relative, path) in &files {
            let test_file = tests
                .iter()
                .any(|t| relative == t || (t.ends_with('/') && relative.starts_with(t.as_str())));
            if test_file || own(relative) || HELD.iter().any(|(held, _)| held == relative) {
                continue;
            }
            for (line, pattern) in readers(relative, &read(path)) {
                found.push(format!("{relative}:{line}: {pattern}"));
            }
        }
        assert!(
            found.is_empty(),
            "nothing on the record reads substance::aerosol before P14.T54.b: {found:#?}"
        );
    }

    fn gamma(radius_m: f64, variance: f64) -> SizeDistribution {
        SizeDistribution::gamma(Metres::new(radius_m), variance).unwrap()
    }

    fn log_normal(radius_m: f64, variance: f64) -> SizeDistribution {
        SizeDistribution::log_normal(Metres::new(radius_m), variance).unwrap()
    }

    fn id(key: &str) -> SubstanceId {
        by_key(key).unwrap()
    }

    /// Every aerosol-material row has at least one index file's entry, every entry is an aerosol
    /// material's, and no two entries share a key, phase and variant.
    #[test]
    fn substance_aerosol_every_aerosol_material_has_an_entry() {
        for row in ids() {
            let substance = row.substance();
            let key = substance.key();
            let entries = extinction_entries()
                .filter(|e| e.key() == key.as_str())
                .count();
            assert_eq!(
                substance.optics().aerosol_material(),
                entries > 0,
                "{key}: {entries} entries"
            );
        }
        let mut seen: Vec<(&str, CondensedPhase, Option<&str>)> = Vec::new();
        for entry in extinction_entries() {
            let identity = (entry.key(), entry.phase(), entry.variant());
            assert!(!seen.contains(&identity), "{identity:?} twice");
            seen.push(identity);
        }
        assert_eq!(seen.len(), 49);
    }

    /// Each grid has a row per radius of its family (every radius for the gamma distribution, the
    /// first 100 for the log-normal) and a column per variance, and the axes rise.
    #[test]
    fn substance_aerosol_the_grids_match_their_axes() {
        let radii = &condensate_extinction::EFFECTIVE_RADII_M;
        let gamma_v = &condensate_extinction::GAMMA_EFFECTIVE_VARIANCES;
        let log_normal_v = &condensate_extinction::LOG_NORMAL_EFFECTIVE_VARIANCES;
        for axis in [&radii[..], &gamma_v[..], &log_normal_v[..]] {
            assert!(axis.windows(2).all(|w| w[0] < w[1]), "{axis:?}");
        }
        assert_eq!(gamma_v.len(), GAMMA_VARIANCE_NODES);
        assert_eq!(log_normal_v.len(), LOG_NORMAL_VARIANCE_NODES);
        assert!((radii[0] - 1e-9).abs() < 1e-24, "{}", radii[0]);
        for entry in extinction_entries() {
            assert_eq!(
                entry.gamma_extinction.len(),
                radii.len(),
                "{}",
                entry.file()
            );
            assert_eq!(entry.gamma_ln_albedo.len(), radii.len(), "{}", entry.file());
            assert_eq!(entry.log_normal_extinction.len(), 100, "{}", entry.file());
            assert_eq!(entry.log_normal_ln_albedo.len(), 100, "{}", entry.file());
            let positive = entry.gamma_extinction.iter().flatten().all(|&q| q > 0.0)
                && entry
                    .log_normal_extinction
                    .iter()
                    .flatten()
                    .all(|&q| q > 0.0);
            let albedo = entry.gamma_ln_albedo.iter().flatten().all(|&a| a <= 0.0)
                && entry
                    .log_normal_ln_albedo
                    .iter()
                    .flatten()
                    .all(|&a| a <= 0.0);
            assert!(positive && albedo, "{}", entry.file());
        }
    }

    /// At a node the optics are the table's: `Q̄_ext` and ln ω as tabled, and `κ_ext` = 3 `Q̄_ext` ÷
    /// (4 ρ `r_eff`) with ρ the row's grain density, to rounding.
    #[test]
    fn substance_aerosol_reproduces_the_table_at_its_nodes() {
        let radii = &condensate_extinction::EFFECTIVE_RADII_M;
        let variances = &condensate_extinction::GAMMA_EFFECTIVE_VARIANCES;
        let entry = extinction_entries()
            .find(|e| e.file() == "water.json")
            .unwrap();
        let water = id("H2O");
        let temperature = Kelvin::new(300.0);
        let density = water
            .substance()
            .grain_density(CondensedPhase::Liquid, temperature)
            .unwrap()
            .value();
        for (k, j) in [(0, 0), (40, 5), (106, 11), (60, 3)] {
            let optics = aerosol_optics(
                water,
                CondensedPhase::Liquid,
                FileVariant::Substance,
                &gamma(radii[k], variances[j]),
                temperature,
            )
            .unwrap();
            let q = entry.gamma_extinction[k][j];
            assert!(
                (optics.extinction_efficiency() / q - 1.0).abs() < 1e-14,
                "{k}, {j}"
            );
            let kappa = 3.0 * q / (4.0 * density * radii[k]);
            assert!(
                (optics.mass_extinction_m2_per_kg() / kappa - 1.0).abs() < 1e-14,
                "{k}, {j}"
            );
            let omega = math::exp(entry.gamma_ln_albedo[k][j]);
            assert!((optics.single_scattering_albedo() / omega - 1.0).abs() < 1e-14);
        }
    }

    /// `acid_uv_absorber`'s index at 550 nm is the 75 wt% acid's (its absorber's k is 0 there,
    /// R08.T5.d), and so is its density, so its optics equal `H2SO4`'s at that variant to the bit.
    #[test]
    fn substance_aerosol_acid_uv_absorber_is_the_75_wt_acid_at_550_nm() {
        let temperature = Kelvin::new(240.0);
        for mode in [
            gamma(2e-8, 0.1),
            gamma(1.05e-6, 0.07),
            gamma(30e-6, 0.3),
            log_normal(0.487e-6, 0.214),
            log_normal(4.27e-6, 0.064),
        ] {
            let acid = aerosol_optics(
                id("H2SO4"),
                CondensedPhase::Liquid,
                FileVariant::Named("75wt%"),
                &mode,
                temperature,
            )
            .unwrap();
            let absorber = aerosol_optics(
                id("acid_uv_absorber"),
                CondensedPhase::Liquid,
                FileVariant::Substance,
                &mode,
                temperature,
            )
            .unwrap();
            assert_same_bits(
                absorber.mass_extinction_m2_per_kg(),
                acid.mass_extinction_m2_per_kg(),
            );
            assert_same_bits(
                absorber.single_scattering_albedo(),
                acid.single_scattering_albedo(),
            );
            assert!(absorber.is_stand_in() && !acid.is_stand_in());
        }
    }

    /// Venus's four deck modes through the table and the 75 wt% acid's file (n 1.431 at 550 nm),
    /// combined by mass: κ = Σ N `C̄_ext` ÷ Σ N ρ ⟨V⟩. The modes' radii are Pollack et al. 1993's
    /// (Icarus 103, 1–42), their ln σ rounded to two decimals as Reyes-Guerrero, Pérez-Hoyos and
    /// Garate-Lopez 2026 print them (arXiv:2607.22187, §3.1), from Pollack et al.'s `σ_g` = 1.56,
    /// 1.29, 1.23 and 1.28 (R. Haus et al. 2016, Icarus 272, 178–205, §2), and their columns
    /// N₀ (`z_c` + `H_up` + `H_lo`) of Haus et al.'s profiles, as Reyes-Guerrero et al. tabulate them
    /// (Table 2; science-r08-sulphur-co2ice.md). The acid is a `sphere` file, so its entry is
    /// Mie's. The sulphur note's 417 m² kg⁻¹ is at n = 1.44 (the fit's test holds that to 2%); the
    /// file's index and the table's interpolation keep it within 1%.
    #[test]
    fn substance_aerosol_venus_s_deck_is_the_sulphur_note_s() {
        let acid = id("H2SO4");
        let temperature = Kelvin::new(250.0);
        let mut extinction = 0.0;
        let mut mass = 0.0;
        for (modal_radius_m, ln_sigma, column_per_cm2) in [
            (0.30e-6, 0.44, 3.97e8),
            (1.00e-6, 0.25, 7.50e7),
            (1.40e-6, 0.21, 6.05e7),
            (3.65e-6, 0.25, 1.33e7),
        ] {
            let s2: f64 = ln_sigma * ln_sigma;
            let mode = log_normal(modal_radius_m * math::exp(2.5 * s2), math::exp_m1(s2));
            let optics = aerosol_optics(
                acid,
                CondensedPhase::Liquid,
                FileVariant::Named("75wt%"),
                &mode,
                temperature,
            )
            .unwrap();
            // ⟨πr²⟩ = π r_g² e^(2σ²) per particle; ⟨V⟩ = (4/3) r_eff ⟨πr²⟩.
            let area = std::f64::consts::PI * modal_radius_m * modal_radius_m * math::exp(2.0 * s2);
            let column = column_per_cm2 * 1e4;
            extinction += column * optics.extinction_efficiency() * area;
            mass += column * SULPHURIC_ACID_75_DENSITY_KG_M3 * 4.0 / 3.0
                * mode.effective_radius().value()
                * area;
        }
        let kappa = extinction / mass;
        assert!((kappa / 417.0 - 1.0).abs() < 0.01, "{kappa} m² kg⁻¹");
        assert!((extinction - 34.5).abs() < 0.5, "τ {extinction}");
    }

    /// The 75 wt% acid's density, `rows/condensed.rs`'s.
    const SULPHURIC_ACID_75_DENSITY_KG_M3: f64 = 1_662.6;

    /// A stand-in file's entry carries its flag through to the optics, and a measured one does not.
    #[test]
    fn substance_aerosol_a_stand_in_file_carries_its_flag() {
        let mode = gamma(1e-6, 0.1);
        let haze = aerosol_optics(
            id("ice_giant_haze"),
            CondensedPhase::Solid,
            FileVariant::Substance,
            &mode,
            Kelvin::new(80.0),
        )
        .unwrap();
        let ice = aerosol_optics(
            id("H2O"),
            CondensedPhase::Solid,
            FileVariant::Substance,
            &mode,
            Kelvin::new(200.0),
        )
        .unwrap();
        assert!(haze.is_stand_in());
        assert!(!ice.is_stand_in());
        let stand_ins: Vec<&str> = extinction_entries()
            .filter(|e| e.is_stand_in())
            .map(ExtinctionEntry::file)
            .collect();
        assert!(stand_ins.contains(&"ammonia-ice.json"), "{stand_ins:?}");
        assert!(!stand_ins.contains(&"water.json"), "{stand_ins:?}");
    }

    /// Large particles reach the geometric limit, `κ_ext` → 3 × 2 ÷ (4 ρ `r_eff`), and a lossless
    /// file's albedo is 1.
    #[test]
    fn substance_aerosol_large_particles_meet_the_geometric_limit() {
        let water = id("H2O");
        let t = Kelvin::new(290.0);
        let radius = 100e-6;
        let optics = aerosol_optics(
            water,
            CondensedPhase::Liquid,
            FileVariant::Substance,
            &gamma(radius, 0.1),
            t,
        )
        .unwrap();
        assert!(
            (optics.extinction_efficiency() / 2.0 - 1.0).abs() < 0.01,
            "{optics:?}"
        );
        let salt = aerosol_optics(
            id("NaCl"),
            CondensedPhase::Solid,
            FileVariant::Substance,
            &gamma(radius, 0.1),
            t,
        )
        .unwrap();
        assert_same_bits(salt.single_scattering_albedo(), 1.0);
    }

    /// Each refusal is its own: a row that is no aerosol material, a phase or variant with no
    /// file, a file whose density is not the row's or not read, a size off the grid, and a
    /// temperature that is not finite.
    #[test]
    fn substance_aerosol_refuses_what_it_cannot_serve() {
        let mode = gamma(1e-6, 0.1);
        let t = Kelvin::new(250.0);
        let liquid = CondensedPhase::Liquid;
        let solid = CondensedPhase::Solid;
        let resolve = |key: &str, phase, variant: Option<&str>, mode: &SizeDistribution, t| {
            aerosol_optics(id(key), phase, variant.into(), mode, t)
        };
        for key in ["granite", "Ne", "SO2"] {
            assert!(
                matches!(
                    resolve(key, solid, None, &mode, t),
                    Err(ResolveAerosolError::NotModelled { .. })
                ),
                "{key}"
            );
        }
        for (key, phase, variant) in [
            ("H2SO4", liquid, None),
            ("H2SO4", solid, Some("75wt%")),
            ("C2H6", solid, None),
            ("H2O", liquid, Some("S36")),
        ] {
            assert!(
                matches!(
                    resolve(key, phase, variant, &mode, t),
                    Err(ResolveAerosolError::NoFile { .. })
                ),
                "{key} {phase:?} {variant:?}"
            );
        }
        for (key, phase, variant, temperature) in [
            ("H2SO4", liquid, Some("84.5wt%"), 250.0),
            ("H2O", liquid, Some("S35"), 290.0),
            ("Ar", solid, None, 60.0),
            ("H2S", solid, None, 150.0),
            ("S8", liquid, None, 400.0),
            ("H2O", solid, None, 300.0),
        ] {
            assert!(
                matches!(
                    resolve(key, phase, variant, &mode, Kelvin::new(temperature)),
                    Err(ResolveAerosolError::NoDensity { .. })
                ),
                "{key} {phase:?} {variant:?}"
            );
        }
        for off in [
            gamma(0.5e-9, 0.1),
            gamma(150e-6, 0.1),
            gamma(1e-6, 0.03),
            gamma(1e-6, 0.47),
            log_normal(40e-6, 0.1),
            log_normal(1e-6, 0.7),
        ] {
            assert!(
                matches!(
                    resolve("H2O", liquid, None, &off, Kelvin::new(290.0)),
                    Err(ResolveAerosolError::OffGrid { .. })
                ),
                "{off:?}"
            );
        }
        assert!(matches!(
            resolve("H2O", liquid, None, &mode, Kelvin::new(f64::NAN)),
            Err(ResolveAerosolError::NonFiniteTemperature { .. })
        ));
    }

    /// A distribution needs a finite positive radius, and a variance in (0, 0.5) for the gamma
    /// distribution or positive for the log-normal.
    #[test]
    fn substance_aerosol_size_distributions_hold_their_ranges() {
        let r = Metres::new(1e-6);
        for v in [0.0, -0.1, 0.5, f64::NAN] {
            assert!(matches!(
                SizeDistribution::gamma(r, v),
                Err(BuildSizeDistributionError::Variance { .. })
            ));
        }
        for v in [0.0, -0.1, f64::NAN, f64::INFINITY] {
            assert!(matches!(
                SizeDistribution::log_normal(r, v),
                Err(BuildSizeDistributionError::Variance { .. })
            ));
        }
        assert!(SizeDistribution::log_normal(r, 0.9).is_ok());
        for radius in [0.0, -1e-6, f64::INFINITY, f64::NAN] {
            assert!(matches!(
                SizeDistribution::log_normal(Metres::new(radius), 0.1),
                Err(BuildSizeDistributionError::Radius { .. })
            ));
        }
        let mode = gamma(2e-6, 0.2);
        assert_eq!(mode.kind(), SizeDistributionKind::Gamma);
        assert_same_bits(mode.effective_radius().value(), 2e-6);
        assert_same_bits(mode.effective_variance(), 0.2);
    }

    /// The cubic's weights sum to 1, reproduce a cubic in the coordinate on uneven nodes and at
    /// the ends, and refuse a point outside the nodes.
    #[test]
    fn substance_aerosol_cubic_weights_are_lagrange_s() {
        let nodes = [1.0, 1.5, 2.5, 4.0, 4.2, 7.0];
        let cubic = |x: f64| 2.0 * x * x * x - x * x + 3.0;
        for at in [1.0, 1.2, 2.0, 3.9, 4.1, 6.5, 7.0] {
            let (indices, weights) = cubic_weights(&nodes, at, |x| x).unwrap();
            let value: f64 = indices
                .iter()
                .zip(weights)
                .map(|(&i, w)| w * cubic(nodes[i]))
                .sum();
            assert!((value - cubic(at)).abs() < 1e-9, "{at}");
            assert!((weights.iter().sum::<f64>() - 1.0).abs() < 1e-12, "{at}");
            assert!(
                indices.contains(&nodes.partition_point(|&n| n < at).min(5)),
                "{at}"
            );
        }
        assert_eq!(cubic_weights(&nodes, 0.9, |x| x), None);
        assert_eq!(cubic_weights(&nodes, 7.1, |x| x), None);
        assert_eq!(cubic_weights(&nodes[..3], 1.2, |x| x), None);
    }

    /// The grid's documented ends are on it: `r_eff` 1 nm, `v_eff` 0.04 and 0.45 (gamma) or 0.65
    /// (log-normal), and the last radius of each family.
    #[test]
    fn substance_aerosol_the_grid_s_ends_resolve() {
        let radii = &condensate_extinction::EFFECTIVE_RADII_M;
        let water = id("H2O");
        let t = Kelvin::new(290.0);
        for mode in [
            gamma(1e-9, 0.04),
            gamma(1e-9, 0.45),
            gamma(radii[radii.len() - 1], 0.04),
            log_normal(1e-9, 0.04),
            log_normal(1e-9, 0.65),
            log_normal(radii[99], 0.65),
        ] {
            assert!(
                aerosol_optics(
                    water,
                    CondensedPhase::Liquid,
                    FileVariant::Substance,
                    &mode,
                    t
                )
                .is_ok(),
                "{mode:?}"
            );
        }
        assert_same_bits(radii[0], 1e-9);
        assert_same_bits(condensate_extinction::GAMMA_EFFECTIVE_VARIANCES[0], 0.04);
        assert_same_bits(condensate_extinction::GAMMA_EFFECTIVE_VARIANCES[11], 0.45);
        assert_same_bits(
            condensate_extinction::LOG_NORMAL_EFFECTIVE_VARIANCES[13],
            0.65,
        );
    }

    /// Each entry's basis is its shape class's (plan 14's record of P14.T49.e): 17 of the 21 files
    /// with a phase table take it and carry its sizes, the nine other ices and the four minerals
    /// outside the hexahedra's kernels fall back to spheres, and the rest are spheres. A mode is
    /// labelled by where its `r_eff` lies against its table's sizes: iron's longwave table ends at
    /// 23 µm and water ice's begins at 0.32 µm.
    #[test]
    fn substance_aerosol_each_entry_carries_its_basis() {
        let count =
            |is: fn(EntryBasis) -> bool| extinction_entries().filter(|e| is(e.basis())).count();
        assert_eq!(count(|b| matches!(b, EntryBasis::PhaseTable { .. })), 17);
        assert_eq!(count(|b| matches!(b, EntryBasis::SphereFallback)), 13);
        assert_eq!(count(|b| matches!(b, EntryBasis::Sphere)), 19);
        let fe = |radius_m: f64| {
            aerosol_optics(
                id("Fe"),
                CondensedPhase::Solid,
                FileVariant::Substance,
                &gamma(radius_m, 0.1),
                Kelvin::new(1_500.0),
            )
            .unwrap()
            .extinction_basis()
        };
        assert_eq!(fe(1e-6), ExtinctionBasis::PhaseTable);
        assert_eq!(fe(50e-6), ExtinctionBasis::SpheresOutsidePhaseTable);
        let solid = |key: &str, radius_m: f64, t: f64| {
            aerosol_optics(
                id(key),
                CondensedPhase::Solid,
                FileVariant::Substance,
                &gamma(radius_m, 0.1),
                Kelvin::new(t),
            )
            .unwrap()
            .extinction_basis()
        };
        assert_eq!(solid("H2O", 30e-6, 220.0), ExtinctionBasis::PhaseTable);
        assert_eq!(
            solid("H2O", 0.1e-6, 220.0),
            ExtinctionBasis::SpheresOutsidePhaseTable
        );
        assert_eq!(solid("NH3", 1e-6, 120.0), ExtinctionBasis::SphereFallback);
        let water = aerosol_optics(
            id("H2O"),
            CondensedPhase::Liquid,
            FileVariant::Substance,
            &gamma(1e-6, 0.1),
            Kelvin::new(290.0),
        )
        .unwrap();
        assert_eq!(water.extinction_basis(), ExtinctionBasis::Sphere);
    }

    /// A lossless file's ω is 1 to the bit at its nodes and between them, and its committed ln ω
    /// is +0.0 at every node (the fit writes no −0.0): aluminium oxide's sphere fallback, sodium
    /// chloride's floor table and liquid argon's spheres.
    #[test]
    fn substance_aerosol_a_lossless_file_s_albedo_is_one_to_the_bit() {
        for (key, phase, t) in [
            ("Al2O3", CondensedPhase::Solid, 1_800.0),
            ("NaCl", CondensedPhase::Solid, 900.0),
            ("Ar", CondensedPhase::Liquid, 85.0),
        ] {
            let entry = extinction_entries()
                .find(|e| e.key() == key && e.phase() == phase)
                .unwrap();
            let gamma_values = entry.gamma_ln_albedo.iter().flatten();
            for &value in gamma_values.chain(entry.log_normal_ln_albedo.iter().flatten()) {
                assert_same_bits(value, 0.0);
            }
            for mode in [
                gamma(1e-9, 0.04),
                gamma(0.37e-6, 0.137),
                gamma(5.3e-6, 0.3),
                log_normal(2.1e-8, 0.61),
                log_normal(30e-6, 0.05),
            ] {
                let optics = aerosol_optics(
                    id(key),
                    phase,
                    FileVariant::Substance,
                    &mode,
                    Kelvin::new(t),
                )
                .unwrap();
                assert_same_bits(optics.single_scattering_albedo(), 1.0);
            }
        }
    }

    /// A file's null variant is the substance itself, and a named one its name.
    #[test]
    fn substance_aerosol_file_variants_follow_the_files() {
        assert_eq!(FileVariant::from(None), FileVariant::Substance);
        assert_eq!(
            FileVariant::from(Some("75wt%")),
            FileVariant::Named("75wt%")
        );
        assert_eq!(density_variant("H2SO4"), FileVariant::Named("75wt%"));
        assert_eq!(density_variant("H2O"), FileVariant::Substance);
    }
}
