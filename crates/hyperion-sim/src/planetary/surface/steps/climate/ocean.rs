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
//! takes water's ratio, 1, and its record says so ([`SeaLiquid::has_column`]). The mixed layer is
//! kept at 50 m whatever the liquid, low confidence for another world's winds.
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
//!   state, inside the tridiagonal solve. `ΔT_O` = max(0, `T_s` − `T_R`): `T_s` the edge's sea
//!   temperature, `T_R` the coldest month's band-mean sea temperature at the return latitude `φ_R`,
//!   the wind template's first node, where the Hadley and Ferrel cells meet at the surface
//!   ([`wind::return_latitude_deg`](super::wind::return_latitude_deg); 35.4° on Earth), from the
//!   previous orbit; the steady state and the first orbit take their own state's.
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
use super::partition::{
    Circulation, UPPER_BRANCH_ENERGY_RATIO, edge_fluxes, energy_flux_equator, hadley_share,
};
use super::transport::rotation_ratio;
use super::wind;
use crate::math;
use crate::planetary::surface::inputs::CoarseInputs;
use crate::substance::{CondensedPhase, SubstanceId, by_key};
use crate::units::Kelvin;
use crate::units::consts::EARTH_RADIUS_M;

/// How far above the model's freezing point the sea's liquid and water are compared, K: 10
/// (decision-r09-t13i §2.1): `r_l` and `c_l` ÷ `c_w` are taken at `T_f` + 10 K, a sea just above
/// freezing, each liquid's temperature held into its specific heat column's range.
pub(super) const LIQUID_REFERENCE_ABOVE_FREEZING_K: f64 = 10.0;

/// The ocean's mixed-layer heat capacity in Rose and Marshall's energy-balance model, J m⁻² K⁻¹:
/// 10⁷ (Rose, B. E. J., and Marshall, J. 2009, "Ocean Heat Transport, Sea Ice, and Multiple
/// Climate States: Insights from Energy Balance Models", J. Atmos. Sci. 66, 2828–2843, eqs. 10–11
/// and their `C_o`, read by decision-r09-t13i).
pub(super) const ROSE_MARSHALL_OCEAN_CAPACITY_J_PER_M2_K: f64 = 1.0e7;

/// The ocean's constant diffusivity in Rose and Marshall's energy-balance model, m² s⁻¹: 5.2 × 10⁵
/// (Rose and Marshall 2009, their `K_o`, read by decision-r09-t13i); its gyre version scales `K_o`
/// with the wind-stress curl squared.
pub(super) const ROSE_MARSHALL_OCEAN_DIFFUSIVITY_M2_PER_S: f64 = 5.2e5;

/// Earth's diffusive ocean conductance in the model's latitude form, `K⊕`, W m⁻² K⁻¹: Rose and
/// Marshall's `C_o K_o` over R⊕², 10⁷ × 5.2 × 10⁵ ÷ (6,371 km)² = 0.128 (decision-r09-t13i §2.3.2).
/// Like the atmosphere's Λ ([`transport`](super::transport)) it takes no term in the body's radius.
pub(super) const EARTH_OCEAN_CONDUCTANCE_W_PER_M2_K: f64 = ROSE_MARSHALL_OCEAN_CAPACITY_J_PER_M2_K
    * ROSE_MARSHALL_OCEAN_DIFFUSIVITY_M2_PER_S
    / (EARTH_RADIUS_M * EARTH_RADIUS_M);

/// The slowest rotation over Earth's at which the gyres' rotation factor is taken, Ω ÷ Ω⊕: 1 ÷ 24,
/// where it is held, `f_Ω` = 24^⅓ = 2.88 (decision-r09-t13i §2.3.2; the end of Kaspi and Showman's
/// rotation sweep, which the atmosphere's transport holds too).
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
/// decision-r09-t13i §2.3.3 paraphrases them; Hu and Yang 2014, PNAS 111, 629: "Ocean heat
/// transports would largely decrease if ocean depth is shallower by one order", read).
pub(super) const THERMOCLINE_DEPTH_M: f64 = 700.0;

/// The gyres' rotation factor `f_Ω` at `rate` times Earth's rotation: (Ω ÷ Ω⊕)^−⅓ below Earth's
/// rate, held at [`SLOWEST_GYRE_ROTATION`] (2.88), and 1 at and above it (decision-r09-t13i
/// §2.3.2): Cullum, Stevens and Joshi 2014's Q ∝ Ω^−⅓ from their scale analysis, within 3% of their
/// runs at 1, 3 and 10 days (Astrobiology 14, 645, read by the ruling); Vallis and Farneti 2009's
/// Fig. 17, 1.6 times Earth's at Ω⊕ ÷ 4 and flat above Ω⊕ (read off the figure). Medium-low
/// confidence. A rate that is not a number takes the held end.
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
    /// Whether the registry holds the liquid's specific heat; where it does not, the sea is
    /// water's.
    has_column: bool,
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
                has_column: true,
            },
            None => Self {
                capacity_ratio: 1.0,
                specific_heat_j_per_kg_k: water_heat,
                heat_ratio: 1.0,
                has_column: false,
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

    /// Whether the registry holds the liquid's specific heat.
    #[cfg(test)]
    #[must_use]
    pub(super) const fn has_column(&self) -> bool {
        self.has_column
    }
}

/// The liquid `liquid`'s density, kg m⁻³, and specific heat, J kg⁻¹ K⁻¹, at `t_k`, K, held into
/// its specific heat column's range where it is a pure liquid's (a solution's components are each
/// held into their own); `None` where the registry has either not.
#[must_use]
fn density_and_heat(liquid: SubstanceId, t_k: f64) -> Option<(f64, f64)> {
    let row = liquid.substance();
    let t = match row.liquid_specific_heat_range() {
        Some((low, high)) => t_k.clamp(low.value(), high.value()),
        None => t_k,
    };
    let heat = row.liquid_specific_heat_j_per_kg_k(Kelvin::new(t))?;
    let density = row.grain_density(CondensedPhase::Liquid, Kelvin::new(t))?;
    Some((density.value(), heat))
}

/// Whether the sea's heat transport is taken (decision-r09-t13i §2.3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum OceanTransport {
    /// None: the model as R09.T13.a–g built it, which Kaspi and Showman's fit and the tests' no-sea
    /// comparison read.
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
    /// share.
    diffusive_w_per_m2_k: f64,
    /// Whether the barrier `χ_e` gates the diffusion: on a rotating world; on a locked one χ = 1
    /// (Hu and Yang 2014's aquaplanet carries 0.7–2.0 PW from day to night, read by the ruling).
    walled: bool,
}

impl Currents {
    /// The currents of the body of `inputs`, whose ice forms as `freezing` says, over a sea of
    /// mean depth `mean_depth_m`, m, its Ekman cells under `circulation` on a rotating world
    /// (`None` on a locked one).
    #[must_use]
    pub(super) fn of(
        inputs: &CoarseInputs,
        freezing: Freezing,
        mean_depth_m: f64,
        circulation: Option<Circulation>,
    ) -> Self {
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
            walled: circulation.is_some(),
        }
    }
}

/// Where a step's Ekman cells read the return temperature `T_R`, K (decision-r09-t13i §2.3.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Returning {
    /// The state's own band-mean sea temperature at the return latitude: the steady state's and
    /// the first orbit's.
    Own,
    /// The previous orbit's coldest month's, south and north.
    Coldest([f64; 2]),
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
    pub(super) fn new(grid: &ModelGrid, sea_share: &[f64], currents: &Currents) -> Self {
        assert_eq!(sea_share.len(), grid.points(), "one sea share a point");
        let columns = grid.columns();
        let bands = grid.band_means(sea_share);
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
        let diffusive: Vec<f64> = edge_share
            .iter()
            .enumerate()
            .map(|(p, &share)| {
                let k = p / columns;
                if k == 0 {
                    return 0.0;
                }
                let barrier = if currents.walled {
                    let land = 1.0 - f64::midpoint(bands[k - 1], bands[k]);
                    let held = land / WALLED_LAND_SHARE;
                    if held < 1.0 { held } else { 1.0 }
                } else {
                    1.0
                };
                currents.diffusive_w_per_m2_k * barrier * share * grid.edge_conductance(k)
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
            sea_share: sea_share.to_vec(),
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
        let means: Vec<f64> = (0..LATITUDE_BANDS)
            .map(|k| {
                if weights[k] > 0.0 {
                    out[k] / weights[k]
                } else {
                    0.0
                }
            })
            .collect();
        for (k, value) in out.iter_mut().enumerate() {
            *value =
                super::hydrology::nearest_band(k, |b| weights[b] > 0.0).map_or(0.0, |b| means[b]);
        }
    }

    /// The return temperatures, south and north, K, of the bands' sea temperatures `band_sea_k`:
    /// each hemisphere's return band's; zero with no Ekman cells.
    #[must_use]
    pub(super) fn return_k(&self, band_sea_k: &[f64]) -> [f64; 2] {
        self.ekman.map_or([0.0; 2], |(_, [south, north])| {
            [band_sea_k[south], band_sea_k[north]]
        })
    }

    /// The Ekman cells' gain at each band edge, G, into `gains` ([`LATITUDE_BANDS`] + 1 values):
    /// γ = `s_e` G at a point's edge, G = `χ_d c_l` w max(0, `T_s` − `T_R`) ÷ (`h_T` − h̄), from the
    /// bands' mean h `band_energy`, J kg⁻¹, and sea temperatures `band_sea_k`, K, under the
    /// transport coefficient `lambda`, kg m⁻² s⁻¹, and the return temperatures `returning`
    /// (decision-r09-t13i §2.3.1): w the partition's Hadley share about the capped centre of the
    /// bands' energy-flux equator, `h_T` = 1.06 max h̄, h̄ and `T_s` the edge's two bands' means,
    /// `T_R` the edge's hemisphere's (the north's at the equator). Zero at every edge with no Ekman
    /// cells, and at the poles.
    pub(super) fn ekman_gains(
        &self,
        grid: &ModelGrid,
        lambda: f64,
        band_energy: &[f64],
        band_sea_k: &[f64],
        returning: Returning,
        gains: &mut [f64],
    ) {
        gains.fill(0.0);
        let Some((ekman, _)) = self.ekman else {
            return;
        };
        let flux = edge_fluxes(grid, lambda, band_energy);
        let efe = energy_flux_equator(grid, &flux, band_energy);
        let circulation = ekman.circulation;
        let centre = circulation.centre_sine(efe);
        let greatest = band_energy.iter().fold(
            f64::NEG_INFINITY,
            |most, &h| if h > most { h } else { most },
        );
        let upper = UPPER_BRANCH_ENERGY_RATIO * greatest;
        let [south, north] = match returning {
            Returning::Own => self.return_k(band_sea_k),
            Returning::Coldest(held) => held,
        };
        for (edge, gain) in gains.iter_mut().enumerate().take(LATITUDE_BANDS).skip(1) {
            let x = grid.edge_sine(edge);
            let share = hadley_share(x, centre, circulation.width());
            let sea = f64::midpoint(band_sea_k[edge - 1], band_sea_k[edge]);
            let excess = sea - if x >= 0.0 { north } else { south };
            let contrast = if excess > 0.0 { excess } else { 0.0 };
            let energy = f64::midpoint(band_energy[edge - 1], band_energy[edge]);
            *gain = ekman.heat_j_per_kg_k * share * contrast / (upper - energy);
        }
    }
}
