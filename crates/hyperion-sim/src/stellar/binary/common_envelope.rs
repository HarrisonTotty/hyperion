//! Common envelopes, coalescence and collisions (P11.T4.d), after Hurley, Tout and Pols (2002,
//! "BSE") section 2.7.
//!
//! - **Common envelope** (section 2.7.1): the α–λ energy balance. The envelope's binding energy
//!   G M₁ `M_env,1` ÷ (λ R₁), with a giant companion's envelope added (equation 69), is paid from
//!   the orbital energy of the cores, G `M_c1` M′_c2 ÷ 2a (equations 70–72), at efficiency `α_CE`; a
//!   main-sequence or degenerate companion counts whole as its core. If neither core then fills its
//!   Roche lobe, the envelope is gone and the cores remain on a circular orbit, co-rotating; the
//!   giants' cores are what their tracks leave when the envelope goes (`Track::remains_at`).
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

use crate::stellar::Phase;
use crate::stellar::remnant::collapse::MAX_NEUTRON_STAR_MASS;
use crate::stellar::remnant::structure::CHANDRASEKHAR_MASS;
use crate::stellar::sse::{self, NewStar, Remains, Structure, Track};
use crate::units::{Megayears, SolarMasses, Years};

use super::evolve::{Engine, roche_lobe, track_mass};
use super::rlof::{dynamical, kelvin_helmholtz};
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
#[derive(Debug, Clone, Copy)]
struct Side {
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
    fn of(s: &Structure) -> Self {
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

impl Engine {
    /// A common envelope around member `d`, a giant-like donor, and its companion (BSE section
    /// 2.7.1).
    pub(super) fn common_envelope(&mut self, d: usize) {
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
        let lambda = params.lambda;
        let mut binding = s1.mass * s1.envelope / (lambda * s1.radius);
        if s2.kind.is_giant_like() {
            binding += s2.mass * s2.envelope / (lambda * s2.radius);
        }
        let orbit_i = s1.core * s2.core / (2.0 * a_i);
        let orbit_f = orbit_i + binding / params.alpha_ce;
        let a_f = s1.core * s2.core / (2.0 * orbit_f);
        let lobe1 = roche_lobe(s1.core, s2.core, a_f);
        let lobe2 = roche_lobe(s2.core, s1.core, a_f);
        let fills = s1.core_radius > lobe1 || s2.core_radius > lobe2;
        if !fills {
            self.survive_envelope(d, &s2, a_f);
            return;
        }
        // The cores coalesce where the first of them fills its lobe (BSE equation 73).
        let reach =
            |side: &Side, other: &Side| side.core_radius / (roche_lobe(side.core, other.core, 1.0));
        let a_l = reach(&s1, &s2).max(reach(&s2, &s1));
        let orbit_l = (s1.core * s2.core / (2.0 * a_l)).max(orbit_i);
        let binding_f = binding - params.alpha_ce * (orbit_l - orbit_i);
        self.coalesce(Some(CommonEnvelopeRemains {
            binding,
            binding_f,
            s1,
            s2,
        }));
    }

    /// The cores of a common envelope around member `d` survive on a circular orbit of `a_f`
    /// (R☉), co-rotating (BSE section 2.7.1); the companion, `s2`, loses its envelope too if it
    /// is a giant.
    fn survive_envelope(&mut self, d: usize, s2: &Side, a_f: f64) {
        let o = 1 - d;
        self.stripped[d] = true;
        let donor = self.stripped_member(d);
        self.set_member(d, donor);
        if s2.kind.is_giant_like() {
            self.stripped[o] = true;
            let other = self.stripped_member(o);
            self.set_member(o, other);
        }
        let age = self.age;
        if let Some(orbit) = &mut self.orbit {
            orbit.set(age, a_f, 0.0);
        }
        self.kind = SegmentKind::Detached;
        for i in 0..2 {
            self.corotate(i);
        }
    }

    /// Member `i` as its track leaves it when a common envelope or its Roche lobe removes its
    /// envelope now: a naked helium star on its own track, a white dwarf the binary carries, or
    /// the member as it was if it has no envelope to lose.
    #[must_use]
    pub(super) fn stripped_member(&self, i: usize) -> Member {
        let age = self.age;
        let Some((track, offset)) = self.members[i].track() else {
            return self.members[i].clone();
        };
        let track_age = (age - offset).max(0.0);
        let (mass, _) = self.current(i);
        match track.remains_at(
            track_age,
            mass,
            self.ctx.draws(i),
            Some((self.until - age).max(0.0)),
        ) {
            // A track already at its remnant goes on as that remnant.
            Remains::Nothing if track.state_at(Years::new(track_age)).phase().is_remnant() => {
                Member::Track {
                    track: Arc::clone(track),
                    offset,
                }
            }
            Remains::Nothing => self.members[i].clone(),
            Remains::HeliumStar(star) => Member::Track {
                track: Arc::new(*star),
                offset: age,
            },
            Remains::WhiteDwarf {
                phase,
                mass,
                last_luminosity,
            } => Member::Remnant {
                phase,
                birth: age,
                origin: cooling_origin(&self.ctx, phase, mass.value(), Some(last_luminosity)),
                mass: Path::starting(age, mass.value()),
            },
        }
    }

    /// Member `i` stripped of its envelope now by its Roche lobe (the end of transfer from a
    /// giant that has given its envelope away), and the pair detached.
    pub(super) fn strip(&mut self, i: usize) {
        self.close_segment();
        self.stripped[i] = true;
        let member = self.stripped_member(i);
        self.set_member(i, member);
        self.kind = self.quiet_kind();
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
        let Some(CommonEnvelopeRemains {
            binding,
            binding_f,
            s1,
            s2,
        }) = remains
        else {
            self.mix();
            return;
        };
        let product_kind = collision_product(s1.kind, s2.kind);
        // Two degenerate helium cores ignite, and nothing is left (BSE section 2.7.2).
        let degenerate = |s: &Side| {
            (matches!(s.kind, Kind::HertzsprungGap | Kind::GiantBranch) && s.degenerate)
                || s.kind == Kind::HeliumWhiteDwarf
        };
        let total = s1.mass + s2.mass;
        let product = if degenerate(&s1) && degenerate(&s2) {
            Member::Gone
        } else if s2.kind.number() >= 13 || s1.kind.number() >= 13 {
            // An unstable Thorne–Żytkow object keeps only its compact core (BSE section 2.7.2).
            let compact = if s2.kind.number() >= 13 { s2 } else { s1 };
            self.compact(compact.kind, compact.mass)
        } else {
            let core = if s2.kind == Kind::HeliumMainSequence && s1.kind.number() <= 6 {
                s1.true_core + s2.mass
            } else {
                s1.true_core + s2.true_core
            };
            let x = self.ctx.giant_exponent();
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
            let fraction = s1.fraction;
            self.new_star(product_kind, mf, core, burnt, fraction)
        };
        self.finish_merger(product);
    }

    /// A collision or coalescence without a common envelope: the two stars merge with their masses
    /// (BSE section 2.7.3), the product in the primary's place.
    #[expect(
        clippy::too_many_lines,
        reason = "one arm per row of BSE's collision matrix (their table 2)"
    )]
    pub(super) fn mix(&mut self) {
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
            ) => self.new_star(Kind::PulsingAgb, total, m_hi, 1.0, 0.0),
            (Kind::HeliumWhiteDwarf, Kind::HeliumMainSequence) => {
                let tau = s_lo.burnt * m_lo / total;
                self.helium_main_sequence(total, tau)
            }
            (
                Kind::CarbonOxygenWhiteDwarf | Kind::OxygenNeonWhiteDwarf,
                Kind::HeliumMainSequence,
            ) => self.new_star(Kind::HeliumGiant, total, m_hi, 1.0, 0.0),
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
                self.new_star(Kind::HeliumGiant, total, m_hi, 1.0, 0.0)
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
                if total < chandrasekhar {
                    self.white_dwarf(phase, total)
                } else {
                    self.finish_merger(self.white_dwarf(phase, total));
                    self.accretion_induced_collapse(0, total);
                    return;
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
                // A giant-like star is involved: BSE treats the collision as a common envelope.
                if let Some(giant) = [k0, k1].iter().position(|k| k.is_giant_like()) {
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
    fn finish_merger(&mut self, product: Member) {
        self.close_segment();
        self.set_member(0, product);
        self.set_member(1, Member::Gone);
        self.orbit = None;
        self.kind = SegmentKind::Merged;
        self.stripped[1] = false;
        self.corotate(0);
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

    /// The accretor fills its lobe too during transfer: contact (BSE section 2.6.6).
    /// Two main-sequence stars stay in contact for the thermal timescale of the lighter before
    /// they coalesce (HYPERION's choice: BSE merges them at once, which leaves no contact pairs to
    /// be seen); any other pair goes on as a collision.
    pub(super) fn contact(&mut self) {
        let kinds: [Option<(Kind, Structure)>; 2] = core::array::from_fn(|i| {
            let (m, tau) = self.current(i);
            self.structure(i, self.age, m, tau)
                .map(|s| (Kind::of(s.state.phase(), m), s))
        });
        match kinds {
            [Some((k0, s0)), Some((k1, s1))] if k0.is_main_sequence() && k1.is_main_sequence() => {
                let lifetime = kelvin_helmholtz(&s0, k0).min(kelvin_helmholtz(&s1, k1));
                self.kind = SegmentKind::Contact;
                self.contact_until = (self.age + lifetime).min(self.until);
            }
            _ => self.collide(),
        }
    }

    /// Runs a contact pair to its coalescence (see [`Engine::contact`]): the stars keep their
    /// masses and the orbit its separation, and a carried main sequence ages.
    pub(super) fn contact_phase(&mut self) {
        let end = self.contact_until.min(self.until);
        let steps = 16_u32;
        let start = self.age;
        for k in 1..=steps {
            let age = start + (end - start) * f64::from(k) / f64::from(steps);
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
        }
        if self.age >= self.until && end >= self.until && self.contact_until > self.until {
            return;
        }
        self.close_segment();
        self.mix();
    }

    /// Dynamical transfer from member `d` that merges the pair (BSE sections 2.6.4 and 2.6.5):
    /// a low-mass main-sequence donor over its critical ratio gives its whole mass on
    /// √(`τ_KH` `τ_dyn`), of which a main-sequence accretor keeps what its thermal timescale allows
    /// (equation 64) and a giant all; a white dwarf's, neutron star's or black hole's coalesces
    /// with its companion as a collision does.
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
            let timescale = (kelvin_helmholtz(&sd, kd) * dynamical(&sd)).sqrt();
            let kept = (timescale / kelvin_helmholtz(&so, ko) * md).min(md);
            let total = mo + kept;
            let product = if ko.is_main_sequence() {
                self.main_sequence_merger(false, total, [(mo, &so), (0.0, &so)])
            } else {
                self.new_star(
                    Kind::HertzsprungGap,
                    total,
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
        let reach = tau * lifetime + (self.until - self.age).max(0.0);
        let track = Track::to_age(
            mass,
            self.ctx.composition(),
            self.ctx.draws(0),
            Years::new(reach),
        );
        let at = track
            .age_in_phase(Phase::MainSequence, tau)
            .unwrap_or(tau * lifetime);
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
            Some((self.until - self.age).max(0.0)),
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
        let target = match kind {
            Kind::LowMassMainSequence | Kind::MainSequence => {
                return self.main_sequence_star(m, 0.0);
            }
            Kind::HeliumMainSequence => return self.helium_main_sequence(m, burnt),
            Kind::HertzsprungGap => {
                let mass = track_mass(SolarMasses::new(m));
                let track = Track::full(mass, self.ctx.composition(), self.ctx.draws(0));
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
            let phase = if core < 0.5 {
                Phase::HeliumWhiteDwarf
            } else {
                Phase::CarbonOxygenWhiteDwarf
            };
            self.white_dwarf(phase, core.max(1e-3).min(m))
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
        let (m0, phase, fraction) =
            sse::new_star_mass(target, SolarMasses::new(core), self.ctx.coeffs())?;
        let track = match target {
            NewStar::HeliumGiant => {
                Track::helium_star_full(m0, self.ctx.composition(), self.ctx.draws(i))
            }
            NewStar::GiantBranch
            | NewStar::CoreHeliumBurning { .. }
            | NewStar::EarlyAgb
            | NewStar::PulsingAgb => {
                Track::full(track_mass(m0), self.ctx.composition(), self.ctx.draws(i))
            }
        };
        let at = track.age_in_phase(phase, fraction)?;
        Some(Member::Shaped {
            track: Arc::new(track),
            offset: super::evolve::offset_for(self.age, at),
            mass: Path::starting(self.age, m.max(core)),
        })
    }
}

/// What a common envelope whose cores coalesce leaves to the merger: the envelope's binding
/// energy before and after the spiral-in (÷ G) and the two sides.
#[derive(Debug, Clone, Copy)]
struct CommonEnvelopeRemains {
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
