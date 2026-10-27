//! The supernova test with a clock (plan 09, P09.T17), and a supernova catalogue entry by
//! evaluation time, its explosion as an event, and its light curve (P09.T19).
//!
//! **The test.** A system is a supernova catalogue entry if and only if its death T lies in
//! (−(`W_eff` + L + H), +H], where `W_eff` is its shell's window as the test cuts it ([`claims`],
//! [`SupernovaInterval`]). What the test reads of a death is its [`DeathMarks`]: T, what exploded,
//! the explosion's energy (one uniform of `snr.energy`, keyed by the system's ID) and the fastest
//! speed any member then has. Every side of the carve-out (the grid, the displaced bins, the
//! feature members and the catalogue classes) builds the marks of its own candidate with
//! [`DeathMarks::of`] (or, for a Type Ia candidate, [`DeathMarks::of_type_ia`]) and calls the one
//! [`claims`], after plan 08's [`death_in_interval`](crate::galaxy::displaced::death_in_interval),
//! the shared test's first half, has put the death in the interval at its widest. The test reads
//! the smooth gas at the site and the marks, and for a shell inside a superbubble the interior its
//! caller passes ([`Surroundings`]); it never looks up a feature or another system, so the
//! carve-out's dependency graph stays acyclic.
//!
//! **The entry.** Membership does not depend on the time of a query, so an entry has one ID on
//! both sides of its explosion; what it is depends on the time it is evaluated at (brainstorm,
//! "The supernova test with a clock"):
//!
//! | Evaluated          | The entry is ([`SupernovaState`])                                        |
//! | ------------------ | ------------------------------------------------------------------------ |
//! | Before T           | The living progenitor, or a double white dwarf spiralling together       |
//! | From T             | A supernova of that age: its shell and its remnant at kick × age         |
//! | From T + `W_eff`   | The bare remnant: the shell has merged with the gas                      |
//!
//! The explosion itself is an event, [`SUPERNOVA`], bin 0,
//! number 0, on the entry's member 0 ([`SupernovaEntry::explosion_event`]), so that plan 12's
//! alerts can name it.
//!
//! Light curves are templates by type, documented as ours (Design note 18, the brainstorm names no
//! source), built from sums of smooth terms so that no template has a join ([`LightCurve`]).

use crate::coords::GalacticVelocity;
use crate::galaxy::Galaxy;
use crate::galaxy::catalogue_classes::type_ia::{IaChannel, IaLeftover, IaProgenitor};
use crate::galaxy::displaced::ExplosionSite;
use crate::galaxy::gas::noise::NoiseCache;
use crate::galaxy::snr::{
    ExplosionEnergy, PulsarWindNebula, SHELL_WINDOW_CAP, ShellEnvironment, ShellState, ShellWindow,
    SiteGas, WindowCaps, has_bow_shock, remnant_offset, shell_state_at, shell_window,
};
use crate::id::event_tags::SUPERNOVA;
use crate::id::{EventBin, EventId, EventSubject, EventWord, SystemId};
use crate::math;
use crate::rng::{ObjectKey, Stream, tags};
use crate::stellar::remnant::{Death, DeathKind, NeutronStar, SupernovaType};
use crate::stellar::system::{ClockDeath, SystemStars};
use crate::time::{CLOCK_WINDOW_H, ClockWindow, LIGHT_CROSSING_L, Span, UniverseTime};
use crate::units::consts::{SECONDS_PER_DAY, SECONDS_PER_JULIAN_YEAR, SPEED_OF_LIGHT};
use crate::units::{
    KilometresPerSecond, LightYears, MetresPerSecond, Seconds, SolarMasses, Watts, Years,
};

/// What exploded: a core collapse of plan 06's type, or a Type Ia of its channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SupernovaKind {
    /// A core collapse, typed by the progenitor's envelope at death (plan 06).
    CoreCollapse(SupernovaType),
    /// A thermonuclear explosion of a white dwarf.
    TypeIa(IaChannel),
}

/// A supernova catalogue entry: member 0's ID, the time of its explosion, what exploded, its
/// shell's window as its class cut it, and what it leaves.
///
/// Built by the classes of plan 09's phase 7 from their candidates;
/// [`with_remnant`](Self::with_remnant), [`with_leftover`](Self::with_leftover) and
/// [`with_progenitor`](Self::with_progenitor) add what the kind has.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SupernovaEntry {
    system: SystemId,
    explosion: UniverseTime,
    kind: SupernovaKind,
    window: ShellWindow,
    kick: Option<KilometresPerSecond>,
    pulsar: Option<NeutronStar>,
    leftover: Option<IaLeftover>,
    progenitor: Option<IaProgenitor>,
}

/// What an entry is at one evaluation time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SupernovaState {
    /// Before the explosion: the living progenitor, whose state is plan 06's star at the same
    /// time, or a double white dwarf spiralling together.
    Progenitor {
        /// The time left until the explosion.
        until_explosion: Span,
        /// A double white dwarf's orbital period while it spirals in.
        pair_period: Option<Seconds>,
    },
    /// A supernova of `age` with its shell.
    Supernova {
        /// The time since the explosion.
        age: Years,
        /// The shell.
        shell: ShellState,
        /// What the explosion left, and where.
        remnant: RemnantState,
    },
    /// The shell has merged with the gas; the remnant remains.
    BareRemnant {
        /// The time since the explosion.
        age: Years,
        /// What the explosion left, and where.
        remnant: RemnantState,
    },
}

/// What an explosion left, at one time after it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RemnantState {
    offset: Option<LightYears>,
    bow_shock: bool,
    nebula: Option<PulsarWindNebula>,
    survivor: Option<LightYears>,
}

impl RemnantState {
    /// How far the compact remnant is from the explosion site: kick × age. `None` if the
    /// explosion left none (a Type Ia) or the entry has no kick.
    #[must_use]
    pub const fn offset(&self) -> Option<LightYears> {
        self.offset
    }

    /// Whether the compact remnant drives a bow shock past 0.68 of its shell's radius; always
    /// `false` once the shell has merged.
    #[must_use]
    pub const fn bow_shock(&self) -> bool {
        self.bow_shock
    }

    /// Its pulsar's wind nebula, while the spin-down powers one.
    #[must_use]
    pub const fn nebula(&self) -> Option<PulsarWindNebula> {
        self.nebula
    }

    /// How far a Type Ia's survivor, the entry's member 1, is from the site: speed × age.
    #[must_use]
    pub const fn survivor_offset(&self) -> Option<LightYears> {
        self.survivor
    }
}

impl SupernovaEntry {
    /// The entry of member 0 `system` whose `kind` explodes at `explosion` with the shell
    /// `window`, already cut at its class's cap and lifetime.
    #[must_use]
    pub const fn new(
        system: SystemId,
        explosion: UniverseTime,
        kind: SupernovaKind,
        window: ShellWindow,
    ) -> Self {
        Self {
            system,
            explosion,
            kind,
            window,
            kick: None,
            pulsar: None,
            leftover: None,
            progenitor: None,
        }
    }

    /// The same entry with a compact remnant kicked at `kick` from the site, a pulsar if `pulsar`
    /// is one.
    #[must_use]
    pub const fn with_remnant(
        self,
        kick: KilometresPerSecond,
        pulsar: Option<NeutronStar>,
    ) -> Self {
        Self {
            kick: Some(kick),
            pulsar,
            ..self
        }
    }

    /// The same entry with a Type Ia's leftover.
    #[must_use]
    pub const fn with_leftover(self, leftover: IaLeftover) -> Self {
        Self {
            leftover: Some(leftover),
            ..self
        }
    }

    /// The same entry with a Type Ia's progenitor, whose pair's period it reports before the
    /// explosion.
    #[must_use]
    pub const fn with_progenitor(self, progenitor: IaProgenitor) -> Self {
        Self {
            progenitor: Some(progenitor),
            ..self
        }
    }

    /// Member 0's ID, the same at every time.
    #[must_use]
    pub const fn system(&self) -> SystemId {
        self.system
    }

    /// When it explodes, T.
    #[must_use]
    pub const fn explosion(&self) -> UniverseTime {
        self.explosion
    }

    /// What exploded.
    #[must_use]
    pub const fn kind(&self) -> SupernovaKind {
        self.kind
    }

    /// Its shell's window.
    #[must_use]
    pub const fn window(&self) -> &ShellWindow {
        &self.window
    }

    /// The explosion as an event: tag [`SUPERNOVA`], bin 0, number 0, on member 0.
    ///
    /// # Panics
    ///
    /// Never: bin 0 is in range.
    #[must_use]
    pub fn explosion_event(&self) -> EventId {
        let bin = EventBin::new(0).expect("bin 0 is in range");
        EventId::new(
            EventSubject::System(self.system),
            EventWord::new(SUPERNOVA, bin, 0),
        )
    }

    /// The entry at `t`: the progenitor before T, the supernova from T, the bare remnant from T +
    /// `W_eff`.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::galaxy::catalogue_classes::supernova::{SupernovaEntry, SupernovaKind, SupernovaState};
    /// use hyperion_sim::galaxy::snr::{ExplosionEnergy, SiteGas, shell_window};
    /// use hyperion_sim::id::SystemId;
    /// use hyperion_sim::stellar::remnant::SupernovaType;
    /// use hyperion_sim::time::{Span, UniverseTime};
    /// use hyperion_sim::units::{Dex, HydrogenPerCm3, KelvinPerCm3, KilometresPerSecond};
    ///
    /// let site = SiteGas::uniform(HydrogenPerCm3::new(1.0), KelvinPerCm3::new(3_800.0))?;
    /// let window = shell_window(&site, ExplosionEnergy::MEDIAN, Dex::new(0.0));
    /// let system = SystemId::from_raw(0xF000_0007_0000_0000)?;
    /// let entry = SupernovaEntry::new(
    ///     system,
    ///     UniverseTime::EPOCH,
    ///     SupernovaKind::CoreCollapse(SupernovaType::IIP),
    ///     window,
    /// )
    /// .with_remnant(KilometresPerSecond::new(300.0), None);
    /// let decade = Span::from_julian_years(10).ok_or("in range")?;
    /// let before = UniverseTime::EPOCH.checked_sub(decade).ok_or("in range")?;
    /// assert!(matches!(entry.state_at(before), SupernovaState::Progenitor { .. }));
    /// let after = UniverseTime::from_julian_years(10_000).ok_or("in range")?;
    /// assert!(matches!(entry.state_at(after), SupernovaState::Supernova { .. }));
    /// let late = UniverseTime::from_julian_years(5_000_000).ok_or("in range")?;
    /// assert!(matches!(entry.state_at(late), SupernovaState::BareRemnant { .. }));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn state_at(&self, t: UniverseTime) -> SupernovaState {
        let since = t.checked_since(self.explosion).unwrap_or(Span::ZERO);
        if t < self.explosion {
            let until = self.explosion.checked_since(t).unwrap_or(Span::ZERO);
            return SupernovaState::Progenitor {
                until_explosion: until,
                pair_period: self.progenitor.and_then(|p| p.period_at(t)),
            };
        }
        let age = Years::new(since.as_julian_years_f64());
        let offset = self.kick.map(|kick| remnant_offset(kick, age));
        let nebula = self
            .pulsar
            .and_then(|ns| PulsarWindNebula::of(&ns.state_at(age)));
        let survivor = self.leftover.and_then(|l| l.offset_at(age));
        match shell_state_at(&self.window, age) {
            Some(shell) => SupernovaState::Supernova {
                age,
                shell,
                remnant: RemnantState {
                    offset,
                    bow_shock: offset.is_some_and(|o| has_bow_shock(o, &shell)),
                    nebula,
                    survivor,
                },
            },
            None => SupernovaState::BareRemnant {
                age,
                remnant: RemnantState {
                    offset,
                    bow_shock: false,
                    nebula,
                    survivor,
                },
            },
        }
    }
}

/// Nadyozhin's (1994, ApJS 92, 527) heating by the decay of ⁵⁶Ni and ⁵⁶Co per solar mass of
/// nickel, erg/s: `6.45 × 10⁴³ e^(−t ÷ 8.8 d)` and `1.45 × 10⁴³ e^(−t ÷ 111.3 d)`.
const NICKEL_ERG_S: f64 = 6.45e43;
const NICKEL_DAYS: f64 = 8.8;
const COBALT_ERG_S: f64 = 1.45e43;
const COBALT_DAYS: f64 = 111.3;

/// The share of ⁵⁶Co's decay energy carried by positrons' kinetic energy, which stays trapped
/// when the γ-rays escape: 0.12 of the 3.73 megaelectronvolts a decay releases, 3.2%
/// (Nadyozhin 1994, ApJS 92, 527, p. 529; the annihilation photons escape with the γ-rays).
const POSITRON_SHARE: f64 = 0.032;

/// The energy a core collapse radiates in neutrinos, erg, and the burst's e-folding time, s: 3 ×
/// 10⁵³ erg over a few seconds, the order of a neutron star's gravitational binding energy and of
/// SN 1987A's burst (ours, a template parameter; no source pinned).
const NEUTRINO_ERG: f64 = 3e53;
const NEUTRINO_SECONDS: f64 = 3.0;

/// Watts per erg per second.
const WATTS_PER_ERG_S: f64 = 1e-7;

/// A light-curve template: shock breakout, a plateau, and radioactive heating that the ejecta
/// release after a diffusion time and trap until the γ-rays escape. Every figure is a parameter
/// of the generator version, ours (Design note 18).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LightCurve {
    /// Shock breakout's peak, erg/s, and its e-folding time, s.
    breakout: (f64, f64),
    /// The plateau's luminosity, erg/s, its rise time, its end and its fall's width, days, and
    /// its decline's e-folding time, days (infinite for a flat plateau).
    plateau: Option<(f64, f64, f64, f64, f64)>,
    /// The nickel mass, M☉.
    nickel: f64,
    /// The diffusion time `τ_m`, days: the radioactive term rises as `1 − e^(−(t ÷ τ_m)²)`.
    diffusion_days: f64,
    /// The γ-ray escape time `t_γ`, days: the trapped share is `1 − e^(−(t_γ ÷ t)²)`.
    gamma_days: f64,
}

impl LightCurve {
    /// The template of `kind`, or `None` for a core collapse that made no supernova.
    ///
    /// | Kind          | Breakout      | Plateau                        | ⁵⁶Ni   | `τ_m` | `t_γ` |
    /// | ------------- | ------------- | ------------------------------ | ------ | ---- | ----- |
    /// | IIP           | 10⁴⁵, 2,000 s | 10⁴², to 100 d                 | 0.03   | 60 d | 400 d |
    /// | IIL           | 10⁴⁵, 2,000 s | 2 × 10⁴², falling 40 d, to 80 d | 0.05   | 40 d | 300 d |
    /// | IIb           | 10⁴⁵, 500 s   | none                           | 0.10   | 15 d | 150 d |
    /// | Ib, Ic        | 10⁴⁴, 100 s   | none                           | 0.10   | 12 d | 120 d |
    /// | Ia            | none          | none                           | 0.60   | 13 d | 35 d  |
    /// | Iax           | none          | none                           | 0.05   | 8 d  | 30 d  |
    ///
    /// The shapes follow the familiar types (plateau of a red supergiant, a Type Ia's peak near
    /// 1.1 × 10⁴³ erg/s about 15 days in); the numbers are typical values, not fits.
    ///
    /// # Examples
    ///
    /// A plateau supernova is still near its plateau a month in, and a Type Ia outshines it at its
    /// peak:
    ///
    /// ```
    /// use hyperion_sim::galaxy::catalogue_classes::supernova::{LightCurve, SupernovaKind};
    /// use hyperion_sim::galaxy::catalogue_classes::type_ia::IaChannel;
    /// use hyperion_sim::stellar::remnant::SupernovaType;
    /// use hyperion_sim::units::Years;
    ///
    /// let month = Years::new(30.0 / 365.25);
    /// let iip = LightCurve::luminosity(SupernovaKind::CoreCollapse(SupernovaType::IIP), month);
    /// assert!((0.8e35..2e35).contains(&iip.value())); // about 10⁴² erg/s, in W
    /// let ia = LightCurve::luminosity(SupernovaKind::TypeIa(IaChannel::Merger), Years::new(15.0 / 365.25));
    /// assert!(ia.value() > 5.0 * iip.value());
    /// ```
    #[must_use]
    pub const fn of(kind: SupernovaKind) -> Option<Self> {
        match kind {
            SupernovaKind::CoreCollapse(t) => match t {
                SupernovaType::IIP => Some(Self {
                    breakout: (1e45, 2_000.0),
                    plateau: Some((1e42, 2.0, 100.0, 3.0, f64::INFINITY)),
                    nickel: 0.03,
                    diffusion_days: 60.0,
                    gamma_days: 400.0,
                }),
                SupernovaType::IIL => Some(Self {
                    breakout: (1e45, 2_000.0),
                    plateau: Some((2e42, 3.0, 80.0, 5.0, 40.0)),
                    nickel: 0.05,
                    diffusion_days: 40.0,
                    gamma_days: 300.0,
                }),
                SupernovaType::IIb => Some(Self::rise(0.10, 15.0, 150.0, (1e45, 500.0))),
                SupernovaType::Ib | SupernovaType::Ic => {
                    Some(Self::rise(0.10, 12.0, 120.0, (1e44, 100.0)))
                }
                SupernovaType::None => None,
            },
            SupernovaKind::TypeIa(IaChannel::Iax) => Some(Self::rise(0.05, 8.0, 30.0, (0.0, 1.0))),
            SupernovaKind::TypeIa(
                IaChannel::Merger | IaChannel::DoubleDetonation | IaChannel::HydrogenDonor,
            ) => Some(Self::rise(0.60, 13.0, 35.0, (0.0, 1.0))),
        }
    }

    /// A template without a plateau: breakout, then a rise and a radioactive tail.
    const fn rise(nickel: f64, diffusion_days: f64, gamma_days: f64, breakout: (f64, f64)) -> Self {
        Self {
            breakout,
            plateau: None,
            nickel,
            diffusion_days,
            gamma_days,
        }
    }

    /// The bolometric electromagnetic luminosity of a supernova of `kind`, `age` after the
    /// explosion: zero before it and for a core collapse that made no supernova.
    #[must_use]
    pub fn luminosity(kind: SupernovaKind, age: Years) -> Watts {
        Self::of(kind).map_or(Watts::ZERO, |curve| curve.at(age))
    }

    /// The luminosity in neutrinos `age` after the explosion: a burst of 3 × 10⁵³ erg with an
    /// e-folding time of 3 s for every core collapse, failed ones included; zero for a Type Ia.
    #[must_use]
    pub fn neutrino_luminosity(kind: SupernovaKind, age: Years) -> Watts {
        let seconds = age.value() * crate::units::consts::SECONDS_PER_JULIAN_YEAR;
        match kind {
            SupernovaKind::CoreCollapse(_) if seconds >= 0.0 => Watts::new(
                NEUTRINO_ERG / NEUTRINO_SECONDS
                    * math::exp(-seconds / NEUTRINO_SECONDS)
                    * WATTS_PER_ERG_S,
            ),
            SupernovaKind::CoreCollapse(_) | SupernovaKind::TypeIa(_) => Watts::ZERO,
        }
    }

    /// This template's luminosity `age` after the explosion; zero before it.
    #[must_use]
    pub fn at(&self, age: Years) -> Watts {
        let seconds = age.value() * crate::units::consts::SECONDS_PER_JULIAN_YEAR;
        if seconds.is_nan() || seconds < 0.0 {
            return Watts::ZERO;
        }
        let days = seconds / SECONDS_PER_DAY;
        let (peak, fold) = self.breakout;
        let breakout = peak * math::exp(-seconds / fold);
        let plateau = self
            .plateau
            .map_or(0.0, |(level, rise, end, width, decline)| {
                let on = 1.0 - math::exp(-days / rise);
                let off = 1.0 / (1.0 + math::exp((days - end) / width));
                let fall = if decline.is_finite() {
                    math::exp(-days / decline)
                } else {
                    1.0
                };
                level * on * off * fall
            });
        let released =
            1.0 - math::exp(-(days / self.diffusion_days) * (days / self.diffusion_days));
        let trapped = if days > 0.0 {
            1.0 - math::exp(-(self.gamma_days / days) * (self.gamma_days / days))
        } else {
            1.0
        };
        let nickel = NICKEL_ERG_S * math::exp(-days / NICKEL_DAYS);
        let cobalt = COBALT_ERG_S
            * math::exp(-days / COBALT_DAYS)
            * ((1.0 - POSITRON_SHARE) * trapped + POSITRON_SHARE);
        let radioactive = self.nickel * released * (nickel * trapped + cobalt);
        Watts::new((breakout + plateau + radioactive) * WATTS_PER_ERG_S)
    }
}

/// How far an entry's fastest member may travel from its site over the entry's whole lifetime:
/// 4,096 ly, the reach of the catalogue classes' lookup, eight rings of 512 ly cells (P09.T32).
///
/// The brainstorm holds "the entry's lifetime ... to 4,096 ly ÷ its fastest member's speed, so
/// that lookups stay within a known number of rings of cells" ("The supernova test with a clock");
/// [`lifetime_cap`] turns it into a cap on the window. A parameter of the generator version.
pub const LIFETIME_REACH: LightYears = LightYears::new(4_096.0);

/// The clock's L + 2H, the part of an entry's lifetime that is not its window: 264,144 Julian
/// years.
#[must_use]
fn horizon_length() -> Span {
    LIGHT_CROSSING_L
        .checked_add(CLOCK_WINDOW_H)
        .and_then(|span| span.checked_add(CLOCK_WINDOW_H))
        .expect("L + 2H is 264,144 years of seconds, far inside a span's range")
}

/// The longest window an entry whose fastest member moves at `speed` (galactic frame) may have, so
/// that its whole lifetime, `W_eff` + L + 2H, keeps that member within [`LIFETIME_REACH`] of its
/// site: 4,096 ly ÷ `speed` − (L + 2H), in Julian years.
///
/// Light covers a light-year in a Julian year, so 4,096 ly ÷ v is 4,096 × c ÷ v years. It is
/// negative above 4,096 ly ÷ (L + 2H), about 4,649 km/s, where the test cuts the window to nothing,
/// and infinite at rest. A survivor at 2,500 km/s has 4,096 × 299,792.458 ÷ 2,500 = 491,179.96
/// years, which leaves 227,035.96 for the window, under the plan's 0.23 Myr.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::catalogue_classes::supernova::lifetime_cap;
/// use hyperion_sim::units::{KilometresPerSecond, MetresPerSecond};
///
/// let fast = lifetime_cap(MetresPerSecond::from(KilometresPerSecond::new(2_500.0)));
/// assert!((fast.value() - 227_035.963).abs() < 1e-3);
/// assert!(lifetime_cap(MetresPerSecond::ZERO).value().is_infinite());
/// ```
#[must_use]
pub fn lifetime_cap(speed: MetresPerSecond) -> Years {
    let crossing = LIFETIME_REACH.value() * SPEED_OF_LIGHT / speed.value();
    Years::new(crossing - horizon_length().as_julian_years_f64())
}

/// What the supernova test reads of one death (plan 09, P09.T17): whose it is, when the star
/// explodes, as what, with what energy, and the fastest galactic-frame speed any member of the
/// entry then has.
///
/// Built by [`DeathMarks::of`] from a system's stars, the one constructor that the cells, the
/// displaced bins, the feature members and the catalogue's core collapses all use, or by
/// [`DeathMarks::of_type_ia`] from a Type Ia candidate's progenitor, which has no stars to read.
///
/// The energy is one uniform of the `snr.energy` stream keyed by the system's ID
/// (`ObjectKey::from(SystemId)`), word 0, through the law of its death: an iron core's collapse's
/// and a normal Type Ia's [`ExplosionEnergy::from_uniform`], an electron-capture supernova's
/// [`ExplosionEnergy::electron_capture_from_uniform`], a Type Iax's
/// [`ExplosionEnergy::iax_from_uniform`] (ruling 136.10). Words 1–7 are reserved.
///
/// The plan's sketch also carried the star's metallicity. Ruling 136.9 has the window read the
/// site gas's instead, which [`claims`] reads at the site, so the marks hold none.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeathMarks {
    system: SystemId,
    death_time: UniverseTime,
    kind: SupernovaKind,
    energy: ExplosionEnergy,
    fastest_member_speed: MetresPerSecond,
}

impl DeathMarks {
    /// The marks of the death of `stars`'s primary at `site`, for a system whose galactic-frame
    /// velocity at the death is `velocity`, or `None` for a death that makes no supernova shell.
    ///
    /// - T is the primary's [`SystemStars::death_time`], and the kind its death's
    ///   ([`SupernovaKind::of_death`]): a white dwarf's birth, a collapse with no supernova, a
    ///   disruption and a death off the clock give `None`.
    /// - The fastest member is the compact remnant, at `velocity` plus the natal kick
    ///   ([`SystemStars::natal_kick`]), or at `velocity` where it has none. The system's companions
    ///   are not members of the entry and are not read (plan 11's binaries).
    /// - The energy is the `snr.energy` draw (the type's documentation), through the
    ///   electron-capture law for an electron-capture death.
    ///
    /// `velocity` is the caller's: the grid passes plan 08's `epoch_velocity` of its record, a
    /// feature member its feature's bulk motion plus its own, a catalogue candidate the velocity it
    /// drew. The site is where plan 08's
    /// [`death_in_interval`](crate::galaxy::displaced::death_in_interval) put the death, whose
    /// time is [`SystemStars::death_time`]'s to the nanosecond.
    ///
    /// # Panics
    ///
    /// In debug builds, if `site`'s death is not the primary's T: a side whose site and stars
    /// disagree would split the shared test.
    #[must_use]
    pub fn of(
        galaxy: &Galaxy,
        stars: &SystemStars,
        site: &ExplosionSite,
        velocity: GalacticVelocity,
    ) -> Option<Self> {
        let ClockDeath::At(death_time, _) = stars.death_time() else {
            return None;
        };
        let death = stars.primary().death()?;
        let kind = SupernovaKind::of_death(&death)?;
        debug_assert_eq!(
            site.death(),
            death_time,
            "the site's death is the primary's: {:?}",
            stars.record().id()
        );
        let after = stars.natal_kick().map_or(velocity, |kick| {
            velocity + GalacticVelocity::new(kick.velocity())
        });
        let system = stars.record().id();
        Some(Self {
            system,
            death_time,
            kind,
            energy: drawn_energy(galaxy, system, EnergyLaw::of_death(&death)),
            fastest_member_speed: after.speed(),
        })
    }

    /// The marks of a Type Ia candidate `system` (plan 09, P09.T34.a) whose `progenitor` explodes
    /// at `site` leaving `leftover`, the system moving at `velocity` (galactic frame) before the
    /// explosion.
    ///
    /// T is the progenitor's explosion and the kind its channel. The fastest member is the
    /// survivor, the entry's member 1, whose speed [`IaLeftover::speed`] gives from the site but in
    /// no direction, so it is bounded by |`velocity`| + that speed; with no survivor that flies
    /// off (a merger destroys both stars, a Type Iax leaves its partly burnt dwarf at the site) it
    /// is |`velocity`|. The energy is the `snr.energy` draw, through the Iax law for a Type Iax.
    ///
    /// A recent Type Ia has no grid side to agree with: plan 11's binaries redraw any explosion
    /// before +H (P09.T35), so this second constructor splits no shared test.
    ///
    /// # Panics
    ///
    /// In debug builds, if `site`'s death is not the progenitor's explosion.
    ///
    /// # Examples
    ///
    /// A Type Ia candidate drawn from the galaxy's delay-time distribution to explode within the
    /// clock window is an entry, in the smooth gas at its site:
    ///
    /// ```
    /// use hyperion_sim::Seed;
    /// use hyperion_sim::coords::{GalacticPosition, GalacticVelocity};
    /// use hyperion_sim::galaxy::catalogue_classes::supernova::{DeathMarks, Surroundings, claims};
    /// use hyperion_sim::galaxy::catalogue_classes::type_ia::{
    ///     DelayTimeDistribution, IaLeftover, IaProgenitor,
    /// };
    /// use hyperion_sim::galaxy::displaced::ExplosionSite;
    /// use hyperion_sim::galaxy::{Galaxy, Population};
    /// use hyperion_sim::id::SystemId;
    /// use hyperion_sim::rng::{ObjectKey, Stream, tags};
    /// use hyperion_sim::time::{ClockWindow, UniverseTime};
    ///
    /// let galaxy = Galaxy::new(Seed::new(9));
    /// let system = SystemId::from_raw(0xF000_0007_0000_0000)?;
    /// let key = ObjectKey::from(system);
    /// let delays = DelayTimeDistribution::from_galaxy(&galaxy);
    /// let after = UniverseTime::from_julian_years(-1_000).ok_or("on the clock")?;
    /// let mut stream = Stream::open(galaxy.seed(), tags::CLASS_TYPE_IA_PROGENITOR, key);
    /// let progenitor =
    ///     IaProgenitor::draw(&delays, &mut stream, Population::OldThinDisc, after, ClockWindow::END)
    ///         .ok_or("the old thin disc has Type Ia")?;
    /// let mut stream = Stream::open(galaxy.seed(), tags::CLASS_TYPE_IA_LEFTOVER, key);
    /// let leftover = IaLeftover::draw(progenitor.channel(), &mut stream);
    /// let place = GalacticPosition::from_light_years([0.0, 26_000.0, 300.0]).ok_or("in the cube")?;
    /// let site = ExplosionSite::new(place, progenitor.explosion());
    /// let velocity = GalacticVelocity::new([0.0, -220e3, 10e3]);
    /// let marks = DeathMarks::of_type_ia(&galaxy, system, &progenitor, &leftover, &site, velocity);
    /// // Within L + H of the epoch a death is claimed whatever its window.
    /// assert!(claims(&galaxy, &site, &marks, Surroundings::TypeIa));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn of_type_ia(
        galaxy: &Galaxy,
        system: SystemId,
        progenitor: &IaProgenitor,
        leftover: &IaLeftover,
        site: &ExplosionSite,
        velocity: GalacticVelocity,
    ) -> Self {
        debug_assert_eq!(
            site.death(),
            progenitor.explosion(),
            "the site's death is the progenitor's explosion: {system:?}"
        );
        let kind = SupernovaKind::TypeIa(progenitor.channel());
        let survivor = leftover
            .speed()
            .map_or(0.0, |speed| MetresPerSecond::from(speed).value());
        Self {
            system,
            death_time: progenitor.explosion(),
            kind,
            energy: drawn_energy(galaxy, system, EnergyLaw::of_channel(progenitor.channel())),
            fastest_member_speed: MetresPerSecond::new(velocity.speed().value() + survivor),
        }
    }

    /// The marks with every field given: for the tests' sweeps over marks no system has.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn from_parts(
        system: SystemId,
        death_time: UniverseTime,
        kind: SupernovaKind,
        energy: ExplosionEnergy,
        fastest_member_speed: MetresPerSecond,
    ) -> Self {
        Self {
            system,
            death_time,
            kind,
            energy,
            fastest_member_speed,
        }
    }

    /// The system whose star explodes: the entry's member 0.
    #[must_use]
    pub const fn system(&self) -> SystemId {
        self.system
    }

    /// T, the clock time of the explosion.
    #[must_use]
    pub const fn death_time(&self) -> UniverseTime {
        self.death_time
    }

    /// What explodes.
    #[must_use]
    pub const fn kind(&self) -> SupernovaKind {
        self.kind
    }

    /// The explosion's kinetic energy.
    #[must_use]
    pub const fn energy(&self) -> ExplosionEnergy {
        self.energy
    }

    /// `v_fast`: the fastest galactic-frame speed any member of the entry has after T, or a bound
    /// on it.
    #[must_use]
    pub const fn fastest_member_speed(&self) -> MetresPerSecond {
        self.fastest_member_speed
    }
}

/// Which of [`ExplosionEnergy`]'s laws an explosion's energy is drawn from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EnergyLaw {
    /// An iron core's collapse or a normal Type Ia: [`ExplosionEnergy::from_uniform`].
    Standard,
    /// An oxygen–neon core's collapse by electron capture:
    /// [`ExplosionEnergy::electron_capture_from_uniform`].
    ElectronCapture,
    /// A Type Iax: [`ExplosionEnergy::iax_from_uniform`].
    Iax,
}

impl EnergyLaw {
    /// The law of a single star's `death`: electron capture's for an electron-capture death, the
    /// standard law for every other.
    #[must_use]
    fn of_death(death: &Death) -> Self {
        if death.kind() == DeathKind::ElectronCapture {
            Self::ElectronCapture
        } else {
            Self::Standard
        }
    }

    /// The law of a Type Ia of `channel`: the Iax law for a Type Iax, the standard law for the
    /// other three.
    #[must_use]
    fn of_channel(channel: IaChannel) -> Self {
        match channel {
            IaChannel::Iax => Self::Iax,
            IaChannel::Merger | IaChannel::DoubleDetonation | IaChannel::HydrogenDonor => {
                Self::Standard
            }
        }
    }

    /// The energy at rank `u` of this law.
    #[must_use]
    fn energy(self, u: f64) -> ExplosionEnergy {
        match self {
            Self::Standard => ExplosionEnergy::from_uniform(u),
            Self::ElectronCapture => ExplosionEnergy::electron_capture_from_uniform(u),
            Self::Iax => ExplosionEnergy::iax_from_uniform(u),
        }
    }
}

/// The energy of `system`'s explosion under `law`: word 0 of its `snr.energy` stream, one uniform
/// ([`DeathMarks`]).
#[must_use]
fn drawn_energy(galaxy: &Galaxy, system: SystemId, law: EnergyLaw) -> ExplosionEnergy {
    let u = Stream::open(galaxy.seed(), tags::SNR_ENERGY, ObjectKey::from(system)).uniform();
    law.energy(u)
}

impl SupernovaKind {
    /// The supernova a single star's `death` makes, or `None` for a death that makes no shell
    /// (P09.T17).
    ///
    /// | Death ([`DeathKind`])     | Supernova                                                   |
    /// | ------------------------- | ----------------------------------------------------------- |
    /// | `CoreCollapse`            | that collapse's type, which plan 06 sets from the envelopes |
    /// | `ElectronCapture`         | a core collapse typed by the same rule                      |
    /// | `EnvelopeLoss`            | none: a white dwarf is born, with no explosion              |
    /// | `DirectCollapse`          | none: complete fallback to a black hole, with no supernova  |
    /// | `PairInstability`         | none: not modelled as an entry (below)                      |
    /// | `ThermonuclearDisruption` | none: not modelled as an entry (below)                      |
    ///
    /// An electron-capture death carries no type of its own, so it takes plan 06's
    /// [`SupernovaType`] rule on the progenitor's hydrogen envelope and the helium outside its
    /// carbon–oxygen core at death, the rule that types an iron core's collapse (P06.T10.e,
    /// ruling 46). A super-AGB progenitor keeps its hydrogen envelope, so it is a Type II, as the
    /// electron-capture candidate SN 2018zd is, "a Type II-P (plateau) SN" (Hiramatsu et al. 2021,
    /// Nature Astronomy 5, 903). A
    /// companion-stripped mark leaves the envelope a single star's until plan 11, as it does for
    /// an iron core's type.
    ///
    /// Two deaths that explode are left on the grid, with no entry and no shell, and are not
    /// modelled here (ours): a pair-instability supernova, whose 5–94 × 10⁵¹ erg (Heger and
    /// Woosley 2002, ApJ 567, 532, §3.1 and their Fig. 1) lies far above [`ExplosionEnergy`]'s
    /// range and so above every window the caps allow for, and a single
    /// star's thermonuclear disruption (the carbon ignition of Hurley, Pols and Tout 2000, MNRAS
    /// 315, 543, section 6), which none of the catalogue's Type Ia channels describes. A
    /// `CoreCollapse` of [`SupernovaType::None`] gives none too, though plan 06's track never makes
    /// one.
    #[must_use]
    pub fn of_death(death: &Death) -> Option<Self> {
        match death.kind() {
            DeathKind::CoreCollapse { supernova } => match supernova {
                SupernovaType::None => None,
                SupernovaType::IIP
                | SupernovaType::IIL
                | SupernovaType::IIb
                | SupernovaType::Ib
                | SupernovaType::Ic => Some(Self::CoreCollapse(supernova)),
            },
            DeathKind::ElectronCapture => {
                let progenitor = death.progenitor();
                let helium = (progenitor.helium_core_mass().value()
                    - progenitor.co_core_mass().value())
                .max(0.0);
                Some(Self::CoreCollapse(SupernovaType::of_envelopes(
                    progenitor.envelope_mass(),
                    SolarMasses::new(helium),
                )))
            }
            DeathKind::EnvelopeLoss
            | DeathKind::DirectCollapse
            | DeathKind::PairInstability
            | DeathKind::ThermonuclearDisruption => None,
        }
    }
}

/// A superbubble's interior as a shell inside it reads it: the hot gas it explodes into and how far
/// the site is from the bubble's wall (P09.T17, for P09.T36.a's members).
///
/// The caller builds it from the member's own parent feature: [`SiteGas::hot_interior`] of the
/// bubble's interior density and temperature, and the bubble's radius less the site's distance
/// from its centre. A negative or NaN distance ends the shell at once.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BubbleInterior {
    gas: SiteGas,
    wall: LightYears,
}

impl BubbleInterior {
    /// The interior of hot `gas` whose wall lies `wall` from the site.
    #[must_use]
    pub const fn new(gas: SiteGas, wall: LightYears) -> Self {
        Self { gas, wall }
    }

    /// The hot gas inside the bubble.
    #[must_use]
    pub const fn gas(&self) -> &SiteGas {
        &self.gas
    }

    /// The distance from the site to the bubble's wall.
    #[must_use]
    pub const fn wall(&self) -> LightYears {
        self.wall
    }
}

/// Where an explosion happens, as the supernova test reads its gas: in the smooth field at the
/// site, or inside a superbubble whose interior the caller passes (P09.T17).
///
/// [`ShellEnvironment`] stays fieldless, since the caps and their scan use it; this adds the one
/// environment's state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Surroundings {
    /// A core collapse in field gas: the young field, the runaways and walkaways.
    Field,
    /// A Type Ia, in the smooth gas at its site.
    TypeIa,
    /// A core collapse inside a nursery's superbubble, with its interior.
    Bubble(BubbleInterior),
}

impl Surroundings {
    /// The environment whose cap the window is cut at.
    #[must_use]
    pub const fn environment(&self) -> ShellEnvironment {
        match self {
            Self::Field => ShellEnvironment::Field,
            Self::TypeIa => ShellEnvironment::TypeIa,
            Self::Bubble(_) => ShellEnvironment::Bubble,
        }
    }
}

/// The slots of the noise cache one site's gas is read with: none, since a cache changes what the
/// read costs, never what it returns, and one read has nothing to reuse.
const SITE_CACHE_SLOTS: usize = 0;

/// The shell window of the death `marks` at `site`, as the supernova test cuts it: its duration is
/// `W_eff` (P09.T17).
///
/// `W` is [`shell_window`] of the site's gas, the marks' energy and the site gas's metallicity
/// ([`GasField::metallicity_at`](crate::galaxy::gas::field::GasField::metallicity_at), which the
/// window holds to −1…+0.5 dex), not the star's (ruling 136.9). CMB88's ζ is the metallicity
/// factor of the cooling function, "`ζ_m` = 1 for solar abundances" (Cioffi, McKee and Bertschinger
/// 1988, ApJ 334, 252, p. 259), and the gas that cools is the swept-up ambient's. The
/// gas is the smooth field's at the site, [`SiteGas::at`], with no modifiers, or, inside a
/// superbubble, the interior the caller passes, the window then ended where the shell reaches the
/// wall ([`ShellWindow::ended_at_wall`]). `W_eff` = min(`W`, the environment's cap,
/// [`lifetime_cap`] of the marks' fastest member), floored at zero, the cap being the galaxy's
/// ([`Galaxy::window_caps`]).
///
/// # Panics
///
/// In debug builds, if `W` exceeds its environment's cap ([`WindowCaps::cut`]): a bubble interior
/// outside the gas the bubble's cap was scanned over, or a bug in the scan.
#[must_use]
pub fn effective_window(
    galaxy: &Galaxy,
    site: &ExplosionSite,
    marks: &DeathMarks,
    surroundings: Surroundings,
) -> ShellWindow {
    let metallicity = galaxy.gas().metallicity_at(site.position());
    let window = match surroundings {
        Surroundings::Field | Surroundings::TypeIa => {
            let mut cache = NoiseCache::with_capacity(SITE_CACHE_SLOTS);
            let gas = SiteGas::at(galaxy.gas(), site.position(), &mut cache);
            shell_window(&gas, marks.energy(), metallicity)
        }
        Surroundings::Bubble(interior) => {
            shell_window(interior.gas(), marks.energy(), metallicity).ended_at_wall(interior.wall())
        }
    };
    galaxy
        .window_caps()
        .cut(surroundings.environment(), window)
        .ended_at(lifetime_cap(marks.fastest_member_speed()))
}

/// The supernova test with a clock (plan 09, P09.T17): whether the death `marks` at `site` is a
/// supernova catalogue entry, true if and only if −(`W_eff` + L + H) < T ≤ +H.
///
/// `W_eff` is [`effective_window`]'s, rounded to the clock as [`SupernovaInterval::of_window`]
/// does, and T the marks' death. It takes no time: membership does not depend on when the entry is
/// evaluated, so the entry has one ID on both sides of the explosion. It reads the smooth gas
/// field at the site, the marks and the galaxy's caps, never plan 07's modifiers, a feature or
/// another system.
///
/// It is the second half of the one shared test: every side calls plan 08's
/// [`death_in_interval`](crate::galaxy::displaced::death_in_interval) on its own candidate's
/// primary, and keeps or carves an entry only if that is `Some` and this is true. The interval it
/// tests lies inside that function's, (−(`SHELL_WINDOW_CAP` + L + H), +H], since no cap exceeds
/// [`SHELL_WINDOW_CAP`]. A class that also needs the entry's window, to build its
/// [`SupernovaEntry`], asks [`claimed_window`], whose answer this is.
///
/// # Panics
///
/// In debug builds, as [`effective_window`].
///
/// # Examples
///
/// A star that exploded ten thousand years ago is an entry; one that exploded ten million years
/// ago, long after its shell merged, is not:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::GalacticVelocity;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::catalogue_classes::supernova::{DeathMarks, Surroundings, claims};
/// use hyperion_sim::galaxy::displaced::ExplosionSite;
/// use hyperion_sim::galaxy::placement::{CellKey, SystemRecord, generate_cell};
/// use hyperion_sim::id::Layer;
/// use hyperion_sim::stellar::system::{ClockDeath, SystemStars};
/// use hyperion_sim::units::Years;
///
/// let galaxy = Galaxy::new(Seed::new(19));
/// let mut cell = Vec::new();
/// generate_cell(&galaxy, CellKey::new(Layer::E, [0, 203, 0])?, &mut cell);
/// let record = cell.first().ok_or("the cell holds a system")?;
/// let ClockDeath::At(death, _) = SystemStars::generate(&galaxy, record).death_time() else {
///     return Err("a layer-E primary dies on the clock".into());
/// };
/// let lifetime = death.since_epoch().as_julian_years_f64() + record.age_at_epoch().value();
/// let dead_for = |years: f64| -> Option<bool> {
///     let dying = SystemRecord::from_parts(
///         record.id(),
///         *record.epoch_position(),
///         record.origin(),
///         record.population(),
///         record.primary_initial_mass(),
///         Years::new(lifetime + years),
///     );
///     let stars = SystemStars::generate(&galaxy, &dying);
///     let ClockDeath::At(t, _) = stars.death_time() else {
///         return None;
///     };
///     let site = ExplosionSite::new(*dying.epoch_position(), t);
///     let marks = DeathMarks::of(&galaxy, &stars, &site, GalacticVelocity::default())?;
///     Some(claims(&galaxy, &site, &marks, Surroundings::Field))
/// };
/// assert_eq!(dead_for(1e4), Some(true));
/// assert_eq!(dead_for(1e7), Some(false));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn claims(
    galaxy: &Galaxy,
    site: &ExplosionSite,
    marks: &DeathMarks,
    surroundings: Surroundings,
) -> bool {
    claimed_window(galaxy, site, marks, surroundings).is_some()
}

/// The shell window of a claimed death, as [`effective_window`] cuts it, or `None` where
/// [`claims`] is false: the one route by which a class both tests a death and gets the window
/// its [`SupernovaEntry`] is built with (P09.T33–T36).
///
/// # Panics
///
/// In debug builds, as [`effective_window`].
///
/// # Examples
///
/// A class keeps a claimed candidate as an entry with the window the test cut:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::{GalacticPosition, GalacticVelocity};
/// use hyperion_sim::galaxy::catalogue_classes::supernova::{
///     DeathMarks, SupernovaEntry, Surroundings, claimed_window,
/// };
/// use hyperion_sim::galaxy::catalogue_classes::type_ia::{
///     DelayTimeDistribution, IaLeftover, IaProgenitor,
/// };
/// use hyperion_sim::galaxy::displaced::ExplosionSite;
/// use hyperion_sim::galaxy::{Galaxy, Population};
/// use hyperion_sim::id::SystemId;
/// use hyperion_sim::rng::{ObjectKey, Stream, tags};
/// use hyperion_sim::time::{ClockWindow, UniverseTime};
///
/// let galaxy = Galaxy::new(Seed::new(9));
/// let system = SystemId::from_raw(0xF000_0007_0000_0000)?;
/// let key = ObjectKey::from(system);
/// let delays = DelayTimeDistribution::from_galaxy(&galaxy);
/// let after = UniverseTime::from_julian_years(-1_000).ok_or("on the clock")?;
/// let mut stream = Stream::open(galaxy.seed(), tags::CLASS_TYPE_IA_PROGENITOR, key);
/// let progenitor =
///     IaProgenitor::draw(&delays, &mut stream, Population::OldThinDisc, after, ClockWindow::END)
///         .ok_or("the old thin disc has Type Ia")?;
/// let mut stream = Stream::open(galaxy.seed(), tags::CLASS_TYPE_IA_LEFTOVER, key);
/// let leftover = IaLeftover::draw(progenitor.channel(), &mut stream);
/// let place = GalacticPosition::from_light_years([0.0, 26_000.0, 300.0]).ok_or("in the cube")?;
/// let site = ExplosionSite::new(place, progenitor.explosion());
/// let velocity = GalacticVelocity::new([0.0, -220e3, 10e3]);
/// let marks = DeathMarks::of_type_ia(&galaxy, system, &progenitor, &leftover, &site, velocity);
///
/// let window =
///     claimed_window(&galaxy, &site, &marks, Surroundings::TypeIa).ok_or("a recent explosion")?;
/// let entry = SupernovaEntry::new(marks.system(), marks.death_time(), marks.kind(), window)
///     .with_progenitor(progenitor)
///     .with_leftover(leftover);
/// assert_eq!(entry.window(), &window);
/// assert!(window.duration() <= window.natural_duration());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn claimed_window(
    galaxy: &Galaxy,
    site: &ExplosionSite,
    marks: &DeathMarks,
    surroundings: Surroundings,
) -> Option<ShellWindow> {
    let window = effective_window(galaxy, site, marks, surroundings);
    SupernovaInterval::of_window(window.duration())
        .contains(marks.death_time())
        .then_some(window)
}

/// The interval of deaths a window `W` admits, (−(`W` + L + H), +H] on the universe clock: an
/// entry's, at its `W_eff`, and a class's thinning envelope's, at its environment's cap (P09.T17,
/// for P09.T32–T36).
///
/// `W` is rounded to the clock by flooring to the nanosecond, as every clock time from a float of
/// seconds is ([`Span::from_seconds_f64`]), so the interval never opens before its window allows,
/// and it is held to 0…[`SHELL_WINDOW_CAP`], so every interval lies inside plan 08's
/// [`death_in_interval`](crate::galaxy::displaced::death_in_interval)'s, which opens at
/// −(`SHELL_WINDOW_CAP` + L + H), 134,534,593,094,400 s before the epoch. The start is excluded
/// and the end, +H, included.
///
/// # Examples
///
/// The field's thinning envelope draws its deaths over the interval at the field's cap, whose
/// length is the thinning cap:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::catalogue_classes::supernova::SupernovaInterval;
/// use hyperion_sim::galaxy::snr::ShellEnvironment;
/// use hyperion_sim::time::ClockWindow;
///
/// let galaxy = Galaxy::new(Seed::new(9));
/// let field = SupernovaInterval::of_cap(galaxy.window_caps(), ShellEnvironment::Field);
/// assert_eq!(field.end(), ClockWindow::END);
/// assert!(field.contains(ClockWindow::END));
/// assert!(!field.contains(field.start()));
/// let length = ClockWindow::END.checked_since(field.start()).ok_or("on the clock")?;
/// assert_eq!(
///     length,
///     SupernovaInterval::thinning_cap(galaxy.window_caps(), ShellEnvironment::Field)
/// );
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SupernovaInterval {
    window: Span,
}

impl SupernovaInterval {
    /// The interval of a window of `window` Julian years, held to 0…[`SHELL_WINDOW_CAP`] (a NaN
    /// reads as zero) and floored to the nanosecond.
    ///
    /// # Panics
    ///
    /// Never: a window of at most 4 Myr is far inside the clock's range.
    #[must_use]
    pub fn of_window(window: Years) -> Self {
        let years = if window.value().is_nan() {
            0.0
        } else {
            window.value().clamp(0.0, SHELL_WINDOW_CAP.value())
        };
        let window = Span::from_seconds_f64(years * SECONDS_PER_JULIAN_YEAR)
            .expect("a window of at most 4 Myr is a few 10¹⁴ s");
        Self { window }
    }

    /// The interval at `environment`'s cap in `caps`: the widest any entry of that environment
    /// has, over which a class's thinning envelope draws its deaths (P09.T33.a).
    #[must_use]
    pub fn of_cap(caps: &WindowCaps, environment: ShellEnvironment) -> Self {
        Self::of_window(caps.cap(environment))
    }

    /// The thinning cap of phase 7: `environment`'s cap grown by L + 2H, the length of
    /// [`of_cap`](Self::of_cap)'s interval.
    #[must_use]
    pub fn thinning_cap(caps: &WindowCaps, environment: ShellEnvironment) -> Span {
        Self::of_cap(caps, environment).length()
    }

    /// The window `W` on the clock.
    #[must_use]
    pub const fn window(&self) -> Span {
        self.window
    }

    /// The start, −(`W` + L + H), which the interval excludes.
    ///
    /// # Panics
    ///
    /// Never: 4.3 Myr of seconds is far inside the clock's range.
    #[must_use]
    pub fn start(&self) -> UniverseTime {
        self.window
            .checked_add(LIGHT_CROSSING_L)
            .and_then(|span| span.checked_add(CLOCK_WINDOW_H))
            .and_then(|span| UniverseTime::EPOCH.checked_sub(span))
            .expect("4.3 Myr of seconds is far inside the clock's range")
    }

    /// The end, +H, which the interval includes.
    #[must_use]
    pub const fn end(&self) -> UniverseTime {
        ClockWindow::END
    }

    /// The interval's length, `W` + L + 2H: an entry's whole lifetime.
    ///
    /// # Panics
    ///
    /// Never, as [`start`](Self::start).
    #[must_use]
    pub fn length(&self) -> Span {
        self.window
            .checked_add(horizon_length())
            .expect("4.3 Myr of seconds is far inside a span's range")
    }

    /// Whether the clock time `t` lies in the interval: after the start, and at or before the end.
    #[must_use]
    pub fn contains(&self, t: UniverseTime) -> bool {
        self.start() < t && t <= self.end()
    }
}

#[cfg(test)]
mod clock_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Seed;
    use crate::galaxy::Galaxy;
    use crate::galaxy::Population;
    use crate::galaxy::catalogue_classes::type_ia::DelayTimeDistribution;
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::snr::{ExplosionEnergy, SiteGas, shell_window};
    use crate::id::EventId;
    use crate::rng::{ObjectKey, Stream, tags};
    use crate::stellar::draws::StarDraws;
    use crate::time::CLOCK_WINDOW_H;
    use crate::units::{Dex, HydrogenPerCm3, KelvinPerCm3};

    const ALL_KINDS: [SupernovaKind; 9] = [
        SupernovaKind::CoreCollapse(SupernovaType::IIP),
        SupernovaKind::CoreCollapse(SupernovaType::IIL),
        SupernovaKind::CoreCollapse(SupernovaType::IIb),
        SupernovaKind::CoreCollapse(SupernovaType::Ib),
        SupernovaKind::CoreCollapse(SupernovaType::Ic),
        SupernovaKind::TypeIa(IaChannel::Merger),
        SupernovaKind::TypeIa(IaChannel::DoubleDetonation),
        SupernovaKind::TypeIa(IaChannel::HydrogenDonor),
        SupernovaKind::TypeIa(IaChannel::Iax),
    ];

    fn system() -> SystemId {
        SystemId::from_raw(0xF000_0007_0000_0000).unwrap()
    }

    fn years(t: f64) -> UniverseTime {
        let span =
            Span::from_seconds_f64(t * crate::units::consts::SECONDS_PER_JULIAN_YEAR).unwrap();
        UniverseTime::EPOCH.checked_add(span).unwrap()
    }

    fn core_collapse(t: UniverseTime) -> SupernovaEntry {
        let site = SiteGas::uniform(HydrogenPerCm3::new(1.0), KelvinPerCm3::new(3_800.0)).unwrap();
        let window = shell_window(&site, ExplosionEnergy::MEDIAN, Dex::new(0.0));
        let pulsar = NeutronStar::from_draws(&StarDraws::median());
        SupernovaEntry::new(
            system(),
            t,
            SupernovaKind::CoreCollapse(SupernovaType::IIP),
            window,
        )
        .with_remnant(KilometresPerSecond::new(400.0), Some(pulsar))
    }

    /// P09.T19.a: the state is continuous on either side of T except at the explosion, and the
    /// entry keeps its ID; the state changes kind at T and at T + W.
    #[test]
    fn the_state_is_continuous_on_either_side_of_the_explosion() {
        let explosion = years(300.0);
        let entry = core_collapse(explosion);
        let w = entry.window().duration().value();
        let mut last_radius = 0.0;
        let mut last_offset = 0.0;
        let mut kinds = Vec::new();
        for k in 0..=20_000 {
            let t = years(-1_000.0 + f64::from(k) * (w + 2e4) / 20_000.0);
            let state = entry.state_at(t);
            let label = match state {
                SupernovaState::Progenitor {
                    until_explosion, ..
                } => {
                    assert!(!until_explosion.is_negative());
                    0
                }
                SupernovaState::Supernova { shell, remnant, .. } => {
                    let r = shell.radius().value();
                    let o = remnant.offset().unwrap().value();
                    // A step of about 43 years moves a shell faster than 10⁴ km/s by at most
                    // 1.5 ly, and a remnant at 400 km/s by 0.06 ly.
                    assert!(
                        r >= last_radius && r - last_radius < 1.5,
                        "{r} after {last_radius}"
                    );
                    assert!(
                        o >= last_offset && o - last_offset < 0.1,
                        "{o} after {last_offset}"
                    );
                    last_radius = r;
                    last_offset = o;
                    1
                }
                SupernovaState::BareRemnant { remnant, .. } => {
                    let o = remnant.offset().unwrap().value();
                    assert!(o >= last_offset && o - last_offset < 0.1);
                    assert!(!remnant.bow_shock());
                    last_offset = o;
                    2
                }
            };
            if kinds.last() != Some(&label) {
                kinds.push(label);
            }
        }
        assert_eq!(kinds, vec![0, 1, 2]);
        // One ID on both sides: the event and the entry name member 0 before and after T.
        assert_eq!(entry.system(), system());
        assert_eq!(entry.explosion_event().subject().system(), system());
        // Before T the time left falls by exactly each step.
        let step = Span::from_julian_years(1).unwrap();
        let mut t = years(-50.0);
        let mut last: Option<Span> = None;
        while t < explosion {
            let SupernovaState::Progenitor {
                until_explosion, ..
            } = entry.state_at(t)
            else {
                panic!("a progenitor before T");
            };
            if let Some(before) = last {
                assert_eq!(before.checked_sub(step), Some(until_explosion));
            }
            last = Some(until_explosion);
            t = t.checked_add(step).unwrap();
        }
        // At T the entry is a supernova of age zero.
        assert!(matches!(
            entry.state_at(explosion),
            SupernovaState::Supernova { age, .. } if age == Years::ZERO
        ));
    }

    /// P09.T19.a: the explosion's event ID parses and round-trips, and names the entry.
    #[test]
    fn the_explosion_event_round_trips() {
        let entry = core_collapse(UniverseTime::EPOCH);
        let event = entry.explosion_event();
        let text = event.to_string();
        assert_eq!(text.parse::<EventId>().unwrap(), event);
        assert_eq!(text, "f000000700000000:0202000000000000");
        assert_eq!(event.word().tag(), SUPERNOVA);
        assert_eq!(event.word().bin().get(), 0);
        assert_eq!(event.word().number(), 0);
        assert_eq!(event.subject().system(), entry.system());
        assert_eq!(SUPERNOVA.name(), "class.ev.supernova");
    }

    /// A Type Ia entry reports its pair's period before the explosion and its survivor after.
    #[test]
    fn a_type_ia_entry_reports_its_pair_and_its_survivor() {
        let galaxy =
            Galaxy::from_params(Seed::new(0x0919_0001), GalaxyParams::milky_way_like()).unwrap();
        let delays = DelayTimeDistribution::from_galaxy(&galaxy);
        let after = years(-1e5);
        let until = UniverseTime::EPOCH.checked_add(CLOCK_WINDOW_H).unwrap();
        let progenitor = (0..200_u64)
            .filter_map(|k| {
                let mut s = Stream::open(
                    Seed::new(9),
                    tags::SELFTEST_STREAM,
                    ObjectKey::galaxy_item(k),
                );
                IaProgenitor::draw(&delays, &mut s, Population::OldThinDisc, after, until)
            })
            .find(|p| p.channel() == IaChannel::DoubleDetonation)
            .unwrap();
        let site = SiteGas::uniform(HydrogenPerCm3::new(0.1), KelvinPerCm3::new(3_000.0)).unwrap();
        let window = shell_window(&site, ExplosionEnergy::MEDIAN, Dex::new(0.0));
        let leftover = IaLeftover::SurvivingDonor {
            speed: KilometresPerSecond::new(1_200.0),
        };
        let entry = SupernovaEntry::new(
            system(),
            progenitor.explosion(),
            SupernovaKind::TypeIa(progenitor.channel()),
            window,
        )
        .with_progenitor(progenitor)
        .with_leftover(leftover);
        let before = progenitor
            .explosion()
            .checked_sub(Span::from_julian_years(1_000).unwrap())
            .unwrap();
        let SupernovaState::Progenitor { pair_period, .. } = entry.state_at(before) else {
            panic!("a progenitor before the explosion");
        };
        assert!(pair_period.unwrap().value() > 10.0);
        let later = progenitor
            .explosion()
            .checked_add(Span::from_julian_years(1_000).unwrap())
            .unwrap();
        let SupernovaState::Supernova { remnant, .. } = entry.state_at(later) else {
            panic!("a supernova after the explosion");
        };
        assert!(remnant.offset().is_none());
        assert!(IaChannel::DoubleDetonation.leaves_a_survivor());
        let o = remnant.survivor_offset().unwrap().value();
        assert!((o - 1_200.0 * 1_000.0 / 299_792.458).abs() < 1e-3, "{o}");
    }

    /// P09.T19.b: every template is positive after the explosion, peaks within 100 days and is
    /// under 10⁻³ of its peak after ten years; failed collapses shine in neutrinos only.
    #[test]
    fn light_curves_peak_early_and_fade() {
        for kind in ALL_KINDS {
            let (mut peak, mut peak_day) = (0.0_f64, 0.0);
            for k in 0..=200_000 {
                // Logarithmic in time from 1 s to 20 years.
                let seconds = math::exp10(8.8 * f64::from(k) / 200_000.0);
                let age = Years::new(seconds / crate::units::consts::SECONDS_PER_JULIAN_YEAR);
                let l = LightCurve::luminosity(kind, age).value();
                assert!(l > 0.0, "{kind:?} at {seconds} s");
                if l > peak {
                    peak = l;
                    peak_day = seconds / SECONDS_PER_DAY;
                }
            }
            let ten = LightCurve::luminosity(kind, Years::new(10.0)).value();
            eprintln!(
                "{kind:?}: peak {:.3e} erg/s at {peak_day:.2} d; at 10 yr {:.2e} of it",
                peak * 1e7,
                ten / peak
            );
            assert!(peak_day <= 100.0, "{kind:?}: {peak_day}");
            assert!(ten < 1e-3 * peak, "{kind:?}");
        }
        let failed = SupernovaKind::CoreCollapse(SupernovaType::None);
        assert_eq!(LightCurve::luminosity(failed, Years::new(0.1)), Watts::ZERO);
        assert!(LightCurve::neutrino_luminosity(failed, Years::ZERO).value() > 1e45);
        assert_eq!(
            LightCurve::neutrino_luminosity(SupernovaKind::TypeIa(IaChannel::Merger), Years::ZERO),
            Watts::ZERO
        );
        assert_eq!(
            LightCurve::luminosity(ALL_KINDS[0], Years::new(-1.0)),
            Watts::ZERO
        );
    }

    /// P09.T19.b: a Type Ia peaks near 10⁴³ erg/s some 14–21 days in, and every template is
    /// continuous: sums of smooth terms, with no join.
    #[test]
    fn light_curves_are_continuous() {
        let ia = SupernovaKind::TypeIa(IaChannel::Merger);
        let (peak, day) = (1..=600)
            .map(|k| {
                let d = f64::from(k) * 0.1;
                (
                    LightCurve::luminosity(ia, Years::new(d / 365.25)).value() * 1e7,
                    d,
                )
            })
            .fold((0.0, 0.0), |a, b| if b.0 > a.0 { b } else { a });
        assert!((0.8e43..=1.6e43).contains(&peak), "{peak:e}");
        assert!((14.0..=21.0).contains(&day), "{day}");
        for kind in ALL_KINDS {
            let at = |seconds: f64| {
                LightCurve::luminosity(
                    kind,
                    Years::new(seconds / crate::units::consts::SECONDS_PER_JULIAN_YEAR),
                )
                .value()
            };
            let mut last = at(1.0);
            for k in 1..=202_000 {
                // Steps of 10⁻⁴ in ln t from 1 s to 19 years: no template moves by 1% in one.
                let seconds = math::exp(f64::from(k) * 1e-4);
                let l = at(seconds);
                assert!(
                    (l / last - 1.0).abs() < 0.01,
                    "{kind:?} at {seconds} s: {last} → {l}"
                );
                last = l;
            }
        }
    }

    /// Pins phase 4's outputs bit for bit: the window table and the caps, shell states, the
    /// fixture's Type Ia rates and ancient shares, a dozen progenitors and their leftovers on
    /// their own tags, and light-curve points.
    #[test]
    fn phase_four_golden() {
        use hyperion_testkit::golden;
        use hyperion_testkit::golden::GoldenWriter;

        use crate::GENERATOR_VERSION;
        use crate::galaxy::POPULATIONS;
        use crate::galaxy::snr::testing::window_table;
        use crate::galaxy::snr::{ShellEnvironment, WindowCaps};
        use crate::id::CatalogueSystemId;

        let mut w = GoldenWriter::new();
        w.header(GENERATOR_VERSION.get());
        for (n, window) in window_table(KelvinPerCm3::new(3_800.0)) {
            let label = format!("window n={:e}", n.value());
            w.f64(&label, window.duration().value());
            for fraction in [0.01, 0.3, 0.99] {
                let age = Years::new(window.duration().value() * fraction);
                let shell = shell_state_at(&window, age).unwrap();
                w.f64(
                    &format!("{label} radius at {fraction}"),
                    shell.radius().value(),
                );
                w.f64(
                    &format!("{label} speed at {fraction}"),
                    shell.shock_speed().value(),
                );
            }
        }
        let galaxy =
            Galaxy::from_params(Seed::new(0x0919_0900), GalaxyParams::milky_way_like()).unwrap();
        let caps = WindowCaps::from_galaxy(&galaxy);
        for environment in ShellEnvironment::ALL {
            w.f64(
                &format!("cap {environment:?}"),
                caps.cap(environment).value(),
            );
        }
        let delays = DelayTimeDistribution::from_galaxy(&galaxy);
        w.f64("type ia rate per year", delays.rate_per_year());
        for p in POPULATIONS {
            w.f64(&format!("ancient share {p:?}"), delays.ancient_share(p));
        }
        let after = UniverseTime::EPOCH
            .checked_sub(Span::from_julian_years(4_263_144).unwrap())
            .unwrap();
        let until = UniverseTime::EPOCH.checked_add(CLOCK_WINDOW_H).unwrap();
        for k in 0..12_u32 {
            let id: SystemId = CatalogueSystemId::new(1, [0, 0, 0], k, 0).unwrap().into();
            let key = ObjectKey::from(id);
            let population = POPULATIONS[usize::try_from(k % 7).unwrap()];
            let mut s = Stream::open(galaxy.seed(), tags::CLASS_TYPE_IA_PROGENITOR, key);
            let p = IaProgenitor::draw(&delays, &mut s, population, after, until).unwrap();
            let mut s = Stream::open(galaxy.seed(), tags::CLASS_TYPE_IA_LEFTOVER, key);
            let leftover = IaLeftover::draw(p.channel(), &mut s);
            w.line(&format!(
                "progenitor {k}: {population:?} {:?} explodes {} delay {} s component {} \
                 lifetimes {} {:?} leftover {leftover:?}",
                p.channel(),
                p.explosion(),
                p.delay().seconds(),
                p.component(),
                p.primary_lifetime().seconds(),
                p.secondary_lifetime().map(Span::seconds),
            ));
            w.f64(&format!("progenitor {k} primary"), p.primary_mass().value());
            w.f64(
                &format!("progenitor {k} secondary"),
                p.secondary_mass().value(),
            );
            if let Some(a) = p.separation_after_common_envelope() {
                w.f64(&format!("progenitor {k} separation"), a.value());
            }
        }
        for kind in ALL_KINDS {
            for days in [0.01, 10.0, 50.0, 150.0, 400.0] {
                let l = LightCurve::luminosity(kind, Years::new(days / 365.25)).value();
                w.f64(&format!("{kind:?} at {days} d"), l);
            }
        }
        golden!("catalogue_classes/supernova", w.as_str());
    }
}
