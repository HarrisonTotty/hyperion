//! The batch's passes as a bracketed fixed point (P14.T51.h; decision-p14-t51-batch §4), on the
//! record since P14.T54.a deleted the `p14-snapshot` lane's hold.
//!
//! T12's and T13's passes iterate Aₙ₊₁ = F(Aₙ), F(A) being the Bond albedo one pass at the albedo A
//! gives (P14.T13.c, P14.T51.b). Under the batch they run until the equilibrium temperature settles
//! to [`ATMOSPHERE_CONVERGENCE`] of itself, as P14.T51.b built them, for at most [`PLAIN_PASSES`]:
//! a body that settles keeps T51.b's bits. Three kinds of body do not settle under simple iteration
//! (P14.T51.b's sample, 20 of 954 at 23's stand-ins):
//!
//! - one whose F falls more steeply than the diagonal rises at its fixed point, a stable physical
//!   state that the iteration oscillates about: a temperate world's cloud availability,
//!   min(1, (1 − d) W ÷ `W_ref`), rises steeply with temperature through water's
//!   Clausius–Clapeyron slope, so F′ < −1 there;
//! - one whose F jumps across the diagonal, a change of state or cover (the airless threshold or a
//!   frost's edge crossed) with no self-consistent albedo among the model's discrete states;
//! - one whose F creeps toward the area mix's clamp too slowly to settle, a lossless frost whose
//!   fixed point is the clamp itself (a visible-only frost's until the area mix took P14.T49.i's
//!   bins, 2026-10-19; since then the frosts T49.i holds lossless).
//!
//! Such a body solves g(A) = F(A) − A by bisection instead, [`HALVINGS`] of them, a fixed count:
//!
//! - the bracket is the least and greatest albedo its passes read if g changes sign across them,
//!   else [0, the largest `f64` below 1], where g(0) ≥ 0 and g(1⁻) ≤ 0 always hold, since F maps
//!   into [0, 1) (the area mix's clamp, `LARGEST_BOND_ALBEDO`); a 2-cycle always brackets, since
//!   g(A₁) = A₂ − A₁ and g(A₂) = A₁ − A₂;
//! - each halving is one pass at the midpoint;
//! - the published pass is the bracket's end with the smaller |g|, ties to the lower albedo, and
//!   its |g| is its residual ([`Convergence::Bracketed`]). A residual above [`JUMP_RESIDUAL`] marks
//!   a jump across the diagonal, labelled ([`Convergence::is_jump`]) and listed, never hidden.
//!
//! The passes stop early, to the same bits, where the albedo a pass reads equals, bit for bit, the
//! one read two passes before: F is a function of the albedo alone, so the passes then repeat a
//! 2-cycle, and the least and greatest albedos read, and g at each, are those the plain passes
//! would end with (stated; the ruling's "a detected 2-cycle").
//!
//! # Under a lagged context (P14.T54.a)
//!
//! From 23 a pass also reads a context C ([`Context`]): what the figures of the previous pass's own
//! air give it (decision-pre-23-review item 35; decision-p14-t54a-passes §1). A pass at (A, C)
//! gives P, and the next pass reads A′, the albedo P gives, and C′ = K(P). The plain passes are
//! then Picard iteration of the joint map (A, C) ↦ (F(A, C), K(P(A, C))), whose fixed points are
//! the self-consistent states ([`solve_in_context`]; §1.2):
//!
//! - **The first context**, C₀, is the caller's labelled stand-in: the context K gives a body with
//!   no figures, so that such a body's passes are T51.h's, their count included.
//! - **Settled.** Pass n settles only where all three hold: T51.b's test on the equilibrium
//!   temperature against pass n − 1's; C(n − 1) ≡ C(n); and C(n) ≡ K(P(n)). Here ≡ is
//!   [`Context::agrees`]: every float member within [`CONTEXT_AGREEMENT`] of itself, a NaN never,
//!   and every other member bit for bit ([`floats_agree`]).
//! - **The 2-cycle stop** is on (A, C) two passes back, bit for bit: a pass is a function of
//!   (A, C) alone, so that is exactly where the passes repeat (§1.3).
//! - **The bracket** is T51.h's, under a frozen context `C_f`: the one read by whichever of the
//!   last two plain passes read the lower albedo (`total_cmp`; then the lower albedo given; then
//!   [`Context::rank`]), so that a 2-cycle publishes one pass whichever of its states the passes
//!   stop on (§1.4). The plain passes' least and greatest albedo are re-evaluated under it, an end
//!   that read `C_f` to the bit reused (the same bits), and the bracket falls back to [0, 1⁻]
//!   where they no longer bracket.
//! - **The second round** (§1.5): where the published pass's own context C* = K(P*) differs from
//!   `C_f` in the discrete key ([`Context::key`]), the bracket runs once more frozen at C*. Its
//!   pass is published where its own K agrees with C* in the key; otherwise the passes found no
//!   self-consistent context among the two, and the lower albedo of the two rounds' passes is
//!   published, labelled a context jump ([`ContextFit::Jump`]).
//!
//! So at most [`MOST_PASSES`] passes are taken, 24 + 2 × (2 + 32 + 1) = 94. [`solve`] is
//! [`solve_in_context`] at C = (), every context the same, which takes T51.h's passes to the bit.
//!
//! # The superlinear root (P14.T51.l, from generator version 25)
//!
//! From [`SECANT_ROOT_VERSION`] the bracket's [`HALVINGS`] become at most [`SECANT_STEPS`] steps
//! of the Illinois form of the false position (Dowell and Jarratt 1971, BIT 11, 168) on the same
//! bracket ([`PassRoot::Secant`]; decision-24-cost D4):
//!
//! - a bound of [0, 1⁻] that is an end is taken first, so that both ends hold their g;
//! - each step is the false position's point between the ends, an end kept by two steps in a row
//!   weighing half as much at the next; a step whose point does not lie strictly inside the
//!   bracket, or one taken after two steps that did not halve it, is the midpoint instead, and so is
//!   every step after two false positions in a row that did not halve the ends' least |g|, a jump
//!   across the diagonal, so that a jump still ends at the jump, in about the halvings' count;
//! - the steps stop where the bracket is [`SECANT_STOP_WIDTH`] wide, 2⁻³² in albedo, the width the
//!   halvings leave of [0, 1⁻], or where an end holds g = 0, a fixed point;
//! - a detected 2-cycle (A₁, A₂) is the bracket itself: g(A₁) = −g(A₂), so the first step is the
//!   cycle's midpoint, exact where F is linear;
//! - the published pass is the bracket's end with the smaller |g|, ties to the lower albedo, as
//!   the halvings' is.

use core::cmp::Ordering;
use core::fmt;

use crate::planetary::derive::atmosphere::LARGEST_BOND_ALBEDO;
use crate::planetary::derive::irradiation::BondAlbedo;
use crate::{GENERATOR_VERSION, GeneratorVersion};

#[cfg(doc)]
use crate::planetary::derive::ATMOSPHERE_CONVERGENCE;

/// The most plain passes the batch iterates before it brackets (P14.T51.h): 24, above the 19 that
/// the slowest converging body of P14.T51.b's sample took (decision-p14-t51-batch §4). A body that
/// settles within them keeps P14.T51.b's bits.
pub(crate) const PLAIN_PASSES: u32 = 24;

/// The halvings of a bracket (P14.T51.h): 32, a fixed count, so that the bracket ends 2⁻³² of its
/// first width wide, under 2.4 × 10⁻¹⁰ in albedo: 10⁻⁹ in the equilibrium temperature for an
/// albedo up to 0.94, since d`T_eq` ÷ `T_eq` = dA ÷ 4(1 − A).
pub(crate) const HALVINGS: u32 = 32;

/// The generator version from which a bracketed body's root is P14.T51.l's superlinear one: 25,
/// the first planetary bump after 24 (decision-24-cost D4; the [module](self)'s "The superlinear
/// root").
///
/// It is P14.T51.l's one hold. Below it [`PassRoot::in_force`] is [`PassRoot::Halvings`], T51.h's
/// [`HALVINGS`], and every pass is version 24's bit for bit. 25's bump deletes this hold and the
/// tests' switch, keeps [`PassRoot::Halvings`] as the tests' oracle of the secant steps, and
/// deletes the tripwire (`the_secant_root_is_held_only_until_the_25_bump`).
pub(crate) const SECANT_ROOT_VERSION: GeneratorVersion = GeneratorVersion::new(25);

/// The most secant steps a bracket takes under [`PassRoot::Secant`] (P14.T51.l), each a full pass:
/// 40, above the 33 the slowest of the 269 rounds over the 400 sample systems and the golden
/// systems took (measured 2026-10-30, P14.T51.l's lane; the system tests'
/// `under_the_secant_root_the_passes_and_movers_are_stated` prints the rounds and holds them under
/// the count). Its smooth roots stop in 6 to 15 steps, against the 32 halvings; its jumps across
/// the diagonal, which only halvings close in on, in 26 to 33.
pub(crate) const SECANT_STEPS: u32 = 40;

/// The bracket width at which the secant steps stop, in albedo: 2⁻³², the width [`HALVINGS`] leave
/// of [0, 1⁻], so the stop is T51.h's, 10⁻⁹ in the equilibrium temperature for an albedo up to
/// 0.94.
pub(crate) const SECANT_STOP_WIDTH: f64 = 1.0 / 4_294_967_296.0;

/// How a bracketed body's root is found ([`SECANT_ROOT_VERSION`]'s hold).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum PassRoot {
    /// Version 24's and before: T51.h's [`HALVINGS`] from the plain passes' least and greatest
    /// albedo.
    Halvings,
    /// Version 25's: at most [`SECANT_STEPS`] Illinois steps, a detected 2-cycle its own bracket
    /// (the [module](self)'s "The superlinear root").
    Secant,
}

impl PassRoot {
    /// The root this generator version gives: the secant steps from [`SECANT_ROOT_VERSION`], the
    /// halvings below it; in the tests, the root a test's switch sets on its thread
    /// ([`with_pass_root`]).
    #[must_use]
    pub(crate) fn in_force() -> Self {
        #[cfg(test)]
        if let Some(root) = ROOT.with(core::cell::Cell::get) {
            return root;
        }
        if GENERATOR_VERSION >= SECANT_ROOT_VERSION {
            Self::Secant
        } else {
            Self::Halvings
        }
    }
}

#[cfg(test)]
thread_local! {
    /// The root a test sets on its thread, `None` for the version's.
    static ROOT: core::cell::Cell<Option<PassRoot>> = const { core::cell::Cell::new(None) };
}

#[cfg(test)]
thread_local! {
    /// The secant steps each round took on this thread, while a test collects them
    /// ([`with_secant_rounds`]).
    static ROUNDS: core::cell::RefCell<Option<Vec<u32>>> = const { core::cell::RefCell::new(None) };
}

/// `run`'s result with the secant steps each bracket's round took on this thread, in order: what
/// [`SECANT_STEPS`]'s measurement reads. The thread's log is set back after.
#[cfg(test)]
pub(crate) fn with_secant_rounds<R>(run: impl FnOnce() -> R) -> (R, Vec<u32>) {
    let before = ROUNDS.with(|cell| cell.replace(Some(Vec::new())));
    let result = run();
    let rounds = ROUNDS.with(|cell| cell.replace(before)).unwrap_or_default();
    (result, rounds)
}

#[cfg(test)]
thread_local! {
    /// The ends each bracket round stopped at on this thread, while a test collects them
    /// ([`with_crossings`]).
    static CROSSINGS: core::cell::RefCell<Option<Vec<Crossing>>> =
        const { core::cell::RefCell::new(None) };
}

/// A bracket round's two ends as it stopped, the lower albedo first, each the albedo it read and
/// g = F(A, C) − A there: what P14.T51.l's pinned bodies are checked against (the system tests'
/// `the_secant_root_s_pinned_bodies_end_at_crossings`).
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Crossing {
    /// The lower-albedo end's albedo read and g.
    low: (f64, f64),
    /// The higher-albedo end's albedo read and g.
    high: (f64, f64),
}

#[cfg(test)]
impl Crossing {
    /// The albedo the round published: its end with the smaller |g|, ties to the lower, as
    /// [`bracket`] takes it.
    #[must_use]
    pub(crate) fn published(&self) -> f64 {
        if self.high.1.abs() < self.low.1.abs() {
            self.high.0
        } else {
            self.low.0
        }
    }

    /// Whether the round stopped at a crossing of the diagonal.
    ///
    /// Its ends' g lie on opposite sides of it (g ≥ 0 at one, g < 0 at the other), within
    /// [`SECANT_STOP_WIDTH`] of each other, or with its published end at g = 0 to the bit, an exact
    /// root itself. By comparison, so that a g that is not a number is no crossing.
    #[must_use]
    pub(crate) fn is_valid(&self) -> bool {
        let ((a, g_a), (b, g_b)) = (self.low, self.high);
        let sides = (g_a >= 0.0 && g_b < 0.0) || (g_a < 0.0 && g_b >= 0.0);
        let within = a < b && b - a <= SECANT_STOP_WIDTH;
        let g_published = if self.published().total_cmp(&a).is_eq() {
            g_a
        } else {
            g_b
        };
        // An exact zero is meant: the secant steps stop at one.
        sides && (within || g_published.abs() <= 0.0)
    }
}

/// `run`'s result with the ends each bracket round stopped at on this thread, in order.
///
/// The thread's log is set back after, a panic in `run` included.
#[cfg(test)]
pub(crate) fn with_crossings<R>(run: impl FnOnce() -> R) -> (R, Vec<Crossing>) {
    /// Sets the thread's log back as it drops.
    struct Restore(Option<Vec<Crossing>>);
    impl Drop for Restore {
        fn drop(&mut self) {
            CROSSINGS.with(|cell| cell.replace(self.0.take()));
        }
    }
    let restore = Restore(CROSSINGS.with(|cell| cell.replace(Some(Vec::new()))));
    let result = run();
    let crossings = CROSSINGS
        .with(core::cell::RefCell::take)
        .unwrap_or_default();
    drop(restore);
    (result, crossings)
}

/// `run`'s result with the bracketed root `root` on this thread, for the tests to reach 25's
/// secant steps ahead of the bump; the version's root is set back after, a panic in `run`
/// included. 25's bump deletes it with the hold.
#[cfg(test)]
pub(crate) fn with_pass_root<R>(root: PassRoot, run: impl FnOnce() -> R) -> R {
    /// Sets the thread's root back as it drops.
    struct Restore(Option<PassRoot>);
    impl Drop for Restore {
        fn drop(&mut self) {
            ROOT.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(ROOT.with(|cell| cell.replace(Some(root))));
    run()
}

/// The most passes a body takes under a lagged context (P14.T54.a; decision-p14-t54a-passes §1.5):
/// [`PLAIN_PASSES`], then two rounds of a bracket, each its two ends re-evaluated under its
/// context, its [`HALVINGS`] and the one pass more where a bound of [0, 1⁻] is still an end: 24 +
/// 2 × (2 + 32 + 1) = 94. At C = (), as [`solve`] runs, no end is re-evaluated and no second round
/// runs, so T51.h's [`PLAIN_PASSES`] + [`HALVINGS`] + 1 holds. Under [`PassRoot::Secant`] a round
/// takes its two ends, its two bounds and [`SECANT_STEPS`] at most ([`MOST_SECANT_PASSES`]).
pub(crate) const MOST_PASSES: u32 = PLAIN_PASSES + 2 * (2 + HALVINGS + 1);

/// The most passes a body takes under [`PassRoot::Secant`]: [`PLAIN_PASSES`], then two rounds,
/// each its two ends re-evaluated, its two bounds taken and [`SECANT_STEPS`].
pub(crate) const MOST_SECANT_PASSES: u32 = PLAIN_PASSES + 2 * (2 + 2 + SECANT_STEPS);

/// The residual |F(A) − A| in albedo above which a bracketed body's published pass is a jump across
/// the diagonal (P14.T51.h): 10⁻⁶, some four thousand times the bracket's width at the end of
/// [`HALVINGS`] from [0, 1), so that a continuous F of slope down to −4,000 there falls under it.
pub(crate) const JUMP_RESIDUAL: f64 = 1e-6;

/// How far apart, as a share of the larger, two float members of a context may lie and agree
/// ([`floats_agree`]): 10⁻⁹ (decision-p14-t54a-passes §1.2), the passes' own
/// [`ATMOSPHERE_CONVERGENCE`] on the equilibrium temperature.
pub(crate) const CONTEXT_AGREEMENT: f64 = 1e-9;

/// Whether the float members `a` and `b` of two contexts agree (≡; decision-p14-t54a-passes §1.2):
/// the same bits, or both finite with |a − b| ≤ [`CONTEXT_AGREEMENT`] max(|a|, |b|); never where
/// either is a NaN, so that a NaN never settles.
///
/// An infinity agrees only with itself, by the same-bits test: the lane's extension of the
/// ruling's formula, which as written refuses it, since in IEEE 754 |∞ − ∞| is a NaN and no
/// comparison with a NaN holds (the C1 review's DD3).
#[must_use]
pub(crate) fn floats_agree(a: f64, b: f64) -> bool {
    if a.is_nan() || b.is_nan() {
        return false;
    }
    a.total_cmp(&b).is_eq()
        || (a.is_finite()
            && b.is_finite()
            && (a - b).abs() <= CONTEXT_AGREEMENT * a.abs().max(b.abs()))
}

/// What a pass reads beside its albedo (P14.T54.a; decision-p14-t54a-passes §1): the figures of
/// the previous pass's air, held as plain values, as [`solve_in_context`] compares them.
pub(crate) trait Context: Copy {
    /// The discrete key (§1.5): every member that is not a float, and every branch that sets a
    /// float member, ordered by a derived `Ord`.
    type Key: Copy + Ord + fmt::Debug;

    /// The context's discrete key.
    #[must_use]
    fn key(&self) -> Self::Key;

    /// Whether the context agrees with `other` (≡, §1.2): every float member within
    /// [`CONTEXT_AGREEMENT`] of the other's ([`floats_agree`]), and every other member bit for bit.
    #[must_use]
    fn agrees(&self, other: &Self) -> bool;

    /// A total order on contexts, for the frozen context's last tie: the key's `Ord`, then each
    /// float member's `total_cmp` in a fixed order. `Equal` exactly where every ranked member, the
    /// key's and the floats', is the same bits, which the 2-cycle stop and the reuse of an end
    /// read: so a pass may read nothing of a context but its ranked members (the planetary
    /// context's ride-along first stage is read only through K(P*), by the record; Batch D's
    /// review, D2-2).
    ///
    /// `total_cmp` orders a NaN by its sign bit, which is not portable across targets (the
    /// sim-determinism skill), so a context holds no NaN: the planetary context asserts its floats
    /// finite where K builds it, in every build
    /// ([`PassContext::of`](crate::planetary::derive::pass_record::PassContext::of)).
    #[must_use]
    fn rank(&self, other: &Self) -> Ordering;
}

/// No context: T51.h's passes, a function of the albedo alone ([`solve`]).
impl Context for () {
    type Key = ();

    fn key(&self) -> Self::Key {}

    fn agrees(&self, (): &Self) -> bool {
        true
    }

    fn rank(&self, (): &Self) -> Ordering {
        Ordering::Equal
    }
}

/// How a body's passes ended (P14.T51.h; crate-private, for the tests and the bless note).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Convergence {
    /// The plain passes settled, within [`PLAIN_PASSES`].
    Converged,
    /// The passes did not settle, and bisection found the published pass.
    Bracketed {
        /// |F(A) − A| at the published pass, under the context it read, in albedo: under about
        /// 10⁻⁹ at a fixed point, and above [`JUMP_RESIDUAL`] at a jump across the diagonal.
        residual: f64,
        /// Whether the published pass's own context fits the one it read (P14.T54.a).
        fit: ContextFit,
    },
}

/// Whether a bracketed body's published pass fits its context (P14.T54.a;
/// decision-p14-t54a-passes §1.5): whether the context its own air gives, K(P*), has the discrete
/// key of the one it read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum ContextFit {
    /// The first round's: K(P*) has the key of the frozen context `C_f`. Every bracketed body at
    /// C = () ([`solve`]).
    FirstRound,
    /// The second round's, frozen at C* = K(P*): its pass's own K has C*'s key, a consistent pair
    /// that the first round's context missed.
    SecondRound,
    /// Neither round's: no self-consistent context among the two. The lower albedo of the two
    /// rounds' passes is published, labelled and listed.
    Jump,
}

impl Convergence {
    /// Whether the passes ended at a jump: bracketed with a residual above [`JUMP_RESIDUAL`], a
    /// jump across the diagonal (a state or cover change with no self-consistent albedo in the
    /// model's discrete states), or at a context jump ([`ContextFit::Jump`], decision-p14-t54a-passes
    /// §1.8).
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the convergence tests read it until P14.T54.a's re-check and bless note"
        )
    )]
    #[must_use]
    pub(crate) fn is_jump(self) -> bool {
        match self {
            Self::Bracketed { residual, fit } => {
                residual > JUMP_RESIDUAL || fit == ContextFit::Jump
            }
            Self::Converged => false,
        }
    }
}

/// What [`solve`] and [`solve_in_context`] leave: the albedo and the context the published pass
/// read, what that pass gave and the context its own air gives, how many passes were taken and how
/// they ended.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Solved<P, C = ()> {
    /// The albedo the published pass read.
    pub(crate) albedo: BondAlbedo,
    /// What the published pass gave.
    pub(crate) pass: P,
    /// The context the published pass read.
    pub(crate) context: C,
    /// The context the published pass gives, K(P): the one the next pass would read, and the
    /// record's figures'.
    pub(crate) next: C,
    /// How many passes were taken, the bisection's included.
    pub(crate) count: u32,
    /// How they ended.
    pub(crate) convergence: Convergence,
}

/// One pass as the solver reads it: the albedo and the context it read, what it gave and g =
/// F(A, C) − A there.
#[derive(Debug, Clone, Copy)]
struct Evaluated<P, C> {
    read: BondAlbedo,
    context: C,
    pass: P,
    g: f64,
}

impl<P, C: Context> Evaluated<P, C> {
    /// The pass `pass` at the albedo `read` under the context `context`, whose albedo `gives`
    /// reads.
    #[must_use]
    fn of(read: BondAlbedo, context: C, pass: P, gives: impl Fn(&P) -> BondAlbedo) -> Self {
        let g = gives(&pass).value() - read.value();
        Self {
            read,
            context,
            pass,
            g,
        }
    }

    /// Whether g is not negative here, the side of the diagonal the bisection keeps.
    #[must_use]
    fn rises(&self) -> bool {
        self.g >= 0.0
    }

    /// The canonical order of two passes (decision-p14-t54a-passes §1.4): the albedo read, by
    /// `total_cmp`; then the albedo given; then the context's [`Context::rank`].
    #[must_use]
    fn canonical(&self, other: &Self, gives: impl Fn(&P) -> BondAlbedo) -> Ordering {
        self.read
            .value()
            .total_cmp(&other.read.value())
            .then_with(|| {
                gives(&self.pass)
                    .value()
                    .total_cmp(&gives(&other.pass).value())
            })
            .then_with(|| self.context.rank(&other.context))
    }

    /// The lower of `self` and `other` in the canonical order, `self` where they tie.
    #[must_use]
    fn lower(self, other: Self, gives: impl Fn(&P) -> BondAlbedo) -> Self {
        if self.canonical(&other, gives) == Ordering::Greater {
            other
        } else {
            self
        }
    }
}

/// A bracket's end: a pass taken, or one of [0, 1⁻]'s ends, whose side of the diagonal is known
/// without one.
#[derive(Debug, Clone, Copy)]
enum End<P, C> {
    Taken(Evaluated<P, C>),
    Bound(BondAlbedo),
}

impl<P: Copy, C: Copy> End<P, C> {
    /// The albedo the end reads.
    #[must_use]
    fn read(&self) -> f64 {
        match self {
            Self::Taken(at) => at.read.value(),
            Self::Bound(read) => read.value(),
        }
    }
}

/// The batch's passes from the albedo `start` (P14.T51.b, P14.T51.h): `pass` takes one pass at an
/// albedo, `gives` reads the albedo a pass gives, and `settled` whether a pass has settled against
/// the one before it.
///
/// The plain passes run as P14.T51.b built them, for at most [`PLAIN_PASSES`], and a body that
/// settles publishes its last pass, [`Convergence::Converged`]. One that does not, or that repeats
/// a 2-cycle to the bit first, is bracketed and bisected ([`HALVINGS`]), as the module's
/// documentation says, and publishes the bracket's end with the smaller |F(A) − A|, ties to the
/// lower albedo, [`Convergence::Bracketed`]. At most [`PLAIN_PASSES`] + [`HALVINGS`] + 1 passes are
/// taken: the one more where a bound of [0, 1⁻] is still an end and needs its pass. Under the
/// secant steps from 25 ([`PassRoot::Secant`]), at most [`PLAIN_PASSES`] + 2 + [`SECANT_STEPS`].
///
/// It is [`solve_in_context`] at C = (), to the bit, its count included: every end then read the
/// frozen context and is reused, and every published pass fits its context
/// ([`ContextFit::FirstRound`]).
#[must_use]
pub(crate) fn solve<P: Copy>(
    start: BondAlbedo,
    pass: impl Fn(BondAlbedo) -> P,
    gives: impl Fn(&P) -> BondAlbedo + Copy,
    settled: impl Fn(&P, &P) -> bool,
) -> Solved<P> {
    solve_in_context(
        start,
        (),
        |albedo: BondAlbedo, (): &()| pass(albedo),
        gives,
        |_: &P| {},
        settled,
    )
}

/// The batch's passes from the albedo `start` under the first context `first`, C₀ (P14.T54.a;
/// decision-p14-t54a-passes §1; the [module](self)'s "Under a lagged context"): `pass` takes one
/// pass at an albedo under a context, `gives` reads the albedo a pass gives, `next` the context
/// its own air gives, K(P), and `settled` whether a pass has settled against the one before it by
/// T51.b's test.
///
/// Pass n reads Aₙ, the albedo pass n − 1 gave, and Cₙ = K(Pₙ₋₁), pass 1 the stand-in `first`.
/// It settles where `settled` holds, Cₙ₋₁ ≡ Cₙ and Cₙ ≡ K(Pₙ) ([`Context::agrees`]), and the
/// plain passes stop early on (A, C) two passes back, bit for bit. A body that does not settle is
/// bracketed under its frozen context, and once more under K of the published pass where their
/// keys differ, as the module's documentation says. At most [`MOST_PASSES`] passes are taken,
/// [`MOST_SECANT_PASSES`] under the secant steps from 25 ([`PassRoot::Secant`]).
///
/// Each pass must be a function of the albedo and the context it reads alone, which the early stop
/// and the bisection both need, so `pass` is a `Fn`.
#[must_use]
pub(crate) fn solve_in_context<C: Context, P: Copy>(
    start: BondAlbedo,
    first: C,
    pass: impl Fn(BondAlbedo, &C) -> P,
    gives: impl Fn(&P) -> BondAlbedo + Copy,
    next: impl Fn(&P) -> C,
    settled: impl Fn(&P, &P) -> bool,
) -> Solved<P, C> {
    solve_with(Plain::SOLVE, start, first, pass, gives, next, settled)
}

/// What the plain passes do on a 2-cycle found to the bit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TwoCycles {
    /// Stop, and bracket at once: [`solve`]'s rule.
    Stop,
    /// Run on to the cap: the reference the early stop is tested against.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "the early stop's test compares the two")
    )]
    Run,
}

/// How the plain passes run: what they do on a 2-cycle, and how many they take at most.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Plain {
    cycles: TwoCycles,
    cap: u32,
}

impl Plain {
    /// [`solve`]'s and [`solve_in_context`]'s: stop on a 2-cycle, at most [`PLAIN_PASSES`].
    const SOLVE: Self = Self {
        cycles: TwoCycles::Stop,
        cap: PLAIN_PASSES,
    };
}

/// [`solve_in_context`], whose plain passes run as `plain` says: the tests run them on past a
/// 2-cycle, and to another cap.
#[must_use]
fn solve_with<C: Context, P: Copy>(
    plain: Plain,
    start: BondAlbedo,
    first: C,
    pass: impl Fn(BondAlbedo, &C) -> P,
    gives: impl Fn(&P) -> BondAlbedo + Copy,
    next: impl Fn(&P) -> C,
    settled: impl Fn(&P, &P) -> bool,
) -> Solved<P, C> {
    let evaluate =
        |read: BondAlbedo, context: C| Evaluated::of(read, context, pass(read, &context), gives);
    let mut last = evaluate(start, first);
    let mut last_next = next(&last.pass);
    let mut count = 1;
    let (mut lowest, mut highest) = (last, last);
    let mut previous: Option<Evaluated<P, C>> = None;
    let mut cycled = false;
    while count < plain.cap {
        let now = evaluate(gives(&last.pass), last_next);
        let now_next = next(&now.pass);
        count += 1;
        let done = settled(&last.pass, &now.pass)
            && last.context.agrees(&now.context)
            && now.context.agrees(&now_next);
        let repeats = previous.is_some_and(|two_back| {
            two_back.read.value().total_cmp(&now.read.value()).is_eq()
                && two_back.context.rank(&now.context).is_eq()
        });
        previous = Some(last);
        (last, last_next) = (now, now_next);
        if now.read.value() < lowest.read.value() {
            lowest = now;
        }
        if now.read.value() > highest.read.value() {
            highest = now;
        }
        if done {
            return Solved {
                albedo: now.read,
                pass: now.pass,
                context: now.context,
                next: now_next,
                count,
                convergence: Convergence::Converged,
            };
        }
        if repeats && plain.cycles == TwoCycles::Stop {
            cycled = true;
            break;
        }
    }
    // The frozen context: the one read by the lower-albedo pass of the last two (§1.4).
    let frozen = previous.map_or(last, |previous| previous.lower(last, gives));
    let root = PassRoot::in_force();
    // The plain passes' least and greatest albedo; under the secant steps a detected 2-cycle's
    // two passes, lower albedo first (P14.T51.l).
    let ends = match (root, cycled, previous) {
        (PassRoot::Secant, true, Some(previous)) => {
            if previous.read.value() < last.read.value() {
                (previous, last)
            } else {
                (last, previous)
            }
        }
        (PassRoot::Halvings | PassRoot::Secant, ..) => (lowest, highest),
    };
    let first_round = bracket(frozen.context, ends, (root, &evaluate), &mut count);
    let first_next = next(&first_round.pass);
    let (published, published_next, fit) = if first_next.key() == frozen.context.key() {
        (first_round, first_next, ContextFit::FirstRound)
    } else {
        // The second round, frozen at the context the published pass's own air gives (§1.5).
        let second_round = bracket(first_next, ends, (root, &evaluate), &mut count);
        let second_next = next(&second_round.pass);
        if second_next.key() == first_next.key() {
            (second_round, second_next, ContextFit::SecondRound)
        } else if first_round.canonical(&second_round, gives) == Ordering::Greater {
            (second_round, second_next, ContextFit::Jump)
        } else {
            (first_round, first_next, ContextFit::Jump)
        }
    };
    let cap = match root {
        PassRoot::Halvings => MOST_PASSES,
        PassRoot::Secant => MOST_SECANT_PASSES,
    };
    debug_assert!(
        count <= cap,
        "{count} passes, above {root:?}'s cap of {cap}"
    );
    Solved {
        albedo: published.read,
        pass: published.pass,
        context: published.context,
        next: published_next,
        count,
        convergence: Convergence::Bracketed {
            residual: published.g.abs(),
            fit,
        },
    }
}

/// One round of T51.h's bracket under the context `frozen` (decision-p14-t54a-passes §1.4): its
/// ends `(lowest, highest)`, the plain passes' least and greatest albedo, re-evaluated under it,
/// each that read it to the bit reused, the bracket [0, 1⁻] where they no longer bracket, then the
/// root `root` gives, each step one pass of `evaluate` ([`HALVINGS`] at the midpoint, or the
/// secant steps, [`secant_steps`]), and the end with the smaller |g|, ties to the lower albedo.
/// Every pass taken counts on `count`.
#[must_use]
fn bracket<P: Copy, C: Context>(
    frozen: C,
    (lowest, highest): (Evaluated<P, C>, Evaluated<P, C>),
    (root, evaluate): (PassRoot, &impl Fn(BondAlbedo, C) -> Evaluated<P, C>),
    count: &mut u32,
) -> Evaluated<P, C> {
    let mut under = |end: Evaluated<P, C>| {
        if end.context.rank(&frozen).is_eq() && ends_reused() {
            end
        } else {
            *count += 1;
            evaluate(end.read, frozen)
        }
    };
    let (lowest, highest) = (under(lowest), under(highest));
    let (mut lo, mut hi) = if lowest.rises() == highest.rises() {
        (
            End::Bound(BondAlbedo::from_fraction(0.0)),
            End::Bound(BondAlbedo::from_fraction(LARGEST_BOND_ALBEDO)),
        )
    } else {
        (End::Taken(lowest), End::Taken(highest))
    };
    // The side of the diagonal the lower end keeps: g(0) = F(0) ≥ 0 for the bound.
    let lower_rises = match lo {
        End::Taken(at) => at.rises(),
        End::Bound(_) => true,
    };
    match root {
        PassRoot::Halvings => {
            for _ in 0..HALVINGS {
                let (a, b) = (lo.read(), hi.read());
                let mid = a.midpoint(b);
                if !(a < mid && mid < b) {
                    break;
                }
                let at = evaluate(BondAlbedo::from_fraction(mid), frozen);
                *count += 1;
                if at.rises() == lower_rises {
                    lo = End::Taken(at);
                } else {
                    hi = End::Taken(at);
                }
            }
        }
        PassRoot::Secant => {
            let mut take = |end: End<P, C>| match end {
                End::Taken(at) => at,
                End::Bound(read) => {
                    *count += 1;
                    evaluate(read, frozen)
                }
            };
            let ends = (take(lo), take(hi));
            let (low, high) = secant_steps(ends, lower_rises, |read| {
                *count += 1;
                evaluate(BondAlbedo::from_fraction(read), frozen)
            });
            (lo, hi) = (End::Taken(low), End::Taken(high));
        }
    }
    let mut taken = |end: End<P, C>| match end {
        End::Taken(at) => at,
        End::Bound(read) => {
            *count += 1;
            evaluate(read, frozen)
        }
    };
    let (lo, hi) = (taken(lo), taken(hi));
    #[cfg(test)]
    CROSSINGS.with(|cell| {
        if let Some(crossings) = cell.borrow_mut().as_mut() {
            crossings.push(Crossing {
                low: (lo.read.value(), lo.g),
                high: (hi.read.value(), hi.g),
            });
        }
    });
    if hi.g.abs() < lo.g.abs() { hi } else { lo }
}

/// P14.T51.l's secant steps on the bracket `(low, high)`, its lower-albedo end first, whose lower end
/// lies on the side of the diagonal `lower_rises` says (g ≥ 0 where it is true), each step one pass
/// of `evaluate` at an albedo strictly inside the bracket: the bracket's ends when they stop (the
/// [module](self)'s "The superlinear root").
///
/// Each step is the Illinois false position, a − g′(a) (b − a) ÷ (g′(b) − g′(a)), where g′ is an
/// end's g, halved each time the other end is replaced twice in a row; the midpoint where that
/// point does not lie strictly inside the bracket, where the two steps before did not halve the
/// bracket, and at every step after two false positions in a row whose |g| was more than half the
/// ends' least, a jump across the diagonal, at which g does not fall and only halvings close in. They stop
/// after [`SECANT_STEPS`], where the bracket is [`SECANT_STOP_WIDTH`] wide or one ulp, or where an
/// end holds g = 0.
#[must_use]
fn secant_steps<P: Copy, C: Context>(
    (mut low, mut high): (Evaluated<P, C>, Evaluated<P, C>),
    lower_rises: bool,
    mut evaluate: impl FnMut(f64) -> Evaluated<P, C>,
) -> (Evaluated<P, C>, Evaluated<P, C>) {
    // The ends' weighed g, and which end the last step replaced (the lower where true).
    let (mut g_low, mut g_high) = (low.g, high.g);
    let mut replaced: Option<bool> = None;
    // The bracket's width before each of the last two steps, the older first.
    let mut widths = [f64::INFINITY; 2];
    // How many false positions in a row failed to halve the ends' least |g|; two mark a jump across
    // the diagonal, whose g does not fall toward it, which the halvings then end at.
    let mut stalled = 0_u32;
    #[cfg(test)]
    let mut taken = 0_u32;
    for _ in 0..SECANT_STEPS {
        let (a, b) = (low.read.value(), high.read.value());
        let width = b - a;
        // By comparison, so that a g that is not a number ends the steps too.
        if !(width > SECANT_STOP_WIDTH && low.g.abs() > 0.0 && high.g.abs() > 0.0) {
            break;
        }
        let halving = stalled >= 2 || width > 0.5 * widths[0];
        let false_position = a - g_low * (b - a) / (g_high - g_low);
        let read = if !halving && a < false_position && false_position < b {
            false_position
        } else {
            a.midpoint(b)
        };
        if !(a < read && read < b) {
            break;
        }
        let least = low.g.abs().min(high.g.abs());
        let at = evaluate(read);
        #[cfg(test)]
        {
            taken += 1;
        }
        widths = [widths[1], width];
        // By comparison, so that a g that is not a number is a jump.
        let fell = at.g.abs() <= 0.5 * least;
        if !halving {
            stalled = if fell { 0 } else { stalled + 1 };
        }
        let lower = at.rises() == lower_rises;
        if lower {
            (low, g_low) = (at, at.g);
            if replaced == Some(true) {
                g_high *= 0.5;
            }
        } else {
            (high, g_high) = (at, at.g);
            if replaced == Some(false) {
                g_low *= 0.5;
            }
        }
        replaced = Some(lower);
    }
    #[cfg(test)]
    ROUNDS.with(|cell| {
        if let Some(rounds) = cell.borrow_mut().as_mut() {
            rounds.push(taken);
        }
    });
    (low, high)
}

/// Whether the bracket reuses an end that read its frozen context to the bit: always in the
/// generator, and for the tests' switch off ([`with_ends_evaluated`]) never, which takes the same
/// bits at up to two passes more a round (the C1 review's DD2).
#[must_use]
fn ends_reused() -> bool {
    #[cfg(test)]
    {
        !ENDS_EVALUATED.with(core::cell::Cell::get)
    }
    #[cfg(not(test))]
    {
        true
    }
}

#[cfg(test)]
thread_local! {
    /// Whether the bracket re-evaluates every end on this thread, a test's own.
    static ENDS_EVALUATED: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
}

/// `run`'s result with every bracket's ends re-evaluated under its frozen context, none reused: a
/// test-only switch, set back after (the C1 review's DD2).
#[cfg(test)]
#[must_use]
pub(crate) fn with_ends_evaluated<R>(run: impl FnOnce() -> R) -> R {
    let before = ENDS_EVALUATED.with(|cell| cell.replace(true));
    let result = run();
    ENDS_EVALUATED.with(|cell| cell.set(before));
    result
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;

    /// A stub pass: the albedo `f` gives at the albedo read, held into [0, 1) as the area mix holds
    /// it, with the albedo read beside it.
    fn stub(f: impl Fn(f64) -> f64) -> impl Fn(BondAlbedo) -> (f64, f64) {
        move |read| {
            let gives = f(read.value()).clamp(0.0, LARGEST_BOND_ALBEDO);
            (read.value(), gives)
        }
    }

    /// The stub's albedo given.
    fn gives(pass: &(f64, f64)) -> BondAlbedo {
        BondAlbedo::from_fraction(pass.1)
    }

    /// P14.T51.b's rule on the stub: the albedo given within 10⁻⁹ of the albedo read.
    fn settled(last: &(f64, f64), now: &(f64, f64)) -> bool {
        (now.1 - last.1).abs() <= 1e-9 * now.1
    }

    /// The residual of a bracketed outcome, to compare by its bits.
    fn residual_of(convergence: Convergence) -> f64 {
        let Convergence::Bracketed { residual, .. } = convergence else {
            panic!("{convergence:?}");
        };
        residual
    }

    /// The context fit of a bracketed outcome.
    fn fit_of(convergence: Convergence) -> ContextFit {
        let Convergence::Bracketed { fit, .. } = convergence else {
            panic!("{convergence:?}");
        };
        fit
    }

    /// Whether two outcomes are the same, a bracketed one's residual by its bits.
    fn assert_same_outcome(one: Convergence, two: Convergence) {
        match (one, two) {
            (
                Convergence::Bracketed { residual, fit },
                Convergence::Bracketed {
                    residual: other,
                    fit: other_fit,
                },
            ) => {
                assert_same_bits(residual, other);
                assert_eq!(fit, other_fit);
            }
            _ => assert_eq!(one, two),
        }
    }

    /// The solver from P14.T13.c's 0.3.
    fn solved(f: impl Fn(f64) -> f64) -> Solved<(f64, f64)> {
        solve(BondAlbedo::BEFORE_ATMOSPHERES, stub(f), gives, settled)
    }

    /// A map that contracts fast settles as the plain passes do, and publishes their last pass;
    /// one that contracts too slowly to settle within them (slope 0.5, 29 passes from 0.3) is
    /// bracketed, and reaches its fixed point all the same.
    #[test]
    fn a_contracting_map_settles_by_its_plain_passes() {
        let fast = solved(|a| 0.2 + 0.1 * (a - 0.2));
        assert_eq!(fast.convergence, Convergence::Converged);
        assert!(fast.count <= PLAIN_PASSES, "{}", fast.count);
        assert!(
            (fast.albedo.value() - 0.2).abs() < 1e-9,
            "{:?}",
            fast.albedo
        );
        let slow = solved(|a| 0.2 + 0.5 * (a - 0.2));
        let Convergence::Bracketed { residual, .. } = slow.convergence else {
            panic!("{:?}", slow.convergence);
        };
        assert!(
            (slow.albedo.value() - 0.2).abs() < 1e-9,
            "{:?}",
            slow.albedo
        );
        assert!(residual < JUMP_RESIDUAL, "{residual:e}");
    }

    /// P14.T51.b's loop as it was built, on the stub: from 0.3, at most 64 passes, each reading the
    /// albedo the last gave, until one settles; the albedo the last pass read, what it gave and the
    /// count.
    fn t51b_loop(f: impl Fn(f64) -> f64) -> (f64, (f64, f64), u32) {
        let pass = stub(f);
        let mut albedo = BondAlbedo::BEFORE_ATMOSPHERES;
        let mut out = pass(albedo);
        let mut count = 1;
        while count < 64 {
            let next = gives(&out);
            let now = pass(next);
            count += 1;
            let done = settled(&out, &now);
            (albedo, out) = (next, now);
            if done {
                break;
            }
        }
        (albedo.value(), out, count)
    }

    /// The 361 stubs of P14.T51.h: linear maps of slopes from −0.9 to 0.9 about fixed points from
    /// 0.05 to 0.95, contracting and oscillating-but-converging, each as (fixed point, slope).
    fn t51h_stubs() -> impl Iterator<Item = (f64, f64)> {
        (0..19).flat_map(|k| {
            (0..19).map(move |j| (0.05 + 0.05 * f64::from(k), -0.9 + 0.1 * f64::from(j)))
        })
    }

    /// A body that settles within the plain passes keeps P14.T51.b's bits: over contracting and
    /// oscillating-but-converging maps of slopes from −0.9 to 0.9 about fixed points from 0.05 to
    /// 0.95, every one that P14.T51.b's loop settled within [`PLAIN_PASSES`] gives the same albedo,
    /// pass and count.
    #[test]
    fn on_stubs_the_solver_settles_as_t51b_s_loop() {
        let mut settled_within = 0;
        for (fixed, slope) in t51h_stubs() {
            let f = move |a: f64| fixed + slope * (a - fixed);
            let (albedo, out, count) = t51b_loop(f);
            if count > PLAIN_PASSES {
                continue;
            }
            let solved = solved(f);
            assert_eq!(
                solved.convergence,
                Convergence::Converged,
                "{fixed} {slope}"
            );
            assert_same_bits(solved.albedo.value(), albedo);
            assert_same_bits(solved.pass.0, out.0);
            assert_same_bits(solved.pass.1, out.1);
            assert_eq!(solved.count, count, "{fixed} {slope}");
            settled_within += 1;
        }
        assert!(settled_within > 100, "{settled_within}");
    }

    /// A stub F falling with slope −2 through its fixed point, held into a band from 10⁻³ below it
    /// to 1.5 × 10⁻³ above, as a cloud's availability is held at 1: simple iteration settles into a
    /// 2-cycle at the band's edges, and bisection over it reaches the fixed point within 10⁻¹² (the
    /// bracket's 0.0025 × 2⁻³², 5.8 × 10⁻¹³), its residual three times that at most.
    #[test]
    fn a_map_falling_with_slope_minus_2_reaches_its_fixed_point() {
        let fixed = 0.300_5;
        let solved = solved(|a| (fixed - 2.0 * (a - fixed)).clamp(fixed - 1e-3, fixed + 1.5e-3));
        let Convergence::Bracketed { residual, .. } = solved.convergence else {
            panic!("{:?}", solved.convergence);
        };
        let off = (solved.albedo.value() - fixed).abs();
        assert!(off < 1e-12, "{off:e}");
        assert!(residual <= 3.0 * off + 1e-15, "{residual:e} at {off:e}");
        assert!(!solved.convergence.is_jump());
        // The 2-cycle is found to the bit, so the plain passes stop early.
        assert!(solved.count < PLAIN_PASSES + HALVINGS, "{}", solved.count);
        eprintln!(
            "slope −2: {} passes, {off:e} from the fixed point, residual {residual:e}",
            solved.count
        );
    }

    /// Unbounded, the same slope drives the passes to the clamps; the bracket is then their least
    /// and greatest albedo, and bisection still reaches the fixed point within the bracket's width
    /// at its end, 2⁻³² of its first.
    #[test]
    fn a_map_falling_with_slope_minus_2_to_the_clamps_is_still_bracketed() {
        let fixed = 0.25;
        let solved = solved(|a| fixed - 2.0 * (a - fixed));
        let Convergence::Bracketed { residual, .. } = solved.convergence else {
            panic!("{:?}", solved.convergence);
        };
        let off = (solved.albedo.value() - fixed).abs();
        assert!(off < 1.0 / 4_294_967_296.0, "{off:e}");
        assert!(residual < JUMP_RESIDUAL, "{residual:e}");
    }

    /// A stub with a jump across the diagonal at 0.4 (0.6 below it and 0.2 from it) has no fixed
    /// point: the passes 2-cycle between 0.6 and 0.2, and bisection ends at the jump, its residual
    /// about 0.2, stated, and labelled a jump.
    #[test]
    fn a_map_that_jumps_across_the_diagonal_ends_at_the_jump() {
        let solved = solved(|a| if a < 0.4 { 0.6 } else { 0.2 });
        let Convergence::Bracketed { residual, .. } = solved.convergence else {
            panic!("{:?}", solved.convergence);
        };
        assert!(
            (solved.albedo.value() - 0.4).abs() < 1e-9,
            "{:?}",
            solved.albedo
        );
        assert!((residual - 0.2).abs() < 1e-9, "{residual}");
        assert!(solved.convergence.is_jump());
        eprintln!(
            "a jump at 0.4: published {:?}, residual {residual}",
            solved.albedo
        );
    }

    /// Where g keeps one sign over every albedo the passes read, the bracket is [0, 1⁻]: F(A) =
    /// A + 10⁻³ (1 − A) creeps up without settling, g > 0 at every albedo read, and the bracket's
    /// upper bound is the clamp's fixed point, F(1⁻) = 1⁻, which takes the one pass more.
    #[test]
    fn without_a_sign_change_the_bracket_is_the_whole_range() {
        let creeping = solved(|a| a + 1e-3 * (1.0 - a));
        let Convergence::Bracketed { residual, .. } = creeping.convergence else {
            panic!("{:?}", creeping.convergence);
        };
        assert_same_bits(creeping.albedo.value(), LARGEST_BOND_ALBEDO);
        assert_same_bits(residual, 0.0);
        assert_eq!(creeping.count, PLAIN_PASSES + HALVINGS + 1);
    }

    /// A map whose only fixed point is 0, F(A) = 0.999 A, falls too slowly to settle with g < 0 at
    /// every albedo read: the bracket is [0, 1⁻], its lower bound is kept through every halving
    /// and taken at the end, the one pass more, and the published albedo is 0 exactly.
    #[test]
    fn a_kept_lower_bound_is_taken_and_published() {
        let falling = solved(|a| 0.999 * a);
        assert_same_bits(falling.albedo.value(), 0.0);
        assert_same_bits(residual_of(falling.convergence), 0.0);
        assert_eq!(falling.count, PLAIN_PASSES + HALVINGS + 1);
    }

    /// Ties in |g| go to the lower albedo: from 0.25, F(A) = A + 0.25 below 0.5 and A − 0.25 from
    /// it 2-cycles over {0.25, 0.5}, and the bracket's ends both hold |g| = 0.25 exactly.
    #[test]
    fn ties_go_to_the_lower_albedo() {
        let f = |a: f64| if a < 0.5 { a + 0.25 } else { a - 0.25 };
        let solved = solve(BondAlbedo::from_fraction(0.25), stub(f), gives, settled);
        assert_same_bits(solved.albedo.value(), 0.5 - 1.0 / 17_179_869_184.0);
        assert_same_bits(residual_of(solved.convergence), 0.25);
    }

    /// Two runs agree to the bit.
    #[test]
    fn the_solver_is_deterministic() {
        let f = |a: f64| (0.21 - 2.5 * (a - 0.21)).clamp(0.15, 0.278);
        let (one, two) = (solved(f), solved(f));
        assert_same_bits(one.albedo.value(), two.albedo.value());
        assert_same_bits(one.pass.1, two.pass.1);
        assert_eq!(one.count, two.count);
        assert_same_outcome(one.convergence, two.convergence);
    }

    /// The maps of the 2-cycle tests, each a 2-cycle found to the bit: the jump at 0.4, and the
    /// slopes −2 and −2.5 held into bands.
    const CYCLING: [fn(f64) -> f64; 3] = [
        |a| if a < 0.4 { 0.6 } else { 0.2 },
        |a| (0.300_5 - 2.0 * (a - 0.300_5)).clamp(0.299_5, 0.302),
        |a| (0.21 - 2.5 * (a - 0.21)).clamp(0.15, 0.278),
    ];

    /// The plain passes' rule of the early stop's tests: `cycles` on a 2-cycle, at most `cap`.
    fn plain(cycles: TwoCycles, cap: u32) -> Plain {
        Plain { cycles, cap }
    }

    /// A 2-cycle found to the bit gives the same published pass as the plain passes run to their
    /// end, at either parity of the cap, 24 or 25: the early stop saves passes and moves nothing.
    /// With a context, the frozen one is the lower-albedo pass's of the last two, so an (A, C)
    /// 2-cycle publishes one pass to the bit too (decision-p14-t54a-passes §1.4): the ruling's stub,
    /// F(A, C) = 0.25 − 1.6 (A − 0.25) + 0.002 C, its context C = 1 where the albedo read is above
    /// 0.25 and 0 otherwise, 2-cycles over (0, 1) and (0.652, 0), and publishes its root under
    /// C = 1, A = 0.652 ÷ 2.6, which its own air keeps (K = 1), where freezing at the last pass's
    /// context would publish 0.25 under C = 0 at one parity and 0.2508 at the other.
    #[test]
    fn stopping_at_a_detected_2_cycle_moves_nothing() {
        for f in CYCLING {
            let at = |plain| {
                solve_with(
                    plain,
                    BondAlbedo::BEFORE_ATMOSPHERES,
                    (),
                    |read: BondAlbedo, (): &()| stub(f)(read),
                    gives,
                    |_: &(f64, f64)| {},
                    settled,
                )
            };
            let early = at(Plain::SOLVE);
            for late in [
                at(plain(TwoCycles::Run, PLAIN_PASSES)),
                at(plain(TwoCycles::Stop, PLAIN_PASSES + 1)),
                at(plain(TwoCycles::Run, PLAIN_PASSES + 1)),
            ] {
                assert_same_bits(early.albedo.value(), late.albedo.value());
                assert_same_bits(early.pass.1, late.pass.1);
                assert_same_outcome(early.convergence, late.convergence);
            }
            assert!(
                early.count < at(plain(TwoCycles::Run, PLAIN_PASSES)).count,
                "{}",
                early.count
            );
        }
        let at = |plain| {
            lid_solve(
                plain,
                |a, lid| 0.25 - 1.6 * (a - 0.25) + 0.002 * f64::from(lid.branch),
                |read| Lid::branch(u8::from(read > 0.25)),
            )
        };
        let early = at(Plain::SOLVE);
        for late in [
            at(plain(TwoCycles::Run, PLAIN_PASSES)),
            at(plain(TwoCycles::Stop, PLAIN_PASSES + 1)),
            at(plain(TwoCycles::Run, PLAIN_PASSES + 1)),
        ] {
            assert_same_lid_outcome(&early, &late);
        }
        assert_eq!(early.context, Lid::branch(1));
        assert_eq!(early.next, Lid::branch(1));
        assert_eq!(fit_of(early.convergence), ContextFit::FirstRound);
        assert!(
            (early.albedo.value() - 0.652 / 2.6).abs() < 1e-9,
            "{:?}",
            early.albedo
        );
        assert!(residual_of(early.convergence) < 1e-9);
        assert!(
            early.count < at(plain(TwoCycles::Run, PLAIN_PASSES)).count,
            "{}",
            early.count
        );
    }

    /// P14.T51.h's solver as it was built, before P14.T54.a's context (the oracle of
    /// [`at_no_context_the_solver_is_t51h_s_to_the_bit`]): the albedo the published pass read, its
    /// pass, the count and the residual where it bracketed.
    fn t51h_solve(
        cycles: TwoCycles,
        start: BondAlbedo,
        pass: impl Fn(BondAlbedo) -> (f64, f64),
        settled: impl Fn(&(f64, f64), &(f64, f64)) -> bool,
    ) -> (BondAlbedo, (f64, f64), u32, Option<f64>) {
        #[derive(Clone, Copy)]
        struct At {
            read: BondAlbedo,
            pass: (f64, f64),
            g: f64,
        }
        let of = |read: BondAlbedo| {
            let pass = pass(read);
            At {
                read,
                pass,
                g: gives(&pass).value() - read.value(),
            }
        };
        let mut last = of(start);
        let mut count = 1;
        let (mut lowest, mut highest) = (last, last);
        let mut before: Option<BondAlbedo> = None;
        while count < PLAIN_PASSES {
            let now = of(gives(&last.pass));
            count += 1;
            let done = settled(&last.pass, &now.pass);
            let repeats = before
                .is_some_and(|two_back| two_back.value().total_cmp(&now.read.value()).is_eq());
            before = Some(last.read);
            last = now;
            if now.read.value() < lowest.read.value() {
                lowest = now;
            }
            if now.read.value() > highest.read.value() {
                highest = now;
            }
            if done {
                return (last.read, last.pass, count, None);
            }
            if repeats && cycles == TwoCycles::Stop {
                break;
            }
        }
        let bound = |read: f64| (BondAlbedo::from_fraction(read), None);
        let (mut lo, mut hi) = if (lowest.g >= 0.0) == (highest.g >= 0.0) {
            (bound(0.0), bound(LARGEST_BOND_ALBEDO))
        } else {
            ((lowest.read, Some(lowest)), (highest.read, Some(highest)))
        };
        let lower_rises = lo.1.is_none_or(|at: At| at.g >= 0.0);
        for _ in 0..HALVINGS {
            let (a, b) = (lo.0.value(), hi.0.value());
            let mid = a.midpoint(b);
            if !(a < mid && mid < b) {
                break;
            }
            let at = of(BondAlbedo::from_fraction(mid));
            count += 1;
            if (at.g >= 0.0) == lower_rises {
                lo = (at.read, Some(at));
            } else {
                hi = (at.read, Some(at));
            }
        }
        let mut taken = |(read, at): (BondAlbedo, Option<At>)| {
            at.unwrap_or_else(|| {
                count += 1;
                of(read)
            })
        };
        let (lo, hi) = (taken(lo), taken(hi));
        let published = if hi.g.abs() < lo.g.abs() { hi } else { lo };
        (
            published.read,
            published.pass,
            count,
            Some(published.g.abs()),
        )
    }

    /// A stub map of the albedo read to the albedo given, boxed.
    type Map = Box<dyn Fn(f64) -> f64>;

    /// At C = (), the solver is P14.T51.h's to the bit (decision-p14-t54a-passes §1.9, test 1): on
    /// its 361 stubs and on every map of the other stub tests, from their starts, with the early
    /// stop and run on, the same albedo, pass, count and residual, every bracketed pass fitting its
    /// context's first round.
    #[test]
    fn at_no_context_the_solver_is_t51h_s_to_the_bit() {
        let mut maps: Vec<(f64, Map)> = t51h_stubs()
            .map(|(fixed, slope)| {
                let f: Map = Box::new(move |a| fixed + slope * (a - fixed));
                (0.3, f)
            })
            .collect();
        let others: [(f64, Map); 7] = [
            (0.3, Box::new(|a| 0.2 + 0.1 * (a - 0.2))),
            (0.3, Box::new(|a| 0.2 + 0.5 * (a - 0.2))),
            (0.3, Box::new(|a| 0.25 - 2.0 * (a - 0.25))),
            (0.3, Box::new(|a| a + 1e-3 * (1.0 - a))),
            (0.3, Box::new(|a| 0.999 * a)),
            (
                0.25,
                Box::new(|a| if a < 0.5 { a + 0.25 } else { a - 0.25 }),
            ),
            (
                0.3,
                Box::new(|a| (0.300_5 - 2.0 * (a - 0.300_5)).clamp(0.299_5, 0.302_0)),
            ),
        ];
        maps.extend(others);
        maps.extend(CYCLING.map(|f| {
            let f: Map = Box::new(f);
            (0.3, f)
        }));
        let (mut bracketed, mut compared) = (0, 0);
        for (start, f) in &maps {
            let start = BondAlbedo::from_fraction(*start);
            for cycles in [TwoCycles::Stop, TwoCycles::Run] {
                let now = solve_with(
                    plain(cycles, PLAIN_PASSES),
                    start,
                    (),
                    |read: BondAlbedo, (): &()| stub(f)(read),
                    gives,
                    |_: &(f64, f64)| {},
                    settled,
                );
                let (albedo, pass, count, residual) = t51h_solve(cycles, start, stub(f), settled);
                assert_same_bits(now.albedo.value(), albedo.value());
                assert_same_bits(now.pass.0, pass.0);
                assert_same_bits(now.pass.1, pass.1);
                assert_eq!(now.count, count);
                match residual {
                    None => assert_eq!(now.convergence, Convergence::Converged),
                    Some(residual) => {
                        assert_same_bits(residual_of(now.convergence), residual);
                        assert_eq!(fit_of(now.convergence), ContextFit::FirstRound);
                        bracketed += 1;
                    }
                }
                compared += 1;
            }
        }
        // `solve` is the same: the early stop's rule at the plain passes' cap.
        for (start, f) in &maps {
            let start = BondAlbedo::from_fraction(*start);
            let now = solve(start, stub(f), gives, settled);
            let (albedo, pass, count, _) = t51h_solve(TwoCycles::Stop, start, stub(f), settled);
            assert_same_bits(now.albedo.value(), albedo.value());
            assert_same_bits(now.pass.1, pass.1);
            assert_eq!(now.count, count);
        }
        assert_eq!(compared, 2 * (361 + 7 + 3));
        assert!(bracketed > 20, "{bracketed}");
        eprintln!("T51.h's solver at C = (): {compared} runs compared, {bracketed} bracketed");
    }

    /// A stub context: a float member `level` and a discrete one, its key, `branch`, as a lid or a
    /// tide branch is.
    #[derive(Debug, Clone, Copy, PartialEq)]
    struct Lid {
        level: f64,
        branch: u8,
    }

    impl Lid {
        /// The context of branch `branch` at level 0.
        const fn branch(branch: u8) -> Self {
            Self { level: 0.0, branch }
        }
    }

    impl Context for Lid {
        type Key = u8;

        fn key(&self) -> u8 {
            self.branch
        }

        fn agrees(&self, other: &Self) -> bool {
            self.branch == other.branch && floats_agree(self.level, other.level)
        }

        fn rank(&self, other: &Self) -> Ordering {
            self.branch
                .cmp(&other.branch)
                .then_with(|| self.level.total_cmp(&other.level))
        }
    }

    /// A stub pass under a context: the albedo read, the albedo `f` gives there under the context
    /// read, held into [0, 1), and the context read.
    type LidPass = (f64, f64, Lid);

    /// T51.b's test as the passes take it, on the equilibrium temperature at the albedo each pass
    /// read, `T_eq` ∝ (1 − A)^¼.
    fn read_settled(last: &LidPass, now: &LidPass) -> bool {
        let t_eq = |a: f64| (1.0 - a).sqrt().sqrt();
        (t_eq(now.0) - t_eq(last.0)).abs() <= 1e-9 * t_eq(now.0)
    }

    /// The solver under a context from 0.3 and the stand-in context at level 0 on branch 0, with
    /// the plain passes' rule `plain`: each pass gives `f` of the albedo and the context it read,
    /// and its air the context `k` of the albedo it read.
    fn lid_solve(
        plain: Plain,
        f: impl Fn(f64, Lid) -> f64,
        k: impl Fn(f64) -> Lid,
    ) -> Solved<LidPass, Lid> {
        lid_solve_from(plain, f, |pass: &LidPass| k(pass.0))
    }

    /// [`lid_solve`], the context `next` of the whole pass.
    fn lid_solve_from(
        plain: Plain,
        f: impl Fn(f64, Lid) -> f64,
        next: impl Fn(&LidPass) -> Lid,
    ) -> Solved<LidPass, Lid> {
        solve_with(
            plain,
            BondAlbedo::BEFORE_ATMOSPHERES,
            Lid::branch(0),
            |read: BondAlbedo, lid: &Lid| {
                let a = read.value();
                (a, f(a, *lid).clamp(0.0, LARGEST_BOND_ALBEDO), *lid)
            },
            |pass: &LidPass| BondAlbedo::from_fraction(pass.1),
            next,
            read_settled,
        )
    }

    /// Whether two outcomes under a context are the same: the albedo, the pass and the contexts to
    /// the bit, and how they ended.
    fn assert_same_lid_outcome(one: &Solved<LidPass, Lid>, two: &Solved<LidPass, Lid>) {
        assert_same_bits(one.albedo.value(), two.albedo.value());
        assert_same_bits(one.pass.1, two.pass.1);
        for (a, b) in [(one.context, two.context), (one.next, two.next)] {
            assert_eq!(a.branch, b.branch);
            assert_same_bits(a.level, b.level);
        }
        assert_same_outcome(one.convergence, two.convergence);
    }

    /// A weakly coupled linear (A, C) stub settles at its joint fixed point (decision-p14-t54a-passes
    /// §1.9, test 2): F(A, C) = 0.25 + 10⁻³ (A − 0.25) + 10⁻³ (C − 0.5) and K(P) = 0.5 + 10⁻³ (F −
    /// 0.25), whose only fixed point is (0.25, 0.5), from 0.3 under C₀ = 0. The lag's eigenvalues are
    /// about ±10⁻³, so it settles in a few passes, within 10⁻¹² of the point in both, and its
    /// published context agrees with the one its own air gives.
    #[test]
    fn a_weakly_coupled_context_settles_at_its_joint_fixed_point() {
        let solved = lid_solve_from(
            Plain::SOLVE,
            |a, lid| 0.25 + 1e-3 * (a - 0.25) + 1e-3 * (lid.level - 0.5),
            |pass: &LidPass| Lid {
                level: 0.5 + 1e-3 * (pass.1 - 0.25),
                branch: 0,
            },
        );
        assert_eq!(solved.convergence, Convergence::Converged);
        assert!(solved.count > 3 && solved.count < 10, "{}", solved.count);
        assert!(
            (solved.albedo.value() - 0.25).abs() < 1e-12,
            "{:?}",
            solved.albedo
        );
        assert!(
            (solved.context.level - 0.5).abs() < 1e-12,
            "{:?}",
            solved.context
        );
        assert!(solved.context.agrees(&solved.next));
        // The published pass's own context is the one the passes would read next.
        assert_same_bits(solved.next.level, 0.5 + 1e-3 * (solved.pass.1 - 0.25));
        eprintln!(
            "weakly coupled: {} passes, A {:e} from 0.25, C {:e} from 0.5",
            solved.count,
            solved.albedo.value() - 0.25,
            solved.context.level - 0.5
        );
    }

    /// The settle hole (decision-p14-t54a-passes §1.2, test 3; the ruling's stub): F(0.3, C₀) = 0.3
    /// exactly, so pass 2 reads pass 1's albedo, but the context has changed after pass 1 (K = 1
    /// always), and under it F(A, 1) = 0.6 − 0.5 (A − 0.3). Pass 2's equilibrium temperature is
    /// pass 1's to the bit, but its context is not pass 1's, so it is not published, where the
    /// lean would have published it with a residual of 0.3. The passes reach F(·, 1)'s fixed point,
    /// 0.5, instead (slope −0.5: bracketed after the plain passes, a fixed point).
    #[test]
    fn a_pass_whose_context_moved_after_the_one_before_is_not_published() {
        let solved = lid_solve(
            Plain::SOLVE,
            |a, lid| {
                if lid.branch == 0 {
                    0.3
                } else {
                    0.6 - 0.5 * (a - 0.3)
                }
            },
            |_| Lid::branch(1),
        );
        assert!(solved.count > 2, "{}", solved.count);
        assert_eq!(solved.context, Lid::branch(1));
        let residual = solved.pass.1 - solved.albedo.value();
        assert!(residual.abs() < 1e-8, "{residual:e}");
        assert!(
            (solved.albedo.value() - 0.5).abs() < 1e-8,
            "{:?}",
            solved.albedo
        );
        assert!(!solved.convergence.is_jump());
        // The hole itself: pass 2 settles by T51.b's test alone.
        let start = BondAlbedo::BEFORE_ATMOSPHERES.value();
        assert!(read_settled(
            &(start, 0.3, Lid::branch(0)),
            &(0.3, 0.6, Lid::branch(1))
        ));
        eprintln!(
            "the settle hole: {} passes, {:?}, published {:?}",
            solved.count, solved.convergence, solved.albedo
        );
    }

    /// Where the albedo 2-cycles to the bit but the context does not repeat, the plain passes do
    /// not stop early: a context that moves every pass (its level the one read plus 1) under the
    /// jump at 0.4 runs all [`PLAIN_PASSES`], where at C = () the same map stops at its 2-cycle.
    #[test]
    fn an_albedo_2_cycle_under_a_moving_context_does_not_stop_early() {
        let jump = |a: f64, _: Lid| if a < 0.4 { 0.6 } else { 0.2 };
        let moving = lid_solve_from(Plain::SOLVE, jump, |pass: &LidPass| Lid {
            level: pass.2.level + 1.0,
            branch: 0,
        });
        // 24 plain passes, both ends re-evaluated under the frozen context, 32 halvings.
        assert_eq!(moving.count, PLAIN_PASSES + 2 + HALVINGS);
        assert_eq!(fit_of(moving.convergence), ContextFit::FirstRound);
        assert!(moving.convergence.is_jump());
        let plain = solved(|a| if a < 0.4 { 0.6 } else { 0.2 });
        assert!(plain.count < moving.count - 2, "{}", plain.count);
        assert_same_bits(plain.albedo.value(), moving.albedo.value());
    }

    /// The bracket's ends are re-evaluated under the frozen context (decision-p14-t54a-passes §1.4,
    /// test 5), and it falls back to [0, 1⁻] where they no longer bracket: under F(A, 0) = A + 0.5
    /// and F(A, 1) = A − 0.5, held into [0, 1), with K = 1 below 0.5 and 0 from it, the passes
    /// from 0.3 2-cycle over (0.3 + 2⁻⁵⁴, 0) and (0.8, 1) (the subtraction is exact), whose ends,
    /// 0.3 under C₀ = 0 and 0.8 under 1, bracket under their own contexts (g = ±0.5). The frozen
    /// context is the lower-albedo pass's, 0, under which both rise; so the bracket is [0, 1⁻], and
    /// the published pass is its upper bound, F's clamp under C = 0, residual 0. The lower end read
    /// C = 0 and is reused, the upper is re-evaluated: 4 + 1 + 32 + 1 passes.
    #[test]
    fn the_ends_are_re_evaluated_under_the_frozen_context() {
        let solved = lid_solve(
            Plain::SOLVE,
            |a, lid| if lid.branch == 0 { a + 0.5 } else { a - 0.5 },
            |read| Lid::branch(u8::from(read < 0.5)),
        );
        assert_same_bits(solved.albedo.value(), LARGEST_BOND_ALBEDO);
        assert_same_bits(residual_of(solved.convergence), 0.0);
        assert_eq!(solved.context, Lid::branch(0));
        assert_eq!(fit_of(solved.convergence), ContextFit::FirstRound);
        assert_eq!(solved.count, 4 + 1 + HALVINGS + 1);
    }

    /// The second round (decision-p14-t54a-passes §1.5, test 6): F(A, C) = 0.25 − 1.6 (A − 0.25) +
    /// 0.002 C, whose root is 0.25 under C = 0 and 0.652 ÷ 2.6 under C = 1, and K = 1 where the
    /// albedo read is below `edge`. The passes 2-cycle over (0, 0) and (0.65, 1), so the first round
    /// freezes C = 0 and finds 0.25, whose own air gives C = 1.
    fn second_round(edge: f64) -> Solved<LidPass, Lid> {
        lid_solve(
            Plain::SOLVE,
            |a, lid| 0.25 - 1.6 * (a - 0.25) + 0.002 * f64::from(lid.branch),
            move |read| Lid::branch(u8::from(read < edge)),
        )
    }

    /// With the edge at 0.26, both roots lie below it: C = 0's is not self-consistent, and C = 1's
    /// is, which the second round finds and publishes, its context fitting.
    #[test]
    fn a_consistent_pair_only_at_the_other_context_is_found_by_the_second_round() {
        let solved = second_round(0.26);
        assert_eq!(fit_of(solved.convergence), ContextFit::SecondRound);
        assert!(!solved.convergence.is_jump());
        assert_eq!(solved.context, Lid::branch(1));
        assert_eq!(solved.next, Lid::branch(1));
        assert!(
            (solved.albedo.value() - 0.652 / 2.6).abs() < 1e-9,
            "{:?}",
            solved.albedo
        );
        assert!(residual_of(solved.convergence) < 1e-9);
        // 8 plain passes; each round re-evaluates one end and takes its 32 halvings.
        assert_eq!(solved.count, 8 + 2 * (1 + HALVINGS));
    }

    /// With the edge at 0.2504, between the roots, neither context is self-consistent (a
    /// flip-flop): the lower albedo of the two rounds' passes, the first round's 0.25 under C = 0,
    /// is published, labelled a context jump, though its residual under the context it read is a
    /// fixed point's.
    #[test]
    fn a_flip_flop_publishes_the_lower_albedo_as_a_context_jump() {
        let solved = second_round(0.250_4);
        assert_eq!(fit_of(solved.convergence), ContextFit::Jump);
        assert!(solved.convergence.is_jump());
        assert_eq!(solved.context, Lid::branch(0));
        assert_eq!(solved.next, Lid::branch(1));
        assert!(
            (solved.albedo.value() - 0.25).abs() < 1e-9,
            "{:?}",
            solved.albedo
        );
        assert!(residual_of(solved.convergence) < 1e-9);
        assert!(solved.count <= MOST_PASSES, "{}", solved.count);
    }

    /// The passes are at most [`MOST_PASSES`], 94, and a body can take them all: F(A) = A + 0.01
    /// creeps up without settling under a context whose level moves every pass, so the plain passes
    /// run to 24 and neither end read the frozen context; g > 0 everywhere, so each round's bracket
    /// is [0, 1⁻] and takes its upper bound's pass; and the bound's own air gives another branch
    /// (K's branch is 1 above 0.9), so a second round runs, which fits.
    #[test]
    fn the_passes_take_at_most_94() {
        assert_eq!(MOST_PASSES, 94);
        let solved = lid_solve_from(
            Plain::SOLVE,
            |a, _| a + 0.01,
            |pass: &LidPass| Lid {
                level: pass.2.level + 1.0,
                branch: u8::from(pass.0 > 0.9),
            },
        );
        assert_eq!(solved.count, MOST_PASSES);
        assert_eq!(fit_of(solved.convergence), ContextFit::SecondRound);
        assert_same_bits(solved.albedo.value(), LARGEST_BOND_ALBEDO);
        for edge in [0.26, 0.250_4] {
            assert!(second_round(edge).count <= MOST_PASSES);
        }
    }

    /// Under the secant steps the lagged contexts keep their outcomes, in fewer passes: the
    /// consistent pair only at the other context is still found by the second round, the
    /// flip-flop still publishes the lower albedo as a context jump, and a body that takes every
    /// pass it can takes at most [`MOST_SECANT_PASSES`], 112.
    #[test]
    fn under_the_secant_root_the_lagged_contexts_keep_their_outcomes() {
        assert_eq!(MOST_SECANT_PASSES, 112);
        let (pair, halving_pair) = (
            with_pass_root(PassRoot::Secant, || second_round(0.26)),
            second_round(0.26),
        );
        assert_eq!(fit_of(pair.convergence), ContextFit::SecondRound);
        assert_eq!(pair.context, Lid::branch(1));
        let off = (pair.albedo.value() - 0.652 / 2.6).abs();
        assert!(off < 1e-12, "{off:e}");
        assert!(
            pair.count < halving_pair.count,
            "{} against {}",
            pair.count,
            halving_pair.count
        );
        let flip_flop = with_pass_root(PassRoot::Secant, || second_round(0.250_4));
        assert_eq!(fit_of(flip_flop.convergence), ContextFit::Jump);
        assert_eq!(flip_flop.context, Lid::branch(0));
        let off = (flip_flop.albedo.value() - 0.25).abs();
        assert!(off < 1e-12, "{off:e}");
        let creeping = with_pass_root(PassRoot::Secant, || {
            lid_solve_from(
                Plain::SOLVE,
                |a, _| a + 0.01,
                |pass: &LidPass| Lid {
                    level: pass.2.level + 1.0,
                    branch: u8::from(pass.0 > 0.9),
                },
            )
        });
        assert!(creeping.count <= MOST_SECANT_PASSES, "{}", creeping.count);
        assert_same_bits(creeping.albedo.value(), LARGEST_BOND_ALBEDO);
        eprintln!(
            "the pair {} passes against the halvings' {}, the flip-flop {}, the creeping {}",
            pair.count, halving_pair.count, flip_flop.count, creeping.count
        );
    }

    /// Two runs under a context agree to the bit, the second round's and the context jump's
    /// included.
    #[test]
    fn the_solver_under_a_context_is_deterministic() {
        for edge in [0.26, 0.250_4, 0.0] {
            assert_same_lid_outcome(&second_round(edge), &second_round(edge));
        }
    }

    /// Floats agree within 10⁻⁹ of the larger, to the same bits (infinities and signed zeros
    /// included), and never where either is a NaN.
    #[test]
    fn floats_agree_within_1e_9_and_never_at_a_nan() {
        assert!(floats_agree(1.0, 1.0 + 0.9e-9));
        assert!(!floats_agree(1.0, 1.0 + 1.1e-9));
        assert!(floats_agree(0.0, -0.0));
        assert!(floats_agree(f64::INFINITY, f64::INFINITY));
        assert!(!floats_agree(f64::INFINITY, f64::NEG_INFINITY));
        assert!(!floats_agree(f64::NAN, f64::NAN));
        assert!(!floats_agree(1.0, f64::NAN));
        assert!(!floats_agree(0.0, 1e-300));
    }

    // P14.T51.l: the superlinear root, behind its hold at 25.

    /// [`solved`] under the secant steps.
    fn secant_solved(f: impl Fn(f64) -> f64) -> Solved<(f64, f64)> {
        with_pass_root(PassRoot::Secant, || solved(f))
    }

    /// P14.T51.l is held until the 24 → 25 bump: the root in force is T51.h's halvings.
    #[test]
    fn the_secant_root_is_held_only_until_the_25_bump() {
        assert!(
            GENERATOR_VERSION < SECANT_ROOT_VERSION,
            "the 24 → 25 bump puts P14.T51.l's secant steps on the record (decision-24-cost D4): \
             delete `SECANT_ROOT_VERSION`, `PassRoot::in_force`'s version test, the tests' \
             `with_pass_root` and its switch in `with_every_25_hold`, and this test, keeping \
             `PassRoot::Halvings` as the tests' oracle of the secant steps; and read \
             `MOST_SECANT_PASSES`, 112, where `MOST_PASSES`, 94, bounds a body's passes \
             (`DerivedBody::atmosphere_passes`' docs, the system tests' whole-sample check)"
        );
        assert_eq!(PassRoot::in_force(), PassRoot::Halvings);
        assert_eq!(
            with_pass_root(PassRoot::Secant, PassRoot::in_force),
            PassRoot::Secant
        );
        assert_eq!(PassRoot::in_force(), PassRoot::Halvings);
    }

    /// T51.h's stubs under the secant steps reach the halvings' fixed points: slope −2 held into its
    /// band and slope −2 to the clamps within 10⁻¹² of the fixed point, each in fewer passes; the
    /// jump at 0.4 within the stop's 2⁻³², its residual about 0.2, labelled a jump.
    #[test]
    fn under_the_secant_root_t51h_s_stubs_reach_their_fixed_points() {
        /// A stub's name, its fixed point and its map.
        type Case = (&'static str, f64, Box<dyn Fn(f64) -> f64>);
        let cases: [Case; 2] = [
            (
                "slope −2 in its band",
                0.300_5,
                Box::new(|a: f64| (0.300_5 - 2.0 * (a - 0.300_5)).clamp(0.299_5, 0.302)),
            ),
            (
                "slope −2 to the clamps",
                0.25,
                Box::new(|a: f64| 0.25 - 2.0 * (a - 0.25)),
            ),
        ];
        for (what, fixed, f) in &cases {
            let halvings = solved(f);
            let secant = secant_solved(f);
            let off = (secant.albedo.value() - fixed).abs();
            assert!(off < 1e-12, "{what}: {off:e}");
            assert!(residual_of(secant.convergence) < JUMP_RESIDUAL, "{what}");
            assert!(
                secant.count < halvings.count,
                "{what}: {} against {}",
                secant.count,
                halvings.count
            );
            eprintln!(
                "{what}: {} passes against the halvings' {}, {off:e} from the fixed point",
                secant.count, halvings.count
            );
        }
        let jump = |a: f64| if a < 0.4 { 0.6 } else { 0.2 };
        let (halvings, secant) = (solved(jump), secant_solved(jump));
        assert!(
            (secant.albedo.value() - 0.4).abs() <= SECANT_STOP_WIDTH,
            "{:?}",
            secant.albedo
        );
        let residual = residual_of(secant.convergence);
        assert!((residual - 0.2).abs() < 1e-9, "{residual}");
        assert!(secant.convergence.is_jump(), "{:?}", secant.convergence);
        eprintln!(
            "the jump at 0.4: {} passes against the halvings' {}",
            secant.count, halvings.count
        );
    }

    /// A stub of slope −1 through its fixed point, F(A) = 2A* − A, about which every pass
    /// 2-cycles: the cycle is its own bracket, and the secant steps reach the fixed point within
    /// 10⁻¹² in a few passes, well within [`SECANT_STEPS`]; and so with a curvature, F(A) = 2A* −
    /// A − 3 (A − A*)², whose cycle is no longer symmetric.
    #[test]
    fn a_stub_of_slope_minus_1_brackets_within_the_fixed_count() {
        let fixed = 0.27;
        for (what, f) in [
            (
                "linear",
                Box::new(move |a: f64| 2.0 * fixed - a) as Box<dyn Fn(f64) -> f64>,
            ),
            (
                "curved",
                Box::new(move |a: f64| 2.0 * fixed - a - 3.0 * (a - fixed) * (a - fixed)),
            ),
        ] {
            let halvings = solved(&f);
            let secant = secant_solved(&f);
            let off = (secant.albedo.value() - fixed).abs();
            assert!(off < 1e-12, "{what}: {off:e}");
            assert!(
                secant.count < PLAIN_PASSES + SECANT_STEPS,
                "{what}: {}",
                secant.count
            );
            assert!(
                secant.count < halvings.count,
                "{what}: {} against {}",
                secant.count,
                halvings.count
            );
            eprintln!(
                "slope −1, {what}: {} passes against the halvings' {}, {off:e} from the fixed point",
                secant.count, halvings.count
            );
        }
    }

    /// Under the secant steps the bounds keep their halvings' outcomes: a creeping map's upper bound
    /// is the clamp's fixed point, a falling map's lower bound 0, each taken first and published at
    /// g = 0; and the halvings' tie, a jump whose two sides hold |g| = 0.25 exactly, still goes to
    /// the lower albedo: two false positions (0.375, 0.4375) fail to halve |g|, the rest are
    /// halvings, and the lower end stops at 0.5 − 2⁻³², the stop's width below the jump, where g is
    /// 0.25 exactly.
    #[test]
    fn under_the_secant_root_the_bounds_and_ties_keep_their_outcomes() {
        let creeping = secant_solved(|a| a + 1e-3 * (1.0 - a));
        assert_same_bits(creeping.albedo.value(), LARGEST_BOND_ALBEDO);
        assert_same_bits(residual_of(creeping.convergence), 0.0);
        let falling = secant_solved(|a| 0.999 * a);
        assert_same_bits(falling.albedo.value(), 0.0);
        assert_same_bits(residual_of(falling.convergence), 0.0);
        let f = |a: f64| if a < 0.5 { a + 0.25 } else { a - 0.25 };
        let tie = with_pass_root(PassRoot::Secant, || {
            solve(BondAlbedo::from_fraction(0.25), stub(f), gives, settled)
        });
        assert_same_bits(tie.albedo.value(), 0.5 - SECANT_STOP_WIDTH);
        assert_same_bits(residual_of(tie.convergence), 0.25);
        assert!(tie.convergence.is_jump(), "{:?}", tie.convergence);
        eprintln!(
            "creeping {} passes, falling {}, the tie {}",
            creeping.count, falling.count, tie.count
        );
    }

    /// Over T51.h's 361 stubs and 19 of slope −1, every one that brackets reaches its fixed point
    /// within 10⁻¹⁰ under the secant steps, as under the halvings, in no more passes; a stub that
    /// settles within the plain passes is the halvings' to the bit.
    #[test]
    fn over_t51h_s_stubs_the_secant_root_takes_no_more_passes() {
        let (mut bracketed, mut halving_passes, mut secant_passes) = (0, 0, 0);
        for (fixed, slope) in
            t51h_stubs().chain((0..19).map(|k| (0.05 + 0.05 * f64::from(k), -1.0)))
        {
            let f = move |a: f64| fixed + slope * (a - fixed);
            let (halvings, secant) = (solved(f), secant_solved(f));
            match halvings.convergence {
                Convergence::Converged => {
                    assert_eq!(
                        secant.convergence,
                        Convergence::Converged,
                        "{fixed} {slope}"
                    );
                    assert_same_bits(secant.albedo.value(), halvings.albedo.value());
                    assert_eq!(secant.count, halvings.count, "{fixed} {slope}");
                }
                Convergence::Bracketed { .. } => {
                    let off = (secant.albedo.value() - fixed).abs();
                    assert!(off < 1e-10, "{fixed} {slope}: {off:e}");
                    assert!(secant.count <= halvings.count, "{fixed} {slope}");
                    bracketed += 1;
                    halving_passes += halvings.count;
                    secant_passes += secant.count;
                }
            }
        }
        assert!(bracketed > 10, "{bracketed}");
        eprintln!(
            "{bracketed} bracketed stubs: {secant_passes} passes against the halvings' {halving_passes}"
        );
    }

    /// Under the secant steps two runs agree to the bit.
    #[test]
    fn the_secant_root_is_deterministic() {
        let f = |a: f64| (0.21 - 2.5 * (a - 0.21)).clamp(0.15, 0.278);
        let (one, two) = (secant_solved(f), secant_solved(f));
        assert_same_bits(one.albedo.value(), two.albedo.value());
        assert_same_bits(one.pass.1, two.pass.1);
        assert_eq!(one.count, two.count);
        assert_same_outcome(one.convergence, two.convergence);
    }

    /// Under each root, a jump across the diagonal and a steep crossing each end at a round that
    /// stopped at a crossing, its published end the solver's; and ends on one side, too far apart
    /// or whose g is not a number are no crossing.
    #[test]
    fn under_each_root_a_bracket_round_stops_at_a_crossing() {
        let jump = |a: f64| if a < 0.4 { 0.6 } else { 0.2 };
        let steep = |a: f64| (0.21 - 2.5 * (a - 0.21)).clamp(0.15, 0.278);
        for root in [PassRoot::Halvings, PassRoot::Secant] {
            for (what, f) in [("jump", &jump as &dyn Fn(f64) -> f64), ("steep", &steep)] {
                let (solved, crossings) = with_crossings(|| with_pass_root(root, || solved(f)));
                let [crossing] = crossings[..] else {
                    panic!("{what} under {root:?}: {crossings:?}");
                };
                assert!(crossing.is_valid(), "{what} under {root:?}: {crossing:?}");
                assert_same_bits(crossing.published(), solved.albedo.value());
            }
        }
        let crossing = |low, high| Crossing { low, high }.is_valid();
        let a = 0.3;
        let b = a + SECANT_STOP_WIDTH;
        assert!(crossing((a, 1e-3), (b, -1e-3)));
        assert!(crossing((a, -1e-3), (b, 1e-3)));
        assert!(crossing((a, 0.0), (a + 0.1, -1e-3)), "a root at the end");
        assert!(!crossing((a, 1e-3), (b, 2e-3)), "one side");
        assert!(
            !crossing((a, 1e-3), (a + 2.0 * SECANT_STOP_WIDTH, -1e-3)),
            "too wide"
        );
        assert!(!crossing((a, f64::NAN), (b, -1e-3)), "not a number");
    }
}
