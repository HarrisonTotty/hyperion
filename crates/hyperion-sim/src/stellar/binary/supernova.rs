//! Deaths and supernovae in a binary (P11.T4.e), after Hurley, Tout and Pols (2002, "BSE")
//! section 2.5 and appendix A1.
//!
//! - **A massive primary** (plan 11, design note 16): a primary of `m_cc(Z) − 1` M☉ or more whose
//!   single-star death is a collapse collapses at plan 06's death age with plan 06's remnant and
//!   kick. The engine takes the age as a fixed boundary. If the binary would end the star sooner
//!   (a stripped helium star that dies first, say), the star is held at its last living state until
//!   then, with the mass it had, and the orbit is left as it was (ruling 129.4a); if the pair has
//!   merged, the product collapses; if nothing living is left, there is no collapse. The collapse
//!   takes the star's last living mass, never its remnant's (P11.T4.k, ruling p11-supernova-pins of
//!   2026-10-06): a primary still on its own track then is the pin's own track at its death, which
//!   explodes from the death's progenitor mass (helium core plus envelope), losing that less plan
//!   06's remnant (BSE appendix A1's ΔM₁), and goes on showing its own remnant, plan 06's. A main
//!   sequence the binary has prolonged past the pin's own ends at that track's end first, so the
//!   star collapses from past its main sequence (P11.T4.n, [`Engine::end_pinned_main_sequence`]).
//! - **Any other death** sheds its mass at the death, through appendix A1 or the white dwarf's
//!   quiet loss, and never before it: the step that lands on it keeps the dying star's living mass
//!   (ruling 129.4a).
//! - **A companion's death** is its own track's, evaluated freely (design note 16): the remnant its
//!   remnant draws decide, and the kick of plan 06's law (P06.T19) with the collapse channel of its
//!   death and the progenitor marked [`Stripping::Companion`] if the binary took its envelope, so
//!   that the law's low mode applies as the brainstorm says (always for electron capture and
//!   accretion-induced collapse; for a stripped star with probability 1 below a 2 M☉ core falling to
//!   0 at 3).
//! - **The orbit after it** (appendix A1): the relative position and velocity at the moment of
//!   explosion, the exploding star's kick added to its velocity and the mass it lost removed, and
//!   the new orbit found from that state. BSE draws the moment's mean anomaly at random, since its
//!   orbit is averaged; here the orbit is on rails (plan 11, design note 6), so the moment is where
//!   the stars are then, from the orbit's elements at the universe time of the explosion. The kick's
//!   direction is plan 06's draw, along the galactic axes, which are the system frame's. An orbit
//!   that comes out unbound leaves two single stars ([`SegmentKind::Disrupted`]). The pair's
//!   barycentre recoils (equation A14), which the record keeps.
//! - **A pair the pre-test passes over** (P11.T4.l, ruling p11-supernova-pins of 2026-10-06,
//!   finding F3) is two single stars on its drawn orbit, and each of its stars' collapses is
//!   applied once to that orbit in closed form ([`Engine::passed_over`]): appendix A1 from the
//!   orbit's rails at the collapse, widened by Jeans's mode for the mass the winds have taken. The
//!   engine takes the pair over from a collapse whose new orbit can interact.
//! - **Accretion-induced collapse** of a white dwarf at the Chandrasekhar mass (section 2.6.5) is
//!   `rlof.rs`'s, through [`Engine::explode`].
//! - **Every phase that advances the engine's age acts on a death or the pin that it reaches**
//!   (P11.T4.k, ruling p11-t4k-faults of 2026-10-06): the detached and transfer steps' stops, a
//!   transfer's last step included (`rlof.rs`), the contact's knots, which stop at the pin
//!   (`common_envelope.rs`), and a backstop in [`Engine::run`] for a pin due at the engine's age
//!   that a phase left behind.
//! - **A bare core with no life left** ([`Remains::Ended`]: a helium giant's core at or above the
//!   Chandrasekhar mass, ruling 129.4c, or a stripped core whose helium star's own clock has
//!   already ended, P11.T4.k) dies at once through [`Engine::die`], on the orbit its stripping
//!   left, or is the pinned primary's hold until its pin (`common_envelope.rs`).
//! - **A giant whose core is a white dwarf's** ([`Engine::dwarf_cores`]: one a carbon–oxygen or
//!   oxygen–neon dwarf fed hydrogen swelled into, `Engine::swell`, or a merger product placed around
//!   one; P11.T4's swollen dwarf and dwarf limit) ends where its own track dies as the dwarf's core
//!   would ([`SwollenDeath`]), since gaining and losing a hydrogen envelope does not change a
//!   core's composition (fix-p11-swollen-dwarf-kind): the swell places the giant on a track by its
//!   core alone, which types its death by its own core at the base of the AGB (HPT section 6).
//!
//! [`Remains::Ended`]: crate::stellar::sse::Remains::Ended

use std::sync::Arc;

use crate::coords::{SystemVector, SystemVelocity};
use crate::orbit::{Eccentricity, KeplerElements, Orbit, elements_from_state};
use crate::stellar::Phase;
use crate::stellar::remnant::collapse::{
    OXYGEN_NEON_CAPTURE_MASS, RemnantDraws, electron_capture_remnant,
};
use crate::stellar::remnant::structure::{CHANDRASEKHAR_MASS, below_the_chandrasekhar_mass};
use crate::stellar::remnant::{
    CollapseChannel, CompactRemnant, DeathKind, KickDraws, KickLaw, NatalKick, ProgenitorAtDeath,
    RemnantKind, RemnantRecipe, StandardKickLaw, Stripping, SupernovaType,
};
use crate::stellar::sse;
use crate::time::{Span, UniverseTime};
use crate::units::consts::{SECONDS_PER_JULIAN_YEAR, SOLAR_RADIUS_M};
use crate::units::{GravitationalParameter, Megayears, Radians, SolarMasses, Years};

use super::co_chandrasekhar::ChandrasekharPath;
use super::detached::last_living;
use super::dwarf_limit::CoreSource;
use super::evolve::{Engine, LiveOrbit, OwnMainSequenceEnd, Pin, PreTest, clock_resolution_years};
use super::star::{Member, Path, cooling_origin, positive};
use super::timeline::{
    BinaryInput, BinaryTimeline, Component, Context, FixedOrbit, MAX_REPORTED_ECCENTRICITY,
    OrbitPath, Segment, SegmentKind, SupernovaRecord,
};

/// How the giant a dwarf swelled into ends where its own track dies: as the dwarf's core would.
///
/// Read by [`Engine::die`] and [`Engine::core_limit_age`]: the swell places the giant on a track
/// by its core alone, and the track types its death by its own core at the base of the AGB (HPT
/// section 6), which need not be the dwarf's kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SwollenDeath {
    /// The track's own death, which the dwarf's kind agrees with, or no dwarf's.
    Own,
    /// The white dwarf of `phase`, the dwarf's kind, where the track leaves the other kind.
    WhiteDwarf(Phase),
    /// An oxygen–neon core that dies with its envelope on where the track's would not collapse by
    /// electron capture: it does, an electron-capture supernova (Nomoto 1984, ApJ 277, 791) with
    /// no Type Ia candidate (`Engine::electron_capture_collapse`; lean, 2026-11-03, where the
    /// ruling has the accretion-induced collapse of a bare dwarf, whose remnant and kick are the
    /// same).
    ElectronCapture,
    /// A carbon–oxygen core that dies with its envelope on where the track's would not ignite
    /// carbon: carbon ignites in the degenerate core at `Mc,SN` and nothing is left (HPT section 6,
    /// [`DeathKind::ThermonuclearDisruption`]), a Type Ia candidate from 30 (P11.T4's
    /// `co_chandrasekhar`).
    Disruption,
}

impl SwollenDeath {
    /// How a giant whose core came from `source` ends where its own track's death is `death`, with
    /// its carbon–oxygen core at `core`, leaving a remnant of `phase`.
    ///
    /// The track's own `Mc,BAGB` decides how a kept core ends, whatever kind it keeps (P11.T4's
    /// kept-core endings): a track that collapses an iron core collapses the kept core too, a
    /// modelling choice after BSE, whose merger product takes the type of the giant it makes (BSE
    /// table 2; research-leans-38). The kept kind sets only what is left where the track leaves a
    /// white dwarf or collapses an oxygen–neon core by electron capture, there a carbon–oxygen core
    /// below the Chandrasekhar mass ([`below_the_chandrasekhar_mass`]) being left as its dwarf and
    /// one at it disrupting, with no Type Ia candidate. Before the fix a kept carbon–oxygen core
    /// disrupted and left nothing where its track would collapse it: 378 such products in 17,814
    /// pairs of the dwarf-limit census's layer D (2026-11-05), several times layer D's Type Ia.
    #[must_use]
    pub(super) fn of(
        source: CoreSource,
        death: DeathKind,
        core: SolarMasses,
        phase: Phase,
    ) -> Self {
        let Some(kind) = source.dwarf() else {
            return Self::Own;
        };
        match death {
            DeathKind::ElectronCapture
                if kind == Phase::CarbonOxygenWhiteDwarf && below_the_chandrasekhar_mass(core) =>
            {
                Self::WhiteDwarf(kind)
            }
            DeathKind::CoreCollapse { .. }
            | DeathKind::DirectCollapse
            | DeathKind::PairInstability => Self::Own,
            DeathKind::EnvelopeLoss if source.dwarf_phase(phase) == phase => Self::Own,
            DeathKind::EnvelopeLoss => Self::WhiteDwarf(kind),
            DeathKind::ElectronCapture if kind == Phase::OxygenNeonWhiteDwarf => Self::Own,
            DeathKind::ThermonuclearDisruption if kind == Phase::CarbonOxygenWhiteDwarf => {
                Self::Own
            }
            DeathKind::ElectronCapture | DeathKind::ThermonuclearDisruption => {
                if kind == Phase::OxygenNeonWhiteDwarf {
                    Self::ElectronCapture
                } else {
                    Self::Disruption
                }
            }
        }
    }
}

impl SwollenDeath {
    /// Counts the death of a giant whose core is a dwarf's on this thread
    /// ([`dwarf_core_deaths`](super::dwarf_limit::dwarf_core_deaths)).
    #[cfg(test)]
    pub(super) fn count(self) {
        use super::dwarf_limit::Ending;

        super::dwarf_limit::count_dwarf_core_death(match self {
            Self::Own => Ending::Own,
            Self::WhiteDwarf(_) => Ending::WhiteDwarf,
            Self::ElectronCapture => Ending::ElectronCapture,
            Self::Disruption => Ending::Disruption,
        });
    }
}

/// The bisections of a core's limit on a phase's span ([`Engine::core_limit_age`]):
/// 64 halve a span of a million years far below a step's length (the engine's clock resolution,
/// `clock_resolution_years`).
const CAPTURE_BISECTIONS: u32 = 64;

/// The first track age, years, in `from_years`–`to_years` at which `core_msun`, a core's mass in
/// M☉ at a track age and non-decreasing there, reaches `limit_msun`, to [`CAPTURE_BISECTIONS`]
/// halvings of the span or until they stop moving (the age returned has reached it): `from_years`
/// if it already has, `None` if it does not by `to_years` or the span is empty.
#[must_use]
pub(super) fn first_age_at(
    core_msun: impl Fn(f64) -> f64,
    from_years: f64,
    to_years: f64,
    limit_msun: f64,
) -> Option<f64> {
    if from_years >= to_years || core_msun(to_years) < limit_msun {
        return None;
    }
    if core_msun(from_years) >= limit_msun {
        return Some(from_years);
    }
    let (mut lo, mut hi) = (from_years, to_years);
    for _ in 0..CAPTURE_BISECTIONS {
        let mid = lo + 0.5 * (hi - lo);
        if mid <= lo || mid >= hi {
            break;
        }
        if core_msun(mid) >= limit_msun {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    Some(hi)
}

/// The mass, M☉, at which a core that is a dwarf's of `source` reaches its end under `recipe`
/// ([`Engine::core_limit_age`]): an oxygen–neon one at the capture mass under
/// [`RemnantRecipe::MandelMuller2020`], at which electron captures on ²⁴Mg and ²⁰Ne collapse it
/// (Miyaji et al. 1980, PASJ 32, 303; Nomoto 1984, ApJ 277, 791; ruling 57), as the recipe caps
/// the oxygen–neon cores of single stars outside the capture window there ([`capture_mass`]), and
/// at the Chandrasekhar mass under [`RemnantRecipe::Hurley2000`], which caps none; `None` for a
/// core that is no dwarf's, and for a carbon–oxygen one, which ends as its track does (P11.T4's
/// kept-core endings, [`SwollenDeath::of`]).
#[must_use]
pub(super) fn core_limit(source: CoreSource, recipe: RemnantRecipe) -> Option<SolarMasses> {
    match (source, recipe) {
        (CoreSource::Other | CoreSource::CarbonOxygenDwarf, _) => None,
        (CoreSource::OxygenNeonDwarf, RemnantRecipe::MandelMuller2020) => Some(capture_mass()),
        (CoreSource::OxygenNeonDwarf, RemnantRecipe::Hurley2000) => Some(CHANDRASEKHAR_MASS),
    }
}

#[cfg(test)]
thread_local! {
    /// The capture mass of this thread's runs, M☉: [`OXYGEN_NEON_CAPTURE_MASS`] unless a test
    /// lowers it ([`with_capture_mass`]); the generator's builds have no such knob.
    static CAPTURE_MASS: core::cell::Cell<f64> =
        const { core::cell::Cell::new(OXYGEN_NEON_CAPTURE_MASS.value()) };
}

/// The capture mass of an oxygen–neon core under the default recipe ([`core_limit`]):
/// [`OXYGEN_NEON_CAPTURE_MASS`], or in a test build the mass a test has set
/// ([`with_capture_mass`]).
#[must_use]
fn capture_mass() -> SolarMasses {
    #[cfg(test)]
    {
        SolarMasses::new(CAPTURE_MASS.with(core::cell::Cell::get))
    }
    #[cfg(not(test))]
    {
        OXYGEN_NEON_CAPTURE_MASS
    }
}

/// `run` with the capture mass at `mass_msun` on this thread, for this call alone, so that a test
/// can reach an oxygen–neon core's limit, which no built track's carbon–oxygen core does: the
/// generator's path is untouched, [`capture_mass`] being [`OXYGEN_NEON_CAPTURE_MASS`] outside test
/// builds, and here outside this call.
#[cfg(test)]
pub(super) fn with_capture_mass<T>(mass_msun: f64, run: impl FnOnce() -> T) -> T {
    /// Puts the thread's capture mass back when the run ends, a panicking one included.
    struct Restore(f64);
    impl Drop for Restore {
        fn drop(&mut self) {
            CAPTURE_MASS.with(|c| c.set(self.0));
        }
    }
    let _restore = Restore(CAPTURE_MASS.with(|c| c.replace(mass_msun)));
    run()
}

impl Engine {
    /// Member `i`'s own track dies now.
    pub(super) fn die(&mut self, i: usize) {
        self.close_segment();
        let Some((track, offset)) = self.members[i]
            .track()
            .map(|(t, o)| (std::sync::Arc::clone(t), o))
        else {
            return;
        };
        let (Some(death), Some(remnant)) = (track.death(), track.remnant()) else {
            return;
        };
        // A pinned primary does not die before plan 06's age, nor at it, where the pin collapses
        // it once (P11.T4.k; `Engine::run`'s backstop): it is held at its last living
        // state, which is the member's own at the mass the binary gives it, and the orbit is left
        // as it is (ruling 129.4a as amended by P11.T4.g). A star on its own track has its
        // track's mass there (the track's mass at the death itself is its remnant's); one the
        // binary carries, the binary's, with its radius at that mass (HPT section 7.1) and its
        // luminosity through the small-envelope perturbation (HPT section 6.3). A star with no envelope left there is a naked helium star or a white dwarf
        // (HPT section 6; BSE `hrdiag`): it is stripped first, and is held only if that
        // leaves it with nothing to lose.
        if self.pin_pending(i) {
            self.hold(&track, offset);
            return;
        }
        let before = match &self.members[i] {
            Member::Shaped { mass, .. } => mass.last(),
            Member::Track { .. }
            | Member::MainSequence { .. }
            | Member::Cooling { .. }
            | Member::Frozen { .. }
            | Member::Remnant { .. }
            | Member::Gone => {
                let p = death.progenitor();
                p.helium_core_mass().value() + p.envelope_mass().value()
            }
        };
        // From now the member is what its death leaves, at the engine's age itself: the offset
        // puts the track's age there at its death or past it, never an ulp short, where the
        // track would still give the living star's mass and the next step would keep the orbit's
        // angular momentum through the mass the death has already taken (ruling 129.4a).
        let offset = if self.age - offset >= death.age().value() {
            offset
        } else {
            super::evolve::offset_for(self.age, death.age().value())
        };
        // A giant whose core is a dwarf's dies as the dwarf's core would (P11.T4's swollen dwarf).
        let phase = track
            .remnant_clock()
            .map_or(Phase::NoRemnant, |(phase, ..)| phase);
        let core = death.progenitor().co_core_mass();
        let source = self.dwarf_cores[i];
        let ending = SwollenDeath::of(source, death.kind(), core, phase);
        #[cfg(test)]
        if source != CoreSource::Other {
            ending.count();
        }
        let member = match ending {
            SwollenDeath::Own => Member::Track {
                track: std::sync::Arc::clone(&track),
                offset,
            },
            SwollenDeath::WhiteDwarf(phase) => {
                // A collapse the kept core does not take leaves the dwarf of its core.
                let mass = if death.kind().is_sudden() {
                    core.value().min(before)
                } else {
                    remnant.mass().value()
                };
                let last = track.state_at(Years::new(last_living(death.age().value())));
                Member::Remnant {
                    phase,
                    birth: self.age,
                    origin: cooling_origin(&self.ctx, phase, mass, Some(last.luminosity())),
                    mass: Path::starting(self.age, mass),
                }
            }
            SwollenDeath::ElectronCapture => {
                self.electron_capture_collapse(i, death.progenitor().co_core_mass(), before);
                return;
            }
            SwollenDeath::Disruption => {
                let masses = self.ignition_masses(i, core, before);
                self.end_at_the_chandrasekhar_mass(
                    i,
                    before,
                    ChandrasekharPath::KeptCoreAtDeath,
                    masses,
                );
                return;
            }
        };
        if source == CoreSource::CarbonOxygenDwarf {
            self.own_carbon_ignition(i, death.kind(), core, before);
        }
        let left = self.members_mass_left_msun(&member, &remnant);
        self.set_member(i, member);
        if !death.kind().is_sudden() || matches!(ending, SwollenDeath::WhiteDwarf(_)) {
            // A white dwarf born as the envelope goes: continuous, and whatever envelope a companion
            // left on the star goes with it, without a kick.
            let lost = before - left;
            let bound = self.orbit.is_some();
            if lost > 1e-9 && bound {
                self.lose_mass(i, before, left, None);
            }
            // An orbit the loss unbinds leaves both stars: the pair is disrupted, not merged
            // (finding F1 of ruling p11-channels, 2026-10-06, the label only).
            self.kind =
                if bound && self.orbit.is_none() && !self.members.iter().any(Member::is_gone) {
                    SegmentKind::Disrupted {
                        by: Component::of_index(i),
                    }
                } else {
                    self.quiet_kind()
                };
            return;
        }
        let kick = self.companion_kick(i, &death, &remnant);
        self.explode(i, before, remnant, kick);
    }

    /// Whether member `i` is the primary whose pin is still to come, or due now (P11.T4.k): its
    /// collapse is plan 06's, so the binary ends it no sooner (design note 16).
    #[must_use]
    pub(super) fn pin_pending(&self, i: usize) -> bool {
        i == 0
            && self.pin.is_some_and(|p| {
                p.death.age().value() >= self.age - clock_resolution_years(self.age)
            })
    }

    /// The mass, M☉, a death leaves in `member`, its own track's `remnant` or a white dwarf the
    /// engine makes of it ([`SwollenDeath::WhiteDwarf`]).
    #[must_use]
    fn members_mass_left_msun(&self, member: &Member, remnant: &CompactRemnant) -> f64 {
        match member {
            Member::Remnant { mass, .. } => mass.at(self.age),
            Member::Track { .. }
            | Member::Shaped { .. }
            | Member::MainSequence { .. }
            | Member::Cooling { .. }
            | Member::Frozen { .. }
            | Member::Gone => remnant.mass().value(),
        }
    }

    /// Member `i`, a giant of `before` M☉ whose core of `core` is an oxygen–neon dwarf's
    /// ([`Engine::dwarf_cores`]), collapses now by electron capture where its own track would not.
    ///
    /// The core collapses inside its hydrogen envelope, an electron-capture supernova (Nomoto
    /// 1984, ApJ 277, 791; HPT section 6, the engine's [`DeathKind::ElectronCapture`]), into Mandel
    /// and Müller's 1.26 M☉ neutron star ([`electron_capture_remnant`]) with the low-mode kick of
    /// electron capture and no Type Ia candidate (P11.T4's swollen dwarf and dwarf limit): at its
    /// track's death ([`SwollenDeath::ElectronCapture`]), or where its core
    /// reaches its limit first ([`Engine::core_limit_age`]). Its envelope is the rest of
    /// `before`.
    fn electron_capture_collapse(&mut self, i: usize, core: SolarMasses, before: f64) {
        let core = SolarMasses::new(core.value().min(before));
        let envelope = SolarMasses::new((before - core.value()).max(0.0));
        let progenitor = ProgenitorAtDeath::new(core, core, envelope, Stripping::None);
        let death = crate::stellar::remnant::Death::new(
            Years::new(self.age),
            DeathKind::ElectronCapture,
            progenitor,
        );
        let remnant = electron_capture_remnant();
        let kick = self.companion_kick(i, &death, &remnant);
        self.set_member(
            i,
            Member::Remnant {
                phase: Phase::NeutronStar,
                birth: self.age,
                origin: Megayears::ZERO,
                mass: Path::starting(self.age, remnant.mass().value()),
            },
        );
        self.explode(i, before, remnant, kick);
    }

    /// The engine's age, years, at which member `i`'s core, a white dwarf's whose kind its track
    /// would not give it ([`Engine::dwarf_cores`]), reaches its limit ([`core_limit`]) inside the
    /// member's current phase, where it ends as that dwarf's core does
    /// ([`Engine::reach_core_limit`]; P11.T4's swollen dwarf and dwarf limit, research-leans-35
    /// §7–§8); `None` for any other member, for a pinned primary whose pin is
    /// pending, or where the phase ends first, whose boundary the next step's limit starts from.
    ///
    /// Only an oxygen–neon core has a limit ([`core_limit`]): a carbon–oxygen one ends as its track
    /// does (P11.T4's kept-core endings). An oxygen–neon core on a carbon–oxygen track would grow to that track's `Mc,SN`,
    /// 1.44 M☉, or leave a dwarf above the capture mass. The core is the track's own carbon–oxygen
    /// core ([`Track::carbon_oxygen_core_at`]), which the binary's mass does not change, found on
    /// the phase's span by bisection ([`first_age_at`]). No built track's carbon–oxygen core passes
    /// about 1.20 M☉, so an oxygen–neon core's limit is reached only where its track would
    /// collapse.
    ///
    /// [`Track::carbon_oxygen_core_at`]: crate::stellar::sse::Track::carbon_oxygen_core_at
    #[must_use]
    pub(super) fn core_limit_age(&self, i: usize) -> Option<f64> {
        let source = self.dwarf_cores[i];
        if source == CoreSource::Other || self.pin_pending(i) {
            return None;
        }
        let (track, offset) = match &self.members[i] {
            Member::Track { track, offset } | Member::Shaped { track, offset, .. } => {
                (track, *offset)
            }
            Member::MainSequence { .. }
            | Member::Cooling { .. }
            | Member::Frozen { .. }
            | Member::Remnant { .. }
            | Member::Gone => return None,
        };
        let death = track.death()?;
        let phase = track
            .remnant_clock()
            .map_or(Phase::NoRemnant, |(phase, ..)| phase);
        let core = death.progenitor().co_core_mass();
        if SwollenDeath::of(source, death.kind(), core, phase) == SwollenDeath::Own {
            return None;
        }
        // The phase the engine's age is in or about to enter, as the step's own limits read it.
        let (_, end, now) = super::evolve::phase_ahead(track, offset, self.age);
        let to = last_living(end.min(death.age().value()));
        let core = |age: f64| {
            track
                .carbon_oxygen_core_at(age)
                .map_or(0.0, SolarMasses::value)
        };
        let limit = core_limit(source, self.ctx.track_options().remnant())?;
        let at = first_age_at(core, now, to, limit.value())?;
        Some(self.age.max(offset + at))
    }

    /// Member `i`'s core, a white dwarf's, reaches its limit now ([`Engine::core_limit_age`]): an
    /// oxygen–neon one collapses by electron capture.
    pub(super) fn reach_core_limit(&mut self, i: usize) {
        self.close_segment();
        let (m, _) = self.current(i);
        let source = self.dwarf_cores[i];
        match source {
            CoreSource::OxygenNeonDwarf => {
                #[cfg(test)]
                super::dwarf_limit::count_dwarf_core_death(
                    super::dwarf_limit::Ending::CaptureAtLimit,
                );
                let limit = core_limit(source, self.ctx.track_options().remnant());
                self.electron_capture_collapse(i, limit.unwrap_or(CHANDRASEKHAR_MASS), m);
            }
            // A core that is no dwarf's, or a carbon–oxygen one, has no limit (`core_limit_age`).
            CoreSource::Other | CoreSource::CarbonOxygenDwarf => {}
        }
    }

    /// Holds the pinned primary, whose own track `track` (at `offset`) dies now, at its last
    /// living state until plan 06's death age (ruling 129.4a; P11.T4.g). The state is the
    /// member's structure at the last living age with the mass the binary gives it
    /// ([`Member::evaluate`]'s, read at the track's age itself), and its core radius is that
    /// structure's. A structure with no envelope (M ≤ Mc) is stripped first through
    /// [`Engine::stripped_member_at`] at the same age: a helium star lives on with the pin still
    /// waiting, and a bare core with no life left (at or above the Chandrasekhar mass, ruling
    /// 129.4c, or a helium star past its own end, P11.T4.k) is held at its last living state, as
    /// is the core of a white dwarf (P11.T4's fix, `pinned_white_dwarfs`).
    fn hold(&mut self, track: &std::sync::Arc<sse::Track>, offset: f64) {
        let living = last_living(self.age - offset);
        let carried = match &self.members[0] {
            Member::Shaped { mass, .. } => Some(mass.last()),
            Member::Track { .. }
            | Member::MainSequence { .. }
            | Member::Cooling { .. }
            | Member::Frozen { .. }
            | Member::Remnant { .. }
            | Member::Gone => None,
        };
        let structure = match carried {
            Some(mass) => Some(track.structure_at(living, mass.max(1e-6))),
            None => track.own_structure_at(living),
        };
        if let Some(structure) = &structure
            && structure.state.envelope_mass().value() <= 0.0
            && let Some((member, collapses)) = self.stripped_member_at(0, living)
        {
            debug_assert!(
                !collapses,
                "a pinned primary's bare core is held, not exploded"
            );
            self.stripped[0] = true;
            self.set_member(0, member);
            self.kind = self.quiet_kind();
            return;
        }
        // A star on its own track keeps its track's state bit for bit; one the binary carries
        // takes its structure's at its own mass.
        let state = match (carried, &structure) {
            (Some(_), Some(structure)) => structure.state,
            _ => track.state_at(Years::new(living)),
        };
        let mass = carried.unwrap_or_else(|| state.mass().value());
        let held = crate::stellar::StarState::new(crate::stellar::StarStateParts {
            phase: state.phase(),
            age: state.age(),
            mass: SolarMasses::new(mass),
            core_mass: SolarMasses::new(state.core_mass().value().min(mass)),
            luminosity: state.luminosity(),
            radius: state.radius(),
            mass_loss_rate: crate::units::SolarMassesPerYear::ZERO,
            phase_fraction: state.phase_fraction(),
        });
        let core_radius = structure.map_or(crate::units::SolarRadii::ZERO, |s| s.core_radius);
        self.set_member(
            0,
            Member::Frozen {
                state: held,
                core_radius,
            },
        );
    }

    /// The kick of member `i`'s `remnant` after its own `death` (plan 06's law under the pair's
    /// stellar rules, P06.T19), its progenitor marked companion-stripped if the binary took its
    /// envelope.
    #[must_use]
    fn companion_kick(
        &self,
        i: usize,
        death: &crate::stellar::remnant::Death,
        remnant: &CompactRemnant,
    ) -> Option<NatalKick> {
        let stripping = if self.stripped[i] {
            Stripping::Companion
        } else {
            death.progenitor().stripping()
        };
        own_kick(&self.ctx, i, death, remnant, stripping)
    }

    /// Member `i`, a merger's product of `mass` M☉ with an iron core, collapses now.
    ///
    /// The product is one that no star can hold, its envelope on, whose carbon–oxygen core of
    /// `co_core`, inside a helium core of `helium_core`, is an iron core (P11.T4's dwarf limit,
    /// [`Unplaced::IronCore`]; HPT section 6, BSE section 2.7.4). It takes plan
    /// 06's iron-core collapse ([`sse::iron_core_fate`]) under the pair's remnant recipe and the
    /// member's remnant draws, the supernova of its hydrogen envelope and the helium outside its
    /// carbon–oxygen core ([`SupernovaType::of_envelopes`]), and the kick of plan 06's law for an
    /// iron core, the progenitor unstripped. A thermally pulsing star's helium core is its
    /// carbon–oxygen core, as plan 06's own death at `Mc,SN` takes it (`pulsing_agb_end`), so its
    /// supernova is hydrogen-rich; a helium giant's is the whole star, with no hydrogen.
    ///
    /// [`Unplaced::IronCore`]: super::dwarf_limit::Unplaced::IronCore
    pub(super) fn iron_core_collapse(
        &mut self,
        i: usize,
        co_core: SolarMasses,
        helium_core: SolarMasses,
        mass: SolarMasses,
    ) {
        let envelope = SolarMasses::new((mass.value() - helium_core.value()).max(0.0));
        let helium = SolarMasses::new((helium_core.value() - co_core.value()).max(0.0));
        let fate = sse::iron_core_fate(
            self.ctx.track_options().remnant(),
            Years::new(self.age),
            SupernovaType::of_envelopes(envelope, helium),
            co_core,
            ProgenitorAtDeath::new(co_core, helium_core, envelope, Stripping::None),
            RemnantDraws::of(self.ctx.draws(i)),
        );
        self.unplaced_collapse(i, &fate, mass);
    }

    /// Member `i`, a merger's helium-giant product of `mass` M☉ that no star can hold, whose
    /// carbon–oxygen core of `co_core` is an oxygen–neon one, collapses now by electron capture.
    ///
    /// The product's mass (P11.T4's kept-core endings, [`Unplaced::ElectronCapture`];
    /// research-leans-40) stands for its helium star's, and so for `Mc,BAGB` (HPT section 6.1),
    /// and one of 1.6–2.25 M☉ has an oxygen–neon core (HPT section 6), as plan 06's helium star of
    /// that mass collapses (`helium_star_end`). It takes plan 06's electron-capture supernova
    /// under the pair's remnant recipe ([`sse::oxygen_neon_capture_fate`]: Mandel and Müller's
    /// 1.26 M☉ neutron star, MM20 section 3, or HPT's equation 92 of the core at `Mc,SN`, which
    /// their equation 75 puts at the Chandrasekhar mass at least, and so at it for such a star),
    /// with no hydrogen, the low-mode kick of electron capture, and no Type Ia candidate. The
    /// progenitor is taken unstripped, as for an iron core ([`Engine::iron_core_collapse`]); an
    /// electron capture's kick does not read the stripping.
    ///
    /// [`Unplaced::ElectronCapture`]: super::dwarf_limit::Unplaced::ElectronCapture
    pub(super) fn capture_collapse(&mut self, i: usize, co_core: SolarMasses, mass: SolarMasses) {
        let progenitor = ProgenitorAtDeath::new(co_core, mass, SolarMasses::ZERO, Stripping::None);
        let mc_sn = SolarMasses::new(co_core.value().max(CHANDRASEKHAR_MASS.value()));
        let fate = sse::oxygen_neon_capture_fate(
            self.ctx.track_options().remnant(),
            Years::new(self.age),
            mc_sn,
            progenitor,
        );
        self.unplaced_collapse(i, &fate, mass);
    }

    /// Member `i`, a merger's product of `mass` M☉ that no star can hold, ends now in `fate`: its
    /// remnant, with plan 06's kick for its death, the progenitor unstripped
    /// ([`Engine::iron_core_collapse`], [`Engine::capture_collapse`]).
    fn unplaced_collapse(&mut self, i: usize, fate: &sse::Fate, mass: SolarMasses) {
        let kick = own_kick(&self.ctx, i, &fate.death, &fate.remnant, Stripping::None);
        let member = if fate.remnant.kind() == RemnantKind::None {
            Member::Gone
        } else {
            Member::Remnant {
                phase: fate.phase,
                birth: self.age,
                origin: Megayears::ZERO,
                mass: Path::starting(self.age, fate.remnant.mass().value()),
            }
        };
        self.set_member(i, member);
        self.explode(i, mass.value(), fate.remnant, kick);
    }

    /// The primary's pinned death now (design note 16): plan 06's remnant and kick, whatever the
    /// pair has made of the star, from the star's last living mass, never its remnant's (ruling
    /// 129.4a; P11.T4.k, ruling p11-supernova-pins of 2026-10-06).
    ///
    /// - **A primary still on its own track** is the pin's own track at its death: the step lands
    ///   on the death, where the pin wins the tie with the track's own [`Stop::Death`], and the
    ///   track there already shows its remnant. It is living by construction. Its mass before is
    ///   the death's progenitor mass (helium core plus envelope), as [`Engine::die`] takes it for a
    ///   star on its own track, and it stays its own track, which shows plan 06's remnant bit for
    ///   bit (finding F7: read at the pin, the remnant was taken for nothing living, and the
    ///   collapse left no record, no mass loss and no kick).
    /// - **A star the binary carries on the pin's own track** (a donor in stable transfer, say)
    ///   meets the same tie when its track dies at the pin: it is read at its track's last living
    ///   instant, on its closed forms, and its mass before is the mass the binary gives it there
    ///   (the carried tie, ruling p11-t4k-faults of 2026-10-06: the pins ruling's "the carried or
    ///   held mass otherwise").
    /// - **Any other form** (a star the binary carries on another track, a held one, a merger's
    ///   product) is read as it is now; if nothing living is left there is no collapse.
    ///
    /// [`Stop::Death`]: super::detached::Stop::Death
    pub(super) fn pinned_collapse(&mut self) {
        self.close_segment();
        let Some(pin) = self.pin.take() else {
            return;
        };
        let own_death = self.own_death_now(pin.death.age().value());
        if let (
            Member::Track { .. },
            Some(OwnDeath {
                track,
                offset_years,
                death,
            }),
        ) = (&self.members[0], &own_death)
        {
            let p = death.progenitor();
            let before = p.helium_core_mass().value() + p.envelope_mass().value();
            let remnant = held_to(pin.remnant, before);
            debug_assert!(
                track.remnant() == Some(remnant),
                "an untouched primary's own track shows plan 06's remnant: {:?} against {remnant:?}",
                track.remnant()
            );
            self.set_member(
                0,
                Member::Track {
                    track: std::sync::Arc::clone(track),
                    offset: *offset_years,
                },
            );
            self.explode(0, before, remnant, pin.kick);
            return;
        }
        let (mass, living) =
            if let (Member::Shaped { mass, .. }, Some(OwnDeath { track, death, .. })) =
                (&self.members[0], own_death)
            {
                // A star the binary carries whose own track dies now: read on its closed forms at its
                // last living instant, at the mass the binary gives it. That mass is the landing
                // step's, as `die` reads it: the carried mass does not fall at the death, and differs
                // from its path's at the last living instant only by the wind of 10⁻¹² of the age.
                let carried = mass.last();
                let last = last_living(death.age().value());
                let living = track
                    .structure_at(last, carried.max(1e-6))
                    .state
                    .phase()
                    .is_living();
                (carried, living)
            } else {
                let (mass, tau) = self.current(0);
                let living = self
                    .structure(0, self.age, mass, tau)
                    .is_some_and(|s| s.state.phase().is_living());
                (mass, living)
            };
        if !living {
            return;
        }
        let remnant = held_to(pin.remnant, mass);
        let phase = match remnant.kind() {
            RemnantKind::BlackHole => Phase::BlackHole,
            RemnantKind::NeutronStar => Phase::NeutronStar,
            RemnantKind::WhiteDwarf => Phase::CarbonOxygenWhiteDwarf,
            RemnantKind::None => Phase::NoRemnant,
        };
        let member = if remnant.kind() == RemnantKind::None {
            Member::Gone
        } else {
            Member::Remnant {
                phase,
                birth: self.age,
                origin: Megayears::ZERO,
                mass: Path::starting(self.age, remnant.mass().value()),
            }
        };
        self.set_member(0, member);
        self.explode(0, mass, remnant, pin.kick);
    }

    /// The primary's own track at its death now, if member 0 is on the pin's own track (a star on
    /// it, or one the binary carries on its closed forms) and that track dies now: its death's age
    /// on the track is the pin's, `pin_age_years`, and it falls at the engine's age, both to the
    /// resolution of the engine's clock ([`clock_resolution_years`]). For
    /// [`Engine::pinned_collapse`].
    #[must_use]
    fn own_death_now(&self, pin_age_years: f64) -> Option<OwnDeath> {
        let (track, offset) = match &self.members[0] {
            Member::Track { track, offset } | Member::Shaped { track, offset, .. } => {
                (track, *offset)
            }
            Member::MainSequence { .. }
            | Member::Cooling { .. }
            | Member::Frozen { .. }
            | Member::Remnant { .. }
            | Member::Gone => return None,
        };
        let death = track.death()?;
        let at = death.age().value();
        let the_pins = (at - pin_age_years).abs() <= clock_resolution_years(pin_age_years);
        if !the_pins || (at + offset - self.age).abs() > clock_resolution_years(self.age) {
            return None;
        }
        // Placed as `die` places a death: at it or past it, never an ulp short.
        let offset_years = if self.age - offset >= at {
            offset
        } else {
            super::evolve::offset_for(self.age, at)
        };
        Some(OwnDeath {
            track: std::sync::Arc::clone(track),
            offset_years,
            death,
        })
    }

    /// Member `i`, of mass `before`, explodes now into `remnant` with `kick` (BSE appendix A1), the
    /// member already replaced by what it leaves: the orbit after it, or the pair unbound, and the
    /// record.
    ///
    /// A pair this death unbinds is [`SegmentKind::Disrupted`] by `i`, even if `i` leaves nothing (a
    /// pinned primary's pair instability, [`Member::Gone`] from [`Engine::pinned_collapse`]): the
    /// pair is a merger only if the other member is gone, as the closed form labels the same death.
    /// A pair already unbound keeps its label, that of the death that unbound it
    /// ([`Engine::quiet_kind`]), as the closed form ([`Engine::passed_over`]) and the white dwarf's
    /// birth in [`Engine::die`] do.
    pub(super) fn explode(
        &mut self,
        i: usize,
        before: f64,
        remnant: CompactRemnant,
        kick: Option<NatalKick>,
    ) {
        let after = remnant.mass().value();
        let kick = kick.filter(|_| self.ctx.params().natal_kicks);
        let was_bound = self.orbit.is_some();
        let (velocities, bound) = self.lose_mass(i, before, after, kick);
        self.supernovae.push(SupernovaRecord::new(
            Years::new(self.age),
            Component::of_index(i),
            remnant,
            kick,
            bound,
            velocities,
        ));
        self.kind = if bound || self.orbit.is_some() || !was_bound {
            self.quiet_kind()
        } else if self.members[1 - i].is_gone() {
            SegmentKind::Merged
        } else {
            SegmentKind::Disrupted {
                by: Component::of_index(i),
            }
        };
    }

    /// Member `i` loses `before − after` M☉ at once, with `kick`: the new orbit, or none if it is
    /// unbound, and each member's velocity after in the frame of the barycentre before (the pair's
    /// recoil twice for a bound pair, their equation A14), with whether the pair stays bound.
    fn lose_mass(
        &mut self,
        i: usize,
        before: f64,
        after: f64,
        kick: Option<NatalKick>,
    ) -> ([SystemVelocity; 2], bool) {
        let kick_velocity = kick.map_or([0.0; 3], |k| k.velocity());
        let o = 1 - i;
        let (m_other, _) = self.current(o);
        if self.orbit.is_none() || self.members[o].is_gone() || !positive(m_other) {
            self.orbit = None;
            return (alone(i, kick_velocity), false);
        }
        let mut masses = [0.0; 2];
        masses[i] = before;
        masses[o] = m_other;
        let Some(elements) = self.elements_now(masses) else {
            self.orbit = None;
            return (alone(i, kick_velocity), false);
        };
        let t = self.universe_time();
        let state = elements.relative_state_at(t);
        let (orbit, velocities) = appendix_a1(i, masses, after, kick_velocity, state, t, self.age);
        let bound = orbit.is_some();
        self.orbit = orbit;
        (velocities, bound)
    }

    /// The universe time of the engine's age now, from the system's age at the epoch.
    #[must_use]
    fn universe_time(&self) -> UniverseTime {
        universe_time_at(&self.ctx, self.age)
    }

    /// The timeline of a pair the pre-test passes over (plan 11, design note 7; P11.T4.l, ruling
    /// p11-supernova-pins of 2026-10-06, finding F3): two single stars on the drawn orbit of
    /// `input`, `members` their own forms from zero age, run to `until_years`, whose stars' core
    /// collapses before then change that orbit.
    ///
    /// - **Each collapse** ([`collapses_before`]) is applied once, by BSE appendix A1, from the
    ///   orbit on its rails at the collapse's universe time ([`jeans_widened_state`]): the pinned
    ///   primary's at plan 06's age with its remnant and kick (`pin`, design note 16) from its last
    ///   living mass (P11.T4.k), any other star's at its own track's death with its own remnant
    ///   and its kick by plan 06's law.
    /// - **Bound:** the pair goes on, on the new orbit's fixed elements; a later collapse is
    ///   applied the same way on that orbit. **Unbound:** [`SegmentKind::Disrupted`] by the star
    ///   that died, both stars their own models; a later collapse is recorded, alone, and the label
    ///   stays the first's, as the engine's [`Engine::explode`] keeps it.
    /// - Every collapse writes its [`SupernovaRecord`], and the stars stay their own tracks bit for
    ///   bit: only the orbit and the records change. A white dwarf's birth keeps the orbit (the
    ///   physics of finding F1, under its own ruling).
    /// - **After a bound collapse** the pre-test is asked again of the new orbit, from the
    ///   collapse to `until_years` ([`interacts_after_collapse`]), unless `second` says to run
    ///   every such pair through the engine (the tests' check of that pre-test). If it passes, the
    ///   engine runs the pair from the collapse on that orbit, each living star carried with its
    ///   zero-age spin's angular momentum ([`Engine::zero_age_spin`]), and with the pinned
    ///   primary's own main-sequence end still ahead as its stop (`own_end`, P11.T4.n); otherwise
    ///   the orbit stays fixed.
    ///
    /// The build-age contract: up to its first collapse the pair shows the drawn orbit whatever age
    /// it is run to, and from a collapse on the closed form and any engine run start at the
    /// collapse. So a pair the pre-test, and the pre-test asked again after each bound collapse,
    /// pass over at both ages is the same, to the earlier age, as one run past it, bit for bit.
    /// Where the pre-test asked again passes over a pair at an age but not at a later one, the
    /// later run's engine evolves the orbit from the collapse where the earlier keeps it fixed, and
    /// the later run shows no interaction before the earlier age (P11.T4.j's wording). A collapse
    /// at `until_years` itself is not reached, as the engine's own deaths at its age are not.
    #[must_use]
    pub(super) fn passed_over(
        input: &BinaryInput,
        ctx: Arc<Context>,
        until_years: f64,
        members: [Member; 2],
        pin: Option<Pin>,
        own_end: Option<OwnMainSequenceEnd>,
        second: PreTest,
    ) -> BinaryTimeline {
        let until = until_years;
        let drawn = *input.orbit();
        let first = OrbitPath {
            fixed: Some(FixedOrbit::always(drawn)),
            ..LiveOrbit::of(input, Years::ZERO).path()
        };
        let collapses = collapses_before(&ctx, &members, pin, until);
        let mut segments = Vec::with_capacity(collapses.len() + 1);
        let mut records = Vec::with_capacity(collapses.len());
        let (mut kind, mut path, mut start) = (SegmentKind::Detached, Some(first), 0.0);
        // The orbit on its rails, and the pair's mass it was set with, M☉ (`jeans_widened_state`).
        let [m1, m2] = input.masses().map(SolarMasses::value);
        let mut rails = Some((drawn, m1 + m2));
        for collapse in collapses {
            let Collapse {
                age_years: age,
                member: i,
                before_msun: before,
                remnant,
                kick,
            } = collapse;
            segments.push(Segment::new(
                kind,
                start,
                age,
                members.clone(),
                path.take(),
                None,
            ));
            start = age;
            let o = 1 - i;
            let other = members[o].mass_at(age);
            let kick = kick.filter(|_| ctx.params().natal_kicks);
            let kick_velocity = kick.map_or([0.0; 3], |k| k.velocity());
            let t = universe_time_at(&ctx, age);
            let after = remnant.mass().value();
            let (orbit, velocities) = match rails {
                Some((elements, mass_on_rails)) if positive(other) => {
                    let mut masses = [0.0; 2];
                    masses[i] = before;
                    masses[o] = other;
                    let state =
                        jeans_widened_state(&elements, mass_on_rails, masses[0] + masses[1], t);
                    appendix_a1(i, masses, after, kick_velocity, state, t, age)
                }
                Some(_) | None => (None, alone(i, kick_velocity)),
            };
            let record = SupernovaRecord::new(
                Years::new(age),
                Component::of_index(i),
                remnant,
                kick,
                orbit.is_some(),
                velocities,
            );
            records.push(record);
            let Some(live) = orbit else {
                if rails.is_some() {
                    kind = SegmentKind::Disrupted {
                        by: Component::of_index(i),
                    };
                }
                rails = None;
                continue;
            };
            let mut after_masses = [0.0; 2];
            after_masses[i] = after;
            after_masses[o] = other;
            let total_after = after_masses[0] + after_masses[1];
            // The pre-test reads A1's own eccentricity; the orbit is shown at most at the largest
            // a bound pair is reported with.
            let tested = rails_elements(&live, total_after, live.e);
            let ask = match second {
                PreTest::Gate => true,
                #[cfg(test)]
                PreTest::Past | PreTest::PastAfterCollapse => false,
            };
            if !ask || interacts_after_collapse(input, &members, &tested, after_masses, age, until)
            {
                let pending = pin.filter(|p| p.death.age().value() > age);
                let members = members.map(|m| m.restarted(age));
                let mut engine = Self::new(Arc::clone(&ctx), age, until, members, live, pending)
                    .with_own_main_sequence_end(own_end);
                engine.take_over(segments, records);
                engine.run();
                return engine.finish();
            }
            let elements =
                rails_elements(&live, total_after, live.e.min(MAX_REPORTED_ECCENTRICITY));
            kind = SegmentKind::Detached;
            path = Some(OrbitPath {
                fixed: Some(FixedOrbit::always(elements)),
                ..live.path()
            });
            rails = Some((elements, total_after));
        }
        segments.push(Segment::new(kind, start, until, members, path, None));
        BinaryTimeline::new(segments, None, records, until, ctx, false)
    }

    /// Takes over a passed-over pair at the engine's start, a collapse whose new orbit can interact
    /// ([`Engine::passed_over`]): its `segments` and supernova `records` so far, the next segment
    /// starting there, and each living star's spin its zero-age one's ([`Engine::zero_age_spin`]).
    pub(super) fn take_over(&mut self, segments: Vec<Segment>, records: Vec<SupernovaRecord>) {
        for k in 0..2 {
            if let Some(spin) = self.zero_age_spin(k) {
                self.spins[k] = spin;
            }
        }
        self.segment_start = self.age;
        self.segments = segments;
        self.supernovae = records;
    }

    /// The spin angular momentum, M☉ R☉² yr⁻¹, that the engine carries member `k` with when it
    /// takes over a passed-over pair after a collapse ([`Engine::passed_over`]): for a living star
    /// on its own track, the angular momentum it arrived on its main sequence with, the engine's
    /// own start there ([`Engine::new`]: HPT equations 107 and 108, BSE equation 35), times the
    /// wind's factor (M ÷ M₀)^{2/(3k′₃)} for its mass M now and M₀ then, but no more than its
    /// moment of inertia now at its break-up rate √(G M ÷ R³). `None` for any other member, which
    /// keeps [`Engine::new`]'s.
    ///
    /// An upper bound on the spin of the single star it has been until then, in HPT's model
    /// (k′₂ = 0.1), held at break-up as BSE holds a star's. A single star only loses spin after its
    /// zero age: to its wind, which takes (2/3) ΔM R² Ω (HPT equation 110), and to magnetic braking
    /// (BSE equation 50). With a moment of inertia of at most k′₃ M R² (BSE equation 35, k′₂ < k′₃
    /// and Rc ≤ R), the wind alone leaves at most J₀ (M ÷ M₀)^{2/(3k′₃)}, in discrete steps too
    /// (Bernoulli's inequality), and braking only lowers it. Hurley's published `evolv2.f` caps each
    /// star's spin at break-up on every step of nonzero length (SSE's `evolv1.f` does not).
    /// [`Engine::new`]'s own start, the zero-age velocity at the star's radius then, would spin a red
    /// supergiant past break-up (a 10 M☉ star at 500 R☉: 232 km/s against 62 km/s). The lane's
    /// choice, plan 11's Risks, "P11.T4.l as built".
    #[must_use]
    pub(super) fn zero_age_spin(&self, k: usize) -> Option<f64> {
        let Member::Track { track, offset } = &self.members[k] else {
            return None;
        };
        let (mass, tau) = self.current(k);
        let now = self
            .structure(k, self.age, mass, tau)
            .filter(|s| s.state.phase().is_living())?;
        let arrival = offset + track.main_sequence_arrival()?.value();
        let zero_age = self.structure(k, arrival, mass, tau)?;
        let r0 = zero_age.state.radius().value();
        let zero_age_momentum = super::star::moment_of_inertia(&zero_age)
            * super::star::zams_spin(zero_age.state.mass().value(), r0);
        let (m, r) = (now.state.mass().value(), now.state.radius().value());
        let m0 = zero_age.state.mass().value();
        if !(positive(m) && positive(r) && positive(m0)) {
            return Some(zero_age_momentum);
        }
        let wind = crate::math::powf_positive(m / m0, 2.0 / (3.0 * sse::CORE_GYRATION));
        let break_up = (super::star::G * m / (r * r * r)).sqrt();
        Some((zero_age_momentum * wind).min(super::star::moment_of_inertia(&now) * break_up))
    }

    /// After a member's change of phase: a carried main sequence that has reached its end goes
    /// on along a track of its mass from there (HPT section 7.1), whose mass the binary carries.
    ///
    /// A track with no Hertzsprung gap dies at its main sequence's end: plan 06's massive helium
    /// stars collapse from their helium main sequence (one of 19.5 M☉ at Z = 2.1 × 10⁻⁴ in the
    /// fit's sample 49 of E's cell 3579, `pair_light::FIT_SEED`). The star is placed at that death
    /// and dies there ([`Engine::die`], BSE section 2.5's collapse with its mass loss and kick;
    /// P11.T4.o). Before T4.o it was left at its remnant with nothing acted on, a remnant of the
    /// carried mass with no supernova, and a transfer from it went on from a dead donor (the
    /// engine finding of "P11.T17.b as built").
    pub(super) fn after_boundary(&mut self) {
        for i in 0..2 {
            let Member::MainSequence { helium, mass, tau } = &self.members[i] else {
                continue;
            };
            if tau.last() < 1.0 {
                continue;
            }
            let (helium, m) = (*helium, mass.last());
            // A hydrogen star's track starts its main sequence at its arrival (P06.T15.b).
            let start = if helium {
                0.0
            } else {
                sse::main_sequence_start(
                    super::evolve::track_mass(SolarMasses::new(m)),
                    self.ctx.composition(),
                )
            };
            let guess = start + sse::main_sequence_lifetime(self.ctx.coeffs(), helium, m);
            let (composition, draws) = (self.ctx.composition(), self.ctx.draws(i));
            let options = self.ctx.track_options();
            let build = |reach: f64| {
                if helium {
                    sse::Track::helium_star(
                        SolarMasses::new(m.min(sse::MAX_INITIAL_MASS.value())),
                        composition,
                        draws,
                        Years::new(reach),
                    )
                } else {
                    sse::Track::to_age_with(
                        super::evolve::track_mass(SolarMasses::new(m)),
                        composition,
                        draws,
                        options,
                        Years::new(reach),
                    )
                }
            };
            let phase = if helium {
                Phase::HeliumHertzsprungGap
            } else {
                Phase::HertzsprungGap
            };
            let (track, placed) = super::evolve::track_reaching(
                guess,
                self.reach_span_years(self.age),
                build,
                |track| track.age_in_phase(phase, 0.0),
            );
            let dies = placed.is_none() && track.lifetime().is_some();
            let start = placed
                .or_else(|| track.lifetime().map(Years::value))
                .unwrap_or(0.0);
            self.members[i] = Member::Shaped {
                track: std::sync::Arc::new(track),
                offset: super::evolve::offset_for(self.age, start),
                mass: Path::starting(self.age, m),
            };
            if dies {
                self.die(i);
            }
        }
    }
}

/// The elements of `live`, the orbit BSE appendix A1 leaves a passed-over pair of `total_msun` M☉
/// (P11.T4.l), at `eccentricity`, as the engine reads an orbit it carries
/// ([`Engine::elements_now`]): A1's own to ask the pre-test of, and held to the largest a bound
/// pair is reported with to show.
#[must_use]
fn rails_elements(live: &LiveOrbit, total_msun: f64, eccentricity: f64) -> KeplerElements {
    KeplerElements::from_semi_major_axis(
        live.axis_metres(),
        GravitationalParameter::from_solar_masses(SolarMasses::new(total_msun)),
        Eccentricity::new(eccentricity).expect("A1's bound orbit has an eccentricity in [0, 1)"),
        live.orientation,
        live.mean_anomaly,
    )
    .expect("A1's bound orbit has a positive, finite axis about a positive mass")
}

/// The velocities after member `i`'s explosion with `kick_velocity` (m s⁻¹) where there is no
/// orbit to change: its kick, and nothing for the other, primary first.
#[must_use]
fn alone(i: usize, kick_velocity: [f64; 3]) -> [SystemVelocity; 2] {
    let alone = [SystemVelocity::new(kick_velocity), SystemVelocity::ZERO];
    if i == 0 { alone } else { [alone[1], alone[0]] }
}

/// BSE appendix A1 at a collapse: member `i` of the pair of `masses_msun` (the dying star's its
/// last living mass) leaves `after_msun`, with the natal kick `kick_velocity` (m s⁻¹, along the
/// system frame's axes), where the secondary's position and velocity relative to the primary are
/// `state` (m, m s⁻¹) at universe time `t`, the engine's age `age_years`.
///
/// The mass is lost at once (Blaauw 1961) and the kick added to the exploding star's velocity in
/// the barycentre's frame; the orbit after is the one the new relative state makes about the new
/// mass. It returns that orbit, as the engine carries it from `age_years`, or none if the pair is
/// unbound, and each member's velocity after in the frame of the barycentre before (the pair's
/// recoil twice for a bound pair, their equation A14).
#[expect(
    clippy::many_single_char_names,
    reason = "the names are the symbols of BSE's appendix A1"
)]
#[must_use]
fn appendix_a1(
    i: usize,
    masses_msun: [f64; 2],
    after_msun: f64,
    kick_velocity: [f64; 3],
    state: (SystemVector, SystemVelocity),
    t: UniverseTime,
    age_years: f64,
) -> (Option<LiveOrbit>, [SystemVelocity; 2]) {
    let (masses, after) = (masses_msun, after_msun);
    let (r, v) = state;
    let v_rel = v.metres_per_second();
    let total = masses[0] + masses[1];
    // Velocities in the barycentre's frame before the explosion (secondary relative to
    // primary).
    let mut v0 = v_rel.map(|c| -c * masses[1] / total);
    let mut v1 = v_rel.map(|c| c * masses[0] / total);
    let mut after_masses = masses;
    after_masses[i] = after;
    if i == 0 {
        for (c, k) in v0.iter_mut().zip(kick_velocity) {
            *c += k;
        }
    } else {
        for (c, k) in v1.iter_mut().zip(kick_velocity) {
            *c += k;
        }
    }
    let rel: [f64; 3] = core::array::from_fn(|k| v1[k] - v0[k]);
    let total_after = after_masses[0] + after_masses[1];
    let centre: [f64; 3] =
        core::array::from_fn(|k| (after_masses[0] * v0[k] + after_masses[1] * v1[k]) / total_after);
    let apart = [SystemVelocity::new(v0), SystemVelocity::new(v1)];
    if !positive(after) {
        return (None, apart);
    }
    let mu = GravitationalParameter::from_solar_masses(SolarMasses::new(total_after));
    // A state the inversion refuses is taken as unbound: a radial one (no angular momentum, which
    // a kick leaves with probability zero), or one whose orbit no element set on the universe
    // clock holds (an open orbit's pericentre time off the clock, or a period that overflows),
    // which only an orbit far wider than any bound pair's gives.
    let orbit = elements_from_state(r, SystemVelocity::new(rel), mu, t).ok();
    let recoil = [SystemVelocity::new(centre), SystemVelocity::new(centre)];
    match orbit {
        Some(Orbit::Bound(k)) => {
            let a = k.semi_major_axis().value() / SOLAR_RADIUS_M;
            let e = k.eccentricity().value();
            let orbit =
                LiveOrbit::new(age_years, a, e, *k.orientation(), k.mean_anomaly_at_epoch());
            (Some(orbit), recoil)
        }
        Some(Orbit::Open(o)) if o.eccentricity() < 1.0 => {
            // Bound, but so nearly parabolic that plan 14 carries it open (ruling 39): kept at
            // the largest eccentricity a secular pair is reported with.
            let e = MAX_REPORTED_ECCENTRICITY;
            let a = o.pericentre().value() / SOLAR_RADIUS_M / (1.0 - o.eccentricity());
            let orbit = LiveOrbit::new(age_years, a, e, *o.orientation(), Radians::new(0.0));
            (Some(orbit), recoil)
        }
        Some(Orbit::Open(_)) | None => (None, apart),
    }
}

/// The universe time of the engine's age `age_years` in a pair of `ctx`, from the system's age
/// at the epoch.
#[must_use]
pub(super) fn universe_time_at(ctx: &Context, age_years: f64) -> UniverseTime {
    let years = age_years - ctx.age_at_epoch().value();
    Span::from_seconds_f64(years * SECONDS_PER_JULIAN_YEAR)
        .and_then(|span| UniverseTime::EPOCH.checked_add(span))
        .unwrap_or(UniverseTime::EPOCH)
}

/// A passed-over pair's state for BSE appendix A1 at universe time `t` (P11.T4.l): the secondary's
/// position and velocity relative to the primary on `elements`, the orbit on its rails (its phase
/// then), widened for the mass the stars' winds have taken since the orbit was set, from
/// `mass_on_rails_msun` to `mass_now_msun` (M☉).
///
/// The widening is Jeans's mode with no accretion (Jeans 1924, MNRAS 85, 2; Hurley, Tout and Pols
/// 2002, section 2.2): mass lost isotropically from its star, slowly against the orbital period,
/// keeps a M and e, and the true anomaly is kept, so r × f and v ÷ f with f = `mass_on_rails` ÷
/// `mass_now`, since v² = G M (2/r − 1/a). It is a closed form of the stars' masses at the
/// collapse, so it does not depend on the age asked. It holds while the stars' mass-loss times
/// M ÷ Ṁ, some 10⁴–10⁶ years in a massive star's late phases, far exceed the period; beyond
/// periods of about 10⁷ days it extrapolates, as BSE's orbit-averaged rates do (the science check).
/// The orbit shown before the collapse stays `elements` (design note 7's convention for a
/// passed-over pair).
#[must_use]
pub(super) fn jeans_widened_state(
    elements: &KeplerElements,
    mass_on_rails_msun: f64,
    mass_now_msun: f64,
    t: UniverseTime,
) -> (SystemVector, SystemVelocity) {
    let f = mass_on_rails_msun / mass_now_msun;
    let (r, v) = elements.relative_state_at(t);
    (
        r * f,
        SystemVelocity::new(v.metres_per_second().map(|c| c / f)),
    )
}

/// One core collapse of a passed-over pair's star ([`collapses_before`]).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Collapse {
    /// When, the engine's age, years.
    age_years: f64,
    /// Whose: 0 for the primary, 1 for the secondary.
    member: usize,
    /// The star's last living mass, M☉: its death's progenitor mass, helium core plus envelope.
    before_msun: f64,
    /// What it leaves.
    remnant: CompactRemnant,
    /// Its natal kick, before the engine's switch.
    kick: Option<NatalKick>,
}

/// The core collapses before `until_years` of a passed-over pair of `members`, its stars on their
/// own tracks from zero age, in age order, the primary's first at a tie (P11.T4.l).
///
/// - **The pinned primary** (`pin`, design note 16): plan 06's age, remnant and kick, from the
///   living mass of the pin's own track at its death (P11.T4.k), and the remnant held to that mass,
///   as [`Engine::pinned_collapse`] takes it.
/// - **Any other star:** its own track's sudden death, its own remnant, and its kick by plan 06's
///   law ([`own_kick`]) with its progenitor's own stripping, since no companion took its envelope.
///
/// A white dwarf's birth is no collapse, and a death the track is not built to is not before
/// `until_years`.
#[must_use]
fn collapses_before(
    ctx: &Context,
    members: &[Member; 2],
    pin: Option<Pin>,
    until_years: f64,
) -> Vec<Collapse> {
    let mut found: Vec<Collapse> = (0..2)
        .filter_map(|i| {
            let Member::Track { track, offset } = &members[i] else {
                return None;
            };
            let death = track.death()?;
            let age = offset + death.age().value();
            if !(death.kind().is_sudden() && age < until_years) {
                return None;
            }
            let p = death.progenitor();
            let before = p.helium_core_mass().value() + p.envelope_mass().value();
            if i == 0
                && let Some(pin) = pin
            {
                let at = pin.death.age().value();
                debug_assert!(
                    (at - age).abs() <= clock_resolution_years(at),
                    "the pin at {at} yr is its own track's death at {age} yr"
                );
                return Some(Collapse {
                    age_years: at,
                    member: 0,
                    before_msun: before,
                    remnant: held_to(pin.remnant, before),
                    kick: pin.kick,
                });
            }
            let remnant = track.remnant()?;
            Some(Collapse {
                age_years: age,
                member: i,
                before_msun: before,
                remnant,
                kick: own_kick(ctx, i, &death, &remnant, p.stripping()),
            })
        })
        .collect();
    found.sort_by(|a, b| {
        a.age_years
            .total_cmp(&b.age_years)
            .then(a.member.cmp(&b.member))
    });
    found
}

/// The kick of member `i`'s `remnant` (of a pair of `ctx`) after its own `death`, by plan 06's law
/// under the pair's stellar rules (P06.T19), its progenitor taken with `stripping` (marked [`Stripping::Companion`] where the
/// binary took its envelope); `None` for a death of no collapse or one that leaves nothing.
#[must_use]
fn own_kick(
    ctx: &Context,
    i: usize,
    death: &crate::stellar::remnant::Death,
    remnant: &CompactRemnant,
    stripping: Stripping,
) -> Option<NatalKick> {
    let channel = CollapseChannel::of(death.kind())?;
    if remnant.kind() == RemnantKind::None {
        return None;
    }
    let p = death.progenitor();
    let progenitor = ProgenitorAtDeath::new(
        p.co_core_mass(),
        p.helium_core_mass(),
        p.envelope_mass(),
        stripping,
    );
    Some(StandardKickLaw::under(ctx.rules()).kick(
        channel,
        &progenitor,
        remnant,
        &KickDraws::of(ctx.draws(i)),
    ))
}

/// Whether a passed-over pair of `members` (its stars on their own tracks from zero age, of
/// `input`) can interact on `elements`, the orbit its collapse at `start_years` leaves with the
/// stars' masses `masses_msun` (M☉) then, by `until_years`: design note 7's pre-test asked again
/// of the new orbit, from the collapse (P11.T4.l).
///
/// - **The lobe test** at the new periastron, each star at its largest radius by `until_years`
///   (its own track's [`Track::max_radius_until`], which bounds the span's; a star dead by the
///   collapse its remnant's), and at its least ratio over the span: its least living mass by
///   `until_years` against the other's mass at the collapse (the science check's must-fix on the
///   earlier port: the companion's wind shrinks its lobe as it loses mass).
/// - **P11.T4.j's decay bound** from the collapse, with the wind-spin term
///   ([`decay_reaches`](super::detached::decay_reaches)).
///
/// [`Track::max_radius_until`]: crate::stellar::sse::Track::max_radius_until
#[must_use]
fn interacts_after_collapse(
    input: &BinaryInput,
    members: &[Member; 2],
    elements: &KeplerElements,
    masses_msun: [f64; 2],
    start_years: f64,
    until_years: f64,
) -> bool {
    use super::detached::{TestedOrbit, decay_reaches, wind_loss_msun};
    use super::evolve::{largest_radii_rsun, reaches_lobe_on};

    let (start, until) = (start_years, until_years);
    if start >= until {
        return false;
    }
    let wind_loss_msun =
        core::array::from_fn(|i| wind_loss_msun(&members[i], masses_msun[i], until));
    let least: [f64; 2] = core::array::from_fn(|i| masses_msun[i] - wind_loss_msun[i]);
    let tested = TestedOrbit {
        elements: *elements,
        masses_msun,
        lobe_ratios: [least[0] / masses_msun[1], least[1] / masses_msun[0]],
        wind_loss_msun,
    };
    let mut radii = largest_radii_rsun(input, members, until);
    for (radius, member) in radii.iter_mut().zip(members) {
        if let Member::Track { track, offset } = member
            && track
                .lifetime()
                .is_some_and(|life| offset + life.value() <= start)
        {
            *radius = track.state_at(Years::new(until - offset)).radius().value();
        }
    }
    reaches_lobe_on(&tested, radii)
        || decay_reaches(input.params(), &tested, members, start, until, radii)
}

/// Plan 06's `remnant` for a pinned primary of `mass_msun` M☉ before its collapse: its single
/// star's, and of plan 06's kind at the star's own mass where the binary has stripped it below
/// that, since a star cannot leave more than itself (a finding recorded in plan 11's Risks).
#[must_use]
fn held_to(remnant: CompactRemnant, mass_msun: f64) -> CompactRemnant {
    if remnant.mass().value() > mass_msun {
        CompactRemnant::new(remnant.kind(), SolarMasses::new(mass_msun))
    } else {
        remnant
    }
}

/// The primary's own track at its death now ([`Engine::own_death_now`]).
struct OwnDeath {
    track: std::sync::Arc<sse::Track>,
    /// The track's offset, years, placed so that the track is at its death at the engine's age.
    offset_years: f64,
    death: crate::stellar::remnant::Death,
}

#[cfg(test)]
mod agreement;
