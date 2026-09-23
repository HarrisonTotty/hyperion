//! The protoplanetary disc of one orbit host: the budget and the ruler of what its planets can be
//! (plan 14, P14.T3; design notes 5 and 6).
//!
//! The disc never decides a system's architecture class, which comes from observed frequencies
//! (design note 5). It scales what the class places: the characteristic planet mass through the
//! isolation mass, the reach of the system through its edges, the mass of belts and halo, and
//! whether the class's giants can exist at all through the snow line and the solid budget.
//!
//! # The model
//!
//! - **Gas mass** `M_d` = f × M★, with log₁₀ f normal about −2.0 with σ = 0.5 dex, capped at −1.0
//!   ([`GAS_FRACTION_MEDIAN_DEX`], [`GAS_FRACTION_SIGMA_DEX`], [`GAS_FRACTION_CAP_DEX`]).
//! - **Surface density** `Σ_gas` ∝ r⁻¹ exp(−r ÷ `r_c`), the self-similar profile with γ = 1, which
//!   Andrews et al. (2010, ApJ 723, 1241, eq. 1 and Table 4) fit to the Ophiuchus discs as
//!   γ = 0.9 ± 0.2. As there, it is normalised to `M_d` from the inner edge outwards, to infinity,
//!   where the integral is `r_c` exp(−r ÷ `r_c`) in closed form (ruling 38).
//! - **Solids** are a share of the gas set by its composition: rock inside the snow line and rock
//!   and water ice beyond it, each scaled by 10^\[Fe/H\] ([`ROCK_MASS_FRACTION`],
//!   [`WATER_ICE_MASS_FRACTION`], [`ICE_ENHANCEMENT`]), so the solid mass scales as 10^\[Fe/H\]
//!   exactly.
//! - **Snow line** at 2.7 au × √(L ÷ L☉) of the host's zero-age main-sequence luminosity
//!   ([`snow_line`]; design note 6).
//! - **Inner edge** at the largest of 2.5 zero-age stellar radii, the host's fluid Roche limit for
//!   a body of 1,000 kg m⁻³, and the corotation radius at a drawn stellar rotation period, where
//!   the star's magnetosphere truncates the disc.
//! - **Characteristic radius** `r_c` = 30 au × (M★ ÷ M☉)^½, with 0.3 dex of scatter, and the
//!   **outer edge** at 3 `r_c`, inside which 95% of the profile's mass lies
//!   ([`OUTER_EDGE_CHARACTERISTIC_RADII`]; ruling 38). The 5% beyond it is not in the disc.
//! - **Truncation** to the radii a caller passes (the stable zone of a multiple system, P14.T9;
//!   the strip radius of design note 14, P14.T29), renormalising nothing: a truncated disc has lost
//!   that mass. A disc truncated to nothing is [`Disc::None`].
//! - **Lifetime**, an argument (ruling 33): a circumstellar disc's is plan 06's
//!   [`disc_lifetime`](crate::stellar::premain::disc_lifetime) of its star's own rank, and a
//!   circumbinary disc's the same law at the pair's total mass, of the rank this module draws
//!   ([`DiscDraws::circumbinary_lifetime`]).
//!
//! # Draws
//!
//! All on [`tags::PLANET_DISC`], keyed by the system's ID, with the orbit host's number in the draw
//! number (design note 4): host h reads words 16h to 16h + 15 ([`DISC_WORDS_PER_HOST`]). Words 0–1
//! are the gas fraction's standard normal, 2–3 the rotation period's, 4–5 the characteristic
//! radius's, and word 6 a circumbinary disc's lifetime rank; 7–15 are reserved.
//!
//! # Two readings of plan 14, both ruled
//!
//! Plan 14's text was written from memory, and two of its sentences were read against their
//! sources as follows. Ruling 38 settled both.
//!
//! - *The solid share.* The plan writes Mₛ = `M_d` × Z☉ × 10^\[Fe/H\] with Z☉ = 0.0149, "times an
//!   ice enhancement beyond the snow line (factor 2)", both from Lodders (2003). Z☉ = 0.0149 is her
//!   protosolar heavy-element fraction Z₀, all of which is not solid: her Table 11 has 0.489% of a
//!   solar-composition gas condensing as rock and 0.571% as water ice, the rest of the 1.49% being
//!   methane and ammonia ices that condense only far out, and noble gases. Read literally the
//!   sentence would make the solids beyond the snow line 3% of the gas, twice every heavy element
//!   there is. So the solid share is taken from Table 11 itself: 0.489% × 10^\[Fe/H\] inside the
//!   snow line and 1.060% × 10^\[Fe/H\] beyond it, a step of 2.17, the plan's "factor 2". Ruled as
//!   built.
//! - *The outer edge.* The plan's "outer radius `r_c`" is the profile's characteristic radius, not
//!   its edge: Andrews et al. (2010) normalise `M_d` to infinity, so e⁻¹ of the mass (37%) lies
//!   beyond `r_c`, and a Kuiper-like belt (P14.T21.b) needs solids there. The edge is at 3 `r_c`.

use core::f64::consts::{PI, TAU};
use std::error::Error;
use std::fmt;

use super::derive::limits::roche_limit_fluid;
use crate::id::SystemId;
use crate::math;
use crate::rng::{ObjectKey, Seed, Stream, tags};
use crate::stellar::draws::{StandardNormal, UnitUniform};
use crate::stellar::premain;
use crate::units::consts::{EARTH_MASS_KG, GM_SUN, METRES_PER_AU, SOLAR_MASS_KG, SOLAR_RADIUS_M};
use crate::units::{
    Dex, EarthMasses, Kilograms, KilogramsPerCubicMetre, KilogramsPerSquareMetre, Megayears,
    Metres, Seconds, SolarLuminosities, SolarMasses, SolarRadii,
};

/// Words of the [`tags::PLANET_DISC`] stream that one orbit host owns: host h reads words 16h
/// onwards. Changing it moves every host after the first, which is a generator-version change.
pub const DISC_WORDS_PER_HOST: u64 = 16;

/// The median of log₁₀(`M_d` ÷ M★), the disc's gas mass over its host's: −2.0, a disc of 1% of
/// its star.
///
/// Plan 14's figure. Andrews et al. (2013, ApJ 771, 129, §3.2.2 and abstract) find `M_d` ∝ M★
/// with a typical ratio of 0.2–0.6% (log −2.7 to −2.2) and ±0.7 dex of scatter in the Class II
/// discs of Taurus, from millimetre dust masses and a gas-to-dust ratio of 100. The plan's median
/// sits 0.3–0.7 dex above those, which counts the solids already grown past millimetre sizes and the
/// younger, heavier discs planets form in; its σ of 0.5 is narrower than their scatter, much of
/// which is evolution.
pub const GAS_FRACTION_MEDIAN_DEX: f64 = -2.0;

/// The scatter of log₁₀(`M_d` ÷ M★): 0.5 dex (plan 14; see [`GAS_FRACTION_MEDIAN_DEX`]).
pub const GAS_FRACTION_SIGMA_DEX: f64 = 0.5;

/// The cap on log₁₀(`M_d` ÷ M★): −1.0, a disc of a tenth of its star.
///
/// Discs become gravitationally unstable once `M_d` ÷ M★ ≳ 0.1 (Kratter and Lodato 2016, ARAA 54,
/// 271, abstract, and §2, eq. 3) and fragment or redistribute their mass rather than grow
/// heavier.
pub const GAS_FRACTION_CAP_DEX: f64 = -1.0;

/// The mass fraction of a solar-composition gas that condenses as rock (silicates, oxides, metal
/// and troilite): 0.489%.
///
/// Lodders (2003, ApJ 591, 1220), Table 11, "Total rock" for the solar-system (protosolar)
/// composition, whose heavy-element fraction is Z₀ = 0.0149 (her abstract and §2.4). This is the
/// share of the gas that is solid inside the snow line, at \[Fe/H\] = 0.
pub const ROCK_MASS_FRACTION: f64 = 0.004_89;

/// The mass fraction of a solar-composition gas that condenses as water ice: 0.571%.
///
/// Lodders (2003), Table 11, "H₂O ice" for the solar-system composition (water ice to rock 1.17).
/// Ammonia hydrate (0.097%) condenses at 131 K, about 4.6 au × √(L ÷ L☉), and methane ice
/// (0.330%) at 41 K, about 47 au × √(L ÷ L☉), beyond most discs; both are left out, the ammonia a
/// step of 9% beyond its own line (Lodders 2003, Tables 10 and 11).
pub const WATER_ICE_MASS_FRACTION: f64 = 0.005_71;

/// How many times more solid a solar-composition gas is beyond the snow line than inside it:
/// (rock + water ice) ÷ rock = 2.17, the plan's "factor 2" (Lodders 2003, Table 11).
pub const ICE_ENHANCEMENT: f64 =
    (ROCK_MASS_FRACTION + WATER_ICE_MASS_FRACTION) / ROCK_MASS_FRACTION;

/// The snow line of a host of one solar luminosity: 2.7 au.
///
/// Hayashi (1981, Prog. Theor. Phys. Suppl. 70, 35): an optically thin disc's temperature
/// T = 280 K (r ÷ 1 au)^−½ (L ÷ L☉)^¼ falls to water ice's 170 K at (280 ÷ 170)² = 2.7 au. Kennedy
/// and Kenyon (2008, ApJ 673, 502, §3 and Fig. 1) call this the canonical distance and locate the
/// snow line at the same 170 K; published normalisations differ by about 15%.
pub const SNOW_LINE_AT_SOLAR_LUMINOSITY_AU: f64 = 2.7;

/// The inner edge's floor in zero-age stellar radii: 2.5 (plan 14).
pub const INNER_EDGE_STELLAR_RADII: f64 = 2.5;

/// The bulk density for which the inner edge's fluid Roche limit is taken: 1,000 kg m⁻³, the
/// least dense body that could form there (plan 14).
pub const INNER_EDGE_BODY_DENSITY: KilogramsPerCubicMetre = KilogramsPerCubicMetre::new(1_000.0);

/// The median rotation period of a young star, whose corotation radius is where its magnetosphere
/// truncates the disc: 8 days.
///
/// Plan 14's figure. Lee and Chiang (2017, ApJ 842, 40, Fig. 2) show the rotation periods of
/// pre-main-sequence stars of the Orion Nebula Cluster, NGC 2362 and NGC 2547 peaking near 10 days
/// with a tail to shorter periods, and derive from them the break in the occurrence of Kepler's
/// sub-Neptunes at about 10 days; Mulders et al. (2018, AJ 156, 24, Table 2) find the innermost
/// planets of Kepler's systems clustered at 12 (+3, −2) days.
pub const COROTATION_PERIOD_MEDIAN_DAYS: f64 = 8.0;

/// The scatter of the rotation period: 0.25 dex (plan 14).
///
/// Mulders et al.'s (2018) innermost-planet distribution, rising as P^1.6 and falling as P^−0.9 on
/// either side of its peak, is 0.52 dex wide at half maximum, a σ of 0.22 dex.
pub const COROTATION_PERIOD_SIGMA_DEX: f64 = 0.25;

/// The characteristic radius of a solar-mass host's disc: 30 au.
///
/// Plan 14's figure. Andrews et al. (2010, Tables 4 and 5) fit characteristic radii of 14–198 au
/// to sixteen Ophiuchus discs around 0.3–2 M☉ stars, a median of 39 au and 0.36 dex of scatter, in
/// a sample weighted to bright discs; fainter discs are smaller (their §4.1, and Andrews et al.
/// 2018, ApJ 865, 157).
pub const CHARACTERISTIC_RADIUS_AT_SOLAR_MASS_AU: f64 = 30.0;

/// The scatter of the characteristic radius: 0.3 dex.
///
/// Andrews et al. (2018, Table 1): the continuum sizes of 105 discs scale as M★^0.58±0.10 with a
/// dispersion of 0.30 dex about that relation. The exponent of plan 14's `r_c` ∝ M★^½ lies within
/// their error.
pub const CHARACTERISTIC_RADIUS_SIGMA_DEX: f64 = 0.3;

/// The half-width of a body's feeding zone, in Hill radii: 2√3.
///
/// Lissauer (1987, Icarus 69, 249; 1993, ARAA 31, 129): a body sweeps up the planetesimals within
/// 2√3 Hill radii of its orbit on either side, a zone 4√3 ≈ 6.9 Hill radii wide. With it the
/// isolation mass is (8 ÷ √3) π^(3/2) C^(3/2) M★^−½ Σ^(3/2) a³, which gives 0.07 M⊕ at 1 au and
/// 9 M⊕ at 5 au for Σ = 10 g cm⁻² about a solar mass (Armitage 2007, arXiv:astro-ph/0701485v6,
/// eqs. 198–203). Plan 14 wrote the zone as "10 Hill radii"; this is the source's width, which
/// ruling 38 kept.
pub const FEEDING_ZONE_HALF_WIDTH_HILL: f64 = 3.464_101_615_137_754_6;

/// The untruncated outer edge, in characteristic radii: 3 (ruling 38).
///
/// The profile's mass beyond a radius r is a fraction exp(−(r − `r_in`) ÷ `r_c`) of the whole, so
/// 3 `r_c` encloses 1 − e⁻³ = 95.0% of it for an inner edge far inside `r_c`, and a little more
/// otherwise.
pub const OUTER_EDGE_CHARACTERISTIC_RADII: f64 = 3.0;

/// Seconds in a day of 86,400 s.
const SECONDS_PER_DAY: f64 = 86_400.0;

/// The snow line of a host of luminosity `l`: 2.7 au × √(L ÷ L☉) (Hayashi 1981), in metres.
///
/// Plan 14 (design note 6) passes the host's zero-age main-sequence luminosity, because the snow
/// line decides where things formed and draws may not depend on time.
///
/// # Panics
///
/// In debug builds, if `l` is negative or not finite.
///
/// # Examples
///
/// ```
/// use hyperion_sim::planetary::disc::snow_line;
/// use hyperion_sim::units::{AstronomicalUnits, SolarLuminosities};
///
/// let au = |l: f64| AstronomicalUnits::from(snow_line(SolarLuminosities::new(l))).value();
/// assert!((au(1.0) - 2.7).abs() < 1e-12);
/// // The zero-age Sun, 0.7 L☉, had its snow line at 2.26 au.
/// assert!((au(0.7) - 2.259).abs() < 1e-3);
/// ```
#[must_use]
pub fn snow_line(l: SolarLuminosities) -> Metres {
    debug_assert!(
        l.value().is_finite() && l.value() >= 0.0,
        "a luminosity is finite and not negative, got {}",
        l.value()
    );
    Metres::new(SNOW_LINE_AT_SOLAR_LUMINOSITY_AU * METRES_PER_AU * l.value().sqrt())
}

/// The isolation mass of a body at `a` in a disc of solid surface density `surface_density` about
/// a host of mass `host_mass`: the mass of the planetesimals in its feeding zone, once the zone has
/// grown with the body's Hill radius to hold no more (Lissauer 1987).
///
/// With a feeding zone of C = [`FEEDING_ZONE_HALF_WIDTH_HILL`] Hill radii either side, M = 2πa ×
/// 2C a (M ÷ 3M★)^⅓ × Σ, whose solution is M = (4πC a² Σ)^(3/2) ÷ √(3M★). Σ is the local density;
/// the closed form assumes it constant across the zone.
///
/// # Panics
///
/// In debug builds, if `host_mass` is not positive, or `a` or `surface_density` is negative.
#[must_use]
pub fn isolation_mass(
    surface_density: KilogramsPerSquareMetre,
    a: Metres,
    host_mass: SolarMasses,
) -> EarthMasses {
    debug_assert!(host_mass.value() > 0.0, "a host's mass is positive");
    debug_assert!(a.value() >= 0.0 && surface_density.value() >= 0.0);
    let a = a.value();
    let zone = 4.0 * PI * FEEDING_ZONE_HALF_WIDTH_HILL * a * a * surface_density.value();
    let host = 3.0 * host_mass.value() * SOLAR_MASS_KG;
    EarthMasses::new(zone * zone.sqrt() / host.sqrt() / EARTH_MASS_KG)
}

/// What the disc reads of its host, all fixed at the zero-age main sequence (design note 6): its
/// mass, \[Fe/H\], luminosity and radius.
///
/// A star's are plan 06's [`zams::luminosity`](crate::stellar::sse::zams::luminosity) and
/// [`zams::radius`](crate::stellar::sse::zams::radius) of its initial mass and composition; a
/// substellar host's luminosity will be its cooling fit's at 10 Myr (P14.T27).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiscHost {
    mass: SolarMasses,
    fe_h: Dex,
    zams_luminosity: SolarLuminosities,
    zams_radius: SolarRadii,
}

impl DiscHost {
    /// A host of mass `mass`, iron abundance `fe_h`, zero-age luminosity `zams_luminosity` and
    /// zero-age radius `zams_radius`.
    ///
    /// # Errors
    ///
    /// - [`BuildDiscHostError::MassNotPositive`] unless the mass is positive and finite.
    /// - [`BuildDiscHostError::MetallicityNotFinite`] if \[Fe/H\] is not finite.
    /// - [`BuildDiscHostError::LuminosityNotPositive`] unless the luminosity is positive and
    ///   finite.
    /// - [`BuildDiscHostError::RadiusNotPositive`] unless the radius is positive and finite.
    pub fn new(
        mass: SolarMasses,
        fe_h: Dex,
        zams_luminosity: SolarLuminosities,
        zams_radius: SolarRadii,
    ) -> Result<Self, BuildDiscHostError> {
        let positive = |x: f64| x.is_finite() && x > 0.0;
        if !positive(mass.value()) {
            return Err(BuildDiscHostError::MassNotPositive);
        }
        if !fe_h.value().is_finite() {
            return Err(BuildDiscHostError::MetallicityNotFinite);
        }
        if !positive(zams_luminosity.value()) {
            return Err(BuildDiscHostError::LuminosityNotPositive);
        }
        if !positive(zams_radius.value()) {
            return Err(BuildDiscHostError::RadiusNotPositive);
        }
        Ok(Self {
            mass,
            fe_h,
            zams_luminosity,
            zams_radius,
        })
    }

    /// The host's mass: a star's initial mass, or a pair's total for a circumbinary disc.
    #[must_use]
    pub const fn mass(&self) -> SolarMasses {
        self.mass
    }

    /// The host's \[Fe/H\], dex relative to the Sun, as drawn.
    #[must_use]
    pub const fn fe_h(&self) -> Dex {
        self.fe_h
    }

    /// The host's zero-age main-sequence luminosity.
    #[must_use]
    pub const fn zams_luminosity(&self) -> SolarLuminosities {
        self.zams_luminosity
    }

    /// The host's zero-age main-sequence radius.
    #[must_use]
    pub const fn zams_radius(&self) -> SolarRadii {
        self.zams_radius
    }
}

/// A [`DiscHost`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildDiscHostError {
    /// The mass was not positive and finite.
    MassNotPositive,
    /// \[Fe/H\] was not finite.
    MetallicityNotFinite,
    /// The zero-age luminosity was not positive and finite.
    LuminosityNotPositive,
    /// The zero-age radius was not positive and finite.
    RadiusNotPositive,
}

impl fmt::Display for BuildDiscHostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::MassNotPositive => "a disc host's mass must be positive and finite",
            Self::MetallicityNotFinite => "a disc host's [Fe/H] must be finite",
            Self::LuminosityNotPositive => "a disc host's luminosity must be positive and finite",
            Self::RadiusNotPositive => "a disc host's radius must be positive and finite",
        })
    }
}

impl Error for BuildDiscHostError {}

/// The radii a disc is cut to: a stable zone's limits (P14.T9) or the strip radius of design
/// note 14 (P14.T29). Nothing, until those tasks land.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Truncation {
    inner: Option<Metres>,
    outer: Option<Metres>,
}

impl Truncation {
    /// No truncation: the disc keeps its own edges.
    pub const NONE: Self = Self {
        inner: None,
        outer: None,
    };

    /// This truncation with nothing of the disc kept inside `inner`.
    ///
    /// # Panics
    ///
    /// In debug builds, if `inner` is negative or not finite.
    #[must_use]
    pub fn with_inner(self, inner: Metres) -> Self {
        debug_assert!(inner.value().is_finite() && inner.value() >= 0.0);
        Self {
            inner: Some(inner),
            ..self
        }
    }

    /// This truncation with nothing of the disc kept outside `outer`.
    ///
    /// # Panics
    ///
    /// In debug builds, if `outer` is negative or not finite.
    #[must_use]
    pub fn with_outer(self, outer: Metres) -> Self {
        debug_assert!(outer.value().is_finite() && outer.value() >= 0.0);
        Self {
            outer: Some(outer),
            ..self
        }
    }

    /// The inner truncation radius, if any.
    #[must_use]
    pub const fn inner(&self) -> Option<Metres> {
        self.inner
    }

    /// The outer truncation radius, if any.
    #[must_use]
    pub const fn outer(&self) -> Option<Metres> {
        self.outer
    }
}

/// The random variates of one orbit host's disc, as drawn from [`tags::PLANET_DISC`] by
/// [`DiscDraws::for_host`], or given explicitly by a test or a tool.
///
/// The fields are plain variates with no invariant between them, so they are public, as plan 06's
/// `StarDrawsParts` are.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiscDraws {
    /// The gas-to-host mass ratio's standard normal. Words 0–1 of the host's block.
    pub gas_fraction: StandardNormal,
    /// The host's rotation period's standard normal. Words 2–3.
    pub corotation_period: StandardNormal,
    /// The characteristic radius's standard normal. Words 4–5.
    pub characteristic_radius: StandardNormal,
    /// A circumbinary disc's lifetime rank (ruling 33). Word 6. A circumstellar disc reads its
    /// star's `star.disc_lifetime` rank instead, and leaves this unused.
    pub circumbinary_lifetime_rank: UnitUniform,
}

impl DiscDraws {
    /// Every variate at its median: the disc of the median host.
    pub const MEDIAN: Self = Self {
        gas_fraction: StandardNormal::ZERO,
        corotation_period: StandardNormal::ZERO,
        characteristic_radius: StandardNormal::ZERO,
        circumbinary_lifetime_rank: UnitUniform::HALF,
    };

    /// The draws of orbit host number `host` of `system`, in the universe of `seed`: words
    /// 16 × host onwards of `system`'s [`tags::PLANET_DISC`] stream.
    ///
    /// A single star's disc is host 0. The numbering of the hosts of a multiple system is P14.T9's.
    #[must_use]
    pub fn for_host(seed: Seed, system: SystemId, host: u8) -> Self {
        let mut stream = Stream::open(seed, tags::PLANET_DISC, ObjectKey::from(system));
        stream.seek(u64::from(host) * DISC_WORDS_PER_HOST);
        let gas_fraction = draw_normal(&mut stream);
        let corotation_period = draw_normal(&mut stream);
        let characteristic_radius = draw_normal(&mut stream);
        let circumbinary_lifetime_rank = draw_rank(&mut stream);
        Self {
            gas_fraction,
            corotation_period,
            characteristic_radius,
            circumbinary_lifetime_rank,
        }
    }

    /// A circumbinary disc's lifetime: plan 06's law at the pair's total mass `pair_mass`, of this
    /// host's [`circumbinary_lifetime_rank`](Self::circumbinary_lifetime_rank) (ruling 33).
    ///
    /// # Panics
    ///
    /// In debug builds, if `pair_mass` is not positive and finite.
    #[must_use]
    pub fn circumbinary_lifetime(&self, pair_mass: SolarMasses) -> Megayears {
        premain::disc_lifetime(pair_mass, self.circumbinary_lifetime_rank)
    }
}

/// The next two words of `stream` as a standard normal.
#[must_use]
fn draw_normal(stream: &mut Stream) -> StandardNormal {
    StandardNormal::new(stream.standard_normal()).expect("a Box–Muller variate is finite")
}

/// The next word of `stream` as a rank.
#[must_use]
fn draw_rank(stream: &mut Stream) -> UnitUniform {
    UnitUniform::new(stream.uniform_open()).expect("an open uniform lies strictly between 0 and 1")
}

/// A host's protoplanetary disc, or its absence.
///
/// [`derive()`] returns [`Disc::None`] when no annulus lies between the inner and outer edges:
/// either the truncation radii leave none, or the untruncated inner edge is already at or beyond
/// the outer edge of 3 `r_c` (a drawn corotation radius far out and a drawn `r_c` far in).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Disc {
    /// No disc: nothing lies between its edges, and the host forms no planets.
    None,
    /// A disc with a positive annulus between its edges.
    Present(DiscProfile),
}

impl Disc {
    /// The disc's profile, unless there is no disc.
    #[must_use]
    pub const fn profile(&self) -> Option<&DiscProfile> {
        match self {
            Self::None => None,
            Self::Present(profile) => Some(profile),
        }
    }

    /// The gas mass between the edges; zero without a disc.
    #[must_use]
    pub fn gas_mass(&self) -> SolarMasses {
        self.profile()
            .map_or(SolarMasses::ZERO, DiscProfile::gas_mass)
    }

    /// The solid mass between the edges, the budget of design note 5; zero without a disc.
    #[must_use]
    pub fn solid_mass(&self) -> EarthMasses {
        self.profile()
            .map_or(EarthMasses::ZERO, DiscProfile::solid_mass)
    }
}

/// A protoplanetary disc between its edges: its masses, lifetime, snow line and surface density.
///
/// The gas's surface density is Σ(r) = K e^(−r ÷ `r_c`) ÷ r between the edges, with K set so that
/// the profile from the untruncated inner edge to infinity holds `M_d`; the solids' is a share of
/// it, rock inside the snow line and rock and water ice beyond, scaled by 10^\[Fe/H\]. Every mass
/// is a closed-form integral of it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiscProfile {
    host_mass: SolarMasses,
    lifetime: Megayears,
    snow_line: Metres,
    inner_edge: Metres,
    outer_edge: Metres,
    characteristic_radius: Metres,
    corotation_period: Seconds,
    /// K of `Σ_gas`(r) = K e^(−r ÷ `r_c`) ÷ r, in kg m⁻¹.
    gas_scale: f64,
    /// 10^\[Fe/H\], which multiplies both solid shares.
    metal_scale: f64,
    drawn_gas_mass: Kilograms,
    gas_mass: Kilograms,
    solid_mass: Kilograms,
}

impl DiscProfile {
    /// The host's mass.
    #[must_use]
    pub const fn host_mass(&self) -> SolarMasses {
        self.host_mass
    }

    /// How long the gas disc lives: the lifetime [`derive()`] was given.
    #[must_use]
    pub const fn lifetime(&self) -> Megayears {
        self.lifetime
    }

    /// The snow line, from the host's zero-age luminosity ([`snow_line`]).
    #[must_use]
    pub const fn snow_line(&self) -> Metres {
        self.snow_line
    }

    /// The inner edge, after truncation.
    #[must_use]
    pub const fn inner_edge(&self) -> Metres {
        self.inner_edge
    }

    /// The outer edge, after truncation.
    #[must_use]
    pub const fn outer_edge(&self) -> Metres {
        self.outer_edge
    }

    /// The characteristic radius `r_c` of the taper; the untruncated outer edge is 3 `r_c`.
    #[must_use]
    pub const fn characteristic_radius(&self) -> Metres {
        self.characteristic_radius
    }

    /// The host's drawn rotation period, whose corotation radius is one candidate for the inner
    /// edge.
    #[must_use]
    pub const fn corotation_period(&self) -> Seconds {
        self.corotation_period
    }

    /// The gas mass as drawn, `M_d` = f × M★: the whole profile's, from the untruncated inner edge
    /// to infinity, of which about 5% lies beyond the outer edge.
    #[must_use]
    pub fn drawn_gas_mass(&self) -> SolarMasses {
        SolarMasses::from(self.drawn_gas_mass)
    }

    /// The gas mass between the edges: `M_d`, less the part beyond 3 `r_c` and what a truncation
    /// cut away.
    #[must_use]
    pub fn gas_mass(&self) -> SolarMasses {
        SolarMasses::from(self.gas_mass)
    }

    /// The solid mass between the edges, which is [`solid_mass_between`](Self::solid_mass_between)
    /// of the edges.
    #[must_use]
    pub fn solid_mass(&self) -> EarthMasses {
        EarthMasses::from(self.solid_mass)
    }

    /// The solid mass between radii `a` and `b`, of the part of that annulus inside the disc; zero
    /// if `b` is not beyond `a`.
    ///
    /// A closed form of the surface density, additive over adjoining intervals.
    #[must_use]
    pub fn solid_mass_between(&self, a: Metres, b: Metres) -> EarthMasses {
        EarthMasses::from(Kilograms::new(
            self.solid_kilograms_between(a.value(), b.value()),
        ))
    }

    /// The surface density of solids at radius `r`: zero outside the edges.
    #[must_use]
    pub fn surface_density(&self, r: Metres) -> KilogramsPerSquareMetre {
        let r = r.value();
        if r < self.inner_edge.value() || r > self.outer_edge.value() {
            return KilogramsPerSquareMetre::ZERO;
        }
        KilogramsPerSquareMetre::new(self.gas_density(r) * self.solid_share(r))
    }

    /// The surface density of gas at radius `r`: zero outside the edges.
    #[must_use]
    pub fn gas_surface_density(&self, r: Metres) -> KilogramsPerSquareMetre {
        let r = r.value();
        if r < self.inner_edge.value() || r > self.outer_edge.value() {
            return KilogramsPerSquareMetre::ZERO;
        }
        KilogramsPerSquareMetre::new(self.gas_density(r))
    }

    /// The isolation mass of a body at `a` ([`isolation_mass`] of the local surface density of
    /// solids): zero outside the edges.
    ///
    /// # Panics
    ///
    /// In debug builds, if `a` is negative.
    #[must_use]
    pub fn isolation_mass(&self, a: Metres) -> EarthMasses {
        isolation_mass(self.surface_density(a), a, self.host_mass)
    }

    /// `Σ_gas`(r) = K e^(−r ÷ `r_c`) ÷ r, in kg m⁻², without the edges.
    #[must_use]
    fn gas_density(&self, r: f64) -> f64 {
        self.gas_scale * math::exp(-r / self.characteristic_radius.value()) / r
    }

    /// The share of the gas that is solid at `r`: rock inside the snow line, rock and water ice
    /// from it outwards, times 10^\[Fe/H\].
    #[must_use]
    fn solid_share(&self, r: f64) -> f64 {
        let share = if r < self.snow_line.value() {
            ROCK_MASS_FRACTION
        } else {
            ROCK_MASS_FRACTION + WATER_ICE_MASS_FRACTION
        };
        self.metal_scale * share
    }

    /// The solid mass between `a` and `b`, clipped to the edges, in kilograms: 2πK times the
    /// shares times the taper integral on each side of the snow line.
    #[must_use]
    fn solid_kilograms_between(&self, a: f64, b: f64) -> f64 {
        let a = a.max(self.inner_edge.value());
        let b = b.min(self.outer_edge.value());
        if b <= a {
            return 0.0;
        }
        let snow = self.snow_line.value();
        let rc = self.characteristic_radius.value();
        let rock = if a < snow {
            taper_integral(a, b.min(snow), rc)
        } else {
            0.0
        };
        let icy = if b > snow {
            taper_integral(a.max(snow), b, rc)
        } else {
            0.0
        };
        let shares =
            ROCK_MASS_FRACTION * rock + (ROCK_MASS_FRACTION + WATER_ICE_MASS_FRACTION) * icy;
        self.metal_scale * (TAU * self.gas_scale * shares)
    }
}

/// ∫ₐᵇ e^(−r ÷ `r_c`) dr = `r_c` (e^(−a ÷ `r_c`) − e^(−b ÷ `r_c`)), for a ≤ b, in the form that
/// keeps a narrow interval's integral accurate: −`r_c` e^(−a ÷ `r_c`) (e^(−(b − a) ÷ `r_c`) − 1).
#[must_use]
fn taper_integral(a: f64, b: f64, rc: f64) -> f64 {
    -rc * math::exp(-a / rc) * math::exp_m1(-(b - a) / rc)
}

/// The protoplanetary disc of `host`, with lifetime `lifetime`, from the variates `draws`, cut to
/// `truncation` (P14.T3).
///
/// Nothing is drawn here: the variates come from [`DiscDraws::for_host`], and the lifetime from
/// plan 06's [`disc_lifetime`](crate::stellar::premain::disc_lifetime) of the star's rank, or
/// [`DiscDraws::circumbinary_lifetime`] (ruling 33).
///
/// The result is [`Disc::None`] when no annulus is left between the edges: when the truncation's
/// inner radius is at or beyond its outer one, or beyond the disc's own outer edge, and when the
/// untruncated inner edge already lies at or beyond 3 `r_c`.
///
/// # Panics
///
/// In debug builds, if `lifetime` is not positive and finite.
///
/// # Examples
///
/// The disc of a solar-mass, solar-metallicity star, from its zero-age state and its own disc
/// lifetime rank:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::id::{BodyId, SystemId};
/// use hyperion_sim::planetary::disc::{self, DiscDraws, DiscHost, Truncation};
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::stellar::draws::StarDraws;
/// use hyperion_sim::stellar::premain::disc_lifetime;
/// use hyperion_sim::stellar::sse::{ZCoeffs, zams};
/// use hyperion_sim::units::{AstronomicalUnits, SolarMasses};
///
/// let (seed, system) = (Seed::new(7), SystemId::from_raw(0x0200_0800_2000_0000)?);
/// let (mass, composition) = (SolarMasses::new(1.0), Composition::SOLAR);
/// let coeffs = ZCoeffs::new(composition.z_fit());
/// let host = DiscHost::new(
///     mass,
///     composition.fe_h(),
///     zams::luminosity(mass, &coeffs),
///     zams::radius(mass, &coeffs),
/// )?;
/// let star = StarDraws::for_star(seed, BodyId::new(system, 0));
/// let lifetime = disc_lifetime(mass, star.disc_lifetime());
///
/// let draws = DiscDraws::for_host(seed, system, 0);
/// let disc = disc::derive(&host, lifetime, &draws, Truncation::NONE);
/// let profile = disc.profile().expect("an untruncated disc");
/// assert_eq!(profile.lifetime(), lifetime);
/// // The zero-age Sun's snow line, from 0.70 L☉.
/// let snow = AstronomicalUnits::from(profile.snow_line()).value();
/// assert!((2.2..2.3).contains(&snow));
/// assert!(profile.inner_edge() < profile.snow_line());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn derive(
    host: &DiscHost,
    lifetime: Megayears,
    draws: &DiscDraws,
    truncation: Truncation,
) -> Disc {
    debug_assert!(
        lifetime.value().is_finite() && lifetime.value() > 0.0,
        "a disc lifetime is positive and finite, got {}",
        lifetime.value()
    );
    let m = host.mass.value();

    let log_fraction = (GAS_FRACTION_MEDIAN_DEX
        + GAS_FRACTION_SIGMA_DEX * draws.gas_fraction.value())
    .min(GAS_FRACTION_CAP_DEX);
    let gas_mass = math::exp10(log_fraction) * m * SOLAR_MASS_KG;

    let period = COROTATION_PERIOD_MEDIAN_DAYS
        * SECONDS_PER_DAY
        * math::exp10(COROTATION_PERIOD_SIGMA_DEX * draws.corotation_period.value());
    let corotation = math::cbrt(GM_SUN * m * period * period / (4.0 * PI * PI));
    let radius = host.zams_radius.value() * SOLAR_RADIUS_M;
    let stellar_density = m * SOLAR_MASS_KG / (4.0 / 3.0 * PI * radius * radius * radius);
    let roche = roche_limit_fluid(
        Metres::new(radius),
        KilogramsPerCubicMetre::new(stellar_density),
        INNER_EDGE_BODY_DENSITY,
    )
    .value();
    let inner = (INNER_EDGE_STELLAR_RADII * radius)
        .max(roche)
        .max(corotation);

    let rc = CHARACTERISTIC_RADIUS_AT_SOLAR_MASS_AU
        * METRES_PER_AU
        * m.sqrt()
        * math::exp10(CHARACTERISTIC_RADIUS_SIGMA_DEX * draws.characteristic_radius.value());
    let outer = OUTER_EDGE_CHARACTERISTIC_RADII * rc;
    if inner >= outer {
        return Disc::None;
    }
    // The profile from the inner edge to infinity: ∫ e^(−r ÷ r_c) dr = r_c e^(−r_in ÷ r_c).
    let gas_scale = gas_mass / (TAU * rc * math::exp(-inner / rc));

    let inner_edge = truncation.inner.map_or(inner, |t| inner.max(t.value()));
    let outer_edge = truncation.outer.map_or(outer, |t| outer.min(t.value()));
    if inner_edge >= outer_edge {
        return Disc::None;
    }
    let mut profile = DiscProfile {
        host_mass: host.mass,
        lifetime,
        snow_line: snow_line(host.zams_luminosity),
        inner_edge: Metres::new(inner_edge),
        outer_edge: Metres::new(outer_edge),
        characteristic_radius: Metres::new(rc),
        corotation_period: Seconds::new(period),
        gas_scale,
        metal_scale: math::exp10(host.fe_h.value()),
        drawn_gas_mass: Kilograms::new(gas_mass),
        gas_mass: Kilograms::new(TAU * gas_scale * taper_integral(inner_edge, outer_edge, rc)),
        solid_mass: Kilograms::ZERO,
    };
    profile.solid_mass = Kilograms::new(profile.solid_kilograms_between(inner_edge, outer_edge));
    Disc::Present(profile)
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::order::assert_order_independent;

    use super::*;
    use crate::coords::{CellSize, GenCell};
    use crate::id::Layer;
    use crate::units::AstronomicalUnits;

    const SEED: Seed = Seed::new(0x5eed_0000_0014_0003);

    fn au(x: f64) -> Metres {
        Metres::from(AstronomicalUnits::new(x))
    }

    fn in_au(m: Metres) -> f64 {
        AstronomicalUnits::from(m).value()
    }

    fn system(index: u32) -> SystemId {
        let cell = GenCell::new(CellSize::Ly8, [12, -40, 3]).unwrap();
        SystemId::from_parts(Layer::A, cell, index).unwrap()
    }

    /// A host with zero-age properties close to the Sun's: 0.70 L☉ and 0.89 R☉.
    fn sun_like(mass: f64, fe_h: f64) -> DiscHost {
        DiscHost::new(
            SolarMasses::new(mass),
            Dex::new(fe_h),
            SolarLuminosities::new(0.70 * math::powi(mass, 4)),
            SolarRadii::new(0.89 * math::powf(mass, 0.8)),
        )
        .unwrap()
    }

    fn normal(z: f64) -> StandardNormal {
        StandardNormal::new(z).unwrap()
    }

    fn present(disc: Disc) -> DiscProfile {
        match disc {
            Disc::Present(profile) => profile,
            Disc::None => panic!("expected a disc"),
        }
    }

    fn median_disc(host: &DiscHost) -> DiscProfile {
        present(derive(
            host,
            Megayears::new(2.0),
            &DiscDraws::MEDIAN,
            Truncation::NONE,
        ))
    }

    /// The median and the half-width of the central 68.27%, of `values` sorted in place.
    fn median_and_sigma(values: &mut [f64]) -> (f64, f64) {
        values.sort_by(f64::total_cmp);
        let at = |q: f64| {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                clippy::cast_precision_loss,
                reason = "a quantile's position in a sample of 10⁵ is a small positive integer"
            )]
            let i = (q * (values.len() - 1) as f64).round() as usize;
            values[i]
        };
        (at(0.5), (at(0.841_344_7) - at(0.158_655_3)) / 2.0)
    }

    /// 10⁵ hosts' draws: 400 systems of 250 orbit hosts each.
    fn many_draws() -> Vec<DiscDraws> {
        (0..400)
            .flat_map(|s| (0..250_u8).map(move |h| DiscDraws::for_host(SEED, system(s), h)))
            .collect()
    }

    #[test]
    fn host_draws_are_words_sixteen_h_onwards_of_the_system_stream() {
        let id = system(5);
        for host in [0_u8, 1, 3, 200] {
            let mut stream = Stream::open(SEED, tags::PLANET_DISC, ObjectKey::from(id));
            stream.seek(16 * u64::from(host));
            let draws = DiscDraws::for_host(SEED, id, host);
            assert_same_bits(draws.gas_fraction.value(), stream.standard_normal());
            assert_same_bits(draws.corotation_period.value(), stream.standard_normal());
            assert_same_bits(
                draws.characteristic_radius.value(),
                stream.standard_normal(),
            );
            assert_same_bits(
                draws.circumbinary_lifetime_rank.value(),
                stream.uniform_open(),
            );
            assert!(stream.position() <= 16 * u64::from(host) + DISC_WORDS_PER_HOST);
        }
    }

    #[test]
    fn a_host_s_draws_do_not_depend_on_what_was_drawn_before() {
        let keys: Vec<(u32, u8)> = (0..6).flat_map(|s| (0..8).map(move |h| (s, h))).collect();
        assert_order_independent(&keys, |&(s, h)| DiscDraws::for_host(SEED, system(s), h));
        assert_ne!(
            DiscDraws::for_host(SEED, system(0), 0),
            DiscDraws::for_host(SEED, system(0), 1)
        );
        assert_ne!(
            DiscDraws::for_host(SEED, system(0), 0),
            DiscDraws::for_host(SEED, system(1), 0)
        );
    }

    #[test]
    fn the_same_inputs_give_the_same_disc_twice() {
        let host = sun_like(0.8, -0.2);
        let run = || {
            derive(
                &host,
                Megayears::new(3.1),
                &DiscDraws::for_host(SEED, system(9), 2),
                Truncation::NONE.with_outer(au(12.0)),
            )
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn the_lifetime_is_the_argument() {
        let host = sun_like(1.0, 0.0);
        for lifetime in [0.3, 1.733, 9.0, 15.0] {
            let disc = present(derive(
                &host,
                Megayears::new(lifetime),
                &DiscDraws::MEDIAN,
                Truncation::NONE,
            ));
            assert_same_bits(disc.lifetime().value(), lifetime);
        }
        // A circumbinary disc's lifetime is plan 06's law at the pair's total mass.
        let draws = DiscDraws {
            circumbinary_lifetime_rank: UnitUniform::HALF,
            ..DiscDraws::MEDIAN
        };
        let pair = draws.circumbinary_lifetime(SolarMasses::new(4.0)).value();
        let law = premain::disc_lifetime(SolarMasses::new(4.0), UnitUniform::HALF).value();
        assert_same_bits(pair, law);
    }

    #[test]
    fn gas_fractions_have_the_median_and_width_of_the_model() {
        let mut logs: Vec<f64> = many_draws()
            .iter()
            .map(|d| {
                let disc = median_or_drawn(&sun_like(1.0, 0.0), d);
                math::log10(disc.drawn_gas_mass().value())
            })
            .collect();
        let (median, sigma) = median_and_sigma(&mut logs);
        // Within 2% of the median fraction (0.0086 dex) and of the width.
        assert!(
            (math::exp10(median) / 0.01 - 1.0).abs() < 0.02,
            "median 10^{median}"
        );
        assert!((sigma / 0.5 - 1.0).abs() < 0.02, "σ {sigma} dex");
        // The cap holds a tenth of the star.
        assert!(logs.iter().all(|&l| l <= -1.0 + 1e-12));
        assert!(
            logs.iter().any(|&l| (l + 1.0).abs() < 1e-12),
            "some disc reaches the cap"
        );
    }

    /// The disc of `host` from `draws` with a 2 Myr lifetime and no truncation.
    fn median_or_drawn(host: &DiscHost, draws: &DiscDraws) -> DiscProfile {
        present(derive(host, Megayears::new(2.0), draws, Truncation::NONE))
    }

    #[test]
    fn rotation_periods_have_the_median_and_width_of_the_model() {
        let mut logs: Vec<f64> = many_draws()
            .iter()
            .map(|d| {
                let disc = median_or_drawn(&sun_like(1.0, 0.0), d);
                math::log10(disc.corotation_period().value() / SECONDS_PER_DAY)
            })
            .collect();
        let (median, sigma) = median_and_sigma(&mut logs);
        assert!(
            (math::exp10(median) / 8.0 - 1.0).abs() < 0.02,
            "median 10^{median} d"
        );
        assert!((sigma / 0.25 - 1.0).abs() < 0.02, "σ {sigma} dex");
    }

    #[test]
    fn characteristic_radii_have_the_median_and_width_of_the_model() {
        for (mass, expected_au) in [(1.0, 30.0), (0.25, 15.0)] {
            let host = sun_like(mass, 0.0);
            let mut logs: Vec<f64> = many_draws()
                .iter()
                .filter_map(|d| {
                    let disc = derive(&host, Megayears::new(2.0), d, Truncation::NONE);
                    disc.profile()
                        .map(|p| math::log10(in_au(p.characteristic_radius())))
                })
                .collect();
            // An inner edge beyond 3 r_c needs both draws far out at once; none of these does.
            assert_eq!(logs.len(), 100_000);
            let (median, sigma) = median_and_sigma(&mut logs);
            assert!(
                (math::exp10(median) / expected_au - 1.0).abs() < 0.02,
                "{mass} M☉: median 10^{median} au"
            );
            assert!((sigma / 0.3 - 1.0).abs() < 0.02, "{mass} M☉: σ {sigma} dex");
        }
    }

    #[test]
    fn solid_mass_scales_as_ten_to_the_iron_abundance_exactly() {
        let draws = DiscDraws::for_host(SEED, system(3), 0);
        let solar = median_or_drawn(&sun_like(1.0, 0.0), &draws);
        for fe_h in [-2.5, -0.4, 0.0, 0.1, 0.5] {
            let disc = median_or_drawn(&sun_like(1.0, fe_h), &draws);
            let ratio = disc.solid_mass().value() / solar.solid_mass().value();
            let expected = math::exp10(fe_h);
            assert!(
                (ratio / expected - 1.0).abs() < 4.0 * f64::EPSILON,
                "[Fe/H] {fe_h}: {ratio} against {expected}"
            );
            // The gas does not change with the metals.
            assert_same_bits(disc.gas_mass().value(), solar.gas_mass().value());
        }
    }

    #[test]
    fn the_solid_share_steps_up_by_the_ice_enhancement_at_the_snow_line() {
        let disc = median_disc(&sun_like(1.0, 0.0));
        let snow = disc.snow_line();
        let just_inside = Metres::new(snow.value() * (1.0 - 1e-12));
        let inside = disc.surface_density(just_inside).value()
            / disc.gas_surface_density(just_inside).value();
        let outside = disc.surface_density(snow).value() / disc.gas_surface_density(snow).value();
        assert!((inside - 0.004_89).abs() < 1e-15, "{inside}");
        assert!((outside / inside - ICE_ENHANCEMENT).abs() < 1e-12);
        assert!((ICE_ENHANCEMENT - 2.168).abs() < 1e-3, "{ICE_ENHANCEMENT}");
    }

    #[test]
    fn snow_line_of_one_solar_luminosity_is_two_point_seven_au() {
        assert!((in_au(snow_line(SolarLuminosities::new(1.0))) - 2.7).abs() < 1e-12);
        assert!((in_au(snow_line(SolarLuminosities::new(4.0))) - 5.4).abs() < 1e-12);
        let disc = median_disc(&sun_like(1.0, 0.0));
        assert!((in_au(disc.snow_line()) - 2.7 * 0.7_f64.sqrt()).abs() < 1e-12);
    }

    /// ∫ 2πr Σ dr over [a, b] by composite Simpson in r on 20,000 intervals.
    fn integrate_solids(disc: &DiscProfile, from: f64, to: f64) -> f64 {
        let intervals = 20_000_u32;
        let step = (to - from) / f64::from(intervals);
        let integrand = |r: f64| TAU * r * disc.surface_density(Metres::new(r)).value();
        let mut sum = integrand(from) + integrand(to);
        for i in 1..intervals {
            let weight = if i % 2 == 1 { 4.0 } else { 2.0 };
            sum += weight * integrand(from + step * f64::from(i));
        }
        sum * step / 3.0
    }

    #[test]
    fn the_integrated_surface_density_returns_the_solid_mass() {
        for (s, h, mass, fe_h) in [(1, 0, 1.0, 0.0), (2, 7, 0.3, -0.5), (3, 1, 2.0, 0.3)] {
            let host = sun_like(mass, fe_h);
            let disc = median_or_drawn(&host, &DiscDraws::for_host(SEED, system(s), h));
            let (inner, snow, outer) = (
                disc.inner_edge().value(),
                disc.snow_line().value(),
                disc.outer_edge().value(),
            );
            // The integrand steps at the snow line, so each side is integrated on its own.
            let integral = if snow > inner && snow < outer {
                // The rocky side ends just short of the snow line, where the share steps up.
                integrate_solids(&disc, inner, snow.next_down())
                    + integrate_solids(&disc, snow, outer)
            } else {
                integrate_solids(&disc, inner, outer)
            };
            let solid = disc.solid_mass().value() * EARTH_MASS_KG;
            assert!(
                (integral / solid - 1.0).abs() < 1e-9,
                "{mass} M☉: {integral} kg against {solid} kg"
            );
        }
    }

    #[test]
    fn the_solid_mass_between_the_edges_is_the_solid_mass_and_adds_up() {
        let disc = median_or_drawn(
            &sun_like(1.0, 0.1),
            &DiscDraws::for_host(SEED, system(4), 0),
        );
        let (inner, outer) = (disc.inner_edge(), disc.outer_edge());
        assert_same_bits(
            disc.solid_mass_between(inner, outer).value(),
            disc.solid_mass().value(),
        );
        // Beyond the edges there is nothing more.
        assert_same_bits(
            disc.solid_mass_between(Metres::ZERO, au(1e4)).value(),
            disc.solid_mass().value(),
        );
        assert_same_bits(disc.solid_mass_between(au(3.0), au(2.0)).value(), 0.0);
        let cuts = [0.05, 0.1, 0.7, 2.259, 2.26, 4.0, 11.0, 29.0, 60.0].map(au);
        for window in cuts.windows(3) {
            let [a, m, b] = [window[0], window[1], window[2]];
            let whole = disc.solid_mass_between(a, b).value();
            let parts =
                disc.solid_mass_between(a, m).value() + disc.solid_mass_between(m, b).value();
            assert!(
                (parts - whole).abs() <= 1e-12 * whole.max(f64::MIN_POSITIVE),
                "{} to {} au: {parts} against {whole}",
                in_au(a),
                in_au(b)
            );
        }
    }

    #[test]
    fn the_median_solar_disc_has_its_expected_figures() {
        let disc = median_disc(&sun_like(1.0, 0.0));
        assert!((disc.drawn_gas_mass().value() - 0.01).abs() < 1e-15);
        assert!((in_au(disc.characteristic_radius()) - 30.0).abs() < 1e-12);
        assert!((in_au(disc.outer_edge()) - 90.0).abs() < 1e-12);
        // The profile is normalised to infinity, and 3 r_c holds 95% of it.
        let kept = disc.gas_mass() / disc.drawn_gas_mass();
        let expected = 1.0
            - math::exp(-(disc.outer_edge() - disc.inner_edge()) / disc.characteristic_radius());
        assert!((kept - expected).abs() < 1e-14, "{kept}");
        assert!((kept - 0.95).abs() < 0.001, "{kept}");
        // The corotation radius at 8 days about a solar mass, 0.0783 au, is the inner edge.
        assert!(
            (in_au(disc.inner_edge()) - 0.078_3).abs() < 1e-4,
            "{}",
            in_au(disc.inner_edge())
        );
        // About 32 M⊕ of solids, most of it icy, and some beyond 30 au for a Kuiper-like belt.
        let solids = disc.solid_mass().value();
        assert!((30.0..34.0).contains(&solids), "{solids} M⊕");
        let belt = disc.solid_mass_between(au(30.0), au(50.0)).value();
        assert!(belt > 5.0, "{belt} M⊕ in 30–50 au");
        let rocky = disc
            .solid_mass_between(disc.inner_edge(), disc.snow_line())
            .value();
        assert!(rocky < 0.1 * solids, "{rocky} M⊕ of rock");
    }

    #[test]
    fn the_inner_edge_is_the_largest_of_its_three_limits() {
        // The corotation radius wins at the median period.
        let sun = sun_like(1.0, 0.0);
        let disc = median_disc(&sun);
        let period = 8.0 * SECONDS_PER_DAY;
        let corotation = math::cbrt(GM_SUN * period * period / (4.0 * PI * PI));
        assert!((disc.inner_edge().value() / corotation - 1.0).abs() < 1e-14);
        // At a very short period the stellar radii or the Roche limit take over.
        let fast = DiscDraws {
            corotation_period: normal(-8.0),
            ..DiscDraws::MEDIAN
        };
        let disc = median_or_drawn(&sun, &fast);
        let radius = 0.89 * SOLAR_RADIUS_M;
        let rho = SOLAR_MASS_KG / (4.0 / 3.0 * PI * radius * radius * radius);
        let roche = 2.456 * radius * math::cbrt(rho / 1_000.0);
        let floor = (2.5 * radius).max(roche);
        assert!((disc.inner_edge().value() / floor - 1.0).abs() < 1e-12);
        // The Sun's fluid Roche limit for 1,000 kg m⁻³ lies at 2.75 R☉, beyond 2.5 × 0.89 R☉.
        assert!(
            (roche / SOLAR_RADIUS_M - 2.75).abs() < 0.01,
            "{}",
            roche / SOLAR_RADIUS_M
        );
    }

    #[test]
    fn truncation_cuts_mass_away_and_renormalises_nothing() {
        let host = sun_like(1.0, 0.0);
        let draws = DiscDraws::for_host(SEED, system(6), 0);
        let whole = median_or_drawn(&host, &draws);
        let cut = present(derive(
            &host,
            Megayears::new(2.0),
            &draws,
            Truncation::NONE.with_inner(au(0.5)).with_outer(au(5.0)),
        ));
        assert!((in_au(cut.inner_edge()) - 0.5).abs() < 1e-12);
        assert!((in_au(cut.outer_edge()) - 5.0).abs() < 1e-12);
        // The same surface density where both have it, and only the annulus's mass.
        let at = au(1.3);
        assert_same_bits(
            cut.surface_density(at).value(),
            whole.surface_density(at).value(),
        );
        let kept = whole.solid_mass_between(au(0.5), au(5.0)).value();
        assert!((cut.solid_mass().value() / kept - 1.0).abs() < 1e-14);
        assert!(cut.gas_mass().value() < whole.gas_mass().value());
        // A truncation outside the disc's own edges changes nothing.
        let loose = present(derive(
            &host,
            Megayears::new(2.0),
            &draws,
            Truncation::NONE.with_inner(au(0.001)).with_outer(au(1e4)),
        ));
        assert_eq!(loose, whole);
    }

    #[test]
    fn inner_edge_below_outer_edge_or_no_disc() {
        let host = sun_like(1.0, 0.0);
        let draws = DiscDraws::for_host(SEED, system(8), 0);
        let inverted = Truncation::NONE.with_inner(au(3.0)).with_outer(au(2.0));
        assert_eq!(
            derive(&host, Megayears::new(2.0), &draws, inverted),
            Disc::None
        );
        let inside_the_star = Truncation::NONE.with_outer(au(0.01));
        assert_eq!(
            derive(&host, Megayears::new(2.0), &draws, inside_the_star),
            Disc::None
        );
        assert_same_bits(Disc::None.solid_mass().value(), 0.0);
        // Over many hosts, zones and draws every disc has a positive annulus.
        let mut formed = 0;
        for (i, d) in many_draws().iter().take(20_000).enumerate() {
            #[expect(clippy::cast_precision_loss, reason = "i is below 20,000")]
            let x = i as f64;
            let host = sun_like(0.1 + (x % 97.0) / 20.0, 0.0);
            let zone = Truncation::NONE
                .with_inner(au(0.01 * (x % 13.0)))
                .with_outer(au(0.5 + (x % 29.0)));
            match derive(&host, Megayears::new(1.0), d, zone) {
                Disc::Present(p) => {
                    formed += 1;
                    assert!(p.inner_edge() < p.outer_edge());
                    assert!(p.solid_mass().value() > 0.0 && p.gas_mass().value() > 0.0);
                }
                Disc::None => {}
            }
        }
        assert!(formed > 15_000, "{formed} discs");
    }

    /// Hayashi's (1981) minimum-mass solar nebula: solids of 7.1 g cm⁻² × (r ÷ 1 au)^−3/2 inside
    /// the snow line at 2.7 au and 30 g cm⁻² × (r ÷ 1 au)^−3/2 beyond it, a hundredth or so of its
    /// gas, 1.7 × 10³ g cm⁻² × (r ÷ 1 au)^−3/2 (Armitage 2007, eq. 4), in kg m⁻².
    fn hayashi_solids(r_au: f64) -> KilogramsPerSquareMetre {
        let at_1_au = if r_au < 2.7 { 71.0 } else { 300.0 };
        KilogramsPerSquareMetre::new(at_1_au * math::powf(r_au, -1.5))
    }

    #[test]
    fn a_minimum_mass_nebula_isolates_about_an_earth_mass_at_five_au() {
        let sun = SolarMasses::new(1.0);
        // Kennedy and Kenyon (2008, ApJ 673, 502, §2): in the minimum-mass nebula "M_iso ≈ 0.1 (1)
        // M⊕ at 1 (5) AU". Hayashi's nebula gives 1.1 M⊕ at 5 au.
        let at_5 = isolation_mass(hayashi_solids(5.0), au(5.0), sun).value();
        assert!((0.5..2.0).contains(&at_5), "{at_5} M⊕ at 5 au");
        // At 1 au it gives 0.039 M⊕: Armitage's 0.07 M⊕ for 10 g cm⁻² (eq. 202) at 7.1 g cm⁻².
        let at_1 = isolation_mass(hayashi_solids(1.0), au(1.0), sun).value();
        let armitage = 0.07 * math::powf(0.71, 1.5);
        assert!((at_1 / armitage - 1.0).abs() < 0.1, "{at_1} M⊕ at 1 au");
    }

    #[test]
    fn isolation_masses_match_lissauer_s_closed_form() {
        // An enhanced disc of Σ = 10 g cm⁻² about a solar mass, four times Hayashi's at 5 au:
        // 0.07 M⊕ at 1 au and 9 M⊕ at 5 au (Armitage 2007, eqs. 202–203, after Lissauer 1993 and
        // Pollack et al. 1996).
        let sigma = KilogramsPerSquareMetre::new(100.0);
        let sun = SolarMasses::new(1.0);
        let at_1 = isolation_mass(sigma, au(1.0), sun).value();
        let at_5 = isolation_mass(sigma, au(5.0), sun).value();
        assert!((0.05..0.2).contains(&at_1), "{at_1} M⊕ at 1 au");
        assert!((3.0..15.0).contains(&at_5), "{at_5} M⊕ at 5 au");
        // Armitage's eq. 201 evaluated on its own: (8 ÷ √3) π^(3/2) C^(3/2) M★^(−1/2) Σ^(3/2) a³.
        let c = FEEDING_ZONE_HALF_WIDTH_HILL;
        let a = au(1.0).value();
        let eq_201 =
            8.0 / 3.0_f64.sqrt() * math::powf(PI * c, 1.5) * math::powf(100.0, 1.5) * a * a * a
                / (SOLAR_MASS_KG).sqrt()
                / EARTH_MASS_KG;
        assert!(
            (at_1 / eq_201 - 1.0).abs() < 1e-12,
            "{at_1} against {eq_201}"
        );
        assert!((at_5 / at_1 - 125.0).abs() < 1e-9);
        // It scales as Σ^(3/2) a³ M★^(−1/2).
        let double = isolation_mass(sigma * 4.0, au(2.0), SolarMasses::new(4.0)).value();
        assert!((double / (at_1 * 8.0 * 8.0 / 2.0) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn a_disc_s_isolation_mass_is_the_closed_form_of_its_own_surface_density() {
        let disc = median_disc(&sun_like(1.0, 0.0));
        for a in [0.1, 1.0, 5.0, 20.0] {
            let own = disc.isolation_mass(au(a)).value();
            let closed = isolation_mass(disc.surface_density(au(a)), au(a), SolarMasses::new(1.0));
            assert_same_bits(own, closed.value());
            assert!(own > 0.0);
        }
        assert_same_bits(disc.isolation_mass(au(0.01)).value(), 0.0);
        assert!(disc.isolation_mass(au(40.0)).value() > 0.0);
        assert_same_bits(disc.isolation_mass(au(100.0)).value(), 0.0);
    }

    #[test]
    fn hosts_are_validated_once() {
        let ok = (
            SolarMasses::new(1.0),
            Dex::ZERO,
            SolarLuminosities::new(1.0),
            SolarRadii::new(1.0),
        );
        assert!(DiscHost::new(ok.0, ok.1, ok.2, ok.3).is_ok());
        assert_eq!(
            DiscHost::new(SolarMasses::new(-1.0), ok.1, ok.2, ok.3),
            Err(BuildDiscHostError::MassNotPositive)
        );
        assert_eq!(
            DiscHost::new(ok.0, Dex::new(f64::NAN), ok.2, ok.3),
            Err(BuildDiscHostError::MetallicityNotFinite)
        );
        assert_eq!(
            DiscHost::new(ok.0, ok.1, SolarLuminosities::ZERO, ok.3),
            Err(BuildDiscHostError::LuminosityNotPositive)
        );
        assert_eq!(
            DiscHost::new(ok.0, ok.1, ok.2, SolarRadii::new(f64::INFINITY)),
            Err(BuildDiscHostError::RadiusNotPositive)
        );
        for e in [
            BuildDiscHostError::MassNotPositive,
            BuildDiscHostError::MetallicityNotFinite,
            BuildDiscHostError::LuminosityNotPositive,
            BuildDiscHostError::RadiusNotPositive,
        ] {
            let text = e.to_string();
            assert!(text.starts_with('a') && !text.ends_with('.'), "{text}");
        }
    }
}
