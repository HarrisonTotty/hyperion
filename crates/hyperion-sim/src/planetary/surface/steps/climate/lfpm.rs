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
//! # The order and the sweeps
//!
//! The land cells are solved by Gauss–Seidel sweeps in the month's upwind order
//! (decision-r09-t13ge §2.3, e3):
//!
//! - The order is Kahn's over the month's upwind graph of the land cells, ties to the lowest cell
//!   index; where its queue empties with cells left, a cycle (a closed loop of a zonal wind, or two
//!   cells blowing into each other), the lowest-index cell left is taken next and its unresolved
//!   upwind edges are cut.
//! - Each cell's balance is solved for its own outflow with every neighbour at its latest value:
//!   the upwind cells' are this sweep's, a cut edge's source and a later neighbour's dispersion the
//!   last sweep's. The first sweep has none of the dispersion and nothing across a cut edge.
//! - The advection is exact after one sweep on a month without cycles. The dispersion's lagged
//!   half converges geometrically, slowest for crosswind errors smooth over many cells, which the
//!   sweep carries downwind: about κ' ÷ (κ' + Δs ÷ `L̃_l`) a sweep over a long continent, κ' =
//!   `L_d` ÷ Δx (0.98 at level 8). A first sweep without dispersion starts such errors small. A
//!   convergence line, where the cells on either side blow into each other, is a chain of
//!   two-cell cycles, and each cut half of them converges as slowly.
//! - The count is fixed, [`SWEEPS`]. Each run reports how far each sweep moved the rates, its
//!   largest cell's change and its global mean's ([`SweepChange`]), by which part 2 set the count
//!   on the reference Earth: the global mean moving by under 10⁻³ of itself in the last sweep.
//!
//! Every loop runs in a fixed order (the cells', the order's, the edges' in [`Edge::ALL`]'s), in
//! `f64`, every exponential through [`crate::math`], with no `HashMap` and no threads.
//!
//! [`Edge::ALL`]: hyperion_surface::cube::Edge::ALL

use std::cmp::Reverse;
use std::collections::BinaryHeap;

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

/// The Gauss–Seidel sweeps a month's solve takes (decision-r09-t13ge §2.3, e3): 14, the count at
/// which the ruling's measure holds in every month on the reference Earth through the coarse pass
/// (its generated continents and R09.T12.e's sea, level 8, the step's own winds; R09.T13.e's second
/// part): the global mean rate moves in the last sweep by under 10⁻³ of the run's own global mean,
/// 9.5 × 10⁻⁴ at most (its eleventh month). Over 64 sweeps the measure is first met at the 9th to
/// the 13th sweep by month against the converged mean, and at 13 two months still read 1.0 × 10⁻³
/// against their own. On R09.T13.g's synthetic sea at level 7 it was met from the 5th to the 8th
/// with each band's land in one block; with Natural Earth's land by box and its heights placed by
/// each box's Earth2014 median it is met from the 9th to the 13th in seven months and not within
/// 14 in five, whose last sweep reads 1.0 to 1.4 × 10⁻³ (plan 09's deferred finding E-L2).
///
/// The largest cell's change in the last sweep is not that small: at 14 sweeps it is 0.16 to 1.2
/// of the global mean, and still 0.013 to 0.31 at 64, where cycles of the month's upwind graph are
/// cut (19–25% of the land's cells; the record's deferred findings, E-L2). On this module's strip
/// worlds, with no cycle, it falls under 10⁻³ of the global mean from the 15th sweep over the 3 km
/// ridge and from the 31st over 3,000 km of flat land at level 8 (from the 7th and 12th at
/// level 7).
pub(super) const SWEEPS: u32 = 14;

/// The least ŵ · n̂ at which an edge is downwind: a wind within 10⁻⁹ rad of an edge's direction
/// passes nothing across it. Far above the rounding of a unit normal (about 10⁻¹⁶), so that a
/// wind along an edge makes no two cells each the other's downwind, and far below any share of
/// the air that matters.
pub(super) const DOWNWIND_TOLERANCE: f64 = 1e-9;

/// The model's lengths, recycling and sweep count: Hergarten and Robl's §7 set
/// ([`Parameters::HERGARTEN_ROBL`]), or a variant of it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Parameters {
    condensation_length_m: f64,
    fallout_length_m: f64,
    long_range_length_m: f64,
    dispersion_length_m: f64,
    sea_level_recycled_share: f64,
    height_scale_m: f64,
    sweeps: u32,
}

impl Parameters {
    /// Hergarten and Robl 2022's §7 set, with [`SWEEPS`] sweeps (decision-r09-t13ge §2.1, e1).
    pub(super) const HERGARTEN_ROBL: Self = Self {
        condensation_length_m: CONDENSATION_LENGTH_M,
        fallout_length_m: FALLOUT_LENGTH_M,
        long_range_length_m: LONG_RANGE_LENGTH_M,
        dispersion_length_m: DISPERSION_LENGTH_M,
        sea_level_recycled_share: SEA_LEVEL_RECYCLED_SHARE,
        height_scale_m: HEIGHT_SCALE_M,
        sweeps: SWEEPS,
    };

    /// The parameters with `sweeps` Gauss–Seidel sweeps.
    ///
    /// # Panics
    ///
    /// If `sweeps` is zero.
    #[cfg(test)]
    #[must_use]
    pub(super) const fn with_sweeps(self, sweeps: u32) -> Self {
        assert!(sweeps > 0, "a solve takes at least one sweep");
        Self { sweeps, ..self }
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

    /// The sweeps a solve takes.
    #[cfg(test)]
    #[must_use]
    pub(super) const fn sweeps(&self) -> u32 {
        self.sweeps
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

/// How far one sweep moved the land's rates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct SweepChange {
    /// The largest change of a land cell's rate, kg m⁻² s⁻¹.
    pub(super) largest_kg_m2_s: f64,
    /// The change of the global mean rate, the sea's held, kg m⁻² s⁻¹, its magnitude: the measure
    /// decision-r09-t13ge §2.3 sets the sweep count by.
    pub(super) mean_kg_m2_s: f64,
}

/// What a month's solve did that is no output: its sweeps' convergence, its cycles, and the land's
/// water budget at its last sweep.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Diagnostics {
    /// How far each sweep moved the rates, in order.
    pub(super) sweeps: Vec<SweepChange>,
    /// The area-weighted mean rate over every cell, the land's P and the sea's `P_ls`,
    /// kg m⁻² s⁻¹: the scale the last sweep's changes are read against.
    pub(super) global_mean_kg_m2_s: f64,
    /// The land cells.
    pub(super) land_cells: u32,
    /// The land cells whose upwind edges were cut to break a cycle.
    pub(super) cut_cells: u32,
    /// The water the land's air took in from the sea, by the wind and by dispersion, kg s⁻¹.
    pub(super) inflow_kg_s: f64,
    /// The precipitation that ran off, Σ (1 − ε) P A over the land, kg s⁻¹.
    pub(super) runoff_kg_s: f64,
    /// The precipitation that evaporated back into the air, Σ ε P A over the land, kg s⁻¹.
    pub(super) recycled_kg_s: f64,
    /// The water the land's air passed to the sea, kg s⁻¹.
    pub(super) outflow_kg_s: f64,
}

impl Diagnostics {
    /// The last sweep's changes over the global mean rate, its largest cell's and its global
    /// mean's (decision-r09-t13ge §2.3's measure); zero where nothing rains.
    #[cfg(test)]
    #[must_use]
    pub(super) fn last_relative_changes(&self) -> SweepChange {
        let last = self.sweeps.last().copied().unwrap_or(SweepChange {
            largest_kg_m2_s: 0.0,
            mean_kg_m2_s: 0.0,
        });
        let mean = self.global_mean_kg_m2_s;
        let relative = |change: f64| if mean > 0.0 { change / mean } else { 0.0 };
        SweepChange {
            largest_kg_m2_s: relative(last.largest_kg_m2_s),
            mean_kg_m2_s: relative(last.mean_kg_m2_s),
        }
    }
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
    /// What the sea gives it by the wind and by dispersion, kg s⁻¹, vapour then cloud water.
    sea_kg_s: [f64; 2],
    /// Its area, m².
    area_m2: f64,
    /// How its rate follows from its inflow.
    rule: Rule,
}

/// How a land cell's rate follows from its inflow.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Rule {
    /// Its outflow is its matrix's inverse times its inflow, and its rate its cloud water's
    /// outflow over W `L_f`.
    Through {
        inverse: [[f64; 2]; 2],
        rate_per_cloud: f64,
    },
    /// Its rate is its whole inflow over (1 − ε) A.
    Sink { rate_per_inflow: f64 },
}

/// Whether a sweep's balances carry the transverse dispersion: the first sweep's do not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dispersion {
    Off,
    On,
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

/// Each land cell's balance, in `order`, with or without the dispersion.
#[must_use]
fn rows(
    mesh: &Mesh,
    month: &Month<'_>,
    parameters: &Parameters,
    locals: &[Local],
    order: &[u32],
    dispersion: Dispersion,
) -> Vec<Row> {
    order
        .iter()
        .map(|&cell| {
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
                    Rule::Through {
                        inverse: inverse(cell_matrix(parameters, path, height_m, spread)),
                        rate_per_cloud: 1.0 / (local.width_m * parameters.fallout_length_m),
                    }
                }
                Kind::Sink => Rule::Sink {
                    rate_per_inflow: 1.0
                        / ((1.0 - parameters.recycled_share(height_m)) * mesh.areas_m2[i]),
                },
                Kind::Sea | Kind::CalmSea => unreachable!("the order holds land cells alone"),
            };
            Row {
                cell,
                neighbours: faces.map(|face| face.neighbour),
                pulls,
                sea_kg_s,
                area_m2: mesh.areas_m2[i],
                rule,
            }
        })
        .collect()
}

/// The month's upwind order of the land cells, and how many had upwind edges cut: Kahn's
/// algorithm over the edges along which one land cell passes air to another, ties to the lowest
/// cell index, and where the queue empties with cells left, the lowest-index cell left next
/// (decision-r09-t13ge §2.3, e3).
#[must_use]
fn upwind_order(mesh: &Mesh, locals: &[Local]) -> (Vec<u32>, u32) {
    // Each land cell's downwind land neighbours, through the edges whose share of its air is
    // positive.
    let downstream = |i: usize| {
        let local = &locals[i];
        mesh.faces[i]
            .iter()
            .zip(local.dots)
            .filter(move |(face, d)| {
                local.kind == Kind::Through
                    && downwind(*d) * face.length_m > 0.0
                    && locals[slot(face.neighbour)].kind.is_land()
            })
            .map(|(face, _)| slot(face.neighbour))
    };
    let mut pending = vec![0_u8; locals.len()];
    for i in 0..locals.len() {
        for j in downstream(i) {
            pending[j] += 1;
        }
    }
    let land = |i: usize| locals[i].kind.is_land();
    let mut queued = vec![false; locals.len()];
    let mut ready = BinaryHeap::new();
    for i in (0..locals.len()).filter(|&i| land(i) && pending[i] == 0) {
        queued[i] = true;
        ready.push(Reverse(index(i)));
    }
    let mut order = Vec::with_capacity(locals.iter().filter(|l| l.kind.is_land()).count());
    let (mut next, mut cut) = (0, 0_u32);
    loop {
        while let Some(Reverse(cell)) = ready.pop() {
            order.push(cell);
            for j in downstream(slot(cell)) {
                pending[j] -= 1;
                if pending[j] == 0 && !queued[j] {
                    queued[j] = true;
                    ready.push(Reverse(index(j)));
                }
            }
        }
        while next < locals.len() && (!land(next) || queued[next]) {
            next += 1;
        }
        if next == locals.len() {
            break;
        }
        queued[next] = true;
        cut += 1;
        ready.push(Reverse(index(next)));
    }
    (order, cut)
}

/// One Gauss–Seidel sweep over `rows`, updating each cell's outflow and rate in turn from its
/// neighbours' latest outflows: the largest change of a rate, kg m⁻² s⁻¹, and the change of the
/// land's rainfall, Σ ΔP A in the rows' order, kg s⁻¹.
fn sweep(
    rows: &[Row],
    outflow_kg_s: &mut [[f64; 2]],
    precipitation_kg_m2_s: &mut [f64],
) -> (f64, f64) {
    let (mut change, mut rained_kg_s) = (0.0, 0.0);
    for row in rows {
        let mut inflow = row.sea_kg_s;
        for (&j, &pull) in row.neighbours.iter().zip(&row.pulls) {
            let q = outflow_kg_s[slot(j)];
            inflow[0] += pull * q[0];
            inflow[1] += pull * q[1];
        }
        let i = slot(row.cell);
        let rate = match row.rule {
            Rule::Through {
                inverse: m,
                rate_per_cloud,
            } => {
                let q = [
                    m[0][0] * inflow[0] + m[0][1] * inflow[1],
                    m[1][0] * inflow[0] + m[1][1] * inflow[1],
                ];
                outflow_kg_s[i] = q;
                q[1] * rate_per_cloud
            }
            Rule::Sink { rate_per_inflow } => (inflow[0] + inflow[1]) * rate_per_inflow,
        };
        let moved = rate - precipitation_kg_m2_s[i];
        rained_kg_s += moved * row.area_m2;
        if moved.abs() > change {
            change = moved.abs();
        }
        precipitation_kg_m2_s[i] = rate;
    }
    (change, rained_kg_s)
}

/// The month's orographic precipitation over `mesh` (the [module documentation](self)): each land
/// cell's rate and recycled share, by [`Parameters::sweeps`] Gauss–Seidel sweeps in the month's
/// upwind order, the first without dispersion.
///
/// # Panics
///
/// If a slice of `month` does not hold one value a cell, a height is not finite, a sea cell's rate
/// is negative or not finite, a wind is not finite or has no direction along the surface, or a
/// rate comes out not finite (a fault of the model).
#[must_use]
pub(super) fn solve(mesh: &Mesh, month: &Month<'_>, parameters: &Parameters) -> Orographic {
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
    let locals: Vec<Local> = (0..cells)
        .map(|i| Local::of(mesh, i, month.cover[i], month.winds[i]))
        .collect();
    let (order, cut_cells) = upwind_order(mesh, &locals);
    let mut outflow_kg_s = vec![[0.0; 2]; cells];
    let mut precipitation_kg_m2_s = vec![0.0; cells];
    let area_m2 = mesh.areas_m2.iter().sum::<f64>();
    let mut sweeps = Vec::with_capacity(slot(parameters.sweeps));
    let mut balances = rows(mesh, month, parameters, &locals, &order, Dispersion::Off);
    for n in 0..parameters.sweeps {
        if n == 1 {
            // The first sweep's balances go before the rest are built, so that both never stand.
            drop(core::mem::take(&mut balances));
            balances = rows(mesh, month, parameters, &locals, &order, Dispersion::On);
        }
        let (largest, rained_kg_s) =
            sweep(&balances, &mut outflow_kg_s, &mut precipitation_kg_m2_s);
        sweeps.push(SweepChange {
            largest_kg_m2_s: largest,
            mean_kg_m2_s: (rained_kg_s / area_m2).abs(),
        });
    }
    for &rate in &precipitation_kg_m2_s {
        assert!(rate.is_finite(), "a rate must be finite, got {rate}");
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
    let mut diagnostics = budget(mesh, month, parameters, &solved);
    diagnostics.sweeps = sweeps;
    diagnostics.land_cells = index(order.len());
    diagnostics.cut_cells = cut_cells;
    Orographic {
        precipitation_kg_m2_s,
        recycled_shares,
        diagnostics,
    }
}

/// A month's solve at its last sweep, which its budget reads.
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
/// what passed to the sea, the dispersion counted as the last sweep's balances carried it (none
/// after one sweep); the sweeps' fields left empty for [`solve`].
#[must_use]
fn budget(
    mesh: &Mesh,
    month: &Month<'_>,
    parameters: &Parameters,
    solved: &Solved<'_>,
) -> Diagnostics {
    let Solved {
        locals,
        outflow_kg_s,
        precipitation_kg_m2_s,
        recycled_shares,
    } = *solved;
    let dispersion = if parameters.sweeps > 1 {
        Dispersion::On
    } else {
        Dispersion::Off
    };
    let mut diagnostics = Diagnostics {
        sweeps: Vec::new(),
        global_mean_kg_m2_s: 0.0,
        land_cells: 0,
        cut_cells: 0,
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
