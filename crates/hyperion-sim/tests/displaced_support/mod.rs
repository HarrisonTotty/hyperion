//! A test-only form table for the displaced classes, assembled from the brainstorm's figures until
//! plan 15's `tables::displaced_forms` exists (plan 08, Risks: "The table arrives late").
//!
//! It is not a fit. It has the table's shape (56 disc-born classes by speed and age bin, and eight
//! speed bins for each of the five old sources), heights of a few hundred light-years for the
//! slow classes rising with speed and age, a spheroid that takes more of the weight as the speed
//! rises, a slope of 2.5 for the fastest classes, and the brainstorm's own-form shares at a
//! corotation ratio of 1.2 (bar 0.95, 0.61, 0.20, 0.05; bulge 0.91, 0.76, 0.55, 0.25; nuclear disc
//! 0.88, 0.67, 0.43), continued by Design note 13's placeholders. The forms' tests and bounds'
//! tests read it; nothing generated does.

use hyperion_sim::Seed;
use hyperion_sim::galaxy::displaced::forms::{CoredPowerLawParams, FlaredLayerParams};
use hyperion_sim::galaxy::displaced::{AGE_BINS, GalaxyScales, SPEED_BINS};
use hyperion_sim::galaxy::fields::Fields;
use hyperion_sim::galaxy::imf::MassFunctionKind;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::potential::{MassModel, PotentialTables};

/// A representative speed of each speed bin, in `v_c`.
pub const SPEED_MIDS: [f64; SPEED_BINS] = [0.15, 0.37, 0.67, 1.07, 1.52, 1.97, 2.5, 3.5];

/// A representative time since death of each age bin, in `R_d ÷ v_c`.
pub const AGE_MIDS: [f64; AGE_BINS] = [0.05, 0.2, 0.6, 1.5, 3.0, 6.0, 20.0];

/// The old sources whose classes have one cored power law per speed bin.
pub const OLD_SOURCES: usize = 5;

/// One disc-born class's row: the layer, the spheroid and `⟨uτ⟩`.
#[must_use]
pub fn disc_born(speed: usize, age: usize) -> (FlaredLayerParams, CoredPowerLawParams, f64) {
    let u = SPEED_MIDS[speed];
    let tau = AGE_MIDS[age].min(8.0);
    let s = f64::from(u8::try_from(speed).unwrap());
    let spheroid_weight = (0.02 * s * s * (0.5 + 0.1 * tau)).min(0.9);
    let layer = FlaredLayerParams {
        weight: 1.0 - spheroid_weight,
        h_r: 1.0 + 0.3 * u,
        // 0.04 R_d is about 280 ly at the fixture's 2.15 kpc disc.
        h_0: 0.04 + 0.1 * u * tau.sqrt(),
        r_flare: 3.0 + 2.0 * u,
        beta: 1.0 + 0.12 * s,
    };
    let spheroid = CoredPowerLawParams {
        weight: spheroid_weight,
        a: 0.8 + u,
        q: (0.3 + 0.1 * s).min(1.0),
        gamma: (6.0 - 0.5 * s).max(2.5),
    };
    (layer, spheroid, u * AGE_MIDS[age])
}

/// One old source's spheroid in speed bin `speed`: source 0 is the thick disc, 1 the halo, 2 the
/// bulge, 3 the bar and 4 the nuclear disc.
#[must_use]
pub fn old_born(source: usize, speed: usize) -> CoredPowerLawParams {
    let u = SPEED_MIDS[speed];
    let (a0, q0) = [(1.0, 0.4), (0.5, 0.8), (0.2, 0.5), (0.3, 0.3), (0.03, 0.3)][source];
    let s = f64::from(u8::try_from(speed).unwrap());
    CoredPowerLawParams {
        weight: 1.0,
        a: a0 * (1.0 + u),
        q: (q0 + 0.08 * s).min(1.0),
        gamma: (5.0 - 0.35 * s).max(2.5),
    }
}

/// The own-form shares of the bar (source 3), the bulge (2) and the nuclear disc (4) by speed bin
/// at the corotation ratios 1.0, 1.2 and 1.4: the brainstorm's at 1.2, and a tenth more and less
/// of the spheroid's share at the ends (a longer corotation keeps less of the bar's shape).
#[must_use]
pub fn own_shares(source: usize) -> [[f64; 3]; SPEED_BINS] {
    let at_1_2: [f64; SPEED_BINS] = match source {
        2 => [0.91, 0.76, 0.55, 0.25, 0.10, 0.0, 0.0, 0.0],
        3 => [0.95, 0.61, 0.20, 0.05, 0.02, 0.0, 0.0, 0.0],
        4 => [0.88, 0.67, 0.43, 0.20, 0.05, 0.0, 0.0, 0.0],
        _ => [0.0; SPEED_BINS],
    };
    at_1_2.map(|s| [s + 0.1 * (1.0 - s) * s, s, s - 0.1 * (1.0 - s) * s])
}

/// The scales and the fields of the galaxy of `params`.
#[must_use]
pub fn scales_and_fields(params: &GalaxyParams) -> (GalaxyScales, Fields) {
    let model = MassModel::new(params);
    let scales = GalaxyScales::new(params, &PotentialTables::in_plane(&model));
    (scales, Fields::new(params, &model))
}

/// The parameters of seed `i` of the sweeps.
#[must_use]
pub fn seed_params(i: u64) -> GalaxyParams {
    GalaxyParams::from_seed(
        Seed::new(0x0810_0000_0000_0000 + i),
        MassFunctionKind::default(),
    )
}
