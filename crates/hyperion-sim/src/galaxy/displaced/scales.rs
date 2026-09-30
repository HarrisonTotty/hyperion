//! The galaxy's scales, in which plan 15's displaced forms are dimensionless (plan 08, P08.T1 and
//! Design note 12).

use crate::galaxy::consts::LIGHT_YEARS_PER_YEAR_PER_KM_S;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::potential::PotentialTables;
use crate::units::{KilometresPerSecond, LightYears, Years};

/// Where the circular speed `v_c` is read, in thin-disc scale lengths (plan 08, Design note 12).
pub const CIRCULAR_SPEED_LENGTHS: f64 = 3.0;

/// Where the nuclear disc's circular speed is read, in its own scale lengths (plan 08, Design note
/// 12).
pub const NUCLEAR_SPEED_LENGTHS: f64 = 1.5;

/// The scales of one galaxy that the displaced classes are measured in (plan 08, Design note 12):
/// they match plan 15's P15.T6.
///
/// - `R_d`, the thin disc's scale length;
/// - `v_c`, the circular speed at 3 `R_d` in the plane;
/// - the time unit `R_d ÷ v_c` (for the brainstorm's 2.6 kpc disc in a 230 km/s curve, 11 Myr);
/// - the escape ratio `v_esc ÷ v_c` at 3 `R_d` in the plane;
/// - the nuclear disc's circular speed at 1.5 of its scale lengths, against which its own classes'
///   speeds are taken;
/// - the bar's corotation ratio, at which the own-form shares are interpolated.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::displaced::GalaxyScales;
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::galaxy::potential::{MassModel, PotentialTables};
///
/// let params = GalaxyParams::milky_way_like();
/// let scales = GalaxyScales::new(&params, &PotentialTables::in_plane(&MassModel::new(&params)));
/// // A remnant kicked at the circular speed crosses a scale length in one time unit.
/// let crossing = scales.r_d().value() / scales.v_c().value();
/// assert!(crossing > 0.0 && scales.escape_ratio() > 2.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GalaxyScales {
    r_d: LightYears,
    v_c: KilometresPerSecond,
    tau_unit: Years,
    escape_ratio: f64,
    nuclear_v_c: KilometresPerSecond,
    corotation_ratio: f64,
}

impl GalaxyScales {
    /// The scales of the galaxy of `params`, whose potential's in-plane tables are `potential`.
    #[must_use]
    pub fn new(params: &GalaxyParams, potential: &PotentialTables) -> Self {
        let r_d = params.thin_disc().length();
        let at = LightYears::new(CIRCULAR_SPEED_LENGTHS * r_d.value());
        let v_c = potential.v_circ(at);
        let nuclear =
            LightYears::new(NUCLEAR_SPEED_LENGTHS * params.nuclear_disc().length().value());
        Self {
            r_d,
            v_c,
            tau_unit: Years::new(r_d.value() / (v_c.value() * LIGHT_YEARS_PER_YEAR_PER_KM_S)),
            escape_ratio: potential.escape_speed_in_plane(at).value() / v_c.value(),
            nuclear_v_c: potential.v_circ(nuclear),
            corotation_ratio: params.bar().corotation_ratio(),
        }
    }

    /// `R_d`, the thin disc's scale length.
    #[must_use]
    pub fn r_d(&self) -> LightYears {
        self.r_d
    }

    /// `v_c`, the circular speed at 3 `R_d`.
    #[must_use]
    pub fn v_c(&self) -> KilometresPerSecond {
        self.v_c
    }

    /// The time unit `R_d ÷ v_c`.
    #[must_use]
    pub fn tau_unit(&self) -> Years {
        self.tau_unit
    }

    /// `v_esc ÷ v_c` at 3 `R_d` in the plane.
    #[must_use]
    pub fn escape_ratio(&self) -> f64 {
        self.escape_ratio
    }

    /// The nuclear disc's circular speed, at 1.5 of its scale lengths.
    #[must_use]
    pub fn nuclear_v_c(&self) -> KilometresPerSecond {
        self.nuclear_v_c
    }

    /// The bar's corotation radius over its half-length.
    #[must_use]
    pub fn corotation_ratio(&self) -> f64 {
        self.corotation_ratio
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::potential::MassModel;

    /// P08.T1 with ruling 105.4: at Milky Way values the escape ratio lies within 2.3–2.6, and the
    /// time unit `R_d ÷ v_c` within 9.0–10.0 Myr: the fixture's 2.15 kpc disc (ruling 32) in a 224
    /// km/s curve gives 9.4, where the brainstorm's 2.6 kpc disc gave 11.
    #[test]
    fn galaxy_scales_at_milky_way_values() {
        let params = GalaxyParams::milky_way_like();
        let scales = GalaxyScales::new(
            &params,
            &PotentialTables::in_plane(&MassModel::new(&params)),
        );
        let myr = scales.tau_unit().value() / 1e6;
        assert!((9.0..10.0).contains(&myr), "{myr} Myr");
        let by_hand = scales.r_d().value() * 9.460_730_472_580_8e12
            / (scales.v_c().value() * 3.155_76e7)
            / 1e6;
        assert!(
            (myr / by_hand - 1.0).abs() < 1e-9,
            "{myr} against {by_hand}"
        );
        assert!(
            (2.3..2.6).contains(&scales.escape_ratio()),
            "{}",
            scales.escape_ratio()
        );
        assert!((110.0..150.0).contains(&scales.nuclear_v_c().value()));
        assert!((scales.corotation_ratio() - params.bar().corotation_ratio()).abs() < 1e-15);
    }
}
