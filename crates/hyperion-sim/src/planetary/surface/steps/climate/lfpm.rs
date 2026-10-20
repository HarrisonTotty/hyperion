//! The orographic precipitation over land (R09.T13.e; Design note 8, decision-r09-t13ge
//! §2.1–§2.4, §2.8): Hergarten and Robl's linear feedback precipitation model, carried along each
//! month's wind over the field's cells from the sea's large-scale rate at the coast.
//!
//! The source is Hergarten, S., and Robl, J. 2022, "The linear feedback precipitation model (LFPM
//! 1.0) – a simple and efficient model for orographic precipitation in the context of landform
//! evolution modeling", Geosci. Model Dev. 15, 2063–2084; equation numbers below are theirs, read
//! in full. The module is pure functions over a [`Mesh`] and one month's inputs ([`Month`]), with
//! no state; the step's water cycle ([`hydrology`](super::hydrology)) builds the mesh once and
//! solves each month along the month's recorded winds.
//!
//! # The model
//!
//! Two vertically integrated water fluxes per unit width move with the wind, the vapour's `F_v`
//! and the cloud water's `F_c` (eq. 3), kg m⁻¹ s⁻¹ here. Vapour condenses against the cloud
//! water's re-evaporation, S = −(`F_v` − β `F_c`) ÷ `L_c` (eq. 7); cloud water falls out at
//! P = `F_c` ÷ `L_f` (eq. 8); and a share ε of what falls evaporates straight back into the vapour
//! (eqs. 42–43), so that the rate that runs off is (1 − ε) P (eq. 44). In the steady state, with
//! a transverse dispersion of length `L_d` across the wind (eqs. 4–5), along the wind x and across
//! it y:
//!
//! ```text
//! −∂F_v/∂x + L_d ∂²F_v/∂y² − (F_v − β F_c) ÷ L_c + ε F_c ÷ L_f = 0    (eq. 42)
//! −∂F_c/∂x + L_d ∂²F_c/∂y² + (F_v − β F_c) ÷ L_c −   F_c ÷ L_f = 0    (eq. 43)
//! ```
//!
//! Height enters through the re-evaporation coefficient, β = β₀ e^(−H/H₀) (eq. 16), and the
//! recycled share, ε = ε₀ e^(−H/H₀) (eq. 49), H the cell's height above the sea level, a negative
//! height read as 0. β₀ is set by the long-range length `L_l`, β₀ = (1 − `L_c` ÷ `L_l`)(`L_l` ÷
//! `L_f` − 1) (eq. 33; eq. 32 expanded reads `L_l` ÷ `L_f` + `L_c` ÷ `L_l` − φ − 1, the same),
//! 18.05 at the set below. This β is the cloud water's re-evaporation, not the evaporation's
//! net-radiation share that decision-r09-t13ge §2.6 also calls β: it is named
//! [`Parameters::reevaporation`] here.
//!
//! The wind's speed drops out. The model's lengths are the speed times its time constants
//! (`L_c` = v `τ_c` and `L_f` = v `τ_f`, eqs. 7–8; the diffusivity `L_d` v, eq. 4), the set
//! fixes the lengths, and with them fixed eqs. 42–43 hold no speed: the steady state reads the
//! wind's direction alone, and whether the air moves at all ([`CellWind`]).
//!
//! # The set
//!
//! Hergarten and Robl's §7 set for the India–Asia collision zone (the Ghats, the Himalaya and the
//! Tibetan Plateau, on a 1 km grid, against `TRMM2b31` and `WorldClim` 2.1): `L_c` = `L_f` = 25 km,
//! `L_l` = 500 km, ε₀ = 0.75, H₀ = 2 km, `L_d` = 25 km. They chose it "as reasonable values" and
//! found that "a formal fit would not be very useful"; it is not a South American fit, which plan
//! 09's Design note 8 called it (decision-r09-t13ge §2.1, e1). One set serves every world. With
//! ε₀ the effective long-range length is `L̃_l` = `L_c` ÷ λ̃₋ = 2,004 km (eqs. 24, 45–47;
//! [`Parameters::effective_long_range_length_m`]), about `L_l` ÷ (1 − ε₀) (eq. 48), which the
//! inland decay follows.
//!
//! # The sea
//!
//! The sea is the boundary (their §7: "Ocean areas were considered to be boundaries, where a
//! uniform influx was assumed"). A sea cell's air carries the total flux F = `L_l` `P_ls` per unit
//! width, `P_ls` its large-scale rate, split in the long-range mode as `F_c` = F `L_f` ÷ `L_l` and
//! `F_v` = F − `F_c` (eq. 31), so that the model's own rate over it, `F_c` ÷ `L_f`, is `P_ls`.
//! Each land cell takes from an upwind sea neighbour what that cell's air carries across their
//! shared edge, and exchanges with it by dispersion against that boundary value; what a land cell
//! passes to a sea neighbour leaves the land. Sea cells' rates are not this model's, and it returns
//! zero over them. The sea is the cells [`Cover::Sea`] names, `seas::under_sea`'s.
//!
//! # The discretisation
//!
//! First-order upwind, implicit and conservative on the cell graph, each cell's value its
//! outflow's (backward Euler along the wind).
//!
//! - A cell's outflow, Q = F W in kg s⁻¹ for each component, leaves through its downwind edges in
//!   proportion to (ŵ · n̂)⁺ ℓ, ŵ the wind's unit direction, n̂ the edge's outward unit normal in
//!   the cell's tangent plane (its great circle's pole, projected) and ℓ its length. W, the sum of
//!   (ŵ · n̂)⁺ ℓ over the cell's edges, is its width across the wind (a convex cell's projected
//!   width), and Δs = A ÷ W, A its area, the air's mean path through it.
//! - Its balance is `(I + (Δs ÷ L_c) Ã + δ I) Q = Q_in + Σ κ f_j`, Ã = [[1, −β̃], [−1, β̃ + φ̃]]
//!   with β̃ = β + εφ, φ̃ = (1 − ε)φ and φ = `L_c` ÷ `L_f` (eqs. 18, 45–47), and its rate
//!   P = `Q_c` ÷ (W `L_f`) (eq. 8). On a chain of cells along the wind, Δs = Δx and this is their
//!   eq. 19.
//! - The matrix has a positive diagonal and non-positive off-diagonal entries, and its determinant,
//!   1 + h(1 + β + φ) + h² φ(1 − ε) at δ = 0 (h = Δs ÷ `L_c`), is positive, so its inverse is
//!   non-negative at every Δs ÷ L: no flux goes negative however long a cell is against `L_c` and
//!   `L_f` (25 km against cells of 23.6–42.4 km at level 8 and 47–85 km at level 7). The scheme's
//!   error is first order: a mode of length L decays over Δs ÷ ln(1 + Δs ÷ L) ≈ L + Δs ÷ 2.
//! - The dispersion across each edge is κ (`f_j` − `f_i`), f = Q ÷ W the per-width flux and
//!   κ = `L_d` (ℓ ÷ d) ½(`s_i` + `s_j`), d the distance between the centres and s = 1 − (ŵ · n̂)²
//!   the share of the edge's normal that lies across each cell's wind: the two-point flux of
//!   `L_d` ∂²F/∂y², which on square cells along the wind is eq. 19's
//!   `L_d`(`F_y−1` − 2F + `F_y+1`). It acts only between two cells whose air moves (the
//!   diffusivity is `L_d` v): a sea cell's per-width flux is its boundary value, and calm air
//!   disperses nothing.
//! - A land cell whose air does not move, or that has no downwind edge, passes nothing on: in the
//!   steady state its column loses all that enters as runoff, (1 − ε) P A = `Q_in`, the recycled
//!   ε P raining again in place (eqs. 42–44 with no outflow).
//! - An edge is downwind only if ŵ · n̂ exceeds [`DOWNWIND_TOLERANCE`], so that a wind along an
//!   edge, which rounding tilts either way, makes no pair of cells each the other's downwind.
//!
//! # The order and the solve
//!
//! Each month's land cells are solved in the order of the strongly connected components of its
//! upwind graph, by sweeps accelerated by restarted GMRES until one further sweep moves no land
//! cell's rate by more than 10⁻⁴ of it (decision-r09-t13e-lfpm-convergence, which amends
//! decision-r09-t13ge §2.3, e3):
//!
//! - **The order** ([`upwind_components`]). The month's upwind graph has an edge from each land
//!   cell whose air moves on to each land neighbour across an edge with (ŵ · n̂)⁺ ℓ > 0. Its
//!   strongly connected components are Tarjan's (Tarjan, R. 1972, "Depth-First Search and Linear
//!   Graph Algorithms", SIAM J. Comput. 1, 146–160), found without recursion, so that a level-8
//!   continent cannot exhaust a WebAssembly stack: roots in cell-index order, each cell's edges in
//!   [`Edge::ALL`]'s. The components go in the reverse of Tarjan's completion order, a topological
//!   order of the condensation, and each one's members in index order. Every edge between two
//!   components then points forward, so that nothing downstream of a cycle is solved before it. A
//!   cycle is a closed loop of a zonal wind, two cells blowing into each other across a
//!   convergence line, or two east–west neighbours whose side edges lean together as the meridians
//!   converge (plan 09's E-L2).
//! - **A sweep** ([`sweep`]) solves the components in turn, every other cell at its latest value.
//!   A single cell's balance is solved for its own outflow. A component of 2 to
//!   [`EXACT_BLOCK_CELLS`] cells has its whole balance, `M_i` `Q_i` − Σ `pull_ij` `Q_j` over its
//!   members, solved at once by Gaussian elimination in member order without pivoting, factorised
//!   once a month. That needs no pivoting because the block is a column diagonally dominant
//!   M-matrix: each column's pulls to other cells sum to at most 1 + `δ_j`. A larger component's
//!   cells take one Gauss–Seidel pass in index order. So a sweep is a fixed affine map,
//!   F(x) = G x + c, of x, the vapour and cloud-water outflows of the land cells whose air moves
//!   on.
//! - **The start** is one sweep without the dispersion from no air. With exact blocks it is the
//!   exact advective solution wherever no component exceeds the limit (a month without cycles, or
//!   with small ones, needs nothing more where the wind carries no dispersion).
//! - **The iteration** is restarted GMRES([`RESTART`]) on (I − G) x = c, c = F(0), F one sweep with
//!   the dispersion (Saad, Y., and Schultz, M. H. 1986, "GMRES: A Generalized Minimal Residual
//!   Algorithm for Solving Nonsymmetric Linear Systems", SIAM J. Sci. Stat. Comput. 7, 856–869),
//!   by modified Gram–Schmidt and Givens rotations ([`cycle`]). It removes the dispersion's lag:
//!   a plain sweep keeps a crosswind error smooth over many cells by about κ' ÷ (κ' + Δs ÷ `L̃_l`),
//!   κ' = `L_d` ÷ Δx, 0.98 at level 8, so that plain sweeps converge over about twice a
//!   continent's length in cells (plan 09's E-L1).
//! - **The stop**, at each restart: its residual sweep, F(x) − x, moves no land cell's rate (sinks
//!   included) by more than [`TOLERANCE`] of max(its rate, [`RATE_FLOOR_SHARE`] `P̄_sea`), `P̄_sea`
//!   the month's area mean of the sea's large-scale rate over its sea cells. The month keeps that
//!   sweep's result. A month with no land whose air moves on, or none of the sea's air reaching
//!   it (‖c‖ = 0), stops at the start. The cap is [`MAX_RESTARTS`] restarts. A month that reaches
//!   it keeps its last sweep and is flagged as not converged ([`Diagnostics`]); it never panics.
//!
//! Every loop runs in a fixed order (the cells', the components', each component's members', the
//! edges' in [`Edge::ALL`]'s, and the unknowns' in cell-index order, vapour then cloud water), in
//! `f64`, every sum and product in that order with no fused multiply–add, every exponential
//! through [`crate::math`], with no `HashMap` and no threads. The work that decides the order is
//! integer work. The iteration count varies between worlds but is a function of the inputs alone,
//! and no month starts from another's.
//!
//! [`Edge::ALL`]: hyperion_surface::cube::Edge::ALL

use hyperion_surface::cube::PATCH_QUADS;
use hyperion_surface::num::assert_finite;

use crate::math;
use crate::planetary::surface::grid::CellGraph;
use crate::units::Metres;

#[cfg(test)]
mod tests;

/// `L_c`, the length over which vapour condenses into cloud water, the wind speed times the
/// condensation time (eq. 7), m: 25 km, Hergarten and Robl 2022's §7 set for the India–Asia
/// collision zone, chosen "as reasonable values" and not fitted (decision-r09-t13ge §2.1, e1).
/// Medium confidence there and low elsewhere: one set for every world.
pub(super) const CONDENSATION_LENGTH_M: f64 = 25_000.0;

/// `L_f`, the length over which cloud water falls out, the wind speed times the fallout time
/// (eq. 8), m: 25 km, their §7 set, as [`CONDENSATION_LENGTH_M`].
pub(super) const FALLOUT_LENGTH_M: f64 = 25_000.0;

/// `L_l`, the long-range transport length of moisture over level ground with no recycling
/// (eq. 25), m: 500 km, their §7 set, as [`CONDENSATION_LENGTH_M`]; it fixes β₀ (eq. 33). They
/// cite Makarieva et al. 2009's decay of about 600 km inland over unforested land as its order of
/// magnitude (§4.1; as they report it).
pub(super) const LONG_RANGE_LENGTH_M: f64 = 500_000.0;

/// `L_d`, the dispersion length, the diffusivity across the wind over the wind speed (eq. 4), m:
/// 25 km, their §7 set, as [`CONDENSATION_LENGTH_M`].
pub(super) const DISPERSION_LENGTH_M: f64 = 25_000.0;

/// ε₀, the share of the precipitation at the sea level that evaporates straight back into the air
/// (eqs. 42–44, 49): 0.75, their §7's "rather high evaporation ratio", with which, they note,
/// lower ratios with a longer `L_l` give similar results. With ε(H) it gives the Earth's land a
/// runoff ratio of 0.425 over its hypsometry, against about 0.36–0.40 (decision-r09-t13ge §2.1,
/// the latter from memory).
pub(super) const SEA_LEVEL_RECYCLED_SHARE: f64 = 0.75;

/// H₀, the height over which the re-evaporation β and the recycled share ε fall by a factor e
/// (eqs. 15–16, 49), m: 2 km, their §7 set (decision-r09-t13ge §2.1, e1).
///
/// It is water's on the Earth, a closed figure for the composition audit. Their eqs. 12–15 give
/// H₀ = T₀² ÷ (a Γ) for β ∝ e^(−a/T); a condensable's Clausius–Clapeyron slope a = M L ÷ R makes
/// it R T₀² ÷ (M L Γ), 2.7 km for the Earth's water at 288 K and 5.8 K km⁻¹ (its latent heat
/// there 2.47 MJ kg⁻¹, from memory).
pub(super) const HEIGHT_SCALE_M: f64 = 2_000.0;

/// τ, the stop test's share (decision-r09-t13e-lfpm-convergence §3.2 item 5): a month's solve
/// stops at the first restart where one further sweep moves no land cell's rate by more than
/// 10⁻⁴ of max(its rate, [`RATE_FLOOR_SHARE`] of the month's mean sea rate). The ruling's target
/// is 1% a land cell, monthly and annual: that keeps a threshold on a cell's rate, R10's arid bound
/// and R09.T15's Köppen–Geiger classes among them, within about one level-8 field cell where the
/// rain decays inland over `L̃_l` (its §2). In the ruling's 2-D replica of this discretisation the
/// worst cell's true error was at most 26 times this measure over every restart of its
/// component-ordered runs (39 times under Kahn's order), and at 10⁻⁴ its worst cell was 0.16% off:
/// a factor of 6 inside the target, for the reference Earth's longer continents and stronger
/// cycles. The ruling's figure, medium confidence; not loosened to save time (its §3.4).
pub(super) const TOLERANCE: f64 = 1e-4;

/// The floor of the stop test's scale as a share of `P̄_sea`, the month's area-mean large-scale
/// rate over its sea cells: 10⁻³ (decision-r09-t13e-lfpm-convergence §3.2 item 5), so that a land
/// cell all but dry is measured against a thousandth of the sea's rate, not against its own.
pub(super) const RATE_FLOOR_SHARE: f64 = 1e-3;

/// GMRES's restart length, the Krylov steps between two restarts: 10, the ruling's GMRES(10)
/// (decision-r09-t13e-lfpm-convergence §3.2 item 4; the method Saad and Schultz 1986's, the
/// [module documentation](self)'s citation, whose bibliographic data the ruling checked against
/// Crossref). Each step is one sweep; the basis is 11 vectors of the unknowns, about 20 MB at
/// level 8 on the reference Earth, one month at a time.
pub(super) const RESTART: usize = 10;

/// The restarts a month's solve takes at most: 30, about 330 sweep-equivalents
/// (decision-r09-t13e-lfpm-convergence §3.2 item 6). A month that reaches it keeps its last sweep
/// and is flagged as not converged ([`Diagnostics::converged`]); it never panics, so that an odd
/// world still generates. In the ruling's replica the stop came after 47–113 sweep-equivalents. On
/// the reference Earth through the coarse pass (level 8, its generated continents and R09.T12.e's
/// sea) every month stops after 20–27 restarts (223–300 sweep-equivalents), and on R09.T13.g's
/// synthetic Earth at level 7 after 8–13: 19–25% of the coarse pass's land lies in components
/// larger than [`EXACT_BLOCK_CELLS`], the largest 11,333 cells, which the Gauss–Seidel pass leaves
/// to the Krylov iteration (plan 09's deferred finding E-L8).
pub(super) const MAX_RESTARTS: u32 = 30;

/// The largest strongly connected component of a month's upwind graph that a sweep solves
/// exactly, cells: 16, at most 32 unknowns (decision-r09-t13e-lfpm-convergence §3.2 item 2). A
/// larger one takes one Gauss–Seidel pass over its members in index order, which keeps the sweep
/// a fixed linear map; the Krylov iteration absorbs the rest.
pub(super) const EXACT_BLOCK_CELLS: usize = 16;

/// The share τ of the tests' deep solves' stop test (decision-r09-t13e-lfpm-convergence §3.5: "a
/// deep solve's (`with_tolerance`, relative residual 10⁻¹²)"), the reference the production solve
/// is held to; each must meet the stop test at [`DEEP_MEASURE`] at least.
#[cfg(test)]
pub(super) const DEEP_TOLERANCE: f64 = 1e-12;

/// The stop test's measure a deep solve must meet whether or not rounding lets it reach
/// [`DEEP_TOLERANCE`] (the ruling's "the deep solve meets the stop test at 10⁻¹⁰").
#[cfg(test)]
pub(super) const DEEP_MEASURE: f64 = 1e-10;

/// The restarts a deep solve may take: 400, against the 41–53 the reference Earth's months take.
#[cfg(test)]
pub(super) const DEEP_RESTARTS: u32 = 400;

/// The least ŵ · n̂ at which an edge is downwind: a wind within 10⁻⁹ rad of an edge's direction
/// passes nothing across it. Far above the rounding of a unit normal (about 10⁻¹⁶), so that a
/// wind along an edge makes no two cells each the other's downwind, and far below any share of
/// the air that matters.
pub(super) const DOWNWIND_TOLERANCE: f64 = 1e-9;

/// The model's lengths and recycling and the solve's stop: Hergarten and Robl's §7 set with the
/// ruled tolerance and cap ([`Parameters::HERGARTEN_ROBL`]), or a variant of it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Parameters {
    condensation_length_m: f64,
    fallout_length_m: f64,
    long_range_length_m: f64,
    dispersion_length_m: f64,
    sea_level_recycled_share: f64,
    height_scale_m: f64,
    tolerance: f64,
    max_restarts: u32,
}

impl Parameters {
    /// Hergarten and Robl 2022's §7 set (decision-r09-t13ge §2.1, e1), solved to [`TOLERANCE`]
    /// within [`MAX_RESTARTS`] restarts (decision-r09-t13e-lfpm-convergence).
    pub(super) const HERGARTEN_ROBL: Self = Self {
        condensation_length_m: CONDENSATION_LENGTH_M,
        fallout_length_m: FALLOUT_LENGTH_M,
        long_range_length_m: LONG_RANGE_LENGTH_M,
        dispersion_length_m: DISPERSION_LENGTH_M,
        sea_level_recycled_share: SEA_LEVEL_RECYCLED_SHARE,
        height_scale_m: HEIGHT_SCALE_M,
        tolerance: TOLERANCE,
        max_restarts: MAX_RESTARTS,
    };

    /// The parameters with the stop test's share τ = `tolerance`, for the tests' deep solves.
    ///
    /// # Panics
    ///
    /// If `tolerance` is not finite and positive.
    #[cfg(test)]
    #[must_use]
    pub(super) fn with_tolerance(self, tolerance: f64) -> Self {
        assert!(
            tolerance.is_finite() && tolerance > 0.0,
            "a tolerance is finite and positive, got {tolerance}"
        );
        Self { tolerance, ..self }
    }

    /// A deep solve's parameters: the set solved to [`DEEP_TOLERANCE`] within [`DEEP_RESTARTS`]
    /// restarts (decision-r09-t13e-lfpm-convergence §3.5).
    #[cfg(test)]
    #[must_use]
    pub(super) fn deep() -> Self {
        Self::HERGARTEN_ROBL
            .with_tolerance(DEEP_TOLERANCE)
            .with_max_restarts(DEEP_RESTARTS)
    }

    /// The parameters with at most `restarts` restarts a month: fewer, for the cap's test, or
    /// more, for a deep solve.
    #[cfg(test)]
    #[must_use]
    pub(super) const fn with_max_restarts(self, restarts: u32) -> Self {
        Self {
            max_restarts: restarts,
            ..self
        }
    }

    /// The parameters with the recycled share at the sea level ε₀ = `share`.
    ///
    /// # Panics
    ///
    /// If `share` is not in [0, 1): at 1 nothing would run off and the air would never dry.
    #[cfg(test)]
    #[must_use]
    pub(super) fn with_sea_level_recycled_share(self, share: f64) -> Self {
        assert!(
            (0.0..1.0).contains(&share),
            "a recycled share is in [0, 1), got {share}"
        );
        Self {
            sea_level_recycled_share: share,
            ..self
        }
    }

    /// The stop test's share τ.
    #[cfg(test)]
    #[must_use]
    pub(super) const fn tolerance(&self) -> f64 {
        self.tolerance
    }

    /// The restarts a month's solve takes at most.
    #[cfg(test)]
    #[must_use]
    pub(super) const fn max_restarts(&self) -> u32 {
        self.max_restarts
    }

    /// φ = `L_c` ÷ `L_f` (eq. 18).
    #[must_use]
    fn length_ratio(&self) -> f64 {
        self.condensation_length_m / self.fallout_length_m
    }

    /// β₀, the cloud water's re-evaporation coefficient at the sea level, from the long-range
    /// length: (1 − `L_c` ÷ `L_l`)(`L_l` ÷ `L_f` − 1) (eq. 33), 18.05 at the set.
    #[must_use]
    pub(super) fn sea_level_reevaporation(&self) -> f64 {
        (1.0 - self.condensation_length_m / self.long_range_length_m)
            * (self.long_range_length_m / self.fallout_length_m - 1.0)
    }

    /// e^(−H ÷ H₀) at `height_m` above the sea level, a negative height read as 0 (eqs. 16, 49).
    #[must_use]
    fn height_factor(&self, height_m: f64) -> f64 {
        let height_m = if height_m > 0.0 { height_m } else { 0.0 };
        math::exp(-height_m / self.height_scale_m)
    }

    /// β, the cloud water's re-evaporation coefficient at `height_m` above the sea level,
    /// β₀ e^(−H/H₀) (eq. 16).
    #[must_use]
    pub(super) fn reevaporation(&self, height_m: f64) -> f64 {
        self.sea_level_reevaporation() * self.height_factor(height_m)
    }

    /// ε, the share of the precipitation at `height_m` above the sea level that evaporates back
    /// into the air, ε₀ e^(−H/H₀) (eq. 49).
    #[must_use]
    pub(super) fn recycled_share(&self, height_m: f64) -> f64 {
        self.sea_level_recycled_share * self.height_factor(height_m)
    }

    /// `L̃_l`, the effective long-range length at the sea level with the recycling, m: `L_c` ÷ λ̃₋,
    /// λ̃₋ the smaller root of λ² − (1 + β̃ + φ̃)λ + φ̃ = 0 (eqs. 23–24) with β̃ = β₀ + ε₀φ and
    /// φ̃ = (1 − ε₀)φ (eqs. 45–47): 2,003.8 km at the set, about `L_l` ÷ (1 − ε₀) (eq. 48). It is
    /// taken as `L_c` λ̃₊ ÷ φ̃, since λ̃₋ λ̃₊ = φ̃ (eq. 27's product), which avoids eq. 24's
    /// cancellation.
    #[cfg(test)]
    #[must_use]
    pub(super) fn effective_long_range_length_m(&self) -> f64 {
        let phi = self.length_ratio();
        let share = self.sea_level_recycled_share;
        let feedback = self.sea_level_reevaporation() + share * phi;
        let fallout = (1.0 - share) * phi;
        let half = 0.5 * (1.0 + feedback + fallout);
        let larger = half + (half * half - fallout).sqrt();
        self.condensation_length_m * larger / fallout
    }

    /// The sea's air per unit width at the large-scale rate `rate_kg_m2_s`, kg m⁻¹ s⁻¹, vapour
    /// then cloud water: the total `L_l` `P_ls` in the long-range mode, `F_c` = F `L_f` ÷ `L_l`
    /// and `F_v` = F − `F_c` (eq. 31).
    #[must_use]
    fn sea_air(&self, rate_kg_m2_s: f64) -> [f64; 2] {
        let total = self.long_range_length_m * rate_kg_m2_s;
        let cloud = total * self.fallout_length_m / self.long_range_length_m;
        [total - cloud, cloud]
    }
}

/// What covers a cell, as this model reads it: land, where it rains, or the sea, its boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Cover {
    /// Ground, where the model rains.
    Land,
    /// Liquid ([`seas::under_sea`](super::seas::under_sea)): the boundary, whose air arrives at
    /// the sea's large-scale rate.
    Sea,
}

/// A cell's wind in a month, as this model reads it: its direction, or still air. Its speed drops
/// out of the steady state (the [module documentation](self)).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum CellWind {
    /// Still air: the cell passes nothing on and disperses nothing.
    Calm,
    /// Air moving towards a direction along the surface at the cell's centre, a vector in the
    /// body-fixed frame of any non-zero length; its part tangent to the surface is taken and
    /// normalised.
    Towards([f64; 3]),
}

/// One of a cell's edges, as the model reads it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Face {
    /// The cell across the edge.
    neighbour: u32,
    /// Which of the neighbour's faces this edge is, an index into its four.
    back: u8,
    /// The edge's outward unit normal in the cell's tangent plane.
    normal: [f64; 3],
    /// The edge's length, m.
    length_m: f64,
    /// The edge's length over the distance between the two cells' centres.
    conductance: f64,
}

/// The cells' geometry the model reads: each cell's area and up direction, and each of its four
/// edges' neighbour, outward normal, length and length over the centres' distance, in cell-index
/// order. Built once per field ([`Mesh::of_graph`]) and read every month.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Mesh {
    areas_m2: Vec<f64>,
    ups: Vec<[f64; 3]>,
    faces: Vec<[Face; 4]>,
}

impl Mesh {
    /// The geometry of `graph`'s cells on a sphere of `radius`: each cell's area its solid angle
    /// times R², its up direction its centre, and its edges in [`Edge::ALL`]'s order, each the
    /// great-circle arc between the cell's corners on it (the patch's corner vertices, whose bits
    /// the cells that share them share), its normal the arc's pole turned away from the cell's
    /// centre and projected into the cell's tangent plane.
    ///
    /// [`Edge::ALL`]: hyperion_surface::cube::Edge::ALL
    ///
    /// # Panics
    ///
    /// If `radius` is not finite and positive.
    #[must_use]
    pub(super) fn of_graph(graph: &CellGraph, radius: Metres) -> Self {
        let radius_m = radius.value();
        assert!(
            radius_m.is_finite() && radius_m > 0.0,
            "a body's radius is finite and positive, got {radius_m} m"
        );
        let last = u8::try_from(PATCH_QUADS).expect("a patch has 64 quads a side");
        let count = slot(graph.len());
        let mut areas_m2 = Vec::with_capacity(count);
        let mut faces = Vec::with_capacity(count);
        for n in 0..graph.len() {
            let (cell, centre) = (graph.cell(n), graph.centre(n));
            let corners =
                [(0, 0), (last, 0), (0, last), (last, last)].map(|(x, y)| cell.vertex_dir(x, y));
            // Edge::ALL's order: u = 0 (vertex column 0), u = 1, v = 0 (vertex row 0), v = 1.
            let ends = [(0, 2), (1, 3), (0, 1), (2, 3)];
            let neighbours = graph.edge_neighbours(n);
            areas_m2.push(graph.solid_angle_sr(n) * radius_m * radius_m);
            faces.push(core::array::from_fn(|k| {
                let ((a, b), across) = ((corners[ends[k].0], corners[ends[k].1]), neighbours[k]);
                let pole = cross(a, b);
                let outward = if dot(pole, centre) < 0.0 {
                    pole
                } else {
                    scale(pole, -1.0)
                };
                let edge_rad = arc(a, b);
                let back = graph
                    .edge_neighbours(across)
                    .iter()
                    .position(|&m| m == n)
                    .expect("the graph's edge neighbours are mirrored");
                Face {
                    neighbour: across,
                    back: u8::try_from(back).expect("a cell has four edges"),
                    normal: unit(tangent(outward, centre)),
                    length_m: edge_rad * radius_m,
                    conductance: edge_rad / graph.arc(n, across).value(),
                }
            }));
        }
        Self {
            areas_m2,
            ups: graph.centres().to_vec(),
            faces,
        }
    }

    /// The number of cells.
    ///
    /// # Panics
    ///
    /// Never for a mesh of a coarse level, which has under 2³² cells.
    #[cfg(test)]
    #[must_use]
    pub(super) fn len(&self) -> u32 {
        u32::try_from(self.areas_m2.len()).expect("a mesh has under 2^32 cells")
    }
}

/// One month's inputs to [`solve`], each slice one value a cell in the mesh's cell order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Month<'a> {
    /// Each cell's height above the sea level, m: its water surface less the sea level; a
    /// negative height reads as 0.
    pub(super) heights_m: &'a [f64],
    /// What covers each cell.
    pub(super) cover: &'a [Cover],
    /// Each cell's wind.
    pub(super) winds: &'a [CellWind],
    /// The large-scale precipitation rate over each sea cell, kg m⁻² s⁻¹, non-negative; read over
    /// the sea alone.
    pub(super) sea_rates_kg_m2_s: &'a [f64],
}

/// A month's orographic precipitation: each cell's rate and recycled share, and what the solve
/// did.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Orographic {
    /// Each cell's precipitation rate P, kg m⁻² s⁻¹, non-negative: the land's, and zero over the
    /// sea, whose rate is not this model's.
    pub(super) precipitation_kg_m2_s: Vec<f64>,
    /// Each cell's recycled share ε(H) (eq. 49), so that its runoff-generating rate is (1 − ε) P
    /// (eq. 44): the land's, and zero over the sea.
    pub(super) recycled_shares: Vec<f64>,
    /// What the solve did that is no output.
    pub(super) diagnostics: Diagnostics,
}

/// A month's strongly connected components by size (decision-r09-t13e-lfpm-convergence §3.2
/// item 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(super) struct ComponentSizes {
    /// The components of one cell.
    pub(super) single: u32,
    /// The components of two cells.
    pub(super) pairs: u32,
    /// The components of 3 to [`EXACT_BLOCK_CELLS`] cells.
    pub(super) small: u32,
    /// The components of more than [`EXACT_BLOCK_CELLS`] cells, each swept by Gauss–Seidel.
    pub(super) large: u32,
    /// The cells in those large components.
    pub(super) large_cells: u32,
    /// The largest component's cells; zero on a month of no land.
    pub(super) largest: u32,
}

/// What a month's solve did that is no output (decision-r09-t13e-lfpm-convergence §3.2 item 7):
/// its iteration, its components, and the land's water budget at its final state.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Diagnostics {
    /// The GMRES restarts the solve took, at most its parameters' cap.
    pub(super) restarts: u32,
    /// Its sweep-equivalents: the dispersion-off start, the sea's own sweep c = F(0), each
    /// restart's residual sweep and each Krylov step's.
    pub(super) sweeps: u32,
    /// The stop test's measure at the last restart: the largest change of a land cell's rate in
    /// the restart's residual sweep over max(its rate, [`RATE_FLOOR_SHARE`] `P̄_sea`); zero where
    /// the solve stopped at the start.
    pub(super) measure: f64,
    /// Whether the measure met the tolerance within the cap; a month that reached the cap keeps
    /// its last sweep.
    pub(super) converged: bool,
    /// ‖F(x) − x‖₂ ÷ ‖c‖₂ at the last restart, over the unknowns; zero where the solve stopped at
    /// the start.
    pub(super) relative_residual: f64,
    /// The month's components by size.
    pub(super) components: ComponentSizes,
    /// `P̄_sea`, the area mean of the sea's large-scale rate over the month's sea cells,
    /// kg m⁻² s⁻¹; zero on a world of no sea.
    pub(super) sea_mean_kg_m2_s: f64,
    /// The land cells whose final rate came out below zero and was read as zero: GMRES's iterate
    /// need not be non-negative, though the converged solve is, so a cell whose rate lies within
    /// the stop test's tolerance of zero can end a rounding below it.
    pub(super) negative_cells: u32,
    /// The area-weighted mean rate over every cell, the land's P and the sea's `P_ls`,
    /// kg m⁻² s⁻¹.
    pub(super) global_mean_kg_m2_s: f64,
    /// The land cells.
    pub(super) land_cells: u32,
    /// The water the land's air took in from the sea, by the wind and by dispersion, kg s⁻¹.
    pub(super) inflow_kg_s: f64,
    /// The precipitation that ran off, Σ (1 − ε) P A over the land, kg s⁻¹.
    pub(super) runoff_kg_s: f64,
    /// The precipitation that evaporated back into the air, Σ ε P A over the land, kg s⁻¹.
    pub(super) recycled_kg_s: f64,
    /// The water the land's air passed to the sea, kg s⁻¹.
    pub(super) outflow_kg_s: f64,
}

/// The runoff-generating rate (1 − ε(H)) P of a cell under precipitation `precipitation_kg_m2_s`,
/// kg m⁻² s⁻¹, at `height_m` above the sea level, at Hergarten and Robl's set (eqs. 44, 49;
/// decision-r09-t13ge §2.8, e8): applied to a cell's annual mean rate, the annual runoff R09.T14
/// reads, ε depending on the height alone. Zero over liquid, and on a dry world, whose rate is
/// zero. The recycled ε P is the land's evapotranspiration, which the global water balance counts
/// as evaporation.
///
/// # Panics
///
/// If the rate is negative or not finite, or the height not finite.
#[must_use]
pub(crate) fn runoff_kg_m2_s(precipitation_kg_m2_s: f64, height_m: f64, cover: Cover) -> f64 {
    assert!(
        precipitation_kg_m2_s.is_finite() && precipitation_kg_m2_s >= 0.0,
        "a precipitation rate is finite and non-negative, got {precipitation_kg_m2_s} kg m^-2 s^-1"
    );
    let height_m = assert_finite(height_m);
    match cover {
        Cover::Land => {
            (1.0 - Parameters::HERGARTEN_ROBL.recycled_share(height_m)) * precipitation_kg_m2_s
        }
        Cover::Sea => 0.0,
    }
}

/// How a cell takes part in a month's solve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Sea under moving air: the boundary, which gives air and disperses against its own.
    Sea,
    /// Sea under still air: it gives nothing.
    CalmSea,
    /// Land whose air moves on.
    Through,
    /// Land whose air goes nowhere: calm, or with no downwind edge.
    Sink,
}

impl Kind {
    /// Whether the cell is land.
    #[must_use]
    const fn is_land(self) -> bool {
        match self {
            Self::Through | Self::Sink => true,
            Self::Sea | Self::CalmSea => false,
        }
    }

    /// Whether the cell's air moves, and so disperses.
    #[must_use]
    const fn moves(self) -> bool {
        match self {
            Self::Sea | Self::Through => true,
            Self::CalmSea | Self::Sink => false,
        }
    }
}

/// A cell's wind against its edges in one month.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Local {
    kind: Kind,
    /// ŵ · n̂ at each edge; zero under still air.
    dots: [f64; 4],
    /// W, the cell's width across the wind, Σ (ŵ · n̂)⁺ ℓ over its downwind edges, m.
    width_m: f64,
}

impl Local {
    /// Cell `cell`'s of `mesh` under `wind`, covered by `cover`.
    ///
    /// # Panics
    ///
    /// If the wind is not finite or has no direction along the surface.
    #[must_use]
    fn of(mesh: &Mesh, cell: usize, cover: Cover, wind: CellWind) -> Self {
        let still = |kind| Self {
            kind,
            dots: [0.0; 4],
            width_m: 0.0,
        };
        let towards = match wind {
            CellWind::Calm => {
                return still(match cover {
                    Cover::Land => Kind::Sink,
                    Cover::Sea => Kind::CalmSea,
                });
            }
            CellWind::Towards(towards) => towards,
        };
        let along = tangent(towards, mesh.ups[cell]);
        let length = norm(along);
        assert!(
            towards.iter().all(|c| c.is_finite()) && length > 0.0,
            "a wind has a finite direction along the surface, got {towards:?}"
        );
        let direction = scale(along, 1.0 / length);
        let faces = &mesh.faces[cell];
        let dots = faces.map(|face| dot(direction, face.normal));
        let width_m = dots
            .iter()
            .zip(faces)
            .fold(0.0, |sum, (&d, face)| sum + downwind(d) * face.length_m);
        let kind = match cover {
            Cover::Sea => Kind::Sea,
            Cover::Land if width_m > 0.0 => Kind::Through,
            Cover::Land => Kind::Sink,
        };
        Self {
            kind,
            dots,
            width_m,
        }
    }
}

/// A land cell's balance, as a sweep solves it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Row {
    /// The cell.
    cell: u32,
    /// Its neighbours, whose latest outflows it reads.
    neighbours: [u32; 4],
    /// The share of each neighbour's outflow, each component, that the cell takes in: by the
    /// wind, and by the dispersion over the neighbour's width.
    pulls: [f64; 4],
    /// Each neighbour's place among the members of the cell's component where a sweep solves that
    /// component whole, else [`OUTSIDE`].
    block: [u8; 4],
    /// What the sea gives it by the wind and by dispersion, kg s⁻¹, vapour then cloud water.
    sea_kg_s: [f64; 2],
    /// How its rate follows from its inflow.
    rule: Rule,
}

/// A neighbour outside the cell's exactly solved component, or a cell whose component is not.
const OUTSIDE: u8 = u8::MAX;

/// How a land cell's rate follows from its inflow.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Rule {
    /// Its outflow is its matrix's inverse times its inflow (or, in an exact block, the block's
    /// solve with the matrix on its diagonal), and its rate its cloud water's outflow over
    /// W `L_f`.
    Through {
        matrix: [[f64; 2]; 2],
        inverse: [[f64; 2]; 2],
        rate_per_cloud: f64,
    },
    /// Its rate is its whole inflow over (1 − ε) A.
    Sink { rate_per_inflow: f64 },
}

/// Whether a sweep's balances carry the transverse dispersion: the start's do not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dispersion {
    Off,
    On,
}

/// What a sweep takes from the sea: its air, for F, or none, for the sweep's linear part G that
/// the Krylov steps apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    Sea,
    Nothing,
}

/// The share of an edge's normal along the wind that carries air out of a cell, (ŵ · n̂)⁺, zero
/// at or below [`DOWNWIND_TOLERANCE`].
#[must_use]
fn downwind(dot: f64) -> f64 {
    if dot > DOWNWIND_TOLERANCE { dot } else { 0.0 }
}

/// The share of an edge's normal across the wind, 1 − (ŵ · n̂)².
#[must_use]
fn across(dot: f64) -> f64 {
    1.0 - dot * dot
}

/// The cell matrix I + h Ã + δ I of a cell whose path over `L_c` is `path` (h), at `height_m`,
/// with dispersion diagonal `spread` (δ): Ã = [[1, −β̃], [−1, β̃ + φ̃]] (eqs. 18, 45–47).
#[must_use]
fn cell_matrix(parameters: &Parameters, path: f64, height_m: f64, spread: f64) -> [[f64; 2]; 2] {
    let phi = parameters.length_ratio();
    let share = parameters.recycled_share(height_m);
    let feedback = parameters.reevaporation(height_m) + share * phi;
    let fallout = (1.0 - share) * phi;
    [
        [1.0 + path + spread, -path * feedback],
        [-path, 1.0 + path * (feedback + fallout) + spread],
    ]
}

/// The inverse of a 2 × 2 matrix of positive determinant.
#[must_use]
fn inverse(m: [[f64; 2]; 2]) -> [[f64; 2]; 2] {
    let det = m[0][0] * m[1][1] - m[0][1] * m[1][0];
    [
        [m[1][1] / det, -m[0][1] / det],
        [-m[1][0] / det, m[0][0] / det],
    ]
}

/// The dispersion coefficient κ of face `face` between a cell of wind `local` and its neighbour
/// of wind `other`, m: `L_d` (ℓ ÷ d) ½(`s_i` + `s_j`) where both cells' air moves and the first is
/// land whose air moves on, else zero.
#[must_use]
fn spread_m(parameters: &Parameters, face: &Face, k: usize, local: &Local, other: &Local) -> f64 {
    if local.kind != Kind::Through || !other.kind.moves() {
        return 0.0;
    }
    let shares = f64::midpoint(
        across(local.dots[k]),
        across(other.dots[usize::from(face.back)]),
    );
    parameters.dispersion_length_m * face.conductance * shares
}

/// Cell `cell`'s balance, a land cell of `locals`, with or without the dispersion; its places in
/// a block all [`OUTSIDE`].
#[must_use]
fn row(
    mesh: &Mesh,
    month: &Month<'_>,
    parameters: &Parameters,
    locals: &[Local],
    cell: u32,
    dispersion: Dispersion,
) -> Row {
    let i = slot(cell);
    let (local, faces) = (&locals[i], &mesh.faces[i]);
    let mut pulls = [0.0; 4];
    let mut sea_kg_s = [0.0; 2];
    let mut spread = 0.0;
    for (k, face) in faces.iter().enumerate() {
        let j = slot(face.neighbour);
        let other = &locals[j];
        let kappa_m = match dispersion {
            Dispersion::On => spread_m(parameters, face, k, local, other),
            Dispersion::Off => 0.0,
        };
        let given = downwind(other.dots[usize::from(face.back)]) * face.length_m;
        match other.kind {
            Kind::Through => pulls[k] = (given + kappa_m) / other.width_m,
            Kind::Sea => {
                let air = parameters.sea_air(month.sea_rates_kg_m2_s[j]);
                for (sea, flux) in sea_kg_s.iter_mut().zip(air) {
                    *sea += flux * given + kappa_m * flux;
                }
            }
            Kind::CalmSea | Kind::Sink => {}
        }
        if local.kind == Kind::Through {
            spread += kappa_m / local.width_m;
        }
    }
    let height_m = month.heights_m[i];
    let rule = match local.kind {
        Kind::Through => {
            let path = mesh.areas_m2[i] / local.width_m / parameters.condensation_length_m;
            let matrix = cell_matrix(parameters, path, height_m, spread);
            Rule::Through {
                matrix,
                inverse: inverse(matrix),
                rate_per_cloud: 1.0 / (local.width_m * parameters.fallout_length_m),
            }
        }
        Kind::Sink => Rule::Sink {
            rate_per_inflow: 1.0 / ((1.0 - parameters.recycled_share(height_m)) * mesh.areas_m2[i]),
        },
        Kind::Sea | Kind::CalmSea => unreachable!("the components hold land cells alone"),
    };
    Row {
        cell,
        neighbours: faces.map(|face| face.neighbour),
        pulls,
        block: [OUTSIDE; 4],
        sea_kg_s,
        rule,
    }
}

/// Whether a sweep solves a component of `size` cells whole: 2 to [`EXACT_BLOCK_CELLS`].
#[must_use]
fn solved_whole(size: usize) -> bool {
    (2..=EXACT_BLOCK_CELLS).contains(&size)
}

/// A month's land balances, with or without the dispersion, in its components' order, with the
/// LU factors of each component a sweep solves whole.
#[derive(Debug, Clone, PartialEq)]
struct Balances {
    /// Each land cell's balance, in [`Components::cells`]' order.
    rows: Vec<Row>,
    /// Each component's offset into `factors`: where its block's factors start, for a component
    /// a sweep solves whole, and the next one's for any other.
    offsets: Vec<usize>,
    /// The blocks' LU factors, each block's 2n × 2n matrix row by row for its n members, each
    /// member's vapour then cloud water: L below the diagonal (its unit diagonal not stored), U
    /// on and above it.
    factors: Vec<f64>,
}

impl Balances {
    /// The balances of `locals`' land cells in `components`' order, with or without the
    /// dispersion, and their blocks' factors ([`factorise`]).
    #[must_use]
    fn new(
        mesh: &Mesh,
        month: &Month<'_>,
        parameters: &Parameters,
        locals: &[Local],
        components: &Components,
        dispersion: Dispersion,
    ) -> Self {
        // Each land cell's entry in the components' order.
        let mut entry = vec![usize::MAX; locals.len()];
        for (t, &cell) in components.cells.iter().enumerate() {
            entry[slot(cell)] = t;
        }
        let mut rows = Vec::with_capacity(components.cells.len());
        let mut offsets = Vec::with_capacity(components.len());
        let mut factors = Vec::new();
        for c in 0..components.len() {
            let range = components.range(c);
            let whole = solved_whole(range.len());
            for &cell in &components.cells[range.clone()] {
                let mut row = row(mesh, month, parameters, locals, cell, dispersion);
                if whole {
                    for (place, &j) in row.block.iter_mut().zip(&row.neighbours) {
                        let t = entry[slot(j)];
                        if range.contains(&t) {
                            *place = u8::try_from(t - range.start)
                                .expect("a block has at most EXACT_BLOCK_CELLS members");
                        }
                    }
                }
                rows.push(row);
            }
            offsets.push(factors.len());
            if whole {
                factorise(&rows[range], &mut factors);
            }
        }
        Self {
            rows,
            offsets,
            factors,
        }
    }
}

/// Appends to `factors` the LU factors of the balance of the block `rows`, a component a sweep
/// solves whole (decision-r09-t13e-lfpm-convergence §3.2 item 2): `M_i` `Q_i` − Σ `pull_ij` `Q_j`
/// over its members j, each member's vapour then cloud water in member order, by Doolittle's
/// elimination without pivoting. The block is a non-singular M-matrix and column diagonally
/// dominant (each column's pulls to other members sum to at most 1 + `δ_j`, and its in-cell term
/// is −h or −hβ̃), so its pivots stay positive and the elimination is stable.
///
/// # Panics
///
/// If a member is a sink, which has no downwind edge and so is a component of its own.
fn factorise(rows: &[Row], factors: &mut Vec<f64>) {
    let m = 2 * rows.len();
    let base = factors.len();
    factors.resize(base + m * m, 0.0);
    let a = &mut factors[base..];
    for (p, row) in rows.iter().enumerate() {
        let Rule::Through { matrix, .. } = row.rule else {
            unreachable!("a sink has no downwind edge, so it is a component of its own");
        };
        for (r, line) in matrix.iter().enumerate() {
            for (q, &value) in line.iter().enumerate() {
                a[(2 * p + r) * m + 2 * p + q] = value;
            }
        }
        for (&place, &pull) in row.block.iter().zip(&row.pulls) {
            if place != OUTSIDE {
                let b = usize::from(place);
                a[2 * p * m + 2 * b] -= pull;
                a[(2 * p + 1) * m + 2 * b + 1] -= pull;
            }
        }
    }
    for c in 0..m {
        let pivot = a[c * m + c];
        debug_assert!(pivot > 0.0, "a block's pivot is positive, got {pivot}");
        for r in c + 1..m {
            let l = a[r * m + c] / pivot;
            a[r * m + c] = l;
            for q in c + 1..m {
                a[r * m + q] -= l * a[c * m + q];
            }
        }
    }
}

/// The strongly connected components of a graph, in topological order.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Components {
    /// The nodes, each component's members contiguous and in index order, the components in a
    /// topological order of the condensation: every edge between two components points from the
    /// earlier to the later.
    cells: Vec<u32>,
    /// Each component's first entry in `cells`, then one past the last one's end: one more than
    /// the components.
    starts: Vec<u32>,
}

impl Components {
    /// The number of components.
    #[must_use]
    fn len(&self) -> usize {
        self.starts.len() - 1
    }

    /// Component `c`'s entries in [`Components::cells`].
    #[must_use]
    fn range(&self, c: usize) -> core::ops::Range<usize> {
        slot(self.starts[c])..slot(self.starts[c + 1])
    }

    /// The components by size (decision-r09-t13e-lfpm-convergence §3.2 item 7).
    #[must_use]
    fn sizes(&self) -> ComponentSizes {
        let mut sizes = ComponentSizes::default();
        for c in 0..self.len() {
            let size = index(self.range(c).len());
            match size {
                1 => sizes.single += 1,
                2 => sizes.pairs += 1,
                _ if solved_whole(slot(size)) => sizes.small += 1,
                _ => {
                    sizes.large += 1;
                    sizes.large_cells += size;
                }
            }
            sizes.largest = sizes.largest.max(size);
        }
        sizes
    }
}

/// A node not yet reached by [`strongly_connected`]'s search.
const UNSEEN: u32 = u32::MAX;

/// A node on [`strongly_connected`]'s depth-first path: it, its successors and how many of them
/// it has tried.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Frame {
    node: usize,
    successors: [Option<u32>; 4],
    tried: usize,
}

/// [`strongly_connected`]'s state: Tarjan's numbering and low links, his stack and the
/// depth-first path in place of the recursion.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Search {
    number: Vec<u32>,
    low: Vec<u32>,
    on_stack: Vec<bool>,
    stack: Vec<u32>,
    path: Vec<Frame>,
    next: u32,
}

impl Search {
    /// A search over `count` nodes, none reached.
    #[must_use]
    fn new(count: usize) -> Self {
        Self {
            number: vec![UNSEEN; count],
            low: vec![0; count],
            on_stack: vec![false; count],
            stack: Vec::new(),
            path: Vec::new(),
            next: 0,
        }
    }

    /// Reaches `node`, whose successors are `successors`: numbers it and steps the path onto it.
    fn enter(&mut self, node: usize, successors: [Option<u32>; 4]) {
        self.number[node] = self.next;
        self.low[node] = self.next;
        self.next += 1;
        self.stack.push(index(node));
        self.on_stack[node] = true;
        self.path.push(Frame {
            node,
            successors,
            tried: 0,
        });
    }
}

/// The strongly connected components of the graph on the nodes below `count` for which `member`
/// holds, the successors of each `successors(node)` (members, in that order), by Tarjan's
/// algorithm (Tarjan 1972, the [module documentation](self)'s citation) without recursion: roots
/// in index order, each node's successors in their order. The components go in the reverse of
/// their completion order, which Tarjan's algorithm emits in reverse topological order, so that
/// every edge between two components points forward; each one's members in index order.
///
/// # Panics
///
/// If a node's successor is out of range, or there are 2³² nodes or more.
#[must_use]
fn strongly_connected(
    count: usize,
    member: impl Fn(usize) -> bool,
    successors: impl Fn(usize) -> [Option<u32>; 4],
) -> Components {
    let mut search = Search::new(count);
    // The members of each component as it completes, and where each ends.
    let (mut done, mut ends) = (Vec::new(), Vec::new());
    for root in 0..count {
        if !member(root) || search.number[root] != UNSEEN {
            continue;
        }
        search.enter(root, successors(root));
        while let Some(frame) = search.path.last_mut() {
            let v = frame.node;
            if let Some(&successor) = frame.successors.get(frame.tried) {
                frame.tried += 1;
                if let Some(w) = successor {
                    let w = slot(w);
                    debug_assert!(member(w), "a successor is a member");
                    if search.number[w] == UNSEEN {
                        search.enter(w, successors(w));
                    } else if search.on_stack[w] {
                        search.low[v] = search.low[v].min(search.number[w]);
                    }
                }
                continue;
            }
            search.path.pop();
            if let Some(parent) = search.path.last() {
                let p = parent.node;
                search.low[p] = search.low[p].min(search.low[v]);
            }
            if search.low[v] == search.number[v] {
                let start = done.len();
                loop {
                    let w = search
                        .stack
                        .pop()
                        .expect("a component's root is on the stack");
                    search.on_stack[slot(w)] = false;
                    done.push(w);
                    if slot(w) == v {
                        break;
                    }
                }
                done[start..].sort_unstable();
                ends.push(done.len());
            }
        }
    }
    let mut cells = Vec::with_capacity(done.len());
    let mut starts = Vec::with_capacity(ends.len() + 1);
    starts.push(0);
    for c in (0..ends.len()).rev() {
        let begin = if c == 0 { 0 } else { ends[c - 1] };
        cells.extend_from_slice(&done[begin..ends[c]]);
        starts.push(index(cells.len()));
    }
    Components { cells, starts }
}

/// The month's upwind graph's strongly connected components (decision-r09-t13e-lfpm-convergence
/// §3.2 item 1): over `locals`' land cells, an edge from each whose air moves on to each land
/// neighbour across an edge with (ŵ · n̂)⁺ ℓ > 0, each cell's edges in [`Edge::ALL`]'s order.
///
/// [`Edge::ALL`]: hyperion_surface::cube::Edge::ALL
#[must_use]
fn upwind_components(mesh: &Mesh, locals: &[Local]) -> Components {
    strongly_connected(
        locals.len(),
        |i| locals[i].kind.is_land(),
        |i| {
            let (local, faces) = (&locals[i], &mesh.faces[i]);
            core::array::from_fn(|k| {
                let face = &faces[k];
                let passes = local.kind == Kind::Through
                    && downwind(local.dots[k]) * face.length_m > 0.0
                    && locals[slot(face.neighbour)].kind.is_land();
                passes.then_some(face.neighbour)
            })
        },
    )
}

/// What `row`'s cell takes in from the sea (none under [`Source::Nothing`]) and from its
/// neighbours' latest outflows in `outflow_kg_s` but those `skip` names, kg s⁻¹, vapour then
/// cloud water, its edges in order.
#[must_use]
fn inflow(row: &Row, outflow_kg_s: &[[f64; 2]], source: Source, skip: Skip) -> [f64; 2] {
    let mut inflow = match source {
        Source::Sea => row.sea_kg_s,
        Source::Nothing => [0.0; 2],
    };
    for ((&j, &pull), &place) in row.neighbours.iter().zip(&row.pulls).zip(&row.block) {
        if skip == Skip::Block && place != OUTSIDE {
            continue;
        }
        let q = outflow_kg_s[slot(j)];
        inflow[0] += pull * q[0];
        inflow[1] += pull * q[1];
    }
    inflow
}

/// Which neighbours [`inflow`] leaves out: none, or those in the cell's own block, whose share the
/// block's solve carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Skip {
    None,
    Block,
}

/// One sweep (the [module documentation](self)), in `components`' order: a single cell, and each
/// member of a component larger than [`EXACT_BLOCK_CELLS`] in turn, solved for its own outflow
/// from its neighbours' latest; a component of 2 to [`EXACT_BLOCK_CELLS`] cells solved whole. Each
/// land cell's outflow is updated in `outflow_kg_s`; a sink keeps none.
fn sweep(
    balances: &Balances,
    components: &Components,
    outflow_kg_s: &mut [[f64; 2]],
    source: Source,
) {
    for c in 0..components.len() {
        let rows = &balances.rows[components.range(c)];
        if solved_whole(rows.len()) {
            let factors = &balances.factors[balances.offsets[c]..];
            solve_block(rows, factors, outflow_kg_s, source);
            continue;
        }
        for row in rows {
            if let Rule::Through { inverse: m, .. } = row.rule {
                let inflow = inflow(row, outflow_kg_s, source, Skip::None);
                outflow_kg_s[slot(row.cell)] = [
                    m[0][0] * inflow[0] + m[0][1] * inflow[1],
                    m[1][0] * inflow[0] + m[1][1] * inflow[1],
                ];
            }
        }
    }
}

/// Solves the block `rows` whole from its factors `factors` ([`factorise`]'s, at their start),
/// every neighbour outside it at its latest outflow in `outflow_kg_s`, and writes its members'
/// outflows there.
fn solve_block(rows: &[Row], factors: &[f64], outflow_kg_s: &mut [[f64; 2]], source: Source) {
    let m = 2 * rows.len();
    let mut b = [0.0; 2 * EXACT_BLOCK_CELLS];
    for (pair, row) in b.as_chunks_mut::<2>().0.iter_mut().zip(rows) {
        *pair = inflow(row, outflow_kg_s, source, Skip::Block);
    }
    for c in 0..m {
        let known = b[c];
        for (r, value) in b.iter_mut().enumerate().take(m).skip(c + 1) {
            *value -= factors[r * m + c] * known;
        }
    }
    for r in (0..m).rev() {
        let line = &factors[r * m..(r + 1) * m];
        let sum = line[r + 1..]
            .iter()
            .zip(&b[r + 1..m])
            .fold(b[r], |sum, (&u, &q)| sum - u * q);
        b[r] = sum / line[r];
    }
    for (&pair, row) in b.as_chunks::<2>().0.iter().zip(rows) {
        outflow_kg_s[slot(row.cell)] = pair;
    }
}

/// Each land cell's rate at the outflows `outflow_kg_s`, kg m⁻² s⁻¹, into `rates` (one a cell; the
/// sea's left as they are): a cell whose air moves on, its cloud water's outflow over W `L_f`
/// (eq. 8); a sink, its whole inflow, the sea's and its neighbours', over (1 − ε) A.
fn rates_at(rows: &[Row], outflow_kg_s: &[[f64; 2]], rates: &mut [f64]) {
    for row in rows {
        let i = slot(row.cell);
        rates[i] = match row.rule {
            Rule::Through { rate_per_cloud, .. } => outflow_kg_s[i][1] * rate_per_cloud,
            Rule::Sink { rate_per_inflow } => {
                let inflow = inflow(row, outflow_kg_s, Source::Sea, Skip::None);
                (inflow[0] + inflow[1]) * rate_per_inflow
            }
        };
    }
}

/// The stop test's measure (decision-r09-t13e-lfpm-convergence §3.2 item 5): the largest change
/// of a land cell's rate from `before` to `after` over max(its rate before, `floor_kg_m2_s`);
/// zero where nothing moved.
#[must_use]
fn measure(rows: &[Row], before: &[f64], after: &[f64], floor_kg_m2_s: f64) -> f64 {
    let mut largest = 0.0;
    for row in rows {
        let i = slot(row.cell);
        let moved = (after[i] - before[i]).abs();
        if moved > 0.0 {
            let scale = if before[i] > floor_kg_m2_s {
                before[i]
            } else {
                floor_kg_m2_s
            };
            let share = if scale > 0.0 {
                moved / scale
            } else {
                f64::INFINITY
            };
            if share > largest {
                largest = share;
            }
        }
    }
    largest
}

/// The unknowns of the cells `through` (in their order, each one's vapour then cloud water) from
/// `outflow_kg_s` into `x`.
fn pack(through: &[u32], outflow_kg_s: &[[f64; 2]], x: &mut [f64]) {
    for (pair, &cell) in x.as_chunks_mut::<2>().0.iter_mut().zip(through) {
        *pair = outflow_kg_s[slot(cell)];
    }
}

/// The unknowns `x` of the cells `through` into `outflow_kg_s`.
fn unpack(through: &[u32], x: &[f64], outflow_kg_s: &mut [[f64; 2]]) {
    for (&pair, &cell) in x.as_chunks::<2>().0.iter().zip(through) {
        outflow_kg_s[slot(cell)] = pair;
    }
}

/// a · b over two vectors of the unknowns, summed in their order.
#[must_use]
fn inner(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).fold(0.0, |sum, (&p, &q)| sum + p * q)
}

/// A month's sweep as a map of its unknowns, the vapour and cloud-water outflows of the land cells
/// whose air moves on.
#[derive(Debug, Clone, Copy)]
struct Map<'a> {
    balances: &'a Balances,
    components: &'a Components,
    /// The land cells whose air moves on, in cell-index order.
    through: &'a [u32],
}

impl Map<'_> {
    /// One sweep from `x` with the sea's air, F(x), or without it, G x, into `out`, by way of
    /// `outflow_kg_s`, which holds no outflow but the unknowns'.
    fn apply(&self, x: &[f64], source: Source, outflow_kg_s: &mut [[f64; 2]], out: &mut [f64]) {
        unpack(self.through, x, outflow_kg_s);
        sweep(self.balances, self.components, outflow_kg_s, source);
        pack(self.through, outflow_kg_s, out);
    }
}

/// One cycle of GMRES([`RESTART`]) (Saad and Schultz 1986, the [module documentation](self)'s
/// citation) on (I − G) x = c from `x`, whose residual c − (I − G) x, F(x) − x, is `residual`, of
/// 2-norm `norm` > 0: x moves by V y, y the least-squares solution over the Krylov space of up to
/// [`RESTART`] steps, the basis V (`basis`, [`RESTART`] + 1 vectors of the unknowns) by modified
/// Gram–Schmidt and the Hessenberg matrix reduced to its triangle by Givens rotations, every sum
/// in the unknowns' order. Returns the steps taken, a sweep each: [`RESTART`], or fewer where the
/// space is exhausted, the solve then exact in it.
fn cycle(
    map: &Map<'_>,
    x: &mut [f64],
    residual: &[f64],
    norm: f64,
    basis: &mut [Vec<f64>],
    outflow_kg_s: &mut [[f64; 2]],
) -> u32 {
    debug_assert!(norm > 0.0, "a cycle starts from a residual");
    let mut hessenberg = [[0.0; RESTART]; RESTART + 1];
    let mut rotations = [(0.0, 0.0); RESTART];
    let mut rotated = [0.0; RESTART + 1];
    rotated[0] = norm;
    for (v, &r) in basis[0].iter_mut().zip(residual) {
        *v = r / norm;
    }
    let mut steps = 0;
    for j in 0..RESTART {
        let (head, tail) = basis.split_at_mut(j + 1);
        let next_vector = &mut tail[0];
        // (I − G) v_j.
        map.apply(&head[j], Source::Nothing, outflow_kg_s, next_vector);
        for (out, &v) in next_vector.iter_mut().zip(&head[j]) {
            *out = v - *out;
        }
        for (i, earlier) in head.iter().enumerate() {
            let projection = inner(next_vector, earlier);
            hessenberg[i][j] = projection;
            for (out, &v) in next_vector.iter_mut().zip(earlier) {
                *out -= projection * v;
            }
        }
        let length = inner(next_vector, next_vector).sqrt();
        hessenberg[j + 1][j] = length;
        for (i, &(cosine, sine)) in rotations[..j].iter().enumerate() {
            let (upper, lower) = (hessenberg[i][j], hessenberg[i + 1][j]);
            hessenberg[i][j] = cosine * upper + sine * lower;
            hessenberg[i + 1][j] = -sine * upper + cosine * lower;
        }
        let diagonal = math::hypot(hessenberg[j][j], length);
        debug_assert!(diagonal > 0.0, "(I − G) is not singular");
        let (cosine, sine) = (hessenberg[j][j] / diagonal, length / diagonal);
        rotations[j] = (cosine, sine);
        hessenberg[j][j] = diagonal;
        hessenberg[j + 1][j] = 0.0;
        rotated[j + 1] = -sine * rotated[j];
        rotated[j] *= cosine;
        steps = j + 1;
        if length <= 0.0 {
            break;
        }
        for out in next_vector.iter_mut() {
            *out /= length;
        }
    }
    let mut weights = [0.0; RESTART];
    for i in (0..steps).rev() {
        let line = &hessenberg[i];
        let sum = line[i + 1..steps]
            .iter()
            .zip(&weights[i + 1..steps])
            .fold(rotated[i], |sum, (&entry, &weight)| sum - entry * weight);
        weights[i] = sum / line[i];
    }
    for (&weight, v) in weights[..steps].iter().zip(basis.iter()) {
        for (out, &value) in x.iter_mut().zip(v) {
            *out += weight * value;
        }
    }
    index(steps)
}

/// What a month's iteration did.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Iteration {
    restarts: u32,
    sweeps: u32,
    measure: f64,
    converged: bool,
    relative_residual: f64,
    /// Whether the final state is a sweep with the dispersion, or the start.
    dispersion: Dispersion,
}

impl Iteration {
    /// A solve that stopped at the start, after `sweeps` sweeps.
    #[must_use]
    const fn at_start(sweeps: u32) -> Self {
        Self {
            restarts: 0,
            sweeps,
            measure: 0.0,
            converged: true,
            relative_residual: 0.0,
            dispersion: Dispersion::Off,
        }
    }
}

/// The Krylov iteration (decision-r09-t13e-lfpm-convergence §3.2 items 3–6) from the start's
/// outflows in `outflow_kg_s`, which it leaves holding the final state: restarted
/// GMRES([`RESTART`]) on (I − G) x = c ([`cycle`]), each restart's residual F(x) − x one sweep
/// from x, until that sweep moves no land cell's rate by more than the parameters' tolerance of
/// max(its rate, `floor_kg_m2_s`), or the cap, the month keeping that sweep's result. It stops at
/// the start where the sea's air reaches no unknown (‖c‖ = 0).
fn iterate(
    map: &Map<'_>,
    parameters: &Parameters,
    floor_kg_m2_s: f64,
    outflow_kg_s: &mut [[f64; 2]],
) -> Iteration {
    let unknowns = 2 * map.through.len();
    let mut x = vec![0.0; unknowns];
    pack(map.through, outflow_kg_s, &mut x);
    let mut swept = vec![0.0; unknowns];
    map.apply(&vec![0.0; unknowns], Source::Sea, outflow_kg_s, &mut swept);
    let sea_norm = inner(&swept, &swept).sqrt();
    if sea_norm <= 0.0 {
        unpack(map.through, &x, outflow_kg_s);
        return Iteration::at_start(2);
    }
    let mut iteration = Iteration {
        restarts: 0,
        sweeps: 2,
        measure: 0.0,
        converged: false,
        relative_residual: 0.0,
        dispersion: Dispersion::On,
    };
    let cells = outflow_kg_s.len();
    let (mut before, mut after) = (vec![0.0; cells], vec![0.0; cells]);
    let mut basis: Vec<Vec<f64>> = Vec::new();
    loop {
        unpack(map.through, &x, outflow_kg_s);
        rates_at(&map.balances.rows, outflow_kg_s, &mut before);
        sweep(map.balances, map.components, outflow_kg_s, Source::Sea);
        iteration.sweeps += 1;
        rates_at(&map.balances.rows, outflow_kg_s, &mut after);
        pack(map.through, outflow_kg_s, &mut swept);
        for (r, &xi) in swept.iter_mut().zip(&x) {
            *r -= xi;
        }
        let norm = inner(&swept, &swept).sqrt();
        iteration.measure = measure(&map.balances.rows, &before, &after, floor_kg_m2_s);
        iteration.relative_residual = norm / sea_norm;
        if iteration.measure <= parameters.tolerance {
            iteration.converged = true;
            return iteration;
        }
        if iteration.restarts >= parameters.max_restarts {
            return iteration;
        }
        if basis.is_empty() {
            basis = vec![vec![0.0; unknowns]; RESTART + 1];
        }
        iteration.sweeps += cycle(map, &mut x, &swept, norm, &mut basis, outflow_kg_s);
        iteration.restarts += 1;
    }
}

/// `P̄_sea`: the area mean of `month`'s large-scale rate over its sea cells, kg m⁻² s⁻¹, summed in
/// cell-index order; zero on a world of no sea.
#[must_use]
fn sea_mean_kg_m2_s(mesh: &Mesh, month: &Month<'_>) -> f64 {
    let (mut rained, mut area) = (0.0, 0.0);
    for ((&a, &cover), &rate) in mesh
        .areas_m2
        .iter()
        .zip(month.cover)
        .zip(month.sea_rates_kg_m2_s)
    {
        if cover == Cover::Sea {
            rained += rate * a;
            area += a;
        }
    }
    if area > 0.0 { rained / area } else { 0.0 }
}

/// Checks that `month` holds one value a cell of `mesh`, its heights finite and its sea rates
/// finite and non-negative.
///
/// # Panics
///
/// If it does not.
fn check(mesh: &Mesh, month: &Month<'_>) {
    let cells = mesh.areas_m2.len();
    for (name, len) in [
        ("heights", month.heights_m.len()),
        ("covers", month.cover.len()),
        ("winds", month.winds.len()),
        ("sea rates", month.sea_rates_kg_m2_s.len()),
    ] {
        assert_eq!(len, cells, "a month holds {cells} {name}, one a cell");
    }
    for (&height_m, (&cover, &rate)) in month
        .heights_m
        .iter()
        .zip(month.cover.iter().zip(month.sea_rates_kg_m2_s))
    {
        assert!(
            height_m.is_finite(),
            "a height must be finite, got {height_m} m"
        );
        assert!(
            cover == Cover::Land || (rate.is_finite() && rate >= 0.0),
            "a sea's rate is finite and non-negative, got {rate} kg m^-2 s^-1"
        );
    }
}

/// The month's orographic precipitation over `mesh` (the [module documentation](self)): each land
/// cell's rate and recycled share, solved in the order of the month's upwind components from a
/// sweep without the dispersion, by GMRES([`RESTART`]) until one further sweep moves no land
/// cell's rate by more than the parameters' tolerance of max(its rate, [`RATE_FLOOR_SHARE`] of
/// the sea's mean rate), or the cap.
///
/// # Panics
///
/// If a slice of `month` does not hold one value a cell, a height is not finite, a sea cell's rate
/// is negative or not finite, a wind is not finite or has no direction along the surface, or a
/// rate comes out not finite (a fault of the model).
#[must_use]
pub(super) fn solve(mesh: &Mesh, month: &Month<'_>, parameters: &Parameters) -> Orographic {
    check(mesh, month);
    let cells = mesh.areas_m2.len();
    let locals: Vec<Local> = (0..cells)
        .map(|i| Local::of(mesh, i, month.cover[i], month.winds[i]))
        .collect();
    let components = upwind_components(mesh, &locals);
    let through: Vec<u32> = (0..cells)
        .filter(|&i| locals[i].kind == Kind::Through)
        .map(index)
        .collect();
    let sea_mean_kg_m2_s = sea_mean_kg_m2_s(mesh, month);
    let mut outflow_kg_s = vec![[0.0; 2]; cells];
    // The start, its balances dropped before the rest are built, so that both never stand.
    sweep(
        &Balances::new(
            mesh,
            month,
            parameters,
            &locals,
            &components,
            Dispersion::Off,
        ),
        &components,
        &mut outflow_kg_s,
        Source::Sea,
    );
    let balances = Balances::new(
        mesh,
        month,
        parameters,
        &locals,
        &components,
        Dispersion::On,
    );
    let iteration = if through.is_empty() {
        Iteration::at_start(1)
    } else {
        let map = Map {
            balances: &balances,
            components: &components,
            through: &through,
        };
        let floor_kg_m2_s = RATE_FLOOR_SHARE * sea_mean_kg_m2_s;
        iterate(&map, parameters, floor_kg_m2_s, &mut outflow_kg_s)
    };
    let mut precipitation_kg_m2_s = vec![0.0; cells];
    rates_at(&balances.rows, &outflow_kg_s, &mut precipitation_kg_m2_s);
    let mut negative_cells = 0_u32;
    for &cell in &components.cells {
        let rate = &mut precipitation_kg_m2_s[slot(cell)];
        assert!(rate.is_finite(), "a rate must be finite, got {rate}");
        if *rate < 0.0 {
            negative_cells += 1;
        }
        if *rate <= 0.0 {
            *rate = 0.0;
        }
    }
    let recycled_shares: Vec<f64> = (0..cells)
        .map(|i| {
            if locals[i].kind.is_land() {
                parameters.recycled_share(month.heights_m[i])
            } else {
                0.0
            }
        })
        .collect();
    let solved = Solved {
        locals: &locals,
        outflow_kg_s: &outflow_kg_s,
        precipitation_kg_m2_s: &precipitation_kg_m2_s,
        recycled_shares: &recycled_shares,
    };
    let mut diagnostics = budget(mesh, month, parameters, &solved, iteration.dispersion);
    diagnostics.restarts = iteration.restarts;
    diagnostics.sweeps = iteration.sweeps;
    diagnostics.measure = iteration.measure;
    diagnostics.converged = iteration.converged;
    diagnostics.relative_residual = iteration.relative_residual;
    diagnostics.components = components.sizes();
    diagnostics.sea_mean_kg_m2_s = sea_mean_kg_m2_s;
    diagnostics.negative_cells = negative_cells;
    diagnostics.land_cells = index(components.cells.len());
    Orographic {
        precipitation_kg_m2_s,
        recycled_shares,
        diagnostics,
    }
}

/// A month's solve at its final state, which its budget reads.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Solved<'a> {
    /// Each cell's wind against its edges.
    locals: &'a [Local],
    /// Each cell's outflow, kg s⁻¹, vapour then cloud water; zero but on land whose air moves on.
    outflow_kg_s: &'a [[f64; 2]],
    /// Each cell's rate, kg m⁻² s⁻¹.
    precipitation_kg_m2_s: &'a [f64],
    /// Each cell's recycled share.
    recycled_shares: &'a [f64],
}

/// The land's water budget and the global mean rate at `solved`'s values, summed in cell-index
/// order and each cell's edges in turn: what the sea gave and took, what fell and recycled, and
/// what passed to the sea, the dispersion counted as the final state's balances carried it
/// (`dispersion`: none at the start); the iteration's fields left for [`solve`].
#[must_use]
fn budget(
    mesh: &Mesh,
    month: &Month<'_>,
    parameters: &Parameters,
    solved: &Solved<'_>,
    dispersion: Dispersion,
) -> Diagnostics {
    let Solved {
        locals,
        outflow_kg_s,
        precipitation_kg_m2_s,
        recycled_shares,
    } = *solved;
    let mut diagnostics = Diagnostics {
        restarts: 0,
        sweeps: 0,
        measure: 0.0,
        converged: true,
        relative_residual: 0.0,
        components: ComponentSizes::default(),
        sea_mean_kg_m2_s: 0.0,
        negative_cells: 0,
        global_mean_kg_m2_s: 0.0,
        land_cells: 0,
        inflow_kg_s: 0.0,
        runoff_kg_s: 0.0,
        recycled_kg_s: 0.0,
        outflow_kg_s: 0.0,
    };
    let (mut rained_kg_s, mut area_m2) = (0.0, 0.0);
    for (i, local) in locals.iter().enumerate() {
        let a = mesh.areas_m2[i];
        area_m2 += a;
        if !local.kind.is_land() {
            rained_kg_s += month.sea_rates_kg_m2_s[i] * a;
            continue;
        }
        let fallen_kg_s = precipitation_kg_m2_s[i] * a;
        rained_kg_s += fallen_kg_s;
        diagnostics.runoff_kg_s += (1.0 - recycled_shares[i]) * fallen_kg_s;
        diagnostics.recycled_kg_s += recycled_shares[i] * fallen_kg_s;
        let own = outflow_kg_s[i][0] + outflow_kg_s[i][1];
        for (k, face) in mesh.faces[i].iter().enumerate() {
            let j = slot(face.neighbour);
            let other = &locals[j];
            if other.kind.is_land() {
                continue;
            }
            if local.kind == Kind::Through {
                diagnostics.outflow_kg_s +=
                    downwind(local.dots[k]) * face.length_m / local.width_m * own;
            }
            if other.kind == Kind::Sea {
                let [vapour, cloud] = parameters.sea_air(month.sea_rates_kg_m2_s[j]);
                let given = downwind(other.dots[usize::from(face.back)]) * face.length_m;
                diagnostics.inflow_kg_s += (vapour + cloud) * given;
                if dispersion == Dispersion::On && local.kind == Kind::Through {
                    let kappa_m = spread_m(parameters, face, k, local, other);
                    diagnostics.inflow_kg_s += kappa_m * (vapour + cloud - own / local.width_m);
                }
            }
        }
    }
    diagnostics.global_mean_kg_m2_s = rained_kg_s / area_m2;
    diagnostics
}

/// The index into a mesh's vectors of cell `cell`.
///
/// # Panics
///
/// Never on the targets the sim builds for: a `u32` fits a `usize` on 32- and 64-bit targets.
#[must_use]
fn slot(cell: u32) -> usize {
    usize::try_from(cell).expect("a cell index fits a usize on every target the sim builds for")
}

/// The cell index of vector index `i`.
///
/// # Panics
///
/// Never for a mesh, which has under 2³² cells.
#[must_use]
fn index(i: usize) -> u32 {
    u32::try_from(i).expect("a mesh has under 2^32 cells")
}

/// a · b.
#[must_use]
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// a × b.
#[must_use]
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// s a.
#[must_use]
fn scale(a: [f64; 3], s: f64) -> [f64; 3] {
    [s * a[0], s * a[1], s * a[2]]
}

/// |a|.
#[must_use]
fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

/// a ÷ |a|.
#[must_use]
fn unit(a: [f64; 3]) -> [f64; 3] {
    scale(a, 1.0 / norm(a))
}

/// The part of `a` tangent to the unit sphere at the unit vector `at`.
#[must_use]
fn tangent(a: [f64; 3], at: [f64; 3]) -> [f64; 3] {
    let along = dot(a, at);
    [
        a[0] - along * at[0],
        a[1] - along * at[1],
        a[2] - along * at[2],
    ]
}

/// The angle between two unit vectors, rad, by atan2 of their cross and dot products.
#[must_use]
fn arc(a: [f64; 3], b: [f64; 3]) -> f64 {
    math::atan2(norm(cross(a, b)), dot(a, b))
}
