//! Binary classes from state (plan 11, P11.T5): what an interacting binary is at an age, and the
//! carved classes that the catalogue and the grid split between them.
//!
//! [`classify`] reads one [`BinaryState`] and a [`ClassContext`] (the pair's composition, draws
//! and parameters, and each neutron star's pulsar at that age) and names the pair's class
//! ([`BinaryClass`]). It is a pure function of the two: the same state and context give the same
//! class whatever was asked before. [`BinaryTimeline::class_at`] builds the context from the
//! timeline and classifies its state. The rules, in the order they are tried, so that a state
//! that meets several takes the first:
//!
//! 1. **Contact**: both stars fill their lobes.
//! 2. **Roche-lobe overflow**, by what the accretor is:
//!    - a white dwarf fed helium is an AM Canum Venaticorum star; fed by a giant, a symbiotic star;
//!      a carbon–oxygen dwarf fed hydrogen at the steady-burning rate or faster, a Type Ia
//!      progenitor (the single-degenerate channel); otherwise a cataclysmic variable, magnetic by
//!      the white dwarf's own draw ([`MAGNETIC_CV_SHARE`]), a nova-like above the disc-instability
//!      line and a dwarf nova below it ([`cv_critical_rate`]);
//!    - a neutron star or black hole is a supergiant high-mass X-ray binary for a donor of
//!      [`HMXB_MIN_DONOR_MASS`] or more, and a low-mass one below, persistent above the
//!      irradiated disc's instability line and transient below it ([`xrb_critical_rate`]);
//!    - a main-sequence star fed by a lighter subgiant or giant is an Algol.
//! 3. **A bound pair**: two neutron stars are a double neutron star; two white dwarfs that merge
//!    within [`AGE_OF_UNIVERSE`], not both of helium, are a Type Ia progenitor if their total
//!    exceeds the Chandrasekhar mass or the heavier is a carbon–oxygen or oxygen–neon dwarf of
//!    [`MIN_DETONATABLE_MASS`] or more (the double-degenerate channel, ruling 129.1), and a double
//!    white dwarf otherwise; a neutron star or black hole beside a Be star (plan 06's) of
//!    [`HMXB_MIN_DONOR_MASS`] or more on an orbit of [`BEX_MAX_PERIOD`] or less is a Be/X-ray
//!    binary; one fed by the wind of a star of [`HMXB_MIN_DONOR_MASS`] or more above
//!    [`XRB_MIN_LUMINOSITY`] is a supergiant X-ray binary, and one fed by a lighter giant's wind
//!    above [`SYMBIOTIC_XRB_MIN_LUMINOSITY`] a symbiotic X-ray binary, a low-mass kind (ruling
//!    129.3); a white dwarf fed by a giant's wind above [`SYMBIOTIC_MIN_LUMINOSITY`] is a symbiotic
//!    star, however wide the orbit (design note 7).
//! 4. **One star** of the pair: a recycled radio pulsar faster than 30 ms is a millisecond pulsar
//!    (`recycling.rs`); the helium giant under the Chandrasekhar mass that a helium and a
//!    carbon–oxygen (or oxygen–neon) white dwarf merge into is an R Coronae Borealis star; a helium
//!    main-sequence star of about half a solar mass is a hot subdwarf; a main-sequence star above
//!    its population's turn-off mass at the pair's age is a blue straggler.
//!
//! [`carved_class`] is the one deterministic test that the grid and the catalogue both apply
//! (design notes 8 and 10): a stellar or neutron-star merger at an age inside the source horizon,
//! or an X-ray binary or accreting white dwarf at any age inside it, the white dwarfs split by
//! their nova recurrence period against L ([`nova_recurrence_period`]). The fifth carved case,
//! exploded as a Type Ia, is P11.T6's.
//!
//! Every threshold is a named constant with its source, re-checked for P11.T5; those marked
//! provisional rest on a secondary source or on no class definition, and plan 11's Risks lists
//! them.

use crate::math;
use crate::orbit::{peters_merger_time, roche_lobe_radius};
use crate::rng::Threshold;
use crate::stellar::classify::{SpectralLetter, subtype_from_teff};
use crate::stellar::draws::StarDraws;
use crate::stellar::remnant::RemnantKind;
use crate::stellar::remnant::structure::CHANDRASEKHAR_MASS;
use crate::stellar::sse::turn_off_mass;
use crate::stellar::{Composition, Phase, StarState, rotation};
use crate::time::{CLOCK_WINDOW_H, LIGHT_CROSSING_L};
use crate::units::consts::{
    GM_SUN, SECONDS_PER_JULIAN_YEAR, SOLAR_LUMINOSITY_W, SOLAR_MASS_KG, SOLAR_RADIUS_M,
};
use crate::units::{
    Days, Metres, SolarLuminosities, SolarMasses, SolarMassesPerYear, Watts, Years,
};

use super::params::BinaryParams;
use super::recycling::{PulsarAt, is_millisecond_pulsar, pulsar_at};
use super::rlof::NOVA_RATE;
use super::star::{G, Kind, Surface};
use super::timeline::{BinaryState, BinaryTimeline, Component, SegmentKind};

/// The kinds of cataclysmic variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CvKind {
    /// A dwarf nova: fed below the disc-instability line, so its disc erupts.
    DwarfNova,
    /// A nova-like: fed above the line, so its disc stays hot.
    NovaLike,
    /// A polar or intermediate polar: the white dwarf's field channels the flow.
    Magnetic,
    /// An AM Canum Venaticorum star: a white dwarf fed helium.
    AmCvn,
}

/// The kinds of low-mass X-ray binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum XrbKind {
    /// Fed through the inner Lagrangian point above the irradiated disc's instability line.
    Persistent,
    /// Fed below it: outbursts between long quiescence.
    Transient,
    /// A symbiotic X-ray binary: a neutron star or black hole fed by a low-mass giant's wind,
    /// which Avakyan et al. (2023, A&A 675, A199, section 3) count as a subclass of the low-mass
    /// X-ray binaries (ruling 129.3).
    Symbiotic,
}

/// The kinds of high-mass X-ray binary, as P11.T8.b's catalogue splits them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HmxbKind {
    /// A Be/X-ray binary: the companion is plan 06's Be star.
    BeX,
    /// A supergiant X-ray binary: fed by a massive star's wind or through its inner Lagrangian
    /// point, and persistent.
    Supergiant,
}

/// What an interacting binary is at an age (plan 11's Provides).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum BinaryClass {
    /// None of the classes below.
    #[default]
    None,
    /// A lighter subgiant or giant filling its lobe onto a main-sequence star.
    Algol,
    /// Both stars fill their lobes.
    Contact,
    /// A main-sequence star above its population's turn-off mass at its age.
    BlueStraggler,
    /// A stripped helium-burning core of about half a solar mass.
    HotSubdwarf,
    /// The hydrogen-deficient giant a white-dwarf merger makes.
    RCoronaeBorealis,
    /// A white dwarf fed by a giant.
    Symbiotic,
    /// A white dwarf fed by a Roche-lobe-filling star.
    CataclysmicVariable(CvKind),
    /// A neutron star or black hole fed by a low-mass star.
    LowMassXrayBinary(XrbKind),
    /// A neutron star or black hole fed by a massive star.
    HighMassXrayBinary(HmxbKind),
    /// A recycled radio pulsar spinning faster than 30 ms.
    MillisecondPulsar,
    /// Two bound neutron stars.
    DoubleNeutronStar,
    /// Two bound white dwarfs that are not a Type Ia progenitor.
    DoubleWhiteDwarf,
    /// A candidate Type Ia progenitor: two white dwarfs, not both of helium, that merge within the
    /// age of the universe and are heavy enough to detonate, or a carbon–oxygen white dwarf
    /// growing by steady hydrogen burning.
    TypeIaProgenitor,
}

impl BinaryClass {
    /// Every class and kind, [`BinaryClass::None`] first.
    pub const ALL: [Self; 20] = [
        Self::None,
        Self::Algol,
        Self::Contact,
        Self::BlueStraggler,
        Self::HotSubdwarf,
        Self::RCoronaeBorealis,
        Self::Symbiotic,
        Self::CataclysmicVariable(CvKind::DwarfNova),
        Self::CataclysmicVariable(CvKind::NovaLike),
        Self::CataclysmicVariable(CvKind::Magnetic),
        Self::CataclysmicVariable(CvKind::AmCvn),
        Self::LowMassXrayBinary(XrbKind::Persistent),
        Self::LowMassXrayBinary(XrbKind::Transient),
        Self::LowMassXrayBinary(XrbKind::Symbiotic),
        Self::HighMassXrayBinary(HmxbKind::BeX),
        Self::HighMassXrayBinary(HmxbKind::Supergiant),
        Self::MillisecondPulsar,
        Self::DoubleNeutronStar,
        Self::DoubleWhiteDwarf,
        Self::TypeIaProgenitor,
    ];

    /// Whether this is an X-ray binary, of low or high mass.
    #[must_use]
    pub const fn is_xray_binary(self) -> bool {
        matches!(
            self,
            Self::LowMassXrayBinary(_) | Self::HighMassXrayBinary(_)
        )
    }
}

/// The carved classes: the binary classes the catalogue holds and the grid refuses (plan 11,
/// design note 8), in plan 09's registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CarvedClass {
    /// An accreting white dwarf whose novae recur within L (design note 12).
    AccretingWdFast,
    /// An accreting white dwarf whose novae recur more slowly than L, or not at all.
    AccretingWdSlow,
    /// An X-ray binary.
    XrayBinary,
    /// Two stars, at least one of them living, that merge.
    StellarMerger,
    /// A neutron star that merges with a neutron star or a black hole.
    NeutronStarMerger,
}

/// The share of cataclysmic variables whose white dwarf is magnetic: 15 of 42, the 12 polars and
/// 3 intermediate polars of Pala et al.'s (2020, MNRAS 494, 3799, table 1) 150 pc sample, their
/// "36 per cent", uncorrected for its 77% completeness. The white dwarf's own `star.magnetism` mark
/// decides it, so that a white dwarf whose progenitor had plan 06's fossil field, whose mark lies
/// lower still, is always magnetic.
pub const MAGNETIC_CV_SHARE: f64 = 15.0 / 42.0;

/// The least donor mass of a high-mass X-ray binary, M☉: 8, Fortin et al.'s (2023, A&A 671, A149,
/// section 1) "M ≥ 8 M☉". Plan 11 has two X-ray classes, so the intermediate-mass systems (Her
/// X-1's 2 M☉ donor) are low-mass ones, as Avakyan et al.'s (2023, A&A 675, A199, section 2)
/// catalogue counts them.
pub const HMXB_MIN_DONOR_MASS: SolarMasses = SolarMasses::new(8.0);

/// The least accretion luminosity at which a neutron star or black hole fed by the wind of a
/// star of [`HMXB_MIN_DONOR_MASS`] or more is a supergiant X-ray binary, W: 10³⁵ erg s⁻¹, the
/// floor of the supergiant systems' luminosities, (0.7–1) × 10³⁵ erg s⁻¹ (Lutovinov et al. 2013,
/// MNRAS 431, 327, section 4.1; ruling 129.5). It applies to wind-fed systems alone: a pair in
/// Roche-lobe overflow onto a compact star is an X-ray binary in quiescence too.
pub const XRB_MIN_LUMINOSITY: Watts = Watts::new(1.0e28);

/// The least accretion luminosity at which a neutron star or black hole fed by the wind of a giant
/// lighter than [`HMXB_MIN_DONOR_MASS`] is a symbiotic X-ray binary, W: 10³² erg s⁻¹, the faint end
/// of the known systems (Yungelson et al. 2019, A&A 632, A3, table 1 and section 4.1; ruling
/// 129.3).
pub const SYMBIOTIC_XRB_MIN_LUMINOSITY: Watts = Watts::new(1.0e25);

/// The longest orbital period of a Be/X-ray binary, days: 1,000 (provisional; ruling 129.2). The
/// confirmed systems lie at 12.7–330 d with O8–B2 companions (Reig 2011, Ap&SS 332, 1, table 1);
/// PSR B1259−63, beyond 1,000 d, is not one. No eccentricity condition applies (KS 1947+300's is
/// 0.03).
pub const BEX_MAX_PERIOD: Days = Days::new(1_000.0);

/// The least mass at which the heavier of two merging carbon–oxygen or oxygen–neon white dwarfs
/// makes the pair a Type Ia progenitor below the Chandrasekhar mass, M☉: 0.85, "the minimum
/// detonatable WD mass may be ≃ 0.85 M⊙" (Shen et al. 2018, ApJ 865, 15, abstract and section
/// 4.1; ruling 129.1). P11.T6's pool still holds every white-dwarf merger, since the Type Ia
/// share η ≈ 1/6 is of all of them (Maoz, Hallakoun and Badenes 2018).
pub const MIN_DETONATABLE_MASS: SolarMasses = SolarMasses::new(0.85);

/// The least accretion luminosity at which a white dwarf fed by a giant's wind is a symbiotic
/// star: 10 L☉, the faint end of the accretion-powered high states, "10–1000 L⊙" (Mikołajewska
/// 2011, arXiv:1011.5657, section 4; ruling 129.5). Steady hydrogen burning would add to it only
/// above [`STEADY_BURNING_RATE`], where accretion onto any white dwarf already gives more than a
/// hundred L☉, so accretion alone is compared.
pub const SYMBIOTIC_MIN_LUMINOSITY: SolarLuminosities = SolarLuminosities::new(10.0);

/// The masses of a hot subdwarf, M☉: a helium main-sequence star of 0.32–0.8 M☉, about the
/// canonical 0.47 of a degenerate ignition (Heber 2016, PASP 128, 082001, section 3). The lower
/// edge is the least core that ignites helium, 0.319 M☉ from a 2.05 M☉ progenitor (Han et al.
/// 2002, MNRAS 336, 449, table 1); the upper holds the merger channel's He-sdO stars (Heber 2016,
/// section 8; above about 1 M☉ a helium star is a stripped star, not a subdwarf).
pub const HOT_SUBDWARF_MASSES: (SolarMasses, SolarMasses) =
    (SolarMasses::new(0.32), SolarMasses::new(0.8));

/// The age of the universe, years: 13.787 Gyr (Planck Collaboration 2020, A&A 641, A6, table 2),
/// within which two white dwarfs must merge to be a Type Ia progenitor, "within a Hubble time" as
/// Napiwotzki et al. (2020, A&A 638, A131, sections 1 and 5) put it; the pair must also exceed
/// the Chandrasekhar mass or hold a dwarf of [`MIN_DETONATABLE_MASS`].
pub const AGE_OF_UNIVERSE: Years = crate::planetary::context::UNIVERSE_AGE;

/// The rate of hydrogen onto a white dwarf from which it burns steadily and grows, M☉ yr⁻¹:
/// 1.03 × 10⁻⁷, below which it erupts in novae (Hurley, Tout and Pols 2002, section 2.6.6): the
/// engine's own line, shared. The published line rises with the dwarf's mass, from 2.5 × 10⁻⁸ at
/// 0.51 M☉ to 3.8 × 10⁻⁷ at 1.34 M☉ (Wolf et al. 2013, ApJ 777, 136, table 1); the classes follow
/// the engine.
pub(crate) const STEADY_BURNING_RATE: SolarMassesPerYear = SolarMassesPerYear::new(NOVA_RATE);

/// The outer radius of an accretion disc as a share of its accretor's Roche lobe, where the lobe
/// is larger than the donor's: 0.9 (a choice of the research behind P11.T5, not a published
/// prescription; it agrees with Paczyński's radius to about 10% for q ≤ 0.5). A lighter donor's
/// disc takes Paczyński's radius instead ([`disc_radius`]).
pub(crate) const DISC_RADIUS_SHARE: f64 = 0.9;

/// Grams per second in one solar mass per year.
const GRAMS_PER_SECOND_PER_MSUN_PER_YEAR: f64 = SOLAR_MASS_KG * 1.0e3 / SECONDS_PER_JULIAN_YEAR;

/// The disc-instability line of a cataclysmic variable, M☉ yr⁻¹: the least transfer rate that
/// keeps an outer disc of radius `r_out` about a white dwarf of `mass` hot, Ṁ⁺ = 8.07 × 10¹⁵
/// α₀.₁^−0.01 R₁₀^2.64 M₁^−0.89 g s⁻¹ for a hydrogen disc (Lasota, Dubus and Kruk 2008, A&A 486,
/// 523, equation A.1; Coriat, Fender and Dubus 2012, MNRAS 424, 1991, equation 1), at the hot
/// branch's α = 0.1. Below it the disc erupts: a dwarf nova.
#[must_use]
pub(crate) fn cv_critical_rate(mass: SolarMasses, r_out: Metres) -> SolarMassesPerYear {
    let r10 = r_out.value() * 100.0 / 1.0e10;
    let g_s = 8.07e15
        * math::powf_positive(r10, 2.64)
        * math::powf_positive(mass.value().max(1e-6), -0.89);
    SolarMassesPerYear::new(g_s / GRAMS_PER_SECOND_PER_MSUN_PER_YEAR)
}

/// The disc-instability line of an X-ray-irradiated disc, M☉ yr⁻¹: Ṁ⁺ = 9.5 × 10¹⁴ C₋₃^−0.36
/// α₀.₁^(0.04 + 0.01 log C₋₃) R₁₀^(2.39 − 0.10 log C₋₃) M₁^(−0.64 + 0.08 log C₋₃) g s⁻¹ (Lasota,
/// Dubus and Kruk 2008, equation A.2; Coriat, Fender and Dubus 2012, equation 2), at α = 0.1 and
/// their irradiation constant C = 10⁻³ (LDK08 section 3), where it is 9.5 × 10¹⁴ R₁₀^2.39 M₁^−0.64,
/// about an accretor of `mass` with an outer disc radius `r_out`. Below it an X-ray binary is
/// transient.
#[must_use]
pub(crate) fn xrb_critical_rate(mass: SolarMasses, r_out: Metres) -> SolarMassesPerYear {
    let r10 = r_out.value() * 100.0 / 1.0e10;
    let g_s = 9.5e14
        * math::powf_positive(r10, 2.39)
        * math::powf_positive(mass.value().max(1e-6), -0.64);
    SolarMassesPerYear::new(g_s / GRAMS_PER_SECOND_PER_MSUN_PER_YEAR)
}

/// The envelope mass at which hydrogen accreted at `rate` onto a white dwarf of `mass` ignites in a
/// nova, M☉: log₁₀ `M_ign` = −3.728 − 0.2934 (log₁₀ Ṁ + 9) + 1.920 log₁₀(1.44 − M), a least-squares
/// fit to Yaron et al.'s (2005, ApJ 623, 398) table 2 at a core of 10⁷ K, 0.65–1.40 M☉ and
/// 10⁻¹¹–10⁻⁷ M☉ yr⁻¹ (20 models; 0.125 dex rms, 0.251 dex at worst, confirmed by ruling 129.5),
/// made by the research behind P11.T5, since no published closed form covers that range. The cold
/// core is Townsley and Bildsten's (2004, ApJ 600, 390) equilibrium, 5.5 × 10⁶ K. Its recurrence
/// times run 1.2–2.3 times Wolf et al.'s (2013, table 2).
///
/// The mass is held to 0.6–1.40 M☉, so that a helium dwarf takes the lightest carbon–oxygen
/// dwarf's ignition mass, and the rate to the fit's own 10⁻¹¹–10⁻⁷: extrapolated to 10⁻¹² it runs
/// 0.26–0.50 dex above Yaron et al.'s rows there (ruling 129.5; P11.T8.a re-checks it).
#[must_use]
pub(crate) fn nova_ignition_mass(mass: SolarMasses, rate: SolarMassesPerYear) -> SolarMasses {
    let m = mass.value().clamp(0.6, 1.40);
    let log_rate = math::log10(rate.value().max(1.0e-11)).min(-7.0);
    let log_mass = -3.728 - 0.2934 * (log_rate + 9.0) + 1.920 * math::log10(1.44 - m);
    SolarMasses::new(math::exp10(log_mass))
}

/// The recurrence period of the novae of a white dwarf of `mass` fed at `rate`: its ignition mass
/// over the rate, infinite for no accretion or for steady burning ([`STEADY_BURNING_RATE`]).
#[must_use]
pub(crate) fn nova_recurrence_period(mass: SolarMasses, rate: SolarMassesPerYear) -> Years {
    let r = rate.value();
    if r <= 0.0 || rate >= STEADY_BURNING_RATE {
        return Years::new(f64::INFINITY);
    }
    Years::new(nova_ignition_mass(mass, rate).value() / r)
}

/// What a class is read from beyond the state: the pair's composition, each star's draws, the
/// engine's parameters (for the winds' speed and focusing), and each neutron star's pulsar at the
/// state's age.
#[derive(Debug, Clone, Copy)]
pub struct ClassContext<'a> {
    composition: &'a Composition,
    draws: [&'a StarDraws; 2],
    params: &'a BinaryParams,
    pulsars: [Option<PulsarAt>; 2],
    merged_from: Option<[Phase; 2]>,
}

impl<'a> ClassContext<'a> {
    /// The context of a pair of `composition`, `draws` and `params`, whose stars are the pulsars
    /// `pulsars` (`None` for a star that is not a neutron star, or one with no recorded birth),
    /// and which, if it has merged, merged from stars of the phases `merged_from`.
    #[must_use]
    pub(crate) const fn new(
        composition: &'a Composition,
        draws: [&'a StarDraws; 2],
        params: &'a BinaryParams,
        pulsars: [Option<PulsarAt>; 2],
        merged_from: Option<[Phase; 2]>,
    ) -> Self {
        Self {
            composition,
            draws,
            params,
            pulsars,
            merged_from,
        }
    }

    /// The pair's composition.
    #[must_use]
    pub const fn composition(&self) -> &Composition {
        self.composition
    }

    /// Component `c`'s pulsar, if it is one.
    #[must_use]
    pub const fn pulsar(&self, c: Component) -> Option<&PulsarAt> {
        self.pulsars[c.index()].as_ref()
    }
}

/// A white dwarf's or compact star's feeding: which component accretes, and at what rate.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Feeding {
    accretor: usize,
    rate: SolarMassesPerYear,
}

/// A class and, for an accreting compact star, what feeds it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Classified {
    class: BinaryClass,
    feeding: Option<Feeding>,
}

impl Classified {
    const NONE: Self = Self {
        class: BinaryClass::None,
        feeding: None,
    };

    const fn plain(class: BinaryClass) -> Self {
        Self {
            class,
            feeding: None,
        }
    }

    const fn fed(class: BinaryClass, accretor: usize, rate: SolarMassesPerYear) -> Self {
        Self {
            class,
            feeding: Some(Feeding { accretor, rate }),
        }
    }
}

/// The class of a binary in `state` with `context` (plan 11's Provides): see the module's
/// documentation for the rules and their order.
///
/// # Examples
///
/// BSE's cataclysmic variable (Hurley, Tout and Pols 2002, section 3.2) is one while its
/// main-sequence star fills its lobe onto the white dwarf:
///
/// ```
/// use hyperion_sim::orbit::{Eccentricity, KeplerElements, Orientation};
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::stellar::binary::{BinaryClass, BinaryInput, evolve};
/// use hyperion_sim::stellar::draws::StarDraws;
/// use hyperion_sim::units::{GravitationalParameter, Radians, Seconds, SolarMasses, Years};
///
/// let orbit = KeplerElements::from_period(
///     Seconds::new(630.0 * 86_400.0),
///     GravitationalParameter::from_solar_masses(SolarMasses::new(7.3)),
///     Eccentricity::CIRCULAR,
///     Orientation::new(Radians::new(0.3), Radians::new(0.0), Radians::new(0.0))?,
///     Radians::new(0.0),
/// )?;
/// let pair = BinaryInput::new(
///     SolarMasses::new(6.0),
///     SolarMasses::new(1.3),
///     Composition::SOLAR,
///     orbit,
///     [StarDraws::median(), StarDraws::median()],
///     Years::new(1.0e10),
/// )?;
/// let timeline = evolve(&pair, Years::new(1.5e10));
/// let classes: Vec<BinaryClass> = timeline
///     .segments()
///     .iter()
///     .map(|s| timeline.class_at(s.start()))
///     .collect();
/// assert!(classes.iter().any(|c| matches!(c, BinaryClass::CataclysmicVariable(_))));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn classify(state: &BinaryState, context: &ClassContext<'_>) -> BinaryClass {
    classified(state, context).class
}

/// [`classify`], with what feeds an accreting compact star.
#[must_use]
fn classified(state: &BinaryState, ctx: &ClassContext<'_>) -> Classified {
    match state.kind() {
        SegmentKind::Contact => return Classified::plain(BinaryClass::Contact),
        SegmentKind::StableTransfer { donor } => {
            if let Some(class) = transfer_class(state, ctx, donor.index()) {
                return class;
            }
        }
        SegmentKind::Detached
        | SegmentKind::CommonEnvelope
        | SegmentKind::Merged
        | SegmentKind::Disrupted { .. } => {}
    }
    if let Some(class) = pair_class(state, ctx) {
        return class;
    }
    star_class(state, ctx).map_or(Classified::NONE, Classified::plain)
}

/// Whether `phase` is a white dwarf's.
#[must_use]
const fn is_white_dwarf(phase: Phase) -> bool {
    matches!(
        phase,
        Phase::HeliumWhiteDwarf | Phase::CarbonOxygenWhiteDwarf | Phase::OxygenNeonWhiteDwarf
    )
}

/// Whether `phase` is a neutron star's or a black hole's.
#[must_use]
const fn is_neutron_star_or_black_hole(phase: Phase) -> bool {
    matches!(phase, Phase::NeutronStar | Phase::BlackHole)
}

/// Whether `phase` is a giant's: the first giant branch, core helium burning, the asymptotic
/// giant branch.
#[must_use]
const fn is_giant(phase: Phase) -> bool {
    matches!(
        phase,
        Phase::FirstGiantBranch
            | Phase::CoreHeliumBurning
            | Phase::EarlyAgb
            | Phase::ThermallyPulsingAgb
    )
}

/// The class of a pair in stable transfer from `d`, if its accretor makes it one.
#[must_use]
fn transfer_class(state: &BinaryState, ctx: &ClassContext<'_>, d: usize) -> Option<Classified> {
    let a = 1 - d;
    let stars = state.stars();
    let (donor, accretor) = (&stars[d], &stars[a]);
    let rate = SolarMassesPerYear::new(
        state
            .transfer_rate()
            .map_or(0.0, SolarMassesPerYear::value)
            .max(0.0),
    );
    let donor_surface = Kind::of(donor.phase(), donor.mass().value()).surface();
    let accretor_phase = accretor.phase();
    if is_white_dwarf(accretor_phase) {
        let class = match donor_surface {
            Surface::Helium => BinaryClass::CataclysmicVariable(CvKind::AmCvn),
            Surface::Carbon => return None,
            Surface::Hydrogen => {
                if is_giant(donor.phase()) {
                    BinaryClass::Symbiotic
                } else if accretor_phase == Phase::CarbonOxygenWhiteDwarf
                    && rate >= STEADY_BURNING_RATE
                {
                    BinaryClass::TypeIaProgenitor
                } else {
                    BinaryClass::CataclysmicVariable(cv_kind(state, ctx, d, rate))
                }
            }
        };
        return Some(Classified::fed(class, a, rate));
    }
    if is_neutron_star_or_black_hole(accretor_phase) {
        let class = if donor.mass() >= HMXB_MIN_DONOR_MASS {
            BinaryClass::HighMassXrayBinary(HmxbKind::Supergiant)
        } else {
            let persistent = rate >= xrb_critical_rate(accretor.mass(), disc_radius(state, a));
            BinaryClass::LowMassXrayBinary(if persistent {
                XrbKind::Persistent
            } else {
                XrbKind::Transient
            })
        };
        return Some(Classified::fed(class, a, rate));
    }
    let algol = accretor_phase == Phase::MainSequence
        && donor.mass() < accretor.mass()
        && matches!(
            donor.phase(),
            Phase::MainSequence | Phase::HertzsprungGap | Phase::FirstGiantBranch
        );
    algol.then_some(Classified::plain(BinaryClass::Algol))
}

/// The kind of a cataclysmic variable whose donor `d` feeds the white dwarf at `rate`.
#[must_use]
fn cv_kind(
    state: &BinaryState,
    ctx: &ClassContext<'_>,
    d: usize,
    rate: SolarMassesPerYear,
) -> CvKind {
    let a = 1 - d;
    if ctx.draws[a]
        .magnetism()
        .is_below(Threshold::from_probability(MAGNETIC_CV_SHARE))
    {
        return CvKind::Magnetic;
    }
    let line = cv_critical_rate(state.stars()[a].mass(), disc_radius(state, a));
    if rate >= line {
        CvKind::NovaLike
    } else {
        CvKind::DwarfNova
    }
}

/// The outer radius of the disc about component `a`, zero for a pair with no orbit: Paczyński's
/// (1977, ApJ 216, 822) largest disc, 0.60 a ÷ (1 + q) with q the donor's mass over the accretor's
/// (as Lasota, Dubus and Kruk 2008, equation 8, take it, for 0.03 < q < 1), and for a donor at
/// least as heavy as the accretor, where that fit does not hold, [`DISC_RADIUS_SHARE`] of the
/// accretor's Roche lobe (Eggleton 1983).
#[must_use]
fn disc_radius(state: &BinaryState, a: usize) -> Metres {
    let Some(orbit) = state.orbit() else {
        return Metres::new(0.0);
    };
    let stars = state.stars();
    let (ma, md) = (stars[a].mass().value(), stars[1 - a].mass().value());
    if !(ma > 0.0 && md > 0.0) {
        return Metres::new(0.0);
    }
    let separation = orbit.semi_major_axis();
    let q = md / ma;
    if q < 1.0 {
        return Metres::new(0.60 * separation.value() / (1.0 + q));
    }
    let lobe = roche_lobe_radius(ma / md, separation);
    Metres::new(DISC_RADIUS_SHARE * lobe.value())
}

/// The class of a bound pair, if it is a double degenerate or a wind-fed compact star.
#[must_use]
fn pair_class(state: &BinaryState, ctx: &ClassContext<'_>) -> Option<Classified> {
    let orbit = state.orbit()?;
    let [s0, s1] = state.stars();
    let (p0, p1) = (s0.phase(), s1.phase());
    if p0 == Phase::NeutronStar && p1 == Phase::NeutronStar {
        return Some(Classified::plain(BinaryClass::DoubleNeutronStar));
    }
    if is_white_dwarf(p0) && is_white_dwarf(p1) {
        let a = orbit.semi_major_axis();
        let carbon = p0 != Phase::HeliumWhiteDwarf || p1 != Phase::HeliumWhiteDwarf;
        let heavier = if s0.mass() >= s1.mass() { s0 } else { s1 };
        let detonatable = s0.mass().value() + s1.mass().value() > CHANDRASEKHAR_MASS.value()
            || (heavier.phase() != Phase::HeliumWhiteDwarf
                && heavier.mass() >= MIN_DETONATABLE_MASS);
        let progenitor = carbon
            && detonatable
            && s0.mass().value() > 0.0
            && s1.mass().value() > 0.0
            && a.value() > 0.0
            && peters_merger_time(s0.mass(), s1.mass(), a, orbit.eccentricity()) <= AGE_OF_UNIVERSE;
        return Some(Classified::plain(if progenitor {
            BinaryClass::TypeIaProgenitor
        } else {
            BinaryClass::DoubleWhiteDwarf
        }));
    }
    for c in 0..2 {
        let d = 1 - c;
        let (compact, donor) = (&state.stars()[c], &state.stars()[d]);
        let cp = compact.phase();
        if !(is_white_dwarf(cp) || is_neutron_star_or_black_hole(cp)) || !donor.phase().is_living()
        {
            continue;
        }
        let period = Days::from(orbit.period());
        if is_neutron_star_or_black_hole(cp)
            && donor.mass() >= HMXB_MIN_DONOR_MASS
            && period <= BEX_MAX_PERIOD
            && is_be_star(donor, ctx.composition, ctx.draws[d])
        {
            return Some(Classified::plain(BinaryClass::HighMassXrayBinary(
                HmxbKind::BeX,
            )));
        }
        let rate = wind_accretion_rate(state, ctx.params, d);
        let luminosity = accretion_luminosity(compact, rate).value();
        let massive = donor.mass() >= HMXB_MIN_DONOR_MASS;
        if is_neutron_star_or_black_hole(cp) && massive {
            if luminosity >= XRB_MIN_LUMINOSITY.value() {
                return Some(Classified::fed(
                    BinaryClass::HighMassXrayBinary(HmxbKind::Supergiant),
                    c,
                    rate,
                ));
            }
            continue;
        }
        if !is_giant(donor.phase()) {
            continue;
        }
        if is_neutron_star_or_black_hole(cp) && luminosity >= SYMBIOTIC_XRB_MIN_LUMINOSITY.value() {
            return Some(Classified::fed(
                BinaryClass::LowMassXrayBinary(XrbKind::Symbiotic),
                c,
                rate,
            ));
        }
        if is_white_dwarf(cp) && luminosity >= SYMBIOTIC_MIN_LUMINOSITY.value() * SOLAR_LUMINOSITY_W
        {
            return Some(Classified::fed(BinaryClass::Symbiotic, c, rate));
        }
    }
    None
}

/// Whether `star` is plan 06's Be star: a B-type main-sequence star without a fossil field (which
/// makes it a Bp star) spinning above [`BE_CRITICAL_FRACTION`](rotation::BE_CRITICAL_FRACTION)
/// of its critical speed, by its own rotation draw (P06.T25's `rotation_class`).
#[must_use]
fn is_be_star(star: &StarState, composition: &Composition, draws: &StarDraws) -> bool {
    if star.phase() != Phase::MainSequence {
        return false;
    }
    let b_type = subtype_from_teff(star.effective_temperature())
        .is_some_and(|code| code.letter() == SpectralLetter::B);
    b_type
        && rotation::fossil_field(star, draws).is_none()
        && rotation::rotation(star, composition, draws, None)
            .is_some_and(|spin| spin.critical_fraction() > rotation::BE_CRITICAL_FRACTION)
}

/// The rate at which component `1 − d` takes `d`'s wind, M☉ yr⁻¹: Bondi and Hoyle's share as the
/// engine takes it (Hurley, Tout and Pols 2002, equation 6 with equation 9's wind speed), no more
/// than [`BinaryParams::max_wind_accretion`] of the wind; zero without an orbit or a wind.
#[must_use]
fn wind_accretion_rate(state: &BinaryState, params: &BinaryParams, d: usize) -> SolarMassesPerYear {
    let Some(orbit) = state.orbit() else {
        return SolarMassesPerYear::ZERO;
    };
    let stars = state.stars();
    let (donor, accretor) = (&stars[d], &stars[1 - d]);
    let wind = donor.mass_loss_rate().value();
    let (m_donor, m_accretor, radius) = (
        donor.mass().value(),
        accretor.mass().value(),
        donor.radius().value(),
    );
    if !(wind > 0.0 && m_donor > 0.0 && m_accretor > 0.0 && radius > 0.0) {
        return SolarMassesPerYear::ZERO;
    }
    let separation = orbit.semi_major_axis().value() / SOLAR_RADIUS_M;
    let ecc = orbit.eccentricity().value();
    let beta = params
        .wind_speed_factor
        .of(Kind::of(donor.phase(), m_donor), m_donor);
    let v_wind2 = 2.0 * beta * G * m_donor / radius;
    let v_orb2 = G * (m_donor + m_accretor) / separation;
    let slowing = 1.0 + v_orb2 / v_wind2;
    let focus = G * m_accretor / v_wind2;
    let rate = params.bondi_hoyle * focus * focus
        / (2.0 * separation * separation)
        / (slowing * slowing.sqrt())
        / (1.0 - ecc * ecc).sqrt()
        * wind;
    SolarMassesPerYear::new(rate.min(params.max_wind_accretion * wind))
}

/// The luminosity of accretion at `rate` onto `star`: G M Ṁ ÷ R, zero for a star of no radius.
#[must_use]
fn accretion_luminosity(star: &StarState, rate: SolarMassesPerYear) -> Watts {
    let r = star.radius().value() * SOLAR_RADIUS_M;
    let rate = rate.value();
    if !(r > 0.0 && rate > 0.0) {
        return Watts::new(0.0);
    }
    Watts::new(GM_SUN * star.mass().value() * rate * SOLAR_MASS_KG / SECONDS_PER_JULIAN_YEAR / r)
}

/// The class one star of the pair gives it, if any.
#[must_use]
fn star_class(state: &BinaryState, ctx: &ClassContext<'_>) -> Option<BinaryClass> {
    let stars = state.stars();
    let recycled_msp = ctx
        .pulsars
        .iter()
        .flatten()
        .any(|p| p.recycled() && is_millisecond_pulsar(p.state()));
    if recycled_msp {
        return Some(BinaryClass::MillisecondPulsar);
    }
    if state.kind() == SegmentKind::Merged
        && let Some([p0, p1]) = ctx.merged_from
    {
        let product = &stars[0];
        let carbon = |p: Phase| {
            matches!(
                p,
                Phase::CarbonOxygenWhiteDwarf | Phase::OxygenNeonWhiteDwarf
            )
        };
        let white_dwarfs = (p0 == Phase::HeliumWhiteDwarf && carbon(p1))
            || (p1 == Phase::HeliumWhiteDwarf && carbon(p0));
        if white_dwarfs
            && matches!(
                product.phase(),
                Phase::HeliumHertzsprungGap | Phase::HeliumGiantBranch
            )
            && product.mass() <= CHANDRASEKHAR_MASS
        {
            return Some(BinaryClass::RCoronaeBorealis);
        }
    }
    let (low, high) = HOT_SUBDWARF_MASSES;
    if stars
        .iter()
        .any(|s| s.phase() == Phase::HeliumMainSequence && s.mass() >= low && s.mass() <= high)
    {
        return Some(BinaryClass::HotSubdwarf);
    }
    let mut turn_off: Option<SolarMasses> = None;
    for s in stars {
        if s.phase() != Phase::MainSequence {
            continue;
        }
        let limit = *turn_off.get_or_insert_with(|| turn_off_mass(state.age(), ctx.composition));
        if s.mass().value() > limit.value() * (1.0 + BLUE_STRAGGLER_MARGIN) {
            return Some(BinaryClass::BlueStraggler);
        }
    }
    None
}

/// The relative margin by which a blue straggler exceeds the turn-off mass: 10⁻⁹, so that a star
/// ending its main sequence at its own initial mass, which the turn-off's bisection finds only to
/// its last bits, is never one.
const BLUE_STRAGGLER_MARGIN: f64 = 1.0e-9;

impl BinaryTimeline {
    /// The context in which the state at `age` is classified: the pair's composition, draws and
    /// parameters, and each neutron star's pulsar then, recycled by what it accreted since its
    /// birth (`recycling.rs`).
    #[must_use]
    pub fn class_context(&self, age: Years) -> ClassContext<'_> {
        let ctx = self.context();
        let pulsars = Component::BOTH.map(|c| pulsar_at(self, c, ctx.draws(c.index()), age));
        let merged_from = self
            .merger_age()
            .filter(|&t| t <= age)
            .and_then(|t| pre_merger_state(self, t))
            .map(|state| state.stars().map(|s| s.phase()));
        ClassContext::new(
            ctx.composition(),
            [ctx.draws(0), ctx.draws(1)],
            ctx.params(),
            pulsars,
            merged_from,
        )
    }

    /// The pair's class at `age` ([`classify`] of the state then, in its context).
    #[must_use]
    pub fn class_at(&self, age: Years) -> BinaryClass {
        classify(&self.state_at(age), &self.class_context(age))
    }
}

/// The source horizon in the pair's ages, for a system `age_at_epoch` old at the epoch:
/// −(H + L) to +H about it (plan 01), cut to the timeline's span; `None` if they do not meet.
#[must_use]
fn horizon_years(timeline: &BinaryTimeline, age_at_epoch: Years) -> Option<(f64, f64)> {
    let now = age_at_epoch.value();
    let h = CLOCK_WINDOW_H.as_julian_years_f64();
    let l = LIGHT_CROSSING_L.as_julian_years_f64();
    let lo = (now - (h + l)).max(0.0);
    let hi = (now + h).min(timeline.until().value());
    (lo <= hi).then_some((lo, hi))
}

/// Which merger a pair that coalesced at `t` made: a neutron-star merger for two compact stars
/// of which at least one is a neutron star, a stellar merger if either was living, and none
/// otherwise (two white dwarfs, P11.T6's pool; a star destroyed by its own explosion, which the
/// engine also marks by its member being gone).
#[must_use]
fn merger_class(timeline: &BinaryTimeline, t: Years) -> Option<CarvedClass> {
    let destroyed = timeline
        .supernovae()
        .iter()
        .any(|s| s.remnant().kind() == RemnantKind::None && s.age().total_cmp(&t).is_eq());
    if destroyed {
        return None;
    }
    let state = pre_merger_state(timeline, t)?;
    let [p0, p1] = state.stars().map(|s| s.phase());
    if is_neutron_star_or_black_hole(p0)
        && is_neutron_star_or_black_hole(p1)
        && (p0 == Phase::NeutronStar || p1 == Phase::NeutronStar)
    {
        Some(CarvedClass::NeutronStarMerger)
    } else if p0.is_living() || p1.is_living() {
        Some(CarvedClass::StellarMerger)
    } else {
        None
    }
}

/// The pair before it coalesced at `t`: its state halfway through the last segment that began
/// before `t` (the common envelope that ends a merger has no length, so this is the phase before
/// it), `None` if no segment did.
#[must_use]
fn pre_merger_state(timeline: &BinaryTimeline, t: Years) -> Option<BinaryState> {
    let before = timeline
        .segments()
        .iter()
        .rfind(|s| s.start() < t && s.kind() != SegmentKind::Merged)?;
    Some(timeline.state_at(Years::new(f64::midpoint(before.start().value(), t.value()))))
}

/// The ages at which [`carved_class`] classifies a timeline inside the horizon `(lo, hi)`: each
/// segment's first age there and the midpoint of its span there, `now`, and `hi`, in rising order
/// without repeats.
#[must_use]
fn horizon_sample_years(timeline: &BinaryTimeline, (lo, hi): (f64, f64), now: f64) -> Vec<f64> {
    let mut ages = Vec::with_capacity(2 * timeline.segments().len() + 2);
    for s in timeline.segments() {
        let (start, end) = (s.start().value().max(lo), s.end().value().min(hi));
        if start > hi || end < lo {
            continue;
        }
        ages.push(start);
        if end > start {
            ages.push(f64::midpoint(start, end));
        }
    }
    ages.push(now.clamp(lo, hi));
    ages.push(hi);
    ages.sort_by(f64::total_cmp);
    ages.dedup_by(|a, b| a.total_cmp(b).is_eq());
    ages
}

/// The carved class of `timeline` for a system `age_at_epoch` old at the epoch, if it has one
/// (plan 11's Provides; design notes 8 and 10): the one test the grid and the catalogue share.
///
/// In order: a merger whose age lies in the source horizon, a neutron-star merger or a stellar
/// one ([`CarvedClass`]); an X-ray binary at any age of the horizon; an accreting white dwarf
/// (a cataclysmic variable, a symbiotic star or a single-degenerate Type Ia progenitor) at any
/// age of it, fast if its novae recur within L at the epoch's age, or at the first age of the
/// horizon at which it accretes if it does not then, and slow otherwise.
///
/// "Any age" is read at each segment's first age inside the horizon, the midpoint of its span
/// there, the epoch's age and the horizon's end: the classes it looks for are those of a segment
/// (a pair in stable transfer onto a compact star is one of them throughout), and a wind-fed class
/// is read at those ages alone, identically on both sides.
#[must_use]
pub fn carved_class(timeline: &BinaryTimeline, age_at_epoch: Years) -> Option<CarvedClass> {
    let (lo, hi) = horizon_years(timeline, age_at_epoch)?;
    if let Some(t) = timeline.merger_age()
        && (lo..=hi).contains(&t.value())
        && let Some(class) = merger_class(timeline, t)
    {
        return Some(class);
    }
    let now = age_at_epoch.value().clamp(lo, hi);
    let mut xray = false;
    let mut white_dwarf: Option<(f64, bool)> = None;
    for age in horizon_sample_years(timeline, (lo, hi), now) {
        let at = Years::new(age);
        let state = timeline.state_at(at);
        let found = classified(&state, &timeline.class_context(at));
        if found.class.is_xray_binary() {
            xray = true;
            continue;
        }
        let Some(feeding) = found.feeding else {
            continue;
        };
        let accretor = &state.stars()[feeding.accretor];
        if !is_white_dwarf(accretor.phase()) {
            continue;
        }
        let at_now = age.total_cmp(&now).is_eq();
        if white_dwarf.is_none_or(|(_, earlier_now)| at_now && !earlier_now) {
            let recurrence = nova_recurrence_period(accretor.mass(), feeding.rate);
            white_dwarf = Some((recurrence.value(), at_now));
        }
    }
    if xray {
        return Some(CarvedClass::XrayBinary);
    }
    white_dwarf.map(|(recurrence, _)| {
        if recurrence <= LIGHT_CROSSING_L.as_julian_years_f64() {
            CarvedClass::AccretingWdFast
        } else {
            CarvedClass::AccretingWdSlow
        }
    })
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::lcg::Lcg;

    use super::*;
    use crate::galaxy::imf::{Chabrier, MassFunction};
    use crate::id::{BodyId, Layer, SystemId};
    use crate::orbit::{Eccentricity, KeplerElements, Orientation};
    use crate::stellar::StarState;
    use crate::stellar::binary::tests::{describe, pair, period_days};
    use crate::stellar::binary::{
        BinaryInput, BinaryMarks, BinaryParams, MarkedMerger, MarkedPhase, can_interact, evolve,
    };
    use crate::stellar::multiplicity::MultiplicityModel;
    use crate::units::{Days, Dex, GravitationalParameter, HeliumExcess, Radians, Seconds};
    use crate::{Seed, coords::GenCell};

    fn run(m1: f64, m2: f64, period: f64, e: f64, params: BinaryParams) -> BinaryTimeline {
        evolve(
            &pair(m1, m2, period, e, 0.02).with_params(params),
            Years::new(1.5e10),
        )
    }

    /// The midpoint of segment `i`'s span inside the timeline.
    fn mid(timeline: &BinaryTimeline, i: usize) -> Years {
        let s = &timeline.segments()[i];
        Years::new(f64::midpoint(
            s.start().value(),
            s.end().value().min(timeline.until().value()),
        ))
    }

    /// The first segment satisfying `test`, by index.
    fn segment(
        timeline: &BinaryTimeline,
        test: impl Fn(SegmentKind, &BinaryState) -> bool,
    ) -> Option<usize> {
        (0..timeline.segments().len()).find(|&i| {
            let s = &timeline.segments()[i];
            test(s.kind(), &timeline.state_at(mid(timeline, i)))
        })
    }

    fn algol() -> BinaryTimeline {
        run(
            2.9,
            0.9,
            8.0,
            0.7,
            BinaryParams::GENERATOR.with_alpha_ce(3.0),
        )
    }

    fn cataclysmic() -> BinaryTimeline {
        run(
            6.0,
            1.3,
            630.0,
            0.0,
            BinaryParams::GENERATOR.with_alpha_ce(1.0),
        )
    }

    fn double_neutron_star() -> BinaryTimeline {
        let params = BinaryParams {
            natal_kicks: false,
            ..BinaryParams::GENERATOR.with_alpha_ce(3.0)
        };
        run(13.6, 10.0, period_days(13.6, 10.0, 100.0), 0.0, params)
    }

    fn common_envelope_merger() -> BinaryTimeline {
        run(
            2.0,
            0.2,
            period_days(2.0, 0.2, 50.0),
            0.0,
            BinaryParams::GENERATOR.with_alpha_ce(3.0),
        )
    }

    fn type_ia_candidate() -> BinaryTimeline {
        run(
            3.2,
            2.0,
            5.0,
            0.0,
            BinaryParams::GENERATOR.with_alpha_ce(3.0),
        )
    }

    /// P11.T5: BSE's Algol (section 3.1) is an Algol once the mass ratio has reversed, and its
    /// gainer a blue straggler beside the white dwarf the donor becomes.
    #[test]
    fn the_papers_algol_is_an_algol_then_a_blue_straggler() {
        let t = algol();
        let transfer = segment(&t, |k, s| {
            k == SegmentKind::StableTransfer {
                donor: Component::Primary,
            } && s.stars()[0].mass() < s.stars()[1].mass()
        })
        .unwrap_or_else(|| panic!("no reversed transfer\n{}", describe(&t)));
        assert_eq!(t.class_at(mid(&t, transfer)), BinaryClass::Algol);
        let straggler = segment(&t, |k, s| {
            k == SegmentKind::Detached
                && s.stars()[0].phase() == Phase::CarbonOxygenWhiteDwarf
                && s.stars()[1].phase() == Phase::MainSequence
        })
        .unwrap_or_else(|| panic!("no white dwarf beside the gainer\n{}", describe(&t)));
        assert_eq!(t.class_at(mid(&t, straggler)), BinaryClass::BlueStraggler);
    }

    /// P11.T5: BSE's cataclysmic variable (section 3.2) is one, of the kind its rate and period
    /// give, and an accreting white dwarf of the carved class its recurrence period gives.
    #[test]
    fn the_papers_cataclysmic_variable_is_one() {
        let t = cataclysmic();
        let cv = segment(&t, |k, s| {
            k == SegmentKind::StableTransfer {
                donor: Component::Secondary,
            } && s.stars()[1].phase() == Phase::MainSequence
                && is_white_dwarf(s.stars()[0].phase())
        })
        .unwrap_or_else(|| panic!("no cataclysmic variable\n{}", describe(&t)));
        let age = mid(&t, cv);
        let state = t.state_at(age);
        let rate = state.transfer_rate().expect("transfer");
        let line = cv_critical_rate(state.stars()[0].mass(), disc_radius(&state, 0));
        let kind = if rate >= line {
            CvKind::NovaLike
        } else {
            CvKind::DwarfNova
        };
        assert_eq!(t.class_at(age), BinaryClass::CataclysmicVariable(kind));
        let recurrence = nova_recurrence_period(state.stars()[0].mass(), rate);
        let expected = if recurrence.value() <= LIGHT_CROSSING_L.as_julian_years_f64() {
            CarvedClass::AccretingWdFast
        } else {
            CarvedClass::AccretingWdSlow
        };
        assert_eq!(
            carved_class(&t, age),
            Some(expected),
            "P_rec {recurrence:?}"
        );
        // Later its donor, a giant, fills its lobe: a symbiotic star.
        let symbiotic = segment(&t, |k, s| {
            matches!(k, SegmentKind::StableTransfer { .. }) && is_giant(s.stars()[1].phase())
        })
        .expect("the donor's giant branch");
        assert_eq!(t.class_at(mid(&t, symbiotic)), BinaryClass::Symbiotic);
    }

    /// P11.T5: BSE's double neutron star (section 3.4, without kicks) is a supergiant high-mass
    /// X-ray binary before its second supernova, which the carve takes, and a double neutron star
    /// after it, which it does not.
    #[test]
    fn the_papers_double_neutron_star_is_one() {
        let t = double_neutron_star();
        let second = t.supernovae()[1].age().value();
        let after = Years::new(second * (1.0 + 1e-6));
        assert_eq!(t.class_at(after), BinaryClass::DoubleNeutronStar);
        assert_eq!(carved_class(&t, after), None);
        let wind = segment(&t, |k, s| {
            k == SegmentKind::Detached
                && s.stars()[0].phase() == Phase::NeutronStar
                && s.stars()[1].phase() == Phase::HertzsprungGap
        })
        .unwrap_or_else(|| panic!("no supergiant beside the neutron star\n{}", describe(&t)));
        let age = mid(&t, wind);
        assert_eq!(
            t.class_at(age),
            BinaryClass::HighMassXrayBinary(HmxbKind::Supergiant)
        );
        assert_eq!(carved_class(&t, age), Some(CarvedClass::XrayBinary));
    }

    /// P11.T5: BSE's common-envelope merger (section 4.2.2) is a stellar merger at its merger's
    /// age and for the horizon after it, and not once it has left the horizon.
    #[test]
    fn a_common_envelope_merger_is_a_stellar_merger_in_the_horizon() {
        let t = common_envelope_merger();
        let merger = t.merger_age().expect("a merger").value();
        for offset in [0.0, 500.0, -1.0e5] {
            let now = Years::new(merger - offset);
            assert_eq!(
                carved_class(&t, now),
                Some(CarvedClass::StellarMerger),
                "{offset}"
            );
        }
        let h = CLOCK_WINDOW_H.as_julian_years_f64();
        let l = LIGHT_CROSSING_L.as_julian_years_f64();
        assert_eq!(carved_class(&t, Years::new(merger - 2.0 * h)), None);
        assert_eq!(carved_class(&t, Years::new(merger + h + l + 1.0)), None);
    }

    /// P11.T5 and ruling 129.1: Tout et al.'s Algol (BSE section 3.4) leaves two carbon–oxygen
    /// dwarfs of 0.49 and 0.66 M☉, 1.15 M☉ together, too light to detonate: a double white dwarf,
    /// whose merger P11.T6's pool still holds, and which is not a carved class.
    #[test]
    fn the_type_ia_candidate_is_a_double_white_dwarf_too_light_to_detonate() {
        let t = type_ia_candidate();
        let dwarfs = segment(&t, |_, s| {
            s.stars()
                .iter()
                .all(|star| star.phase() == Phase::CarbonOxygenWhiteDwarf)
                && s.orbit().is_some()
        })
        .unwrap_or_else(|| panic!("no double white dwarf\n{}", describe(&t)));
        let state = t.state_at(mid(&t, dwarfs));
        let [m0, m1] = state.stars().map(|s| s.mass().value());
        assert!(
            m0 + m1 < CHANDRASEKHAR_MASS.value() && m0.max(m1) < MIN_DETONATABLE_MASS.value(),
            "{m0} + {m1} M☉"
        );
        assert_eq!(t.class_at(mid(&t, dwarfs)), BinaryClass::DoubleWhiteDwarf);
        assert!(t.pooled_ia().is_some(), "the merger is still pooled");
        let merger = t.merger_age().expect("the dwarfs merge");
        assert_eq!(carved_class(&t, merger), None);
        // Before, its donor was left a hot subdwarf by the first transfer.
        let subdwarf = segment(&t, |_, s| s.stars()[0].phase() == Phase::HeliumMainSequence)
            .expect("a stripped helium star");
        assert_eq!(t.class_at(mid(&t, subdwarf)), BinaryClass::HotSubdwarf);
    }

    /// A state held as marked: `stars` detached on a circular orbit of `period_days`, with `draws`.
    fn detached(stars: [StarState; 2], period_days: f64, draws: [StarDraws; 2]) -> BinaryTimeline {
        let total = stars[0].mass().value() + stars[1].mass().value();
        let orbit = KeplerElements::from_period(
            Seconds::new(period_days * 86_400.0),
            GravitationalParameter::from_solar_masses(SolarMasses::new(total)),
            Eccentricity::CIRCULAR,
            Orientation::new(Radians::new(0.3), Radians::new(0.0), Radians::new(0.0))
                .expect("an orientation"),
            Radians::new(0.0),
        )
        .expect("an orbit");
        let now = Years::new(1.0e8);
        let marks = BinaryMarks::phase(
            Composition::SOLAR,
            draws,
            now,
            MarkedPhase {
                kind: SegmentKind::Detached,
                start: Years::new(0.0),
                end: Years::new(2.0e8),
                stars,
                orbit,
                transfer_rate: None,
            },
        )
        .expect("valid marks");
        BinaryTimeline::from_marks(marks)
    }

    /// A star of `phase`, `mass`, `luminosity` and `radius`, losing `wind` M☉ yr⁻¹.
    fn star(phase: Phase, mass: f64, luminosity: f64, radius: f64, wind: f64) -> StarState {
        StarState::new(crate::stellar::StarStateParts {
            phase,
            age: Years::new(1.0e8),
            mass: SolarMasses::new(mass),
            core_mass: SolarMasses::new(if phase.is_remnant() { mass } else { 0.0 }),
            luminosity: crate::units::SolarLuminosities::new(luminosity),
            radius: crate::units::SolarRadii::new(radius),
            mass_loss_rate: SolarMassesPerYear::new(wind),
            phase_fraction: 0.5,
        })
    }

    /// Ruling 129.1: a close pair of white dwarfs is a Type Ia progenitor above the Chandrasekhar
    /// mass, or with a carbon–oxygen dwarf of 0.85 M☉ or more, and a double white dwarf otherwise.
    #[test]
    fn a_double_white_dwarf_is_a_progenitor_only_if_it_can_detonate() {
        let co = Phase::CarbonOxygenWhiteDwarf;
        let he = Phase::HeliumWhiteDwarf;
        let class = |a: (Phase, f64), b: (Phase, f64)| {
            let stars = [
                star(a.0, a.1, 1e-3, 0.01, 0.0),
                star(b.0, b.1, 1e-3, 0.012, 0.0),
            ];
            detached(stars, 0.1, [StarDraws::median(), StarDraws::median()])
                .class_at(Years::new(1.0e8))
        };
        assert_eq!(class((co, 0.9), (co, 0.5)), BinaryClass::TypeIaProgenitor);
        assert_eq!(class((co, 0.8), (co, 0.7)), BinaryClass::TypeIaProgenitor);
        assert_eq!(class((co, 0.66), (co, 0.49)), BinaryClass::DoubleWhiteDwarf);
        assert_eq!(class((co, 0.7), (he, 0.4)), BinaryClass::DoubleWhiteDwarf);
        assert_eq!(class((he, 0.45), (he, 0.45)), BinaryClass::DoubleWhiteDwarf);
    }

    /// Ruling 129.2: a neutron star beside a Be star is a Be/X-ray binary only for a Be star of
    /// 8 M☉ or more on an orbit of 1,000 d or less.
    #[test]
    fn a_be_x_ray_binary_needs_a_massive_be_star_on_a_short_orbit() {
        let neutron_star = star(Phase::NeutronStar, 1.4, 1e-4, 1.7e-5, 0.0);
        let b_star = |mass: f64, luminosity: f64, radius: f64| {
            star(Phase::MainSequence, mass, luminosity, radius, 0.0)
        };
        let fast = (1..100)
            .map(|k| {
                StarDraws::from_parts(crate::stellar::draws::StarDrawsParts {
                    rotation: crate::stellar::draws::UnitUniform::new(f64::from(k) / 100.0)
                        .expect("a rank"),
                    ..*StarDraws::median().parts()
                })
            })
            .find(|d| {
                is_be_star(&b_star(12.0, 1.0e4, 5.0), &Composition::SOLAR, d)
                    && is_be_star(&b_star(6.0, 1.0e3, 3.2), &Composition::SOLAR, d)
            })
            .expect("some rotation rank makes both a 12 and a 6 M☉ B star a Be star");
        let class = |donor: StarState, period: f64| {
            detached(
                [neutron_star, donor],
                period,
                [StarDraws::median(), fast.clone()],
            )
            .class_at(Years::new(1.0e8))
        };
        let be_x = BinaryClass::HighMassXrayBinary(HmxbKind::BeX);
        assert_eq!(class(b_star(12.0, 1.0e4, 5.0), 100.0), be_x);
        assert_ne!(class(b_star(12.0, 1.0e4, 5.0), 2_000.0), be_x);
        assert_ne!(class(b_star(6.0, 1.0e3, 3.2), 100.0), be_x);
    }

    /// Ruling 129.3: a neutron star fed by a low-mass giant's wind is a symbiotic X-ray binary, a
    /// low-mass X-ray binary, carved as one.
    #[test]
    fn a_neutron_star_fed_by_a_giants_wind_is_a_symbiotic_x_ray_binary() {
        let giant = star(Phase::FirstGiantBranch, 1.5, 500.0, 50.0, 1.0e-8);
        let draws = [StarDraws::median(), StarDraws::median()];
        let neutron_star = star(Phase::NeutronStar, 1.4, 1e-4, 1.7e-5, 0.0);
        let t = detached([neutron_star, giant], 1_000.0, draws.clone());
        let now = Years::new(1.0e8);
        assert_eq!(
            t.class_at(now),
            BinaryClass::LowMassXrayBinary(XrbKind::Symbiotic)
        );
        assert_eq!(carved_class(&t, now), Some(CarvedClass::XrayBinary));
        // A wind a millionth as strong feeds it under 10³² erg/s: no X-ray binary.
        let faint = star(Phase::FirstGiantBranch, 1.5, 500.0, 50.0, 1.0e-14);
        let t_faint = detached([neutron_star, faint], 1_000.0, draws.clone());
        assert!(!t_faint.class_at(now).is_xray_binary());
        // A white dwarf in its place is never an X-ray binary; it is a symbiotic star only above
        // 10 L☉ of accretion, which a_wide_white_dwarf_beside_a_giant_is_symbiotic reaches.
        let white_dwarf = star(Phase::CarbonOxygenWhiteDwarf, 0.8, 1e-2, 0.01, 0.0);
        let t = detached([white_dwarf, giant], 1_000.0, draws);
        assert!(!t.class_at(now).is_xray_binary());
    }

    /// Design note 7: a pair too wide to interact is classified from its state, and a white dwarf
    /// fed by its companion's giant wind is a symbiotic star however wide the orbit.
    #[test]
    fn a_wide_white_dwarf_beside_a_giant_is_symbiotic() {
        for period in [3.0e4, 1.0e5, 3.0e5] {
            let input = pair(3.0, 2.6, period, 0.0, 0.02);
            let until = Years::new(1.0e9);
            if can_interact(&input, until) {
                continue;
            }
            let t = evolve(&input, until);
            // Every 10⁴ years: the companion's superwind, where the white dwarf takes enough of its
            // wind, lasts about 10⁵ years at the end of its thermally pulsing AGB.
            let found = (0..=70_000).find_map(|k| {
                let age = Years::new(3.0e8 + 7.0e8 * f64::from(k) / 70_000.0);
                (t.class_at(age) == BinaryClass::Symbiotic).then_some(age)
            });
            let Some(age) = found else {
                continue;
            };
            let state = t.state_at(age);
            assert_eq!(t.segments().len(), 1, "a wide pair is one segment");
            assert!(is_white_dwarf(state.stars()[0].phase()));
            assert!(is_giant(state.stars()[1].phase()));
            return;
        }
        panic!("no wide white dwarf and giant pair was symbiotic");
    }

    /// The reference timelines, and a low-mass X-ray binary of the prior.
    fn references() -> Vec<BinaryTimeline> {
        vec![
            xray_binary(),
            algol(),
            cataclysmic(),
            double_neutron_star(),
            common_envelope_merger(),
            type_ia_candidate(),
        ]
    }

    fn ages(t: &BinaryTimeline) -> Vec<Years> {
        (0..t.segments().len())
            .flat_map(|i| [t.segments()[i].start(), mid(t, i)])
            .collect()
    }

    /// P11.T5: a class is a pure function of the state and its context: classified in any order,
    /// and from a copy of the state, it is the same.
    #[test]
    fn classification_is_a_pure_function_of_state() {
        for t in references() {
            let ages = ages(&t);
            let forward: Vec<BinaryClass> = ages.iter().map(|&a| t.class_at(a)).collect();
            let backward: Vec<BinaryClass> = ages.iter().rev().map(|&a| t.class_at(a)).collect();
            assert_eq!(
                forward,
                backward.into_iter().rev().collect::<Vec<_>>(),
                "{}",
                describe(&t)
            );
            for &age in ages.iter().rev() {
                let state = t.state_at(age);
                let copy = state;
                let context = t.class_context(age);
                assert_eq!(classify(&state, &context), classify(&copy, &context));
                assert_eq!(classify(&copy, &context), t.class_at(age));
            }
        }
    }

    /// The marks that define `t` at `now`: the phase that holds then, or its merger.
    fn marks_at(t: &BinaryTimeline, now: Years) -> Option<BinaryMarks> {
        let ctx = t.context();
        let draws = [ctx.draws(0).clone(), ctx.draws(1).clone()];
        let composition = *ctx.composition();
        if let Some(merger) = t.merger_age() {
            let before = t
                .segments()
                .iter()
                .rfind(|s| s.start() < merger && s.kind() != SegmentKind::Merged)?;
            let at = Years::new(f64::midpoint(before.start().value(), merger.value()));
            let state = t.state_at(at);
            let product = t.state_at(Years::new(merger.value().max(now.value())));
            return BinaryMarks::merger(
                composition,
                draws,
                now,
                MarkedMerger {
                    age: merger,
                    before: before.kind(),
                    stars: *state.stars(),
                    orbit: *state.orbit()?,
                    transfer_rate: state.transfer_rate(),
                    product: product.stars()[0],
                },
            )
            .inspect_err(|e| panic!("marks read off an evolved timeline are valid: {e}"))
            .ok();
        }
        let index = t
            .segments()
            .partition_point(|s| s.start() <= now)
            .saturating_sub(1);
        let segment = &t.segments()[index];
        let state = t.state_at(now);
        BinaryMarks::phase(
            composition,
            draws,
            now,
            MarkedPhase {
                kind: segment.kind(),
                start: segment.start(),
                end: Years::new(segment.end().value().min(t.until().value())),
                stars: *state.stars(),
                orbit: *state.orbit()?,
                transfer_rate: state.transfer_rate(),
            },
        )
        .inspect_err(|e| panic!("marks read off an evolved timeline are valid: {e}"))
        .ok()
    }

    /// Whether the segment holding `now` covers the whole source horizon about it, so that the
    /// marks of that segment see every age the carve reads.
    fn phase_covers_horizon(t: &BinaryTimeline, now: Years) -> bool {
        let index = t
            .segments()
            .partition_point(|s| s.start() <= now)
            .saturating_sub(1);
        let s = &t.segments()[index];
        let h = CLOCK_WINDOW_H.as_julian_years_f64();
        let l = LIGHT_CROSSING_L.as_julian_years_f64();
        s.start().value() <= (now.value() - h - l).max(0.0) && s.end().value() >= now.value() + h
    }

    /// Checks that the timeline built from `t`'s marks at `now` gives the same carved class and
    /// the same state at `now`, returning whether it was checked.
    fn check_marks(t: &BinaryTimeline, now: Years) -> bool {
        let Some(class) = carved_class(t, now) else {
            return false;
        };
        let merger = matches!(
            class,
            CarvedClass::StellarMerger | CarvedClass::NeutronStarMerger
        );
        if !merger && !phase_covers_horizon(t, now) {
            return false;
        }
        // Marks hold the state at `now`, so they carry a phase's class only where it holds then: a
        // wind-fed class that crosses its luminosity line later in the horizon is not one.
        let found = classified(&t.state_at(now), &t.class_context(now));
        let holds_now = match class {
            CarvedClass::XrayBinary => found.class.is_xray_binary(),
            CarvedClass::AccretingWdFast | CarvedClass::AccretingWdSlow => found.feeding.is_some(),
            CarvedClass::StellarMerger | CarvedClass::NeutronStarMerger => true,
        };
        if !holds_now {
            return false;
        }
        let Some(marks) = marks_at(t, now) else {
            return false;
        };
        let built = BinaryTimeline::from_marks(marks);
        assert_eq!(carved_class(&built, now), Some(class), "{}", describe(t));
        if !merger || now < t.merger_age().unwrap_or(now) {
            assert_eq!(built.state_at(now), t.state_at(now), "{}", describe(t));
        }
        true
    }

    /// P11.T5 and design note 10: a timeline built from the marks read off an evolved timeline has
    /// the same carved class and the same state now.
    #[test]
    fn a_timeline_from_marks_keeps_its_carved_class_and_state() {
        let mut checked = [0_usize; 3];
        let references = references();
        for t in &references {
            for i in 0..t.segments().len() {
                let now = mid(t, i);
                if check_marks(t, now) {
                    let slot = match carved_class(t, now) {
                        Some(CarvedClass::XrayBinary) => 0,
                        Some(CarvedClass::AccretingWdFast | CarvedClass::AccretingWdSlow) => 1,
                        _ => 2,
                    };
                    checked[slot] += 1;
                }
            }
            if let Some(merger) = t.merger_age()
                && check_marks(t, merger)
            {
                checked[2] += 1;
            }
        }
        assert!(
            checked.iter().all(|&n| n > 0),
            "marks checked for X-ray binaries, accreting white dwarfs and mergers: {checked:?}"
        );
    }

    /// The instability line of a cataclysmic variable in transfer rate against period follows the
    /// power law Coriat, Fender and Dubus (2012, section 4) fit to the same relations, (2.6 ± 0.9)
    /// × 10¹⁶ g s⁻¹ `P_hr`^1.76, and the irradiated line (2.9 ± 0.9) × 10¹⁵ `P_hr`^1.59 for a neutron
    /// star.
    #[test]
    fn the_disc_instability_lines_follow_coriat_et_al() {
        let line = |m1: f64, m2: f64, hours: f64, irradiated: bool| {
            let orbit = KeplerElements::from_period(
                Seconds::new(hours * 3_600.0),
                GravitationalParameter::from_solar_masses(SolarMasses::new(m1 + m2)),
                Eccentricity::CIRCULAR,
                Orientation::new(Radians::new(0.0), Radians::new(0.0), Radians::new(0.0))
                    .expect("an orientation"),
                Radians::new(0.0),
            )
            .expect("an orbit");
            let r = 0.60 * orbit.semi_major_axis().value() / (1.0 + m2 / m1);
            let rate = if irradiated {
                xrb_critical_rate(SolarMasses::new(m1), Metres::new(r))
            } else {
                cv_critical_rate(SolarMasses::new(m1), Metres::new(r))
            };
            rate.value() * GRAMS_PER_SECOND_PER_MSUN_PER_YEAR
        };
        for hours in [2.0, 5.0, 24.0] {
            let cv = line(1.4, 0.7, hours, false) / (2.6e16 * math::powf(hours, 1.76));
            let lmxb = line(1.4, 0.7, hours, true) / (2.9e15 * math::powf(hours, 1.59));
            assert!((0.65..1.35).contains(&cv), "{hours} h: {cv}");
            assert!((0.65..1.35).contains(&lmxb), "{hours} h: {lmxb}");
        }
    }

    /// The novae of a massive white dwarf recur faster than a light one's, and faster at a higher
    /// rate; steady burning makes none.
    #[test]
    fn nova_recurrence_falls_with_mass_and_rate() {
        let p = |m: f64, rate: f64| {
            nova_recurrence_period(SolarMasses::new(m), SolarMassesPerYear::new(rate)).value()
        };
        assert!(p(1.3, 1.0e-9) < p(0.8, 1.0e-9));
        assert!(p(1.0, 1.0e-8) < p(1.0, 1.0e-10));
        assert!(p(1.0, 2.0e-7).is_infinite());
        assert!(p(1.0, 0.0).is_infinite());
        // The fit at 1 M☉ and 10⁻⁹ M☉/yr: M_ign = 10^(−3.728 + 1.920 log 0.44) = 3.9 × 10⁻⁵ M☉.
        let m = nova_ignition_mass(SolarMasses::new(1.0), SolarMassesPerYear::new(1.0e-9));
        assert!((m.value() / 3.87e-5 - 1.0).abs() < 0.01, "{m:?}");
        // Ruling 129.5: the rate is held to the fit's floor of 10⁻¹¹ M☉/yr, not extrapolated.
        let at = |rate: f64| {
            nova_ignition_mass(SolarMasses::new(1.0), SolarMassesPerYear::new(rate)).value()
        };
        assert!((at(1.0e-12) / at(1.0e-11) - 1.0).abs() < 1e-12);
        assert!(at(1.0e-11) > at(3.0e-11));
    }

    /// A pair drawn from the multiplicity model's priors: the primary from Chabrier's system
    /// function on `lo`–`hi` M☉, the period, mass ratio and eccentricity from the model at that
    /// mass, the metallicity uniform in [Fe/H] on −1.5 to +0.4, and each star's draws from its own
    /// seed. The slow census in `tests/binary_classes.rs` draws its pairs the same way.
    fn prior_pair_in(lcg: &mut Lcg, index: u64, (lo, hi): (f64, f64)) -> BinaryInput {
        let model = MultiplicityModel::default_v1();
        let imf = Chabrier::provisional();
        let m1 = imf.quantile_in(lo, hi, lcg.next_f64());
        let periods = model.period_distribution(SolarMasses::new(m1));
        let period = Days::new(math::exp10(periods.quantile(lcg.next_f64())));
        let q = model
            .mass_ratio_distribution(SolarMasses::new(m1), period)
            .quantile(lcg.next_f64());
        let m2 = (q * m1).max(0.08);
        let e = model
            .eccentricity_distribution(SolarMasses::new(m1), period)
            .quantile(lcg.next_f64())
            .min(0.95);
        let fe_h = -1.5 + 1.9 * lcg.next_f64();
        let orbit = KeplerElements::from_period(
            Seconds::from(period),
            GravitationalParameter::from_solar_masses(SolarMasses::new(m1 + m2)),
            Eccentricity::new(e).expect("an eccentricity below 1"),
            Orientation::new(
                Radians::new(math::acos(1.0 - 2.0 * lcg.next_f64())),
                Radians::new(core::f64::consts::TAU * lcg.next_f64()),
                Radians::new(core::f64::consts::TAU * lcg.next_f64()),
            )
            .expect("an orientation"),
            Radians::new(core::f64::consts::TAU * lcg.next_f64()),
        )
        .expect("an orbit");
        let cell = GenCell::new(Layer::E.cell_size(), [0, 0, 0]).expect("a cell");
        let system = SystemId::from_parts(Layer::E, cell, 0).expect("a system");
        let seed = Seed::new(0x5eed_0000 + index);
        BinaryInput::new(
            SolarMasses::new(m1),
            SolarMasses::new(m2),
            Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::new(0.0)),
            orbit,
            [
                StarDraws::for_star(seed, BodyId::new(system, 0)),
                StarDraws::for_star(seed, BodyId::new(system, 1)),
            ],
            Years::new(1.0e10),
        )
        .expect("a pair")
    }

    /// A low-mass X-ray binary: the 25th layer-E pair of the prior sample of seed 12 (8.09 and 2.69
    /// M☉, 101 d, [Fe/H] −1.10), whose neutron star is fed for longer than the source horizon.
    fn xray_binary() -> BinaryTimeline {
        let mut lcg = Lcg::new(12);
        let mut input = None;
        for index in 0..25 {
            input = Some(prior_pair_in(&mut lcg, index, (8.0, 150.0)));
        }
        evolve(&input.expect("25 pairs"), AGE_OF_UNIVERSE)
    }
}
