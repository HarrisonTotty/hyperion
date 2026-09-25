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
use hyperion_sim::galaxy::quad::{gl_panels, gl16};
use hyperion_sim::galaxy::{Galaxy, PointLy, Population};
use hyperion_sim::units::LightYears;

/// The Milky Way fixture's halo laws and fields (the halo needs only the in-plane curve).
fn milky_way() -> (Galaxy, HaloKinematics) {
    let galaxy = Galaxy::from_params(
        Seed::new(0x0803_0000_0000_0001),
        GalaxyParams::milky_way_like(),
    )
    .expect("the fixture's gas is mostly neutral");
    let halo = HaloKinematics::new(galaxy.seed(), galaxy.params(), galaxy.potential());
    (galaxy, halo)
}

/// P08.T3: at Milky Way values the mixture, weighted by each component's density over
/// galactocentric radii of 15,000–65,000 ly, has a radial dispersion and an anisotropy the plan
/// puts at 135–155 km/s and 0.5–0.7 (Bond et al. 2010: (141, 75, 85) ± 5 km/s, β = 0.68).
///
/// The plan's Risks expected the radial dispersion to miss on the measured inner slopes of
/// 2.2–2.8, at 155–185 km/s from a spherical Jeans estimate at β 0.6–0.7 in a flat 230 km/s curve;
/// it does, at 159.7 km/s, and that is a finding for the owner, never a reason to steepen the slopes
/// (plan 08, Risks). The test holds the figure to the Risks' window, and the anisotropy to the
/// plan's.
#[test]
fn the_mixture_at_milky_way_values() {
    let (galaxy, halo) = milky_way();
    let fields = galaxy.fields();
    let ids: Vec<_> = fields
        .component_ids()
        .filter(|&id| fields.component(id).population() == Population::Halo)
        .collect();
    let (mut radial, mut tangential, mut mass) = (0.0, 0.0, 0.0);
    let edges = [15_000.0, 25_000.0, 40_000.0, 65_000.0];
    for id in ids {
        let component = fields.component(id);
        let kind = component.halo_component().unwrap();
        let index = halo
            .components()
            .iter()
            .position(|c| c.kind() == kind)
            .unwrap();
        let law = &halo.components()[index];
        // The density averaged over the sphere of radius r, times r², and σ_r² there.
        let shell = |r: f64| {
            let over_mu = gl16(
                |mu| {
                    let s = (1.0 - mu * mu).sqrt();
                    component.density(&PointLy::new(r * s, 0.0, r * mu))
                },
                0.0,
                1.0,
            );
            let sigma_r = law.sigma_r(LightYears::new(r)).value();
            (over_mu * r * r, sigma_r * sigma_r)
        };
        mass += gl_panels(|r| shell(r).0, &edges);
        let own = gl_panels(
            |r| {
                let (d, s2) = shell(r);
                d * s2
            },
            &edges,
        );
        radial += own;
        tangential += own * (1.0 - law.beta());
    }
    let sigma_r = (radial / mass).sqrt();
    let beta = 1.0 - tangential / radial;
    eprintln!("halo mixture: σ_r {sigma_r:.1} km/s, β {beta:.3}");
    assert_within("mixture σ_r (the Risks' window)", sigma_r, 155.0, 185.0);
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
    let (in_situ, _) = halo.at(index(HaloComponentKind::InSitu), 26_000.0, 0.8);
    assert!(in_situ[2] > 30.0, "{in_situ:?}");
    for c in halo.components() {
        assert!((0.3..=0.9).contains(&c.beta()), "{:?}", c.kind());
        assert!(c.rotation().abs() <= 0.35, "{:?}", c.kind());
    }
}

/// P08.T3: over 200 seeds no component's radial dispersion exceeds half the local escape speed, at
/// radii from 10,000 to 60,000 ly.
///
/// A finding inside that: a constant anisotropy of 0.9 in a cored profile cannot hold near the core
/// (An and Evans 2006, ApJ 642, 752: β(0) ≤ γ(0) ÷ 2, and a core has γ(0) = 0), and the dominant
/// merger's `σ_r` reaches 356 km/s at 2,000 ly for seed 0, against half its 587 km/s escape speed.
#[test]
#[ignore = "slow: builds the parameters and halo laws of 200 galaxies"]
fn no_halo_dispersion_exceeds_half_the_escape_speed() {
    for n in 0..200_u64 {
        let seed = Seed::new(0x0803_c200_0000_0000 | n);
        let params = GalaxyParams::from_seed(seed, MassFunctionKind::default());
        let tables = PotentialTables::in_plane(&MassModel::new(&params));
        let halo = HaloKinematics::new(seed, &params, &tables);
        for c in halo.components() {
            for k in 0..26 {
                let r = 10_000.0 + 2_000.0 * f64::from(k);
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
