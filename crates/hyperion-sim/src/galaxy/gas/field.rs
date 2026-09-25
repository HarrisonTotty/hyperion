//! The gas field of one galaxy: density, pressure, phase and dust anywhere in the root cube (plan
//! 07, P07.T6).
//!
//! [`GasField`] assembles the parts: the smooth layers ([`SmoothGas`]), the lanes on the neutral
//! layer ([`Lanes`]), the lattice noise ([`log_normal_factor`]), the pressure ([`Pressure`]), the
//! phases ([`GasPhase`]) and the dust-to-gas ratio that follows the young thin disc's metallicity.
//! It is built once per galaxy, beside the stellar fields whose arm geometry and metallicity it
//! copies, and is held by the [`Galaxy`](crate::galaxy::Galaxy) handle.
//!
//! The density at a point is `n = n_disc × F + n_cor` (Design note 10): the three layers, the
//! neutral one with its lanes, times the log-normal factor, plus the corona as an additive,
//! un-noised floor, since hot gas fills the voids. Dust follows the disc term only, because grains
//! do not survive in the corona: the dust-bearing density is `n_disc × F × ζ` with `ζ =
//! 10^[M/H]` (Design note 13).

use crate::Seed;
use crate::coords::GalacticPosition;
use crate::galaxy::bounds::{BOUND_MARGIN, CellBox, ROUNDING_SLACK};
use crate::galaxy::fields::Fields;
use crate::galaxy::fields::metallicity::Metallicity;
use crate::galaxy::gas::MASS_PER_HYDROGEN_FACTOR;
use crate::galaxy::gas::lanes::Lanes;
use crate::galaxy::gas::modifiers::{self, GasModifier};
use crate::galaxy::gas::noise::{NoiseCache, SmoothingScale, log_normal_factor};
use crate::galaxy::gas::params::{BuildGasParamsError, GasParams};
use crate::galaxy::gas::phase::{GasPhase, ThermalState, warm_neutral_share};
use crate::galaxy::gas::pressure::Pressure;
use crate::galaxy::gas::smooth::{GasLayer, SmoothGas};
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::{PointLy, Population};
use crate::math;
use crate::units::consts::{BOLTZMANN_CONSTANT, HYDROGEN_MASS_KG, METRES_PER_LIGHT_YEAR};
use crate::units::{HydrogenPerCm3, Kelvin, KelvinPerCm3, MetresPerSecond, Years};

/// The adiabatic index of a monatomic gas, 5 ÷ 3, which a thermal sound speed is taken with.
const ADIABATIC_INDEX: f64 = 5.0 / 3.0;

/// The gas and dust field of one galaxy (plan 07).
///
/// A pure function of the seed, the galaxy's parameters and the generator version: immutable, with
/// no interior mutability and nothing on the heap, so it is `Send + Sync` and cheap to clone. Every
/// cache it could want is the caller's [`NoiseCache`].
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::gas::noise::{NoiseCache, SmoothingScale};
/// use hyperion_sim::galaxy::gas::phase::GasPhase;
/// use hyperion_sim::units::LightYears;
///
/// let galaxy = Galaxy::new(Seed::new(42));
/// let gas = galaxy.gas();
/// let here = GalacticPosition::from_light_years([0.0, 26_000.0, 20.0]).expect("in the cube");
/// // The mean field near the Sun: about one hydrogen atom per cubic centimetre.
/// let mean = gas.mean_density(&here).value();
/// assert!((0.2..3.0).contains(&mean));
/// // What a supernova shell here expands into, smoothed to its scale.
/// let mut cache = NoiseCache::with_capacity(256);
/// let site = gas.state(&here, SmoothingScale::AtLeast(LightYears::new(250.0)), &mut cache);
/// assert!(site.pressure() >= gas.params().pressure_floor());
/// // Far above the disc the gas is the hot corona.
/// let high = GalacticPosition::from_light_years([0.0, 26_000.0, 30_000.0]).expect("in the cube");
/// let corona = gas.state(&high, SmoothingScale::Full, &mut cache);
/// assert_eq!(corona.phase(), GasPhase::Hot);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct GasField {
    seed: Seed,
    params: GasParams,
    smooth: SmoothGas,
    lanes: Lanes,
    pressure: Pressure,
    /// The young thin disc's metallicity, read at the epoch: the gas's, since the youngest stars
    /// formed from it.
    metallicity: Metallicity,
}

impl GasField {
    /// The gas of the galaxy `galaxy_params`, whose stellar fields are `fields`, with `seed`
    /// keying this plan's own draws and the lattice noise.
    ///
    /// # Errors
    ///
    /// [`BuildGasParamsError`] from [`GasParams::from_galaxy`], for a galaxy built by hand whose
    /// warm ionised layer and molecular disc outweigh its gas (ruling 22 of 2026-09-22). No drawn
    /// galaxy comes near it.
    pub fn new(
        seed: Seed,
        galaxy_params: &GalaxyParams,
        fields: &Fields,
    ) -> Result<Self, BuildGasParamsError> {
        Ok(Self::with_params(
            seed,
            GasParams::from_galaxy(seed, galaxy_params)?,
            fields,
        ))
    }

    /// The gas of `params` on the stellar fields `fields`, with `seed` keying the lattice noise:
    /// for a fixture such as [`GasParams::milky_way_like`] on the Milky Way fixture's fields.
    ///
    /// `params` must belong to the galaxy `fields` were built for, as [`GasField::new`]'s do.
    ///
    /// # Panics
    ///
    /// Never for fields [`Fields::new`] built, which always hold the young thin disc.
    #[must_use]
    pub fn with_params(seed: Seed, params: GasParams, fields: &Fields) -> Self {
        let young = fields
            .components()
            .iter()
            .find(|c| c.population() == Population::YoungThinDisc)
            .expect("every galaxy's fields hold the young thin disc");
        Self {
            seed,
            params,
            smooth: SmoothGas::new(&params),
            lanes: Lanes::new(*fields.arms(), params.lane()),
            pressure: Pressure::new(&params),
            metallicity: young.metallicity_model(),
        }
    }

    /// The seed keying the lattice noise.
    #[must_use]
    pub fn seed(&self) -> Seed {
        self.seed
    }

    /// The gas parameters.
    #[must_use]
    pub fn params(&self) -> &GasParams {
        &self.params
    }

    /// The smooth layers, without lanes or noise.
    #[must_use]
    pub fn smooth(&self) -> &SmoothGas {
        &self.smooth
    }

    /// The lanes on the neutral layer.
    #[must_use]
    pub fn lanes(&self) -> &Lanes {
        &self.lanes
    }

    /// The pressure model.
    #[must_use]
    pub fn pressure_model(&self) -> &Pressure {
        &self.pressure
    }

    /// The mean hydrogen density at `p`: the three layers, lanes on, plus the corona, with no
    /// noise. It is the expectation of [`density`](Self::density) over seeds.
    #[must_use]
    pub fn mean_density(&self, p: &GalacticPosition) -> HydrogenPerCm3 {
        let layers = self.layers(&Site::of(p));
        HydrogenPerCm3::new(layers.disc() + self.smooth.corona_density())
    }

    /// The mean neutral gas at `p`: the neutral layer with its lanes and the molecular disc, no
    /// noise and no corona. It is what [`neutral_bound`](Self::neutral_bound) bounds over a cell.
    #[must_use]
    pub fn mean_neutral_density(&self, p: &GalacticPosition) -> HydrogenPerCm3 {
        let layers = self.layers(&Site::of(p));
        HydrogenPerCm3::new(layers.neutral + layers.molecular)
    }

    /// The hydrogen density at `p` with the lattice noise read at `scale`: `n_disc × F + n_cor`.
    /// `cache` changes what the call costs, never what it returns.
    #[must_use]
    pub fn density(
        &self,
        p: &GalacticPosition,
        scale: SmoothingScale,
        cache: &mut NoiseCache,
    ) -> HydrogenPerCm3 {
        let disc = self.layers(&Site::of(p)).disc();
        let factor = self.noise(p, scale, cache);
        HydrogenPerCm3::new(disc * factor + self.smooth.corona_density())
    }

    /// The hydrogen density a ship at `p` moves through, with the full noise and the features'
    /// modifiers `mods` applied: the quantity a sublight radiation and erosion load is proportional
    /// to (brainstorm, "What dust and gas do").
    ///
    /// Inside a hole it is the hole's interior density; each cloud adds its Plummer density
    /// (Design note 16). With no modifiers it is [`density`](Self::density) at
    /// [`SmoothingScale::Full`], bit for bit.
    #[must_use]
    pub fn density_with(
        &self,
        p: &GalacticPosition,
        mods: &[GasModifier],
        cache: &mut NoiseCache,
    ) -> HydrogenPerCm3 {
        let density = self.density(p, SmoothingScale::Full, cache).value();
        let (density, _) = modifiers::apply(mods, p, density, 0.0);
        HydrogenPerCm3::new(density)
    }

    /// The thermal pressure `P ÷ k` at `p`, K cm⁻³: never below the floor and never rising with
    /// the height (Design note 11).
    #[must_use]
    pub fn pressure(&self, p: &GalacticPosition) -> KelvinPerCm3 {
        self.pressure_at(&Site::of(p))
    }

    /// The phase of gas of density `n` at pressure `p_over_k` (Design note 12).
    #[must_use]
    pub fn phase(&self, n: HydrogenPerCm3, p_over_k: KelvinPerCm3) -> GasPhase {
        GasPhase::of(n, p_over_k)
    }

    /// The dust per hydrogen nucleus at `p`, in units of the solar ratio: `ζ = 10^[M/H]` with
    /// `[M/H]` the young thin disc's mean metallicity at the epoch at `p`'s cylindrical radius
    /// (Design note 13). It is 1 where the gas is solar and at most `10^0.5`, since plan 02 clamps
    /// the mean to +0.5 dex, and it is the same all along a vertical line.
    #[must_use]
    pub fn dust_per_hydrogen(&self, p: &GalacticPosition) -> f64 {
        self.zeta(Site::of(p).r)
    }

    /// The gas at `p` as a site reads it: density with the noise at `scale`, pressure, phase,
    /// temperature and sound speed. Plan 09's supernova shell reads it as its `SiteGas` at
    /// `SmoothingScale::AtLeast(250 ly)`, with no modifiers, since the shell may read only the
    /// fields and its own star's marks.
    #[must_use]
    pub fn state(
        &self,
        p: &GalacticPosition,
        scale: SmoothingScale,
        cache: &mut NoiseCache,
    ) -> GasState {
        let site = Site::of(p);
        let density = self.density(p, scale, cache);
        GasState {
            density,
            pressure: self.pressure_at(&site),
            thermal: self.thermal(&site, density),
        }
    }

    /// An upper bound on [`mean_neutral_density`](Self::mean_neutral_density) at every point of
    /// `cell`: the mean neutral gas, which plan 09 thins its clouds and star-forming regions
    /// against (P07.T6.b).
    ///
    /// It follows the brainstorm's rule under "Exact placement by thinning", term by term. The
    /// neutral layer's vertical factor `exp(−|z| ÷ h_n)` never rises with `|z|` and takes its
    /// value at the cell's nearest corner; its radial factor `exp(−R_m ÷ R − R ÷ R_g)` is unimodal
    /// in `R` with its peak at `√(R_m R_g)`, so over the cell's range of `R`
    /// ([`CellBox::r_cyl_range`]) it is bounded by its peak if the range holds it and by the
    /// nearer end's value otherwise, as plan 02's
    /// [`UnimodalFactor`](crate::galaxy::bounds::UnimodalFactor) states; and the lane factor is
    /// [`Lanes::sup`] over the cell. The molecular disc's density, which never rises with `R` or
    /// `|z|`, is added at the nearest corner. Each envelope carries plan 02's `1 + BOUND_MARGIN`
    /// and the radii its rounding slack, so the bound holds of the density as computed, not only
    /// as written.
    ///
    /// The bound is on the mean field: a consumer that thins against the noisy field multiplies by
    /// its own cap on the log-normal factor. Because every gas layer is exponential in height, the
    /// ratio of the gas density to an exponential proposal in `z` still peaks at the height nearest
    /// the plane, so plan 09's Design note 21 bound holds for its clouds as written.
    #[must_use]
    pub fn neutral_bound(&self, cell: &CellBox) -> HydrogenPerCm3 {
        let corner = cell.nearest_corner();
        let z = corner.z.abs();
        let radii = cell.r_cyl_range();
        let (r_lo, r_hi) = (
            radii.lo * (1.0 - ROUNDING_SLACK),
            radii.hi * (1.0 + ROUNDING_SLACK),
        );
        let peak = self.smooth.neutral_peak_radius();
        let r_neutral = if peak < r_lo {
            r_lo
        } else if peak > r_hi {
            r_hi
        } else {
            peak
        };
        let margin = 1.0 + BOUND_MARGIN;
        let neutral = self.smooth.density(GasLayer::Neutral, r_neutral, z) * margin;
        let lanes = self.lanes.sup(cell);
        let molecular = self.smooth.density(GasLayer::Molecular, r_lo, z) * margin;
        HydrogenPerCm3::new(neutral * lanes + molecular)
    }

    /// The bytes the field owns on the heap: none, since every part of it is inline.
    #[must_use]
    #[expect(
        clippy::unused_self,
        reason = "a method like every other part's, so that a part added later with a heap changes \
                  no caller"
    )]
    pub(crate) fn heap_bytes(&self) -> usize {
        0
    }

    /// The three layers at `site`, lanes on, cm⁻³.
    #[must_use]
    pub(crate) fn layers(&self, site: &Site) -> Layers {
        Layers {
            neutral: self.smooth.density(GasLayer::Neutral, site.r, site.z)
                * self.lanes.factor(site.r, site.theta),
            warm: self.smooth.density(GasLayer::Warm, site.r, site.z),
            molecular: self.smooth.density(GasLayer::Molecular, site.r, site.z),
        }
    }

    /// The thermal pressure `P ÷ k` at `site`, K cm⁻³.
    #[must_use]
    pub(crate) fn pressure_at(&self, site: &Site) -> KelvinPerCm3 {
        KelvinPerCm3::new(self.pressure.at(&self.smooth, site.r, site.z))
    }

    /// The thermal state of gas of density `n` at `site`: at the pressure there, with the smooth
    /// layers' neutral share there.
    #[must_use]
    pub(crate) fn thermal(&self, site: &Site, n: HydrogenPerCm3) -> ThermalState {
        let layers = self.layers(site);
        ThermalState::of(
            n,
            self.pressure_at(site),
            warm_neutral_share(layers.neutral, layers.warm),
        )
    }

    /// The log-normal factor at `p` at `scale`, with this galaxy's seed and `σ_ln`.
    #[must_use]
    pub(crate) fn noise(
        &self,
        p: &GalacticPosition,
        scale: SmoothingScale,
        cache: &mut NoiseCache,
    ) -> f64 {
        log_normal_factor(self.seed, p, self.params.sigma_ln(), scale, cache)
    }

    /// `ζ = 10^[M/H]` at cylindrical radius `r` (ly).
    #[must_use]
    pub(crate) fn zeta(&self, r: f64) -> f64 {
        math::exp10(self.metallicity.at(r, Years::ZERO).mean().value())
    }
}

/// Where a position lies, in the field's own coordinates: its cylindrical radius and height in
/// light-years and its azimuth in radians, from the position's float metres once.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Site {
    pub(crate) r: f64,
    pub(crate) theta: f64,
    pub(crate) z: f64,
}

impl Site {
    /// The site of `p`.
    #[must_use]
    pub(crate) fn of(p: &GalacticPosition) -> Self {
        let cylindrical = p.to_cylindrical();
        Self {
            r: cylindrical.radius().value() / METRES_PER_LIGHT_YEAR,
            theta: cylindrical.azimuth().value(),
            z: cylindrical.height().value() / METRES_PER_LIGHT_YEAR,
        }
    }

    /// The site of the point `p` in float light-years, for the maps, which work in them as plan
    /// 02's do.
    #[must_use]
    pub(crate) fn of_ly(p: &PointLy) -> Self {
        Self {
            r: (p.x * p.x + p.y * p.y).sqrt(),
            theta: math::atan2(p.y, p.x),
            z: p.z,
        }
    }
}

/// The three layers' densities at one site, the neutral one with its lanes, cm⁻³.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Layers {
    pub(crate) neutral: f64,
    pub(crate) warm: f64,
    pub(crate) molecular: f64,
}

impl Layers {
    /// `n_disc`: the three together, in [`GasLayer::ALL`]'s order.
    #[must_use]
    pub(crate) fn disc(&self) -> f64 {
        self.neutral + self.warm + self.molecular
    }
}

/// The gas at one site: its density, pressure, phase and ionisation, and the temperature and sound
/// speed they give (plan 09's `SiteGas`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GasState {
    density: HydrogenPerCm3,
    pressure: KelvinPerCm3,
    thermal: ThermalState,
}

impl GasState {
    /// The hydrogen density.
    #[must_use]
    pub fn density(&self) -> HydrogenPerCm3 {
        self.density
    }

    /// The thermal pressure `P ÷ k`.
    #[must_use]
    pub fn pressure(&self) -> KelvinPerCm3 {
        self.pressure
    }

    /// The phase.
    #[must_use]
    pub fn phase(&self) -> GasPhase {
        self.thermal.phase()
    }

    /// The share of the hydrogen that is neutral ([`ThermalState::neutral_share`]).
    #[must_use]
    pub fn neutral_share(&self) -> f64 {
        self.thermal.neutral_share()
    }

    /// The particles per hydrogen nucleus `x` the temperature is taken with: 2.3 when hot, 1.1
    /// when cold, and `1.1 + 1.2 (1 − f)` when warm with `f` the neutral share (ruling 91 of
    /// 2026-09-22).
    #[must_use]
    pub fn particles_per_hydrogen(&self) -> f64 {
        self.thermal.particles_per_hydrogen()
    }

    /// The temperature (Design note 12): the equilibrium `(P ÷ k) ÷ (x n)`, held to 5,000–10,000 K
    /// when warm (ruling 98 of 2026-09-22; [`ThermalState::temperature`]); infinite where the
    /// density is 0.
    #[must_use]
    pub fn temperature(&self) -> Kelvin {
        self.thermal.temperature(self.density, self.pressure)
    }

    /// Whether the gas is warm and its temperature clamped to 5,000 or 10,000 K
    /// ([`ThermalState::clamped`]), so that `T × x n` is not `P`.
    #[must_use]
    pub fn temperature_clamped(&self) -> bool {
        self.thermal.clamped()
    }

    /// `P ÷ ρ` in m² s⁻²: `(P ÷ k) k × 10⁶` over `1.4 m_H n × 10⁶`, the two cm⁻³-to-m⁻³ factors
    /// cancelling.
    fn pressure_per_mass(&self) -> f64 {
        self.pressure.value() * BOLTZMANN_CONSTANT
            / (MASS_PER_HYDROGEN_FACTOR * HYDROGEN_MASS_KG * self.density.value())
    }

    /// The adiabatic sound speed `c = √(γ P ÷ ρ)`, with `γ = 5 ÷ 3` and `ρ = 1.4 m_H n`: how fast
    /// a small disturbance crosses the gas. Infinite where the density is 0.
    #[must_use]
    pub fn thermal_sound_speed(&self) -> MetresPerSecond {
        MetresPerSecond::new((ADIABATIC_INDEX * self.pressure_per_mass()).sqrt())
    }

    /// The isothermal sound speed `C₀ = √(P ÷ ρ)`, `ρ = 1.4 m_H n`: what a supernova shell merges
    /// with the ambient gas against, when its shock has slowed to a few times it (Cioffi, McKee and
    /// Bertschinger 1988, ApJ 334, 252, p. 264: "the ambient isothermal sound speed"; ruling 98 of
    /// 2026-09-22). Infinite where the density is 0.
    #[must_use]
    pub fn isothermal_sound_speed(&self) -> MetresPerSecond {
        MetresPerSecond::new(self.pressure_per_mass().sqrt())
    }
}
