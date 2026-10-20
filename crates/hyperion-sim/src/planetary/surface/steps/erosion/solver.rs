//! Tzathas et al. 2024's analytical solution of the stream-power law with n = 1 on a river network
//! (R09.T14.b; CGF 43(2), doi:10.1111/cgf.15033, §4).
//!
//! With n = 1, ∂z/∂t = u − k A^m ‖∇z‖ is hyperbolic, and along a river path, s the distance from
//! the bound, its characteristics obey dτ/ds = 1/a(s), a = k A^m (their eqs. 3 and 5). A point x
//! at time t takes the construction's elevation from where its characteristic left the initial
//! surface, `D_x,t`, where τ is t less, plus the uplift gathered on the way:
//!
//! - z(x, t) = z0(`D_x,t`) + ∫ from `D_x,t` to x of u/a ds (eq. 8), while that point lies on the
//!   path;
//! - z(x, t) = z0(0) + ∫ from 0 to x of u/a ds (eq. 9), the steady state, once the characteristic
//!   reaches the bound, where u vanishes and z is held at z0(0) (eq. 4).
//!
//! The recursion (§4.2–4.3) marches each river tree from its root depth first, keeping the path
//! from the root to the cell in an array indexed by depth (the [`Network`]'s `depth`), with, per
//! entry, τ (years from the path's base), Σ (∫ u/a, metres per unit of the uplift's scale λ) and
//! z0. For a cell c with receiver r over a link of length ℓ:
//!
//! - 1/a = `correction(c)` ÷ (K `A_eq`^m), m = [`DRAINAGE_EXPONENT`], with Tzathas et al.'s
//!   slope correction (§4.3) and the equivalent area (Hergarten 2021, eq. 16); the link takes its
//!   upstream cell's 1/a, so dτ = ℓ/a, `τ_c` = `τ_r` + dτ and `Σ_c` = `Σ_r` + `u_c` dτ (eqs.
//!   10–18, with δx the link);
//! - at steady state, or where `τ_c` ≤ t, z = the path's base + λ `Σ_c` (eq. 9);
//! - otherwise `D` lies where τ = `τ_c` − t: a binary search of the path's τ, which does not
//!   decrease from the base, finds its link, inside which z0 and Σ are interpolated linearly in τ,
//!   which is linear in distance there (the paper's "linear interpolation of the value of z0 at the
//!   two cell points enclosing the position D"); z = z0(D) + λ (`Σ_c` − Σ(D)) (eq. 8).
//!
//! A root, a fixed cell (a bound: the sea, a lake's bed) and a cell of zero equivalent area (no
//! runoff upstream, its own included, so 1/a is infinite) keep their ground and reset the path for
//! their donors to their base, τ = 0, Σ = 0 and z0 = the base: the water surface for a water cell,
//! the ground otherwise. No deposition (detachment-limited; Tzathas et al. erode without uplift
//! only by lowering): a cell with no uplift never ends above its construction. Where its base
//! already lies at or above it, it keeps its construction at every scale of the uplift; where only
//! the uplift downstream of it would raise it above it (a part F above zero), its construction is
//! a ceiling.
//!
//! The elevation is linear in λ, z = base + λ F, below that ceiling, and the solution returns the
//! two parts, so that the uplift's scale (decision-r09-t14-choices Q1) is one division.
//! Implemented from the paper alone; no code is copied or paraphrased from Tzathas et al.'s
//! repository (Inria research-only licence).

use hyperion_surface::field::LogSteepness;
use hyperion_surface::num;

use super::super::plates::slot;
use super::drainage::Routing;
use crate::math;

/// The stream-power law's drainage exponent m, at n = 1: 0.45, the concavity θ of the steepness
/// index (Design note 9, after Kirby and Whipple 2012, confirmed in Adams et al. 2020 and
/// Hergarten 2021; [`LogSteepness::THETA`]).
pub(crate) const DRAINAGE_EXPONENT: f64 = LogSteepness::THETA;

/// A river network as the solver reads it: each cell's receiver and the link's length, its slope
/// correction, and a depth-first order with each cell's depth. T14.a's [`Routing`] is one; a test
/// builds a 1-D chain by hand.
pub(crate) trait Network {
    /// The number of cells.
    fn len(&self) -> u32;

    /// Every cell once, each after its receiver, depth first: the cells after a cell at depth d and
    /// before the next cell at depth d or less are its upstream tree.
    fn preorder(&self) -> &[u32];

    /// Cell `cell`'s depth: 0 at a root, its receiver's plus 1 elsewhere.
    fn depth(&self, cell: u32) -> u32;

    /// Cell `cell`'s receiver and the length of the link to it, metres, or `None` at a root.
    fn downstream(&self, cell: u32) -> Option<(u32, f64)>;

    /// Cell `cell`'s slope correction, 0 to 1: the drop per metre to its receiver over the norm
    /// of its downstream gradient (Tzathas et al. 2024, §4.3).
    fn correction(&self, cell: u32) -> f64;
}

impl Network for Routing {
    fn len(&self) -> u32 {
        Routing::len(self)
    }

    fn preorder(&self) -> &[u32] {
        Routing::preorder(self)
    }

    fn depth(&self, cell: u32) -> u32 {
        Routing::depth(self, cell)
    }

    fn downstream(&self, cell: u32) -> Option<(u32, f64)> {
        self.receiver(cell).map(|link| (link.cell, link.length_m))
    }

    fn correction(&self, cell: u32) -> f64 {
        Routing::correction(self, cell)
    }
}

/// When the solution is taken.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Time {
    /// At steady state (Tzathas et al.'s eq. 9 everywhere): a world wet now.
    Steady,
    /// After this many Earth-equivalent years of flow, finite and non-negative (eq. 8): a world
    /// dry now, its wet epoch's `effective_flow`.
    Years(f64),
}

/// The per-cell inputs of a solve, each one a cell in cell-index order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Cells<'a> {
    /// The initial terrain z0, the construction, metres: finite.
    pub(crate) z0_m: &'a [f64],
    /// The uplift at λ = 1, u ≥ 0, m a⁻¹ per unit of λ (decision-r09-t14-choices Q1): finite.
    pub(crate) uplift_m_per_year: &'a [f64],
    /// The erodibility K, m^0.1 a⁻¹: finite and positive.
    pub(crate) erodibility_m0_1_per_year: &'a [f64],
    /// The equivalent drainage area `A_eq`, accumulated over the network, its own included, m²:
    /// finite and non-negative.
    pub(crate) equivalent_area_m2: &'a [f64],
    /// `Some(base)` for a cell held as a bound, not eroded, whose donors take `base` (metres) as
    /// their base level: the sea's surface over a sea cell, a lake's over its bed.
    pub(crate) fixed: &'a [Option<f64>],
}

/// A solve's elevations, z = min(base + λ F, ceiling) for the uplift's scale λ ≥ 0, each a cell's
/// in metres.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Solution {
    base: Vec<f64>,
    per_lambda: Vec<f64>,
    ceiling: Vec<f64>,
}

impl Solution {
    /// Each cell's elevation at λ = 0, metres: its path's base, or z0(D), the construction where
    /// its characteristic left it.
    #[must_use]
    pub(crate) fn base_m(&self) -> &[f64] {
        &self.base
    }

    /// Each cell's F = ∫ u/a from its base or from D, metres per unit of λ, non-negative.
    #[must_use]
    pub(crate) fn per_lambda_m(&self) -> &[f64] {
        &self.per_lambda
    }

    /// Cell `cell`'s elevation at the uplift's scale `lambda` (≥ 0), metres: base + λ F, held at
    /// its construction where the cell has no uplift and the uplift downstream would raise it
    /// above it (no deposition), which it is only where F is above zero.
    ///
    /// # Panics
    ///
    /// If `cell` is not a cell of the solve.
    #[must_use]
    pub(crate) fn elevation_m(&self, cell: u32, lambda: f64) -> f64 {
        let c = slot(cell);
        num::min(self.base[c] + lambda * self.per_lambda[c], self.ceiling[c])
    }
}

/// One entry of the path from a tree's root to the cell being solved.
#[derive(Debug, Clone, Copy, Default)]
struct Step {
    /// τ, years from the path's base (the last reset at or below it).
    tau_years: f64,
    /// Σ, ∫ u/a from the base, metres per unit of λ.
    sigma_m: f64,
    /// z0 at the entry, metres: its base at a reset.
    z0_m: f64,
    /// The depth of the path's base, the last reset at or below this entry.
    start: usize,
}

/// Tzathas et al.'s solution on `network` with `cells`' inputs at `time` (see the module
/// documentation).
///
/// # Panics
///
/// If an input is not one a cell, a value is not finite, an uplift or an equivalent area is
/// negative, an erodibility is not positive, the time is negative or not finite, or the network's
/// depths are not its receivers' plus 1.
#[must_use]
pub(crate) fn solve<N: Network + ?Sized>(network: &N, cells: &Cells<'_>, time: Time) -> Solution {
    let count = slot(network.len());
    for (what, len) in [
        ("height", cells.z0_m.len()),
        ("uplift", cells.uplift_m_per_year.len()),
        ("erodibility", cells.erodibility_m0_1_per_year.len()),
        ("equivalent area", cells.equivalent_area_m2.len()),
        ("bound", cells.fixed.len()),
    ] {
        assert_eq!(len, count, "one {what} a cell");
    }
    if let Time::Years(years) = time {
        assert!(
            years.is_finite() && years >= 0.0,
            "a time is finite and non-negative, not {years} years"
        );
    }
    assert_eq!(
        network.preorder().len(),
        count,
        "the preorder holds every cell"
    );
    let mut solution = Solution {
        base: vec![0.0; count],
        per_lambda: vec![0.0; count],
        ceiling: vec![f64::INFINITY; count],
    };
    let mut path: Vec<Step> = Vec::new();
    for &cell in network.preorder() {
        let at = slot(cell);
        let depth = slot(network.depth(cell));
        if path.len() <= depth {
            path.resize(depth + 1, Step::default());
        }
        let z0 = num::assert_finite(cells.z0_m[at]);
        let area = num::assert_finite(cells.equivalent_area_m2[at]);
        assert!(area >= 0.0, "cell {cell}'s equivalent area is {area} m²");
        let reset = match (cells.fixed[at], network.downstream(cell)) {
            (Some(base), _) => Some(num::assert_finite(base)),
            (None, None) => Some(z0),
            (None, Some(_)) if area <= 0.0 => Some(z0),
            (None, Some(_)) => None,
        };
        if let Some(base) = reset {
            solution.base[at] = z0;
            path[depth] = Step {
                tau_years: 0.0,
                sigma_m: 0.0,
                z0_m: base,
                start: depth,
            };
            continue;
        }
        let (receiver, length_m) = network
            .downstream(cell)
            .expect("a cell that is not reset drains");
        assert!(
            depth > 0 && slot(network.depth(receiver)) + 1 == depth,
            "cell {cell}'s depth is its receiver's plus 1"
        );
        let erodibility = cells.erodibility_m0_1_per_year[at];
        assert!(
            erodibility.is_finite() && erodibility > 0.0,
            "cell {cell}'s erodibility is {erodibility} m^0.1 a⁻¹"
        );
        let uplift = num::assert_finite(cells.uplift_m_per_year[at]);
        assert!(uplift >= 0.0, "cell {cell}'s uplift is {uplift} m a⁻¹");
        let inv_a = network.correction(cell) / (erodibility * math::powf(area, DRAINAGE_EXPONENT));
        let dtau = num::assert_finite(length_m * inv_a);
        let below = path[depth - 1];
        let here = Step {
            tau_years: below.tau_years + dtau,
            sigma_m: below.sigma_m + uplift * dtau,
            z0_m: z0,
            start: below.start,
        };
        path[depth] = here;
        let (mut base, mut per_lambda) = match time {
            Time::Years(years) if here.tau_years > years => {
                departure(&path[here.start..=depth], years)
            }
            Time::Steady | Time::Years(_) => (path[here.start].z0_m, here.sigma_m),
        };
        if uplift <= 0.0 {
            if base >= z0 {
                (base, per_lambda) = (z0, 0.0);
            } else if per_lambda > 0.0 {
                solution.ceiling[at] = z0;
            }
        }
        solution.base[at] = base;
        solution.per_lambda[at] = per_lambda;
    }
    solution
}

/// The base and F of the last entry of `path` (from its base to the cell, τ non-decreasing, the
/// cell's τ above `t`) at time `t`: z0 and Σ at D, where τ is the cell's less t, interpolated
/// linearly in τ inside the link that holds it, and the cell's Σ less Σ(D) (Tzathas et al.'s
/// eq. 8).
fn departure(path: &[Step], t: f64) -> (f64, f64) {
    let last = path.len() - 1;
    let cell = path[last];
    let target = cell.tau_years - t;
    // The first entry is the base at τ = 0 ≤ target, so at least one entry lies at or below it;
    // where rounding puts the cell's own τ there too (t far below an ulp of τ), D is the cell.
    let j = (path.partition_point(|step| step.tau_years <= target) - 1).min(last - 1);
    let (lo, hi) = (path[j], path[j + 1]);
    let span = hi.tau_years - lo.tau_years;
    let f = if span > 0.0 {
        num::min(1.0, num::max(0.0, (target - lo.tau_years) / span))
    } else {
        1.0
    };
    let z0_d = lo.z0_m + f * (hi.z0_m - lo.z0_m);
    let sigma_d = lo.sigma_m + f * (hi.sigma_m - lo.sigma_m);
    (z0_d, num::max(0.0, cell.sigma_m - sigma_d))
}
