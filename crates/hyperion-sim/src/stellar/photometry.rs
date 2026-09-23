//! Absolute photometry from a star's state: the bolometric correction to V, the absolute V
//! magnitude and the B − V colour (plan 06, P06.T23.a).
//!
//! All three read the mean dwarf sequence of Pecaut and Mamajek (2013, ApJS 208, 9) in Mamajek's
//! version 2022.04.16, the table [`classify`](crate::stellar::classify) types dwarfs on,
//! interpolated linearly in log₁₀ T<sub>eff</sub>. The table's bolometric corrections are on the
//! IAU 2015 system, with M<sub>bol</sub>☉ = 4.74 ([`SOLAR_ABSOLUTE_BOLOMETRIC_MAGNITUDE`]), so the
//! Sun (5,772 K) reads BC<sub>V</sub> = −0.085 and M<sub>V</sub> = 4.825. They are the dwarfs'
//! corrections, read at the star's temperature whatever its gravity: a giant's differs little from
//! G to early K, but Straižys and Kuriliene (1981, Ap&SS 80, 353, Table III) put the M giants'
//! 0.1–0.5 mag more negative than the dwarfs' at the same temperature to M3 III, 1.1 mag at M5 III
//! and 1.7 mag at M6 III, so a late M giant's M<sub>V</sub> here is up to that much too bright.
//!
//! - Above the table's hottest row (O3V, 44,900 K), BC<sub>V</sub> falls by 7.5 mag per dex of
//!   temperature from O3V's −4.01, the Rayleigh–Jeans limit, where the flux in V grows as
//!   T<sub>eff</sub> and the bolometric flux as T<sub>eff</sub>⁴, and B − V stays at O3V's −0.330.
//!   A blackbody at 550 nm falls only 6.7–7.3 mag per dex over 45,000–150,000 K; the calibration is
//!   the Montreal 0.6 M☉ DA models (Bédard et al. 2020, ApJ 901, 93), whose BC<sub>V</sub> falls
//!   3.93 mag over that range against this continuation's 4.01 (−4.15 against −4.01 at the anchor,
//!   −8.16 against −7.94 at 150,000 K), and whose B − V moves only from −0.30 to −0.36.
//! - The table gives BC<sub>V</sub> down to L5V (1,710 K) and B − V down to M9V (2,380 K); cooler
//!   objects have none here, as none is measured: a T dwarf is not observed in V.
//! - White dwarfs, neutron stars and black holes have no absolute V magnitude here. A degenerate
//!   atmosphere is not on the dwarf sequence: at 4,000 K the Montreal DA models give BC<sub>V</sub>
//!   = −0.40 where the M dwarfs' is −1.02, and B − V differs by up to 0.25 mag from 8,000 to 40,000
//!   K. Their photometry needs the white-dwarf model grids, which this table is not.
//!
//! Nothing here takes the logarithm of a luminosity that may be zero: a black hole has none
//! (ruling 40), and a zero luminosity has no magnitude.
//!
//! Extinction and apparent magnitudes belong to plan 07.

use crate::math;
use crate::stellar::StarState;
use crate::stellar::classify::pm13::{self, DwarfRow};
use crate::units::{Kelvin, Magnitudes, SolarLuminosities};

/// The Sun's absolute bolometric magnitude, 4.74 mag: IAU 2015 Resolution B2's zero point
/// (M<sub>bol</sub> = 0 at 3.0128 × 10²⁸ W) applied to the nominal solar luminosity of Resolution
/// B3, 3.828 × 10²⁶ W.
pub const SOLAR_ABSOLUTE_BOLOMETRIC_MAGNITUDE: Magnitudes = Magnitudes::new(4.74);

/// How far BC<sub>V</sub> falls per dex of effective temperature beyond the table's hottest row,
/// mag: the Rayleigh–Jeans limit, 2.5 × (4 − 1), where the V flux grows as T and the bolometric as
/// T⁴.
const RAYLEIGH_JEANS_BC_SLOPE: f64 = 7.5;

/// The absolute bolometric magnitude of a luminosity: M<sub>bol</sub> = 4.74 − 2.5 log₁₀(L ÷ L☉).
///
/// `None` for a luminosity that is not finite and positive: a black hole, or nothing, has no
/// magnitude.
///
/// # Examples
///
/// ```
/// use hyperion_sim::stellar::photometry::absolute_bolometric_magnitude;
/// use hyperion_sim::units::SolarLuminosities;
///
/// let vega = absolute_bolometric_magnitude(SolarLuminosities::new(40.0)).ok_or("luminous")?;
/// assert!((vega.value() - 0.735).abs() < 1e-3);
/// assert_eq!(absolute_bolometric_magnitude(SolarLuminosities::ZERO), None);
/// # Ok::<(), &str>(())
/// ```
#[must_use]
pub fn absolute_bolometric_magnitude(luminosity: SolarLuminosities) -> Option<Magnitudes> {
    let l = luminosity.value();
    (l.is_finite() && l > 0.0).then(|| {
        Magnitudes::new(SOLAR_ABSOLUTE_BOLOMETRIC_MAGNITUDE.value() - 2.5 * math::log10(l))
    })
}

/// The V-band bolometric correction BC<sub>V</sub> = M<sub>bol</sub> − M<sub>V</sub> at effective
/// temperature `teff`, from the mean dwarf sequence (see the [module](self) documentation).
///
/// `None` below the table's last correction (L5V, 1,710 K) and for a temperature that is not
/// finite and positive.
///
/// # Examples
///
/// ```
/// use hyperion_sim::stellar::photometry::bolometric_correction_v;
/// use hyperion_sim::units::Kelvin;
///
/// let sun = bolometric_correction_v(Kelvin::new(5_772.0)).ok_or("tabulated")?;
/// assert!((sun.value() + 0.0847).abs() < 1e-4);
/// assert_eq!(bolometric_correction_v(Kelvin::new(1_000.0)), None);
/// # Ok::<(), &str>(())
/// ```
#[must_use]
pub fn bolometric_correction_v(teff: Kelvin) -> Option<Magnitudes> {
    column_with_hot_tail(
        teff,
        |r| r.bc_v,
        |t| -RAYLEIGH_JEANS_BC_SLOPE * (math::log10(t) - math::log10(pm13::hottest().teff_k)),
    )
}

/// The intrinsic B − V colour at effective temperature `teff`, from the mean dwarf sequence (see
/// the [module](self) documentation).
///
/// `None` below the table's last colour (M9V, 2,380 K) and for a temperature that is not finite
/// and positive.
#[must_use]
pub fn colour_b_v(teff: Kelvin) -> Option<Magnitudes> {
    column_with_hot_tail(teff, |r| r.colour_b_v, |_| 0.0)
}

/// The absolute V magnitude of a star: M<sub>V</sub> = M<sub>bol</sub> − BC<sub>V</sub> at its
/// effective temperature.
///
/// `None` for a white dwarf, a neutron star, a black hole or nothing (see the
/// [module](self) documentation), for a star without luminosity, and where the table has no
/// correction (below 1,710 K).
///
/// # Examples
///
/// ```
/// use hyperion_sim::stellar::photometry::absolute_magnitude_v;
/// use hyperion_sim::stellar::{Phase, StarState, StarStateParts};
/// use hyperion_sim::units::{SolarLuminosities, SolarMasses, SolarMassesPerYear, SolarRadii, Years};
///
/// let sun = StarState::new(StarStateParts {
///     phase: Phase::MainSequence,
///     age: Years::new(4.57e9),
///     mass: SolarMasses::new(1.0),
///     core_mass: SolarMasses::ZERO,
///     luminosity: SolarLuminosities::new(1.0),
///     radius: SolarRadii::new(1.0),
///     mass_loss_rate: SolarMassesPerYear::ZERO,
///     phase_fraction: 0.45,
/// });
/// let m_v = absolute_magnitude_v(&sun).ok_or("the Sun has a magnitude")?;
/// assert!((m_v.value() - 4.825).abs() < 1e-3);
/// # Ok::<(), &str>(())
/// ```
#[must_use]
pub fn absolute_magnitude_v(state: &StarState) -> Option<Magnitudes> {
    if !state.phase().is_living() {
        return None;
    }
    let m_bol = absolute_bolometric_magnitude(state.luminosity())?;
    let bc = bolometric_correction_v(state.effective_temperature())?;
    Some(m_bol - bc)
}

/// A column of the dwarf sequence at `teff`, continued above the hottest row by `tail`, the
/// change from that row's value as a function of the temperature.
#[must_use]
fn column_with_hot_tail(
    teff: Kelvin,
    column: impl Fn(&DwarfRow) -> Option<f64>,
    tail: impl Fn(f64) -> f64,
) -> Option<Magnitudes> {
    let t = teff.value();
    if !(t.is_finite() && t > 0.0) {
        return None;
    }
    let value = if t > pm13::hottest().teff_k {
        column(&pm13::hottest())? + tail(t)
    } else {
        pm13::column_at(t, column)?
    };
    Some(Magnitudes::new(value))
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::bits;

    use super::*;
    use crate::stellar::{Phase, StarStateParts};
    use crate::units::{SolarMasses, SolarMassesPerYear, SolarRadii, Years};

    fn state(phase: Phase, luminosity: f64, radius: f64) -> StarState {
        let mass = SolarMasses::new(1.0);
        StarState::new(StarStateParts {
            phase,
            age: Years::new(4.57e9),
            mass,
            core_mass: if phase.is_remnant() {
                mass
            } else {
                SolarMasses::ZERO
            },
            luminosity: SolarLuminosities::new(luminosity),
            radius: SolarRadii::new(radius),
            mass_loss_rate: SolarMassesPerYear::ZERO,
            phase_fraction: 0.5,
        })
    }

    /// The plan's figure: the Sun is G2V with M<sub>V</sub> = 4.81 ± 0.05. The table puts it at
    /// 4.825.
    #[test]
    fn the_suns_absolute_magnitude_is_4_8() {
        let m_v = absolute_magnitude_v(&state(Phase::MainSequence, 1.0, 1.0)).unwrap();
        assert!((m_v.value() - 4.81).abs() < 0.05, "{m_v:?}");
        assert!((m_v.value() - 4.8247).abs() < 1e-3, "{m_v:?}");
        let b_v = colour_b_v(Kelvin::new(5_772.0)).unwrap();
        assert!((b_v.value() - 0.65).abs() < 1e-3, "{b_v:?}");
    }

    /// At every row the table's own numbers come back, bit for bit, since the interpolation's
    /// fraction is then exactly zero.
    #[test]
    fn the_rows_are_reproduced_exactly() {
        for row in pm13::DWARF_SEQUENCE {
            let t = Kelvin::new(row.teff_k);
            let bc = bolometric_correction_v(t).map(|m| bits(m.value()));
            assert_eq!(bc, row.bc_v.map(bits), "{row:?}");
            let colour = colour_b_v(t).map(|m| bits(m.value()));
            assert_eq!(colour, row.colour_b_v.map(bits), "{row:?}");
        }
    }

    /// Continuous across the hottest row, and falling at the Rayleigh–Jeans rate beyond it.
    #[test]
    fn the_hot_tail_continues_the_table() {
        let edge = pm13::hottest().teff_k;
        let below = bolometric_correction_v(Kelvin::new(edge * (1.0 - 1e-12))).unwrap();
        let above = bolometric_correction_v(Kelvin::new(edge * (1.0 + 1e-12))).unwrap();
        assert!((below.value() - above.value()).abs() < 1e-9);
        let ten_times = bolometric_correction_v(Kelvin::new(edge * 10.0)).unwrap();
        assert!((ten_times.value() - (-4.01 - 7.5)).abs() < 1e-9);
        let hot = colour_b_v(Kelvin::new(1e6)).map(|m| bits(m.value()));
        assert_eq!(hot, Some(bits(-0.330)));
    }

    /// Between two rows, a correction and a colour lie between the rows' own values, from the
    /// last row with a value to the hottest.
    #[test]
    fn values_lie_between_their_rows() {
        let rows = &pm13::DWARF_SEQUENCE;
        let mut t = 1_710.0_f64;
        while t < rows[0].teff_k {
            let hot = rows.iter().rposition(|r| r.teff_k >= t).unwrap();
            let (a, b) = (&rows[hot], &rows[(hot + 1).min(rows.len() - 1)]);
            let within = |value: f64, x: f64, y: f64| value >= x.min(y) && value <= x.max(y);
            let bc = bolometric_correction_v(Kelvin::new(t)).unwrap().value();
            assert!(
                within(bc, a.bc_v.unwrap(), b.bc_v.unwrap_or(bc)),
                "{t} K: {bc}"
            );
            if let Some(colour) = colour_b_v(Kelvin::new(t)) {
                let colour = colour.value();
                assert!(
                    within(colour, a.colour_b_v.unwrap(), b.colour_b_v.unwrap()),
                    "{t} K: {colour}"
                );
            }
            t *= 1.001;
        }
    }

    #[test]
    fn nothing_without_a_calibration_has_a_magnitude() {
        assert_eq!(bolometric_correction_v(Kelvin::new(1_700.0)), None);
        assert_eq!(colour_b_v(Kelvin::new(2_370.0)), None);
        assert_eq!(bolometric_correction_v(Kelvin::ZERO), None);
        assert_eq!(bolometric_correction_v(Kelvin::new(f64::NAN)), None);
        assert_eq!(colour_b_v(Kelvin::new(-5.0)), None);
        // A white dwarf of 10,000 K and a black hole, which has no luminosity at all.
        let wd = state(Phase::CarbonOxygenWhiteDwarf, 3e-3, 0.0126);
        assert!(wd.effective_temperature().value() > 9_000.0);
        assert_eq!(absolute_magnitude_v(&wd), None);
        assert_eq!(
            absolute_magnitude_v(&state(Phase::BlackHole, 0.0, 1e-5)),
            None
        );
        assert_eq!(
            absolute_bolometric_magnitude(SolarLuminosities::new(f64::INFINITY)),
            None
        );
    }

    /// A 1,700 K L dwarf is fainter in V than any table row gives, so it has no M<sub>V</sub>; one
    /// at L4 (1,870 K) does.
    #[test]
    fn brown_dwarfs_have_a_magnitude_only_where_the_table_gives_one() {
        let l4 = state(Phase::Substellar, math::exp10(-3.7), 0.1);
        let t = l4.effective_temperature().value();
        assert!(t > 1_710.0, "{t}");
        assert!(absolute_magnitude_v(&l4).is_some());
        let cooler = state(Phase::Substellar, math::exp10(-4.4), 0.09);
        assert!(cooler.effective_temperature().value() < 1_710.0);
        assert_eq!(absolute_magnitude_v(&cooler), None);
    }
}
