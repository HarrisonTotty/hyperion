//! The cometary halo: a system's Oort cloud, a statistical population and nothing else (plan 14,
//! P14.T21.d; design notes 11 and 14).
//!
//! # What a halo is
//!
//! - **Bounds.** From 2,000 au × (M★ ÷ M☉)^⅓ ([`HALO_INNER_RADIUS_AU`]) out to the smallest of
//!   10⁵ au × (M★ ÷ M☉)^⅓ ([`HALO_OUTER_RADIUS_AU`], ruling 84.2), the system's strip radius, 0.49 of its
//!   sphere of influence (design note 14), and a third of the distance to any wide companion
//!   ([`COMPANION_SHARE`]). A halo whose outer radius falls below its inner one does not exist,
//!   which is the case throughout the nuclear cluster.
//! - **Comets.** A number over 1 km drawn log-uniform over 10¹¹–10¹² ([`COMET_COUNT`]), scaled by
//!   the disc's solids against a median solar-mass disc's ([`REFERENCE_DISC_SOLIDS`]), since the
//!   halo is the planetesimals the giants scattered out (design note 5).
//! - **A scatterer.** Present only if the system has a planet over [`SCATTERER_MASS`] (10 M⊕)
//!   beyond its host's snow line to scatter them ([`Scatterer`]).
//! - **Comets reaching the inner system.** A rate of long-period comets reaching perihelion inside
//!   5 au × √(L ÷ L☉), which P14.T31 turns into `system.comet` events
//!   ([`CometaryHalo::comet_rate`]): the halo supplies every long-period comet, new and returning,
//!   three times those on their first passage ([`CometaryHalo::new_comet_rate`]; ruling 84.5).
//!   Nothing here makes an event.
//! - **Evolved hosts.** A host that loses mass loses part of its halo (design note 11):
//!   [`CometaryHalo::at`].
//!
//! # Sources, re-checked
//!
//! - **Radii.** Duncan, Quinn and Tremaine (1987, AJ 94, 1330) start their comets at 2,000 au and
//!   end with a cloud from a few thousand to about 10⁵ au. Kaib and Volk (2022, arXiv:2206.00010,
//!   §2) put its inner edge where the Galactic tide's torque and planetary diffusion balance,
//!   near 5,000 au (for bodies scattered by Uranus and Neptune), and its outer limit at the
//!   Galactic tide's reach, 1–2 × 10⁵ au (Heisler and Tremaine 1986, Icarus 65, 13); the cloud
//!   spans "a few thousand au and ∼10⁵ au". Ruling 84.2 puts the outer radius at 10⁵ au in place
//!   of plan 14's 50,000 and keeps the inner at 2,000; the strip radius of design note 14 is the
//!   tide's own bound, 1.39 × 10⁵ au for the Sun (0.4895 of 2.83 × 10⁵ au).
//! - **Numbers.** Francis (2005, ApJ 635, 1348, §6): about 5 × 10¹¹ comets to absolute magnitude
//!   17 in the outer cloud; Kaib and Volk (2022): 7–8 × 10¹¹ of total magnitude under 11, a
//!   radius of about 0.3 km (Sosa and Fernández 2011). Plan 14's 10¹¹–10¹² over 1 km holds them.
//! - **The rate.** Kaib and Volk (2022, §2) quote 2.9 dynamically new comets a year reaching
//!   perihelion inside 4 au with total magnitude under 11 (Francis 2005, §6.2, about 3; Fouchard
//!   et al. 2017, 3.6), against 7.5 × 10¹¹ such comets in the cloud: a share of 3.9 × 10⁻¹² of the
//!   cloud a year ([`NEW_COMET_SHARE_PER_YEAR`]), the same for any size limit that counts both
//!   alike. Francis finds new comets' perihelia uniform in distance, about 0.8 a year per au, so
//!   the rate inside 5 au × √L is that share × (5√L ÷ 4). Every long-period comet, new and
//!   returning, is three times that ([`LONG_PERIOD_PER_NEW_COMET`]): about 11 a year inside 4 au
//!   of which about 3 are new (Francis 2005; Vokrouhlický, Nesvorný and Dones 2019, AJ 157, 181,
//!   §2.3). The Solar System input, 7.5 × 10¹¹ comets (Kaib and Volk 2022, §2.4.1), gives 10.9
//!   long-period comets a year inside 5 au.
//! - **Mass loss** (design note 11; Veras et al. 2011, MNRAS 417, 2104, eq. 15 and §3.3; ruling
//!   84.4). Whether a comet's orbit follows the host's mass loss adiabatically depends on Veras
//!   et al.'s mass-loss index Ψ = (1 ÷ 2π) (α ÷ 1 M☉ yr⁻¹) (a ÷ 1 au)^(3⁄2) (M ÷ 1 M☉)^(−3⁄2),
//!   α the mass-loss rate. Deep in the adiabatic regime, Ψ ≪ 1, it keeps every comet; in the
//!   runaway regime it loses what an instantaneous loss would, `L_inst`: a comet at distance r on
//!   an orbit of semi-major axis a stays bound only if r > 2a (1 − M ÷ M₀), averaged over the time
//!   along each orbit and a thermal distribution of eccentricities, f(e) = 2e
//!   ([`surviving_share`]). Between, the loss at a is `L_inst` × clamp(log(Ψ ÷ 0.1) ÷ log 30, 0, 1),
//!   from the transition near Ψ = 0.1 (Veras et al.'s `Ψ_bif` of 0.1–1) to full at Ψ = 3, integrated
//!   over the halo's comets, whose number density falls as r^(−3.5) (Duncan, Quinn and Tremaine
//!   1987; [`surviving_fraction`]). A Sun-like star's thermally pulsing AGB, 0.5 M☉ over about
//!   1 Myr, has Ψ ≈ 3 at 10⁵ au, where it loses Veras et al.'s "up to 20%", and Ψ ≈ 0.1 at 10⁴
//!   au, inside which nothing is lost. The survivors' orbits widen by M₀ ÷ M, as the planets' do,
//!   and the halo is cut again at the strip radius. A supernova is the limit of an infinite rate.
//!
//! # Index and draws
//!
//! The halo is the population in belt slot `0xEF` ([`HALO_SLOT`]), sub-index 0, and has no named
//! members. On [`tags::COMETARY_POPULATION`], keyed by the system's ID, word 0 is the rank of its
//! count; words 1–7 are reserved. A system has one halo, so the draw number carries no host; a
//! halo per host would take words 8h onwards.

use core::f64::consts::{PI, TAU};

use crate::events::EventsPerSecond;
use crate::id::SystemId;
use crate::math;
use crate::planetary::index::{BodyIndex, BodySlot, BodySub};
use crate::planetary::placement::Neighbour;
use crate::rng::{ObjectKey, Seed, Stream, tags};
use crate::stellar::draws::UnitUniform;
use crate::units::consts::METRES_PER_AU;
use crate::units::{EarthMasses, Metres, SolarLuminosities, SolarMasses, SolarMassesPerYear};

/// The belt slot of the cometary halo: `Belt(15)`, slot `0xEF`, the last.
pub const HALO_SLOT: u8 = 15;

/// The halo's inner radius about a host of 1 M☉, au: 2,000, scaling as (M★ ÷ M☉)^⅓ (P14.T21.d;
/// Duncan, Quinn and Tremaine 1987).
pub const HALO_INNER_RADIUS_AU: f64 = 2_000.0;

/// The halo's outer radius about a host of 1 M☉ before its other bounds, au: 10⁵, scaling as
/// (M★ ÷ M☉)^⅓ (P14.T21.d; ruling 84.2, in place of the plan's 50,000; Kaib and Volk 2022).
pub const HALO_OUTER_RADIUS_AU: f64 = 100_000.0;

/// Long-period comets reaching the inner system for each on its first passage: 3 (ruling 84.5;
/// Vokrouhlický, Nesvorný and Dones 2019, §2.3: about 11 a year inside 4 au, about 3 of them
/// new).
pub const LONG_PERIOD_PER_NEW_COMET: f64 = 3.0;

/// The mass-loss index Ψ below which a comet's orbit follows its host's mass loss
/// adiabatically: 0.1, the low end of Veras et al.'s (2011, eq. 22) `Ψ_bif` of 0.1–1 (ruling 84.4).
pub const ADIABATIC_INDEX_LIMIT: f64 = 0.1;

/// The mass-loss index Ψ from which a comet is lost as in an instantaneous loss: 3, where Veras et
/// al. (2011, §3.3.2) find up to 20% of an Oort cloud at 10⁵ au ejected (ruling 84.4).
pub const RUNAWAY_INDEX: f64 = 3.0;

/// Points of the quadrature over the halo's radii in [`surviving_fraction`]: 256, a fixed count.
const RADIAL_POINTS: u32 = 256;

/// The share of the distance to a wide companion beyond which the halo does not reach: a third
/// (P14.T21.d; plan 14's figure, given without a source). The caller passes the companion's
/// pericentre distance.
pub const COMPANION_SHARE: f64 = 1.0 / 3.0;

/// The range of a halo's number of comets over 1 km about a median solar-mass disc, log-uniform:
/// 10¹¹–10¹² (P14.T21.d).
pub const COMET_COUNT: (f64, f64) = (1e11, 1e12);

/// The solids of the median disc of a 1 M☉ host of solar metallicity, the disc the comet count is
/// scaled against: 32.2 M⊕ (P14.T3 as built, ruling 38; tested against the disc itself).
pub const REFERENCE_DISC_SOLIDS: EarthMasses = EarthMasses::new(32.2);

/// The mass above which a planet beyond the snow line scatters planetesimals into a halo: 10 M⊕
/// (P14.T21.d; plan 14's figure, given without a source). Uranus and Neptune, 14.5 and 17.1 M⊕,
/// which scattered the comets of the Oort cloud's inner edge (Kaib and Volk 2022), pass it.
pub const SCATTERER_MASS: EarthMasses = EarthMasses::new(10.0);

/// The share of a halo's comets that reach perihelion inside 4 au on their first passage each
/// year: 2.9 ÷ 7.5 × 10¹¹ = 3.9 × 10⁻¹² (Kaib and Volk 2022, §2; module documentation).
pub const NEW_COMET_SHARE_PER_YEAR: f64 = 2.9 / 7.5e11;

/// The perihelion distance the share of [`NEW_COMET_SHARE_PER_YEAR`] counts to, au: 4.
pub const NEW_COMET_PERIHELION_AU: f64 = 4.0;

/// The inner system about a host of 1 L☉ that the rate of new comets counts arrivals into, au: 5,
/// scaling as √(L ÷ L☉), P14.T31's reach for a comet's perihelion.
pub const INNER_SYSTEM_AU: f64 = 5.0;

/// Points of the midpoint rule over eccentricity in [`surviving_share`]: 1,024, a fixed count.
const SURVIVAL_POINTS: u32 = 1_024;

/// Whether a system has a planet to scatter planetesimals into a halo: one over
/// [`SCATTERER_MASS`] beyond its host's snow line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Scatterer {
    /// No such planet: the system has no halo.
    Absent,
    /// At least one.
    Present,
}

impl Scatterer {
    /// Whether any of `planets`, about a host whose snow line is `snow_line`, is a scatterer: over
    /// [`SCATTERER_MASS`] and at or beyond the snow line.
    #[must_use]
    pub fn among(planets: &[Neighbour], snow_line: Metres) -> Self {
        if planets
            .iter()
            .any(|p| p.mass() > SCATTERER_MASS && p.semi_major_axis() >= snow_line)
        {
            Self::Present
        } else {
            Self::Absent
        }
    }

    /// Present if either is: for a system of several hosts, each judged against its own snow line.
    #[must_use]
    pub const fn or(self, other: Self) -> Self {
        match (self, other) {
            (Self::Absent, Self::Absent) => Self::Absent,
            (Self::Present, _) | (_, Self::Present) => Self::Present,
        }
    }
}

/// What a system's halo reads of its host, all plain arguments (P14.T21.d).
///
/// P14.T30.a builds it: the mass is the zero-age mass the halo surrounds (the primary's, or a
/// close pair's), the solids those of that host's disc, the strip radius the context's
/// (`SystemContext::strip_radius`) and the companion distance the pericentre of the nearest wide
/// companion's orbit, if any.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HaloHost {
    mass: SolarMasses,
    disc_solids: EarthMasses,
    strip_radius: Metres,
    companion: Option<Metres>,
}

impl HaloHost {
    /// A host of zero-age mass `mass` whose disc held `disc_solids` of solids, in a system of
    /// strip radius `strip_radius`, with its nearest wide companion at `companion`.
    ///
    /// # Panics
    ///
    /// In debug builds, unless the mass and the strip radius are positive and the solids and the
    /// companion's distance not negative.
    #[must_use]
    pub fn new(
        mass: SolarMasses,
        disc_solids: EarthMasses,
        strip_radius: Metres,
        companion: Option<Metres>,
    ) -> Self {
        debug_assert!(mass.value() > 0.0 && strip_radius.value() > 0.0);
        debug_assert!(disc_solids.value() >= 0.0);
        debug_assert!(companion.is_none_or(|d| d.value() >= 0.0));
        Self {
            mass,
            disc_solids,
            strip_radius,
            companion,
        }
    }
}

/// A system's cometary halo (P14.T21.d): a statistical population in belt slot `0xEF`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CometaryHalo {
    host_mass: SolarMasses,
    inner_edge: Metres,
    outer_edge: Metres,
    strip_radius: Metres,
    comets: f64,
}

impl CometaryHalo {
    /// The halo's body index: belt slot `0xEF`, sub-index 0.
    ///
    /// # Panics
    ///
    /// Never: belt slot 15 is in the layout.
    #[must_use]
    pub fn index(&self) -> BodyIndex {
        BodyIndex::new(BodySlot::Belt(HALO_SLOT), BodySub::Primary)
            .expect("belt slot 15 is in the layout")
    }

    /// The zero-age mass of the host it surrounds.
    #[must_use]
    pub const fn host_mass(&self) -> SolarMasses {
        self.host_mass
    }

    /// Its inner radius, from the host, as formed.
    #[must_use]
    pub const fn inner_edge(&self) -> Metres {
        self.inner_edge
    }

    /// Its outer radius, from the host, as formed: never beyond the strip radius.
    #[must_use]
    pub const fn outer_edge(&self) -> Metres {
        self.outer_edge
    }

    /// Its number of comets over 1 km, as formed.
    #[must_use]
    pub const fn comets(&self) -> f64 {
        self.comets
    }

    /// The rate at which comets on their first passage reach perihelion inside 5 au × √(L ÷ L☉)
    /// of a host of luminosity `luminosity`, as formed: [`NEW_COMET_SHARE_PER_YEAR`] of the comets
    /// a year per 4 au of perihelion distance (module documentation).
    #[must_use]
    pub fn new_comet_rate(&self, luminosity: SolarLuminosities) -> EventsPerSecond {
        new_comet_rate(self.comets, luminosity)
    }

    /// The rate at which long-period comets, new and returning, reach perihelion inside 5 au ×
    /// √(L ÷ L☉) of a host of luminosity `luminosity`, as formed:
    /// [`LONG_PERIOD_PER_NEW_COMET`] times [`new_comet_rate`](Self::new_comet_rate) (ruling
    /// 84.5). P14.T31 makes the events.
    ///
    /// # Examples
    ///
    /// The Solar System's 7.5 × 10¹¹ comets send about eleven a year inside 5 au:
    ///
    /// ```
    /// use hyperion_sim::Seed;
    /// use hyperion_sim::id::SystemId;
    /// use hyperion_sim::planetary::halo::{HaloHost, Scatterer, halo};
    /// use hyperion_sim::units::{AstronomicalUnits, EarthMasses, Metres, SolarLuminosities, SolarMasses};
    ///
    /// let strip = Metres::from(AstronomicalUnits::new(1.39e5));
    /// let sun = HaloHost::new(SolarMasses::new(1.0), EarthMasses::new(32.2), strip, None);
    /// let system = SystemId::from_raw(0x0200_0800_2000_0000)?;
    /// let halo = halo(Seed::new(7), system, &sun, Scatterer::Present).expect("a halo");
    /// let per_year = |comets: f64| {
    ///     halo.comet_rate(SolarLuminosities::new(1.0)).value() * 31_557_600.0 * comets / halo.comets()
    /// };
    /// assert!((10.5..14.0).contains(&per_year(7.5e11)));
    /// # Ok::<(), hyperion_sim::id::DecodeSystemIdError>(())
    /// ```
    #[must_use]
    pub fn comet_rate(&self, luminosity: SolarLuminosities) -> EventsPerSecond {
        comet_rate(self.comets, luminosity)
    }

    /// The halo about its host once the host's mass is `mass_now`, having lost it at a rate of
    /// `mass_loss_rate` in its fastest phase (the mean rate over a thermally pulsing AGB, the
    /// mass it shed there over its duration; infinite for a supernova): what survives (design
    /// note 11, [`surviving_fraction`]), with its radii widened by the mass lost and cut again at
    /// the strip radius; `None` once nothing of it is left. A host that has lost no mass keeps the
    /// halo as formed.
    ///
    /// # Examples
    ///
    /// The Sun as a white dwarf of 0.54 M☉, after an AGB shedding 0.5 M☉ in a million years,
    /// keeps nearly every comet, and loses a supernova's share only in the limit:
    ///
    /// ```
    /// use hyperion_sim::Seed;
    /// use hyperion_sim::id::SystemId;
    /// use hyperion_sim::planetary::halo::{HaloHost, Scatterer, halo};
    /// use hyperion_sim::units::{AstronomicalUnits, EarthMasses, Metres, SolarMasses, SolarMassesPerYear};
    ///
    /// let strip = Metres::from(AstronomicalUnits::new(1.39e5));
    /// let sun = HaloHost::new(SolarMasses::new(1.0), EarthMasses::new(32.2), strip, None);
    /// let system = SystemId::from_raw(0x0200_0800_2000_0000)?;
    /// let halo = halo(Seed::new(7), system, &sun, Scatterer::Present).expect("a halo");
    /// let white_dwarf = SolarMasses::new(0.54);
    /// let agb = halo.at(white_dwarf, SolarMassesPerYear::new(5e-7)).expect("a halo is left");
    /// assert!(agb.comets() / halo.comets() > 0.95);
    /// assert!(agb.inner_edge() > halo.inner_edge());
    /// let sudden = halo.at(white_dwarf, SolarMassesPerYear::new(f64::INFINITY)).expect("some left");
    /// assert!((0.75..0.85).contains(&(sudden.comets() / halo.comets())));
    /// # Ok::<(), hyperion_sim::id::DecodeSystemIdError>(())
    /// ```
    #[must_use]
    pub fn at(&self, mass_now: SolarMasses, mass_loss_rate: SolarMassesPerYear) -> Option<HaloAt> {
        let ratio = (mass_now.value() / self.host_mass.value()).min(1.0);
        if ratio <= 0.0 {
            return None;
        }
        let share = surviving_fraction(
            ratio,
            mass_loss_rate,
            self.host_mass,
            self.inner_edge,
            self.outer_edge,
        );
        if share <= 0.0 {
            return None;
        }
        let inner = self.inner_edge / ratio;
        let outer = Metres::new((self.outer_edge.value() / ratio).min(self.strip_radius.value()));
        (outer > inner).then_some(HaloAt {
            inner_edge: inner,
            outer_edge: outer,
            comets: self.comets * share,
        })
    }
}

/// The share of a halo between `inner` and `outer` about a host of initial mass `host_mass` that
/// stays bound when the host's mass falls to `ratio` of it at a rate of `mass_loss_rate` (design
/// note 11; ruling 84.4; module documentation): 1 − `L_inst` × the mean over the halo's comets of
/// clamp(log(Ψ ÷ 0.1) ÷ log 30, 0, 1), with `L_inst` = 1 − [`surviving_share`]`(ratio)` and Ψ
/// Veras et al.'s (2011, eq. 15) index at each radius for the host's initial mass.
///
/// The comets' number density falls as r^(−3.5), so their number per unit ln r as r^(−½); the
/// mean is a midpoint rule of fixed length in ln r.
#[must_use]
pub fn surviving_fraction(
    ratio: f64,
    mass_loss_rate: SolarMassesPerYear,
    host_mass: SolarMasses,
    inner: Metres,
    outer: Metres,
) -> f64 {
    let instantaneous = 1.0 - surviving_share(ratio);
    if instantaneous <= 0.0 {
        return 1.0;
    }
    let (lo, hi) = (math::ln(inner.value()), math::ln(outer.value()));
    if hi <= lo {
        return 1.0 - instantaneous;
    }
    let alpha = mass_loss_rate.value().max(0.0);
    let step = (hi - lo) / f64::from(RADIAL_POINTS);
    let span = math::ln(RUNAWAY_INDEX / ADIABATIC_INDEX_LIMIT);
    let (mut weight, mut lost) = (0.0, 0.0);
    for n in 0..RADIAL_POINTS {
        let r = math::exp(lo + (f64::from(n) + 0.5) * step);
        let w = 1.0 / r.sqrt();
        let a_au = r / METRES_PER_AU;
        let psi = alpha * a_au * a_au.sqrt() / (TAU * host_mass.value() * host_mass.value().sqrt());
        let regime = if psi.is_infinite() {
            1.0
        } else if psi <= ADIABATIC_INDEX_LIMIT {
            0.0
        } else {
            (math::ln(psi / ADIABATIC_INDEX_LIMIT) / span).clamp(0.0, 1.0)
        };
        weight += w;
        lost += w * regime;
    }
    1.0 - instantaneous * lost / weight
}

/// A halo at a time, after its host's mass loss ([`CometaryHalo::at`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HaloAt {
    inner_edge: Metres,
    outer_edge: Metres,
    comets: f64,
}

impl HaloAt {
    /// Its inner radius.
    #[must_use]
    pub const fn inner_edge(&self) -> Metres {
        self.inner_edge
    }

    /// Its outer radius, never beyond the strip radius.
    #[must_use]
    pub const fn outer_edge(&self) -> Metres {
        self.outer_edge
    }

    /// Its comets over 1 km.
    #[must_use]
    pub const fn comets(&self) -> f64 {
        self.comets
    }

    /// Its rate of new comets about a host now of luminosity `luminosity`
    /// ([`CometaryHalo::new_comet_rate`]).
    #[must_use]
    pub fn new_comet_rate(&self, luminosity: SolarLuminosities) -> EventsPerSecond {
        new_comet_rate(self.comets, luminosity)
    }

    /// Its rate of long-period comets about a host now of luminosity `luminosity`
    /// ([`CometaryHalo::comet_rate`]).
    #[must_use]
    pub fn comet_rate(&self, luminosity: SolarLuminosities) -> EventsPerSecond {
        comet_rate(self.comets, luminosity)
    }
}

/// The rate of long-period comets of a halo of `comets` about a host of luminosity `luminosity`.
#[must_use]
fn comet_rate(comets: f64, luminosity: SolarLuminosities) -> EventsPerSecond {
    EventsPerSecond::new(LONG_PERIOD_PER_NEW_COMET * new_comet_rate(comets, luminosity).value())
}

/// The rate of new comets of a halo of `comets` about a host of luminosity `luminosity`.
#[must_use]
fn new_comet_rate(comets: f64, luminosity: SolarLuminosities) -> EventsPerSecond {
    let reach = INNER_SYSTEM_AU * luminosity.value().max(0.0).sqrt();
    EventsPerSecond::per_julian_year(
        comets * NEW_COMET_SHARE_PER_YEAR * reach / NEW_COMET_PERIHELION_AU,
    )
}

/// The share of a halo that stays bound when its host's mass falls at once to `ratio` of what it
/// was (design note 11): the time-averaged share of a thermal distribution of orbits, f(e) = 2e,
/// on which a comet lies beyond 2a (1 − `ratio`), where it is still bound after the loss.
///
/// Along an orbit of eccentricity e the share of the time spent beyond x a, 1 − e < x < 1 + e, is
/// ((π − E₀) + e sin E₀) ÷ π with cos E₀ = (1 − x) ÷ e, the eccentric anomaly where r = x a; the
/// average over e is a midpoint rule of fixed length. 1 for no loss, 0 once the host has lost
/// everything, 0.71 at half.
///
/// # Examples
///
/// ```
/// use hyperion_sim::planetary::halo::surviving_share;
///
/// assert!((surviving_share(1.0) - 1.0).abs() < 1e-12);
/// assert!((surviving_share(0.5) - (0.5 + 2.0 / (3.0 * core::f64::consts::PI))).abs() < 1e-4);
/// assert!(surviving_share(0.1) < 0.2);
/// ```
#[must_use]
pub fn surviving_share(ratio: f64) -> f64 {
    if ratio >= 1.0 {
        return 1.0;
    }
    if ratio <= 0.0 {
        return 0.0;
    }
    let x = 2.0 * (1.0 - ratio);
    let step = 1.0 / f64::from(SURVIVAL_POINTS);
    (0..SURVIVAL_POINTS)
        .map(|n| {
            let e = (f64::from(n) + 0.5) * step;
            let beyond = if x <= 1.0 - e {
                1.0
            } else if x >= 1.0 + e {
                0.0
            } else {
                let cos = ((1.0 - x) / e).clamp(-1.0, 1.0);
                let e0 = math::acos(cos);
                let sin = (1.0 - cos * cos).max(0.0).sqrt();
                ((PI - e0) + e * sin) / PI
            };
            2.0 * e * beyond * step
        })
        .sum()
}

/// The draws of `system`'s halo on [`tags::COMETARY_POPULATION`]: the rank of its count, word 0.
///
/// # Panics
///
/// Never: an open uniform lies strictly inside (0, 1).
#[must_use]
pub fn count_rank(seed: Seed, system: SystemId) -> UnitUniform {
    let mut stream = Stream::open(seed, tags::COMETARY_POPULATION, ObjectKey::from(system));
    UnitUniform::new(stream.uniform_open()).expect("an open uniform lies strictly inside (0, 1)")
}

/// The halo of `system` about `host`, in the universe of `seed`, if it has one (P14.T21.d): none
/// without a [`Scatterer`], or where its outer radius falls at or below its inner one.
///
/// # Examples
///
/// The Sun's halo reaches 10⁵ au; about a star deep in the nuclear cluster, whose strip radius
/// is a few hundred au, there is none:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::id::SystemId;
/// use hyperion_sim::planetary::halo::{HaloHost, Scatterer, halo};
/// use hyperion_sim::units::{AstronomicalUnits, EarthMasses, Metres, SolarMasses};
///
/// let au = |x: f64| Metres::from(AstronomicalUnits::new(x));
/// let system = SystemId::from_raw(0x0200_0800_2000_0000)?;
/// let sun = HaloHost::new(SolarMasses::new(1.0), EarthMasses::new(32.2), au(1.39e5), None);
/// let halo_of_sun = halo(Seed::new(7), system, &sun, Scatterer::Present).expect("a halo");
/// assert!((AstronomicalUnits::from(halo_of_sun.outer_edge()).value() - 100_000.0).abs() < 1.0);
/// let stripped = HaloHost::new(SolarMasses::new(1.0), EarthMasses::new(32.2), au(300.0), None);
/// assert!(halo(Seed::new(7), system, &stripped, Scatterer::Present).is_none());
/// # Ok::<(), hyperion_sim::id::DecodeSystemIdError>(())
/// ```
#[must_use]
pub fn halo(
    seed: Seed,
    system: SystemId,
    host: &HaloHost,
    scatterer: Scatterer,
) -> Option<CometaryHalo> {
    if scatterer == Scatterer::Absent {
        return None;
    }
    let scale = math::cbrt(host.mass.value()) * METRES_PER_AU;
    let inner = HALO_INNER_RADIUS_AU * scale;
    let outer = (HALO_OUTER_RADIUS_AU * scale)
        .min(host.strip_radius.value())
        .min(
            host.companion
                .map_or(f64::INFINITY, |d| d.value() * COMPANION_SHARE),
        );
    if outer <= inner {
        return None;
    }
    let (lo, hi) = COMET_COUNT;
    let drawn = lo * math::powf(hi / lo, count_rank(seed, system).value());
    let comets = drawn * host.disc_solids.value() / REFERENCE_DISC_SOLIDS.value();
    (comets > 0.0).then_some(CometaryHalo {
        host_mass: host.mass,
        inner_edge: Metres::new(inner),
        outer_edge: Metres::new(outer),
        strip_radius: host.strip_radius,
        comets,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planetary::disc::{self, DiscDraws, DiscHost, Truncation};
    use crate::stellar::Composition;
    use crate::stellar::sse::{ZCoeffs, zams};
    use crate::units::{AstronomicalUnits, Megayears};

    const SEED: Seed = Seed::new(0x0000_0a17);

    fn au(x: f64) -> Metres {
        Metres::from(AstronomicalUnits::new(x))
    }

    fn system(n: u64) -> SystemId {
        SystemId::from_raw(0x0200_0800_2000_0000 + (n << 8)).unwrap()
    }

    fn sun(strip_au: f64) -> HaloHost {
        HaloHost::new(
            SolarMasses::new(1.0),
            REFERENCE_DISC_SOLIDS,
            au(strip_au),
            None,
        )
    }

    /// P14.T21.d (d): no halo extends beyond 0.49 of the sphere of influence (the strip radius),
    /// nor beyond a third of a wide companion's distance, at formation or after its host's mass
    /// loss; its inner radius is 2,000 au × (M★ ÷ M☉)^⅓.
    #[test]
    fn no_halo_reaches_beyond_the_strip_radius() {
        let mut lcg = hyperion_testkit::lcg::Lcg::new(0x4a10);
        let mut seen = 0;
        for n in 0..4_000 {
            let mass = 0.08 * math::powf(150.0 / 0.08, lcg.next_f64());
            let strip = au(300.0 * math::powf(3e3, lcg.next_f64()));
            let companion =
                (lcg.next_f64() < 0.3).then(|| au(1e3 * math::powf(1e3, lcg.next_f64())));
            let host = HaloHost::new(
                SolarMasses::new(mass),
                REFERENCE_DISC_SOLIDS,
                strip,
                companion,
            );
            let Some(halo) = halo(SEED, system(n), &host, Scatterer::Present) else {
                continue;
            };
            seen += 1;
            assert!(halo.outer_edge() <= strip);
            if let Some(d) = companion {
                assert!(halo.outer_edge().value() <= d.value() / 3.0 * (1.0 + 1e-12));
            }
            let inner = AstronomicalUnits::from(halo.inner_edge()).value();
            assert!((inner / (2_000.0 * math::cbrt(mass)) - 1.0).abs() < 1e-12);
            assert!(halo.inner_edge() < halo.outer_edge());
            for kept in [0.9, 0.5, 0.2] {
                let sudden = SolarMassesPerYear::new(f64::INFINITY);
                if let Some(later) = halo.at(SolarMasses::new(mass * kept), sudden) {
                    assert!(later.outer_edge() <= strip);
                    assert!(later.inner_edge() < later.outer_edge());
                    assert!(later.comets() < halo.comets());
                }
            }
        }
        assert!(seen > 1_000, "only {seen} halos");
    }

    #[test]
    fn two_calls_agree_bit_for_bit() {
        for n in 0..50 {
            let once = halo(SEED, system(n), &sun(1.39e5), Scatterer::Present);
            assert!(once.is_some());
            assert_eq!(
                once,
                halo(SEED, system(n), &sun(1.39e5), Scatterer::Present)
            );
        }
    }

    /// P14.T21.d (d): a system with no planet over 10 M⊕ beyond the snow line has no halo.
    #[test]
    fn a_system_without_a_scatterer_has_no_halo() {
        let snow = au(2.7);
        let neptune = Neighbour::new(EarthMasses::new(17.0), au(30.0), 0.01);
        let small = Neighbour::new(EarthMasses::new(9.0), au(30.0), 0.01);
        let hot = Neighbour::new(EarthMasses::new(300.0), au(0.05), 0.0);
        assert_eq!(Scatterer::among(&[small, hot], snow), Scatterer::Absent);
        assert_eq!(
            Scatterer::among(&[small, neptune], snow),
            Scatterer::Present
        );
        assert_eq!(Scatterer::Absent.or(Scatterer::Present), Scatterer::Present);
        assert_eq!(Scatterer::Absent.or(Scatterer::Absent), Scatterer::Absent);
        assert!(halo(SEED, system(1), &sun(1.39e5), Scatterer::Absent).is_none());
        assert!(halo(SEED, system(1), &sun(1.39e5), Scatterer::Present).is_some());
    }

    /// P14.T21.d: a halo whose outer radius falls below its inner one does not exist, as
    /// throughout the nuclear cluster, and a wide companion at 3,000 au leaves none.
    #[test]
    fn a_halo_cut_inside_its_inner_radius_does_not_exist() {
        assert!(halo(SEED, system(2), &sun(1_500.0), Scatterer::Present).is_none());
        let companion = HaloHost::new(
            SolarMasses::new(1.0),
            REFERENCE_DISC_SOLIDS,
            au(1.39e5),
            Some(au(3_000.0)),
        );
        assert!(halo(SEED, system(2), &companion, Scatterer::Present).is_none());
    }

    /// P14.T21.d: 10¹¹–10¹² comets about the median solar disc, in proportion to the disc's
    /// solids; and the reference solids are the median solar disc's own.
    #[test]
    fn the_comet_count_scales_with_the_disc_s_solids() {
        for n in 0..500 {
            let one = halo(SEED, system(n), &sun(1.39e5), Scatterer::Present).unwrap();
            assert!((1e11..=1e12).contains(&one.comets()));
            let rich = HaloHost::new(
                SolarMasses::new(1.0),
                REFERENCE_DISC_SOLIDS * 2.0,
                au(1.39e5),
                None,
            );
            let two = halo(SEED, system(n), &rich, Scatterer::Present).unwrap();
            assert!((two.comets() / one.comets() - 2.0).abs() < 1e-12);
        }
        let m = SolarMasses::new(1.0);
        let composition = Composition::SOLAR;
        let coeffs = ZCoeffs::new(composition.z_fit());
        let host = DiscHost::new(
            m,
            composition.fe_h(),
            zams::luminosity(m, &coeffs),
            zams::radius(m, &coeffs),
        )
        .unwrap();
        let median = disc::derive(
            &host,
            Megayears::new(3.0),
            &DiscDraws::MEDIAN,
            Truncation::NONE,
        );
        let ratio = median.solid_mass().value() / REFERENCE_DISC_SOLIDS.value();
        assert!((ratio - 1.0).abs() < 0.01, "{ratio}");
    }

    /// Design note 11: the share kept is 1 with no loss, falls with the mass lost, and is
    /// Veras et al.'s circular-orbit bound, half and more kept at half the mass.
    #[test]
    fn the_share_kept_falls_with_the_mass_lost() {
        let mut last = 1.0;
        for step in (1..100).rev() {
            let ratio = f64::from(step) / 100.0;
            let share = surviving_share(ratio);
            assert!(share <= last && share > 0.0, "{ratio}: {share}");
            last = share;
        }
        assert!((surviving_share(0.5) - (0.5 + 2.0 / (3.0 * PI))).abs() < 1e-5);
        assert!((surviving_share(1.0) - 1.0).abs() < 1e-15);
        assert!(surviving_share(0.0).abs() < 1e-15);
        assert!(surviving_share(0.05) < 0.05);
        // A share over half keeps every circular orbit: only eccentric comets near pericentre go.
        assert!(surviving_share(0.54) > 0.75);
    }

    /// P14.T21.d and ruling 84.5: the Solar System input's 7.5 × 10¹¹ comets send 11–14
    /// long-period comets a year inside 5 au, three times the new ones, scaling as √L; the rate is
    /// a value, and P14.T31 makes the events.
    #[test]
    fn the_rate_of_comets_follows_the_count() {
        let halo = CometaryHalo {
            host_mass: SolarMasses::new(1.0),
            inner_edge: au(2_000.0),
            outer_edge: au(100_000.0),
            strip_radius: au(1.39e5),
            comets: 7.5e11,
        };
        let year = crate::units::consts::SECONDS_PER_JULIAN_YEAR;
        let per_year = |l: f64| halo.comet_rate(SolarLuminosities::new(l)).value() * year;
        assert!((10.5..14.0).contains(&per_year(1.0)), "{}", per_year(1.0));
        let new = halo.new_comet_rate(SolarLuminosities::new(1.0)).value() * year;
        assert!((per_year(1.0) / new - 3.0).abs() < 1e-12);
        assert!((per_year(4.0) / per_year(1.0) - 2.0).abs() < 1e-12);
        let later = halo
            .at(
                SolarMasses::new(0.54),
                SolarMassesPerYear::new(f64::INFINITY),
            )
            .unwrap();
        assert!(
            later.new_comet_rate(SolarLuminosities::new(1.0))
                < halo.new_comet_rate(SolarLuminosities::new(1.0))
        );
    }

    /// Ruling 84.4: the loss after mass loss depends on radius through Veras et al.'s Ψ. A
    /// Sun-like AGB, 0.5 M☉ in a million years, loses nothing from a halo inside 10⁴ au and
    /// nearly the instantaneous share, 24%, at 10⁵ au; an infinite rate loses that share everywhere,
    /// and a slow one nothing.
    #[test]
    fn the_loss_after_mass_loss_depends_on_radius() {
        let sun = SolarMasses::new(1.0);
        let agb = SolarMassesPerYear::new(5e-7);
        let instantaneous = 1.0 - surviving_share(0.54);
        // 0.24, Veras et al.'s "up to 20%" in the non-adiabatic limit.
        assert!((instantaneous - 0.24).abs() < 0.01, "{instantaneous}");
        let inside = surviving_fraction(0.54, agb, sun, au(2_000.0), au(9_000.0));
        assert!((inside - 1.0).abs() < 1e-15, "{inside}");
        let outside = surviving_fraction(0.54, agb, sun, au(95_000.0), au(100_000.0));
        assert!((1.0 - outside) / instantaneous > 0.9, "{outside}");
        let whole = surviving_fraction(0.54, agb, sun, au(2_000.0), au(100_000.0));
        assert!(whole > outside && whole < inside, "{whole}");
        let sudden = SolarMassesPerYear::new(f64::INFINITY);
        let all = surviving_fraction(0.54, sudden, sun, au(2_000.0), au(100_000.0));
        assert!((all - surviving_share(0.54)).abs() < 1e-12);
        let slow = SolarMassesPerYear::new(1e-12);
        assert!(
            (surviving_fraction(0.54, slow, sun, au(2_000.0), au(100_000.0)) - 1.0).abs() < 1e-15
        );
        // Ψ at 10⁵ au for the AGB: 5 × 10⁻⁷ × 10^7.5 ÷ 2π = 2.5, near Veras et al.'s 3.0.
        let psi = 5e-7 * math::powf(1e5, 1.5) / (2.0 * PI);
        assert!((psi - 2.5).abs() < 0.1);
    }

    /// Design note 4: the count's rank is word 0 of the system's `cometary.population` stream.
    #[test]
    fn the_count_is_word_zero_of_the_system_s_stream() {
        let mut stream = Stream::open(SEED, tags::COMETARY_POPULATION, ObjectKey::from(system(5)));
        assert!((count_rank(SEED, system(5)).value() - stream.uniform_open()).abs() < 1e-300);
        let halo = halo(SEED, system(5), &sun(1.39e5), Scatterer::Present).unwrap();
        assert_eq!(halo.index().get(), 0xef00);
    }
}
