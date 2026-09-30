//! Each class's kinematics and in-cube shares (plan 15, P15.T6.e), read straight from its record.
//!
//! The mean rotation, the three dispersions and the outbound share are the moments of the class's
//! bound members inside the cube ([`ClassHistogram::moments`]), in its speed scale; `in_cube` is
//! the share of the class's orbits bound and inside, `unbound_in_cube` the share unbound but
//! inside. The run is at the fixture's one halo mass, so the value measured at its escape ratio
//! fills all three of `ESCAPE_RATIO_NODES` until the runs at other halo masses exist (the table's
//! notes say so).

use hyperion_sim::galaxy::displaced::forms::ClassKinematics;

use super::histogram::ClassHistogram;

/// A class's kinematics; all zero for a class with no bound member inside the cube.
#[must_use]
pub fn kinematics(record: &ClassHistogram) -> ClassKinematics {
    if record.in_cube_bound == 0 {
        return ClassKinematics {
            mean_phi: 0.0,
            sigma_r: 0.0,
            sigma_phi: 0.0,
            sigma_z: 0.0,
            outbound: 0.0,
        };
    }
    #[expect(clippy::cast_precision_loss, reason = "counts far below 2⁵³")]
    let n = record.in_cube_bound as f64;
    let [vr, vphi, vr2, vphi2, vz2] = record.moments.map(|m| m / n);
    #[expect(clippy::cast_precision_loss, reason = "counts far below 2⁵³")]
    let outbound = record.outbound as f64 / n;
    ClassKinematics {
        mean_phi: vphi,
        sigma_r: (vr2 - vr * vr).max(0.0).sqrt(),
        sigma_phi: (vphi2 - vphi * vphi).max(0.0).sqrt(),
        sigma_z: vz2.max(0.0).sqrt(),
        outbound,
    }
}

/// The class's bound and unbound shares inside the cube, `(in_cube, unbound_in_cube)`.
#[must_use]
pub fn in_cube(record: &ClassHistogram) -> (f64, f64) {
    if record.orbits == 0 {
        return (0.0, 0.0);
    }
    #[expect(clippy::cast_precision_loss, reason = "counts far below 2⁵³")]
    let n = record.orbits as f64;
    #[expect(clippy::cast_precision_loss, reason = "counts far below 2⁵³")]
    let shares = (
        record.in_cube_bound as f64 / n,
        record.in_cube_unbound as f64 / n,
    );
    shares
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tasks::displaced_forms::histogram::Integration;

    #[test]
    fn moments_become_a_mean_and_dispersions() {
        let mut r = ClassHistogram::new(false);
        r.record(
            [100.0, 0.0, 10.0],
            [0.1, 0.9, 0.2],
            true,
            Integration::default(),
        );
        r.record(
            [100.0, 0.0, -10.0],
            [-0.1, 1.1, 0.2],
            true,
            Integration::default(),
        );
        r.record([1e6, 0.0, 0.0], [0.0; 3], false, Integration::default());
        let k = kinematics(&r);
        assert!((k.mean_phi - 1.0).abs() < 1e-12, "{k:?}");
        assert!((k.sigma_r - 0.1).abs() < 1e-12, "{k:?}");
        assert!((k.sigma_phi - 0.1).abs() < 1e-12, "{k:?}");
        assert!((k.sigma_z - 0.2).abs() < 1e-12, "{k:?}");
        assert!((k.outbound - 0.5).abs() < 1e-15, "{k:?}");
        let (bound, unbound) = in_cube(&r);
        assert!((bound - 2.0 / 3.0).abs() < 1e-15 && unbound.abs() < 1e-15);
    }
}
