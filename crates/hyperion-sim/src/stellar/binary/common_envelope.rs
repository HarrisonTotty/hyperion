//! Common envelopes, coalescence and collisions (P11.T4.d), after Hurley, Tout and Pols (2002,
//! "BSE") section 2.7.
//!
//! - **Common envelope** (section 2.7.1): the α–λ energy balance. The envelope's binding energy
//!   G M₁ `M_env,1` ÷ (λ R₁), with a giant companion's envelope added (equation 69), is paid from
//!   the orbital energy of the cores, G `M_c1` M′_c2 ÷ 2a (equations 70–72), at efficiency `α_CE`; a
//!   main-sequence or degenerate companion counts whole as its core. If neither core then fills its
//!   Roche lobe, the envelope is gone and the cores remain on a circular orbit, co-rotating; the
//!   giants' cores are what their tracks leave when the envelope goes (`Track::remains_at`). A
//!   helium giant's carbon–oxygen core at or above the Chandrasekhar mass is no white dwarf: it
//!   collapses at once, companion-stripped (ruling 129.4c), the ultra-stripped supernova of Tauris,
//!   Langer and Podsiadlowski (2015); so does one a Roche lobe strips to its core. A core whose
//!   helium star has no life left dies at once too (P11.T4.k): a massive early-AGB star stripped
//!   in the last few thousand years before its own death collapses; a 1–3 M☉ star stripped late
//!   on its early AGB, whose carbon–oxygen core is past its helium star's `Mc,max` (HPT equation
//!   89), leaves its carbon–oxygen white dwarf; a non-degenerate core too light to burn helium
//!   is a helium white dwarf at once.
//!   Otherwise the cores coalesce where the first filled its lobe, and the product keeps the
//!   envelope left unbound: the mass `M_f` of equation 77 (with R ∝ M^−x, equations 74–76), by
//!   Newton's rule as BSE solves it.
//! - **Coalescence and collisions** (sections 2.7.2 and 2.7.3): the product's type is the collision
//!   matrix's (table 2) for the two types, with the paper's special cases: two degenerate helium
//!   cores ignite and nothing is left; a neutron star or black hole left inside a star makes an
//!   unstable Thorne–Żytkow object that keeps only the compact core. A collision without a giant
//!   conserves mass; two main-sequence stars mix to the age of equation 80, two helium
//!   main-sequence stars likewise; a helium white dwarf and a helium star make a rejuvenated
//!   helium star (equation 81).
//! - **The new star** (section 2.7.4): a giant, core-helium-burning, AGB or helium-giant product is
//!   placed on a track by its core (`new_star_mass`), and a core-helium-burning one at the share
//!   of helium burnt of equations 82 and 83.
//! - **White-dwarf mergers** are plan 11's pooled Type Ia candidates, and the engine makes of each
//!   what BSE makes of it where BSE makes a star, and the non-explosive outcome BSE names where BSE
//!   explodes it: two helium dwarfs a helium star (Webbink 1984, which BSE section 2.6.5 names as
//!   the alternative), two carbon–oxygen dwarfs above the Chandrasekhar mass a neutron star by
//!   accretion-induced collapse (Saio and Nomoto 1998, as section 2.6.5 has it).

use std::sync::Arc;

use crate::stellar::remnant::RemnantRecipe;
use crate::stellar::remnant::collapse::MAX_NEUTRON_STAR_MASS;
use crate::stellar::remnant::structure::{CHANDRASEKHAR_MASS, white_dwarf_radius};
use crate::stellar::remnant::white_dwarf::WhiteDwarfCore;
use crate::stellar::sse::{self, NewStar, Remains, Structure, Track};
use crate::stellar::{Phase, StarState, StarStateParts};
use crate::units::{Megayears, SolarMasses, SolarMassesPerYear, SolarRadii, Years};

use super::dwarf_limit::{DwarfLimitRules, Unplaced};
use super::evolve::{Engine, roche_lobe, track_mass};
use super::params::BinaryParams;
use super::rlof::kelvin_helmholtz;
use super::star::{G, Kind, Member, Path, cooling_origin, moment_of_inertia, positive};
use super::timeline::{IaPoolChannel, PooledIaEvent, SegmentKind};

/// BSE's collision matrix (their table 2, as their code's `instar` sets it): the type of the
/// product of two stars of types k₁ (column) and k₂ (row), 0–14. It is symmetric.
const COLLISION_MATRIX: [[u8; 15]; 15] = [
    [1, 1, 2, 3, 4, 5, 6, 4, 6, 6, 3, 6, 6, 13, 14],
    [1, 1, 2, 3, 4, 5, 6, 4, 6, 6, 3, 6, 6, 13, 14],
    [2, 2, 3, 3, 4, 4, 5, 4, 4, 4, 3, 5, 5, 13, 14],
    [3, 3, 3, 3, 4, 4, 5, 4, 4, 4, 3, 5, 5, 13, 14],
    [4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 13, 14],
    [5, 5, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 13, 14],
    [6, 6, 5, 5, 4, 4, 6, 4, 6, 6, 5, 6, 6, 13, 14],
    [4, 4, 4, 4, 4, 4, 4, 7, 8, 9, 7, 9, 9, 13, 14],
    [6, 6, 4, 4, 4, 4, 6, 8, 8, 9, 7, 9, 9, 13, 14],
    [6, 6, 4, 4, 4, 4, 6, 9, 9, 9, 7, 9, 9, 13, 14],
    [3, 3, 3, 3, 4, 4, 5, 7, 7, 7, 15, 9, 9, 13, 14],
    [6, 6, 5, 5, 4, 4, 6, 9, 9, 9, 9, 11, 12, 13, 14],
    [6, 6, 5, 5, 4, 4, 6, 9, 9, 9, 9, 12, 12, 13, 14],
    [13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 14],
    [14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14],
];

/// Newton iterations for the mass left after a coalescence in a common envelope (BSE equation
/// 77); BSE iterates to 10⁻³ of the mass, which these always reach.
const NEWTON_ITERATIONS: u32 = 40;

/// The type of the product of stars of types `k1` and `k2` (BSE table 2).
#[must_use]
pub(super) fn collision_product(k1: Kind, k2: Kind) -> Kind {
    let (i, j) = (k1.number(), k2.number());
    if i > 14 || j > 14 {
        return if i > 14 { k2 } else { k1 };
    }
    Kind::from_number(usize::from(COLLISION_MATRIX[j][i]))
}

/// One star's side of a common envelope (BSE section 2.7.1's effective values).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Side {
    kind: Kind,
    mass: f64,
    /// The effective core, M′_c: the whole star for a main-sequence or degenerate star.
    core: f64,
    /// Its true core for the product, `M_c`: none for a main-sequence star.
    true_core: f64,
    core_radius: f64,
    radius: f64,
    /// The envelope counted in the binding energy.
    envelope: f64,
    burnt: f64,
    degenerate: bool,
    fraction: f64,
}

impl Side {
    /// The side of a star of structure `s`.
    #[must_use]
    pub(super) fn of(s: &Structure) -> Self {
        let mass = s.state.mass().value();
        let kind = Kind::of(s.state.phase(), mass);
        let mc = s.state.core_mass().value();
        let giant = kind.is_giant_like();
        let (core, true_core, core_radius) = if giant {
            (mc, mc, s.core_radius.value())
        } else if kind.is_remnant() {
            (mass, mass, s.state.radius().value())
        } else {
            (
                mass,
                if kind == Kind::HeliumMainSequence {
                    mass
                } else {
                    0.0
                },
                s.state.radius().value(),
            )
        };
        let burnt = match kind {
            Kind::PulsingAgb
            | Kind::HeliumGap
            | Kind::HeliumGiant
            | Kind::CarbonOxygenWhiteDwarf
            | Kind::OxygenNeonWhiteDwarf => 1.0,
            Kind::CoreHeliumBurning | Kind::EarlyAgb | Kind::HeliumMainSequence => s.burnt,
            Kind::LowMassMainSequence
            | Kind::MainSequence
            | Kind::HertzsprungGap
            | Kind::GiantBranch
            | Kind::HeliumWhiteDwarf
            | Kind::NeutronStar
            | Kind::BlackHole
            | Kind::Massless => 0.0,
        };
        Self {
            kind,
            mass,
            core,
            true_core,
            core_radius,
            radius: s.state.radius().value(),
            envelope: if giant { (mass - mc).max(0.0) } else { 0.0 },
            burnt,
            degenerate: s.degenerate_core,
            fraction: s.state.phase_fraction(),
        }
    }
}

/// The mass ratio, lighter over heavier, below which a contact pair is tidally unstable and
/// coalesces (Rasio 1995, ApJ 444, L41: q ≈ 0.09; ruling 108.1).
pub(super) const CONTACT_MIN_Q: f64 = 0.09;

/// The donor's rate, in units of its thermal rate M ÷ `τ_KH`, above which a contact is dynamic
/// (Nelson and Eggleton 2001, ApJ 552, 664, eq. 8: case AD; ruling 114.1).
const DYNAMIC_CONTACT_RATE: f64 = 10.0;

/// The share of the donor's main-sequence lifetime within which a contact reached at the thermal
/// rate is rapid, `t_contact − t_RLOF < 0.1 t_MS` (Nelson and Eggleton 2001, Table 1: case AR;
/// rulings 111.5 and 114.2).
const RAPID_CONTACT_SHARE: f64 = 0.1;

/// The accretor's overfill of its Roche lobe, as a fraction of the lobe's radius, up to which a
/// rapid contact is temporary (ruling 114.2, after de Mink, Pols and Hilditch 2007, A&A 467, 1181,
/// §3.2 and Fig. 2).
pub(super) const TEMPORARY_CONTACT_OVERFILL: f64 = 0.10;

/// The relative miss, built core against the core asked for, within which a swelled giant is
/// placed exactly ([`Engine::swelled_star`]; ruling p11-c2-swell of 2026-10-06, §3.3 A4).
const SWELL_CORE_TOLERANCE: f64 = 1e-9;

/// The top of the bracket of the core asked of a swelled giant's placement, in units of the core
/// wanted: a placed track's core falls short of the one asked for by up to 8.4% (finding F16;
/// ruling p11-c2-swell, §3.3 A4).
const SWELL_BRACKET: f64 = 1.1;

/// The most track builds a swelled giant's placement makes before it is refused (ruling
/// p11-c2-swell, §3.3 A4).
const SWELL_BUILDS: u32 = 12;

/// Whether a merger may still pass through a common envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Envelope {
    /// BSE's rule: a collision involving a giant-like star is a common envelope.
    Ejectable,
    /// The common envelope has run and found nothing to eject (a held bare core, ruling 129.4c):
    /// the stars merge by the collision matrix alone.
    Spent,
}

/// Nelson and Eggleton's (2001) case of a main-sequence contact (see [`Engine::contact`]).
#[derive(Debug, Clone, Copy, PartialEq)]
enum ContactRegime {
    /// The mass ratio is below Rasio's (1995) 0.09.
    TidallyUnstable,
    /// Case AD: more than ten times the donor's thermal rate.
    Dynamic,
    /// Case AR: the donor's thermal rate or faster, soon after the onset; a deep contact then
    /// coalesces on the lighter star's thermal timescale, years.
    Rapid { light_thermal: f64 },
    /// Nuclear-rate transfer, the loss of angular momentum, or a late contact: the W Ursae Majoris
    /// channel (ruling 108.1).
    Slow,
}

impl Engine {
    /// A common envelope around member `d`, a giant-like donor, and its companion (BSE section
    /// 2.7.1).
    pub(super) fn common_envelope(&mut self, d: usize) {
        // A common envelope is resolved once: whatever merger it ends in merges by the collision
        // matrix and never re-enters it, by any route (`mix_with` reads the flag).
        debug_assert!(
            !self.in_common_envelope,
            "a common envelope entered from inside another"
        );
        self.in_common_envelope = true;
        self.resolve_common_envelope(d);
        self.in_common_envelope = false;
    }

    /// The body of [`Engine::common_envelope`].
    fn resolve_common_envelope(&mut self, d: usize) {
        let o = 1 - d;
        self.begin(SegmentKind::CommonEnvelope);
        self.close_segment();
        // The envelope is over; what follows is set below.
        self.kind = SegmentKind::Detached;
        let (md, td) = self.current(d);
        let (mo, to) = self.current(o);
        let (Some(sd), Some(so)) = (
            self.structure(d, self.age, md, td),
            self.structure(o, self.age, mo, to),
        ) else {
            self.coalesce(None);
            return;
        };
        let a_i = self.orbit.as_ref().map_or(0.0, |orbit| orbit.a);
        let (s1, s2) = (Side::of(&sd), Side::of(&so));
        let params = *self.ctx.params();
        let binding = envelope_binding(&s1, &s2, params.lambda);
        // Nothing to eject from a held bare core (ruling 129.4c): no stripping changes it, so it
        // would touch its companion again at once, envelope after envelope. It merges instead,
        // and the merger must not hand it back to a common envelope (a giant-like bare core
        // beside a main-sequence star otherwise recursed without end).
        if !positive(binding) && matches!(self.members[d], Member::Frozen { .. }) {
            self.mix_with(Envelope::Spent);
            return;
        }
        match envelope_outcome(s1, s2, binding, a_i, &params) {
            EnvelopeOutcome::Survives { a_f } => self.survive_envelope(d, &s2, a_f),
            EnvelopeOutcome::Coalesces(remains) => self.coalesce(Some(remains)),
        }
    }

    /// The cores of a common envelope around member `d` survive on a circular orbit of `a_f`
    /// (R☉), co-rotating (BSE section 2.7.1); the companion, `s2`, loses its envelope too if it
    /// is a giant.
    fn survive_envelope(&mut self, d: usize, s2: &Side, a_f: f64) {
        let o = 1 - d;
        self.stripped[d] = true;
        let (donor, donor_collapses) = self.stripped_member(d);
        self.set_member(d, donor);
        let mut other_collapses = false;
        if s2.kind.is_giant_like() {
            self.stripped[o] = true;
            let (other, collapses) = self.stripped_member(o);
            self.set_member(o, other);
            other_collapses = collapses;
        }
        let age = self.age;
        if let Some(orbit) = &mut self.orbit {
            orbit.set(age, a_f, 0.0);
        }
        self.kind = SegmentKind::Detached;
        for i in 0..2 {
            self.corotate(i);
        }
        // A bare core with no life left dies on the orbit the envelope left (ruling 129.4c;
        // P11.T4.k).
        for (i, collapses) in [(d, donor_collapses), (o, other_collapses)] {
            if collapses && !self.members[i].is_gone() {
                self.die(i);
            }
        }
    }

    /// Member `i` as its track leaves it when a common envelope or its Roche lobe removes its
    /// envelope now: a naked helium star on its own track, a white dwarf the binary carries, or
    /// the member as it was if it has no envelope to lose; with whether the member dies now
    /// ([`Engine::die`], once the caller has set the orbit).
    ///
    /// A bare core with no life left ([`Remains::Ended`]) dies now, on the track its stripping
    /// starts, which dies at once: a core at or above the Chandrasekhar mass collapses (ruling
    /// 129.4c), and a helium star whose own clock has already ended collapses or becomes its white
    /// dwarf (P11.T4.k, ruling p11-supernova-pins of 2026-10-06), through `die`'s companion-stripped
    /// kick or its white dwarf's quiet loss. A pinned primary whose collapse is still to come, or
    /// due now (a strip that lands on the pin), is held at the bare core's last living state
    /// instead, until its pin, which collapses it once (plan 11, design note 16; `Engine::run`'s
    /// backstop).
    /// No member is placed alive on a track that is a remnant from its own start.
    #[must_use]
    pub(super) fn stripped_member(&self, i: usize) -> (Member, bool) {
        let Some((_, offset)) = self.members[i].track() else {
            return (self.members[i].clone(), false);
        };
        self.stripped_member_at(i, (self.age - offset).max(0.0))
            .unwrap_or_else(|| (self.members[i].clone(), false))
    }

    /// [`Engine::stripped_member`] with the member's track read at `track_age` rather than at
    /// the engine's age: for the pinned hold, which strips a bare core at its last living age
    /// (P11.T4.g), where the track itself has already died. `None` where the member has no
    /// envelope to lose and stays as it is.
    ///
    /// A pinned primary whose pin is pending is held at its bare core's last living state whether
    /// the core collapses or is a white dwarf's (P11.T4's fix,
    /// [`pinned_white_dwarfs`](super::pinned_white_dwarfs)).
    #[must_use]
    pub(super) fn stripped_member_at(&self, i: usize, track_age: f64) -> Option<(Member, bool)> {
        let age = self.age;
        let (track, offset) = self.members[i].track()?;
        let (mass, _) = self.current(i);
        let pinned = i == 0 && self.pin_pending();
        let member = match track.remains_at(
            track_age,
            mass,
            self.ctx.draws(i),
            Some(self.reach_span_years(age)),
        ) {
            // A track already at its remnant goes on as that remnant.
            Remains::Nothing if track.state_at(Years::new(track_age)).phase().is_remnant() => {
                Member::Track {
                    track: Arc::clone(track),
                    offset,
                }
            }
            Remains::Nothing => return None,
            Remains::HeliumStar(star) => {
                debug_assert!(
                    !star.is_remnant_from_its_start(),
                    "a helium star dead from its start is placed alive at {age} yr"
                );
                Member::Track {
                    track: Arc::new(*star),
                    offset: age,
                }
            }
            Remains::Ended {
                core, core_radius, ..
            } if pinned => return Some((held_core(&core, core_radius), false)),
            Remains::Ended { track, .. } => {
                return Some((
                    Member::Track {
                        track: Arc::new(*track),
                        offset: age,
                    },
                    true,
                ));
            }
            Remains::WhiteDwarf {
                core, core_radius, ..
            } if pinned => {
                return Some((held_core(&core, core_radius), false));
            }
            Remains::WhiteDwarf {
                phase,
                mass,
                last_luminosity,
                ..
            } => Member::Remnant {
                phase,
                birth: age,
                origin: cooling_origin(&self.ctx, phase, mass.value(), Some(last_luminosity)),
                mass: Path::starting(age, mass.value()),
            },
        };
        Some((member, false))
    }

    /// Member `i` stripped of its envelope now by its Roche lobe (the end of transfer from a
    /// giant that has given its envelope away), and the pair detached.
    pub(super) fn strip(&mut self, i: usize) {
        self.close_segment();
        self.stripped[i] = true;
        let (member, collapses) = self.stripped_member(i);
        self.set_member(i, member);
        self.kind = self.quiet_kind();
        if collapses {
            self.die(i);
        }
    }

    /// Sets member `i` to co-rotate with the orbit, if there is one.
    fn corotate(&mut self, i: usize) {
        let Some(orbit) = &self.orbit else {
            return;
        };
        let (m, tau) = self.current(i);
        let (mo, _) = self.current(1 - i);
        let Some(s) = self.structure(i, self.age, m, tau) else {
            return;
        };
        let omega = (G * (m + mo) / (orbit.a * orbit.a * orbit.a)).sqrt();
        self.spins[i] = (moment_of_inertia(&s) * omega).max(1e-10);
    }

    /// The pair coalesces, with what a common envelope left, `remains`, if there was one (BSE
    /// sections 2.7.1–2.7.4), or in a collision without one.
    fn coalesce(&mut self, remains: Option<CommonEnvelopeRemains>) {
        let Some(remains) = remains else {
            self.mix();
            return;
        };
        let product = self.product_member(coalesced_product(&remains, self.ctx.giant_exponent()));
        self.finish_merger(product);
    }

    /// The member a coalescence in a common envelope leaves, from what [`coalesced_product`]
    /// gives: nothing, a compact star, or a star of its kind, mass and core, placed by
    /// [`Engine::new_star`] at the engine's age.
    #[must_use]
    pub(super) fn product_member(&self, product: CoalescedProduct) -> Member {
        match product {
            CoalescedProduct::Nothing => Member::Gone,
            CoalescedProduct::Compact { kind, mass } => self.compact(kind, mass),
            CoalescedProduct::Star {
                kind,
                mass,
                core,
                burnt,
                fraction,
            } => self.new_star(kind, mass, core, burnt, fraction),
        }
    }
}

/// The envelope's binding energy (÷ G) in a common envelope around a giant-like donor of side `s1`
/// and its companion `s2`, both envelopes if the companion is giant-like too (BSE section 2.7.1,
/// equation 69), at the binding parameter `lambda`.
#[must_use]
pub(super) fn envelope_binding(s1: &Side, s2: &Side, lambda: f64) -> f64 {
    let mut binding = s1.mass * s1.envelope / (lambda * s1.radius);
    if s2.kind.is_giant_like() {
        binding += s2.mass * s2.envelope / (lambda * s2.radius);
    }
    binding
}

/// How a common envelope of `binding` around the donor of side `s1` and its companion `s2` on an
/// orbit of `a_i` R☉ ends (BSE section 2.7.1): the cores survive on the orbit of `a_f` that the
/// α energy balance leaves, or, where either core fills its lobe there, they coalesce where the
/// first of them fills it (equation 73).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum EnvelopeOutcome {
    /// The cores survive on a circular orbit of `a_f` R☉.
    Survives {
        /// The cores' separation, R☉.
        a_f: f64,
    },
    /// The cores coalesce, leaving these remains to the merger.
    Coalesces(CommonEnvelopeRemains),
}

/// [`EnvelopeOutcome`] of a common envelope of `binding` around sides `s1` and `s2` on an orbit
/// of `a_i` R☉ under `params`' α: the engine's own rule, which the stellar merger class's evolved
/// donors read too (P11.T8.c's channels), so that the two agree bit for bit.
#[must_use]
pub(super) fn envelope_outcome(
    s1: Side,
    s2: Side,
    binding: f64,
    a_i: f64,
    params: &BinaryParams,
) -> EnvelopeOutcome {
    let orbit_i = s1.core * s2.core / (2.0 * a_i);
    let orbit_f = orbit_i + binding / params.alpha_ce;
    let a_f = s1.core * s2.core / (2.0 * orbit_f);
    let lobe1 = roche_lobe(s1.core, s2.core, a_f);
    let lobe2 = roche_lobe(s2.core, s1.core, a_f);
    let fills = s1.core_radius > lobe1 || s2.core_radius > lobe2;
    if !fills {
        return EnvelopeOutcome::Survives { a_f };
    }
    // The cores coalesce where the first of them fills its lobe (BSE equation 73).
    let reach =
        |side: &Side, other: &Side| side.core_radius / (roche_lobe(side.core, other.core, 1.0));
    let a_l = reach(&s1, &s2).max(reach(&s2, &s1));
    let orbit_l = (s1.core * s2.core / (2.0 * a_l)).max(orbit_i);
    let binding_f = binding - params.alpha_ce * (orbit_l - orbit_i);
    EnvelopeOutcome::Coalesces(CommonEnvelopeRemains {
        binding,
        binding_f,
        s1,
        s2,
    })
}

/// What a coalescence in a common envelope leaves (BSE sections 2.7.1–2.7.4): nothing, a compact
/// core, or a star of the collision matrix's kind with its mass, core, burnt share and phase
/// fraction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum CoalescedProduct {
    /// Two degenerate helium cores ignite and nothing is left (section 2.7.2).
    Nothing,
    /// An unstable Thorne–Żytkow object keeps its compact core (section 2.7.2).
    Compact {
        /// The core's kind.
        kind: Kind,
        /// Its mass, M☉.
        mass: f64,
    },
    /// A star of the collision matrix's kind (their table 2).
    Star {
        /// Its kind.
        kind: Kind,
        /// Its mass, M☉ (equation 77).
        mass: f64,
        /// Its core's mass, M☉.
        core: f64,
        /// The share of its core's helium burnt.
        burnt: f64,
        /// The donor's fraction through its phase.
        fraction: f64,
    },
}

/// What a coalescence in a common envelope with `remains` leaves, at the giant-radius exponent `x`
/// (BSE sections 2.7.1–2.7.4, table 2 and equation 77): the engine's own rule, which the stellar
/// merger class's evolved donors read too (P11.T8.c's channels), so that the two agree bit for
/// bit.
#[must_use]
pub(super) fn coalesced_product(remains: &CommonEnvelopeRemains, x: f64) -> CoalescedProduct {
    let CommonEnvelopeRemains {
        binding,
        binding_f,
        s1,
        s2,
    } = *remains;
    let product_kind = collision_product(s1.kind, s2.kind);
    // Two degenerate helium cores ignite, and nothing is left (BSE section 2.7.2).
    let degenerate = |s: &Side| {
        (matches!(s.kind, Kind::HertzsprungGap | Kind::GiantBranch) && s.degenerate)
            || s.kind == Kind::HeliumWhiteDwarf
    };
    let total = s1.mass + s2.mass;
    if degenerate(&s1) && degenerate(&s2) {
        CoalescedProduct::Nothing
    } else if s2.kind.number() >= 13 || s1.kind.number() >= 13 {
        // An unstable Thorne–Żytkow object keeps only its compact core (BSE section 2.7.2).
        let compact = if s2.kind.number() >= 13 { s2 } else { s1 };
        CoalescedProduct::Compact {
            kind: compact.kind,
            mass: compact.mass,
        }
    } else {
        let core = if s2.kind == Kind::HeliumMainSequence && s1.kind.number() <= 6 {
            s1.true_core + s2.mass
        } else {
            s1.true_core + s2.true_core
        };
        let mut mf = if binding_f <= 0.0 {
            core
        } else {
            final_mass(total, core, binding_f / binding, x)
        };
        if s2.true_core <= 0.0 && s2.kind != Kind::HeliumMainSequence {
            mf = mf.max(s1.true_core + s2.mass);
        }
        let mf = mf.min(total).max(core);
        let burnt = {
            let weight = |s: &Side| {
                if s.kind == Kind::HeliumMainSequence {
                    s.mass
                } else {
                    s.true_core
                }
            };
            let (w1, w2) = (weight(&s1), weight(&s2));
            if w1 + w2 > 0.0 {
                (s1.burnt * w1 + s2.burnt * w2) / (w1 + w2)
            } else {
                0.0
            }
        };
        CoalescedProduct::Star {
            kind: product_kind,
            mass: mf,
            core,
            burnt,
            fraction: s1.fraction,
        }
    }
}

impl Engine {
    /// A collision or coalescence without a common envelope: the two stars merge with their masses
    /// (BSE section 2.7.3), the product in the primary's place.
    pub(super) fn mix(&mut self) {
        self.mix_with(Envelope::Ejectable);
    }

    /// [`Engine::mix`], with `envelope` saying whether a giant-like star's collision may still go
    /// to a common envelope (BSE's rule) or merges by the collision matrix alone.
    #[expect(
        clippy::too_many_lines,
        reason = "one arm per row of BSE's collision matrix (their table 2)"
    )]
    fn mix_with(&mut self, envelope: Envelope) {
        let (m0, t0) = self.current(0);
        let (m1, t1) = self.current(1);
        let (s0, s1) = (
            self.structure(0, self.age, m0, t0),
            self.structure(1, self.age, m1, t1),
        );
        let (Some(a), Some(b)) = (s0, s1) else {
            // One of them is already gone: the other stays as it is.
            let survivor = if self.members[0].is_gone() {
                self.members[1].clone()
            } else {
                self.members[0].clone()
            };
            self.finish_merger(survivor);
            return;
        };
        let (k0, k1) = (Kind::of(a.state.phase(), m0), Kind::of(b.state.phase(), m1));
        // Body 1 of BSE's `mix` is the more evolved.
        let (hi, lo, s_hi, s_lo) = if k0.number() >= k1.number() {
            (k0, k1, &a, &b)
        } else {
            (k1, k0, &b, &a)
        };
        let (m_hi, m_lo) = (s_hi.state.mass().value(), s_lo.state.mass().value());
        let total = m_hi + m_lo;
        let masses = [SolarMasses::new(m0), SolarMasses::new(m1)];
        let chandrasekhar = CHANDRASEKHAR_MASS.value();
        let product = match (hi, lo) {
            (
                Kind::LowMassMainSequence | Kind::MainSequence,
                Kind::LowMassMainSequence | Kind::MainSequence,
            ) => self.main_sequence_merger(false, total, [(m_hi, s_hi), (m_lo, s_lo)]),
            (Kind::HeliumMainSequence, Kind::HeliumMainSequence) => {
                self.main_sequence_merger(true, total, [(m_hi, s_hi), (m_lo, s_lo)])
            }
            (Kind::HeliumMainSequence, Kind::LowMassMainSequence | Kind::MainSequence) => {
                self.new_star(Kind::CoreHeliumBurning, total, m_hi, s_hi.burnt, 0.0)
            }
            (Kind::HeliumWhiteDwarf, Kind::LowMassMainSequence | Kind::MainSequence) => {
                self.new_star(Kind::GiantBranch, total, m_hi, 0.0, 0.0)
            }
            (
                Kind::CarbonOxygenWhiteDwarf | Kind::OxygenNeonWhiteDwarf,
                Kind::LowMassMainSequence | Kind::MainSequence,
            ) => self.new_star_around(Kind::PulsingAgb, total, m_hi, 1.0, 0.0, Some(hi)),
            (Kind::HeliumWhiteDwarf, Kind::HeliumMainSequence) => {
                let tau = s_lo.burnt * m_lo / total;
                self.helium_main_sequence(total, tau)
            }
            (
                Kind::CarbonOxygenWhiteDwarf | Kind::OxygenNeonWhiteDwarf,
                Kind::HeliumMainSequence,
            ) => self.new_star_around(Kind::HeliumGiant, total, m_hi, 1.0, 0.0, Some(hi)),
            (Kind::HeliumWhiteDwarf, Kind::HeliumWhiteDwarf) => {
                self.pool(PooledIaEvent::new(
                    Years::new(self.age),
                    IaPoolChannel::Merger,
                    masses,
                ));
                self.helium_main_sequence(total, 0.0)
            }
            (Kind::CarbonOxygenWhiteDwarf | Kind::OxygenNeonWhiteDwarf, Kind::HeliumWhiteDwarf) => {
                self.pool(PooledIaEvent::new(
                    Years::new(self.age),
                    IaPoolChannel::Merger,
                    masses,
                ));
                self.new_star_around(Kind::HeliumGiant, total, m_hi, 1.0, 0.0, Some(hi))
            }
            (
                Kind::CarbonOxygenWhiteDwarf | Kind::OxygenNeonWhiteDwarf,
                Kind::CarbonOxygenWhiteDwarf | Kind::OxygenNeonWhiteDwarf,
            ) => {
                self.pool(PooledIaEvent::new(
                    Years::new(self.age),
                    IaPoolChannel::Merger,
                    masses,
                ));
                let phase = if hi == Kind::OxygenNeonWhiteDwarf {
                    Phase::OxygenNeonWhiteDwarf
                } else {
                    Phase::CarbonOxygenWhiteDwarf
                };
                match self.ctx.rules().dwarf_limit {
                    // From 27 `finish_merger` collapses a dwarf at the Chandrasekhar mass, as here.
                    DwarfLimitRules::Chandrasekhar => self.white_dwarf(phase, total),
                    DwarfLimitRules::Unchecked if total < chandrasekhar => {
                        self.white_dwarf(phase, total)
                    }
                    DwarfLimitRules::Unchecked => {
                        self.finish_merger(self.white_dwarf(phase, total));
                        self.accretion_induced_collapse(0, total);
                        return;
                    }
                }
            }
            (
                Kind::NeutronStar | Kind::BlackHole,
                Kind::LowMassMainSequence | Kind::MainSequence | Kind::HeliumMainSequence,
            ) => self.compact(hi, m_hi),
            (Kind::NeutronStar | Kind::BlackHole, _) => {
                let kind = if hi == Kind::NeutronStar && total > MAX_NEUTRON_STAR_MASS.value() {
                    Kind::BlackHole
                } else {
                    hi
                };
                self.compact(kind, total)
            }
            _ => {
                // A giant-like star is involved: BSE treats the collision as a common envelope,
                // unless that envelope is already spent.
                if envelope == Envelope::Ejectable
                    && !self.in_common_envelope
                    && let Some(giant) = [k0, k1].iter().position(|k| k.is_giant_like())
                {
                    self.common_envelope(giant);
                    return;
                }
                let kind = collision_product(hi, lo);
                self.new_star(
                    kind,
                    total,
                    s_hi.state.core_mass().value(),
                    s_hi.burnt,
                    s_hi.state.phase_fraction(),
                )
            }
        };
        self.finish_merger(product);
    }

    /// Records the merger as [`Engine::finish_merger`] does, for a product built by the caller.
    pub(super) fn finish_merger_with(&mut self, product: Member) {
        self.finish_merger(product);
    }

    /// Records the merger: `product` in the primary's place, nothing in the secondary's, no orbit.
    ///
    /// From version 27 a white dwarf product at or above the Chandrasekhar mass collapses at once,
    /// as an accreting one does (`Engine::accretor_events`): a Type Ia candidate is pooled unless
    /// it is oxygen–neon, with the two stars' masses before the merger, and the dwarf collapses by
    /// accretion-induced collapse ([`DwarfLimitRules::collapses`]).
    fn finish_merger(&mut self, product: Member) {
        let limit = self.ctx.rules().dwarf_limit;
        let dwarf = if let Member::Remnant { phase, mass, .. } = &product
            && matches!(
                phase,
                Phase::HeliumWhiteDwarf
                    | Phase::CarbonOxygenWhiteDwarf
                    | Phase::OxygenNeonWhiteDwarf
            )
            && limit.collapses(mass.at(self.age))
        {
            Some((*phase, mass.at(self.age)))
        } else {
            None
        };
        let masses = dwarf.map(|_| [0, 1].map(|i| SolarMasses::new(self.current(i).0)));
        self.close_segment();
        self.set_member(0, product);
        self.set_member(1, Member::Gone);
        self.orbit = None;
        self.kind = SegmentKind::Merged;
        self.stripped[1] = false;
        self.corotate(0);
        if let (Some((phase, m)), Some(masses)) = (dwarf, masses) {
            if phase != Phase::OxygenNeonWhiteDwarf {
                self.pool(PooledIaEvent::new(
                    Years::new(self.age),
                    IaPoolChannel::Accretion,
                    masses,
                ));
            }
            self.accretion_induced_collapse(0, m);
        }
    }

    /// The stars collide at periastron, or come into contact: a common envelope around a
    /// giant-like star, and a merger otherwise (BSE sections 2.7 and 2.8).
    pub(super) fn collide(&mut self) {
        self.close_segment();
        let kinds: [Option<Kind>; 2] = core::array::from_fn(|i| {
            let (m, tau) = self.current(i);
            self.structure(i, self.age, m, tau)
                .map(|s| Kind::of(s.state.phase(), m))
        });
        self.carry(0);
        self.carry(1);
        let fill = |me: &Self, i: usize| {
            let (m, tau) = me.current(i);
            let (mo, _) = me.current(1 - i);
            me.structure(i, me.age, m, tau).map_or(0.0, |s| {
                s.state.radius().value() / roche_lobe(m, mo, me.orbit.as_ref().map_or(1.0, |o| o.a))
            })
        };
        let first = usize::from(fill(self, 0) < fill(self, 1));
        let giant = |i: usize| kinds[i].is_some_and(Kind::is_giant_like);
        if giant(first) {
            self.common_envelope(first);
        } else if giant(1 - first) {
            self.common_envelope(1 - first);
        } else {
            self.mix();
        }
    }

    /// The accretor fills its lobe too during transfer from member `d` at `rate`, M☉ yr⁻¹, over
    /// the last step: contact (BSE section 2.6.6), which BSE ends in coalescence at once.
    ///
    /// Two main-sequence stars stay in contact, as observed contact binaries do (rulings 108.1,
    /// 111.5 and 114 of 2026-09-22, from Rasio 1995, Nelson and Eggleton 2001 and de Mink, Pols
    /// and Hilditch 2007), by Nelson and Eggleton's (2001, ApJ 552, 664, Table 1 and eq. 8) cases:
    ///
    /// - dynamic (AD), at more than ten times the donor's thermal rate M ÷ `τ_KH` (BSE equation
    ///   61), the pair merges dynamically ([`Engine::merge_dynamically`]);
    /// - rapid (AR), at the donor's thermal rate or faster and within a tenth of the donor's
    ///   main-sequence lifetime of the onset of transfer, a contact deeper than
    ///   [`TEMPORARY_CONTACT_OVERFILL`] coalesces on the thermal timescale of the lighter star (a
    ///   shallower one is temporary and never reaches here: [`Engine::contact_relaxes`]);
    /// - slow, at the nuclear rate or under the loss of angular momentum, or later than that tenth,
    ///   it stays in contact until either star leaves the main sequence (and then coalesces) or
    ///   the pair's age is reached;
    /// - a pair whose mass ratio is below Rasio's (1995) 0.09 is tidally unstable and coalesces
    ///   at once.
    ///
    /// The stars keep their masses in contact ([`Engine::contact_phase`]), so the mass ratio does
    /// not fall there. Any other pair goes on as a collision.
    pub(super) fn contact(&mut self, d: usize, rate: f64) {
        let Some(regime) = self.contact_regime(d, rate) else {
            self.collide();
            return;
        };
        let lifetime = match regime {
            ContactRegime::TidallyUnstable => {
                self.mix();
                return;
            }
            ContactRegime::Dynamic => {
                self.merge_dynamically(d);
                return;
            }
            ContactRegime::Rapid { light_thermal } => light_thermal,
            ContactRegime::Slow => self.main_sequence_left(0).min(self.main_sequence_left(1)),
        };
        self.kind = SegmentKind::Contact;
        // Past the pair's age where the contact outlasts it: `contact_phase` then leaves it in
        // contact rather than coalescing at the horizon.
        self.contact_until = self.age + lifetime.max(0.0);
    }

    /// Acts on the accretor filling its lobe during transfer from member `d` at `rate`, M☉ yr⁻¹:
    /// whether it ended the transfer. The first such step of the transfer is the contact's time
    /// ([`Engine::first_contact`]); a temporary contact ([`Engine::contact_relaxes`]) leaves the
    /// pair in semi-detached transfer, and any other closes the segment for [`Engine::contact`].
    pub(super) fn accretor_contact(&mut self, d: usize, rate: f64) -> bool {
        self.first_contact.get_or_insert(self.age);
        if self.contact_relaxes(d, rate) {
            return false;
        }
        self.close_segment();
        self.contact(d, rate);
        true
    }

    /// Whether the accretor's filling of its lobe during transfer from member `d` at `rate`,
    /// M☉ yr⁻¹, is temporary (ruling 114.2): a rapid (AR) contact of two main-sequence stars in
    /// which the accretor overfills its lobe by no more than [`TEMPORARY_CONTACT_OVERFILL`]. The
    /// accretor regains its thermal equilibrium and shrinks back inside its lobe (de Mink, Pols
    /// and Hilditch 2007, A&A 467, 1181, §3.2), so the pair goes on in semi-detached transfer and
    /// is not marked in contact. A departure from BSE, which merges every contact.
    #[must_use]
    pub(super) fn contact_relaxes(&self, d: usize, rate: f64) -> bool {
        if !matches!(
            self.contact_regime(d, rate),
            Some(ContactRegime::Rapid { .. })
        ) {
            return false;
        }
        let a_idx = 1 - d;
        let (ma, ta) = self.current(a_idx);
        let (md, _) = self.current(d);
        let a = self.orbit.as_ref().map_or(0.0, |o| o.a);
        self.structure(a_idx, self.age, ma, ta).is_some_and(|s| {
            s.state.radius().value() <= (1.0 + TEMPORARY_CONTACT_OVERFILL) * roche_lobe(ma, md, a)
        })
    }

    /// Nelson and Eggleton's (2001) case of a contact reached during transfer from member `d` at
    /// `rate`, M☉ yr⁻¹ (see [`Engine::contact`]), or `None` unless both stars are on the main
    /// sequence. The contact's time is the first at which the accretor filled its lobe in this
    /// transfer, [`Engine::first_contact`], or now.
    #[must_use]
    fn contact_regime(&self, d: usize, rate: f64) -> Option<ContactRegime> {
        let kinds: [Option<(Kind, Structure)>; 2] = core::array::from_fn(|i| {
            let (m, tau) = self.current(i);
            self.structure(i, self.age, m, tau)
                .map(|s| (Kind::of(s.state.phase(), m), s))
        });
        let [Some((k0, s0)), Some((k1, s1))] = kinds else {
            return None;
        };
        if !(k0.is_main_sequence() && k1.is_main_sequence()) {
            return None;
        }
        let m = [self.current(0).0, self.current(1).0];
        let light = usize::from(m[1] < m[0]);
        if m[light] < CONTACT_MIN_Q * m[1 - light] {
            return Some(ContactRegime::TidallyUnstable);
        }
        let (sd, kd) = if d == 0 { (&s0, k0) } else { (&s1, k1) };
        let (s_light, k_light) = if light == 0 { (&s0, k0) } else { (&s1, k1) };
        let thermal_rate = m[d] / kelvin_helmholtz(sd, kd);
        if !positive(rate) || rate < thermal_rate {
            return Some(ContactRegime::Slow);
        }
        if rate > DYNAMIC_CONTACT_RATE * thermal_rate {
            return Some(ContactRegime::Dynamic);
        }
        let (onset, main_sequence) = self.overflow_onset;
        let at = self.first_contact.unwrap_or(self.age);
        if at - onset < RAPID_CONTACT_SHARE * main_sequence {
            Some(ContactRegime::Rapid {
                light_thermal: kelvin_helmholtz(s_light, k_light),
            })
        } else {
            Some(ContactRegime::Slow)
        }
    }

    /// How long member `i`, a main-sequence star, has left on its main sequence at its mass now,
    /// years: the whole of it for a star on its own track that has not yet arrived, which is its
    /// zero-age main-sequence star to the engine
    /// ([`engine_track_age_years`](super::star::engine_track_age_years), P11.T4.i), not the time to
    /// its arrival.
    #[must_use]
    pub(super) fn main_sequence_left(&self, i: usize) -> f64 {
        let (m, tau) = self.current(i);
        let left = |track: &Track, offset: f64| {
            let (_, end, track_age) = super::evolve::phase_ahead(track, offset, self.age);
            (end - track_age).max(0.0)
        };
        match &self.members[i] {
            Member::MainSequence { helium, .. } => {
                (1.0 - tau).max(0.0) * sse::main_sequence_lifetime(self.ctx.coeffs(), *helium, m)
            }
            Member::Track { track, offset } => {
                match super::star::arrival_ahead_years(track, *offset, self.age) {
                    Some(arrival) => {
                        let (_, end) = track.phase_span(arrival);
                        (end - arrival).max(0.0)
                    }
                    None => left(track, *offset),
                }
            }
            Member::Shaped { track, offset, .. } => left(track, *offset),
            Member::Cooling { .. }
            | Member::Frozen { .. }
            | Member::Remnant { .. }
            | Member::Gone => f64::INFINITY,
        }
    }

    /// Runs a contact pair to its coalescence, which comes after the pair's age where the contact
    /// outlasts it (see [`Engine::contact`]), or to the primary's pin.
    ///
    /// The stars keep their masses and the orbit its separation, and a carried main sequence ages.
    /// The knots run to the coalescence whatever the pair's age, so that they do not depend on it
    /// (`detached::StepLimit`), and over 16 of the longest steps for a contact that never
    /// coalesces (two stars on the cooling fits).
    ///
    /// A pin inside the contact stops the knots there, a knot landing on it whatever the age asked
    /// (P11.T4.k, ruling p11-t4k-faults of 2026-10-06, finding C: before it the contact ran to its
    /// coalescence past the pin, and the merger's product died at its own later age). The pin is
    /// acted on ([`Engine::pinned_collapse`]) where it lies before the pair's age; otherwise the
    /// pair is left in contact, as a contact outlasting the age is. The pinned primary's own
    /// main-sequence end stops the knots the same way, before the pin (P11.T4.n): a primary the
    /// binary keeps on its main sequence leaves it there, and the pair coalesces
    /// ([`Engine::end_pinned_main_sequence`]). So no pinned primary in contact meets its pin there
    /// now; before T4.n it exploded from its main sequence at plan 06's age.
    pub(super) fn contact_phase(&mut self) {
        let end = if self.contact_until.is_finite() {
            self.contact_until
        } else {
            self.age + 16.0 * super::detached::LONGEST_STEP_YEARS
        };
        let steps = 16_u32;
        let start = self.age;
        let pin = self
            .pin
            .map(|pin| pin.death.age().value())
            .filter(|&at| at > start && at <= end);
        // The pinned primary's own main sequence's end stops the knots as the pin does, before it
        // (P11.T4.n): the contact coalesces there (`Engine::end_pinned_main_sequence`).
        let (m0, tau0) = self.current(0);
        let phase = self.structure(0, start, m0, tau0).map(|s| s.state.phase());
        let own_end = self
            .own_main_sequence_end_after(start, phase)
            .filter(|&at| at <= end);
        for k in 1..=steps {
            let age = start + (end - start) * f64::from(k) / f64::from(steps);
            let age = pin.map_or(age, |at| age.min(at));
            let age = own_end.map_or(age, |at| age.min(at));
            for i in 0..2 {
                let (m, tau) = self.current(i);
                let tau = match &self.members[i] {
                    Member::MainSequence { helium, .. } => {
                        let lifetime = sse::main_sequence_lifetime(self.ctx.coeffs(), *helium, m);
                        (tau + (age - self.age) / lifetime).min(1.0)
                    }
                    Member::Track { .. }
                    | Member::Shaped { .. }
                    | Member::Cooling { .. }
                    | Member::Frozen { .. }
                    | Member::Remnant { .. }
                    | Member::Gone => tau,
                };
                self.members[i].record(age, m, tau);
            }
            if let Some(orbit) = &mut self.orbit {
                let (a, e) = (orbit.a, orbit.e);
                orbit.set(age, a, e);
            }
            self.age = age;
            if own_end.is_some_and(|at| age >= at) {
                if age < self.until {
                    self.end_pinned_main_sequence();
                }
                return;
            }
            if pin.is_some_and(|at| age >= at) {
                if age < self.until {
                    self.contact_until = 0.0;
                    self.pinned_collapse();
                    if self.kind == SegmentKind::Contact {
                        self.begin(self.quiet_kind());
                    }
                }
                return;
            }
        }
        if self.contact_until > self.until {
            return;
        }
        self.close_segment();
        self.mix();
    }

    /// Dynamical transfer from member `d` that merges the pair (BSE sections 2.6.4 and 2.6.5):
    /// a low-mass main-sequence donor over its critical ratio gives its whole mass, of which a
    /// main-sequence accretor keeps both stars' masses as the collision of two main-sequence stars
    /// does (section 2.7.3) and a Hertzsprung-gap accretor the whole donor, as BSE's code gives any
    /// accretor of k 2–6 (P11.T4.d's fix); a white dwarf's, neutron star's or black hole's
    /// coalesces with its companion as a collision does.
    ///
    /// As first built a main-sequence or gap accretor kept only what its thermal timescale allowed
    /// of the donor, √(`τ_KH` `τ_dyn`) ÷ `τ_KH,2` of it (equation 64), a small fraction, and the
    /// rest left the pair: a 0.454 + 0.138 M☉ pair left 0.1375 M☉. BSE's own code, read as COSMIC
    /// carries it (research-leans-16; v3.4.0's `evolv2.f`, the original branch), restricts a
    /// main-sequence accretor so ("Restrict accretion to thermal timescale of secondary", `dm2 =
    /// taum/tkh(j2)*dm1`), but gives an accretor of k 2–6, the gap among them, the whole donor ("Add
    /// all the material to the giant's envelope", `dm2 = dm1`). Its collision rule, `mix.f`, merges
    /// two main-sequence stars with M₁ + M₂, and COSMIC changed the dynamical branch's main-sequence
    /// accretor to full accretion on 2023-07-20 ("This will lead to a merger of the two MS stars").
    /// P11.T4.d's rule as main reads it (the brief of 2026-10-31) is that collision rule: a
    /// main-sequence accretor and the donor make a main-sequence star of M₁ + M₂ at the fractional
    /// age 0.1 Σ Mᵢ τᵢ ÷ M (their equation 80) of both stars, and a gap accretor takes the whole
    /// donor, at its place in the gap on its new mass's track, each with no mass lost.
    pub(super) fn merge_dynamically(&mut self, d: usize) {
        let o = 1 - d;
        let (md, td) = self.current(d);
        let (mo, to) = self.current(o);
        let (Some(sd), Some(so)) = (
            self.structure(d, self.age, md, td),
            self.structure(o, self.age, mo, to),
        ) else {
            self.mix();
            return;
        };
        let kd = Kind::of(sd.state.phase(), md);
        let ko = Kind::of(so.state.phase(), mo);
        if kd == Kind::LowMassMainSequence && (ko.is_main_sequence() || ko == Kind::HertzsprungGap)
        {
            let product = if ko.is_main_sequence() {
                // P11.T4.d's rule, the collision of two main-sequence stars (BSE section 2.7.3):
                // both stars' masses, at both stars' fractional age.
                self.main_sequence_merger(false, md + mo, [(md, &sd), (mo, &so)])
            } else {
                // A Hertzsprung-gap accretor takes the whole donor, as BSE's own dynamical branch
                // gives an accretor of k 2–6 (`evolv2.f`: "Add all the material to the giant's
                // envelope", `dm2 = dm1`), and sits at the accretor's place in the gap on its own
                // mass's track, whose core it takes.
                self.new_star(
                    Kind::HertzsprungGap,
                    mo + md,
                    so.state.core_mass().value(),
                    0.0,
                    so.state.phase_fraction(),
                )
            };
            self.finish_merger(product);
            return;
        }
        self.mix();
    }

    /// The product of two main-sequence (or, with `helium`, helium main-sequence) stars of total
    /// mass `total`: a star of that mass at the fractional age 0.1 Σ Mᵢ τᵢ ÷ M (BSE equation 80) on
    /// its own track.
    #[must_use]
    fn main_sequence_merger(
        &self,
        helium: bool,
        total: f64,
        stars: [(f64, &Structure); 2],
    ) -> Member {
        let tau = 0.1
            * stars
                .iter()
                .map(|(m, s)| m * s.state.phase_fraction())
                .sum::<f64>()
            / total;
        if helium {
            self.helium_main_sequence(total, tau)
        } else {
            self.main_sequence_star(total, tau)
        }
    }

    /// A main-sequence star of mass `m` at fractional age `tau`, on its own track, in slot 0.
    #[must_use]
    fn main_sequence_star(&self, m: f64, tau: f64) -> Member {
        if m < sse::MIN_INITIAL_MASS.value() {
            return Member::Cooling {
                offset: 0.0,
                mass: Path::starting(self.age, m),
            };
        }
        let mass = track_mass(SolarMasses::new(m));
        let lifetime = sse::main_sequence_lifetime(self.ctx.coeffs(), false, mass.value());
        let guess = sse::main_sequence_start(mass, self.ctx.composition()) + tau * lifetime;
        let (composition, draws) = (self.ctx.composition(), self.ctx.draws(0));
        let options = self.ctx.track_options();
        let (track, placed) = super::evolve::track_reaching(
            guess,
            self.reach_span_years(self.age),
            |reach| Track::to_age_with(mass, composition, draws, options, Years::new(reach)),
            |track| track.age_in_phase(Phase::MainSequence, tau),
        );
        let at = placed.unwrap_or(tau * lifetime);
        Member::Track {
            track: Arc::new(track),
            offset: super::evolve::offset_for(self.age, at),
        }
    }

    /// A helium main-sequence star of mass `m` at fractional age `tau`, on its own track.
    #[must_use]
    fn helium_main_sequence(&self, m: f64, tau: f64) -> Member {
        let track = Track::helium_star_from(
            SolarMasses::new(m.min(sse::MAX_INITIAL_MASS.value())),
            tau,
            self.ctx.composition(),
            self.ctx.draws(0),
            Some(self.reach_span_years(self.age)),
        );
        Member::Track {
            track: Arc::new(track),
            offset: self.age,
        }
    }

    /// A white dwarf of `phase` and mass `m` the binary made now.
    #[must_use]
    fn white_dwarf(&self, phase: Phase, m: f64) -> Member {
        Member::Remnant {
            phase,
            birth: self.age,
            origin: Megayears::ZERO,
            mass: Path::starting(self.age, m),
        }
    }

    /// A neutron star or black hole of mass `m` that keeps the compact member's clock.
    #[must_use]
    fn compact(&self, kind: Kind, m: f64) -> Member {
        let phase = if kind == Kind::BlackHole {
            Phase::BlackHole
        } else {
            Phase::NeutronStar
        };
        let birth = self
            .members
            .iter()
            .find_map(|member| match member {
                Member::Remnant {
                    phase: p, birth, ..
                } if *p == Phase::NeutronStar || *p == Phase::BlackHole => Some(*birth),
                Member::Track { track, offset } => track
                    .remnant_clock()
                    .filter(|(p, _, _)| *p == Phase::NeutronStar || *p == Phase::BlackHole)
                    .map(|(_, birth, _)| birth + offset),
                _ => None,
            })
            .unwrap_or(self.age);
        Member::Remnant {
            phase,
            birth: birth.min(self.age),
            origin: Megayears::ZERO,
            mass: Path::starting(self.age, m),
        }
    }

    /// The merger's product of type `kind`, mass `m` and core `core`, with `burnt` of its helium
    /// burnt and `fraction` through its phase (a Hertzsprung gap's), placed as BSE section 2.7.4
    /// places it; a star that cannot be placed becomes the remnant of its core.
    #[must_use]
    fn new_star(&self, kind: Kind, m: f64, core: f64, burnt: f64, fraction: f64) -> Member {
        self.new_star_around(kind, m, core, burnt, fraction, None)
    }

    /// [`Engine::new_star`] for a product whose core is the white dwarf of kind `dwarf` it merged
    /// from, if it is one, which types the dwarf an unplaced product leaves from version 27
    /// ([`DwarfLimitRules::unplaced`]).
    #[must_use]
    fn new_star_around(
        &self,
        kind: Kind,
        m: f64,
        core: f64,
        burnt: f64,
        fraction: f64,
        dwarf: Option<Kind>,
    ) -> Member {
        let target = match kind {
            Kind::LowMassMainSequence | Kind::MainSequence => {
                return self.main_sequence_star(m, 0.0);
            }
            Kind::HeliumMainSequence => return self.helium_main_sequence(m, burnt),
            Kind::HertzsprungGap => {
                let mass = track_mass(SolarMasses::new(m));
                let track = Track::full_with(
                    mass,
                    self.ctx.composition(),
                    self.ctx.draws(0),
                    self.ctx.track_options(),
                );
                let at = track
                    .age_in_phase(Phase::HertzsprungGap, fraction)
                    .unwrap_or(0.0);
                return Member::Shaped {
                    track: Arc::new(track),
                    offset: super::evolve::offset_for(self.age, at),
                    mass: Path::starting(self.age, m),
                };
            }
            Kind::GiantBranch => NewStar::GiantBranch,
            Kind::CoreHeliumBurning => NewStar::CoreHeliumBurning { burnt },
            Kind::EarlyAgb => NewStar::EarlyAgb,
            Kind::PulsingAgb => NewStar::PulsingAgb,
            Kind::HeliumGap | Kind::HeliumGiant => NewStar::HeliumGiant,
            Kind::HeliumWhiteDwarf => return self.white_dwarf(Phase::HeliumWhiteDwarf, m),
            Kind::CarbonOxygenWhiteDwarf => {
                return self.white_dwarf(Phase::CarbonOxygenWhiteDwarf, m);
            }
            Kind::OxygenNeonWhiteDwarf => return self.white_dwarf(Phase::OxygenNeonWhiteDwarf, m),
            Kind::NeutronStar | Kind::BlackHole => return self.compact(kind, m),
            Kind::Massless => return Member::Gone,
        };
        self.placed_star(0, target, m, core).unwrap_or_else(|| {
            match self.ctx.rules().dwarf_limit.unplaced(kind, m, core, dwarf) {
                Unplaced::WhiteDwarf { phase, mass } => self
                    .held_product(target, phase, SolarMasses::new(mass))
                    .unwrap_or_else(|| self.white_dwarf(phase, mass)),
                Unplaced::HeliumStar { mass } => self.helium_main_sequence(mass, burnt),
            }
        })
    }

    /// A merger's product in the primary's place to be placed as `target` that no star of that
    /// kind can hold, whose white dwarf of `phase` and `mass` [`Engine::new_star`] makes of its
    /// core, held at its core's last living state instead while the primary's pin is pending
    /// (P11.T4's fix, [`pinned_white_dwarfs`](super::pinned_white_dwarfs)): design note 16's "if
    /// the pair has merged, the product collapses", at its pin. `None` otherwise.
    ///
    /// The held state is the bare core as `Track::remains_at` holds a stripped AGB star's: a star
    /// of `target`'s phase whose mass is all core, at [`sse::bare_core_light`]'s luminosity and
    /// radius, its white dwarf's at formation below the Chandrasekhar mass (HPT section 6.3's
    /// limit of a giant as its envelope goes, µ = 0). The product has no track whose structure it
    /// could take, so this state is a lean (2026-11-01, recorded in plan 11).
    #[must_use]
    fn held_product(&self, target: NewStar, phase: Phase, mass: SolarMasses) -> Option<Member> {
        if !self.pin_pending() {
            return None;
        }
        let kind = WhiteDwarfCore::of(phase)?;
        let recipe = RemnantRecipe::default();
        let (luminosity, radius) =
            sse::bare_core_light(recipe, self.ctx.composition().z_fit(), kind, mass);
        let state = StarState::new(StarStateParts {
            phase: match target {
                NewStar::GiantBranch => Phase::FirstGiantBranch,
                NewStar::CoreHeliumBurning { .. } => Phase::CoreHeliumBurning,
                NewStar::EarlyAgb => Phase::EarlyAgb,
                NewStar::PulsingAgb => Phase::ThermallyPulsingAgb,
                NewStar::HeliumGiant => Phase::HeliumGiantBranch,
            },
            age: Years::new(self.age),
            mass,
            core_mass: mass,
            luminosity,
            radius,
            mass_loss_rate: SolarMassesPerYear::ZERO,
            phase_fraction: 0.0,
        });
        // sse's degenerate core radius, 5 R_WD, held to the radius.
        let degenerate = 5.0 * white_dwarf_radius(recipe, mass).value();
        Some(Member::Frozen {
            state,
            core_radius: SolarRadii::new(degenerate.min(radius.value())),
        })
    }

    /// A star of mass `m` with core `core` placed as `target` (BSE section 2.7.4) on a track built
    /// in full with member `i`'s draws, whose mass the binary carries from now, or `None` where no
    /// star of that kind has such a core.
    #[must_use]
    pub(super) fn placed_star(
        &self,
        i: usize,
        target: NewStar,
        m: f64,
        core: f64,
    ) -> Option<Member> {
        self.placed_by(i, target, m.max(core), core)
    }

    /// The giant a fed helium star or white dwarf swells into ([`Engine::swell`]): a star of mass
    /// `m` M☉ placed as `target` in member `i`'s slot, whose built track's core at the placement,
    /// read at `m`, is `core` M☉ (below `m`) to [`SWELL_CORE_TOLERANCE`] of it; or `None` where no
    /// ask places one so within [`SWELL_BUILDS`] track builds (ruling p11-c2-swell of 2026-10-06,
    /// §3.3 A4; P11.T4.m).
    ///
    /// [`Engine::placed_star`] builds the new star's track from the zero-age main sequence with its
    /// winds, so the track enters the placed phase at a wind-reduced effective initial mass (HPT
    /// section 7.1) and its core there falls short of the one asked for (finding F16): for a
    /// core-helium-burning star of 8 M☉ at Z = 0.02 by about 7% halfway through core helium
    /// burning, and by 8.4% a tenth of the way through. As an envelope, that miss was a fictitious
    /// giant's (finding C2). BSE's `gntage` starts the star at the placement (`gntage.f` 159–160),
    /// so that its core is the one asked for to its bisection's 10⁻⁵ M☉ in M₀ (lines 24 and 147).
    /// Here the core asked of the placement, c′, is solved for instead: bracketed in
    /// [core, [`SWELL_BRACKET`] core] among the asks that place a star, from the fixed-point step
    /// c′ = core + (core − built) and then by the secant rule, or by bisection where that leaves
    /// the bracket. An ask that places no star, or overshoots, closes the bracket from above. The
    /// trials are a fixed sequence of the inputs. Every other caller keeps `placed_star`'s miss,
    /// whose own fix is F16's task.
    #[must_use]
    pub(super) fn swelled_star(
        &self,
        i: usize,
        target: NewStar,
        m: f64,
        core: f64,
    ) -> Option<Member> {
        let tolerance = SWELL_CORE_TOLERANCE * core;
        // The ask's star and its built core's miss, or `None` where the ask places none.
        let trial = |ask: f64| {
            let member = self.placed_by(i, target, m, ask)?;
            let built = member.evaluate(&self.ctx, i, self.age, m, 0.0)?;
            let miss = built.state.core_mass().value() - core;
            Some((member, miss))
        };
        let (member, miss) = trial(core)?;
        if miss.abs() <= tolerance {
            return Some(member);
        }
        if miss > 0.0 {
            return None;
        }
        let (mut lo, mut top) = (core, SWELL_BRACKET * core);
        let mut last = (core, miss);
        // The next ask, where the last step gives one inside the bracket; a bisection otherwise.
        let mut next = Some(core - miss);
        for _ in 1..SWELL_BUILDS {
            let ask = next
                .filter(|&ask| ask > lo && ask < top)
                .unwrap_or(lo + 0.5 * (top - lo));
            let Some((member, miss)) = trial(ask) else {
                top = ask;
                next = None;
                continue;
            };
            if miss.abs() <= tolerance {
                return Some(member);
            }
            if miss < 0.0 {
                lo = ask;
            } else {
                top = ask;
            }
            // The secant through the last two trials, where they differ.
            let slope = (miss - last.1) / (ask - last.0);
            next = (slope.is_finite() && slope.abs() > 0.0).then(|| ask - miss / slope);
            last = (ask, miss);
        }
        None
    }

    /// A star of mass `m` M☉ that the binary carries from now in member `i`'s slot, placed as
    /// `target` with the core `ask` M☉ asked of [`sse::new_star_mass`] ([`Engine::placed_star`]).
    #[must_use]
    fn placed_by(&self, i: usize, target: NewStar, m: f64, ask: f64) -> Option<Member> {
        let (m0, phase, fraction) =
            sse::new_star_mass(target, SolarMasses::new(ask), self.ctx.coeffs())?;
        let track = match target {
            NewStar::HeliumGiant => {
                Track::helium_star_full(m0, self.ctx.composition(), self.ctx.draws(i))
            }
            NewStar::GiantBranch
            | NewStar::CoreHeliumBurning { .. }
            | NewStar::EarlyAgb
            | NewStar::PulsingAgb => Track::full_with(
                track_mass(m0),
                self.ctx.composition(),
                self.ctx.draws(i),
                self.ctx.track_options(),
            ),
        };
        let at = track.age_in_phase(phase, fraction)?;
        Some(Member::Shaped {
            track: Arc::new(track),
            offset: super::evolve::offset_for(self.age, at),
            mass: Path::starting(self.age, m),
        })
    }
}

/// The pinned primary held at its bare core's last living state `core`, with its structure's
/// `core_radius`, until its pin collapses it (plan 11, design note 16; P11.T4.g).
#[must_use]
fn held_core(core: &StarState, core_radius: SolarRadii) -> Member {
    let state = StarState::new(StarStateParts {
        phase: core.phase(),
        age: core.age(),
        mass: core.mass(),
        core_mass: core.core_mass(),
        luminosity: core.luminosity(),
        radius: core.radius(),
        mass_loss_rate: SolarMassesPerYear::ZERO,
        phase_fraction: core.phase_fraction(),
    });
    Member::Frozen { state, core_radius }
}

/// What a common envelope whose cores coalesce leaves to the merger: the envelope's binding
/// energy before and after the spiral-in (÷ G) and the two sides.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct CommonEnvelopeRemains {
    binding: f64,
    binding_f: f64,
    s1: Side,
    s2: Side,
}

/// The mass `M_f` the product of a coalescence in a common envelope keeps, from the total `total`
/// and the merged core `core`, where the envelope's binding energy has fallen to `ratio` of what
/// it was (BSE equation 77): (`M_f` ÷ M)^(1 + x) (`M_f` − Mc) ÷ (M − Mc) = ratio, by Newton's rule.
#[must_use]
fn final_mass(total: f64, core: f64, ratio: f64, x: f64) -> f64 {
    let exponent = 1.0 + x;
    let target = crate::math::powf_positive(total, exponent) * (total - core) * ratio;
    let mut mf = core.max(total * crate::math::powf_positive(ratio.max(1e-12), 1.0 / exponent));
    for _ in 0..NEWTON_ITERATIONS {
        let f = crate::math::powf_positive(mf, exponent) * (mf - core) - target;
        let slope = crate::math::powf_positive(mf, x) * (exponent * mf - x * core);
        if !positive(slope) {
            break;
        }
        let next = mf - f / slope;
        mf = next.clamp(core, total);
    }
    mf
}
