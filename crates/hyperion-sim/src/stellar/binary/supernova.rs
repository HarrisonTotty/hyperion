//! Deaths and supernovae in a binary (P11.T4.e), after Hurley, Tout and Pols (2002, "BSE")
//! section 2.5 and appendix A1.
//!
//! - **A massive primary** (plan 11, design note 16): a primary of `m_cc(Z) − 1` M☉ or more whose
//!   single-star death is a collapse collapses at plan 06's death age with plan 06's remnant and
//!   kick. The engine takes the age as a fixed boundary. If the binary would end the star sooner
//!   (a stripped helium star that dies first, say), the star is held at its last living state until
//!   then; if the pair has merged, the product collapses; if nothing living is left, there is no
//!   collapse.
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
//! - **Accretion-induced collapse** of a white dwarf at the Chandrasekhar mass (section 2.6.5) is
//!   `rlof.rs`'s, through [`Engine::explode`].

use crate::coords::{SystemVector, SystemVelocity};
use crate::orbit::{Orbit, elements_from_state};
use crate::stellar::Phase;
use crate::stellar::remnant::{
    CollapseChannel, CompactRemnant, KickDraws, KickLaw, NatalKick, ProgenitorAtDeath, RemnantKind,
    StandardKickLaw, Stripping,
};
use crate::stellar::sse;
use crate::time::{Span, UniverseTime};
use crate::units::consts::{SECONDS_PER_JULIAN_YEAR, SOLAR_RADIUS_M};
use crate::units::{GravitationalParameter, Megayears, Radians, SolarMasses, Years};

use super::evolve::{Engine, LiveOrbit};
use super::star::{Member, Path, positive};
use super::timeline::{Component, SegmentKind, SupernovaRecord};

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
        // A pinned primary does not die before plan 06's age: it is held at its last state.
        if i == 0 && self.pin.is_some_and(|p| p.death.age().value() > self.age) {
            let before = (self.age - offset) * (1.0 - 1e-12);
            let state = track.state_at(Years::new(before.max(0.0)));
            let (mass, _) = self.current(0);
            let held = crate::stellar::StarState::new(crate::stellar::StarStateParts {
                phase: state.phase(),
                age: state.age(),
                mass: SolarMasses::new(mass.max(state.core_mass().value())),
                core_mass: state.core_mass(),
                luminosity: state.luminosity(),
                radius: state.radius(),
                mass_loss_rate: crate::units::SolarMassesPerYear::ZERO,
                phase_fraction: state.phase_fraction(),
            });
            self.set_member(0, Member::Frozen { state: held });
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
        self.set_member(
            i,
            Member::Track {
                track: std::sync::Arc::clone(&track),
                offset,
            },
        );
        if !death.kind().is_sudden() {
            // A white dwarf born as the envelope goes: continuous, and whatever envelope a companion
            // left on the star goes with it, without a kick.
            let lost = before - remnant.mass().value();
            if lost > 1e-9 && self.orbit.is_some() {
                self.lose_mass(i, before, remnant.mass().value(), None);
            }
            self.kind = self.quiet_kind();
            return;
        }
        let kick = self.companion_kick(i, &death, &remnant);
        self.explode(i, before, remnant, kick);
    }

    /// The kick of member `i`'s `remnant` after its own `death` (plan 06's law, P06.T19), its
    /// progenitor marked companion-stripped if the binary took its envelope.
    #[must_use]
    fn companion_kick(
        &self,
        i: usize,
        death: &crate::stellar::remnant::Death,
        remnant: &CompactRemnant,
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
            if self.stripped[i] {
                Stripping::Companion
            } else {
                p.stripping()
            },
        );
        Some(StandardKickLaw::default().kick(
            channel,
            &progenitor,
            remnant,
            &KickDraws::of(self.ctx.draws(i)),
        ))
    }

    /// The primary's pinned death now (design note 16): plan 06's remnant and kick, whatever the
    /// pair has made of the star.
    pub(super) fn pinned_collapse(&mut self) {
        self.close_segment();
        let Some(pin) = self.pin.take() else {
            return;
        };
        let (mass, tau) = self.current(0);
        let living = self
            .structure(0, self.age, mass, tau)
            .is_some_and(|s| s.state.phase().is_living());
        if !living {
            return;
        }
        // Plan 06's remnant is its single star's; a star the binary has stripped below it cannot
        // leave more than itself, and keeps plan 06's kind at its own mass (a finding recorded in
        // plan 11's Risks).
        let remnant = if pin.remnant.mass().value() > mass {
            CompactRemnant::new(pin.remnant.kind(), SolarMasses::new(mass))
        } else {
            pin.remnant
        };
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

    /// Member `i`, of mass `before`, explodes now into `remnant` with `kick` (BSE appendix A1), the
    /// member already replaced by what it leaves: the orbit after it, or the pair unbound, and the
    /// record.
    pub(super) fn explode(
        &mut self,
        i: usize,
        before: f64,
        remnant: CompactRemnant,
        kick: Option<NatalKick>,
    ) {
        let after = remnant.mass().value();
        let kick = kick.filter(|_| self.ctx.params().natal_kicks);
        let (velocities, bound) = self.lose_mass(i, before, after, kick);
        self.supernovae.push(SupernovaRecord::new(
            Years::new(self.age),
            Component::of_index(i),
            remnant,
            kick,
            bound,
            velocities,
        ));
        self.kind = if bound || self.orbit.is_some() {
            self.quiet_kind()
        } else if self.members.iter().any(Member::is_gone) {
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
    #[expect(
        clippy::many_single_char_names,
        reason = "the names are the symbols of BSE's appendix A1"
    )]
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
        let alone = [SystemVelocity::new(kick_velocity), SystemVelocity::ZERO];
        let alone = if i == 0 { alone } else { [alone[1], alone[0]] };
        if self.orbit.is_none() || self.members[o].is_gone() || !positive(m_other) {
            self.orbit = None;
            return (alone, false);
        }
        let mut masses = [0.0; 2];
        masses[i] = before;
        masses[o] = m_other;
        let Some(elements) = self.elements_now(masses) else {
            self.orbit = None;
            return (alone, false);
        };
        let t = self.universe_time();
        let (r, v) = elements.relative_state_at(t);
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
        let centre: [f64; 3] = core::array::from_fn(|k| {
            (after_masses[0] * v0[k] + after_masses[1] * v1[k]) / total_after
        });
        if !positive(after) {
            self.orbit = None;
            return ([SystemVelocity::new(v0), SystemVelocity::new(v1)], false);
        }
        let mu = GravitationalParameter::from_solar_masses(SolarMasses::new(total_after));
        let orbit = elements_from_state(r, SystemVelocity::new(rel), mu, t).ok();
        match orbit {
            Some(Orbit::Bound(k)) => {
                let a = k.semi_major_axis().value() / SOLAR_RADIUS_M;
                let e = k.eccentricity().value();
                self.orbit = Some(LiveOrbit::new(
                    self.age,
                    a,
                    e,
                    *k.orientation(),
                    k.mean_anomaly_at_epoch(),
                ));
                (
                    [SystemVelocity::new(centre), SystemVelocity::new(centre)],
                    true,
                )
            }
            Some(Orbit::Open(o)) if o.eccentricity() < 1.0 => {
                // Bound, but so nearly parabolic that plan 14 carries it open (ruling 39): kept at
                // the largest eccentricity a secular pair is reported with.
                let e = 0.9998;
                let a = o.pericentre().value() / SOLAR_RADIUS_M / (1.0 - o.eccentricity());
                self.orbit = Some(LiveOrbit::new(
                    self.age,
                    a,
                    e,
                    *o.orientation(),
                    Radians::new(0.0),
                ));
                (
                    [SystemVelocity::new(centre), SystemVelocity::new(centre)],
                    true,
                )
            }
            Some(Orbit::Open(_)) | None => {
                let _: SystemVector = r;
                self.orbit = None;
                ([SystemVelocity::new(v0), SystemVelocity::new(v1)], false)
            }
        }
    }

    /// The universe time of the engine's age now, from the system's age at the epoch.
    #[must_use]
    fn universe_time(&self) -> UniverseTime {
        let years = self.age - self.ctx.age_at_epoch().value();
        Span::from_seconds_f64(years * SECONDS_PER_JULIAN_YEAR)
            .and_then(|span| UniverseTime::EPOCH.checked_add(span))
            .unwrap_or(UniverseTime::EPOCH)
    }

    /// After a member's change of phase: a carried main sequence that has reached its end goes
    /// on along a track of its mass from there (HPT section 7.1), whose mass the binary carries.
    pub(super) fn after_boundary(&mut self) {
        for i in 0..2 {
            let Member::MainSequence { helium, mass, tau } = &self.members[i] else {
                continue;
            };
            if tau.last() < 1.0 {
                continue;
            }
            let (helium, m) = (*helium, mass.last());
            let reach = sse::main_sequence_lifetime(self.ctx.coeffs(), helium, m)
                + (self.until - self.age).max(0.0);
            let (track, phase) = if helium {
                (
                    sse::Track::helium_star(
                        SolarMasses::new(m.min(sse::MAX_INITIAL_MASS.value())),
                        self.ctx.composition(),
                        self.ctx.draws(i),
                        Years::new(reach),
                    ),
                    Phase::HeliumHertzsprungGap,
                )
            } else {
                (
                    sse::Track::to_age(
                        super::evolve::track_mass(SolarMasses::new(m)),
                        self.ctx.composition(),
                        self.ctx.draws(i),
                        Years::new(reach),
                    ),
                    Phase::HertzsprungGap,
                )
            };
            let start = track
                .age_in_phase(phase, 0.0)
                .or_else(|| track.lifetime().map(Years::value))
                .unwrap_or(0.0);
            self.members[i] = Member::Shaped {
                track: std::sync::Arc::new(track),
                offset: super::evolve::offset_for(self.age, start),
                mass: Path::starting(self.age, m),
            };
        }
    }
}
