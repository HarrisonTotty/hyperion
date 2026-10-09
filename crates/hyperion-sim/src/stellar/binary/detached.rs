//! Detached evolution (P11.T4.b): winds and their accretion, tides, magnetic braking and
//! gravitational radiation, after Hurley, Tout and Pols (2002, "BSE") sections 2.1–2.4.
//!
//! Each is a secular rate, averaged over the orbit as BSE averages it:
//!
//! - **Winds** (section 2.1): each star's own (plan 06's recipe), of which the companion accretes
//!   a Bondi–Hoyle share (equation 6, with the wind speed of equation 9), no more than 0.8 of it
//!   (equation 10). The orbit loses the wind's specific angular momentum and the accreted mass's
//!   drag (equations 15–21), and each spin loses 2/3 of the wind's (HPT equation 110) and gains
//!   2/3 of the accreted wind's (equation 11).
//! - **Tides** (section 2.3): Hut's (1981) equations 25 and 26 with the damping of equation 30
//!   (convective, Rasio et al. 1996, equations 31–33), 42 (radiative, Zahn 1977; with the square
//!   root over M R² ÷ a⁵ the published code has, which the printed equation lost) or 47
//!   (degenerate, Campbell 1984), no star spun past the equilibrium of equation 34.
//! - **Gravitational radiation** (section 2.4, equations 48 and 49), with Peters's (1964)
//!   coefficient from G and c rather than BSE's rounded 8.315 × 10⁻¹⁰.
//! - **Magnetic braking** (section 2.4, equation 50), of any star above 0.35 M☉ with a convective
//!   envelope, on its spin, which tides pass to the orbit.
//!
//! The rates are integrated by midpoint steps (BSE steps by Euler's rule) under the limits of BSE
//! section 2.8: each star's step (5% of its main sequence, 2% of a Hertzsprung gap or helium main
//! sequence, 1% of a giant phase, the published code's `pts1`–`pts3`), 1% of a star's mass, 2%
//! of the orbital angular momentum (equation 88), 3% of a spin under braking. Every step ends a
//! knot of the segment's paths, so `state_at` joins them linearly: the orbit's evolution is on
//! nodes, as the plan asks, and never replayed. A step never crosses a star's change of phase,
//! which ends the segment.
//!
//! Stars on their own tracks keep plan 06's mass: the wind they lose is their track's, and they
//! accrete no wind (the share a wide companion takes is small, and plan 11's design note 7 leaves
//! wind-fed pairs to be classified from their state).

use crate::math;
use crate::orbit::KeplerElements;
use crate::stellar::Phase;
use crate::stellar::sse::{CORE_GYRATION, ENVELOPE_GYRATION, Structure, Track};
use crate::units::consts::{GM_SUN, SECONDS_PER_JULIAN_YEAR, SOLAR_RADIUS_M, SPEED_OF_LIGHT};
use crate::units::{SolarMasses, Years};

use super::BinaryParams;
use super::evolve::{Engine, roche_lobe};
use super::star::{G, Kind, Member, moment_of_inertia, positive};
use super::timeline::BinaryInput;

/// Peters's (1964) (32/5) G³ M☉³ ÷ (c⁵ R☉⁴) in yr⁻¹: the coefficient of BSE equations 48 and 49,
/// which BSE rounds to 8.315 × 10⁻¹⁰.
pub(super) const GRAVITATIONAL_WAVE_RATE: f64 = 32.0 / 5.0 * (GM_SUN * GM_SUN * GM_SUN)
    / (SPEED_OF_LIGHT * SPEED_OF_LIGHT * SPEED_OF_LIGHT * SPEED_OF_LIGHT * SPEED_OF_LIGHT)
    / (SOLAR_RADIUS_M * SOLAR_RADIUS_M * SOLAR_RADIUS_M * SOLAR_RADIUS_M)
    * SECONDS_PER_JULIAN_YEAR;

/// Magnetic braking's constant, M☉ R☉² yr⁻² with Ω in rad yr⁻¹ (BSE equation 50).
pub(super) const MAGNETIC_BRAKING: f64 = 5.83e-16;

/// No magnetic braking for a fully convective star (BSE section 2.4, Rappaport et al. 1983), M☉.
pub(super) const MAGNETIC_BRAKING_FLOOR: f64 = 0.35;

/// Tides act on a star whose radius is at least this share of its Roche lobe (the published
/// code's `rad ≥ 0.01 rol`).
const TIDAL_REACH: f64 = 0.01;

/// The share of a phase one step may take: the main sequence (the published code's `pts1`).
const MAIN_SEQUENCE_STEP: f64 = 0.05;

/// The share of a Hertzsprung gap or helium main sequence one step may take (`pts3`).
const GAP_STEP: f64 = 0.02;

/// The share of a giant phase one step may take (`pts2`).
const GIANT_STEP: f64 = 0.01;

/// The largest share of a star's mass one step may change (BSE section 2.8).
const MASS_STEP: f64 = 0.01;

/// The largest share of the orbital angular momentum one step may change (BSE equation 88).
const ORBIT_STEP: f64 = 0.02;

/// The largest share of a spin that magnetic braking may take in one step (the published code).
const BRAKING_STEP: f64 = 0.03;

/// The share of the age within which a predicted end of a carried main sequence counts as reached
/// (ruling 132.3).
const MAIN_SEQUENCE_END_REACHED: f64 = 1e-9;

/// Bisections for the onset of Roche-lobe overflow inside a step (BSE section 2.8 interpolates
/// to 1 ≤ R ÷ `R_L` ≤ 1.002).
const ONSET_BISECTIONS: u32 = 30;

/// The pair's state at one age, as the integrator carries it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Snapshot {
    pub(super) age: f64,
    /// Orbital angular momentum, M☉ R☉² yr⁻¹ (zero without an orbit).
    pub(super) j: f64,
    pub(super) e: f64,
    pub(super) spins: [f64; 2],
    pub(super) masses: [f64; 2],
    pub(super) taus: [f64; 2],
}

/// Why a detached run stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Stop {
    /// The pair reached or passed its age: the step that took it there is the last, and what it
    /// landed on lies beyond the timeline.
    Until,
    /// A member reached the end of a phase (not its death).
    Boundary,
    /// Member `i`'s own track dies.
    Death(usize),
    /// The primary's pinned death (design note 16).
    Pinned,
    /// Member `i` fills its Roche lobe.
    Roche(usize),
    /// The stars touch at periastron.
    Collision,
    /// Member `i`, whose mass the binary carries, has lost its envelope.
    Stripped(usize),
    /// The orbit can hold no angular momentum left: the stars spiral together.
    Coalescence,
}

/// The limits on one step: its length from the rates, and the events it may land on, of which
/// the earliest wins where it is no longer than the length (to rounding), so that a step that
/// reaches a boundary always stops there.
///
/// The age the pair is run to is not among them: a step is the same whatever the age, and the
/// one that reaches or passes it is the last (`Engine::integrate`). So the knots up to any age
/// are those of a run to any later one, and a timeline's past does not depend on how far it was
/// run (P11's build-age dependence, 2026-10-05: a last step cut at the age was joined to its
/// knot linearly, where a later run's whole step was, and the states between differed).
#[derive(Debug, Clone, Copy)]
pub(super) struct StepLimit {
    dt: f64,
    event: Option<(f64, Stop)>,
}

impl StepLimit {
    /// A step with no limit yet.
    #[must_use]
    pub(super) const fn new() -> Self {
        Self {
            dt: f64::INFINITY,
            event: None,
        }
    }

    /// Limits the step's length to `dt`.
    pub(super) fn length(&mut self, dt: f64) {
        if dt < self.dt {
            self.dt = dt;
        }
    }

    /// An event `stop` in `dt`.
    pub(super) fn event(&mut self, dt: f64, stop: Stop) {
        if self.event.is_none_or(|(at, _)| dt < at) {
            self.event = Some((dt, stop));
        }
    }

    /// The step's length and the event it lands on, if any.
    #[must_use]
    pub(super) fn resolve(self) -> (f64, Option<Stop>) {
        match self.event {
            Some((at, stop)) if at <= self.dt * (1.0 + 1e-6) => (at.max(0.0), Some(stop)),
            Some(_) | None => (self.dt, None),
        }
    }
}

/// The longest step, years, detached or in stable transfer: 10⁹ years, a tenth of the oldest
/// stars' 13 Gyr.
///
/// The age the pair is run to once cut every step and must not ([`StepLimit`]), so a step its
/// rates would leave long is held here: two white dwarfs 1 au apart, whose orbit's 2% limit allows
/// 10¹⁷ years; a twentieth of a slow main sequence; a remnant left alone by a merger. The last
/// step then reads its stars' laws at most that far past the timeline, at a cost of a step a
/// gigayear.
pub(super) const LONGEST_STEP_YEARS: f64 = 1.0e9;

/// The rates at one snapshot.
#[derive(Debug, Clone, Copy)]
pub(super) struct Rates {
    /// `dJ_orb` ÷ dt without tides, M☉ R☉² yr⁻².
    dj: f64,
    de: f64,
    /// `dJ_spin` ÷ dt without tides.
    spin: [f64; 2],
    /// dΩ ÷ dt from tides, rad yr⁻².
    tide: [f64; 2],
    /// Each star's tidal equilibrium spin, rad yr⁻¹ (BSE equation 34), where tides act.
    equilibrium: [Option<f64>; 2],
    inertia: [f64; 2],
    mass: [f64; 2],
    tau: [f64; 2],
    braking: [f64; 2],
}

impl Engine {
    /// Runs the detached (or single) pair to its next event, recording its steps, and acts on
    /// the event.
    pub(super) fn detached_phase(&mut self) {
        let stop = self.integrate();
        match stop {
            Stop::Until => {}
            Stop::Boundary => {
                self.close_segment();
                self.after_boundary();
            }
            Stop::Death(i) => self.die(i),
            Stop::Pinned => self.pinned_collapse(),
            Stop::Roche(i) => self.roche_onset(i),
            Stop::Collision | Stop::Coalescence => self.collide(),
            Stop::Stripped(i) => self.strip(i),
        }
    }

    /// The snapshot now.
    #[must_use]
    pub(super) fn snapshot(&self) -> Snapshot {
        let (m0, t0) = self.current(0);
        let (m1, t1) = self.current(1);
        let j = self
            .orbit
            .as_ref()
            .map_or(0.0, |o| orbital_momentum(m0, m1, o.a, o.e));
        Snapshot {
            age: self.age,
            j,
            e: self.orbit.as_ref().map_or(0.0, |o| o.e),
            spins: self.spins,
            masses: [m0, m1],
            taus: [t0, t1],
        }
    }

    /// Integrates until an event, recording each step.
    fn integrate(&mut self) -> Stop {
        let mut steps = 0_u32;
        // The last step's end and its structures, which are the next step's start's where the
        // two snapshots give the members the same age, masses and τ.
        let mut carried: Option<(Snapshot, [Option<Structure>; 2])> = None;
        loop {
            if self.age >= self.until {
                return Stop::Until;
            }
            steps += 1;
            if steps > super::evolve::MAX_STEPS {
                self.capped = true;
                return Stop::Until;
            }
            let start = self.snapshot();
            let Some(structures) = self.structures_reusing(&start, carried.as_ref()) else {
                return Stop::Coalescence;
            };
            let rates = self.rates(&start, &structures);
            let (dt, stop) = self.step_limit(&start, &structures, &rates);
            let Some(next) = self.step_from(&start, &structures, &rates, dt) else {
                return Stop::Coalescence;
            };
            let Some(next_structures) = self.structures(&next) else {
                return Stop::Coalescence;
            };
            if let Some(event) = self.contact_check(&next, &next_structures) {
                let (at, event) = match event {
                    Stop::Roche(_) => self.onset(&start, &structures, &rates, dt),
                    other => (next, other),
                };
                self.accept(&at);
                return if self.age >= self.until {
                    Stop::Until
                } else {
                    event
                };
            }
            self.accept(&next);
            if self.age >= self.until {
                return Stop::Until;
            }
            // A star the binary carries whose core has grown into its whole mass has lost its
            // envelope: it is stripped before any other stop on the same step (a death, the pin
            // or a phase boundary), as BSE's `hrdiag` makes such a star a helium star or a white
            // dwarf before `evolv2` acts on the step (HPT section 6; `hrdiag` as `evolv2` calls it; P11.T4.g).
            for (i, structure) in next_structures.iter().enumerate() {
                if let (Member::Shaped { .. }, Some(st)) = (&self.members[i], structure)
                    && st.state.envelope_mass().value() <= 0.0
                    && st.state.phase().is_living()
                {
                    return Stop::Stripped(i);
                }
            }
            if let Some(stop) = stop {
                return stop;
            }
            carried = Some((next, next_structures));
        }
    }

    /// The members' structures at `s`, or `None` if the orbit has no room left.
    #[must_use]
    fn structures(&self, s: &Snapshot) -> Option<[Option<Structure>; 2]> {
        self.structures_reusing(s, None)
    }

    /// [`Engine::structures`] at `s`, taking each member's from `prior` (a snapshot and the
    /// structures there) where `prior` asked for it at the same age, mass and τ, bit for bit.
    ///
    /// A member's structure is a pure function of those three while its form stands, so this
    /// changes nothing but the cost: a detached step's end is the next step's start, and its
    /// structures are half of what a step evaluates (`benches/binary.rs`, 2026-09-25).
    #[must_use]
    fn structures_reusing(
        &self,
        s: &Snapshot,
        prior: Option<&(Snapshot, [Option<Structure>; 2])>,
    ) -> Option<[Option<Structure>; 2]> {
        if self.orbit.is_some() && !positive(s.j) {
            return None;
        }
        let asked = |s: &Snapshot, i: usize| [s.age, s.masses[i].max(1e-9), s.taus[i]];
        // `total_cmp` is equal exactly where the bits are: the same arguments, not near ones.
        let same = |p: &Snapshot, i: usize| {
            asked(p, i)
                .iter()
                .zip(asked(s, i))
                .all(|(a, b)| a.total_cmp(&b).is_eq())
        };
        Some(core::array::from_fn(|i| match prior {
            Some((p, structures)) if same(p, i) => structures[i],
            _ => self.structure(i, s.age, s.masses[i].max(1e-9), s.taus[i]),
        }))
    }

    /// Whether the snapshot `s` has a star over its Roche lobe or the stars touching.
    #[must_use]
    fn contact_check(&self, s: &Snapshot, structures: &[Option<Structure>; 2]) -> Option<Stop> {
        self.orbit.as_ref()?;
        let [Some(s0), Some(s1)] = structures else {
            return None;
        };
        let a = semi_major_axis(s.j, s.masses, s.e);
        let r = [s0.state.radius().value(), s1.state.radius().value()];
        if a * (1.0 - s.e) <= r[0] + r[1] {
            return Some(Stop::Collision);
        }
        let fill = |i: usize| r[i] / roche_lobe(s.masses[i], s.masses[1 - i], a);
        let (f0, f1) = (fill(0), fill(1));
        if f0 >= 1.0 || f1 >= 1.0 {
            return Some(Stop::Roche(usize::from(f0 < f1)));
        }
        None
    }

    /// The snapshot at the onset of Roche-lobe overflow inside the step of `dt` from `start`, by
    /// bisection of the step (BSE section 2.8), and the donor.
    #[must_use]
    fn onset(
        &self,
        start: &Snapshot,
        structures: &[Option<Structure>; 2],
        rates: &Rates,
        dt: f64,
    ) -> (Snapshot, Stop) {
        let (mut lo, mut hi) = (0.0, dt);
        let mut found = None;
        for _ in 0..ONSET_BISECTIONS {
            let mid = lo + 0.5 * (hi - lo);
            let hit = self.step_from(start, structures, rates, mid).and_then(|s| {
                let structures = self.structures(&s)?;
                self.contact_check(&s, &structures).map(|stop| (s, stop))
            });
            match hit {
                Some(hit) => {
                    hi = mid;
                    found = Some(hit);
                }
                None => lo = mid,
            }
        }
        found
            .or_else(|| {
                let s = self.step_from(start, structures, rates, dt)?;
                let structures = self.structures(&s)?;
                self.contact_check(&s, &structures).map(|stop| (s, stop))
            })
            .unwrap_or((*start, Stop::Coalescence))
    }

    /// Takes the snapshot `s` as the pair's state, recording it on the paths.
    pub(super) fn accept(&mut self, s: &Snapshot) {
        self.age = s.age;
        self.spins = s.spins;
        for i in 0..2 {
            self.members[i].record(s.age, s.masses[i], s.taus[i]);
        }
        if let Some(orbit) = &mut self.orbit {
            let a = semi_major_axis(s.j, s.masses, s.e);
            orbit.set(s.age, a, s.e);
        }
    }

    /// One midpoint step of `dt` from `s`, or `None` if the orbit loses all its angular momentum.
    #[cfg(test)]
    #[must_use]
    pub(super) fn step(&self, s: &Snapshot, dt: f64) -> Option<Snapshot> {
        let structures = self.structures(s)?;
        let first = self.rates(s, &structures);
        self.step_from(s, &structures, &first, dt)
    }

    /// One midpoint step of `dt` from `s`, whose members' `structures` and `first` rates
    /// ([`Engine::rates`]) the caller has evaluated, or `None` if the orbit loses all its angular
    /// momentum.
    #[must_use]
    fn step_from(
        &self,
        s: &Snapshot,
        structures: &[Option<Structure>; 2],
        first: &Rates,
        dt: f64,
    ) -> Option<Snapshot> {
        let half = self.apply(s, structures, first, 0.5 * dt)?;
        let half_structures = self.structures(&half)?;
        let second = self.rates(&half, &half_structures);
        self.apply(s, structures, &second, dt)
    }

    /// `s` advanced by `dt` at `rates`, the spins read against the structures `structures` of `s`.
    #[must_use]
    fn apply(
        &self,
        s: &Snapshot,
        structures: &[Option<Structure>; 2],
        rates: &Rates,
        dt: f64,
    ) -> Option<Snapshot> {
        let age = s.age + dt;
        let mut j = s.j + rates.dj * dt;
        let mut spins = s.spins;
        for i in 0..2 {
            let Some(structure) = &structures[i] else {
                continue;
            };
            let inertia = rates.inertia[i];
            if !positive(inertia) {
                continue;
            }
            let own = (s.spins[i] + rates.spin[i] * dt).max(1e-10);
            let omega = own / inertia;
            let mut target = omega + rates.tide[i] * dt;
            // A tide drives the spin towards its equilibrium and never past it (BSE section 2.3,
            // "ensuring that the star is not spun down (or up) past the equilibrium spin").
            if let Some(eq) = rates.equilibrium[i] {
                let (lo, hi) = if omega < eq { (omega, eq) } else { (eq, omega) };
                target = target.clamp(lo, hi);
            }
            let tidal = inertia * (target - omega);
            let mut spin = own + tidal;
            j -= tidal;
            let (m, r) = (
                structure.state.mass().value(),
                structure.state.radius().value(),
            );
            if r > 0.0 {
                // No star spins past break-up; the excess leaves with the star's own mass (the
                // published code caps the spin so in a detached pair).
                let break_up = inertia * (G * m / (r * r * r)).sqrt();
                spin = spin.min(break_up);
            }
            spins[i] = spin.max(1e-10);
        }
        let e = {
            let e = s.e + rates.de * dt;
            if e < 1e-10 { 0.0 } else { e.min(0.9999) }
        };
        let mut masses = s.masses;
        let mut taus = s.taus;
        for i in 0..2 {
            match &self.members[i] {
                Member::Track { track, offset } => {
                    // A track that dies inside the step holds its last living mass to the step's
                    // end, the death: what the death sheds leaves at the death (`Engine::die`),
                    // not as a wind the orbit's angular momentum is kept through (ruling 129.4a).
                    let lifetime = track.lifetime().map(Years::value);
                    masses[i] = match lifetime {
                        Some(t) if t + offset > s.age && t + offset <= age => {
                            track.mass_at(last_living(t))
                        }
                        _ => self.members[i].mass_at(age),
                    };
                }
                Member::Frozen { .. } | Member::Gone => {
                    masses[i] = self.members[i].mass_at(age);
                }
                Member::Shaped { .. } | Member::Cooling { .. } | Member::Remnant { .. } => {
                    masses[i] = (s.masses[i] + rates.mass[i] * dt).max(1e-9);
                }
                Member::MainSequence { .. } => {
                    masses[i] = (s.masses[i] + rates.mass[i] * dt).max(1e-9);
                    taus[i] = self.main_sequence_tau(
                        i,
                        (s.taus[i] + rates.tau[i] * dt).min(1.0),
                        masses[i],
                        age,
                    );
                }
            }
        }
        if self.orbit.is_some() && !positive(j) {
            return None;
        }
        Some(Snapshot {
            age,
            j: if self.orbit.is_some() { j } else { 0.0 },
            e,
            spins,
            masses,
            taus,
        })
    }

    /// τ of member `i`'s carried main sequence at `age`, of mass `mass` there, with its end held
    /// as reached: τ = 1 exactly where what is left of it, (1 − τ) times the lifetime at `mass`,
    /// falls within 10⁻⁹ of the age. A step that lands on the end, predicted at the step's start,
    /// leaves a rejuvenated accretor a little short of it, and the next prediction a little later;
    /// the steps close in on it without end but for this (ruling 132.3, pair 0077).
    #[must_use]
    pub(super) fn main_sequence_tau(&self, i: usize, tau: f64, mass: f64, age: f64) -> f64 {
        let Member::MainSequence { helium, .. } = &self.members[i] else {
            return tau;
        };
        let lifetime =
            crate::stellar::sse::main_sequence_lifetime(self.ctx.coeffs(), *helium, mass);
        if (1.0 - tau).max(0.0) * lifetime <= MAIN_SEQUENCE_END_REACHED * age.abs() {
            1.0
        } else {
            tau
        }
    }

    /// The rates at `s` with the members' `structures` there.
    #[expect(
        clippy::many_single_char_names,
        reason = "the names are BSE's own symbols"
    )]
    #[expect(
        clippy::too_many_lines,
        reason = "one block per secular rate of BSE sections 2.1–2.4"
    )]
    #[must_use]
    fn rates(&self, s: &Snapshot, structures: &[Option<Structure>; 2]) -> Rates {
        let params = self.ctx.params();
        let mut r = Rates {
            dj: 0.0,
            de: 0.0,
            spin: [0.0; 2],
            tide: [0.0; 2],
            equilibrium: [None; 2],
            inertia: [0.0; 2],
            mass: [0.0; 2],
            tau: [0.0; 2],
            braking: [0.0; 2],
        };
        let wind: [f64; 2] = core::array::from_fn(|i| {
            structures[i]
                .as_ref()
                .map_or(0.0, |st| st.state.mass_loss_rate().value().max(0.0))
        });
        for (i, st) in structures.iter().enumerate() {
            let Some(st) = st else {
                continue;
            };
            r.inertia[i] = moment_of_inertia(st);
            if let Member::MainSequence { helium, .. } = &self.members[i] {
                let lifetime = crate::stellar::sse::main_sequence_lifetime(
                    self.ctx.coeffs(),
                    *helium,
                    s.masses[i],
                );
                r.tau[i] = 1.0 / lifetime;
            }
        }
        let omega_spin: [f64; 2] = core::array::from_fn(|i| {
            if r.inertia[i] > 0.0 {
                s.spins[i] / r.inertia[i]
            } else {
                0.0
            }
        });
        // Spins: winds, and magnetic braking (BSE equation 50).
        for i in 0..2 {
            let Some(st) = &structures[i] else {
                continue;
            };
            let (m, radius) = (st.state.mass().value(), st.state.radius().value());
            if params.wind_angular_momentum {
                r.spin[i] -= 2.0 / 3.0 * wind[i] * radius * radius * omega_spin[i];
            }
            let kind = Kind::of(st.state.phase(), m);
            if params.magnetic_braking
                && m > MAGNETIC_BRAKING_FLOOR
                && !kind.is_remnant()
                && m > 0.0
            {
                let v = radius * omega_spin[i];
                let braking = MAGNETIC_BRAKING * st.envelope.mass / m * v * v * v;
                r.spin[i] -= braking;
                r.braking[i] = braking;
            }
            if self.members[i].carries_mass() {
                r.mass[i] -= wind[i];
            }
        }
        let Some(_) = self.orbit else {
            return r;
        };
        let [Some(s0), Some(s1)] = structures else {
            return r;
        };
        let m = s.masses;
        let total = m[0] + m[1];
        let e = s.e;
        let a = semi_major_axis(s.j, m, e);
        let omega = (G * total / (a * a * a)).sqrt();
        let one_e2 = 1.0 - e * e;
        let sqrt_one_e2 = one_e2.sqrt();
        let st = [s0, s1];
        // Wind accretion (BSE equations 6–10) onto a member whose mass the binary carries.
        let mut accreted = [0.0; 2];
        let v_orb2 = G * total / a;
        for i in 0..2 {
            let jdx = 1 - i;
            if !self.members[jdx].carries_mass() || !positive(wind[i]) {
                continue;
            }
            let r_i = st[i].state.radius().value();
            let beta = params
                .wind_speed_factor
                .of(Kind::of(st[i].state.phase(), m[i]), m[i]);
            let v_wind2 = 2.0 * beta * G * m[i] / r_i.max(1e-6);
            let ratio = {
                let x = 1.0 + v_orb2 / v_wind2;
                x * x.sqrt()
            };
            let focus = G * m[jdx] / v_wind2;
            let rate =
                params.bondi_hoyle * focus * focus / (2.0 * a * a) / ratio / sqrt_one_e2 * wind[i];
            accreted[jdx] = rate.min(params.max_wind_accretion * wind[i]);
            // A degenerate accretor takes a wind no faster than its Eddington rate either (ruling
            // 132.1: the limit is the accretor's, whatever feeds it).
            let ka = Kind::of(st[jdx].state.phase(), m[jdx]);
            if params.eddington_limit && ka.is_remnant() && ka != Kind::Massless {
                let surface = Kind::of(st[i].state.phase(), m[i]).surface();
                let limit =
                    super::rlof::eddington_rate(&self.ctx, surface, st[jdx].state.radius().value());
                accreted[jdx] = accreted[jdx].min(limit);
            }
            r.mass[jdx] += accreted[jdx];
            // The accreted wind brings its donor's specific spin (the published code's `djtx`).
            if params.wind_angular_momentum {
                r.spin[jdx] += 2.0 / 3.0 * accreted[jdx] * r_i * r_i * omega_spin[i];
            }
        }
        // The orbit's loss to winds and their accretion (BSE equations 14–21, as the published
        // code writes them) and the accretion's circularisation (equation 15).
        if params.wind_angular_momentum {
            let q = [m[0] / m[1], m[1] / m[0]];
            r.dj -= ((wind[0] + q[0] * accreted[0]) * m[1] * m[1]
                + (wind[1] + q[1] * accreted[1]) * m[0] * m[0])
                * a
                * a
                * sqrt_one_e2
                * omega
                / (total * total);
            r.de -= e
                * (accreted[0] * (0.5 / m[0] + 1.0 / total)
                    + accreted[1] * (0.5 / m[1] + 1.0 / total));
        }
        // Gravitational radiation (BSE equations 48 and 49).
        if params.gravitational_radiation {
            let a4 = a * a * a * a;
            let rate =
                GRAVITATIONAL_WAVE_RATE * m[0] * m[1] * total / (a4 * math::powi(sqrt_one_e2, 5));
            r.dj -= s.j * rate * (1.0 + 0.875 * e * e);
            r.de -= e * rate * (19.0 / 6.0 + 121.0 / 96.0 * e * e);
        }
        // Tides (BSE section 2.3).
        if params.tides {
            let lobes = [roche_lobe(m[0], m[1], a), roche_lobe(m[1], m[0], a)];
            let donor_like = {
                let fill = |i: usize| st[i].state.radius().value() / lobes[i];
                usize::from(fill(0) < fill(1))
            };
            for i in 0..2 {
                let lobe = lobes[i];
                let kind = Kind::of(st[i].state.phase(), m[i]);
                let acts = if kind.is_remnant() {
                    i == donor_like && kind.is_white_dwarf()
                } else {
                    st[i].state.radius().value() >= TIDAL_REACH * lobe
                };
                if !acts || !positive(r.inertia[i]) {
                    continue;
                }
                if let Some(tide) = tide(st[i], kind, m[1 - i], a, e, omega, omega_spin[i]) {
                    r.de += tide.de;
                    r.tide[i] = tide.spin_rate;
                    r.equilibrium[i] = Some(tide.equilibrium);
                }
            }
        }
        r
    }

    /// The step's length from `s` and the event it lands on, if its limit is one (BSE section
    /// 2.8's limits; see the module documentation).
    #[must_use]
    fn step_limit(
        &self,
        s: &Snapshot,
        structures: &[Option<Structure>; 2],
        rates: &Rates,
    ) -> (f64, Option<Stop>) {
        let mut limits = StepLimit::new();
        limits.length(LONGEST_STEP_YEARS);
        if let Some(pin) = &self.pin {
            let at = pin.death.age().value();
            if at > s.age {
                limits.event(at - s.age, Stop::Pinned);
            }
        }
        for (i, structure) in structures.iter().enumerate() {
            match &self.members[i] {
                Member::Track { track, offset } | Member::Shaped { track, offset, .. } => {
                    let (start, end, track_age) = super::evolve::phase_ahead(track, *offset, s.age);
                    if end.is_finite() {
                        let dies = track.lifetime().is_some_and(|t| t.value() <= end);
                        let is_death = dies
                            && track
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
                        let kind = structure
                            .as_ref()
                            .map_or(Kind::Massless, |st| Kind::of(st.state.phase(), s.masses[i]));
                        limits.length(phase_step(kind) * (end - start));
                    }
                }
                Member::MainSequence { helium, .. } => {
                    let lifetime = crate::stellar::sse::main_sequence_lifetime(
                        self.ctx.coeffs(),
                        *helium,
                        s.masses[i],
                    );
                    limits.event((1.0 - s.taus[i]) * lifetime, Stop::Boundary);
                    limits.length(MAIN_SEQUENCE_STEP * lifetime);
                }
                Member::Cooling { .. }
                | Member::Frozen { .. }
                | Member::Remnant { .. }
                | Member::Gone => {}
            }
            if self.members[i].carries_mass() {
                let rate = rates.mass[i];
                if rate.abs() > 0.0 {
                    limits.length(MASS_STEP * s.masses[i] / rate.abs());
                }
                if let (Member::Shaped { .. }, Some(st)) = (&self.members[i], structure)
                    && rate < 0.0
                    && st.state.phase().is_living()
                {
                    let envelope = s.masses[i] - st.state.core_mass().value();
                    if envelope > 0.0 {
                        limits.event(envelope / (-rate), Stop::Stripped(i));
                    } else {
                        limits.event(0.0, Stop::Stripped(i));
                    }
                }
            }
            if rates.braking[i] > 0.0 {
                limits.length(BRAKING_STEP * s.spins[i] / rates.braking[i]);
            }
        }
        if self.orbit.is_some() {
            // A tide moves at most the angular momentum that brings its star to equilibrium, so
            // one that cannot move 2% of the orbit's in any step does not limit it.
            let tidal: f64 = (0..2)
                .filter(|&i| {
                    let omega = if rates.inertia[i] > 0.0 {
                        s.spins[i] / rates.inertia[i]
                    } else {
                        0.0
                    };
                    rates.equilibrium[i]
                        .is_some_and(|eq| rates.inertia[i] * (omega - eq).abs() > ORBIT_STEP * s.j)
                })
                .map(|i| rates.inertia[i] * rates.tide[i])
                .sum();
            let total = (rates.dj - tidal).abs();
            if total > 0.0 {
                limits.length(ORBIT_STEP * s.j / total);
            }
        }
        let floor = (1e-9 * s.age).max(1e-3);
        let (dt, stop) = limits.resolve();
        if dt < floor && stop.is_none() {
            (floor, None)
        } else {
            (dt.max(0.0), stop)
        }
    }

    /// The relative orbit's elements now, for a supernova's geometry.
    #[must_use]
    pub(super) fn elements_now(&self, masses: [f64; 2]) -> Option<KeplerElements> {
        let orbit = self.orbit.as_ref()?;
        KeplerElements::from_semi_major_axis(
            orbit.axis_metres(),
            crate::units::GravitationalParameter::from_solar_masses(
                crate::units::SolarMasses::new(masses[0] + masses[1]),
            ),
            crate::orbit::Eccentricity::new(orbit.e.min(0.9998)).ok()?,
            orbit.orientation,
            orbit.mean_anomaly,
        )
        .ok()
    }

    /// The age now as a track age of member `i`, or the age itself for a member on no track.
    #[must_use]
    pub(super) fn member_age(&self, i: usize) -> Years {
        Years::new(match self.members[i].track() {
            Some((_, offset)) => (self.age - offset).max(0.0),
            None => self.age,
        })
    }
}

/// The share of a phase one step may take for a star of `kind`.
#[must_use]
const fn phase_step(kind: Kind) -> f64 {
    match kind {
        Kind::LowMassMainSequence | Kind::MainSequence => MAIN_SEQUENCE_STEP,
        Kind::HertzsprungGap | Kind::HeliumMainSequence => GAP_STEP,
        Kind::GiantBranch
        | Kind::CoreHeliumBurning
        | Kind::EarlyAgb
        | Kind::PulsingAgb
        | Kind::HeliumGap
        | Kind::HeliumGiant => GIANT_STEP,
        Kind::HeliumWhiteDwarf
        | Kind::CarbonOxygenWhiteDwarf
        | Kind::OxygenNeonWhiteDwarf
        | Kind::NeutronStar
        | Kind::BlackHole
        | Kind::Massless => 1.0,
    }
}

/// A track age just before a track's death at track age `death`, years: the star's last living
/// instant, at which its state is still the living star's.
#[must_use]
pub(super) fn last_living(death: f64) -> f64 {
    (death * (1.0 - 1e-12)).max(0.0)
}

/// The orbital angular momentum of masses `m0` and `m1` on an orbit of `a` (R☉) and `e`,
/// M☉ R☉² yr⁻¹: m₀ m₁ √(G a (1 − e²) ÷ (m₀ + m₁)).
#[must_use]
pub(super) fn orbital_momentum(m0: f64, m1: f64, a: f64, e: f64) -> f64 {
    let total = m0 + m1;
    if !(positive(total) && positive(a)) {
        return 0.0;
    }
    m0 * m1 * (G * a * (1.0 - e * e) / total).sqrt()
}

/// The semi-major axis, R☉, of angular momentum `j` for `masses` and `e`: J² M ÷ (G m₀² m₁²
/// (1 − e²)).
#[must_use]
pub(super) fn semi_major_axis(j: f64, masses: [f64; 2], e: f64) -> f64 {
    let [m0, m1] = masses;
    j * j * (m0 + m1) / (G * m0 * m0 * m1 * m1 * (1.0 - e * e))
}

/// A tide's effect on one star.
struct Tide {
    de: f64,
    spin_rate: f64,
    equilibrium: f64,
}

/// The tide raised on a star of structure `st` and `kind` by a companion of `m2` on an orbit of
/// `a`, `e` and orbital frequency `omega`, the star spinning at `spin` (BSE section 2.3: Hut's
/// equations 25, 26 and 34 with the damping of equations 30, 42 or 47).
#[must_use]
fn tide(
    st: &Structure,
    kind: Kind,
    m2: f64,
    a: f64,
    e: f64,
    omega: f64,
    spin: f64,
) -> Option<Tide> {
    let m = st.state.mass().value();
    let radius = st.state.radius().value();
    if !(positive(m) && positive(radius) && positive(a)) {
        return None;
    }
    let q = m2 / m;
    let ra = radius / a;
    let ra2 = ra * ra;
    let ra6 = ra2 * ra2 * ra2;
    let (k_over_t, rg2) = damping(st, kind, q, a, omega, spin)?;
    let e2 = e * e;
    let one_e2 = 1.0 - e2;
    let sqrt_one_e2 = one_e2.sqrt();
    let cube = one_e2 * sqrt_one_e2;
    // Hut's (1981) polynomials in e².
    let f2 = hut_f2(e2);
    let f3 = 1.0 + e2 * (3.75 + e2 * (1.875 + e2 * 0.078_125));
    let f4 = 1.0 + e2 * (1.5 + e2 * 0.125);
    let f5 = hut_f5(e2);
    let de = -27.0 * k_over_t * q * (1.0 + q) * ra6 * ra2 * e
        / math::powi(sqrt_one_e2, 13).max(1e-300)
        * (f3 - 11.0 / 18.0 * cube * f4 * spin / omega);
    let spin_rate = 3.0 * k_over_t * q * q / rg2 * ra6 * omega / math::powi(one_e2, 6)
        * (f2 - cube * f5 * spin / omega);
    Some(Tide {
        de,
        spin_rate,
        equilibrium: f2 * omega / (f5 * cube),
    })
}

/// Hut's (1981, A&A 99, 126) f₂ in `e2` = e²: 1 + 15/2 e² + 45/8 e⁴ + 5/16 e⁶, which BSE's
/// equations 25, 26 and 34 use.
#[must_use]
pub(super) fn hut_f2(e2: f64) -> f64 {
    1.0 + e2 * (7.5 + e2 * (5.625 + e2 * 0.3125))
}

/// Hut's (1981, A&A 99, 126) f₅ in `e2` = e²: 1 + 3 e² + 3/8 e⁴, which BSE's equations 25, 26
/// and 34 use.
#[must_use]
pub(super) fn hut_f5(e2: f64) -> f64 {
    1.0 + e2 * (3.0 + e2 * 0.375)
}

/// (k ÷ T), yr⁻¹, and `r_g²` of the tide on a star of structure `st` and `kind` (BSE equations 30,
/// 42 and 47), with the companion's mass ratio `q` = M₂ ÷ M, the orbit's `a` and `omega` and the
/// star's `spin`.
#[expect(
    clippy::many_single_char_names,
    reason = "the names are BSE's own symbols"
)]
#[must_use]
fn damping(
    st: &Structure,
    kind: Kind,
    q: f64,
    a: f64,
    omega: f64,
    spin: f64,
) -> Option<(f64, f64)> {
    let m = st.state.mass().value();
    let radius = st.state.radius().value();
    let mc = st.state.core_mass().value();
    let radiative = matches!(kind, Kind::CoreHeliumBurning | Kind::HeliumMainSequence)
        || (kind == Kind::MainSequence && m >= 1.25);
    if radiative {
        // Zahn's (1975, 1977) dynamical tide, E₂ = 1.592 × 10⁻⁹ M^2.84 (BSE equations 42 and 43,
        // with the square root the published code has).
        let e2 = 1.592e-9 * math::powf_positive(m, 2.84);
        let k_over_t = 1.9782e4
            * (m * radius * radius / math::powi(a, 5)).sqrt()
            * math::powf_positive(1.0 + q, 5.0 / 6.0)
            * e2;
        return Some((k_over_t, crate::stellar::sse::ENVELOPE_GYRATION));
    }
    if kind.is_remnant() {
        // Campbell's (1984) degenerate damping (BSE equation 47), r_g² = k′₃.
        let l = st.state.luminosity().value();
        let rg2 = crate::stellar::sse::CORE_GYRATION;
        let k_over_t = 2.564e-8 * rg2 * math::powf_positive((l / m).max(1e-30), 5.0 / 7.0);
        return Some((k_over_t, rg2));
    }
    // The equilibrium tide with convective damping (BSE equations 30–33).
    let envelope = st.envelope.mass;
    let depth = st
        .envelope
        .depth
        .min(radius - st.core_radius.value())
        .max(1e-10);
    if !positive(envelope) {
        return None;
    }
    let l = st.state.luminosity().value().max(1e-10);
    let tau_conv = 0.4311 * math::cbrt(envelope * depth * (radius - 0.5 * depth) / (3.0 * l));
    let p_tid = core::f64::consts::TAU / (1e-10 + (omega - spin).abs());
    let f = {
        let x = p_tid / (2.0 * tau_conv);
        (x * x).min(1.0)
    };
    let k_over_t = 2.0 / 21.0 * f / tau_conv * envelope / m;
    let rg2 = crate::stellar::sse::ENVELOPE_GYRATION * (m - mc).max(0.0) / m;
    if !positive(rg2) {
        return None;
    }
    Some((k_over_t, rg2))
}

// ---------------------------------------------------------------------------------------------
// The pre-test's bound on the engine's own decay (P11.T4.j).

/// The pieces each segment of a star's track is cut into for [`decay_reaches`]'s braking integral
/// (ruling p11-channels of 2026-10-06, section 3.1, item 4).
const DECAY_PIECES: u32 = 4;

/// The factor on the decay that [`decay_reaches`] bounds (ruling p11-channels, section 3.1, item
/// 6): for the midpoint integrator, and for the drawn masses standing for the stars' own, which
/// only lose mass.
pub(super) const DECAY_SAFETY: f64 = 1.25;

/// Whether the engine's own sinks of orbital angular momentum can bring a pair into design note
/// 7's lobe test by `until_years`: the pre-test's bound (P11.T4.j, ruling p11-channels of
/// 2026-10-06, section 3.1).
///
/// `members` are the stars on their own forms from zero age (`evolve.rs`'s `own_members`),
/// stepped from `start_years` (the first arrival), and `radii_rsun` their largest radii up to
/// `until_years` as the lobe test reads them, R☉. The bound is the engine's, term by term, so that
/// a pair it passes over shows no interaction before `until_years` in a run to any later age.
///
/// In the semi-latus rectum p = a (1 − e²), with the drawn orbit's p₀ and e₀, the drawn masses m₁
/// and m₂ (M = m₁ + m₂, μ = m₁ m₂ ÷ M) and the engine's units (R☉, M☉, yr):
///
/// 1. The lobe test passes at the periastron `r_c` = maxᵢ Rᵢ ÷ `f_L`(mᵢ ÷ mⱼ) (Eggleton 1983), or
///    `p_c` = `r_c` (1 + e₀).
/// 2. Tides spin no star past its equilibrium Φ √(G M ÷ p³) (BSE equation 34), Φ = f₂(e) ÷ f₅(e)
///    (Hut 1981), taken at e₀: e falls under gravitational radiation, and under tides while no
///    star spins faster than about 1.3 times its equilibrium (Hut 1981; BSE equation 25 and section
///    2.3); a faster star raises e but gives the orbit angular momentum, and the engine reads the
///    lobe at a ≥ p, so `p_c` bounds its Roche test whatever e does. Magnetic braking at that spin
///    (BSE equation 50, K = 5.83 × 10⁻¹⁶) takes
///    d(p⁵) ÷ dt = −10 K (G M ÷ μ) Φ³ Σᵢ (`M_env` ÷ M)ᵢ Rᵢ³ from the orbit, whatever p is.
/// 3. The spins can take at most S = `Ω_c` Σᵢ Iᵢ from the orbit, `Ω_c` the equilibrium spin at
///    `p_c` and Iᵢ bounding BSE equation 35's k′₂ (M − Mc) R² + k′₃ Mc Rc² over the span: k′₂ mᵢ
///    Rᵢ² ([`ENVELOPE_GYRATION`]), plus k′₃ Mc Rc² ([`CORE_GYRATION`]) at their largest
///    ([`largest_core`]), which is zero on the main sequence, and no more than k′₃ mᵢ Rᵢ². That is
///    the Darwin instability's spin-up and a star's growth at a locked spin. Taken first, it leaves
///    J′ = J₀ − S of the orbit's J₀ = μ √(G M p₀): the pair can interact if J′ ≤ 0, and otherwise
///    starts from p′ = p₀ (J′ ÷ J₀)².
/// 4. Braking, B = 10 K (G M ÷ μ) Φ³ Σᵢ ∫ (`M_env` ÷ M)ᵢ Rᵢ³ dt over the span, of each star above
///    [`MAGNETIC_BRAKING_FLOOR`] (no remnant brakes), bounded piece by piece
///    ([`braking_integral`]).
/// 5. Gravitational radiation (BSE equation 48, Peters 1964), W = 10 β′ m₁ m₂ M (1 + 7/8 e₀²) p₀
///    times the span, β′ = [`GRAVITATIONAL_WAVE_RATE`]: its rate at p₀ bounds it at every p ≤ p₀.
/// 6. The pair can interact if p′⁵ − [`DECAY_SAFETY`] (B + W) ≤ `p_c`⁵.
/// 7. A coarse test comes first, with every envelope's share 1 and each star's largest radius
///    over the whole span, and Iᵢ at most k′₃ mᵢ Rᵢ² for a star with a core: a pair it rejects
///    cannot interact, and most pairs end there. It is taken again with the core read from the
///    track before the braking integral.
///
/// The bound is conservative in each term it holds: circularisation keeps p; a super-synchronous
/// star spins the orbit up, and a sub-synchronous one brakes more weakly than at equilibrium, its
/// spin-up being the reservoir's; every term is at its largest. Winds widen the orbit by the mass
/// they take and are left out, but the spin a locked star's wind carries off (HPT equation 110),
/// which the tides restore from the orbit, is not bounded here (ruling p11-channels' reservoir, as
/// ruled): the slack of the reservoir and the zero-miss gate guard it. Before
/// the pair's first interaction every star keeps its track's mass, so the braking floor read on
/// the drawn mass is the engine's. The bound widens with `until_years` as `p_c`, B and W grow,
/// though `Ω_c` falls with `p_c`: tested (`the_pre_test_only_widens_with_age`), not proven. A term
/// whose [`BinaryParams`](super::BinaryParams) switch is off is left out: braking and the spins'
/// reservoir reach the orbit through the tides.
#[must_use]
pub(super) fn decay_reaches(
    input: &BinaryInput,
    members: &[Member; 2],
    start_years: f64,
    until_years: f64,
    radii_rsun: [f64; 2],
) -> bool {
    let (start, until, radii) = (start_years, until_years, radii_rsun);
    let span = until - start;
    let m = input.masses().map(SolarMasses::value);
    let orbit = input.orbit();
    let e0 = orbit.eccentricity().value();
    let e2 = e0 * e0;
    let p0 = orbit.semi_major_axis().value() / SOLAR_RADIUS_M * (1.0 - e2);
    // 1. The critical orbit.
    let p_c = (0..2)
        .map(|i| radii[i] / roche_lobe(m[i], m[1 - i], 1.0))
        .fold(0.0, f64::max)
        * (1.0 + e0);
    if !(positive(span) && positive(p_c) && positive(p0)) {
        return false;
    }
    // 2, 5 and 6: the pair-wide terms.
    let decay = Decay::new(input.params(), m, e2, p0, p_c, span);
    let brakes = |i: usize| decay.brakes(m[i]);
    let track_of = |i: usize| match &members[i] {
        Member::Track { track, offset } => Some((&**track, *offset)),
        Member::Shaped { .. }
        | Member::MainSequence { .. }
        | Member::Cooling { .. }
        | Member::Frozen { .. }
        | Member::Remnant { .. }
        | Member::Gone => None,
    };
    let cored: [Option<(&Track, f64)>; 2] = core::array::from_fn(|i| {
        track_of(i).filter(|&(track, offset)| past_main_sequence(track, offset, until))
    });
    // 3. The spins' moments of inertia: k′₂ m R² with no core, and at most k′₃ m R² (Rc ≤ R).
    let envelope_inertia = |i: usize| ENVELOPE_GYRATION * m[i] * radii[i] * radii[i];
    let inertia_bound = |i: usize| {
        if cored[i].is_some() {
            CORE_GYRATION * m[i] * radii[i] * radii[i]
        } else {
            envelope_inertia(i)
        }
    };
    // 7. The coarse test.
    let coarse_braking = both(|i| {
        if brakes(i) {
            radii[i] * radii[i] * radii[i] * span
        } else {
            0.0
        }
    });
    if !decay.reaches(both(inertia_bound), coarse_braking) {
        return false;
    }
    // 3. The core read from the track, and the coarse test again with it: with no star past its
    // main sequence, the inertia is the coarse test's and the test is not repeated.
    let inertia = if cored.iter().any(Option::is_some) {
        let read = both(|i| match cored[i] {
            Some((track, offset)) => {
                let (mc, rc) = largest_core(track, offset, start, until);
                (envelope_inertia(i) + CORE_GYRATION * mc * rc * rc).min(inertia_bound(i))
            }
            None => envelope_inertia(i),
        });
        if !decay.reaches(read, coarse_braking) {
            return false;
        }
        read
    } else {
        both(envelope_inertia)
    };
    // 4. Braking, piece by piece.
    let braking = both(|i| match track_of(i) {
        Some((track, offset)) if brakes(i) => braking_integral(track, offset, start, until),
        Some(_) | None => 0.0,
    });
    decay.reaches(inertia, braking)
}

/// The sum of `f` over the pair's two stars, the primary's first.
#[must_use]
fn both(f: impl Fn(usize) -> f64) -> f64 {
    f(0) + f(1)
}

/// The pair-wide terms of [`decay_reaches`]'s test.
struct Decay {
    /// J₀, the drawn orbit's angular momentum, M☉ R☉² yr⁻¹.
    j0: f64,
    /// p₀, the drawn orbit's semi-latus rectum, R☉.
    p0_rsun: f64,
    /// `p_c`⁵, the lobe test's semi-latus rectum to the fifth, R☉⁵.
    p_c5: f64,
    /// `Ω_c`, rad yr⁻¹, where tides pass the spins' angular momentum to the orbit, else 0.
    omega_c: f64,
    /// 10 K (G M ÷ μ) Φ³, R☉² yr⁻¹, where braking reaches the orbit, else 0.
    braking: f64,
    /// W, R☉⁵.
    radiation: f64,
}

impl Decay {
    /// The terms for the drawn masses `masses_msun`, e₀² `e2`, p₀ and `p_c` (`p0_rsun`,
    /// `p_c_rsun`) and the span's length `span_years`, under `params`.
    #[must_use]
    fn new(
        params: &BinaryParams,
        masses_msun: [f64; 2],
        e2: f64,
        p0_rsun: f64,
        p_c_rsun: f64,
        span_years: f64,
    ) -> Self {
        let [m1, m2] = masses_msun;
        let total = m1 + m2;
        let mu = m1 * m2 / total;
        let phi = hut_f2(e2) / hut_f5(e2);
        let omega_c = if params.tides {
            phi * (G * total / (p_c_rsun * p_c_rsun * p_c_rsun)).sqrt()
        } else {
            0.0
        };
        let braking = if params.tides && params.magnetic_braking {
            10.0 * MAGNETIC_BRAKING * G * total / mu * phi * phi * phi
        } else {
            0.0
        };
        let radiation = if params.gravitational_radiation {
            10.0 * GRAVITATIONAL_WAVE_RATE
                * m1
                * m2
                * total
                * (1.0 + 0.875 * e2)
                * p0_rsun
                * span_years
        } else {
            0.0
        };
        Self {
            j0: mu * (G * total * p0_rsun).sqrt(),
            p0_rsun,
            p_c5: math::powi(p_c_rsun, 5),
            omega_c,
            braking,
            radiation,
        }
    }

    /// Whether a star of drawn mass `mass_msun` brakes the orbit: above BSE's floor, with braking
    /// and tides on.
    #[must_use]
    fn brakes(&self, mass_msun: f64) -> bool {
        mass_msun > MAGNETIC_BRAKING_FLOOR && positive(self.braking)
    }

    /// Items 3 and 6: whether the orbit, less the spins' reservoir of the moments of inertia
    /// `inertia` (M☉ R☉²), reaches `p_c` under the braking integral `braking` (R☉³ yr) and
    /// gravitational radiation.
    #[must_use]
    fn reaches(&self, inertia: f64, braking: f64) -> bool {
        let j = self.j0 - self.omega_c * inertia;
        if j <= 0.0 {
            return true;
        }
        let ratio = j / self.j0;
        let p = self.p0_rsun * ratio * ratio;
        math::powi(p, 5) - DECAY_SAFETY * (self.braking * braking + self.radiation) <= self.p_c5
    }
}

/// Whether a star on `track`, placed at `offset_years`, has left its main sequence by the engine's
/// age `until_years` (a track with no main sequence counts as having left it): only then can it
/// hold a core.
#[must_use]
fn past_main_sequence(track: &Track, offset_years: f64, until_years: f64) -> bool {
    match track.main_sequence_end() {
        Some(end) => end.value() < until_years - offset_years,
        None => track.main_sequence_arrival().is_none(),
    }
}

/// The track ages, years, from the star's arrival to `until_years`, at which a star on `track`
/// placed at `offset_years` and stepped from the engine's age `start_years` has each segment of its
/// track open in the span: the span's start or the segment's own. A remnant's segment, which never
/// ends, is left out: a remnant neither brakes nor holds a core beyond its whole self, which the
/// envelope term holds.
fn openings(
    track: &Track,
    offset_years: f64,
    start_years: f64,
    until_years: f64,
) -> impl Iterator<Item = (f64, f64, f64)> + '_ {
    let hi = (until_years - offset_years).max(0.0);
    let from = (start_years - offset_years)
        .max(0.0)
        .max(track.main_sequence_arrival().map_or(0.0, Years::value));
    track
        .segment_ages()
        .take_while(move |&(s, _, _)| s < hi)
        .filter(move |&(_, e, _)| e > from && e.is_finite())
        .map(move |(s, e, _)| (s.max(from), s, e))
}

/// The track ages, years, of the pieces of [`braking_integral`] for a star on `track` placed at
/// `offset_years`, from the engine's age `start_years` to `until_years`: each segment of
/// [`openings`] cut into [`DECAY_PIECES`] equal pieces whatever the span, so that the bound only
/// grows with it, each clipped to the span, with the segment's opening.
fn pieces(
    track: &Track,
    offset_years: f64,
    start_years: f64,
    until_years: f64,
) -> impl Iterator<Item = (f64, f64, f64)> + '_ {
    let hi = (until_years - offset_years).max(0.0);
    let n = f64::from(DECAY_PIECES);
    openings(track, offset_years, start_years, until_years).flat_map(move |(first, s, e)| {
        (0..DECAY_PIECES).filter_map(move |k| {
            let a = s + (e - s) * f64::from(k) / n;
            let b = if k + 1 == DECAY_PIECES {
                e
            } else {
                s + (e - s) * f64::from(k + 1) / n
            };
            let (a, b) = (a.max(first), b.min(hi));
            (b > a).then_some((first, a, b))
        })
    })
}

/// The core's largest mass, M☉, and radius, R☉, of a star on `track` placed at `offset_years`,
/// from the engine's age `start_years` to `until_years` ([`decay_reaches`], item 3), read where
/// each segment opens in the span ([`openings`]) and at `until_years`.
///
/// Read so, they may miss their largest between those ages, and no more is needed: the envelope
/// term k′₂ m R² holds k′₂ Mc R², which the star does not have, so I ≤ k′₂ m R² + Mc (k′₃ Rc² −
/// k′₂ R²), and the core adds nothing while Rc ≤ √(k′₂ ÷ k′₃) R ≈ 0.69 R, a giant's whole life.
#[must_use]
fn largest_core(
    track: &Track,
    offset_years: f64,
    start_years: f64,
    until_years: f64,
) -> (f64, f64) {
    let hi = (until_years - offset_years).max(0.0);
    let arrival = track.main_sequence_arrival().map_or(0.0, Years::value);
    openings(track, offset_years, start_years, until_years)
        .map(|(first, _, _)| first)
        .chain(core::iter::once(hi.max(arrival)))
        .filter_map(|age| track.own_structure_at(age))
        .fold((0.0_f64, 0.0_f64), |(mc, rc), st| {
            (
                mc.max(st.state.core_mass().value()),
                rc.max(st.core_radius.value()),
            )
        })
}

/// ∫ (`M_env` ÷ M) R³ dt, R☉³ yr, of a star on `track` placed at `offset_years`, from the engine's
/// age `start_years` to `until_years`, bounded above piece by piece (ruling p11-channels of
/// 2026-10-06, section 3.1, item 4; [`pieces`]).
///
/// On a piece the radius is at most [`Track::max_radius_until`] its end, and the envelope's share
/// `M_env` ÷ M at most its value at the piece's start on the main sequence, where HPT section
/// 7.2's `M_env,0` (1 − τ)^¼ falls with τ; elsewhere at most 1 (the pieces hold no remnant).
/// Before its own arrival the star is its zero-age self, as the engine carries it
/// ([`engine_track_age_years`](super::star::engine_track_age_years)).
#[must_use]
fn braking_integral(track: &Track, offset_years: f64, start_years: f64, until_years: f64) -> f64 {
    let lo = (start_years - offset_years).max(0.0);
    let hi = (until_years - offset_years).max(0.0);
    let arrival = track.main_sequence_arrival().map_or(0.0, Years::value);
    let radius = |b: f64| track.max_radius_until(Years::new(b.max(arrival))).value();
    let cube = |r: f64| r * r * r;
    let share_at = |age: f64| envelope_share(track.own_structure_at(age).as_ref());
    // The main sequence's segments: from its start to the first segment past it.
    let main_sequence = (
        track.main_sequence_start().unwrap_or(f64::INFINITY),
        track
            .main_sequence_end()
            .map_or(f64::INFINITY, Years::value),
    );
    let mut sum = 0.0;
    // Held at its zero-age self before its own arrival.
    if lo < arrival {
        sum += share_at(arrival) * cube(radius(arrival)) * (arrival.min(hi) - lo);
    }
    for (first, a, b) in pieces(track, offset_years, start_years, until_years) {
        let share = if first >= main_sequence.0 && first < main_sequence.1 {
            share_at(a)
        } else {
            1.0
        };
        if share > 0.0 {
            sum += share * cube(radius(b)) * (b - a);
        }
    }
    sum
}

/// `M_env` ÷ M of a star of structure `st` as [`braking_integral`] bounds it from a piece's start:
/// the structure's own on the main sequence, 0 for a remnant or nothing, 1 otherwise.
#[must_use]
fn envelope_share(st: Option<&Structure>) -> f64 {
    match st {
        Some(st) if st.state.phase() == Phase::MainSequence => {
            (st.envelope.mass / st.state.mass().value()).clamp(0.0, 1.0)
        }
        Some(st) if !st.state.phase().is_remnant() => 1.0,
        Some(_) | None => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::stellar::Composition;
    use crate::stellar::binary::BinaryParams;
    use crate::stellar::binary::evolve::LiveOrbit;
    use crate::stellar::binary::tests::pair;
    use crate::stellar::binary::timeline::Context;
    use crate::stellar::draws::StarDraws;
    use crate::stellar::sse::Track;
    use crate::units::SolarMasses;

    /// T4.b: with every sink of angular momentum off (winds, magnetic braking, gravitational
    /// radiation), tides only move it between the orbit and the spins, and the total is kept to
    /// 10⁻⁹ while the orbit circularises.
    #[test]
    fn angular_momentum_is_conserved_without_sinks() {
        let params = BinaryParams {
            gravitational_radiation: false,
            magnetic_braking: false,
            wind_angular_momentum: false,
            ..BinaryParams::GENERATOR
        };
        let input = pair(1.0, 0.8, 4.0, 0.4, 0.02).with_params(params);
        let until = 6.0e9;
        let members = [1.0, 0.8].map(|m| Member::Track {
            track: Arc::new(Track::to_age(
                SolarMasses::new(m),
                &Composition::SOLAR,
                &StarDraws::median(),
                Years::new(until),
            )),
            offset: 0.0,
        });
        let k = input.orbit();
        let a = k.semi_major_axis().value() / crate::units::consts::SOLAR_RADIUS_M;
        let orbit = LiveOrbit::new(
            0.0,
            a,
            k.eccentricity().value(),
            *k.orientation(),
            k.mean_anomaly_at_epoch(),
        );
        let start = super::super::evolve::arrival(&members, until);
        let mut engine = Engine::new(
            Arc::new(Context::of(&input)),
            start,
            until,
            members,
            orbit,
            None,
        );
        let total = |s: &Snapshot| s.j + s.spins[0] + s.spins[1];
        let first = engine.snapshot();
        let initial = total(&first);
        let mut steps = 0;
        while engine.age < until && steps < 5_000 {
            let start = engine.snapshot();
            let structures = engine.structures(&start).expect("an orbit");
            let rates = engine.rates(&start, &structures);
            let (dt, stop) = engine.step_limit(&start, &structures, &rates);
            let next = engine.step(&start, dt).expect("an orbit");
            assert!(
                (total(&next) / initial - 1.0).abs() < 1e-9,
                "at {} yr: {} against {initial}",
                next.age,
                total(&next)
            );
            engine.accept(&next);
            steps += 1;
            if matches!(stop, Some(Stop::Death(_))) {
                break;
            }
        }
        let e = engine.orbit.as_ref().map_or(1.0, |o| o.e);
        assert!(
            e < input.orbit().eccentricity().value(),
            "tides circularise the orbit: {e}"
        );
    }
}
