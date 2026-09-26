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
use crate::stellar::sse::Structure;
use crate::units::Years;
use crate::units::consts::{GM_SUN, SECONDS_PER_JULIAN_YEAR, SOLAR_RADIUS_M, SPEED_OF_LIGHT};

use super::evolve::{Engine, roche_lobe};
use super::star::{G, Kind, Member, moment_of_inertia, positive};

/// Peters's (1964) (32/5) G³ M☉³ ÷ (c⁵ R☉⁴) in yr⁻¹: the coefficient of BSE equations 48 and 49,
/// which BSE rounds to 8.315 × 10⁻¹⁰.
pub(super) const GRAVITATIONAL_WAVE_RATE: f64 = 32.0 / 5.0 * (GM_SUN * GM_SUN * GM_SUN)
    / (SPEED_OF_LIGHT * SPEED_OF_LIGHT * SPEED_OF_LIGHT * SPEED_OF_LIGHT * SPEED_OF_LIGHT)
    / (SOLAR_RADIUS_M * SOLAR_RADIUS_M * SOLAR_RADIUS_M * SOLAR_RADIUS_M)
    * SECONDS_PER_JULIAN_YEAR;

/// Magnetic braking's constant, M☉ R☉² yr⁻² with Ω in rad yr⁻¹ (BSE equation 50).
const MAGNETIC_BRAKING: f64 = 5.83e-16;

/// No magnetic braking for a fully convective star (BSE section 2.4, Rappaport et al. 1983), M☉.
const MAGNETIC_BRAKING_FLOOR: f64 = 0.35;

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
    /// The pair reached its age.
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
#[derive(Debug, Clone, Copy)]
pub(super) struct StepLimit {
    dt: f64,
    event: (f64, Stop),
}

impl StepLimit {
    /// A step that may run to `until`, where the pair stops.
    #[must_use]
    pub(super) const fn new(until: f64) -> Self {
        Self {
            dt: f64::INFINITY,
            event: (until, Stop::Until),
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
        if dt < self.event.0 {
            self.event = (dt, stop);
        }
    }

    /// The step's length and the event it lands on, if any.
    #[must_use]
    pub(super) fn resolve(self) -> (f64, Option<Stop>) {
        let (at, stop) = self.event;
        if at <= self.dt * (1.0 + 1e-6) {
            (at.max(0.0), Some(stop))
        } else {
            (self.dt, None)
        }
    }
}

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
                return event;
            }
            self.accept(&next);
            if let Some(stop) = stop {
                return stop;
            }
            // A star the binary carries whose core has grown into its whole mass has lost its
            // envelope.
            for (i, structure) in next_structures.iter().enumerate() {
                if let (Member::Shaped { .. }, Some(st)) = (&self.members[i], structure)
                    && st.state.envelope_mass().value() <= 0.0
                    && st.state.phase().is_living()
                {
                    return Stop::Stripped(i);
                }
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
                Member::Track { .. } | Member::Frozen { .. } | Member::Gone => {
                    masses[i] = self.members[i].mass_at(age);
                }
                Member::Shaped { .. } | Member::Cooling { .. } | Member::Remnant { .. } => {
                    masses[i] = (s.masses[i] + rates.mass[i] * dt).max(1e-9);
                }
                Member::MainSequence { .. } => {
                    masses[i] = (s.masses[i] + rates.mass[i] * dt).max(1e-9);
                    taus[i] = (s.taus[i] + rates.tau[i] * dt).min(1.0);
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
        let mut limits = StepLimit::new(self.until - s.age);
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
        limits.length(f64::INFINITY);
        let (dt, stop) = limits.resolve();
        if dt < floor && stop.is_none() {
            (floor.min(self.until - s.age), None)
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
    let f2 = 1.0 + e2 * (7.5 + e2 * (5.625 + e2 * 0.3125));
    let f3 = 1.0 + e2 * (3.75 + e2 * (1.875 + e2 * 0.078_125));
    let f4 = 1.0 + e2 * (1.5 + e2 * 0.125);
    let f5 = 1.0 + e2 * (3.0 + e2 * 0.375);
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
        let mut engine = Engine::new(Arc::new(Context::of(&input)), until, members, orbit, None);
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
            if matches!(stop, Some(Stop::Death(_) | Stop::Until)) {
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
