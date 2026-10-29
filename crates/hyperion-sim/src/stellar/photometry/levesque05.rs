//! The V-band bolometric corrections of red supergiants' model atmospheres from Levesque et al.
//! (2005, ApJ 628, 973, Table 6) (plan 06, P06.T43.a, rendering plan R06's ask A4).
//!
//! **Source.** Table 6, "MARCS Bolometric Corrections", of Levesque, Massey, Olsen, Plez,
//! Josselin, Maeder and Meynet (2005), as printed in arXiv:astro-ph/0504337v1 (read through the
//! Internet Archive, 2026-10-28): BC<sub>V</sub> of the MARCS models of log g = 0.0 from 3,200 to
//! 4,300 K by 100 K, with the zero point of Bessell, Castelli and Plez (1998), BC<sub>V</sub>☉ =
//! −0.07 (their section 3.2, with M<sub>bol</sub>☉ = 4.74). Their Table 5's corrections for the
//! Galactic supergiants' temperature scale, K1–M5 I (4,100 to 3,450 K), are this table's, linearly
//! interpolated; and "there is little change with surface gravity (< 0.05 mag over 0.5 dex in log
//! g for 3500 ≤ T<sub>eff</sub> ≤ 4300)" (section 3.2).

/// T<sub>eff</sub> (K) and BC<sub>V</sub> (mag, on BC<sub>V</sub>☉ = −0.07) of Table 6, coolest
/// first, as printed.
pub(super) const MARCS_BC_V: [(f64, f64); 12] = [
    (3200.0, -4.58),
    (3300.0, -3.66),
    (3400.0, -2.81),
    (3500.0, -2.18),
    (3600.0, -1.75),
    (3700.0, -1.45),
    (3800.0, -1.23),
    (3900.0, -1.06),
    (4000.0, -0.92),
    (4100.0, -0.79),
    (4200.0, -0.70),
    (4300.0, -0.61),
];

/// BC<sub>V</sub> (mag, on the table's zero point) at `teff_k`, linear in T<sub>eff</sub> between
/// the table's rows as Levesque et al. interpolate it for their Table 5, a row's own value at its
/// temperature; `None` outside 3,200–4,300 K.
#[must_use]
pub(super) fn bc_v_at(teff_k: f64) -> Option<f64> {
    let (coolest, hottest) = (MARCS_BC_V[0].0, MARCS_BC_V[MARCS_BC_V.len() - 1].0);
    if !(coolest..=hottest).contains(&teff_k) {
        return None;
    }
    let j = MARCS_BC_V
        .partition_point(|&(t, _)| t < teff_k)
        .clamp(1, MARCS_BC_V.len() - 1);
    let ((t_a, bc_a), (t_b, bc_b)) = (MARCS_BC_V[j - 1], MARCS_BC_V[j]);
    let f = (teff_k - t_a) / (t_b - t_a);
    Some((1.0 - f) * bc_a + f * bc_b)
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::bits;

    use super::*;

    /// The rows rise by 100 K with the correction rising, each row's own value comes back exactly,
    /// and between rows the interpolation is linear in temperature; outside the table there is
    /// none.
    #[test]
    fn the_column_is_read_linearly_between_its_rows() {
        for pair in MARCS_BC_V.windows(2) {
            assert!((pair[1].0 - pair[0].0 - 100.0).abs() < 1e-9, "{pair:?}");
            assert!(pair[1].1 > pair[0].1, "{pair:?}");
        }
        for &(t, bc) in &MARCS_BC_V {
            assert_eq!(bc_v_at(t).map(bits), Some(bits(bc)), "{t}");
        }
        let mid = bc_v_at(3_650.0).unwrap();
        assert!((mid - 0.5 * (-1.75 - 1.45)).abs() < 1e-12, "{mid}");
        assert_eq!(bc_v_at(3_199.0), None);
        assert_eq!(bc_v_at(4_301.0), None);
    }
}
