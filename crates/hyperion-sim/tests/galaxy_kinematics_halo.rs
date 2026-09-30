//! The stellar halo's velocity laws (plan 08, P08.T3).

#[expect(
    dead_code,
    reason = "the kinematics tests use only a few shared helpers"
)]
mod common;

use common::assert_within;
use hyperion_sim::Seed;
use hyperion_sim::galaxy::imf::MassFunctionKind;
use hyperion_sim::galaxy::kinematics::halo::HaloKinematics;
use hyperion_sim::galaxy::params::{GalaxyParams, HaloComponentKind};
use hyperion_sim::galaxy::potential::{MassModel, PotentialTables};
use hyperion_sim::galaxy::quad::gl_panels;
use hyperion_sim::galaxy::{Galaxy, PointLy};
use hyperion_sim::math;
use hyperion_sim::units::LightYears;

/// The Milky Way fixture's halo laws and fields (the halo needs only the in-plane curve).
fn milky_way() -> (Galaxy, HaloKinematics) {
    let galaxy = Galaxy::from_params(
        Seed::new(0x0803_0000_0000_0001),
        GalaxyParams::milky_way_like(),
    )
    .expect("the fixture's gas is mostly neutral");
    let halo = HaloKinematics::new(
        galaxy.seed(),
        galaxy.params(),
        galaxy.mass_model(),
        galaxy.potential(),
    );
    (galaxy, halo)
}

/// The mixture's `σ_r` in Bond et al.'s volume, km/s, against the measured spread (ruling 111.2):
/// Bond et al.'s 141 ± 5 and Smith et al.'s 143 ± 2 at the bottom, Bird et al. 2021's 179 and the
/// Sausage's 175 ± 26 at the top.
const MIXTURE_SIGMA_R: (f64, f64) = (140.0, 180.0);

/// A kiloparsec in light-years.
const KPC: f64 = 3_261.563_777_167_433_6;

/// P08.T3 with rulings 105.5 and 111.2: at Milky Way values the mixture, weighted by each
/// component's density over Bond et al.'s volume, 1 < |Z| < 5 kpc and 3 < R < 13 kpc, has a radial
/// dispersion of 140–180 km/s and an anisotropy of 0.5–0.7.
///
/// The fixture reads 165.4 km/s with the monopole, β 0.672, and ruling 111.2 accepts it, no longer
/// provisional: the window spans the measurements of the inner halo rather than Bond et al.'s alone
/// (Bond et al. 2010, ApJ 716, 1: (141, 75, 85) ± 5 km/s, β = 0.68; Smith et al. 2009, MNRAS 399,
/// 1223: 143 ± 2; Bird et al. 2021, ApJ 919, 66, Fig. 5: 140–179 by [Fe/H]; Belokurov et al.
/// 2020, MNRAS 494, 3880, Table 1: the Sausage's 175 ± 26). It was 135–155 until ruling 111.
#[test]
fn the_mixture_at_milky_way_values() {
    let (galaxy, halo) = milky_way();
    let fields = galaxy.fields();
    let (mut radial, mut tangential, mut mass) = (0.0, 0.0, 0.0);
    let (r_edges, z_edges) = (
        [3.0 * KPC, 5.0 * KPC, 8.0 * KPC, 13.0 * KPC],
        [1.0 * KPC, 2.0 * KPC, 3.5 * KPC, 5.0 * KPC],
    );
    for id in fields.component_ids() {
        let component = fields.component(id);
        let Some(kind) = component.halo_component() else {
            continue;
        };
        let index = halo
            .components()
            .iter()
            .position(|c| c.kind() == kind)
            .unwrap();
        let law = &halo.components()[index];
        // Over the volume, in R dR dz (one side of the plane; the other is the same).
        let moments = |r_cyl: f64, z: f64| {
            let r = math::hypot(r_cyl, z);
            let density = component.density(&PointLy::new(r_cyl, 0.0, z)) * r_cyl;
            let sigma_r = law.sigma_r(LightYears::new(r)).value();
            let s2 = sigma_r * sigma_r;
            [density, density * s2, density * s2 * (1.0 - law.beta_at(r))]
        };
        let over = |k: usize| {
            gl_panels(
                |r_cyl| gl_panels(|z| moments(r_cyl, z)[k], &z_edges),
                &r_edges,
            )
        };
        mass += over(0);
        radial += over(1);
        tangential += over(2);
    }
    let sigma_r = (radial / mass).sqrt();
    let beta = 1.0 - tangential / radial;
    eprintln!("halo mixture in Bond et al.'s volume: σ_r {sigma_r:.1} km/s, β {beta:.3}");
    let (lo, hi) = MIXTURE_SIGMA_R;
    assert_within("mixture σ_r in Bond et al.'s volume", sigma_r, lo, hi);
    assert_within("mixture anisotropy", beta, 0.5, 0.7);
}

/// P08.T3: the dominant merger has no net rotation, and the in-situ component a prograde one.
#[test]
fn the_dominant_merger_is_still_and_the_in_situ_halo_turns() {
    let (_, halo) = milky_way();
    let index = |kind| {
        halo.components()
            .iter()
            .position(|c| c.kind() == kind)
            .unwrap()
    };
    let (dominant, _) = halo.at(index(HaloComponentKind::DominantMerger), 26_000.0, 0.8);
    assert!(dominant[2].abs() < 1e-12, "{dominant:?}");
    // The Splash's 25 km/s (Belokurov et al. 2020, Table 1; ruling 105.5).
    let (in_situ, _) = halo.at(index(HaloComponentKind::InSitu), 26_000.0, 1.0);
    assert_within("in-situ rotation, km/s", in_situ[2], 20.0, 30.0);
    for c in halo.components() {
        assert!((0.3..=0.9).contains(&c.beta()), "{:?}", c.kind());
        assert!(c.rotation().abs() <= 0.25, "{:?}", c.kind());
        // β(r) meets An and Evans's central limit, 0 at the centre.
        assert!(c.beta_at(0.0).abs() < 1e-15, "{:?}", c.kind());
    }
}

/// P08.T3: over 200 seeds no component's radial dispersion exceeds half the local escape speed, at
/// radii from 2,000 to 60,000 ly: with ruling 105.5's β(r), which falls to 0 in the core, the check
/// starts near the centre again.
/// A constant anisotropy of 0.9 broke An and Evans's (2006, ApJ 642, 752) limit in the core and
/// put the dominant merger's `σ_r` at 356 km/s at 2,000 ly for seed 0.
#[test]
#[ignore = "slow: builds the parameters and halo laws of 200 galaxies"]
fn no_halo_dispersion_exceeds_half_the_escape_speed() {
    for n in 0..200_u64 {
        let seed = Seed::new(0x0803_c200_0000_0000 | n);
        let params = GalaxyParams::from_seed(seed, MassFunctionKind::default());
        let model = MassModel::new(&params);
        let tables = PotentialTables::in_plane(&model);
        let halo = HaloKinematics::new(seed, &params, &model, &tables);
        for c in halo.components() {
            for k in 0..30 {
                let r = 2_000.0 * f64::from(k + 1);
                let sigma = c.sigma_r(LightYears::new(r)).value();
                let escape = tables.escape_speed_in_plane(LightYears::new(r)).value();
                assert!(
                    sigma < 0.5 * escape,
                    "seed {n}, {:?} at {r} ly: σ_r {sigma:.1} against v_esc {escape:.1}",
                    c.kind()
                );
            }
        }
    }
}
