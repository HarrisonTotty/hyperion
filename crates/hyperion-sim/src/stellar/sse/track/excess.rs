//! The helium-excess hook (plan 06, P06.T17): how a star's excess of helium ΔY
//! (`Composition::helium_excess`) enters its track, through
//! [`tables::helium`](crate::tables::helium).
//!
//! - The main sequence's timescales (its lifetime at each mass, which the effective-age rule reads)
//!   and those of the Hertzsprung gap and the first giant branch are multiplied by
//!   exp(s(m, Z) ΔY), with s from `LIFETIME_SLOPE`: a quadratic in log₁₀ m at four metallicities,
//!   interpolated linearly in log₁₀ Z and held beyond the end nodes. m is the mass the phase's
//!   formulae read: the current mass on the main sequence, the mass the gap was entered with after
//!   it.
//! - On the horizontal branch (core helium burning) log₁₀ `T_eff` is shifted by
//!   `HB_TEMPERATURE_SHIFT` (linear in the envelope mass, M − `M_c`) × ΔY, with the radius changed to
//!   keep the luminosity.
//!
//! The hook exists only for ΔY > 0 ([`HeliumHook::of`]); for every other star the track takes no
//! step of it, so the stars the grid places, all of ΔY = 0, are bit-identical whatever the table
//! holds. The committed table is the identity (P06.T17); plan 15's P15.T7 fits it.

use crate::math;
use crate::stellar::Composition;
use crate::stellar::sse::PhasePoint;
use crate::tables::helium;
use crate::units::{SolarMasses, SolarRadii};

/// A helium-excess table in [`tables::helium`](crate::tables::helium)'s shape.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct HeliumTable {
    /// log₁₀ Z at the four nodes, increasing.
    pub(crate) log_z_nodes: [f64; 4],
    /// (c₀, c₁, c₂) of s at each node.
    pub(crate) lifetime_slope: [[f64; 3]; 4],
    /// (a, b) of the horizontal branch's shift of log₁₀ `T_eff` per unit ΔY, a + b `M_env`.
    pub(crate) hb_temperature_shift: [f64; 2],
}

impl HeliumTable {
    /// The committed table, [`tables::helium`](crate::tables::helium).
    pub(crate) const COMMITTED: Self = Self {
        log_z_nodes: helium::LOG_Z_NODES,
        lifetime_slope: helium::LIFETIME_SLOPE,
        hb_temperature_shift: helium::HB_TEMPERATURE_SHIFT,
    };
}

/// One star's helium-excess correction: its ΔY, its metallicity and the table.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct HeliumHook {
    delta_y: f64,
    /// The slope's coefficients at the star's metallicity.
    slope: [f64; 3],
    shift: [f64; 2],
}

impl HeliumHook {
    /// The correction of a star of `composition` from `table`, or `None` unless its ΔY is
    /// positive.
    #[must_use]
    pub(crate) fn of(composition: &Composition, table: &HeliumTable) -> Option<Self> {
        let delta_y = composition.helium_excess().value();
        if delta_y <= 0.0 {
            return None;
        }
        let log_z = math::log10(composition.z_fit().value());
        let nodes = &table.log_z_nodes;
        let last = nodes.len() - 1;
        let slope = if log_z <= nodes[0] {
            table.lifetime_slope[0]
        } else if log_z >= nodes[last] {
            table.lifetime_slope[last]
        } else {
            let index = nodes[1..last]
                .iter()
                .take_while(|&&node| log_z >= node)
                .count();
            let share = (log_z - nodes[index]) / (nodes[index + 1] - nodes[index]);
            let (below, above) = (table.lifetime_slope[index], table.lifetime_slope[index + 1]);
            std::array::from_fn(|k| (1.0 - share) * below[k] + share * above[k])
        };
        Some(Self {
            delta_y,
            slope,
            shift: table.hb_temperature_shift,
        })
    }

    /// exp(s(m, Z) ΔY), the factor on the timescales of a phase whose formulae read `m` M☉.
    #[must_use]
    pub(crate) fn timescale_factor(&self, m: f64) -> f64 {
        let x = math::log10(m);
        let [c0, c1, c2] = self.slope;
        math::exp((c0 + x * (c1 + x * c2)) * self.delta_y)
    }

    /// `point` of the horizontal branch, at current mass `mass`, shifted in log₁₀ `T_eff` by
    /// (a + b `M_env`) ΔY at constant luminosity.
    #[must_use]
    pub(crate) fn horizontal_branch(&self, point: PhasePoint, mass: SolarMasses) -> PhasePoint {
        let envelope = (mass.value() - point.core_mass.value()).max(0.0);
        let shift = (self.shift[0] + self.shift[1] * envelope) * self.delta_y;
        // L ∝ R² T⁴: a shift of δ in log T at fixed L is one of −2δ in log R.
        PhasePoint {
            radius: SolarRadii::new(point.radius.value() * math::exp10(-2.0 * shift)),
            ..point
        }
    }
}
