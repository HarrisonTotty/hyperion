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

use core::cmp::Ordering;
use core::fmt;

use crate::planetary::derive::atmosphere::LARGEST_BOND_ALBEDO;
use crate::planetary::derive::irradiation::BondAlbedo;

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

/// The most passes a body takes under a lagged context (P14.T54.a; decision-p14-t54a-passes §1.5):
/// [`PLAIN_PASSES`], then two rounds of a bracket, each its two ends re-evaluated under its
/// context, its [`HALVINGS`] and the one pass more where a bound of [0, 1⁻] is still an end: 24 +
/// 2 × (2 + 32 + 1) = 94. At C = (), as [`solve`] runs, no end is re-evaluated and no second round
/// runs, so T51.h's [`PLAIN_PASSES`] + [`HALVINGS`] + 1 holds.
pub(crate) const MOST_PASSES: u32 = PLAIN_PASSES + 2 * (2 + HALVINGS + 1);

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
/// taken: the one more where a bound of [0, 1⁻] is still an end and needs its pass.
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
/// keys differ, as the module's documentation says. At most [`MOST_PASSES`] passes are taken.
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
            break;
        }
    }
    // The frozen context: the one read by the lower-albedo pass of the last two (§1.4).
    let frozen = previous.map_or(last, |previous| previous.lower(last, gives));
    let ends = (lowest, highest);
    let first_round = bracket(frozen.context, ends, &evaluate, &mut count);
    let first_next = next(&first_round.pass);
    let (published, published_next, fit) = if first_next.key() == frozen.context.key() {
        (first_round, first_next, ContextFit::FirstRound)
    } else {
        // The second round, frozen at the context the published pass's own air gives (§1.5).
        let second_round = bracket(first_next, ends, &evaluate, &mut count);
        let second_next = next(&second_round.pass);
        if second_next.key() == first_next.key() {
            (second_round, second_next, ContextFit::SecondRound)
        } else if first_round.canonical(&second_round, gives) == Ordering::Greater {
            (second_round, second_next, ContextFit::Jump)
        } else {
            (first_round, first_next, ContextFit::Jump)
        }
    };
    debug_assert!(
        count <= MOST_PASSES,
        "{count} passes, above the cap of {MOST_PASSES}"
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

/// One round of T51.h's bracket under the context `frozen` (decision-p14-t54a-passes §1.4): the
/// plain passes' least and greatest albedo `(lowest, highest)` re-evaluated under it, each that
/// read it to the bit reused, the bracket [0, 1⁻] where they no longer bracket, [`HALVINGS`]
/// halvings, each one pass of `evaluate` at the midpoint, and the end with the smaller |g|, ties to
/// the lower albedo. Every pass taken counts on `count`.
#[must_use]
fn bracket<P: Copy, C: Context>(
    frozen: C,
    (lowest, highest): (Evaluated<P, C>, Evaluated<P, C>),
    evaluate: &impl Fn(BondAlbedo, C) -> Evaluated<P, C>,
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
    let mut taken = |end: End<P, C>| match end {
        End::Taken(at) => at,
        End::Bound(read) => {
            *count += 1;
            evaluate(read, frozen)
        }
    };
    let (lo, hi) = (taken(lo), taken(hi));
    if hi.g.abs() < lo.g.abs() { hi } else { lo }
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
}
