//! Roche-lobe overflow (P11.T4.c), after Hurley, Tout and Pols (2002, "BSE") section 2.6.
//!
//! - **Onset.** The detached integrator finds it by bisection of its step, where the donor's
//!   radius first reaches its Roche lobe (their section 2.8). An eccentric orbit is circularised
//!   at the onset with its angular momentum kept, and the donor synchronised, as BSE subjects
//!   the rare eccentric case to "instant synchronization at the onset" (section 2.6).
//! - **Stability** (section 2.6.1). Dynamical where the donor's mass ratio q = `M_d` ÷ `M_a` passes
//!   its critical value by type: 0.695 for a low-mass main-sequence star (2.6.4), BSE equation 57
//!   for a giant ((1.67 − x + 2 (Mc ÷ M)⁵) ÷ 2.13, x of HPT equation 47), 0.784 for a helium giant,
//!   4 for a Hertzsprung-gap star (2.6.3) and 0.628 for a white dwarf (2.6.5). The paper gives no
//!   value for a main-sequence star of type 1 or a core-helium-burning star; the published code's
//!   revision of 2001 takes 3 for both, and so does this engine (a main-sequence pair over it comes
//!   into contact, a core-helium-burning donor goes into a common envelope). A Roche lobe inside
//!   the donor's core is a common envelope too, as in the code.
//! - **Stable transfer** (sections 2.6.2 and 2.6.3). BSE transfers mass at a rate that rises
//!   steeply with the overfill, Ṁ = F(M) [ln(R ÷ `R_L`)]³ with F = 3 × 10⁻⁶ min(M, 5)² M☉ yr⁻¹
//!   (equations 58 and 59, raised by 10³ ÷ R for a degenerate donor), held to the thermal rate
//!   M ÷ `τ_KH` for a giant-like donor (equations 60 and 61) and the dynamical rate M ÷ `τ_dyn`
//!   otherwise (equations 62 and 63). BSE steps it explicitly over a few orbits at a time; this
//!   engine takes it implicitly, the rate the overfill at the step's end gives, found by
//!   bisection, which is stable at any step and settles on the same steady overfill. The rate over
//!   each step is the segment's mean rate there. The transfer ends where the donor, given nothing,
//!   would lie inside its lobe by more than BSE's window of 1.002.
//! - **Accretion** (section 2.6.6): a main-sequence, Hertzsprung-gap or core-helium-burning star
//!   takes min(1, 10 `τ_Ṁ` ÷ `τ_KH`) of it (equations 64 and 65) and is rejuvenated; a giant takes all;
//!   a white dwarf fed hydrogen keeps ε of it below 1.03 × 10⁻⁷ M☉ yr⁻¹ (novae, equation 66), all
//!   of it up to 2.71 × 10⁻⁷, and above that swells into a giant; a helium star fed hydrogen
//!   swells into a core-helium-burning or AGB star; degenerate accretors take what the Eddington
//!   limit allows (equations 67 and 68), which is off by default. What is not accreted leaves with
//!   the donor's specific orbital angular momentum, a (M₁ + M₂) = constant for it (section 2.6),
//!   or with the accretor's for novae and super-Eddington loss, as the published code has it.
//!   The donor's transferred spin returns to the orbit and the accretor's spin grows by the
//!   specific angular momentum of the inner disc (equations 54 and 55), up to break-up.
//! - **White dwarfs fed** (section 2.6.6): a helium dwarf that reaches 0.7 M☉ on helium, a
//!   carbon–oxygen dwarf that has taken 0.15 M☉ of helium, and a dwarf at the Chandrasekhar mass
//!   are plan 11's pooled Type Ia candidates (`pooled_ia`); the engine continues as if they did
//!   not explode (P11.T6's mark decides): the helium dwarf ignites as a helium star, the edge-lit
//!   detonation leaves the dwarf accreting, and a dwarf at the Chandrasekhar mass collapses to a
//!   neutron star, as an oxygen–neon dwarf does by accretion-induced collapse (section 2.6.5).
//! - **Contact.** An accretor that fills its own lobe brings the pair into contact (section
//!   2.6.6). Two main-sequence stars stay in contact (ruling 108.1; see `Engine::contact`), and
//!   any other pair collides: a common envelope where either star is giant-like, a coalescence
//!   otherwise.

use std::sync::Arc;

use crate::stellar::Phase;
use crate::stellar::remnant::collapse::{MAX_NEUTRON_STAR_MASS, electron_capture_remnant};
use crate::stellar::remnant::structure::CHANDRASEKHAR_MASS;
use crate::stellar::remnant::{
    CollapseChannel, KickDraws, KickLaw, ProgenitorAtDeath, StandardKickLaw, Stripping,
};
use crate::stellar::sse::{self, NewStar, Structure, Track};
use crate::units::{SolarMasses, Years};

use super::detached::{Snapshot, orbital_momentum, semi_major_axis};
use super::evolve::{Engine, roche_lobe};
use super::star::{G, Kind, Member, Path, Surface, cooling_origin, moment_of_inertia, positive};
use super::timeline::{Component, IaPoolChannel, PooledIaEvent, SegmentKind};

/// The critical mass ratio of a low-mass main-sequence donor (BSE section 2.6.4).
const LOW_MASS_MAIN_SEQUENCE_Q: f64 = 0.695;
/// Of a Hertzsprung-gap donor (BSE section 2.6.3).
const GAP_Q: f64 = 4.0;
/// Of a helium giant (BSE section 2.6.1).
const HELIUM_GIANT_Q: f64 = 0.784;
/// Of a white dwarf (BSE section 2.6.5).
const WHITE_DWARF_Q: f64 = 0.628;
/// Of a main-sequence donor of type 1 onto a main-sequence star and of a core-helium-burning donor
/// (the published code's revision of March 2001; the paper gives none).
const CODE_Q: f64 = 3.0;

/// Below this rate of hydrogen onto a white dwarf, novae (BSE section 2.6.6), M☉ yr⁻¹.
const NOVA_RATE: f64 = 1.03e-7;
/// From this rate a white dwarf fed hydrogen swells into a giant (BSE section 2.6.6), M☉ yr⁻¹.
const GIANT_RATE: f64 = 2.71e-7;
/// A helium white dwarf fed helium ignites at this mass (BSE section 2.6.6), M☉.
const HELIUM_DWARF_IGNITION: f64 = 0.7;
/// A carbon–oxygen white dwarf detonates its accreted helium layer at this mass (BSE section
/// 2.6.6, the edge-lit detonation), M☉.
const EDGE_LIT_HELIUM: f64 = 0.15;

/// The share of the donor's mass one step of stable transfer aims to move (BSE equation 92).
const TRANSFER_STEP: f64 = 0.005;
/// Bisections of a step's transfer.
const TRANSFER_BISECTIONS: u32 = 36;
/// A non-giant donor overfilling its lobe by this factor merges (BSE section 2.6.3).
const OVERFILL_MERGER: f64 = 10.0;
/// The transfer ends where the donor would lie this far inside its lobe, in ln R, after a step
/// with none: BSE's window of 1 ≤ R ÷ `R_L` ≤ 1.002 at the onset (section 2.8), so that a donor at
/// its lobe does not detach and refill at every step.
const DETACHED_BY: f64 = 2e-3;

/// What the onset of transfer, or a step of it, leads to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Stability {
    /// Stable transfer.
    Stable,
    /// A common envelope around the donor (BSE section 2.7.1).
    CommonEnvelope,
    /// Dynamical transfer that merges the pair: a low-mass main-sequence donor (2.6.4), a white
    /// dwarf (2.6.5), a neutron star or black hole, or a main-sequence pair over its critical
    /// ratio.
    Merge,
}

/// How much of the transfer the accretor takes, and what it does.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Accretion {
    /// The share accreted.
    share: f64,
    /// Whether what is lost leaves with the accretor's specific angular momentum (novae,
    /// super-Eddington loss) rather than the donor's.
    from_accretor: bool,
    /// Whether the accretor swells into a giant.
    swells: bool,
}

/// What the trials of one step of stable transfer share ([`Engine::trial_base`]).
#[derive(Debug, Clone, Copy)]
struct TrialBase {
    /// The orbit's angular frequency, rad yr⁻¹.
    omega: f64,
    /// The donor's and the accretor's winds, M☉ yr⁻¹.
    wind: [f64; 2],
    /// The specific angular momentum of the mass lost from the system, R☉² yr⁻¹.
    specific: f64,
    /// The orbit's losses over the step to the winds, gravitational waves and magnetic braking,
    /// in that order, where each is on.
    sinks: [Option<f64>; 3],
    /// The donor's Roche lobe at the step's start, R☉.
    lobe_d: f64,
    /// √(G `M_a` r) at the radius the stream lands on, the accreted mass's specific angular
    /// momentum.
    disc_speed: f64,
    /// The accretor's spin at break-up, if it has one.
    break_up: Option<f64>,
}

impl Engine {
    /// Acts on the onset of Roche-lobe overflow from member `d`.
    pub(super) fn roche_onset(&mut self, d: usize) {
        let a_idx = 1 - d;
        self.close_segment();
        // Circularise at the onset, keeping the orbit's angular momentum (BSE section 2.6).
        if let Some(orbit) = &mut self.orbit
            && orbit.e > 0.0
        {
            let a = orbit.a * (1.0 - orbit.e * orbit.e);
            orbit.set(self.age, a, 0.0);
        }
        self.carry(d);
        self.carry(a_idx);
        self.synchronise(d);
        match self.stability(d) {
            Stability::Stable => {
                self.kind = SegmentKind::StableTransfer {
                    donor: Component::of_index(d),
                };
                self.rates = Some(Path::default());
                self.dt_hint = 0.0;
            }
            Stability::CommonEnvelope => self.common_envelope(d),
            Stability::Merge => self.merge_dynamically(d),
        }
    }

    /// Turns member `i` into the form whose mass the binary carries: a main sequence into a
    /// carried main sequence, a later phase onto its track's closed forms, a remnant into one the
    /// binary feeds.
    pub(super) fn carry(&mut self, i: usize) {
        let age = self.age;
        let Member::Track { track, offset } = &self.members[i] else {
            return;
        };
        let (track, offset) = (Arc::clone(track), *offset);
        let track_age = (age - offset).max(0.0);
        let state = track.state_at(Years::new(track_age));
        let mass = state.mass().value();
        self.members[i] = if state.phase().is_remnant() {
            match track.remnant_clock() {
                Some((phase, birth, origin)) if mass > 0.0 => Member::Remnant {
                    phase,
                    birth: birth + offset,
                    origin,
                    mass: Path::starting(age, mass),
                },
                _ => Member::Gone,
            }
        } else if let Some(tau) = track.main_sequence_fraction(track_age) {
            Member::MainSequence {
                helium: state.phase() == Phase::HeliumMainSequence,
                mass: Path::starting(age, mass),
                tau: Path::starting(age, tau),
            }
        } else {
            Member::Shaped {
                track,
                offset,
                mass: Path::starting(age, mass),
            }
        };
    }

    /// Sets member `i`'s spin to co-rotate with the orbit, from the orbit's angular momentum.
    #[expect(
        clippy::many_single_char_names,
        reason = "the names are BSE's own symbols"
    )]
    pub(super) fn synchronise(&mut self, i: usize) {
        let Some(orbit) = &self.orbit else {
            return;
        };
        let (m, tau) = self.current(i);
        let (mo, _) = self.current(1 - i);
        let Some(s) = self.structure(i, self.age, m, tau) else {
            return;
        };
        let total = m + mo;
        let omega = (G * total / (orbit.a * orbit.a * orbit.a)).sqrt();
        let spin = moment_of_inertia(&s) * omega;
        let j = orbital_momentum(m, mo, orbit.a, orbit.e) + self.spins[i] - spin;
        self.spins[i] = spin.max(1e-10);
        if j > 0.0 {
            let masses = if i == 0 { [m, mo] } else { [mo, m] };
            let e = orbit.e;
            let a = semi_major_axis(j, masses, e);
            let age = self.age;
            if let Some(orbit) = &mut self.orbit {
                orbit.set(age, a, e);
            }
        }
    }

    /// Whether transfer from member `d` is stable now (BSE section 2.6.1; see the module
    /// documentation).
    #[must_use]
    pub(super) fn stability(&self, d: usize) -> Stability {
        let a_idx = 1 - d;
        let (md, td) = self.current(d);
        let (ma, ta) = self.current(a_idx);
        let (Some(sd), Some(sa)) = (
            self.structure(d, self.age, md, td),
            self.structure(a_idx, self.age, ma, ta),
        ) else {
            return Stability::Merge;
        };
        self.stability_of(d, md, ma, &sd, &sa)
    }

    /// [`Engine::stability`] with the donor `d`'s and the accretor's masses now, `md` and `ma`,
    /// and their structures now, `sd` and `sa`, which the caller has evaluated.
    #[must_use]
    fn stability_of(
        &self,
        d: usize,
        md: f64,
        ma: f64,
        sd: &Structure,
        sa: &Structure,
    ) -> Stability {
        let kd = Kind::of(sd.state.phase(), md);
        let ka = Kind::of(sa.state.phase(), ma);
        let q = md / ma;
        let lobe = self.roche_lobe(d, if d == 0 { [md, ma] } else { [ma, md] });
        let inside_core = lobe <= sd.core_radius.value();
        match kd {
            Kind::LowMassMainSequence if q > LOW_MASS_MAIN_SEQUENCE_Q => Stability::Merge,
            Kind::GiantBranch | Kind::EarlyAgb | Kind::PulsingAgb => {
                let share = sd.state.core_mass().value() / md;
                let q_crit =
                    (1.67 - self.ctx.giant_exponent() + 2.0 * crate::math::powi(share, 5)) / 2.13;
                if q > q_crit || inside_core {
                    Stability::CommonEnvelope
                } else {
                    Stability::Stable
                }
            }
            Kind::HeliumGap | Kind::HeliumGiant => {
                if q > HELIUM_GIANT_Q || inside_core {
                    Stability::CommonEnvelope
                } else {
                    Stability::Stable
                }
            }
            Kind::HertzsprungGap if q > GAP_Q => Stability::CommonEnvelope,
            Kind::CoreHeliumBurning if q > CODE_Q => Stability::CommonEnvelope,
            // The published code's q > 3 for a main-sequence pair, taken for any accretor that is
            // not a remnant: a helium star fed at that ratio swells into a giant around its core
            // and the pair comes into contact at once.
            Kind::MainSequence if !ka.is_remnant() && q > CODE_Q => Stability::Merge,
            Kind::HeliumWhiteDwarf | Kind::CarbonOxygenWhiteDwarf | Kind::OxygenNeonWhiteDwarf
                if q > WHITE_DWARF_Q =>
            {
                Stability::Merge
            }
            Kind::NeutronStar | Kind::BlackHole | Kind::Massless => Stability::Merge,
            Kind::LowMassMainSequence
            | Kind::MainSequence
            | Kind::HertzsprungGap
            | Kind::CoreHeliumBurning
            | Kind::HeliumMainSequence
            | Kind::HeliumWhiteDwarf
            | Kind::CarbonOxygenWhiteDwarf
            | Kind::OxygenNeonWhiteDwarf => Stability::Stable,
        }
    }

    /// Runs stable transfer from member `d` to its next event.
    #[expect(
        clippy::too_many_lines,
        reason = "one loop over BSE section 2.6's steps and their exits"
    )]
    pub(super) fn transfer_phase(&mut self, d: usize) {
        let a_idx = 1 - d;
        let mut steps = 0_u32;
        loop {
            if self.age >= self.until || self.capped {
                return;
            }
            if steps > super::evolve::MAX_STEPS {
                self.capped = true;
                return;
            }
            let s = self.snapshot();
            let (Some(sd), Some(sa)) = (
                self.structure(d, s.age, s.masses[d], s.taus[d]),
                self.structure(a_idx, s.age, s.masses[a_idx], s.taus[a_idx]),
            ) else {
                self.collide();
                return;
            };
            // The structures just evaluated are the ones `stability` would evaluate again: the
            // snapshot is the pair now.
            match self.stability_of(d, s.masses[d], s.masses[a_idx], &sd, &sa) {
                Stability::Stable => {}
                Stability::CommonEnvelope => {
                    self.close_segment();
                    self.common_envelope(d);
                    return;
                }
                Stability::Merge => {
                    self.close_segment();
                    self.merge_dynamically(d);
                    return;
                }
            }
            let a = self.orbit.as_ref().map_or(0.0, |o| o.a);
            let lobe_d = roche_lobe(s.masses[d], s.masses[a_idx], a);
            let lobe_a = roche_lobe(s.masses[a_idx], s.masses[d], a);
            let rd = sd.state.radius().value();
            if sa.state.radius().value() >= lobe_a {
                // The last step's rate, before closing the segment takes the path of rates.
                let rate = self.rates.as_ref().map_or(0.0, Path::last);
                self.close_segment();
                self.contact(d, rate);
                return;
            }
            let kd = Kind::of(sd.state.phase(), s.masses[d]);
            if !kd.is_giant_like() && rd > OVERFILL_MERGER * lobe_d {
                self.close_segment();
                self.merge_dynamically(d);
                return;
            }
            let (dt, stop) = self.transfer_limit(&s, d, &sd, &sa);
            let Some((next, rate, accretion, unfed)) = self.transfer_step(&s, d, dt, &sd, &sa)
            else {
                self.close_segment();
                self.collide();
                return;
            };
            if rate <= 0.0 && steps > 0 && unfed < -DETACHED_BY {
                // The donor has shrunk inside its lobe with nothing to give: the transfer is over.
                self.accept(&next);
                self.begin(self.quiet_kind());
                return;
            }
            self.accept(&next);
            self.corotate_donor(d);
            if let Some(rates) = &mut self.rates {
                rates.push(next.age, rate);
            }
            steps += 1;
            self.dt_hint = if rate > 0.0 {
                dt * (TRANSFER_STEP * s.masses[d] / (rate * dt)).clamp(0.5, 2.0)
            } else {
                2.0 * dt
            };
            if self.accretor_events(d, rate, accretion.share * rate * dt, accretion) {
                return;
            }
            if self.donor_events(d) {
                return;
            }
            if let Some(stop) = stop {
                match stop {
                    super::detached::Stop::Until => return,
                    super::detached::Stop::Boundary => {
                        self.close_segment();
                        self.after_boundary();
                        return;
                    }
                    super::detached::Stop::Death(i) => {
                        self.die(i);
                        return;
                    }
                    super::detached::Stop::Pinned => {
                        self.pinned_collapse();
                        return;
                    }
                    super::detached::Stop::Stripped(i) => {
                        self.strip(i);
                        return;
                    }
                    super::detached::Stop::Roche(_)
                    | super::detached::Stop::Collision
                    | super::detached::Stop::Coalescence => {}
                }
            }
        }
    }

    /// The step's length during transfer from member `d` and the event it lands on.
    #[must_use]
    fn transfer_limit(
        &self,
        s: &Snapshot,
        d: usize,
        sd: &Structure,
        sa: &Structure,
    ) -> (f64, Option<super::detached::Stop>) {
        use super::detached::Stop;
        let mut limits = super::detached::StepLimit::new(self.until - s.age);
        if let Some(pin) = &self.pin {
            let at = pin.death.age().value();
            if at > s.age {
                limits.event(at - s.age, Stop::Pinned);
            }
        }
        let structures = [if d == 0 { sd } else { sa }, if d == 0 { sa } else { sd }];
        let mut phase_limit = f64::INFINITY;
        for i in 0..2 {
            match &self.members[i] {
                Member::Track { track, offset } | Member::Shaped { track, offset, .. } => {
                    let (start, end, track_age) = super::evolve::phase_ahead(track, *offset, s.age);
                    if end.is_finite() {
                        let is_death = track
                            .lifetime()
                            .is_some_and(|t| (t.value() - end).abs() <= 1e-9 * end.max(1.0));
                        limits.event(
                            end - track_age,
                            if is_death {
                                Stop::Death(i)
                            } else {
                                Stop::Boundary
                            },
                        );
                        phase_limit = phase_limit.min(0.01 * (end - start));
                    }
                }
                Member::MainSequence { helium, .. } => {
                    let lifetime =
                        sse::main_sequence_lifetime(self.ctx.coeffs(), *helium, s.masses[i]);
                    limits.event((1.0 - s.taus[i]).max(0.0) * lifetime, Stop::Boundary);
                    phase_limit = phase_limit.min(0.02 * lifetime);
                }
                Member::Cooling { .. }
                | Member::Frozen { .. }
                | Member::Remnant { .. }
                | Member::Gone => {}
            }
        }
        let kd = Kind::of(sd.state.phase(), s.masses[d]);
        let thermal = kelvin_helmholtz(structures[d], kd);
        let start = if self.dt_hint > 0.0 {
            self.dt_hint
        } else {
            1e-4 * thermal.min(phase_limit)
        };
        limits.length(start.min(phase_limit).max(1e-6));
        limits.resolve()
    }

    /// One step of `dt` of transfer from member `d` from `s`, with the members' structures `sd` and
    /// `sa` there: the next snapshot, the rate and the accretion (see the module documentation).
    #[must_use]
    fn transfer_step(
        &self,
        s: &Snapshot,
        d: usize,
        dt: f64,
        sd: &Structure,
        sa: &Structure,
    ) -> Option<(Snapshot, f64, Accretion, f64)> {
        let a_idx = 1 - d;
        let (md, ma) = (s.masses[d], s.masses[a_idx]);
        let kd = Kind::of(sd.state.phase(), md);
        let ka = Kind::of(sa.state.phase(), ma);
        // The thermal rate for a giant-like donor, the dynamical rate for any other (BSE
        // equations 60–63).
        let cap = if kd.is_giant_like() {
            md / kelvin_helmholtz(sd, kd)
        } else {
            md / dynamical(sd)
        };
        let envelope = if kd.is_giant_like() {
            (md - sd.state.core_mass().value()).max(0.0)
        } else {
            md
        };
        let x_max = (cap * dt).min(envelope * (1.0 - 1e-9));
        let law = overflow_rate_scale(kd, md, sd.state.radius().value());
        let base = self.trial_base(s, d, dt, sd, sa, kd);
        let trial = |x: f64| self.transfer_trial(s, d, dt, x, &base, sa, kd, ka);
        // Each trial is a pure function of x, so the ends' trials serve as the step itself where
        // the rate is found at an end.
        let zero = trial(0.0)?;
        let unfed = zero.1;
        // BSE's equation 58 taken implicitly, at the step's end: the rate x ÷ dt that
        // F(M) [ln(R ÷ R_L)]³ gives the overfill it leaves. It rises with x on the left and falls
        // on the right, so one root lies in [0, x_max] when the donor overfills unfed.
        let excess_at = |x: f64, fill: f64| {
            let over = fill.max(0.0);
            x / dt - law * over * over * over
        };
        let excess = |x: f64| trial(x).map(|(_, f, _)| excess_at(x, f));
        let (next, x, accretion) = if unfed <= 0.0 {
            (zero.0, 0.0, zero.2)
        } else {
            match trial(x_max) {
                Some((next, f, accretion)) if excess_at(x_max, f) <= 0.0 => {
                    (next, x_max, accretion)
                }
                _ => {
                    let (mut lo, mut hi) = (0.0, x_max);
                    for _ in 0..TRANSFER_BISECTIONS {
                        let mid = lo + 0.5 * (hi - lo);
                        match excess(mid) {
                            Some(g) if g < 0.0 => lo = mid,
                            _ => hi = mid,
                        }
                    }
                    let x = lo + 0.5 * (hi - lo);
                    let (next, _, accretion) = trial(x)?;
                    (next, x, accretion)
                }
            }
        };
        Some((next, x / dt, accretion, unfed))
    }

    /// What every trial of a step of `dt` of transfer from member `d` from `s` shares, with the
    /// members' structures `sd` and `sa` there: all of [`Engine::transfer_trial`] that does not
    /// read the amount tried, evaluated once for the step's bisection rather than once a trial.
    #[must_use]
    fn trial_base(
        &self,
        s: &Snapshot,
        d: usize,
        dt: f64,
        sd: &Structure,
        sa: &Structure,
        kd: Kind,
    ) -> TrialBase {
        let params = self.ctx.params();
        let a_idx = 1 - d;
        let (md, ma) = (s.masses[d], s.masses[a_idx]);
        let total = md + ma;
        let a = semi_major_axis(s.j, s.masses, 0.0);
        let omega = (G * total / (a * a * a)).sqrt();
        let wind = [
            sd.state.mass_loss_rate().value().max(0.0),
            sa.state.mass_loss_rate().value().max(0.0),
        ];
        // The orbit's angular momentum (see the module documentation).
        let specific = ma / total * (ma / total) * a * a * omega;
        let sinks = [
            params.wind_angular_momentum.then(|| {
                (wind[0] * ma * ma + wind[1] * md * md) * a * a * omega / (total * total) * dt
            }),
            params
                .gravitational_radiation
                .then(|| s.j * gravitational_wave_rate(md, ma, a) * dt),
            (params.magnetic_braking && md > 0.35 && !kd.is_remnant()).then(|| {
                let v = sd.state.radius().value().min(roche_lobe(md, ma, a)) * omega;
                5.83e-16 * sd.envelope.mass / md * v * v * v * dt
            }),
        ];
        let lobe_d = roche_lobe(md, ma, a);
        let accretor_radius = Self::accretion_radius(a, md, ma, sa);
        // The accretor spins up to break-up at most, and the excess goes back to the orbit (the
        // published code's RLOF step).
        let inertia = moment_of_inertia(sa);
        let r_a = sa.state.radius().value();
        let break_up =
            (inertia > 0.0 && r_a > 0.0).then(|| inertia * (G * ma / (r_a * r_a * r_a)).sqrt());
        TrialBase {
            omega,
            wind,
            specific,
            sinks,
            lobe_d,
            disc_speed: (G * ma * accretor_radius).sqrt(),
            break_up,
        }
    }

    /// The pair after `dt` of transferring `x` from member `d`, the donor's log overfill of its
    /// lobe then, and the accretion, with the step's `base` ([`Engine::trial_base`]).
    #[expect(
        clippy::too_many_arguments,
        reason = "a step's state, its length, the trial, what the trials share and the two stars"
    )]
    #[must_use]
    fn transfer_trial(
        &self,
        s: &Snapshot,
        d: usize,
        dt: f64,
        x: f64,
        base: &TrialBase,
        sa: &Structure,
        kd: Kind,
        ka: Kind,
    ) -> Option<(Snapshot, f64, Accretion)> {
        let a_idx = 1 - d;
        let (md, ma) = (s.masses[d], s.masses[a_idx]);
        let rate = x / dt;
        let accretion = self.accretion(kd, ka, rate, ma, sa);
        let gained = accretion.share * x;
        let lost = x - gained;
        let wind = base.wind;
        let md2 = (md - x - wind[0] * dt).max(1e-9);
        let ma2 = (ma + gained - wind[1] * dt).max(1e-9);
        let mut j = s.j - lost * base.specific;
        for sink in base.sinks.into_iter().flatten() {
            j -= sink;
        }
        j += x * base.lobe_d * base.lobe_d * base.omega;
        let disc = gained * base.disc_speed;
        j -= disc;
        let mut accretor_spin = s.spins[a_idx] + disc;
        if let Some(break_up) = base.break_up
            && accretor_spin > break_up
        {
            j += accretor_spin - break_up;
            accretor_spin = break_up;
        }
        if !positive(j) {
            return None;
        }
        let mut masses = s.masses;
        masses[d] = md2;
        masses[a_idx] = ma2;
        let mut taus = s.taus;
        taus[d] = self.aged_tau(d, s.taus[d], md2, dt);
        taus[a_idx] = self.rejuvenated_tau(a_idx, s.taus[a_idx], ma, ma2, dt);
        let mut spins = s.spins;
        spins[a_idx] = accretor_spin;
        let next = Snapshot {
            age: s.age + dt,
            j,
            e: 0.0,
            spins,
            masses,
            taus,
        };
        let a2 = semi_major_axis(j, masses, 0.0);
        // Only the donor's radius: a full structure here was most of the engine's time
        // (`benches/binary.rs`, 2026-09-25), since the rate's bisection asks for one per trial.
        let radius = self.members[d].radius(&self.ctx, d, next.age, md2, taus[d])?;
        let lobe = roche_lobe(md2, ma2, a2);
        Some((next, crate::math::ln(radius / lobe), accretion))
    }

    /// The accretion of a transfer at `rate` from a donor of type `kd` onto an accretor of type
    /// `ka`, mass `ma` and structure `sa` (BSE section 2.6.6).
    #[must_use]
    fn accretion(&self, kd: Kind, ka: Kind, rate: f64, ma: f64, sa: &Structure) -> Accretion {
        let params = self.ctx.params();
        let thermal_share = || {
            let tau_mdot = if rate > 0.0 { ma / rate } else { f64::INFINITY };
            (10.0 * tau_mdot / kelvin_helmholtz(sa, ka)).min(1.0)
        };
        let eddington = |share: f64| {
            if !params.eddington_limit || !positive(rate) {
                return (share, false);
            }
            let hydrogen = 0.76 - 3.0 * self.ctx.composition().z_fit().value();
            let limit = 2.08e-3 / (1.0 + hydrogen) * sa.state.radius().value();
            let accreted = (share * rate).min(limit);
            (accreted / rate, accreted < share * rate)
        };
        match ka {
            Kind::LowMassMainSequence
            | Kind::MainSequence
            | Kind::HertzsprungGap
            | Kind::CoreHeliumBurning => Accretion {
                share: thermal_share(),
                from_accretor: false,
                swells: false,
            },
            Kind::HeliumMainSequence | Kind::HeliumGap | Kind::HeliumGiant => {
                if kd.is_helium_star() {
                    Accretion {
                        share: thermal_share(),
                        from_accretor: false,
                        swells: false,
                    }
                } else {
                    Accretion {
                        share: 1.0,
                        from_accretor: false,
                        swells: true,
                    }
                }
            }
            Kind::HeliumWhiteDwarf | Kind::CarbonOxygenWhiteDwarf | Kind::OxygenNeonWhiteDwarf
                if kd.surface() == Surface::Hydrogen =>
            {
                if rate < NOVA_RATE {
                    let (share, _) = eddington(1.0);
                    Accretion {
                        share: share * params.nova_retention,
                        from_accretor: true,
                        swells: false,
                    }
                } else if rate < GIANT_RATE {
                    Accretion {
                        share: 1.0,
                        from_accretor: false,
                        swells: false,
                    }
                } else {
                    let light = (ka == Kind::HeliumWhiteDwarf && ma < 0.05)
                        || (ka != Kind::HeliumWhiteDwarf && ma < 0.5);
                    Accretion {
                        share: 1.0,
                        from_accretor: false,
                        swells: !light,
                    }
                }
            }
            Kind::HeliumWhiteDwarf
            | Kind::CarbonOxygenWhiteDwarf
            | Kind::OxygenNeonWhiteDwarf
            | Kind::NeutronStar
            | Kind::BlackHole => {
                let (share, limited) = eddington(1.0);
                Accretion {
                    share,
                    from_accretor: limited,
                    swells: false,
                }
            }
            Kind::GiantBranch | Kind::EarlyAgb | Kind::PulsingAgb => Accretion {
                share: 1.0,
                from_accretor: false,
                swells: false,
            },
            Kind::Massless => Accretion {
                share: 0.0,
                from_accretor: false,
                swells: false,
            },
        }
    }

    /// The radius, R☉, from which transferred mass lands on the accretor: its surface where it
    /// forms a disc, and the disc's circularisation radius, 1.7 `r_min`, where the stream hits it,
    /// with `r_min` of Ulrich and Burger (1976, eq. 1) as the published code has them.
    #[must_use]
    fn accretion_radius(a: f64, md: f64, ma: f64, sa: &Structure) -> f64 {
        let q = ma / md;
        let r_min = 0.0425 * a * crate::math::powf_positive(q * (1.0 + q), 0.25);
        let r = sa.state.radius().value();
        if r_min > r { r } else { 1.7 * r_min }
    }

    /// τ of member `d`, a donor, after `dt` at mass `m`: a carried main sequence keeps its
    /// fractional age as it loses mass (HPT section 7.1) and advances by dt ÷ `t_MS`.
    #[must_use]
    fn aged_tau(&self, d: usize, tau: f64, m: f64, dt: f64) -> f64 {
        match &self.members[d] {
            Member::MainSequence { helium, .. } => {
                (tau + dt / sse::main_sequence_lifetime(self.ctx.coeffs(), *helium, m)).min(1.0)
            }
            Member::Track { .. }
            | Member::Shaped { .. }
            | Member::Cooling { .. }
            | Member::Frozen { .. }
            | Member::Remnant { .. }
            | Member::Gone => tau,
        }
    }

    /// τ of member `i`, an accretor, from `m0` to `m1` over `dt`: rejuvenated as BSE's code does
    /// (section 2.6.6, after Tout et al. 1997): kept for a star with a radiative core (0.35–1.25
    /// M☉) and lowered by m₀ ÷ m₁ for one whose convective core mixes in the new fuel.
    #[must_use]
    fn rejuvenated_tau(&self, i: usize, tau: f64, m0: f64, m1: f64, dt: f64) -> f64 {
        match &self.members[i] {
            Member::MainSequence { helium, .. } => {
                let mixed = *helium || !(0.35..=1.25).contains(&m1);
                let tau = if mixed && m1 > m0 { tau * m0 / m1 } else { tau };
                (tau + dt / sse::main_sequence_lifetime(self.ctx.coeffs(), *helium, m1)).min(1.0)
            }
            Member::Track { .. }
            | Member::Shaped { .. }
            | Member::Cooling { .. }
            | Member::Frozen { .. }
            | Member::Remnant { .. }
            | Member::Gone => tau,
        }
    }

    /// Holds the donor co-rotating with the orbit during transfer: tides keep it so (BSE section
    /// 2.6), faster than the transfer changes it.
    fn corotate_donor(&mut self, d: usize) {
        let Some(orbit) = &self.orbit else {
            return;
        };
        let (m, tau) = self.current(d);
        let (mo, _) = self.current(1 - d);
        if let Some(s) = self.structure(d, self.age, m, tau) {
            let omega = (G * (m + mo) / (orbit.a * orbit.a * orbit.a)).sqrt();
            self.spins[d] = (moment_of_inertia(&s) * omega).max(1e-10);
        }
    }

    /// Acts on what the accretor did in the last step (see the module documentation): whether
    /// it ended the phase.
    fn accretor_events(&mut self, d: usize, rate: f64, gained: f64, accretion: Accretion) -> bool {
        let a_idx = 1 - d;
        let (ma, ta) = self.current(a_idx);
        let (md, td) = self.current(d);
        let (Some(sa), Some(sd)) = (
            self.structure(a_idx, self.age, ma, ta),
            self.structure(d, self.age, md, td),
        ) else {
            return false;
        };
        let ka = Kind::of(sa.state.phase(), ma);
        let kd = Kind::of(sd.state.phase(), md);
        let masses_now = |me: &Self| {
            let m0 = me.current(0).0;
            let m1 = me.current(1).0;
            [SolarMasses::new(m0), SolarMasses::new(m1)]
        };
        if accretion.swells && rate > 0.0 && self.swell(a_idx, ka, ma, &sa) {
            return false;
        }
        if kd.surface() == Surface::Helium && ka.is_white_dwarf() {
            self.accreted_helium[a_idx] += gained;
        }
        match ka {
            Kind::HeliumWhiteDwarf
                if kd.surface() != Surface::Hydrogen && ma >= HELIUM_DWARF_IGNITION =>
            {
                let masses = masses_now(self);
                self.pool(PooledIaEvent::new(
                    Years::new(self.age),
                    IaPoolChannel::Accretion,
                    [masses[a_idx], masses[d]],
                ));
                self.close_segment();
                let track = Track::helium_star_from(
                    SolarMasses::new(ma),
                    0.0,
                    self.ctx.composition(),
                    self.ctx.draws(a_idx),
                    Some((self.until - self.age).max(0.0)),
                );
                self.set_member(
                    a_idx,
                    Member::Track {
                        track: Arc::new(track),
                        offset: self.age,
                    },
                );
                false
            }
            Kind::CarbonOxygenWhiteDwarf
                if kd.surface() == Surface::Helium
                    && self.accreted_helium[a_idx] >= EDGE_LIT_HELIUM =>
            {
                let masses = masses_now(self);
                self.pool(PooledIaEvent::new(
                    Years::new(self.age),
                    IaPoolChannel::Accretion,
                    [masses[a_idx], masses[d]],
                ));
                false
            }
            Kind::HeliumWhiteDwarf | Kind::CarbonOxygenWhiteDwarf | Kind::OxygenNeonWhiteDwarf
                if ma >= CHANDRASEKHAR_MASS.value() =>
            {
                if ka != Kind::OxygenNeonWhiteDwarf {
                    let masses = masses_now(self);
                    self.pool(PooledIaEvent::new(
                        Years::new(self.age),
                        IaPoolChannel::Accretion,
                        [masses[a_idx], masses[d]],
                    ));
                }
                self.close_segment();
                self.accretion_induced_collapse(a_idx, ma);
                true
            }
            Kind::NeutronStar if ma > MAX_NEUTRON_STAR_MASS.value() => {
                self.close_segment();
                if let Member::Remnant { birth, origin, .. } = &self.members[a_idx] {
                    let (birth, origin) = (*birth, *origin);
                    self.set_member(
                        a_idx,
                        Member::Remnant {
                            phase: Phase::BlackHole,
                            birth,
                            origin,
                            mass: Path::starting(self.age, ma),
                        },
                    );
                }
                false
            }
            _ => false,
        }
    }

    /// Acts on what the donor did in the last step: whether it ended the phase.
    fn donor_events(&mut self, d: usize) -> bool {
        let (md, _) = self.current(d);
        match &self.members[d] {
            Member::Cooling { .. } if md <= 1.01 * crate::stellar::substellar::MIN_MASS.value() => {
                // A donor worn down to a planet's mass is disrupted onto its companion, as BSE's
                // code treats a "planet" in a merger (`mix`).
                let a_idx = 1 - d;
                let (ma, ta) = self.current(a_idx);
                self.close_segment();
                let accretor = self.members[a_idx].restarted(self.age);
                let mut accretor = accretor;
                accretor.record(self.age, ma + md, ta);
                self.finish_merger_with(accretor);
                true
            }
            Member::MainSequence { helium: false, .. } if md < sse::MIN_INITIAL_MASS.value() => {
                self.close_segment();
                let offset = self.age - self.member_age(d).value();
                self.set_member(
                    d,
                    Member::Cooling {
                        offset: offset.min(self.age),
                        mass: Path::starting(self.age, md),
                    },
                );
                false
            }
            Member::MainSequence { helium: true, .. } if md < self.ctx.lightest_helium_star() => {
                self.close_segment();
                let origin = cooling_origin(&self.ctx, Phase::HeliumWhiteDwarf, md, None);
                self.set_member(
                    d,
                    Member::Remnant {
                        phase: Phase::HeliumWhiteDwarf,
                        birth: self.age,
                        origin,
                        mass: Path::starting(self.age, md),
                    },
                );
                self.begin(self.quiet_kind());
                true
            }
            _ => false,
        }
    }

    /// Member `i` of type `kind`, mass `m` and structure `s`, a white dwarf or helium star fed
    /// hydrogen, swells into a giant around its core (BSE sections 2.6.6 and 2.7.4): a helium
    /// dwarf into a giant-branch star, a carbon–oxygen or oxygen–neon dwarf or a helium giant into
    /// a thermally pulsing AGB star, a helium main-sequence star into a core-helium-burning one.
    fn swell(&mut self, i: usize, kind: Kind, m: f64, s: &Structure) -> bool {
        let (target, core) = match kind {
            Kind::HeliumWhiteDwarf => (NewStar::GiantBranch, m),
            Kind::HeliumMainSequence => (NewStar::CoreHeliumBurning { burnt: s.burnt }, m),
            Kind::CarbonOxygenWhiteDwarf | Kind::OxygenNeonWhiteDwarf => (NewStar::PulsingAgb, m),
            Kind::HeliumGap | Kind::HeliumGiant => {
                (NewStar::PulsingAgb, s.state.core_mass().value())
            }
            Kind::LowMassMainSequence
            | Kind::MainSequence
            | Kind::HertzsprungGap
            | Kind::GiantBranch
            | Kind::CoreHeliumBurning
            | Kind::EarlyAgb
            | Kind::PulsingAgb
            | Kind::NeutronStar
            | Kind::BlackHole
            | Kind::Massless => return false,
        };
        // Where no star of the kind has such a core, the accretor keeps the mass as it is (BSE's
        // gntage falls back likewise).
        let Some(member) = self.placed_star(i, target, m, core) else {
            return false;
        };
        self.close_segment();
        self.set_member(i, member);
        true
    }

    /// Member `i`, a white dwarf of mass `m` at the Chandrasekhar mass, collapses to a neutron star
    /// (BSE section 2.6.5): the 1.26 M☉ neutron star of electron capture (plan 06's Mandel and
    /// Müller remnant), with the low-mode kick of accretion-induced collapse (P06.T19).
    pub(super) fn accretion_induced_collapse(&mut self, i: usize, m: f64) {
        let remnant = electron_capture_remnant();
        let law = StandardKickLaw::default();
        let progenitor = ProgenitorAtDeath::new(
            SolarMasses::new(m.min(remnant.mass().value().max(m))),
            SolarMasses::new(m),
            SolarMasses::ZERO,
            if self.stripped[i] {
                Stripping::Companion
            } else {
                Stripping::None
            },
        );
        let kick = law.kick(
            CollapseChannel::AccretionInduced,
            &progenitor,
            &remnant,
            &KickDraws::of(self.ctx.draws(i)),
        );
        self.set_member(
            i,
            Member::Remnant {
                phase: Phase::NeutronStar,
                birth: self.age,
                origin: crate::units::Megayears::ZERO,
                mass: Path::starting(self.age, remnant.mass().value()),
            },
        );
        self.explode(i, m, remnant, Some(kick));
    }
}

/// The Kelvin–Helmholtz timescale of a star of structure `s` and type `kind`, years (BSE
/// equation 61): 10⁷ M `M_x` ÷ (R L), with `M_x` the whole mass for a main-sequence star, a naked
/// helium main-sequence star or a remnant, and the envelope for a giant.
#[must_use]
pub(super) fn kelvin_helmholtz(s: &Structure, kind: Kind) -> f64 {
    let (m, mc) = (s.state.mass().value(), s.state.core_mass().value());
    let (r, l) = (
        s.state.radius().value().max(1e-10),
        s.state.luminosity().value().max(1e-10),
    );
    let share = if kind.is_giant_like() {
        (m - mc).max(1e-10)
    } else {
        m
    };
    1e7 * m * share / (r * l)
}

/// The dynamical timescale of a star of structure `s`, years (BSE equation 63):
/// 5.05 × 10⁻⁵ √(R³ ÷ M).
#[must_use]
pub(super) fn dynamical(s: &Structure) -> f64 {
    let (m, r) = (
        s.state.mass().value().max(1e-10),
        s.state.radius().value().max(1e-10),
    );
    5.05e-5 * (r * r * r / m).sqrt()
}

/// F of BSE equations 58 and 59, M☉ yr⁻¹: 3 × 10⁻⁶ min(M, 5)², raised by 10³ ÷ max(R, 10⁻⁴)
/// for a degenerate donor (types 10 and up), for a donor of type `kind`, mass `m` and radius `r`.
#[must_use]
fn overflow_rate_scale(kind: Kind, m: f64, r: f64) -> f64 {
    let scale = 3e-6 * m.min(5.0) * m.min(5.0);
    if kind.is_remnant() {
        scale * 1e3 / r.max(1e-4)
    } else {
        scale
    }
}

/// The fractional rate of loss of orbital angular momentum to gravitational waves of a circular
/// pair of `m0` and `m1` at `a`, yr⁻¹ (BSE equation 48 at e = 0).
#[must_use]
fn gravitational_wave_rate(m0: f64, m1: f64, a: f64) -> f64 {
    super::detached::GRAVITATIONAL_WAVE_RATE * m0 * m1 * (m0 + m1) / (a * a * a * a)
}
