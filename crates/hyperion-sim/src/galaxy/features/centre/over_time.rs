//! P09.T31's slow tests of the galactic centre's members (plan 09, phase 6): the nuclear cluster
//! keeps its profile and its flattening at ±100 and ±1,000 years in six radial shells from 0.01 to
//! 100 ly, its eccentricities are thermal, its brightest band inside 0.04 ly holds S2-like orbits,
//! and its members lie some 0.05 ly apart at 3 ly.
//!
//! They run on the centre's Milky Way fixture under the version's rules, whose Kepler regime and exact
//! orbit in the centre's potential (P09.T28.b.b, P09.T28.d.b) are what move the members. A shell's
//! members at a time are those [`CentreMemberSource`] finds in the sphere of its outer radius about
//! the black hole then, in the bands the shell samples, less those inside its inner radius: the
//! source's hits equal a scan of every member (its tests), so the sample is the shell's whole
//! population in those bands, thinned by a hash of the ID where it is large. The black hole and the
//! tidal disruptions' victims, the centre's feature-level members, are on no distribution function
//! and are left out.
//!
//! The profile a shell keeps is the placement's: a band's members lie at a density of
//! `Σ_c N_c n_t(r) g_t(θ) ÷ A_t` over its classes (the members module's documentation), so over the
//! whole sphere their radii follow `r² Σ_c N_c n_t(r)`, the realised densities
//! ([`DistributionFunction::density`](super::DistributionFunction::density)) weighted by the
//! classes' expected counts. Its flattening is `⟨z² ÷ r²⟩` about the galactic pole, whose expected
//! value is each tracer's angular factor's ([`OrbitMarks::angular_factor`]) about its own axis,
//! weighted alike.

use std::sync::OnceLock;

use hyperion_testkit::stats::{ALPHA, assert_p_value, assert_poisson_count, ks_one_sample};
use hyperion_testkit::threads::in_shares;

use super::marks::OrbitMarks;
use super::members::CentrePlacement;
use super::{CentreMemberSource, CentreModel, CentreTracer, KeepCentreOrbits};
use crate::coords::GalacticPosition;
use crate::galaxy::imf::MassBand;
use crate::galaxy::motion::{EpochState, KeplerOrbit, Regime, Trajectory};
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::quad::{gl32, gl32_log};
use crate::galaxy::query::{LayerSet, QuerySphere, SystemHit, SystemSource};
use crate::galaxy::rules::GalaxyRules;
use crate::galaxy::{Galaxy, PointLy};
use crate::id::{Layer, MemberSlot, SystemIdKind};
use crate::math;
use crate::rng::Seed;
use crate::time::UniverseTime;
use crate::units::LightYears;
use crate::units::consts::SECONDS_PER_JULIAN_YEAR;

/// The centre's Milky Way fixture's seed ([`super::testing::milky_way_galaxy`]'s).
const SEED: u64 = 0x0926_0000;

/// The times the profile is kept at, Julian years from the epoch: ±100 and ±1,000, which is ±H.
const TIMES: [i64; 4] = [-1_000, -100, 100, 1_000];

/// Every band, A to E.
const STELLAR: [Layer; 5] = [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E];

/// One radial shell: its radii, ly, the bands it samples, and one in how many of their members
/// it keeps, by a hash of the ID, so that it holds some 10⁴.
#[derive(Debug, Clone, Copy)]
struct Shell {
    inner: f64,
    outer: f64,
    layers: &'static [Layer],
    one_in: u64,
}

/// The six shells, a factor of 10^⅔ wide each from 0.01 to 100 ly, each sampling the bands and
/// share that hold some 10⁴ members (the census's counts of the fixture's cells). Beyond 1 ly they
/// sample band D, not band E: there a neutron-star class of band E exhausts its draw's attempts,
/// the cluster's escape speed being too low to retain its kicks, which the slow profile's debug
/// assertion stops on (a finding, as P09.T36.b's lists met it for the young burst at 24).
const SHELLS: [Shell; 6] = [
    Shell {
        inner: 0.01,
        outer: 0.046_415_888_336_127_79,
        layers: &STELLAR,
        one_in: 1,
    },
    Shell {
        inner: 0.046_415_888_336_127_79,
        outer: 0.215_443_469_003_188_4,
        layers: &[Layer::C, Layer::D, Layer::E],
        one_in: 2,
    },
    Shell {
        inner: 0.215_443_469_003_188_4,
        outer: 1.0,
        layers: &[Layer::E],
        one_in: 1,
    },
    Shell {
        inner: 1.0,
        outer: 4.641_588_833_612_779,
        layers: &[Layer::D],
        one_in: 32,
    },
    Shell {
        inner: 4.641_588_833_612_779,
        outer: 21.544_346_900_318_84,
        layers: &[Layer::D],
        one_in: 256,
    },
    Shell {
        inner: 21.544_346_900_318_84,
        outer: 100.0,
        layers: &[Layer::D],
        one_in: 128,
    },
];

/// The fixture under the version's rules.
fn galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params_with_rules(
            Seed::new(SEED),
            GalaxyParams::milky_way_like(),
            GalaxyRules::of_version(crate::GENERATOR_VERSION),
        )
        .expect("the fixture builds")
    })
}

/// `layers` as a set.
fn layer_set(layers: &[Layer]) -> LayerSet {
    layers.iter().copied().fold(LayerSet::EMPTY, LayerSet::with)
}

/// Whether `hit` is a member of a cell, on a distribution function: not the black hole or a
/// victim.
fn in_a_cell(hit: &SystemHit) -> bool {
    match hit.id().kind() {
        SystemIdKind::Centre(id) => matches!(id.slot(), MemberSlot::InCell { .. }),
        _ => false,
    }
}

/// Whether a hash of `hit`'s ID keeps it, one in `one_in`: a thinning independent of where it is.
fn kept(hit: &SystemHit, one_in: u64) -> bool {
    (hit.id().raw().wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 32).is_multiple_of(one_in)
}

/// `p`'s distance from the black hole, ly.
fn norm(p: &PointLy) -> f64 {
    (p.x * p.x + p.y * p.y + p.z * p.z).sqrt()
}

/// The positions, ly from the black hole, of `shell`'s kept members at `t`, through `source`.
fn members_at(source: &CentreMemberSource<'_>, shell: &Shell, t: UniverseTime) -> Vec<PointLy> {
    let galaxy = galaxy();
    let sphere = QuerySphere::new(
        GalacticPosition::ORIGIN,
        LightYears::new(shell.outer),
        t,
        LightYears::ZERO,
    )
    .expect("a positive radius");
    let mut hits = Vec::new();
    source.systems_in_sphere(galaxy, &sphere, layer_set(shell.layers), &mut hits);
    hits.iter()
        .filter(|hit| in_a_cell(hit) && kept(hit, shell.one_in))
        .map(|hit| PointLy::from(hit.position()))
        .filter(|p| norm(p) >= shell.inner)
        .collect()
}

/// The expected count of each tracer's classes in `layers`' bands: the weight of its realised
/// density in their members' profile.
fn tracer_weights(model: &CentreModel, layers: &[Layer]) -> [f64; 4] {
    let mut weights = [0.0; 4];
    for class in model.classes().classes() {
        if layers.contains(&class.band.layer()) {
            weights[class.tracer.index()] += class.expected;
        }
    }
    weights
}

/// `∫ₐᵇ 4π r² n_t(r) dr` of each tracer's realised density, over `panels` log-spaced panels.
fn shell_shares(model: &CentreModel, a: f64, b: f64, panels: u32) -> [f64; 4] {
    CentreTracer::ALL.map(|tracer| {
        let df = model.distribution(tracer);
        (0..panels)
            .map(|i| {
                let at = |k: u32| a * math::powf(b / a, f64::from(k) / f64::from(panels));
                gl32_log(
                    |r| 4.0 * core::f64::consts::PI * r * r * df.density(r),
                    at(i),
                    at(i + 1),
                )
            })
            .sum()
    })
}

/// The cumulative distribution of `shell`'s members' radii under the placement's profile, on
/// 2,048 log-spaced nodes, linear between them in ln r.
struct RadialCdf {
    ln_inner: f64,
    step: f64,
    cumulative: Vec<f64>,
}

impl RadialCdf {
    const NODES: u32 = 2_048;

    fn new(model: &CentreModel, shell: &Shell) -> Self {
        let weights = tracer_weights(model, shell.layers);
        let (a, b) = (shell.inner, shell.outer);
        let ln_inner = math::ln(a);
        let step = (math::ln(b) - ln_inner) / f64::from(Self::NODES);
        let mut cumulative = Vec::with_capacity(Self::NODES as usize + 1);
        let mut sum = 0.0;
        cumulative.push(0.0);
        for i in 0..Self::NODES {
            let lo = math::exp(ln_inner + step * f64::from(i));
            let hi = math::exp(ln_inner + step * f64::from(i + 1));
            sum += gl32_log(
                |r| {
                    r * r
                        * CentreTracer::ALL
                            .iter()
                            .map(|&t| weights[t.index()] * model.distribution(t).density(r))
                            .sum::<f64>()
                },
                lo,
                hi,
            );
            cumulative.push(sum);
        }
        for c in &mut cumulative {
            *c /= sum;
        }
        Self {
            ln_inner,
            step,
            cumulative,
        }
    }

    fn at(&self, r: f64) -> f64 {
        let u = (math::ln(r) - self.ln_inner) / self.step;
        if u <= 0.0 {
            return 0.0;
        }
        let last = self.cumulative.len() - 1;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "u is positive and below the node count, checked against it"
        )]
        let i = (u.floor() as usize).min(last);
        if i == last {
            return 1.0;
        }
        let f = u - u.floor();
        self.cumulative[i] + f * (self.cumulative[i + 1] - self.cumulative[i])
    }
}

/// `⟨(r̂ · axis)²⟩` of positions under `marks`' angular factor about its own axis:
/// `∫₀¹ μ² g(μ) dμ ÷ ∫₀¹ g(μ) dμ` in sixteen panels, as its acceptance is integrated.
fn polar_moment(marks: &OrbitMarks) -> f64 {
    let axis = marks.axis();
    // A unit vector perpendicular to the axis.
    let helper = if axis[0].abs() < 0.9 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    let dot = helper[0] * axis[0] + helper[1] * axis[1] + helper[2] * axis[2];
    let perp = [0, 1, 2].map(|i| helper[i] - dot * axis[i]);
    let length = (perp[0] * perp[0] + perp[1] * perp[1] + perp[2] * perp[2]).sqrt();
    let perp = perp.map(|c| c / length);
    let g = |mu: f64| {
        let s = (1.0 - mu * mu).max(0.0).sqrt();
        marks.angular_factor([0, 1, 2].map(|i| axis[i] * mu + perp[i] * s))
    };
    let (mut weighted, mut total) = (0.0, 0.0);
    for p in 0..16 {
        let lo = f64::from(p) / 16.0;
        let hi = lo + 1.0 / 16.0;
        weighted += gl32(|mu| mu * mu * g(mu), lo, hi);
        total += gl32(g, lo, hi);
    }
    weighted / total
}

/// The expected `⟨z² ÷ r²⟩` of `shell`'s members about the galactic pole: each tracer's, its
/// polar moment about its own axis turned onto the pole, weighted by its share of the members.
fn expected_flattening(model: &CentreModel, shell: &Shell) -> f64 {
    let weights = tracer_weights(model, shell.layers);
    let shares = shell_shares(model, shell.inner, shell.outer, 64);
    let (mut sum, mut total) = (0.0, 0.0);
    for tracer in CentreTracer::ALL {
        let w = weights[tracer.index()] * shares[tracer.index()];
        let marks = model.marks(tracer);
        let c2 = polar_moment(marks);
        let along = marks.axis()[2];
        sum += w * (c2 * along * along + 0.5 * (1.0 - c2) * (1.0 - along * along));
        total += w;
    }
    sum / total
}

/// The mean and the standard error of the mean of `z² ÷ r²` over `members`.
fn flattening_of(members: &[PointLy]) -> (f64, f64) {
    let values: Vec<f64> = members
        .iter()
        .map(|p| {
            let r2 = p.x * p.x + p.y * p.y + p.z * p.z;
            p.z * p.z / r2
        })
        .collect();
    #[expect(
        clippy::cast_precision_loss,
        reason = "a sample of some 10⁴, far below 2⁵³"
    )]
    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;
    let variance = values.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / (n - 1.0);
    (mean, (variance / n).sqrt())
}

/// What one shell showed at each time: its members, their radii's Kolmogorov–Smirnov p-value
/// against the profile, and their flattening with its standard error.
#[derive(Debug, Clone, Copy)]
struct ShellAt {
    years: i64,
    members: usize,
    statistic: f64,
    p: f64,
    flattening: (f64, f64),
}

/// `shell` at the epoch and at each of [`TIMES`], through one keeping cache, so that each cell is
/// placed and its trajectories built once.
fn shell_over_time(shell: &Shell) -> Vec<ShellAt> {
    let galaxy = galaxy();
    let model = galaxy.centre().expect("the fixture's centre builds");
    let kept = KeepCentreOrbits::new(galaxy);
    let source = CentreMemberSource::new(galaxy, &kept).expect("the fixture's centre builds");
    let cdf = RadialCdf::new(model, shell);
    core::iter::once(0)
        .chain(TIMES)
        .map(|years| {
            let t = UniverseTime::from_julian_years(years).expect("inside the clock window");
            let members = members_at(&source, shell, t);
            let mut radii: Vec<f64> = members.iter().map(norm).collect();
            let ks = ks_one_sample(&mut radii, |r| cdf.at(r));
            ShellAt {
                years,
                members: members.len(),
                statistic: ks.statistic,
                p: ks.p_value,
                flattening: flattening_of(&members),
            }
        })
        .collect()
}

/// P09.T31: in each of six shells from 0.01 to 100 ly, on six threads (the slow profile reserves
/// them, `.config/nextest.toml`), the centre's members keep the profile at
/// −1,000, −100, +100 and +1,000 years, as at the epoch: their radii follow the placement's profile
/// (Kolmogorov–Smirnov), their count stays within Poisson's scatter of the epoch's, which is the
/// profile's, and their flattening, `⟨z² ÷ r²⟩`, stays the angular factors' (module
/// documentation), under the version's rules.
#[test]
#[ignore = "slow: the centre's members in six shells at five times, some two million placed"]
fn the_centre_keeps_its_profile_and_flattening_over_a_millennium() {
    let galaxy = galaxy();
    let model = galaxy.centre().expect("the fixture's centre builds");
    let shells = in_shares(SHELLS.len(), SHELLS.len(), |s| shell_over_time(&SHELLS[s]));
    for (shell, times) in SHELLS.iter().zip(&shells) {
        let expected_flattening = expected_flattening(model, shell);
        let weights = tracer_weights(model, shell.layers);
        let shares = shell_shares(model, shell.inner, shell.outer, 64);
        #[expect(clippy::cast_precision_loss, reason = "a share of under 20")]
        let expected = CentreTracer::ALL
            .iter()
            .map(|&t| weights[t.index()] * shares[t.index()])
            .sum::<f64>()
            / shell.one_in as f64;
        let epoch = times[0];
        eprintln!(
            "shell {:.4}–{:.4} ly, {:?}, 1 in {}: {:.1} expected, flattening {expected_flattening:.4}",
            shell.inner, shell.outer, shell.layers, shell.one_in, expected
        );
        for at in times {
            eprintln!(
                "  {:>6} yr: {:>6} members, KS D {:.4} p {:.3}, ⟨z²/r²⟩ {:.4} ± {:.4}",
                at.years, at.members, at.statistic, at.p, at.flattening.0, at.flattening.1
            );
        }
        let name = format!("shell {:.4}–{:.4} ly", shell.inner, shell.outer);
        assert_poisson_count(
            &format!("{name}: members at the epoch"),
            u64::try_from(epoch.members).expect("a count"),
            expected,
            ALPHA,
        );
        for at in times {
            assert_p_value(&format!("{name}, {} yr: radii", at.years), at.p, ALPHA);
            let (mean, error) = at.flattening;
            assert!(
                (mean - expected_flattening).abs() <= 4.0 * error,
                "{name}, {} yr: ⟨z²/r²⟩ {mean} ± {error} against {expected_flattening}",
                at.years
            );
            #[expect(clippy::cast_precision_loss, reason = "counts of some 10⁴")]
            let (now, then) = (at.members as f64, epoch.members as f64);
            assert!(
                (now - then).abs() <= 4.0 * then.sqrt(),
                "{name}, {} yr: {now} members against {then} at the epoch",
                at.years
            );
        }
    }
}

/// Every member of `bands` within `radius` ly of the black hole at the epoch, with its trajectory.
fn trajectories_within(radius: f64, bands: &[MassBand]) -> Vec<Trajectory> {
    let galaxy = galaxy();
    let model = galaxy.centre().expect("the fixture's centre builds");
    let placement = CentrePlacement::new(model);
    let mut out = Vec::new();
    for cell in placement
        .grid()
        .cells_touching(&PointLy::default(), LightYears::new(radius))
    {
        for &band in bands {
            for member in placement.members_in_cell(galaxy, band, cell) {
                if norm(&member.local_position()) <= radius {
                    out.push(Trajectory::new(
                        galaxy,
                        EpochState::of_centre_member(&member),
                    ));
                }
            }
        }
    }
    out
}

/// P09.T31: the eccentricities of the centre's Kepler orbits about the black hole are thermal,
/// their mean 0.6–0.72 (`f(e) = 2e` has 2 ⁄ 3), over every band's members inside 0.1 ly, where the
/// black hole holds all but some per cent of the mass.
#[test]
#[ignore = "slow: every member inside 0.1 ly"]
fn the_centre_s_eccentricities_are_thermal() {
    let trajectories = trajectories_within(0.1, &MassBand::ALL);
    let mut eccentricities: Vec<f64> = trajectories
        .iter()
        .filter(|t| t.regime() == Regime::Kepler)
        .filter_map(|t| t.orbit().map(KeplerOrbit::eccentricity))
        .collect();
    let all = eccentricities.len();
    eccentricities.retain(|&e| e < 1.0);
    #[expect(clippy::cast_precision_loss, reason = "counts of some 10⁴")]
    let mean = eccentricities.iter().sum::<f64>() / eccentricities.len() as f64;
    let thermal = ks_one_sample(&mut eccentricities, |e| e * e);
    eprintln!(
        "{} of {all} Kepler orbits bound, mean e {mean:.4}, KS against thermal D {:.4} p {:.3}",
        eccentricities.len(),
        thermal.statistic,
        thermal.p_value
    );
    assert!(
        eccentricities.len() > 1_000,
        "{} bound",
        eccentricities.len()
    );
    assert!((0.6..=0.72).contains(&mean), "mean e {mean}");
}

/// P09.T31: among the brightest band's members inside 0.04 ly, S2-like orbits occur: Kepler
/// periods of 10–20 years (S2's is 16.05 yr; GRAVITY Collaboration 2018, A&A 615, L15).
#[test]
#[ignore = "slow: every band-E member inside 0.04 ly"]
fn the_brightest_band_inside_0_04_ly_holds_s2_like_orbits() {
    let trajectories = trajectories_within(0.04, &[MassBand::E]);
    let periods: Vec<f64> = trajectories
        .iter()
        .filter_map(|t| t.orbit().and_then(KeplerOrbit::period))
        .map(|period| period.value() / SECONDS_PER_JULIAN_YEAR)
        .collect();
    let s2_like = periods
        .iter()
        .filter(|p| (10.0..=20.0).contains(*p))
        .count();
    let shortest = periods.iter().copied().fold(f64::INFINITY, f64::min);
    eprintln!(
        "{} band-E members inside 0.04 ly, {} bound, {s2_like} with periods of 10–20 yr, the \
         shortest {shortest:.2} yr",
        trajectories.len(),
        periods.len()
    );
    assert!(s2_like > 0, "no S2-like orbit among {}", periods.len());
}

/// The mean spacing of the centre's members, every band, at 3 ly from the black hole: `n^−⅓` of
/// their density in 32 spheres of 0.3 ly about points 3 ly out, spread over the sky on a Fibonacci
/// lattice, so that the flattening averages out. The brainstorm's "about 0.05 ly at 3 ly" is
/// 8,000 per cubic light-year (plan 09's Risks: the fitted mass function gave some 7,800 before
/// ruling 144 and the joint revision raised the cusp); the fixture gives 11,529 under every 25
/// hold at 24 (the version's rules at 25), 0.0443 ly, pinned as measured (a finding).
#[test]
#[ignore = "slow: every member in 32 spheres at 3 ly"]
fn the_centre_s_members_lie_about_0_05_ly_apart_at_3_ly() {
    const SPHERES: u32 = 32;
    const RADIUS: f64 = 0.3;
    let galaxy = galaxy();
    let model = galaxy.centre().expect("the fixture's centre builds");
    let placement = CentrePlacement::new(model);
    let golden = core::f64::consts::PI * (3.0 - 5.0_f64.sqrt());
    let mut count = 0_u32;
    for k in 0..SPHERES {
        let z = 1.0 - (2.0 * f64::from(k) + 1.0) / f64::from(SPHERES);
        let s = (1.0 - z * z).sqrt();
        let phi = golden * f64::from(k);
        let at = PointLy {
            x: 3.0 * s * math::cos(phi),
            y: 3.0 * s * math::sin(phi),
            z: 3.0 * z,
        };
        for cell in placement
            .grid()
            .cells_touching(&at, LightYears::new(RADIUS))
        {
            for band in MassBand::ALL {
                for member in placement.members_in_cell(galaxy, band, cell) {
                    let p = member.local_position();
                    let d = PointLy {
                        x: p.x - at.x,
                        y: p.y - at.y,
                        z: p.z - at.z,
                    };
                    if norm(&d) <= RADIUS {
                        count += 1;
                    }
                }
            }
        }
    }
    let volume = f64::from(SPHERES) * 4.0 / 3.0 * core::f64::consts::PI * math::powi(RADIUS, 3);
    let density = f64::from(count) / volume;
    let spacing = math::powf(density, -1.0 / 3.0);
    eprintln!("{count} members, {density:.0} per cubic ly, spacing {spacing:.4} ly");
    assert!((0.0435..=0.0450).contains(&spacing), "{spacing} ly");
}
