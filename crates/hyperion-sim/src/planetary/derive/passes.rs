//! The batch's passes as a bracketed fixed point (P14.T51.h; decision-p14-t51-batch §4), under the
//! `p14-snapshot` lane's hold ([`SnapshotRules::Batch`]) until P14.T54.a.
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
//! - one whose F creeps toward the area mix's clamp too slowly to settle, a lossless visible-only
//!   frost whose fixed point is the clamp itself (until P14.T49.i's infrared).
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

use crate::planetary::derive::atmosphere::LARGEST_BOND_ALBEDO;
use crate::planetary::derive::irradiation::BondAlbedo;

#[cfg(doc)]
use crate::planetary::derive::{ATMOSPHERE_CONVERGENCE, atmosphere::SnapshotRules};

/// The most plain passes the batch iterates before it brackets (P14.T51.h): 24, above the 19 that
/// the slowest converging body of P14.T51.b's sample took (decision-p14-t51-batch §4). A body that
/// settles within them keeps P14.T51.b's bits.
pub(crate) const PLAIN_PASSES: u32 = 24;

/// The halvings of a bracket (P14.T51.h): 32, a fixed count, so that the bracket ends 2⁻³² of its
/// first width wide, under 2.4 × 10⁻¹⁰ in albedo: 10⁻⁹ in the equilibrium temperature for an
/// albedo up to 0.94, since d`T_eq` ÷ `T_eq` = dA ÷ 4(1 − A).
pub(crate) const HALVINGS: u32 = 32;

/// The residual |F(A) − A| in albedo above which a bracketed body's published pass is a jump across
/// the diagonal (P14.T51.h): 10⁻⁶, some four thousand times the bracket's width at the end of
/// [`HALVINGS`] from [0, 1), so that a continuous F of slope down to −4,000 there falls under it.
pub(crate) const JUMP_RESIDUAL: f64 = 1e-6;

/// How a body's passes ended (P14.T51.h; crate-private, for the tests and the bless note).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Convergence {
    /// T13.c's fixed three passes, under version 22's rules ([`SnapshotRules::Held`]).
    Fixed,
    /// The plain passes settled, within [`PLAIN_PASSES`].
    Converged,
    /// The passes did not settle, and bisection found the published pass.
    Bracketed {
        /// |F(A) − A| at the published pass, in albedo: under about 10⁻⁹ at a fixed point, and
        /// above [`JUMP_RESIDUAL`] at a jump across the diagonal.
        residual: f64,
    },
}

impl Convergence {
    /// Whether the passes ended bracketed with a residual above [`JUMP_RESIDUAL`]: at a jump across
    /// the diagonal, a state or cover change with no self-consistent albedo in the model's
    /// discrete states.
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
            Self::Bracketed { residual } => residual > JUMP_RESIDUAL,
            Self::Fixed | Self::Converged => false,
        }
    }
}

/// What [`solve`] leaves: the albedo the published pass read, what that pass gave, how many passes
/// were taken and how they ended.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Solved<P> {
    /// The albedo the published pass read.
    pub(crate) albedo: BondAlbedo,
    /// What the published pass gave.
    pub(crate) pass: P,
    /// How many passes were taken, the bisection's included.
    pub(crate) count: u32,
    /// How they ended.
    pub(crate) convergence: Convergence,
}

/// One pass as the solver reads it: the albedo it read, what it gave and g = F(A) − A there.
#[derive(Debug, Clone, Copy)]
struct Evaluated<P> {
    read: BondAlbedo,
    pass: P,
    g: f64,
}

impl<P> Evaluated<P> {
    /// The pass `pass` at the albedo `read`, whose albedo `gives` reads.
    #[must_use]
    fn of(read: BondAlbedo, pass: P, gives: impl Fn(&P) -> BondAlbedo) -> Self {
        let g = gives(&pass).value() - read.value();
        Self { read, pass, g }
    }

    /// Whether g is not negative here, the side of the diagonal the bisection keeps.
    #[must_use]
    fn rises(&self) -> bool {
        self.g >= 0.0
    }
}

/// A bracket's end: a pass taken, or one of [0, 1⁻]'s ends, whose side of the diagonal is known
/// without one.
#[derive(Debug, Clone, Copy)]
enum End<P> {
    Taken(Evaluated<P>),
    Bound(BondAlbedo),
}

impl<P: Copy> End<P> {
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
#[must_use]
pub(crate) fn solve<P: Copy>(
    start: BondAlbedo,
    pass: impl Fn(BondAlbedo) -> P,
    gives: impl Fn(&P) -> BondAlbedo + Copy,
    settled: impl Fn(&P, &P) -> bool,
) -> Solved<P> {
    solve_with(TwoCycles::Stop, start, pass, gives, settled)
}

/// What the plain passes do on a 2-cycle found to the bit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TwoCycles {
    /// Stop, and bracket at once: [`solve`]'s rule.
    Stop,
    /// Run on to [`PLAIN_PASSES`]: the reference the early stop is tested against.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "the early stop's test compares the two")
    )]
    Run,
}

/// [`solve`], whose plain passes do `cycles` on a 2-cycle found to the bit. A pass must be a
/// function of the albedo it reads alone, which the early stop and the bisection both need, so
/// `pass` is a `Fn`.
#[must_use]
fn solve_with<P: Copy>(
    cycles: TwoCycles,
    start: BondAlbedo,
    pass: impl Fn(BondAlbedo) -> P,
    gives: impl Fn(&P) -> BondAlbedo + Copy,
    settled: impl Fn(&P, &P) -> bool,
) -> Solved<P> {
    let mut last = Evaluated::of(start, pass(start), gives);
    let mut count = 1;
    let (mut lowest, mut highest) = (last, last);
    let mut before: Option<BondAlbedo> = None;
    while count < PLAIN_PASSES {
        let next = gives(&last.pass);
        let now = Evaluated::of(next, pass(next), gives);
        count += 1;
        let done = settled(&last.pass, &now.pass);
        let repeats =
            before.is_some_and(|two_back| two_back.value().total_cmp(&now.read.value()).is_eq());
        before = Some(last.read);
        last = now;
        if now.read.value() < lowest.read.value() {
            lowest = now;
        }
        if now.read.value() > highest.read.value() {
            highest = now;
        }
        if done {
            return Solved {
                albedo: last.read,
                pass: last.pass,
                count,
                convergence: Convergence::Converged,
            };
        }
        if repeats && cycles == TwoCycles::Stop {
            break;
        }
    }
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
        let read = BondAlbedo::from_fraction(mid);
        let at = Evaluated::of(read, pass(read), gives);
        count += 1;
        if at.rises() == lower_rises {
            lo = End::Taken(at);
        } else {
            hi = End::Taken(at);
        }
    }
    let mut taken = |end: End<P>| match end {
        End::Taken(at) => at,
        End::Bound(read) => {
            count += 1;
            Evaluated::of(read, pass(read), gives)
        }
    };
    let (lo, hi) = (taken(lo), taken(hi));
    let published = if hi.g.abs() < lo.g.abs() { hi } else { lo };
    Solved {
        albedo: published.read,
        pass: published.pass,
        count,
        convergence: Convergence::Bracketed {
            residual: published.g.abs(),
        },
    }
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
        let Convergence::Bracketed { residual } = convergence else {
            panic!("{convergence:?}");
        };
        residual
    }

    /// Whether two outcomes are the same, a bracketed one's residual by its bits.
    fn assert_same_outcome(one: Convergence, two: Convergence) {
        match (one, two) {
            (Convergence::Bracketed { residual: a }, Convergence::Bracketed { residual: b }) => {
                assert_same_bits(a, b);
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
        let Convergence::Bracketed { residual } = slow.convergence else {
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

    /// A body that settles within the plain passes keeps P14.T51.b's bits: over contracting and
    /// oscillating-but-converging maps of slopes from −0.9 to 0.9 about fixed points from 0.05 to
    /// 0.95, every one that P14.T51.b's loop settled within [`PLAIN_PASSES`] gives the same albedo,
    /// pass and count.
    #[test]
    fn on_stubs_the_solver_settles_as_t51b_s_loop() {
        let mut settled_within = 0;
        for k in 0..19 {
            for j in 0..19 {
                let fixed = 0.05 + 0.05 * f64::from(k);
                let slope = -0.9 + 0.1 * f64::from(j);
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
        let Convergence::Bracketed { residual } = solved.convergence else {
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
        let Convergence::Bracketed { residual } = solved.convergence else {
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
        let Convergence::Bracketed { residual } = solved.convergence else {
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
        let Convergence::Bracketed { residual } = creeping.convergence else {
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

    /// A 2-cycle found to the bit gives the same published pass as the plain passes run to their
    /// end: the early stop saves passes and moves nothing.
    #[test]
    fn stopping_at_a_detected_2_cycle_moves_nothing() {
        let maps: [&dyn Fn(f64) -> f64; 3] = [
            &|a| if a < 0.4 { 0.6 } else { 0.2 },
            &|a| (0.300_5 - 2.0 * (a - 0.300_5)).clamp(0.299_5, 0.302),
            &|a| (0.21 - 2.5 * (a - 0.21)).clamp(0.15, 0.278),
        ];
        for f in maps {
            let at = |cycles| {
                solve_with(
                    cycles,
                    BondAlbedo::BEFORE_ATMOSPHERES,
                    stub(f),
                    gives,
                    settled,
                )
            };
            let (early, late) = (at(TwoCycles::Stop), at(TwoCycles::Run));
            assert_same_bits(early.albedo.value(), late.albedo.value());
            assert_same_bits(early.pass.1, late.pass.1);
            assert_same_outcome(early.convergence, late.convergence);
            assert!(early.count < late.count, "{} {}", early.count, late.count);
        }
    }
}
