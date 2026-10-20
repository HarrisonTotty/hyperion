//! The energy-balance model's meridional transport, Design note 8's transport law (R09.T0.b,
//! built by R09.T13.a).
//!
//! The model diffuses near-surface moist static energy h in latitude with the coefficient Λ, kg
//! m⁻² s⁻¹, of Λ (1 ÷ cos φ) ∂/∂φ (cos φ ∂h/∂φ): Siler, Roe and Armour's (2018) p D ÷ (g R²) in
//! latitude form. Λ is Earth's, scaled by the rotation and the surface pressure as Kaspi and
//! Showman's (2015) idealised moist GCM gives them, and by the air's molar mass as Williams and
//! Kasting's law scales the diffusivity (Williams and Kasting 1997, Icarus 129, 254, not read; as
//! Vladilo et al. 2013, ApJ 767, 65, eq. A4, read: D ∝ p `c_p` m⁻² Ω⁻²), under a cap, and halved
//! on a dry world:
//!
//! Λ = min(Λ⊕ `f_Ω`(Ω ÷ Ω⊕) `f_p`(p ÷ p⊕) (m⊕ ÷ m)², 30 Λ⊕ `c_p⊕` ÷ `c_p`), × ½ on a dry world.
//!
//! The law's heat-capacity factor enters through h's own `c_p`, and its rotation and pressure
//! factors are Kaspi and Showman's tables in place of Ω⁻² and p. Λ takes
//! no term in the radius or the gravity, though Kaspi and Showman's sweeps show both (their
//! Figs. 18a and 20b; R09's Risks, "Deferred findings (2026-10-13 rule) (R09.T13.a)").
//!
//! `f_Ω` and `f_p` are tables at Kaspi and Showman's runs, each fitted with this model itself
//! ([`ROTATION_TABLE`], [`PRESSURE_TABLE`]): linear in log–log between the runs and held at the
//! end values beyond them (Design note 8), but for `f_p` below its first run, 0.2 bar, where it
//! falls linearly to zero with the pressure, as plan 14's column factor ramps below 0.2 m⊕
//! ([`pressure_factor`]; R09.T13.i, decision-r09-t13i §2.4): a thin air carries no transport at no
//! air, where a held end would keep 0.308 of Earth's.

use crate::math;
use crate::planetary::hooks::surface::{
    DRY_TRANSPORT_SHARE, EARTH_AIR_MOLAR_MASS_G_PER_MOL, EARTH_AIR_SPECIFIC_HEAT_J_PER_KG_K,
    EARTH_SIDEREAL_DAY, STANDARD_GRAVITY,
};
use crate::units::consts::EARTH_RADIUS_M;

/// Earth's diffusivity of near-surface moist static energy D⊕, m² s⁻¹: 1.16 × 10⁶, at which
/// Siler, Roe and Armour's moist energy-balance model, forced by ERA-Interim's net heating over
/// 1981–2010, best reproduces the observed zonal-mean 2 m air temperature at sea level (2018, J.
/// Climate 31, 7481, §2 and Fig. 2b, read in the authors' copy); "within 10% of the value used in
/// previous studies".
pub(super) const EARTH_MSE_DIFFUSIVITY_M2_S: f64 = 1.16e6;

/// The surface pressure p⊕ that the transport's pressure factor is relative to, Pa: 1 bar, which
/// is both Siler, Roe and Armour's `p_s` ("surface air pressure (1000 hPa)", their eq. 2) and the
/// reference run of Kaspi and Showman's pressure sweep (2015, Fig. 15c).
pub(super) const REFERENCE_PRESSURE_PA: f64 = 1e5;

/// Earth's transport coefficient in latitude form Λ⊕ = p⊕ D⊕ ÷ (g⊕ R⊕²), kg m⁻² s⁻¹:
/// 2.914 × 10⁻⁴, Siler, Roe and Armour's eq. 3 with [`REFERENCE_PRESSURE_PA`],
/// [`EARTH_MSE_DIFFUSIVITY_M2_S`], the standard gravity (theirs is unstated) and Earth's mean
/// radius, 6,371 km.
///
/// Λ⊕ `c_p⊕` is 0.293 W m⁻² K⁻¹ for dry air's [`EARTH_AIR_SPECIFIC_HEAT_J_PER_KG_K`], and about
/// 0.7 for the moist static energy at 288 K and 80% relative humidity, bracketing FILLET's 0.5 and
/// North, Cahalan and Coakley's 0.649 (Design note 8).
pub(super) const EARTH_TRANSPORT_KG_M2_S: f64 = REFERENCE_PRESSURE_PA * EARTH_MSE_DIFFUSIVITY_M2_S
    / (STANDARD_GRAVITY.value() * EARTH_RADIUS_M * EARTH_RADIUS_M);

/// The cap on the transport, as a multiple of Earth's dry-equivalent Λ⊕ `c_p⊕`: 30 (Design note 8;
/// low confidence). It is a guard that only the molar-mass and heat-capacity factors can reach, as
/// in an H₂-rich air, since the tables' product is at most 10.9 (2.12 at Ω⊕ ÷ 24 by 5.13 at 50
/// bar).
pub(super) const TRANSPORT_CAP: f64 = 30.0;

/// One run of Kaspi and Showman's sweeps and the factor this model takes there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct TransportNode {
    /// The run's rotation rate over Earth's, or its surface pressure over 1 bar.
    pub(super) ratio: f64,
    /// The run's equator–pole surface temperature contrast in their GCM, K, read from the
    /// figure.
    pub(super) gcm_contrast_k: f64,
    /// The factor on Λ⊕ at which this model's contrast, scaled by the sweep's reference run over
    /// its own at a factor of 1, is the GCM's.
    pub(super) factor: f64,
}

/// The node of ratio `ratio`, GCM contrast `gcm_contrast_k` and factor `factor`.
#[must_use]
const fn node(ratio: f64, gcm_contrast_k: f64, factor: f64) -> TransportNode {
    TransportNode {
        ratio,
        gcm_contrast_k,
        factor,
    }
}

/// The rotation sweep's reference run, 1 Ω⊕: its contrast, K, 40.2 (Kaspi and Showman 2015, Fig.
/// 8a, read as [`ROTATION_TABLE`] is; the plan's "40 K").
pub(super) const ROTATION_REFERENCE_CONTRAST_K: f64 = 40.2;

/// The pressure sweep's reference run, 1 bar: its contrast, K, 42.1 (Kaspi and Showman 2015, Fig.
/// 15c, read as [`PRESSURE_TABLE`] is; the plan's "42 K"). It differs from the rotation sweep's 1
/// Ω⊕ run because the pressure sweep runs with no convection scheme (their §3.3).
pub(super) const PRESSURE_REFERENCE_CONTRAST_K: f64 = 42.1;

/// `f_Ω`: the transport's factor at each of Kaspi and Showman's 18 rotation runs, Ω⊕ ÷ 24 to
/// 12 Ω⊕ (2015, ApJ 804, 60, arXiv:1407.6349v2, Fig. 8a; `Fitted`).
///
/// - _The GCM's contrasts_ were read from the figure rendered at 600 dpi from the arXiv PDF, each
///   marker's centre to ±0.2 K (14.4 px per kelvin, 615 px per decade). The runs' rates are read
///   as the fractions given, each within 0.5% of its marker's position; Fig. 5 names Ω⊕ ÷ 16 to
///   8 Ω⊕ among them. Design note 8's nine, read to ±1 K, agree within 0.6 K.
/// - _Their configuration_ (§2): an aquaplanet at perpetual equinox, with no ice and a fixed
///   albedo of 0.35, under the insolation of their eq. 10, S = (1,360 W m⁻² ÷ 4)
///   (1 + (1.2 ÷ 4)(1 − 3 sin² φ)) e^(−`τ_s` σ²), `τ_s` = 0.08 and σ their vertical coordinate,
///   whose top of the atmosphere is σ = 0, with a grey two-stream radiation of fixed optical
///   depth and no vapour feedback, at 1 bar.
/// - _The fit_ runs this model in that configuration, seasons off, as far as it can take it: the
///   insolation at the top of the atmosphere (their eq. 10 without its e^(−`τ_s` σ²), which, a
///   uniform factor, cancels to first order in the ratio of contrasts the fit matches), the same
///   albedo, every band sea, no internal heat, Earth's dry air
///   ([`EARTH_AIR_SPECIFIC_HEAT_J_PER_KG_K`]) with water at 80% relative humidity on its phase
///   diagram, its latent heat the curve's Clausius–Clapeyron enthalpy at 288 K (P14.T49.b's fitted
///   curves; refitted at P14.T54.a's switch from the held Clausius–Clapeyron curve, which moved
///   these factors by up to 6%), and one grey τ
///   for every latitude (theirs varies from 8.4 at the equator to 2.2 at the poles, their eq. 8,
///   which this model cannot hold), solved so that the reference run's global mean is 288 K
///   (their Fig. 14b's 1 bar profile averages about 287 K over area, read roughly) and then held,
///   as they hold the optical depth across their sweeps (§3.3, §3.5). The contrast is the cubic's
///   at the equator less its mean at the poles ([`ModelGrid::stencil`]). Each factor is found by
///   48 bisections of ln f over [10⁻³, 10³] so that this model's contrast times
///   [`ROTATION_REFERENCE_CONTRAST_K`] over its own at f = 1 is the GCM's; the test
///   `the_transport_tables_are_their_fit` repeats the fit and holds these constants to it, and
///   `the_contrast_follows_kaspi_and_showman_s_sweeps` holds the contrasts to 2 K.
///
/// [`ModelGrid::stencil`]: super::grid::ModelGrid::stencil
pub(super) const ROTATION_TABLE: [TransportNode; 18] = [
    node(1.0 / 24.0, 23.5, 2.115_60),
    node(1.0 / 20.0, 25.0, 1.957_25),
    node(1.0 / 16.0, 28.2, 1.672_77),
    node(1.0 / 12.0, 28.2, 1.672_77),
    node(1.0 / 8.0, 28.4, 1.656_74),
    node(1.0 / 6.0, 31.6, 1.429_10),
    node(1.0 / 4.0, 36.6, 1.155_05),
    node(1.0 / 3.0, 35.9, 1.188_77),
    node(1.0 / 2.0, 35.5, 1.208_64),
    node(2.0 / 3.0, 36.8, 1.145_65),
    node(1.0, ROTATION_REFERENCE_CONTRAST_K, 1.0),
    node(4.0 / 3.0, 43.4, 0.883_33),
    node(2.0, 48.3, 0.733_42),
    node(3.0, 53.9, 0.593_45),
    node(4.0, 58.6, 0.494_89),
    node(6.0, 65.6, 0.371_68),
    node(8.0, 70.1, 0.303_99),
    node(12.0, 77.0, 0.213_87),
];

/// `f_p`: the transport's factor at each of Kaspi and Showman's 11 surface-pressure runs, 0.2 to
/// 50 bar (2015, ApJ 804, 60, arXiv:1407.6349v2, Fig. 15c; `Fitted`).
///
/// The GCM's contrasts were read as [`ROTATION_TABLE`]'s were, to ±0.2 K (30.3 px per kelvin,
/// 544 px per decade); Design note 8's six agree within 0.1 K. The fit is
/// [`ROTATION_TABLE`]'s at Earth's rotation, each run's model at its own surface pressure, which
/// sets the vapour's specific humidity, ε e ÷ (p − (1 − ε) e), so that the factor carries both
/// the column's mass and what the GCM's eddies make of it, with the optical depth held as theirs
/// is ("we … do not vary optical thickness despite increasing atmospheric mass", §3.3), scaled by
/// [`PRESSURE_REFERENCE_CONTRAST_K`].
pub(super) const PRESSURE_TABLE: [TransportNode; 11] = [
    node(0.2, 47.0, 0.301_79),
    node(0.3, 45.9, 0.425_90),
    node(0.5, 44.3, 0.633_55),
    node(1.0, PRESSURE_REFERENCE_CONTRAST_K, 1.0),
    node(2.0, 39.7, 1.443_07),
    node(3.0, 37.3, 1.790_21),
    node(5.0, 34.9, 2.212_73),
    node(10.0, 30.4, 2.992_27),
    node(20.0, 26.3, 3.876_40),
    node(30.0, 24.0, 4.476_00),
    node(50.0, 21.9, 5.133_77),
];

/// The factor `table` gives at ratio `ratio`: linear in log–log between its nodes, held at the
/// first node's below it and at the last's above it (a NaN takes the first).
#[must_use]
pub(super) fn table_factor(table: &[TransportNode], ratio: f64) -> f64 {
    let (first, last) = (table[0], table[table.len() - 1]);
    if ratio.is_nan() || ratio <= first.ratio {
        return first.factor;
    }
    if ratio >= last.ratio {
        return last.factor;
    }
    let upper = table
        .iter()
        .position(|n| n.ratio > ratio)
        .expect("a ratio below the last node's has a node above it");
    let (a, b) = (table[upper - 1], table[upper]);
    let t = (math::ln(ratio) - math::ln(a.ratio)) / (math::ln(b.ratio) - math::ln(a.ratio));
    math::exp(math::ln(a.factor) + t * (math::ln(b.factor) - math::ln(a.factor)))
}

/// `f_p` at the surface pressure `pressure_ratio` bar ([`PRESSURE_TABLE`]): the table's
/// ([`table_factor`]) at and above its first run, 0.2 bar, and below it linear to zero,
/// `f_p`(0.2 bar) × p ÷ 0.2 bar, as plan 14's column factor ramps below 0.2 m⊕
/// (`COLUMN_TRANSPORT_RANGE`; decision-r09-t13i §2.4, T13.a's deferred finding). At and above
/// 0.2 bar it is the table's to the bit; a ratio that is not a number takes the table's first.
#[must_use]
pub(super) fn pressure_factor(pressure_ratio: f64) -> f64 {
    let first = PRESSURE_TABLE[0];
    if pressure_ratio < first.ratio {
        if pressure_ratio > 0.0 {
            first.factor * (pressure_ratio / first.ratio)
        } else {
            0.0
        }
    } else {
        table_factor(&PRESSURE_TABLE, pressure_ratio)
    }
}

/// What the model diffuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Diffusing {
    /// Near-surface moist static energy, `c_p` T + L q: a world whose climate condensable is among
    /// its surface liquids.
    MoistStaticEnergy,
    /// Sensible heat alone, `c_p` T, under half the transport ([`DRY_TRANSPORT_SHARE`]): a dry
    /// world.
    SensibleHeat,
}

/// The transport coefficient Λ, kg m⁻² s⁻¹, of a world rotating at `rotation_ratio` times Earth's
/// sidereal rate under `pressure_ratio` bar, whose air's mean molar mass is `molar_mass_g_per_mol`
/// and specific heat `specific_heat_j_per_kg_k`, diffusing `diffusing` (the
/// [module documentation](self)).
#[must_use]
pub(super) fn transport_kg_m2_s(
    rotation_ratio: f64,
    pressure_ratio: f64,
    molar_mass_g_per_mol: f64,
    specific_heat_j_per_kg_k: f64,
    diffusing: Diffusing,
) -> f64 {
    let mass = EARTH_AIR_MOLAR_MASS_G_PER_MOL / molar_mass_g_per_mol;
    let scaled = EARTH_TRANSPORT_KG_M2_S
        * table_factor(&ROTATION_TABLE, rotation_ratio)
        * pressure_factor(pressure_ratio)
        * mass
        * mass;
    let cap = TRANSPORT_CAP * EARTH_TRANSPORT_KG_M2_S * EARTH_AIR_SPECIFIC_HEAT_J_PER_KG_K
        / specific_heat_j_per_kg_k;
    let held = if scaled < cap { scaled } else { cap };
    match diffusing {
        Diffusing::MoistStaticEnergy => held,
        Diffusing::SensibleHeat => DRY_TRANSPORT_SHARE * held,
    }
}

/// A rotation period's rate over Earth's sidereal rate, Ω ÷ Ω⊕ = [`EARTH_SIDEREAL_DAY`] ÷ the
/// sidereal period `rotation_period_s`, seconds.
#[must_use]
pub(super) fn rotation_ratio(rotation_period_s: f64) -> f64 {
    EARTH_SIDEREAL_DAY.value() / rotation_period_s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn earth_s_coefficient_is_siler_s_diffusivity_in_latitude_form() {
        assert!(
            (EARTH_TRANSPORT_KG_M2_S - 2.914e-4).abs() < 1e-7,
            "{EARTH_TRANSPORT_KG_M2_S}"
        );
        let dry = EARTH_TRANSPORT_KG_M2_S * EARTH_AIR_SPECIFIC_HEAT_J_PER_KG_K;
        assert!((dry - 0.293).abs() < 1e-3, "{dry}");
    }

    /// The tables are in increasing order with positive factors, interpolate between their nodes
    /// in log–log and hold their ends.
    #[test]
    fn the_tables_interpolate_in_log_log_and_hold_their_ends() {
        for table in [&ROTATION_TABLE[..], &PRESSURE_TABLE[..]] {
            assert!(table.windows(2).all(|w| w[0].ratio < w[1].ratio));
            assert!(
                table
                    .iter()
                    .all(|n| n.factor > 0.0 && n.gcm_contrast_k > 0.0)
            );
            for n in table {
                assert!((table_factor(table, n.ratio) / n.factor - 1.0).abs() < 1e-12);
            }
            let (first, last) = (table[0], table[table.len() - 1]);
            assert!((table_factor(table, first.ratio / 10.0) - first.factor).abs() < 1e-15);
            assert!((table_factor(table, 0.0) - first.factor).abs() < 1e-15);
            assert!((table_factor(table, f64::NAN) - first.factor).abs() < 1e-15);
            assert!((table_factor(table, last.ratio * 10.0) - last.factor).abs() < 1e-15);
            let (a, b) = (table[2], table[3]);
            let mid = math::exp(f64::midpoint(math::ln(a.ratio), math::ln(b.ratio)));
            let expected = math::exp(f64::midpoint(math::ln(a.factor), math::ln(b.factor)));
            assert!((table_factor(table, mid) / expected - 1.0).abs() < 1e-12);
        }
        assert!((table_factor(&ROTATION_TABLE, 1.0) - 1.0).abs() < 1e-15);
        assert!((table_factor(&PRESSURE_TABLE, 1.0) - 1.0).abs() < 1e-15);
    }

    /// R09.T13.i's ramp (decision-r09-t13i §2.5, test 7): `f_p`(0.1 bar) is half `f_p`(0.2 bar),
    /// `f_p`(0) is 0, it is continuous at 0.2 bar and the table's to the bit at and above it.
    #[test]
    fn the_pressure_factor_falls_linearly_to_zero_below_0_2_bar() {
        let at = pressure_factor(0.2);
        hyperion_testkit::float::assert_same_bits(at, PRESSURE_TABLE[0].factor);
        assert!((pressure_factor(0.1) - 0.5 * at).abs() <= 1e-15 * at);
        hyperion_testkit::float::assert_same_bits(pressure_factor(0.0), 0.0);
        assert!((pressure_factor(0.2 * (1.0 - 1e-12)) - at).abs() < 1e-11 * at);
        assert!((pressure_factor(0.2 * (1.0 + 1e-12)) - at).abs() < 1e-11 * at);
        for ratio in [0.2, 0.3, 1.0, 7.0, 50.0, 100.0] {
            hyperion_testkit::float::assert_same_bits(
                pressure_factor(ratio),
                table_factor(&PRESSURE_TABLE, ratio),
            );
        }
        // A thin air's transport falls with it.
        let (mu, cp) = (
            EARTH_AIR_MOLAR_MASS_G_PER_MOL,
            EARTH_AIR_SPECIFIC_HEAT_J_PER_KG_K,
        );
        let thin = transport_kg_m2_s(1.0, 0.01, mu, cp, Diffusing::SensibleHeat);
        let thick = transport_kg_m2_s(1.0, 0.2, mu, cp, Diffusing::SensibleHeat);
        assert!((thin / thick - 0.05).abs() < 1e-12, "{}", thin / thick);
    }

    /// Earth's air at Earth's rotation and 1 bar takes Λ⊕; a dry world half of it; a light air is
    /// capped at 30 times Earth's dry-equivalent coefficient.
    #[test]
    fn the_transport_scales_by_rotation_pressure_and_molar_mass_under_its_cap() {
        let cp = EARTH_AIR_SPECIFIC_HEAT_J_PER_KG_K;
        let mu = EARTH_AIR_MOLAR_MASS_G_PER_MOL;
        let earth = transport_kg_m2_s(1.0, 1.0, mu, cp, Diffusing::MoistStaticEnergy);
        assert!((earth / EARTH_TRANSPORT_KG_M2_S - 1.0).abs() < 1e-12);
        let dry = transport_kg_m2_s(1.0, 1.0, mu, cp, Diffusing::SensibleHeat);
        assert!((dry / earth - 0.5).abs() < 1e-12);
        let half_mass = transport_kg_m2_s(1.0, 1.0, mu / 2.0, cp, Diffusing::MoistStaticEnergy);
        assert!((half_mass / earth - 4.0).abs() < 1e-12);
        // Hydrogen's air: μ 2.3 g mol⁻¹ and c_p near 14,300 J kg⁻¹ K⁻¹, capped.
        let hydrogen = transport_kg_m2_s(1.0, 1.0, 2.3, 14_300.0, Diffusing::MoistStaticEnergy);
        assert!((hydrogen * 14_300.0 / (earth * cp) - TRANSPORT_CAP).abs() < 1e-9);
        assert!((rotation_ratio(EARTH_SIDEREAL_DAY.value()) - 1.0).abs() < 1e-15);
    }
}
