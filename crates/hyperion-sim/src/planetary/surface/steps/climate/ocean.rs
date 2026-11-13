//! The sea's heat in the energy balance (R09.T13.i, decision-r09-t13i §2.1 and §2.3): the heat
//! capacity of a sea of any liquid, and the ocean's heat transport, the Ekman cells inside the
//! Hadley cell and the gyres' and eddies' diffusion, on the model's grid.
//!
//! # The sea's liquid (§2.1)
//!
//! The model's sea is its main liquid's, the one the relief step floods
//! ([`CoarseInputs::liquids`]' first). Its heat capacity is Williams and Kasting's 50 m mixed layer
//! of that liquid, scaled from water's by `r_l` = `ρ_l c_l` ÷ (`ρ_w c_w`), each at the model's
//! freezing point plus [`LIQUID_REFERENCE_ABOVE_FREEZING_K`] and held into each liquid's specific
//! heat column's range ([`SeaLiquid::of`]): water's is exactly 1, the same expression on the same
//! row, so every water world is as built to the bit; a liquid the registry has no specific heat for
//! takes water's ratio, 1. The mixed layer is kept at 50 m whatever the liquid, low confidence for
//! another world's winds.
//!
//! # The ocean's heat transport (§2.3)
//!
//! On the sea's share s of each latitude edge:
//!
//! - **The Ekman cells** (rotating worlds only; Held 2001, J. Atmos. Sci. 58, 943, eqs. 10–13;
//!   Klinger and Marotzke 2000, J. Phys. Oceanogr. 30, 696). Inside the Hadley cell the trades'
//!   stress is the drag on the atmosphere's lower branch, so the ocean's Ekman mass flux equals the
//!   lower branch's over the sea's share of the latitude circle. It carries the surface water
//!   poleward and returns it equatorward in the thermocline, colder by the subducted water's
//!   temperature: `F_O` = s ψ `c_l` `ΔT_O`, with ψ = w F ÷ (`h_T` − h̄) the partition's own Hadley
//!   mass flux ([`partition`](super::partition)). In the step it multiplies the edge's latitude
//!   conductance by 1 + γ, γ = `χ_d` s w `c_l` `ΔT_O` ÷ (`h_T` − h̄), from the step's starting
//!   state, inside the tridiagonal solve. The gross oceanic stability `ΔT_O` follows the basin's
//!   walls ([`ekman_contrast_k`], decision-r09-t13i's §2.2/§2.3 follow-up 5 §1): `χ_e` `ΔT_S` + (1 −
//!   `χ_e`) `ΔT_D`, `χ_e` the barrier factor below. A walled basin's cells are shallow and return the
//!   water subducted between the edge and the return latitude `φ_R` at its late-winter temperature,
//!   Held's eq. 7, `ΔT_S` = max(0, `T_s` − `T_sub`) ([`subducted_k`]); an open ocean's reach the
//!   bottom and return near the whole contrast, `ΔT_D` = max(0, `T_s` − `T_R`)
//!   ([`return_contrast_k`]; Enderton and Marshall 2009's Aqua against Ridge). `T_s` is the edge's
//!   sea temperature, `T_R` and `T_sub`'s bands' the coldest month's band-mean sea temperatures from
//!   the previous orbit, `φ_R` the wind template's first node, where the Hadley and Ferrel cells
//!   meet at the surface ([`wind::return_latitude_deg`]; 35.4° on Earth); the steady state and the
//!   first orbit take their own state's.
//! - **The gyres and eddies, diffusive** (Rose and Marshall 2009, J. Atmos. Sci. 66, 2828):
//!   `F_O,D` = −`K_o` `χ_e` `χ_d` s ∂`T_w`/∂y on the sea's water-surface temperature, with
//!   `K_o` = `K⊕` (`c_l` ÷ `c_w`) `f_Ω` ([`EARTH_OCEAN_CONDUCTANCE_W_PER_M2_K`],
//!   [`rotation_factor`]) and `χ_e` = min(1, L̄ ÷ 0.2) the meridional barrier, 1 on a locked world
//!   ([`WALLED_LAND_SHARE`]). Latitude only, each column with its own s.
//! - **Depth:** both × `χ_d` = min(1, d̄ ÷ 700 m) ([`THERMOCLINE_DEPTH_M`]), d̄ the sea's mean depth.
//!
//! With no sea every term is an exact zero, and a point whose edges carry none is stepped as built,
//! so a world with no sea is the model with no ocean term to the bit. The partition of the water
//! cycle still reads the atmosphere's flux alone (decision-r09-t13-thin-air §2.1).
//!
//! Every sum runs in a fixed order (bands from the south, columns from the west), in `f64`.

use super::ebm::Freezing;
use super::grid::{LATITUDE_BANDS, ModelGrid};
#[cfg(test)]
use super::partition::edge_fluxes;
use super::partition::{
    Circulation, Turning, UPPER_BRANCH_ENERGY_RATIO, edge_fluxes_into, energy_flux_equator,
};
use super::transport::rotation_ratio;
use super::wind;
use crate::math;
use crate::planetary::surface::inputs::CoarseInputs;
use crate::substance::{SubstanceId, by_key};
use crate::units::Kelvin;
use crate::units::consts::EARTH_RADIUS_M;

/// How far above the model's freezing point the sea's liquid and water are compared, K: 10
/// (decision-r09-t13i §2.1): `r_l` and `c_l` ÷ `c_w` are taken at `T_f` + 10 K, a sea just above
/// freezing, each liquid's temperature held into its specific heat column's range.
pub(super) const LIQUID_REFERENCE_ABOVE_FREEZING_K: f64 = 10.0;

/// The ocean's mixed-layer heat capacity in Rose and Marshall's energy-balance model, J m⁻² K⁻¹:
/// 10⁷ (Rose, B. E. J., and Marshall, J. 2009, "Ocean Heat Transport, Sea Ice, and Multiple
/// Climate States: Insights from Energy Balance Models", J. Atmos. Sci. 66, 2828–2843, their `C_o`
/// in Table 1's column for the model with a constant `K_o`, their Fig. 2, and eq. 5b; read by
/// decision-r09-t13i and its science check).
pub(super) const ROSE_MARSHALL_OCEAN_CAPACITY_J_PER_M2_K: f64 = 1.0e7;

/// The ocean's constant diffusivity in Rose and Marshall's energy-balance model, m² s⁻¹: 5.2 × 10⁵
/// (Rose and Marshall 2009, their `K_o` in Table 1's constant-`K_o` column, eq. 5b; read); their
/// gyre version, eqs. 10–11, scales `K_o` with the wind-stress curl squared.
pub(super) const ROSE_MARSHALL_OCEAN_DIFFUSIVITY_M2_PER_S: f64 = 5.2e5;

/// Earth's diffusive ocean conductance in the model's latitude form, `K⊕`, W m⁻² K⁻¹: Rose and
/// Marshall's `C_o K_o` over R⊕², 10⁷ × 5.2 × 10⁵ ÷ (6,371 km)² = 0.128 (decision-r09-t13i §2.3.2).
/// Like the atmosphere's Λ ([`transport`](super::transport)) it takes no term in the body's radius.
pub(super) const EARTH_OCEAN_CONDUCTANCE_W_PER_M2_K: f64 = ROSE_MARSHALL_OCEAN_CAPACITY_J_PER_M2_K
    * ROSE_MARSHALL_OCEAN_DIFFUSIVITY_M2_PER_S
    / (EARTH_RADIUS_M * EARTH_RADIUS_M);

/// The slowest rotation over Earth's at which the gyres' rotation factor is taken, Ω ÷ Ω⊕: 1 ÷ 24,
/// where it is held, `f_Ω` = 24^⅓ = 2.88 (decision-r09-t13i §2.3.2; the end of Kaspi and Showman's
/// rotation sweep, which the atmosphere's transport holds too). The factor is calibrated only to
/// Ω⊕ ÷ 10 (Cullum et al.'s 10-day run) and Ω⊕ ÷ 4 (Vallis and Farneti's Fig. 17a), so from 2.15 to
/// the held 2.88 it is extrapolated.
pub(super) const SLOWEST_GYRE_ROTATION: f64 = 1.0 / 24.0;

/// The land share of a latitude band at which its ocean is walled, its gyres and eddies at their
/// full diffusion: 0.2 (decision-r09-t13i §2.3.2; the ruling's, low confidence). Gyres need
/// meridional walls: with none there are no gyres and no main thermocline (Vallis and Farneti
/// 2009, Q. J. R. Meteorol. Soc. 135, 1643, as the ruling paraphrases them); Czaja and Marshall
/// 2006's channel against basin (J. Atmos. Sci. 63, 1498, eq. 17); Ferreira, Marshall and Campin
/// 2010's about 0 PW at 50° S under the gap (J. Climate 23, 1456); Enderton and Marshall 2009's
/// aquaplanet under 0.35 PW poleward of 50° (J. Atmos. Sci. 66, 1593). On Earth it brings 40° S
/// from the binary gate's −0.96 PW to −0.22, against Trenberth and Caron's −0.16.
pub(super) const WALLED_LAND_SHARE: f64 = 0.2;

/// The sea depth below which the ocean's transport falls with depth, m: 700, the main thermocline
/// (Vallis and Farneti 2009: the transport is insensitive to depth until it falls below it, as
/// decision-r09-t13i §2.3.3 paraphrases them; Hu and Yang 2014, PNAS 111, 629: "both zonal and
/// meridional ocean heat transports would largely decrease if ocean depth is shallower by one
/// order", read).
pub(super) const THERMOCLINE_DEPTH_M: f64 = 700.0;

/// The gyres' rotation factor `f_Ω` at `rate` times Earth's rotation: (Ω ÷ Ω⊕)^−⅓ below Earth's
/// rate, held at [`SLOWEST_GYRE_ROTATION`] (2.88), and 1 at and above it (decision-r09-t13i
/// §2.3.2): Cullum, Stevens and Joshi 2014's Q ∝ Ω^−⅓ from their scale analysis, within 3% of their
/// runs at 1, 3 and 10 days (Astrobiology 14, 645, Table 1, read; a buoyancy-forced ridgeplanet
/// with no wind, applied here to the wind-driven gyres' and eddies' diffusion); Vallis and Farneti
/// 2009's Fig. 17a, 1.6 times Earth's at Ω⊕ ÷ 4 and flat above Ω⊕ (read off the figure).
/// Extrapolated below Ω⊕ ÷ 10 to the held [`SLOWEST_GYRE_ROTATION`]. Medium-low confidence. A
/// rate that is not a number takes the held end.
#[must_use]
pub(super) fn rotation_factor(rate: f64) -> f64 {
    if rate >= 1.0 {
        return 1.0;
    }
    let held = if rate > SLOWEST_GYRE_ROTATION {
        rate
    } else {
        SLOWEST_GYRE_ROTATION
    };
    math::cbrt(1.0 / held)
}

/// The sea's liquid as the energy balance reads it (decision-r09-t13i §2.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct SeaLiquid {
    /// `r_l` = `ρ_l c_l` ÷ (`ρ_w c_w`): the sea's heat capacities' factor on water's.
    capacity_ratio: f64,
    /// `c_l`, J kg⁻¹ K⁻¹, which the Ekman cells carry.
    specific_heat_j_per_kg_k: f64,
    /// `c_l` ÷ `c_w`, which scales the gyres' diffusion.
    heat_ratio: f64,
}

impl SeaLiquid {
    /// The sea's liquid of the body of `inputs`, whose ice forms as `freezing` says: its main
    /// liquid's `r_l`, `c_l` and `c_l` ÷ `c_w` at the freezing point plus
    /// [`LIQUID_REFERENCE_ABOVE_FREEZING_K`] (at the mean surface temperature where no ice forms),
    /// each liquid held into its own column's range; water's sea where the body has no liquid or
    /// the registry no column for its liquid. Water's own is exactly 1, the same expression on the
    /// same row.
    ///
    /// # Panics
    ///
    /// If the registry lacks water's density or specific heat column, which its tests rule out.
    #[must_use]
    pub(super) fn of(inputs: &CoarseInputs, freezing: Freezing) -> Self {
        let reference_k = match freezing {
            Freezing::At { point_k } => point_k + LIQUID_REFERENCE_ABOVE_FREEZING_K,
            Freezing::Never { .. } => inputs.mean_surface_temperature().value(),
        };
        let water_id = crate::planetary::derive::atmosphere::Gas::Water.substance();
        let (water_density, water_heat) = density_and_heat(water_id, reference_k)
            .expect("the registry holds water's liquid density and specific heat");
        let liquid = inputs
            .liquids()
            .first()
            .and_then(|share| by_key(share.substance.as_str()))
            .unwrap_or(water_id);
        match density_and_heat(liquid, reference_k) {
            Some((density, heat)) => Self {
                capacity_ratio: (density * heat) / (water_density * water_heat),
                specific_heat_j_per_kg_k: heat,
                heat_ratio: heat / water_heat,
            },
            None => Self {
                capacity_ratio: 1.0,
                specific_heat_j_per_kg_k: water_heat,
                heat_ratio: 1.0,
            },
        }
    }

    /// `r_l`, the factor on the sea's heat capacities.
    #[must_use]
    pub(super) const fn capacity_ratio(&self) -> f64 {
        self.capacity_ratio
    }

    /// `c_l`, J kg⁻¹ K⁻¹.
    #[must_use]
    pub(super) const fn specific_heat_j_per_kg_k(&self) -> f64 {
        self.specific_heat_j_per_kg_k
    }

    /// `c_l` ÷ `c_w`.
    #[must_use]
    pub(super) const fn heat_ratio(&self) -> f64 {
        self.heat_ratio
    }
}

/// The liquid `liquid`'s density, kg m⁻³, and specific heat, J kg⁻¹ K⁻¹, at `t_k`, K, held into
/// its specific heat column's range where it is a pure liquid's (a solution's components are each
/// held into their own); `None` where the registry has either not. The density is the phase
/// diagram's saturated liquid's, the source of the held grain density of every liquid with a
/// specific heat column (`GrainDensity::OfDiagram`), which the record may not read before
/// P14.T54.a (`substance::optics`' tripwire).
#[must_use]
fn density_and_heat(liquid: SubstanceId, t_k: f64) -> Option<(f64, f64)> {
    let row = liquid.substance();
    let t = match row.liquid_specific_heat_range() {
        Some((low, high)) => t_k.clamp(low.value(), high.value()),
        None => t_k,
    };
    let heat = row.liquid_specific_heat_j_per_kg_k(Kelvin::new(t))?;
    let density = row.phase_diagram()?.liquid_density(Kelvin::new(t))?;
    Some((density.value(), heat))
}

/// Whether the sea's heat transport is taken (decision-r09-t13i §2.3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum OceanTransport {
    /// None: the model as R09.T13.a–g built it, which the tests' no-sea comparison reads (Kaspi and
    /// Showman's fit runs with no ocean grid at all).
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the step always takes the sea's transport; the tests compare the model \
                      without it"
        )
    )]
    None,
    /// The Ekman cells and the gyres' diffusion over a sea of mean depth `mean_depth_m`, m
    /// ([`mean_depth_m`]).
    Currents {
        /// The sea's mean depth, m.
        mean_depth_m: f64,
    },
}

/// The mean depth of the seas of field cells `depths_m`, each cell's water-surface height over its
/// ground where it is flooded (`None` where dry), over their solid angles `solid_angles_sr`,
/// summed in cell-index order: Σ Ω (`z_w` − z) ÷ Σ Ω over the flooded cells, m; zero where none is.
#[must_use]
pub(super) fn mean_depth_m(
    depths_m: impl IntoIterator<Item = Option<f64>>,
    solid_angles_sr: &[f64],
) -> f64 {
    let (sum, area) = depths_m.into_iter().zip(solid_angles_sr).fold(
        (0.0, 0.0),
        |(sum, area), (depth, &omega)| match depth {
            Some(d) => (sum + omega * d, area + omega),
            None => (sum, area),
        },
    );
    if area > 0.0 { sum / area } else { 0.0 }
}

/// The Ekman cells' part of [`Currents`]: the circulation whose Hadley share and capped centre
/// they read, `χ_d c_l`, and the return latitude.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Ekman {
    /// The body's Hadley circulation ([`Circulation::for_inputs`]).
    circulation: Circulation,
    /// `χ_d c_l`, J kg⁻¹ K⁻¹.
    heat_j_per_kg_k: f64,
    /// `φ_R`, degrees: the wind template's first node at the circulation's edge.
    return_deg: f64,
}

/// The ocean's heat transport of one body (decision-r09-t13i §2.3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Currents {
    /// The Ekman cells, or `None` on a world locked 1:1 to its host, whose circulation has no
    /// rotating Hadley cell and no cap.
    ekman: Option<Ekman>,
    /// `K_o χ_d` = `K⊕` (`c_l` ÷ `c_w`) `f_Ω` `χ_d`, W m⁻² K⁻¹, before the barrier and the sea's
    /// share. The barrier `χ_e` gates it where the Ekman cells run, on a rotating world; on a locked
    /// one χ = 1 (Hu and Yang 2014's aquaplanet carries 0.7–2.0 PW from day to night, read by the
    /// ruling).
    diffusive_w_per_m2_k: f64,
}

impl Currents {
    /// The currents of the body of `inputs`, whose ice forms as `freezing` says, over a sea of
    /// mean depth `mean_depth_m`, m, turning as `turning` says: its Ekman cells under its Hadley
    /// circulation ([`Circulation::for_inputs`]) on a rotating world, none on a locked one.
    #[must_use]
    pub(super) fn of(
        inputs: &CoarseInputs,
        freezing: Freezing,
        mean_depth_m: f64,
        turning: Turning,
    ) -> Self {
        let circulation = match turning {
            Turning::Rotating => Some(Circulation::for_inputs(inputs, turning)),
            Turning::Locked => None,
        };
        let liquid = SeaLiquid::of(inputs, freezing);
        let held = mean_depth_m / THERMOCLINE_DEPTH_M;
        // By comparison, so that a depth that is not a number gives no transport.
        let depth_factor = if held >= 1.0 {
            1.0
        } else if held > 0.0 {
            held
        } else {
            0.0
        };
        let rate = rotation_ratio(inputs.spin().rotation_period.value());
        Self {
            ekman: circulation.map(|circulation| Ekman {
                circulation,
                heat_j_per_kg_k: depth_factor * liquid.specific_heat_j_per_kg_k(),
                return_deg: wind::return_latitude_deg(circulation.edge_deg()),
            }),
            diffusive_w_per_m2_k: EARTH_OCEAN_CONDUCTANCE_W_PER_M2_K
                * liquid.heat_ratio()
                * rotation_factor(rate)
                * depth_factor,
        }
    }
}

/// Where a step's Ekman cells read the return temperature `T_R`, K (decision-r09-t13i §2.3.1).
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Returning {
    /// The state's own band-mean sea temperature at the return latitude: the steady state's and
    /// the first orbit's.
    Own,
    /// The previous orbit's, each band's least over the months of its band-mean sea temperature.
    Coldest(Box<[f64; LATITUDE_BANDS]>),
}

/// The ocean's heat transport on one grid: its per-edge constants, and the per-step terms the
/// latitude solve reads.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct OceanGrid {
    /// Each point's sea share at its southern edge, `s_e` = min(`s` south of it, its own): the
    /// sea's connectivity; zero in the southernmost band.
    edge_share: Vec<f64>,
    /// Each point's gyres' conductance at its southern edge, `K_o` `χ_e` `χ_d` `s_e` × the edge's
    /// cos φ ÷ Δφ, W m⁻² K⁻¹.
    diffusive: Vec<f64>,
    /// Each point's sea share, the weights of its band's sea temperature.
    sea_share: Vec<f64>,
    /// Each band edge's sea share, the mean of its points' `s_e` over the columns, which the Ekman
    /// cells' mass flux ψ reads ([`LATITUDE_BANDS`] + 1 values, zero at the poles).
    edge_sea: Vec<f64>,
    /// Each band edge's barrier factor `χ_e` = min(1, L̄ ÷ [`WALLED_LAND_SHARE`]) on a rotating world,
    /// 1 on a locked one ([`LATITUDE_BANDS`] + 1 values).
    barrier: Vec<f64>,
    /// The Ekman cells: `χ_d c_l` and the circulation; and the return bands, south and north:
    /// each the band holding `φ_R` in its hemisphere, or the nearest band with sea, equatorward
    /// first.
    ekman: Option<(Ekman, [usize; 2])>,
    /// Whether any edge carries a term.
    any: bool,
}

impl OceanGrid {
    /// The ocean of `currents` on `grid`, whose points' sea shares are `sea_share`.
    ///
    /// # Panics
    ///
    /// If `sea_share` does not hold one value for each of `grid`'s points.
    #[must_use]
    pub(super) fn new(grid: &ModelGrid, sea_share: Vec<f64>, currents: &Currents) -> Self {
        assert_eq!(sea_share.len(), grid.points(), "one sea share a point");
        let columns = grid.columns();
        let bands = grid.band_means(&sea_share);
        let walled = currents.ekman.is_some();
        let edge_share: Vec<f64> = (0..grid.points())
            .map(|p| {
                let k = p / columns;
                if k == 0 {
                    return 0.0;
                }
                let (south, here) = (sea_share[p - columns], sea_share[p]);
                if south < here { south } else { here }
            })
            .collect();
        let barrier: Vec<f64> = (0..=LATITUDE_BANDS)
            .map(|k| {
                if k == 0 || k == LATITUDE_BANDS || !walled {
                    return 1.0;
                }
                let land = 1.0 - f64::midpoint(bands[k - 1], bands[k]);
                let held = land / WALLED_LAND_SHARE;
                if held < 1.0 { held } else { 1.0 }
            })
            .collect();
        let diffusive: Vec<f64> = edge_share
            .iter()
            .enumerate()
            .map(|(p, &share)| {
                let k = p / columns;
                if k == 0 {
                    return 0.0;
                }
                currents.diffusive_w_per_m2_k * barrier[k] * share * grid.edge_conductance(k)
            })
            .collect();
        let edge_sea: Vec<f64> = (0..=LATITUDE_BANDS)
            .map(|k| {
                if k == 0 || k == LATITUDE_BANDS {
                    return 0.0;
                }
                let row = &edge_share[k * columns..(k + 1) * columns];
                row.iter().fold(0.0, |sum, s| sum + s) / super::grid::index(columns)
            })
            .collect();
        let has_sea = |k: usize| bands[k] > 0.0;
        let ekman = currents.ekman.map(|ekman| {
            let x = math::sin(ekman.return_deg * super::partition::DEGREE_RAD);
            let band = |sine: f64| {
                let held = ModelGrid::band_of(sine);
                super::hydrology::nearest_band(held, has_sea).unwrap_or(held)
            };
            (ekman, [band(-x), band(x)])
        });
        let any = edge_share.iter().any(|&s| s > 0.0)
            && (diffusive.iter().any(|&d| d > 0.0)
                || ekman.is_some_and(|(e, _)| e.heat_j_per_kg_k > 0.0));
        Self {
            edge_share,
            diffusive,
            sea_share,
            edge_sea,
            barrier,
            ekman,
            any,
        }
    }

    /// Whether any edge carries a term: false with no sea, at no depth, or with no transport.
    #[must_use]
    pub(super) const fn any(&self) -> bool {
        self.any
    }

    /// Point `point`'s sea share at its southern edge.
    #[must_use]
    pub(super) fn edge_share(&self, point: usize) -> f64 {
        self.edge_share[point]
    }

    /// Point `point`'s gyres' conductance at its southern edge, W m⁻² K⁻¹.
    #[must_use]
    pub(super) fn diffusive(&self, point: usize) -> f64 {
        self.diffusive[point]
    }

    /// Each band's sea temperature of the water-surface temperatures `water_k`, one a point of
    /// `grid`, K, into `out` (one a band): the sea-share-weighted mean over its points, summed
    /// column by column; a band with no sea takes the nearest band's that has one, equatorward
    /// first; zero where none has.
    pub(super) fn band_sea_k(&self, grid: &ModelGrid, water_k: &[f64], out: &mut [f64]) {
        let columns = grid.columns();
        let mut weights = [0.0; LATITUDE_BANDS];
        for (k, (row, shares)) in water_k
            .chunks_exact(columns)
            .zip(self.sea_share.chunks_exact(columns))
            .enumerate()
        {
            let (sum, weight) = row
                .iter()
                .zip(shares)
                .fold((0.0, 0.0), |(sum, weight), (&t, &s)| {
                    (sum + s * t, weight + s)
                });
            out[k] = sum;
            weights[k] = weight;
        }
        let mut means = [0.0; LATITUDE_BANDS];
        for (k, mean) in means.iter_mut().enumerate() {
            *mean = if weights[k] > 0.0 {
                out[k] / weights[k]
            } else {
                0.0
            };
        }
        for (k, value) in out.iter_mut().enumerate() {
            *value =
                super::hydrology::nearest_band(k, |b| weights[b] > 0.0).map_or(0.0, |b| means[b]);
        }
    }

    /// The bands' return temperatures, K, under `returning`, the state's own band sea temperatures
    /// `band_sea_k` or the previous orbit's coldest month's.
    #[must_use]
    fn cold_k<'b>(band_sea_k: &'b [f64], returning: &'b Returning) -> &'b [f64] {
        match returning {
            Returning::Own => band_sea_k,
            Returning::Coldest(coldest) => coldest.as_slice(),
        }
    }

    /// The Ekman cells' gain at each band edge, G, into `gains` ([`LATITUDE_BANDS`] + 1 values):
    /// γ = `s_e` G at a point's edge, G = `χ_d c_l` w `ΔT_O` ÷ (`h_T` − h̄), from the bands' mean h
    /// `band_energy`, J kg⁻¹, and sea temperatures `band_sea_k`, K, under the transport coefficient
    /// `lambda`, kg m⁻² s⁻¹, and the return temperatures `returning` (decision-r09-t13i §2.3.1 and
    /// its §2.2/§2.3 follow-up 5 §1): w the partition's Hadley share about the capped centre of the
    /// bands' energy-flux equator, `h_T` = 1.06 max h̄, h̄ and `T_s` the edge's two bands' means,
    /// `ΔT_O` the walls' blend [`ekman_contrast_k`] of the open ocean's contrast to `T_R`, the edge's
    /// hemisphere's return temperature (the north's at the equator), and the walled basin's to the
    /// water subducted between the edge and the return band, each band's share of it the decrement
    /// of the poleward Ekman mass flux ψ = s w F ÷ (`h_T` − h̄) across it ([`subducted_k`]; ψ taken as
    /// nothing past the return band). Zero at every edge with no Ekman cells, at the poles, and
    /// wherever a band's h is not finite (a state that is no longer finite stops its run at the
    /// orbit's end).
    pub(super) fn ekman_gains(
        &self,
        grid: &ModelGrid,
        lambda: f64,
        band_energy: &[f64],
        band_sea_k: &[f64],
        returning: &Returning,
        gains: &mut [f64],
    ) {
        gains.fill(0.0);
        let Some((ekman, [south_band, north_band])) = self.ekman else {
            return;
        };
        if !band_energy.iter().all(|h| h.is_finite()) {
            return;
        }
        let mut flux = [0.0; LATITUDE_BANDS + 1];
        edge_fluxes_into(grid, lambda, band_energy, &mut flux);
        let efe = energy_flux_equator(grid, &flux, band_energy);
        let circulation = ekman.circulation;
        let centre = circulation.centre_sine(efe);
        let greatest = band_energy.iter().fold(
            f64::NEG_INFINITY,
            |most, &h| if h > most { h } else { most },
        );
        let upper = UPPER_BRANCH_ENERGY_RATIO * greatest;
        let cold = Self::cold_k(band_sea_k, returning);
        let [south, north] = [cold[south_band], cold[north_band]];
        // The poleward Ekman mass flux at each edge, its positive part.
        let mut psi = [0.0; LATITUDE_BANDS + 1];
        for (edge, mass) in psi.iter_mut().enumerate().take(LATITUDE_BANDS).skip(1) {
            let x = grid.edge_sine(edge);
            let share = circulation.share(x, centre);
            let energy = f64::midpoint(band_energy[edge - 1], band_energy[edge]);
            let poleward = if x >= 0.0 { flux[edge] } else { -flux[edge] };
            let value = self.edge_sea[edge] * share * poleward / (upper - energy);
            *mass = if value > 0.0 { value } else { 0.0 };
        }
        for (edge, gain) in gains.iter_mut().enumerate().take(LATITUDE_BANDS).skip(1) {
            let x = grid.edge_sine(edge);
            let share = circulation.share(x, centre);
            let sea = f64::midpoint(band_sea_k[edge - 1], band_sea_k[edge]);
            // Each band between the edge and the return band, poleward, with the flux at its two
            // edges: band k north of edge k in the north, band k − 1 south of it in the south.
            let subducted = if x >= 0.0 {
                subducted_k((edge..=north_band).map(|k| {
                    let next = if k < north_band { psi[k + 1] } else { 0.0 };
                    (psi[k], next, cold[k])
                }))
            } else {
                subducted_k((south_band + 1..=edge).rev().map(|k| {
                    let next = if k > south_band + 1 { psi[k - 1] } else { 0.0 };
                    (psi[k], next, cold[k - 1])
                }))
            };
            let walled = subducted.map_or(0.0, |t| return_contrast_k(sea, t));
            let open = return_contrast_k(sea, if x >= 0.0 { north } else { south });
            let contrast = ekman_contrast_k(self.barrier[edge], walled, open);
            let energy = f64::midpoint(band_energy[edge - 1], band_energy[edge]);
            *gain = ekman.heat_j_per_kg_k * share * contrast / (upper - energy);
        }
    }
}

impl OceanGrid {
    /// The Ekman cells at edge `edge` of the bands' mean h `band_energy`, J kg⁻¹, sea temperatures
    /// `band_sea_k` and return temperatures `returning`, K, under the transport coefficient `lambda`,
    /// kg m⁻² s⁻¹: their poleward mass flux over the sea, ψ = s w F ÷ (`h_T` − h̄), kg s⁻¹ per 2πR²
    /// of the sphere, and the open ocean's contrast `ΔT_D`, K; `None` with no Ekman cells. The
    /// aquaplanet's print beside Enderton and Marshall's (decision-r09-t13i's §2.4 follow-up 6 §6).
    #[cfg(test)]
    #[must_use]
    pub(super) fn ekman_at(
        &self,
        grid: &ModelGrid,
        lambda: f64,
        band_energy: &[f64],
        band_sea_k: &[f64],
        returning: &Returning,
        edge: usize,
    ) -> Option<(f64, f64)> {
        let (ekman, [south_band, north_band]) = self.ekman?;
        let flux = edge_fluxes(grid, lambda, band_energy);
        let efe = energy_flux_equator(grid, &flux, band_energy);
        let centre = ekman.circulation.centre_sine(efe);
        let greatest = band_energy.iter().fold(
            f64::NEG_INFINITY,
            |most, &h| if h > most { h } else { most },
        );
        let x = grid.edge_sine(edge);
        let share = ekman.circulation.share(x, centre);
        let energy = f64::midpoint(band_energy[edge - 1], band_energy[edge]);
        let poleward = if x >= 0.0 { flux[edge] } else { -flux[edge] };
        let psi = self.edge_sea[edge] * share * poleward
            / (UPPER_BRANCH_ENERGY_RATIO * greatest - energy);
        let cold = Self::cold_k(band_sea_k, returning);
        let sea = f64::midpoint(band_sea_k[edge - 1], band_sea_k[edge]);
        let open = return_contrast_k(
            sea,
            if x >= 0.0 {
                cold[north_band]
            } else {
                cold[south_band]
            },
        );
        Some((psi, open))
    }
}

/// The temperature contrast of an edge's sea, `sea_k`, K, over the water that returns beneath it,
/// `return_k`, K: max(0, `T_s` − `T_ret`). Against the return latitude's coldest month it is the
/// open ocean's `ΔT_D` (decision-r09-t13i §2.3.1), against the subducted water's temperature the
/// walled basin's `ΔT_S` ([`subducted_k`]).
#[must_use]
pub(super) fn return_contrast_k(sea_k: f64, return_k: f64) -> f64 {
    let excess = sea_k - return_k;
    if excess > 0.0 { excess } else { 0.0 }
}

/// The temperature of the water a walled basin's Ekman cells return beneath an edge, K, Held 2001's
/// eq. 7 discretised (J. Atmos. Sci. 58, 943; Klinger and Marotzke 2000, J. Phys. Oceanogr. 30, 696,
/// eq. 11, its integral over temperature of the Ekman transport): "all of the water subducted
/// poleward of this latitude, with each water mass carrying the temperature of the surface water at
/// the latitude of subduction". The bands from the edge poleward to the return band, `bands`, each
/// the poleward Ekman mass flux ψ at its equatorward and poleward edges (the last taken as
/// nothing, Held's V(1) = 0) and its temperature, K: Σ d T ÷ Σ d with d = max(0, ψ − `ψ_next`), the
/// water subducted into each band; `None` where none is (decision-r09-t13i's §2.2/§2.3 follow-up
/// 5 §1). The coldest month's temperatures, as subduction draws the late-winter mixed layer
/// (Stommel 1979, from the ruling's memory). Summed poleward in band order.
#[must_use]
pub(super) fn subducted_k(bands: impl IntoIterator<Item = (f64, f64, f64)>) -> Option<f64> {
    let (sum, weight) = bands
        .into_iter()
        .fold((0.0, 0.0), |(sum, weight), (psi, next, t)| {
            let decrement = psi - next;
            let d = if decrement > 0.0 { decrement } else { 0.0 };
            (sum + d * t, weight + d)
        });
    (weight > 0.0).then(|| sum / weight)
}

/// The Ekman cells' gross oceanic stability at an edge of barrier factor `barrier` (`χ_e`, §2.3.2's
/// min(1, L̄ ÷ 0.2)), K: `χ_e` `ΔT_S` + (1 − `χ_e`) `ΔT_D`, the walled basin's contrast to its
/// subducted water `walled_k` and the open ocean's to the return latitude's coldest month
/// `open_k` (decision-r09-t13i's §2.2/§2.3 follow-up 5 §1). The regimes are Enderton and Marshall
/// 2009's (J. Atmos. Sci. 66, 1593, §3): with no walls the cells reach the ocean's bottom and carry
/// 2.4 PW at 15° in their Aqua, near the whole contrast; one wall makes them weaker and "much
/// shallower", and Klinger and Marotzke's eq. 11 matches a walled basin's GCM. Earth's tropical
/// edges are walled, so Earth takes Held's eq. 7: about a quarter of its observed NH tropical ocean
/// transport, the rest the overturning's and the boundary currents' (I6). Low-medium confidence, the
/// blend linear in a low-confidence factor.
#[must_use]
pub(super) fn ekman_contrast_k(barrier: f64, walled_k: f64, open_k: f64) -> f64 {
    barrier * walled_k + (1.0 - barrier) * open_k
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planetary::surface::reference;
    use hyperion_testkit::float::assert_same_bits;

    /// The index `k` as an `f64`.
    fn at(k: usize) -> f64 {
        super::super::grid::index(k)
    }

    /// The gyres' rotation factor (decision-r09-t13i §2.3.2): 1 at and above Earth's rate, (Ω ÷
    /// Ω⊕)^−⅓ below it, held at Ω⊕ ÷ 24 (24^⅓ = 2.884), and the held end for a rate that is not a
    /// number.
    #[test]
    fn the_gyres_rotation_factor_is_a_cube_root_held_at_a_twenty_fourth() {
        assert_same_bits(rotation_factor(1.0), 1.0);
        assert_same_bits(rotation_factor(3.0), 1.0);
        assert!((rotation_factor(0.25) - math::cbrt(4.0)).abs() < 1e-15);
        let held = rotation_factor(0.001);
        assert!((held - 2.884_499_140_614_816).abs() < 1e-12, "{held}");
        assert_same_bits(rotation_factor(SLOWEST_GYRE_ROTATION), held);
        assert_same_bits(rotation_factor(f64::NAN), held);
    }

    /// The sea's mean depth: its flooded cells' depths weighted by solid angle, in cell order;
    /// zero with none.
    #[test]
    fn the_mean_depth_weights_the_flooded_cells_by_solid_angle() {
        let depth = mean_depth_m([Some(1_000.0), None, Some(4_000.0)], &[1.0, 5.0, 3.0]);
        assert_same_bits(depth, (1_000.0 + 3.0 * 4_000.0) / 4.0);
        assert_same_bits(mean_depth_m([None, None], &[1.0, 1.0]), 0.0);
    }

    /// The currents (decision-r09-t13i §2.3.2–§2.3.3): both terms times min(1, d̄ ÷ 700 m), none
    /// for a depth that is not a number; Earth's diffusion `K⊕` at its own rate and water's ratio;
    /// the Ekman cells on a rotating world only, returning at the template's first node, 35.4°.
    #[test]
    fn the_currents_scale_with_depth_and_turn_only_where_the_world_rotates() {
        let earth = reference::earth_like();
        let freezing = Freezing::of(&earth);
        let deep = Currents::of(&earth, freezing, 3_689.0, Turning::Rotating);
        let shallow = Currents::of(&earth, freezing, 350.0, Turning::Rotating);
        let none = Currents::of(&earth, freezing, f64::NAN, Turning::Rotating);
        let locked = Currents::of(&earth, freezing, 3_689.0, Turning::Locked);
        // Water's ratio is exactly 1; Earth's rotation ratio is its sidereal day's over the
        // template's, within 10⁻⁵ of 1.
        let rate = rotation_ratio(earth.spin().rotation_period.value());
        assert!((rate - 1.0).abs() < 1e-5, "{rate}");
        assert_same_bits(
            deep.diffusive_w_per_m2_k,
            EARTH_OCEAN_CONDUCTANCE_W_PER_M2_K * 1.0 * rotation_factor(rate) * 1.0,
        );
        assert!((shallow.diffusive_w_per_m2_k / deep.diffusive_w_per_m2_k - 0.5).abs() < 1e-15);
        assert_same_bits(none.diffusive_w_per_m2_k, 0.0);
        assert!(locked.ekman.is_none());
        let (deep, shallow) = (deep.ekman.unwrap(), shallow.ekman.unwrap());
        assert!((shallow.heat_j_per_kg_k / deep.heat_j_per_kg_k - 0.5).abs() < 1e-15);
        assert!((deep.return_deg - 35.4).abs() < 0.05, "{}", deep.return_deg);
    }

    /// The ocean on a grid (decision-r09-t13i §2.3.2): each edge's sea share the lesser of its two
    /// points' and none in the southern band; the barrier min(1, L̄ ÷ 0.2) of the edge's two bands
    /// on a rotating world and 1 on a locked one; a band with no sea takes its nearest band's sea
    /// temperature, equatorward first.
    #[test]
    fn the_grid_s_edges_take_the_lesser_sea_and_the_walls_their_land() {
        let earth = reference::earth_like();
        let freezing = Freezing::of(&earth);
        let grid = ModelGrid::zonal();
        let sea: Vec<f64> = (0..LATITUDE_BANDS)
            .map(|k| match k {
                _ if k < 10 => 0.0,
                _ if k % 2 == 0 => 0.9,
                _ => 0.6,
            })
            .collect();
        let currents = |turning| Currents::of(&earth, freezing, 3_689.0, turning);
        let rotating = OceanGrid::new(&grid, sea.clone(), &currents(Turning::Rotating));
        let locked = OceanGrid::new(&grid, sea.clone(), &currents(Turning::Locked));
        assert_same_bits(rotating.edge_share(0), 0.0);
        assert_same_bits(rotating.diffusive(0), 0.0);
        for k in 1..LATITUDE_BANDS {
            let lesser = if sea[k - 1] < sea[k] {
                sea[k - 1]
            } else {
                sea[k]
            };
            assert_same_bits(rotating.edge_share(k), lesser);
            let land = 1.0 - f64::midpoint(sea[k - 1], sea[k]);
            let walled = land / WALLED_LAND_SHARE;
            assert_same_bits(rotating.barrier[k], if walled < 1.0 { walled } else { 1.0 });
            assert_same_bits(locked.barrier[k], 1.0);
        }
        let water: Vec<f64> = (0..LATITUDE_BANDS).map(|k| 250.0 + at(k)).collect();
        let mut bands = vec![0.0; LATITUDE_BANDS];
        rotating.band_sea_k(&grid, &water, &mut bands);
        for k in 0..10 {
            assert_same_bits(bands[k], bands[10]);
        }
        for k in 10..LATITUDE_BANDS {
            assert!((bands[k] - water[k]).abs() < 1e-12, "{k}");
        }
    }

    /// The Ekman cells' gain (decision-r09-t13i §2.3.1): positive inside the Hadley cell, zero at
    /// the poles, everywhere on a locked world, and everywhere where a band's h is not finite.
    #[test]
    fn the_ekman_gain_vanishes_at_the_poles_without_cells_and_on_a_state_that_is_not_finite() {
        let earth = reference::earth_like();
        let freezing = Freezing::of(&earth);
        let grid = ModelGrid::zonal();
        let sea = vec![0.7; LATITUDE_BANDS];
        let currents = |turning| Currents::of(&earth, freezing, 3_689.0, turning);
        let rotating = OceanGrid::new(&grid, sea.clone(), &currents(Turning::Rotating));
        let locked = OceanGrid::new(&grid, sea, &currents(Turning::Locked));
        let cosine = |k: usize| grid.centre_cosine(k);
        let energy: Vec<f64> = (0..LATITUDE_BANDS)
            .map(|k| 2.6e5 + 7.0e4 * cosine(k) * cosine(k))
            .collect();
        let water: Vec<f64> = (0..LATITUDE_BANDS)
            .map(|k| 250.0 + 50.0 * cosine(k) * cosine(k))
            .collect();
        let mut gains = vec![1.0; LATITUDE_BANDS + 1];
        rotating.ekman_gains(&grid, 1.0, &energy, &water, &Returning::Own, &mut gains);
        assert_same_bits(gains[0], 0.0);
        assert_same_bits(gains[LATITUDE_BANDS], 0.0);
        assert!(gains.iter().any(|&g| g > 0.0), "{gains:?}");
        assert!(gains.iter().all(|&g| g >= 0.0));
        locked.ekman_gains(&grid, 1.0, &energy, &water, &Returning::Own, &mut gains);
        assert!(gains.iter().all(|&g| g.total_cmp(&0.0).is_eq()));
        let mut broken = energy.clone();
        broken[30] = f64::NAN;
        rotating.ekman_gains(&grid, 1.0, &broken, &water, &Returning::Own, &mut gains);
        assert!(gains.iter().all(|&g| g.total_cmp(&0.0).is_eq()));
    }

    /// Held's gross oceanic stability (decision-r09-t13i's §2.2/§2.3 follow-up 5 §1): with his
    /// idealised profiles, V ∝ 1 − y and T = `T₀` − ΔT y² on y the sine latitude over the cell's
    /// edge's, the walled basin's contrast `T_s` − `T_sub` at y is his eq. 9, ΔT (1 + y − 2y²) ÷ 3, to
    /// the midpoint sum's error over 4,000 bands.
    #[test]
    fn the_walled_basin_s_return_water_is_held_s_eq_9_on_his_profiles() {
        let n = 4_000;
        let (surface, contrast) = (300.0, 30.0);
        let temperature = |y: f64| surface - contrast * y * y;
        let psi = |k: usize| 1.0 - at(k) / at(n);
        for start in [0, 1_000, 2_000, 3_200] {
            let y = at(start) / at(n);
            let returned = subducted_k(
                (start..n).map(|k| (psi(k), psi(k + 1), temperature((at(k) + 0.5) / at(n)))),
            )
            .expect("water subducts poleward of the edge");
            let walled = return_contrast_k(temperature(y), returned);
            let held = contrast * (1.0 + y - 2.0 * y * y) / 3.0;
            eprintln!("y {y:.2}: the walled contrast {walled:.6} K, Held's eq. 9 {held:.6} K");
            assert!(
                (walled - held).abs() < 1e-6 * contrast,
                "{walled} against {held}"
            );
        }
    }

    /// The walls' blend (decision-r09-t13i's §2.2/§2.3 follow-up 5 §1): χ = 1 gives the walled
    /// basin's contrast and χ = 0 the open ocean's, each to the bit; no subducted water gives none.
    #[test]
    fn the_walls_blend_gives_each_regime_to_the_bit() {
        for (walled, open) in [(1.25, 7.5), (0.0, 3.0), (4.0, 0.0)] {
            assert_same_bits(ekman_contrast_k(1.0, walled, open), walled);
            assert_same_bits(ekman_contrast_k(0.0, walled, open), open);
        }
        assert!(subducted_k([(0.0, 0.0, 280.0), (0.2, 0.5, 270.0)]).is_none());
        assert_same_bits(subducted_k([(0.5, 0.2, 280.0)]).unwrap(), 280.0);
    }

    /// A sea whose liquid the registry has no specific heat for takes water's: its ratios 1
    /// (decision-r09-t13i §2.1). A basalt melt has none.
    #[test]
    fn a_sea_with_no_specific_heat_takes_water_s() {
        let basalt = by_key("basalt").expect("the registry holds basalt");
        assert!(density_and_heat(basalt, 1_500.0).is_none());
        let water = by_key("H2O").expect("the registry holds water");
        assert!(density_and_heat(water, 283.15).is_some());
    }
}
