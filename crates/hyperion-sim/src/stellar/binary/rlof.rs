//! Roche-lobe overflow (P11.T4.c), after Hurley, Tout and Pols (2002, "BSE") section 2.6.
//!
//! - **Onset.** The detached integrator finds it by bisection of its step, where the donor's
//!   radius first reaches its Roche lobe (their section 2.8). An eccentric orbit is circularised
//!   at the onset with its angular momentum kept, and the donor synchronised, as BSE subjects
//!   the rare eccentric case to "instant synchronization at the onset" (section 2.6).
//! - **Stability** (section 2.6.1). Dynamical where the donor's mass ratio q = `M_d` ÷ `M_a` passes
//!   its critical value by type: 0.695 for a low-mass main-sequence star (2.6.4), BSE equation 57
//!   for a giant ((1.67 − x + 2 (Mc ÷ M)⁵) ÷ 2.13, x of HPT equation 47), 0.784 for a helium giant
//!   (but stable at any ratio onto a neutron star or black hole, after Tauris et al. 2015, section
//!   6, a departure from BSE: ruling 129.4b), 4 for a Hertzsprung-gap star (2.6.3) and 0.628 for a
//!   white dwarf (2.6.5). The paper gives no value for a main-sequence star of type 1 or a
//!   core-helium-burning star; the published code's revision of 2001 takes 3 for both, and so does
//!   this engine (a main-sequence pair over it comes into contact, a core-helium-burning donor goes
//!   into a common envelope). A Roche lobe inside the donor's core is a common envelope too, as in
//!   the code.
//! - **Stable transfer** (sections 2.6.2 and 2.6.3). BSE transfers mass at a rate that rises
//!   steeply with the overfill, Ṁ = F(M) [ln(R ÷ `R_L`)]³ with F = 3 × 10⁻⁶ min(M, 5)² M☉ yr⁻¹
//!   (equations 58 and 59, raised by 10³ ÷ R for a degenerate donor), held to the thermal rate
//!   M ÷ `τ_KH` for a giant-like donor (equations 60 and 61) and the dynamical rate M ÷ `τ_dyn`
//!   otherwise (equations 62 and 63). BSE steps it explicitly over a few orbits at a time; this
//!   engine takes it implicitly, the rate the overfill at the step's end gives, found by
//!   bisection, which is stable at any step and settles on the same steady overfill. The rate over
//!   each step is the segment's mean rate there. The transfer ends where the donor, given nothing,
//!   would lie inside its lobe by more than BSE's window of 1.002. A main-sequence donor heavier
//!   than its compact accretor is held to its thermal rate too, and transfers at it wherever a
//!   step at it leaves it overfilling ([`heavy_donor`](super::heavy_donor), P11.T4's fix).
//! - **Accretion** (section 2.6.6): a main-sequence, Hertzsprung-gap or core-helium-burning star
//!   takes min(1, 10 `τ_Ṁ` ÷ `τ_KH`) of it (equations 64 and 65) and is rejuvenated; a giant takes all;
//!   a white dwarf fed hydrogen keeps ε of it below 1.03 × 10⁻⁷ M☉ yr⁻¹ (novae, equation 66), all
//!   of it up to 2.71 × 10⁻⁷, and above that swells into a giant (fed by a donor held to its
//!   thermal rate, it keeps its size and blows the excess off in a wind instead:
//!   [`heavy_donor`](super::heavy_donor), P11.T4's fix); a helium star fed by a
//!   hydrogen-rich donor takes all of it and swells into a core-helium-burning or AGB star, and one
//!   fed by any other donor takes the thermally limited share (BSE `evolv2.f` 1476–1479);
//!   degenerate accretors take what the Eddington limit allows (equations 67 and 68, X the accreted
//!   matter's hydrogen fraction), which is on by default (ruling 132.1, where BSE's table 3 has it
//!   off); a neutron star keeps only a share of it ([`ns_accretion`](super::ns_accretion),
//!   P11.T4's fix). What is not accreted leaves with the
//!   donor's specific orbital angular momentum, a (M₁ + M₂) = constant for it (section 2.6), or
//!   with the accretor's for novae and super-Eddington loss, as the published code has it. The
//!   donor's transferred spin returns to the orbit and the accretor's spin grows by the specific
//!   angular momentum of the inner disc (equations 54 and 55), up to break-up. The loss marked as
//!   the accretor's leaves with the accretor's angular momentum, and onto a compact accretor the
//!   step returns no spin to the orbit and the donor's corotation draws its spin from the orbit,
//!   the pair's total kept, its envelope's spin at its radius held to its Roche lobe as BSE's
//!   transfer reads it; onto any other the donor's corotation after the step is restored with
//!   nothing drawn from the orbit ([`heavy_donor`](super::heavy_donor), P11.T4's fix).
//! - **The swell** (sections 2.6.4, "effectively a reverse of the process which formed the
//!   degenerate remnant star in the first place", and 2.6.6; BSE `evolv2.f` 1480–1490 and
//!   1513–1547, `gntage.f` 99–160; ruling p11-c2-swell of 2026-10-06, P11.T4.m). A fed helium star
//!   or white dwarf becomes a giant around its own core only where the hydrogen accumulates: where
//!   the step's accretion beats 1.05 times the accretor's own net wind over it (BSE's gate for a
//!   helium star, 1480), and the step's net gain beats the new giant's core growth over the step,
//!   the rate at which its hydrogen shell turns the envelope into core (the ruling's burn gate, in
//!   place of BSE's 10⁻⁴ of the mass an orbit, 1481, whose threshold rate scales as 1 ÷ P; after
//!   Nomoto, Saio, Kato and Hachisu 2007, ApJ 663, 1269, section 3: above their `Ṁ_RG` "the
//!   accreted matter is accumulated faster than consumed into He by H-shell burning" and piles up
//!   into a red giant's envelope). BSE swells a white dwarf fed at 2.71 × 10⁻⁷ M☉ yr⁻¹ or more
//!   with no gate (1518–1547); the ruling applies both to it too. The giant's core is the
//!   accretor's mass at the step's start, a helium giant's carbon–oxygen core there, placed exactly
//!   (`Engine::swelled_star`); its mass is the mass after the step, so its envelope is the step's
//!   net gain. Otherwise the accretor keeps its type and the step's mass, burning the hydrogen to
//!   helium as it lands or losing it to its wind, as BSE below its gate and COMPAS always do for a
//!   helium star; a swell refused closes no segment. From generator version 29 a helium dwarf
//!   above `Mc,BGB`(`M_HeF`) swells onto a degenerate branch instead
//!   ([`swollen_dwarf`](super::swollen_dwarf), P11.T4's fix).
//! - **White dwarfs fed** (section 2.6.6): a helium dwarf that reaches 0.7 M☉ on helium, a
//!   carbon–oxygen dwarf that has taken 0.15 M☉ of helium, and a dwarf at the Chandrasekhar mass
//!   are plan 11's pooled Type Ia candidates (`pooled_ia`); the engine continues as if they did
//!   not explode (P11.T6's mark decides): the helium dwarf ignites as a helium star, the edge-lit
//!   detonation leaves the dwarf accreting, and a dwarf at the Chandrasekhar mass collapses to a
//!   neutron star, as an oxygen–neon dwarf does by accretion-induced collapse (section 2.6.5).
//! - **Contact.** An accretor that fills its own lobe brings the pair into contact (section
//!   2.6.6). Two main-sequence stars stay in contact (ruling 108.1; see `Engine::contact`), and
//!   any other pair collides: a common envelope where either star is giant-like, a coalescence
//!   otherwise. A rapid contact whose accretor overfills its lobe by no more than 10% is
//!   temporary, and the transfer goes on (ruling 114.2; `Engine::contact_relaxes`).

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
use super::dwarf_limit;
use super::evolve::{Engine, roche_lobe};
use super::heavy_donor;
use super::ns_accretion::NS_ACCRETION_EFFICIENCY;
use super::star::{
    G, Kind, Member, MemberRadiusAt, Path, Surface, cooling_origin, moment_of_inertia,
    moment_of_inertia_within, positive,
};
use super::swollen_dwarf::SwollenDwarfRules;
use super::timeline::{Component, IaPoolChannel, PooledIaEvent, SegmentKind};

/// The critical mass ratio of a low-mass main-sequence donor (BSE section 2.6.4).
const LOW_MASS_MAIN_SEQUENCE_Q: f64 = 0.695;
/// Of a Hertzsprung-gap donor (BSE section 2.6.3). Plan 08's stripping band reads it (ruling
/// 123.2).
pub(crate) const GAP_Q: f64 = 4.0;
/// Of a helium Hertzsprung-gap star or giant onto any accretor but a neutron star or black hole
/// (BSE section 2.6.1).
const HELIUM_GIANT_Q: f64 = 0.784;
/// Of a white dwarf (BSE section 2.6.5).
const WHITE_DWARF_Q: f64 = 0.628;
/// Of a main-sequence donor of type 1 onto a main-sequence star and of a core-helium-burning donor
/// (the published code's revision of March 2001; the paper gives none). Plan 08's stripping band
/// reads it (ruling 123.2).
pub(crate) const CODE_Q: f64 = 3.0;

/// Below this rate of hydrogen onto a white dwarf, novae (BSE section 2.6.6), M☉ yr⁻¹.
pub(crate) const NOVA_RATE: f64 = 1.03e-7;
/// From this rate a white dwarf fed hydrogen swells into a giant (BSE section 2.6.6), M☉ yr⁻¹.
const GIANT_RATE: f64 = 2.71e-7;
/// The scale, M☉ yr⁻¹, of a white dwarf's critical rate ([`critical_rate`]): Hachisu, Kato and
/// Nomoto 1999 (ApJ 522, 487, equation 5), whose form with the accreted hydrogen fraction X
/// generalises Hachisu, Kato and Nomoto 1996's (ApJ 470, L97) 0.75 × 10⁻⁶ at X = 0.70 (recalled,
/// not read at source: the pod reached no literature).
const CRITICAL_RATE_SCALE: f64 = 5.3e-7;
/// The white dwarf's mass, M☉, at which its critical rate ([`critical_rate`]) falls to nothing
/// (Hachisu, Kato and Nomoto 1996, ApJ 470, L97; recalled, as [`CRITICAL_RATE_SCALE`]).
const CRITICAL_RATE_ZERO_MASS: f64 = 0.4;
/// A fed helium star or white dwarf swells only where the step's accretion exceeds this many times
/// its own net wind over the step (BSE `evolv2.f` 1480, `dmchk = dm2 - 1.05d0*dms(j2)`, for a
/// helium star; the paper, section 2.6.6, states no gate; ruling p11-c2-swell of 2026-10-06, gate
/// (a), which applies it to a white dwarf too).
const SWELL_WIND_FACTOR: f64 = 1.05;
/// A helium white dwarf fed helium ignites at this mass (BSE section 2.6.6), M☉.
const HELIUM_DWARF_IGNITION: f64 = 0.7;
/// A carbon–oxygen white dwarf detonates its accreted helium layer at this mass (BSE section
/// 2.6.6, the edge-lit detonation), M☉.
const EDGE_LIT_HELIUM: f64 = 0.15;

/// The share of the donor's mass one step of stable transfer aims to move (BSE equation 92).
const TRANSFER_STEP: f64 = 0.005;
/// The most times a step of transfer that moves more than 2 × [`TRANSFER_STEP`] of the donor is
/// taken again at half its length (finding F17, ruling p11-c2-swell §4.4; P11.T4.o): from a step
/// of the longest a transfer takes, 10⁹ years, to one of about 4 Myr.
const TRANSFER_HALVINGS: u32 = 8;
/// Bisections of a step's transfer.
const TRANSFER_BISECTIONS: u32 = 36;
/// A non-giant donor overfilling its lobe by this factor merges (BSE section 2.6.3).
const OVERFILL_MERGER: f64 = 10.0;
/// The transfer ends where the donor would lie this far inside its lobe, in ln R, after a step
/// with none: BSE's window of 1 ≤ R ÷ `R_L` ≤ 1.002 at the onset (section 2.8), so that a donor at
/// its lobe does not detach and refill at every step.
const DETACHED_BY: f64 = 2e-3;

#[cfg(test)]
thread_local! {
    /// The transfer steps this thread has started from a donor with nothing living
    /// ([`Engine::transfer_phase`]'s backstop, P11.T4.o), which the tests count; the engine never
    /// reads it.
    pub(super) static DEAD_DONOR_STEPS: core::cell::Cell<u32> = const { core::cell::Cell::new(0) };
}

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

/// The mass ratio, donor over accretor, above which transfer from a giant-branch or asymptotic
/// giant-branch donor whose core is `core_share` of its mass is dynamically unstable, at the giant
/// radius exponent `x`: (1.67 − x + 2 `core_share`⁵) ÷ 2.13 (BSE section 2.6.1, after Hjellming and
/// Webbink 1987). [`Engine::stability`] reads it, and so does the stellar merger class's evolved
/// donor (P11.T8.c's channels), which draws its companion inside it.
#[must_use]
pub(super) fn giant_critical_ratio(core_share: f64, x: f64) -> f64 {
    (1.67 - x + 2.0 * crate::math::powi(core_share, 5)) / 2.13
}

/// How much of the transfer the accretor takes, and what it does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Accretion {
    /// The share accreted.
    pub(super) share: f64,
    /// Whether what is lost leaves with the accretor's specific angular momentum (novae,
    /// super-Eddington loss) rather than the donor's.
    pub(super) from_accretor: bool,
    /// Whether the accretor may swell into a giant ([`Engine::swell`]).
    pub(super) swells: bool,
}

/// What one step of stable transfer gave its accretor, which the swell's gates read
/// ([`Engine::swell`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Fed {
    /// The accretor's type at the step's start.
    pub(super) kind: Kind,
    /// The accretor's mass at the step's start, M☉.
    pub(super) before: f64,
    /// Its core's mass at the step's start, M☉: a helium giant's carbon–oxygen core.
    pub(super) core_before: f64,
    /// The mass it accreted over the step, M☉: its share of the mass transferred.
    pub(super) gained: f64,
    /// The step's length, years.
    pub(super) dt: f64,
}

/// What the trials of one step of stable transfer share ([`Engine::trial_base`]).
#[derive(Debug)]
struct TrialBase<'e> {
    /// The orbit's angular frequency, rad yr⁻¹.
    omega: f64,
    /// The donor's and the accretor's winds, M☉ yr⁻¹.
    wind: [f64; 2],
    /// The specific angular momentum of the mass lost from the system, R☉² yr⁻¹: the donor's.
    specific: f64,
    /// The accretor's specific orbital angular momentum, R☉² yr⁻¹, which what is lost from it takes
    /// (P11.T4's fix, `heavy_donor`).
    accretor_specific: f64,
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
    /// The donor's radius at the step's end, for the mass each trial leaves it.
    donor_radius: MemberRadiusAt<'e>,
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
        let main_sequence = match &self.members[d] {
            Member::MainSequence { helium, .. } => {
                sse::main_sequence_lifetime(self.ctx.coeffs(), *helium, self.current(d).0)
            }
            Member::Track { .. }
            | Member::Shaped { .. }
            | Member::Cooling { .. }
            | Member::Frozen { .. }
            | Member::Remnant { .. }
            | Member::Gone => 0.0,
        };
        self.overflow_onset = (self.age, main_sequence);
        self.first_contact = None;
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
    /// binary feeds. A star that has not yet arrived on its main sequence becomes its zero-age
    /// main-sequence star at τ = 0, which it was to the engine already
    /// ([`engine_track_age_years`](super::star::engine_track_age_years), P11.T4.i), and ages from
    /// there.
    pub(super) fn carry(&mut self, i: usize) {
        let age = self.age;
        let Member::Track { track, offset } = &self.members[i] else {
            return;
        };
        let (track, offset) = (Arc::clone(track), *offset);
        let track_age = super::star::engine_track_age_years(&track, offset, age);
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
    pub(super) fn synchronise(&mut self, i: usize) {
        self.synchronise_by(i, |s, _| moment_of_inertia(s));
    }

    /// [`Engine::synchronise`] for donor `d` after a step of stable transfer onto a compact
    /// accretor (P11.T4's fix, `heavy_donor`), its envelope's spin at its radius held to its Roche
    /// lobe ([`moment_of_inertia_within`]).
    ///
    /// That is how BSE's transfer reads a donor that overfills its lobe. A case BB helium giant
    /// stripped toward its core swells, by HPT's helium-giant radius (their equations 85–88), to
    /// hundreds of R☉ about an orbit of a few, and drawn from the orbit at that radius its
    /// corotation drained the orbit to nothing, step by step, and merged the pair (P11.T10's kicks
    /// at version 25).
    fn synchronise_donor(&mut self, d: usize) {
        self.synchronise_by(d, moment_of_inertia_within);
    }

    /// [`Engine::synchronise`] with member `i`'s moment of inertia `inertia` of its structure and
    /// its Roche lobe in R☉.
    #[expect(
        clippy::many_single_char_names,
        reason = "the names are BSE's own symbols"
    )]
    fn synchronise_by(&mut self, i: usize, inertia: impl Fn(&Structure, f64) -> f64) {
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
        let spin = inertia(&s, roche_lobe(m, mo, orbit.a)) * omega;
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
                let q_crit = giant_critical_ratio(share, self.ctx.giant_exponent());
                if q > q_crit || inside_core {
                    Stability::CommonEnvelope
                } else {
                    Stability::Stable
                }
            }
            // Case BB onto a neutron star or black hole is stable at any ratio (ruling 129.4b):
            // Tauris, Langer and Podsiadlowski (2015, section 6) find "the RLO is always
            // dynamically stable such that these systems avoid evolving into yet another CE", and
            // Vigna-Gómez et al. (2018, section 2.2.5) take it so. A departure from BSE, whose
            // 0.784 stays for every other accretor.
            Kind::HeliumGap | Kind::HeliumGiant
                if matches!(ka, Kind::NeutronStar | Kind::BlackHole) && !inside_core =>
            {
                Stability::Stable
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
    ///
    /// A step whose limit is a death ([`Stop::Death`]) or the primary's pin ([`Stop::Pinned`])
    /// is never ended by the detachment test: the dying donor reads inside its lobe as its
    /// remnant there, and the death or the pin would otherwise never be seen again. It goes
    /// through the step's acts like any other step, T4.g's strip first, and its stop is acted on.
    /// Where that leaves the donor with nothing living, the transfer ends there, as
    /// [`Engine::quiet_kind`] decides (P11.T4.k, ruling p11-t4k-faults of 2026-10-06, finding A:
    /// before it the "transfer is over" return came first, which lost the collapses of case BB
    /// helium donors onto neutron stars and black holes, most of which still transfer when they
    /// collapse, 39 of Tauris, Langer and Podsiadlowski's 2015 table 1's 47, and of pinned transfer
    /// donors). The landing step reads the dying donor's overfill at its death, a remnant's radius,
    /// so it moves no mass: the donor keeps about one step's transfer, BSE equation 92's target of
    /// 0.5% of its mass, in its mass before (a known departure, recorded in plan 11's Risks).
    ///
    /// [`Stop::Death`]: super::detached::Stop::Death
    /// [`Stop::Pinned`]: super::detached::Stop::Pinned
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
            // A transfer whose donor has died has ended (below), so a donor with nothing living
            // here died inside the last step without its limit landing on the death: it dies
            // now, as at `Stop::Death`, dated at that step's end, at most a step late (P11.T4.o;
            // the backstop, which no known pair reaches since `Engine::after_boundary` acts on a
            // death at a main sequence's end). A white dwarf gives mass as a remnant (BSE section
            // 2.6.5).
            if !(sd.state.phase().is_living()
                || Kind::of(sd.state.phase(), s.masses[d]).is_white_dwarf())
            {
                #[cfg(test)]
                DEAD_DONOR_STEPS.with(|n| n.set(n.get() + 1));
                self.die(d);
                self.end_without_a_living_donor(d);
                return;
            }
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
                if self.accretor_contact(d, rate) {
                    return;
                }
            }
            let kd = Kind::of(sd.state.phase(), s.masses[d]);
            if !kd.is_giant_like() && rd > OVERFILL_MERGER * lobe_d {
                self.close_segment();
                self.merge_dynamically(d);
                return;
            }
            let (mut dt, mut stop) = self.transfer_limit(&s, d, &sd, &sa);
            let mut solved = self.transfer_step(&s, d, dt, &sd, &sa);
            // A step that moves more than twice BSE equation 92's share of the donor is taken
            // again at half its length, which then lands on no event (finding F17, P11.T4.o):
            // the accretion's regime and the swell are decided on the step's rate.
            for _ in 0..TRANSFER_HALVINGS {
                match solved {
                    Some((_, rate, ..)) if rate * dt > 2.0 * TRANSFER_STEP * s.masses[d] => {
                        dt *= 0.5;
                        stop = None;
                        solved = self.transfer_step(&s, d, dt, &sd, &sa);
                    }
                    Some(_) | None => break,
                }
            }
            let Some((mut next, rate, accretion, unfed)) = solved else {
                self.close_segment();
                self.collide();
                return;
            };
            let stop = super::detached::main_sequence_end_landed(stop, &s, &mut next);
            let lands_on_death = matches!(
                stop,
                Some(super::detached::Stop::Death(_) | super::detached::Stop::Pinned)
            );
            if rate <= 0.0 && steps > 0 && unfed < -DETACHED_BY && !lands_on_death {
                // The donor has shrunk inside its lobe with nothing to give: the transfer is over.
                self.accept(&next);
                if self.age < self.until {
                    self.begin(self.quiet_kind());
                }
                return;
            }
            self.accept(&next);
            let ka = Kind::of(sa.state.phase(), s.masses[a_idx]);
            if heavy_donor::corotation_from_orbit(ka) {
                self.synchronise_donor(d);
            } else {
                self.corotate_donor(d);
            }
            if let Some(rates) = &mut self.rates {
                rates.push(next.age, rate);
            }
            steps += 1;
            self.dt_hint = if rate > 0.0 {
                dt * (TRANSFER_STEP * s.masses[d] / (rate * dt)).clamp(0.5, 2.0)
            } else {
                2.0 * dt
            };
            // The step that reaches or passes the pair's age is the last, whatever it landed on,
            // which lies beyond the timeline (`detached::StepLimit`).
            if self.age >= self.until {
                return;
            }
            let fed = Fed {
                kind: Kind::of(sa.state.phase(), s.masses[a_idx]),
                before: s.masses[a_idx],
                core_before: sa.state.core_mass().value(),
                gained: accretion.share * rate * dt,
                dt,
            };
            if self.accretor_events(d, rate, fed, accretion) {
                return;
            }
            if self.donor_events(d) {
                return;
            }
            // A star the binary carries, donor or accretor, with no envelope left is a helium star
            // or a white dwarf from this step, before any other stop (HPT section 6; BSE `hrdiag`
            // as `evolv2` calls it; P11.T4.g): it is stripped, and the pair goes on as
            // `quiet_kind` decides.
            if let Some(i) = [d, a_idx].into_iter().find(|&i| self.bare_shaped(i)) {
                self.strip(i);
                return;
            }
            if let Some(stop) = stop {
                match stop {
                    // No step limit is the pair's age (`detached::StepLimit`).
                    super::detached::Stop::Until => return,
                    super::detached::Stop::Boundary | super::detached::Stop::MainSequenceEnd(_) => {
                        self.close_segment();
                        self.after_boundary();
                        self.end_without_a_living_donor(d);
                        return;
                    }
                    super::detached::Stop::Death(i) => {
                        self.die(i);
                        self.end_without_a_living_donor(d);
                        return;
                    }
                    super::detached::Stop::Pinned => {
                        self.pinned_collapse();
                        self.end_without_a_living_donor(d);
                        return;
                    }
                    super::detached::Stop::PinnedMainSequenceEnd => {
                        self.end_pinned_main_sequence();
                        return;
                    }
                    super::detached::Stop::Stripped(i) => {
                        self.strip(i);
                        return;
                    }
                    super::detached::Stop::CoreLimit(i) => {
                        self.reach_core_limit(i);
                        self.end_without_a_living_donor(d);
                        return;
                    }
                    // A fed dwarf at the Chandrasekhar mass collapses in the accretor's events.
                    super::detached::Stop::Roche(_)
                    | super::detached::Stop::Collision
                    | super::detached::Stop::Coalescence
                    | super::detached::Stop::Chandrasekhar(_) => {}
                }
            }
        }
    }

    /// Ends the transfer from member `d` if a death or the pin it has just acted on left that
    /// donor with nothing living, the pair going on as [`Engine::quiet_kind`] decides: a remnant
    /// read as the donor of a next transfer step would merge the pair ([`Engine::stability_of`]).
    pub(super) fn end_without_a_living_donor(&mut self, d: usize) {
        if !matches!(self.kind, SegmentKind::StableTransfer { .. }) {
            return;
        }
        let (m, tau) = self.current(d);
        let living = self
            .structure(d, self.age, m, tau)
            .is_some_and(|s| s.state.phase().is_living());
        if !living {
            self.begin(self.quiet_kind());
        }
    }

    /// Whether member `i` is a star the binary carries ([`Member::Shaped`]) that is living with no
    /// envelope left at the engine's age (M ≤ Mc).
    #[must_use]
    fn bare_shaped(&self, i: usize) -> bool {
        if !matches!(self.members[i], Member::Shaped { .. }) {
            return false;
        }
        let (m, tau) = self.current(i);
        self.structure(i, self.age, m, tau)
            .is_some_and(|s| s.state.envelope_mass().value() <= 0.0 && s.state.phase().is_living())
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
        let mut limits = super::detached::StepLimit::new();
        if let Some(pin) = &self.pin {
            let at = pin.death.age().value();
            if at > s.age {
                limits.event(at - s.age, Stop::Pinned);
            }
        }
        let structures = [if d == 0 { sd } else { sa }, if d == 0 { sa } else { sd }];
        if let Some(at) = self.own_main_sequence_end_after(s.age, Some(structures[0].state.phase()))
        {
            limits.event(at - s.age, Stop::PinnedMainSequenceEnd);
        }
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
                    let lifetime = self.main_sequence_lifetime(*helium, s.masses[i]);
                    limits.event(
                        (1.0 - s.taus[i]).max(0.0) * lifetime,
                        Stop::MainSequenceEnd(i),
                    );
                    phase_limit = phase_limit.min(0.02 * lifetime);
                }
                Member::Cooling { .. }
                | Member::Frozen { .. }
                | Member::Remnant { .. }
                | Member::Gone => {}
            }
            if let Some(at) = self.core_limit_age(i) {
                limits.event((at - s.age).max(0.0), Stop::CoreLimit(i));
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
        limits.length(super::detached::LONGEST_STEP_YEARS);
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
        // equations 60–63); and, P11.T4's fix, the thermal rate for a main-sequence donor heavier
        // than its compact accretor, at which it transfers wherever a step at it leaves it
        // overfilling (`heavy_donor`).
        let thermal = heavy_donor::thermal_rate(kd, ka, md, ma);
        let cap = if kd.is_giant_like() || thermal {
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
                Some((next, f, accretion))
                    if excess_at(x_max, f) <= 0.0 || (thermal && f > 0.0) =>
                {
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
    ) -> TrialBase<'_> {
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
        let accretor_specific = md / total * (md / total) * a * a * omega;
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
            accretor_specific,
            sinks,
            lobe_d,
            disc_speed: (G * ma * accretor_radius).sqrt(),
            break_up,
            donor_radius: self.members[d].radius_at_age(s.age + dt),
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
        base: &TrialBase<'_>,
        sa: &Structure,
        kd: Kind,
        ka: Kind,
    ) -> Option<(Snapshot, f64, Accretion)> {
        let a_idx = 1 - d;
        let (md, ma) = (s.masses[d], s.masses[a_idx]);
        let rate = x / dt;
        let accretion = self.accretion(kd, md, ka, rate, ma, sa);
        let gained = accretion.share * x;
        let lost = x - gained;
        let wind = base.wind;
        let md2 = (md - x - wind[0] * dt).max(1e-9);
        let ma2 = (ma + gained - wind[1] * dt).max(1e-9);
        let specific = if accretion.from_accretor {
            base.accretor_specific
        } else {
            base.specific
        };
        let mut j = s.j - lost * specific;
        for sink in base.sinks.into_iter().flatten() {
            j -= sink;
        }
        // The donor's transferred spin, returned to the orbit here but onto a compact accretor,
        // whose corotation after the step draws the donor's spin from the orbit instead (P11.T4's
        // fix, `heavy_donor`).
        if !heavy_donor::corotation_from_orbit(ka) {
            j += x * base.lobe_d * base.lobe_d * base.omega;
        }
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
        // A main sequence whose end is reached to 10⁻⁹ of the age is handed on at τ = 1 (ruling
        // 132.3).
        for i in [d, a_idx] {
            taus[i] = self.main_sequence_tau(i, taus[i], masses[i], s.age + dt);
        }
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
        // (`benches/binary.rs`, 2026-09-25), since the rate's bisection asks for one per trial;
        // and of that, only what the trial's mass moves (R06.T16.a's cost). `next.age` is the
        // step's end, at which `trial_base` prepared it.
        let radius = base.donor_radius.radius(&self.ctx, d, md2, taus[d])?;
        let lobe = roche_lobe(md2, ma2, a2);
        Some((next, crate::math::ln(radius / lobe), accretion))
    }

    /// The accretion of a transfer at `rate` from a donor of type `kd` and mass `md` onto an
    /// accretor of type `ka`, mass `ma` and structure `sa` (BSE section 2.6.6).
    ///
    /// A helium star takes the whole transfer, and may swell ([`Engine::swell`]), only from a
    /// hydrogen-rich donor (BSE `evolv2.f` 1478–1479, the else branch of `kstar(j1).ge.7`; Hurley,
    /// Tout and Pols 2002, section 2.6.6; ruling p11-c2-swell of 2026-10-06, item B): from a helium
    /// star, a white dwarf or anything else it takes the thermally limited share (1477), as from a
    /// helium donor.
    ///
    /// A white dwarf fed hydrogen at [`GIANT_RATE`] or more by a donor held to its thermal rate
    /// (P11.T4's fix, [`thermal_rate`](super::heavy_donor::thermal_rate)) does not swell ([`Engine::optically_thick_wind`]). Every other white dwarf fed so swells as BSE
    /// has it; the general wind against the swell stays with the calibration (ruling p11-c2-swell
    /// §6).
    #[must_use]
    pub(super) fn accretion(
        &self,
        kd: Kind,
        md: f64,
        ka: Kind,
        rate: f64,
        ma: f64,
        sa: &Structure,
    ) -> Accretion {
        let params = self.ctx.params();
        let thermal_share = || {
            let tau_mdot = if rate > 0.0 { ma / rate } else { f64::INFINITY };
            (10.0 * tau_mdot / kelvin_helmholtz(sa, ka)).min(1.0)
        };
        let eddington = |share: f64| {
            if !params.eddington_limit || !positive(rate) {
                return (share, false);
            }
            let limit = eddington_rate(&self.ctx, kd.surface(), sa.state.radius().value());
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
                if kd.surface() == Surface::Hydrogen {
                    Accretion {
                        share: 1.0,
                        from_accretor: false,
                        swells: true,
                    }
                } else {
                    Accretion {
                        share: thermal_share(),
                        from_accretor: false,
                        swells: false,
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
                } else if heavy_donor::thermal_rate(kd, ka, md, ma) {
                    self.optically_thick_wind(kd, rate, ma)
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
            | Kind::BlackHole => {
                let (share, limited) = eddington(1.0);
                Accretion {
                    share,
                    from_accretor: limited,
                    swells: false,
                }
            }
            // A neutron star keeps a share of it, below its Eddington rate (P11.T4's fix,
            // `ns_accretion`).
            Kind::NeutronStar => Accretion {
                share: eddington(NS_ACCRETION_EFFICIENCY).0,
                from_accretor: true,
                swells: false,
            },
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

    /// The accretion of a white dwarf of `ma` M☉ fed hydrogen by a donor of type `kd` at `rate`, at
    /// least [`GIANT_RATE`], that is held to its thermal rate (P11.T4's fix, [`Engine::accretion`]):
    /// it keeps the larger of its [`critical_rate`] and [`GIANT_RATE`], blows the rest off from
    /// itself in an optically thick wind and keeps its size (Hachisu, Kato and Nomoto 1996, ApJ
    /// 470, L97).
    #[must_use]
    fn optically_thick_wind(&self, kd: Kind, rate: f64, ma: f64) -> Accretion {
        let hydrogen = hydrogen_fraction(&self.ctx, kd.surface());
        let kept = critical_rate(ma, hydrogen).max(GIANT_RATE);
        Accretion {
            share: (kept / rate).min(1.0),
            from_accretor: true,
            swells: false,
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
                (tau + dt / self.main_sequence_lifetime(*helium, m)).min(1.0)
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
    ///
    /// A helium main-sequence star keeps its fraction burnt, as BSE's code does (`evolv2.f`
    /// 1889–1900, which applies the factor only where `kstar(j2).ne.7`), and as COMPAS's `HURLEY`
    /// option does (`HeMS::CalculateMassTransferRejuvenationFactor` returns 1; its default,
    /// `STARTRACK`, takes `m_prev` ÷ m): ruling p11-c2-swell of 2026-10-06, item C; P11.T4.m. The
    /// paper (section 2.6.6) rejuvenates it as a star with a convective core, Mc ∝ M; its code
    /// excludes type 7. Hydrogen fed below the swell's gates lands outside the helium-burning core
    /// either way. Counted as mixed, a helium accretor fed near its end landed each step short of
    /// it and shrank its segments to the cap (finding F15: pair 6695 of layer D, capped at
    /// 183.1 Myr).
    #[must_use]
    pub(super) fn rejuvenated_tau(&self, i: usize, tau: f64, m0: f64, m1: f64, dt: f64) -> f64 {
        match &self.members[i] {
            Member::MainSequence { helium, .. } => {
                let lifetime = self.main_sequence_lifetime(*helium, m1);
                rejuvenated_tau_of_lifetime(*helium, tau, (m0, m1), dt, lifetime).min(1.0)
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

    /// Acts on what the accretor did in the last step, at `rate` from donor `d`, with what it was
    /// `fed` and its `accretion` (see the module documentation): whether it ended the phase. A
    /// swell ([`Engine::swell`]) reads the accretor's type, mass and core at the step's start,
    /// what it gained and the step's length (ruling p11-c2-swell of 2026-10-06, §3.3 A.1).
    fn accretor_events(&mut self, d: usize, rate: f64, fed: Fed, accretion: Accretion) -> bool {
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
        if accretion.swells && rate > 0.0 && self.swell(a_idx, ka, ma, &sa, fed) {
            return false;
        }
        if kd.surface() == Surface::Helium && ka.is_white_dwarf() {
            self.accreted_helium[a_idx] += fed.gained;
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
                    Some(self.reach_span_years(self.age)),
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
            // A dwarf at the Chandrasekhar mass collapses below, edge-lit or not (`dwarf_limit`,
            // P11.T4's fix), but from 29 for a pinned primary's, which is left to its pin.
            Kind::CarbonOxygenWhiteDwarf
                if kd.surface() == Surface::Helium
                    && self.accreted_helium[a_idx] >= EDGE_LIT_HELIUM
                    && (!dwarf_limit::collapses(ma)
                        || (self.ctx.rules().dwarf_limit.leaves_the_pin()
                            && self.pin_pending(a_idx))) =>
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
                let source = self.dwarf_cores[a_idx];
                self.finish_merger_with(super::common_envelope::Product::Member(accretor, source));
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

    /// Member `i` of type `kind`, mass `m` M☉ and structure `s` after a step of transfer, a white
    /// dwarf or helium star `fed` hydrogen, swells into a giant around its own core where the
    /// hydrogen accumulates (BSE sections 2.6.4, 2.6.6 and 2.7.4, `evolv2.f` 1480–1490 and
    /// 1513–1547, `gntage.f` 99–160; ruling p11-c2-swell of 2026-10-06, P11.T4.m): whether it did.
    ///
    /// A helium dwarf becomes a giant-branch star, a carbon–oxygen or oxygen–neon dwarf or a
    /// helium giant a thermally pulsing AGB star, and a helium main-sequence star a
    /// core-helium-burning one with its own fraction burnt, y = τ (BSE 1484). The core is the
    /// accretor's mass at the step's start (BSE's `mass(j2)` and a dwarf's `massc(j2)`), or a
    /// helium giant's carbon–oxygen core then (`massc(j2)`); the mass is `m`, never raised to the
    /// core, so a helium star's or a dwarf's envelope is the step's net gain (BSE's `mt2`, 1489 and
    /// 1546).
    ///
    /// The steps, in the ruling's order:
    ///
    /// 1. gate (a), BSE's for a helium star (1480), the ruling's for a white dwarf too: the step's
    ///    accretion exceeds [`SWELL_WIND_FACTOR`] times the accretor's own net wind over the step,
    ///    the accretion less its net gain, and `m` exceeds the core;
    /// 2. the exact placement ([`Engine::swelled_star`]), where the giant's core is the core asked
    ///    for;
    /// 3. gate (b), the burn gate, in place of BSE's per-orbit 10⁻⁴ of the mass for a helium star
    ///    (1481; BSE has none for a white dwarf): the placed giant's core, read at `m`, grows over
    ///    the step's length by less than the step's net gain, so the hydrogen arrives faster than
    ///    its shell turns it into core (Nomoto et al. 2007, section 3; on the thermally pulsing
    ///    AGB the core's growth is the shell's less third dredge-up, HPT equation 73), and stays
    ///    inside the mass to the step's end, so that the next step does not strip the giant again
    ///    (`hrdiag.f` 357–367, P11.T4.g's strip);
    /// 4. the segment is closed and the member set, a carbon–oxygen or oxygen–neon dwarf's giant
    ///    keeping the dwarf's kind from version 29 ([`Engine::dwarf_cores`], P11.T4's
    ///    `swollen_dwarf`).
    ///
    /// A refusal at any step leaves the member as it is, with the step's mass (for a helium star,
    /// BSE below its gate and COMPAS always), and closes no segment. A step over which the
    /// accretor's own type changes, one that lands on its phase's end or its death, which the
    /// phase then acts on, swells nothing: BSE reads the type at the step's start (`kstar(j2)`,
    /// 1482), and the swell would place the giant around a core of another kind, or give a dying
    /// star a new track.
    pub(super) fn swell(&mut self, i: usize, kind: Kind, m: f64, s: &Structure, fed: Fed) -> bool {
        if kind != fed.kind {
            return false;
        }
        let (target, core) = match kind {
            Kind::HeliumWhiteDwarf => (NewStar::GiantBranch, fed.before),
            Kind::HeliumMainSequence => (NewStar::CoreHeliumBurning { burnt: s.burnt }, fed.before),
            Kind::CarbonOxygenWhiteDwarf | Kind::OxygenNeonWhiteDwarf => {
                (NewStar::PulsingAgb, fed.before)
            }
            Kind::HeliumGap | Kind::HeliumGiant => (NewStar::PulsingAgb, fed.core_before),
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
        let net_gain = m - fed.before;
        let wind = fed.gained - net_gain;
        if !(fed.gained - SWELL_WIND_FACTOR * wind > 0.0 && m > core) {
            return false;
        }
        // From 29 a helium dwarf's core stays degenerate where the held placement would put it on
        // a heavier star's branch (`swollen_dwarf`, P11.T4's fix). The refusal is decided before
        // the track is built, which every refused step would build.
        let degenerate = kind == Kind::HeliumWhiteDwarf
            && self.ctx.rules().swollen_dwarf == SwollenDwarfRules::Degenerate
            && sse::degenerate_giant_mass(SolarMasses::new(core), self.ctx.coeffs()).is_some();
        if degenerate && !self.feeds_a_swollen_dwarf(i, m) {
            return false;
        }
        let degenerate = degenerate
            .then(|| self.degenerate_giant(i, m, core))
            .flatten();
        // Where no star of the kind has such a core, or no ask places one exactly, the accretor
        // keeps the mass as it is (ruling p11-c2-swell, item 7; BSE's gntage instead places a
        // giant of another type, `gntage.f` 34–49, 132–137 and 281–294).
        let Some(member) = degenerate.or_else(|| self.swelled_star(i, target, m, core)) else {
            return false;
        };
        let core_at = |age: f64| {
            member
                .evaluate(&self.ctx, i, age, m, 0.0)
                .map(|s| s.state.core_mass().value())
        };
        let (Some(now), Some(later)) = (core_at(self.age), core_at(self.age + fed.dt)) else {
            return false;
        };
        // Read at `m`, a core that would outgrow the mass is held at it, so the growth alone would
        // pass a core that reaches the mass within the step by the placement's rounding: the
        // giant must also keep an envelope to the step's end, which is the gate's purpose.
        if !(later - now < net_gain && later < m) {
            return false;
        }
        let source = self.dwarf_core(i, kind);
        self.close_segment();
        self.set_member(i, member);
        // From 29 the giant keeps the kind of the carbon–oxygen or oxygen–neon dwarf its core is
        // (`swollen_dwarf`, P11.T4's fix).
        if self.ctx.rules().swollen_dwarf == SwollenDwarfRules::Degenerate {
            self.dwarf_cores[i] = source;
        }
        true
    }

    /// Whether member `i`, a helium white dwarf of `m` M☉ that would swell onto a degenerate branch
    /// (P11.T4's `swollen_dwarf` from version 29), would go on being fed as that giant: its donor's
    /// transfer is not the heavy donor's thermal rate, which holds onto a compact accretor alone
    /// ([`heavy_donor::thermal_rate`]).
    ///
    /// Otherwise the swollen giant, fed at the overflow law's rate onto a giant, far below the
    /// swell's 2.71 × 10⁻⁷ M☉ yr⁻¹ (BSE section 2.6.6; about 10⁻¹⁴ M☉ yr⁻¹ from a 0.9 M☉ donor
    /// onto a 0.38 M☉ dwarf at 100 d, Z = 0.004, as the engine measured it), burns its envelope of
    /// one step into core in about 10⁴ years and is stripped back to the dwarf, which swells again
    /// at the next step: a cycle to [`MAX_SEGMENTS`](super::MAX_SEGMENTS) (measured, 2026-11-02).
    /// The dwarf goes on accreting as it is instead, as below the swell's other gates,
    /// until its core passes the degenerate branch's flash (lean, 2026-11-02).
    #[must_use]
    fn feeds_a_swollen_dwarf(&self, i: usize, m: f64) -> bool {
        let d = 1 - i;
        let (md, td) = self.current(d);
        let Some(sd) = self.structure(d, self.age, md, td) else {
            return false;
        };
        let kd = Kind::of(sd.state.phase(), md);
        !heavy_donor::thermal_rate(kd, Kind::HeliumWhiteDwarf, md, m)
    }

    /// Member `i`, a white dwarf a wind has fed to the Chandrasekhar mass in a detached phase,
    /// collapses as a dwarf fed by stable transfer does there ([`Engine::accretor_events`]; P11.T4's
    /// `dwarf_limit`): a Type Ia candidate pooled unless it is oxygen–neon,
    /// with the two stars' masses, and then its accretion-induced collapse.
    pub(super) fn collapse_fed_dwarf(&mut self, i: usize) {
        let (m, _) = self.current(i);
        let (other, _) = self.current(1 - i);
        if let Member::Remnant { phase, .. } = &self.members[i]
            && *phase != Phase::OxygenNeonWhiteDwarf
        {
            self.pool(PooledIaEvent::new(
                Years::new(self.age),
                IaPoolChannel::Accretion,
                [SolarMasses::new(m), SolarMasses::new(other)],
            ));
        }
        self.close_segment();
        self.accretion_induced_collapse(i, m);
    }

    /// Member `i`, a white dwarf of mass `m` at the Chandrasekhar mass, collapses to a neutron star
    /// (BSE section 2.6.5): the 1.26 M☉ neutron star of electron capture (plan 06's Mandel and
    /// Müller remnant), with the low-mode kick of accretion-induced collapse (P06.T19).
    pub(super) fn accretion_induced_collapse(&mut self, i: usize, m: f64) {
        let remnant = electron_capture_remnant();
        let law = StandardKickLaw::under(self.ctx.rules());
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

/// τ of a carried main sequence (`helium` for a helium one) that grows from `m0` to `m1` M☉ over
/// `dt` years, not yet held to 1: `Engine::rejuvenated_tau`'s rule (BSE section 2.6.6, after Tout
/// et al. 1997), which a wide pair's wind accretion reads too (`wind_accretion`). τ is kept for a
/// star with a radiative core (0.35–1.25 M☉) or a helium main sequence (P11.T4.m) and lowered by
/// m₀ ÷ m₁ for one whose convective core mixes in the new fuel, then advances by dt ÷ `t_MS`(m₁).
#[must_use]
pub(super) fn rejuvenated_tau_unheld(
    coeffs: &sse::ZCoeffs,
    helium: bool,
    tau: f64,
    m0: f64,
    m1: f64,
    dt: f64,
) -> f64 {
    rejuvenated_tau_of_lifetime(
        helium,
        tau,
        (m0, m1),
        dt,
        sse::main_sequence_lifetime(coeffs, helium, m1),
    )
}

/// [`rejuvenated_tau_unheld`] with the main-sequence lifetime of `m1`, `lifetime` (years), given.
#[must_use]
fn rejuvenated_tau_of_lifetime(
    helium: bool,
    tau: f64,
    (m0, m1): (f64, f64),
    dt: f64,
    lifetime: f64,
) -> f64 {
    let mixed = !helium && !(0.35..=1.25).contains(&m1);
    let tau = if mixed && m1 > m0 { tau * m0 / m1 } else { tau };
    tau + dt / lifetime
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

/// The Eddington rate, M☉ yr⁻¹, of a degenerate accretor of radius `radius` (R☉) fed matter of
/// `surface`: 2.08 × 10⁻³ (1 + X)⁻¹ R (BSE equation 67), where X is the accreted matter's hydrogen
/// fraction (ruling 132.1): 0.76 − 3Z for hydrogen-rich matter, none for a helium or carbon
/// surface, which is Case BB's (about 2.9 × 10⁻⁸ M☉ yr⁻¹ onto a neutron star).
#[must_use]
pub(super) fn eddington_rate(ctx: &super::timeline::Context, surface: Surface, radius: f64) -> f64 {
    2.08e-3 / (1.0 + hydrogen_fraction(ctx, surface)) * radius
}

/// The hydrogen fraction X of matter of `surface` (ruling 132.1): 0.76 − 3Z for hydrogen-rich
/// matter (BSE section 2.6.6), none for a helium or carbon surface.
#[must_use]
fn hydrogen_fraction(ctx: &super::timeline::Context, surface: Surface) -> f64 {
    match surface {
        Surface::Hydrogen => 0.76 - 3.0 * ctx.composition().z_fit().value(),
        Surface::Helium | Surface::Carbon => 0.0,
    }
}

/// The critical rate, M☉ yr⁻¹, of a white dwarf of `mass` M☉ fed matter of hydrogen fraction
/// `hydrogen`: the most its shell burns steadily into helium, above which it blows the excess off in
/// an optically thick wind and keeps its size, 5.3 × 10⁻⁷ (1.7 − X) ÷ X (M − 0.4) (Hachisu, Kato
/// and Nomoto 1996, ApJ 470, L97, the wind and the 0.4 M☉; in this form Hachisu, Kato and Nomoto
/// 1999, ApJ 522, 487, equation 5; recalled, not read at source). 4.2 × 10⁻⁷ for 0.95 M☉ at
/// X = 0.70; BSE's 2.71 × 10⁻⁷ is it at 0.76 M☉. It is negative below 0.4 M☉ and unbounded as X
/// falls to nothing; [`Engine::accretion`] reads it for hydrogen-rich matter only (X of 0.67–0.76),
/// and holds it to BSE's [`GIANT_RATE`] at least, which is not Hachisu, Kato and Nomoto's (below
/// 0.76 M☉ the dwarf keeps more than their rate: 1.8 times at 0.6 M☉), so that the share kept is
/// continuous where the swell begins.
#[must_use]
fn critical_rate(mass: f64, hydrogen: f64) -> f64 {
    CRITICAL_RATE_SCALE * (1.7 - hydrogen) / hydrogen * (mass - CRITICAL_RATE_ZERO_MASS)
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
