//! P14.T51.b's area-mix Bond albedo: its pieces against their integrals and sources, the band
//! weighting and the hosts, Sudarsky, Burrows and Pinto's classes, the stand-ins and their labels,
//! the cloud, and the reference Earth's calibration. Every row's albedo is read from the registry,
//! never copied (main, 2026-10-16).

use hyperion_testkit::float::assert_same_bits;

use super::*;
use crate::planetary::chem::partition::{
    ClimateSpan, ColumnInventory, Cover, CrustWater, Ground, WaterCover, partition,
};
use crate::substance::liquid_properties;
use crate::units::{KilogramsPerCubicMetre, MetresPerSecondSquared};

fn id(key: &str) -> SubstanceId {
    by_key(key).unwrap_or_else(|| panic!("{key} is a row"))
}

fn kelvin(t: f64) -> Kelvin {
    Kelvin::new(t)
}

/// What a pass on an old surface (4.5 Gyr, far above [`REGOLITH_FORMATION_TIME`]) in the state
/// `state` gives its lid's weathering, at the maturity `m` and the mean surface temperature `t`.
fn lid_at(m: f64, t: Kelvin, state: SurfaceState) -> LidWeathering {
    LidWeathering {
        maturity: m,
        temperature: t,
        surface_age: Years::new(4.5e9),
        state,
    }
}

/// The same on an airless surface.
fn airless_at(m: f64, t: Kelvin) -> LidWeathering {
    lid_at(m, t, SurfaceState::Airless)
}

/// A body term of the absorption `a` and the scattering `b`, m⁻¹.
fn body(a: f64, b: f64) -> BodyTerm {
    BodyTerm::Coefficients {
        absorption_per_m: a,
        scattering_per_m: b,
    }
}

/// Two shades' every value is the same, to the bit.
fn assert_same_shade(a: &HostShade, b: &HostShade) {
    a.assert_same_bits_as(b);
}

/// The unpolarised Fresnel reflectance of a flat surface of real index `n` > 1 for light from
/// outside at the incidence cosine `mu`.
fn fresnel_outside(mu: f64, n: f64) -> f64 {
    let transmitted = (1.0 - (1.0 - mu * mu) / (n * n)).sqrt();
    let s = (mu - n * transmitted) / (mu + n * transmitted);
    let p = (n * mu - transmitted) / (n * mu + transmitted);
    f64::midpoint(s * s, p * p)
}

/// The same surface's reflectance for light from inside at the incidence cosine `mu`, wholly
/// reflected beyond the critical angle.
fn fresnel_inside(mu: f64, n: f64) -> f64 {
    let sine_squared = n * n * (1.0 - mu * mu);
    if sine_squared >= 1.0 {
        return 1.0;
    }
    let transmitted = (1.0 - sine_squared).sqrt();
    let s = (n * mu - transmitted) / (n * mu + transmitted);
    let p = (mu - n * transmitted) / (mu + n * transmitted);
    f64::midpoint(s * s, p * p)
}

/// 2 ∫₀¹ f(μ) μ dμ by a midpoint rule of `steps` steps.
fn cosine_weighted(f: impl Fn(f64) -> f64, steps: u32) -> f64 {
    let h = 1.0 / f64::from(steps);
    let sum = (0..steps).fold(0.0, |sum, k| {
        let mu = (f64::from(k) + 0.5) * h;
        sum + f(mu) * mu
    });
    2.0 * sum * h
}

/// `r_d`(n) is its integral, within 10⁻¹⁰ of a 200,000-step midpoint rule from n = 1.05 to 3, and
/// the lane's figures from a 2 × 10⁶-step rule, 0.06641 at 1.333 and 0.06751 at 1.34; water's is
/// 3.3 times its normal-incidence reflectance (decision-p14-t49c-split §2).
#[test]
fn the_diffuse_fresnel_reflectance_is_its_integral() {
    for n in [1.05, 1.2, 1.29, 1.333, 1.34, 1.4, 1.6, 2.0, 3.0] {
        let closed = diffuse_fresnel_reflectance(n);
        let integral = cosine_weighted(|mu| fresnel_outside(mu, n), 200_000);
        assert!(
            (closed - integral).abs() < 1e-10,
            "n {n}: {closed} against {integral}"
        );
    }
    assert!((diffuse_fresnel_reflectance(1.333) - 0.066_41).abs() < 5e-6);
    assert!((diffuse_fresnel_reflectance(1.34) - 0.067_51).abs() < 5e-6);
    let normal = (0.333 / 2.333) * (0.333 / 2.333);
    let ratio = diffuse_fresnel_reflectance(1.333) / normal;
    assert!((3.2..3.4).contains(&ratio), "{ratio}");
    for n in [1.0, 0.9, f64::NAN] {
        assert_same_bits(diffuse_fresnel_reflectance(n), 0.0);
    }
}

/// `r_i` = 1 − (1 − `r_d`) ÷ n² is the direct internal integral, total internal reflection
/// included, within 10⁻⁷ of a 200,000-step rule: water's 0.47459 at 1.333, the ruling's 0.475.
#[test]
fn the_internal_reflectance_is_reciprocity_s() {
    for n in [1.2, 1.333, 1.34, 1.6, 2.0] {
        let reciprocal = internal_diffuse_reflectance(n, diffuse_fresnel_reflectance(n));
        let direct = cosine_weighted(|mu| fresnel_inside(mu, n), 200_000);
        assert!(
            (reciprocal - direct).abs() < 1e-7,
            "n {n}: {reciprocal} against {direct}"
        );
    }
    let water = internal_diffuse_reflectance(1.333, diffuse_fresnel_reflectance(1.333));
    assert!((water - 0.474_59).abs() < 5e-6, "{water}");
}

/// The body term is absent without the absorption or the scattering, a deep, flat, pure liquid,
/// and with both it adds light below the surface's, more where the liquid scatters more.
#[test]
fn the_body_term_is_absent_without_absorption_or_scattering() {
    let outer = diffuse_fresnel_reflectance(1.333);
    assert_same_bits(liquid_bin_albedo(1.333, BodyTerm::Absent), outer);
    for term in [
        body(0.0, 0.0),
        body(f64::NAN, 0.01),
        body(0.02, f64::INFINITY),
    ] {
        assert_same_bits(liquid_bin_albedo(1.333, term), outer);
    }
    let clear = liquid_bin_albedo(1.333, body(0.02, 0.002));
    let turbid = liquid_bin_albedo(1.333, body(0.02, 0.02));
    assert!(outer < clear && clear < turbid, "{outer} {clear} {turbid}");
    // A scattering body with no absorption: R∞ = 0.33, at most a third of the light below.
    let lossless = liquid_bin_albedo(1.333, body(0.0, 1.0));
    assert!(lossless < outer + 0.33, "{lossless}");
    // A record that states one coefficient alone has no body term.
    let sea = liquid_properties(id("H2O"), kelvin(288.0), Composition::Reference).unwrap();
    let absorbing = LiquidProperties {
        scattering_per_m: None,
        ..sea.clone()
    };
    let scattering = LiquidProperties {
        absorption_per_m: None,
        ..sea
    };
    for record in [&absorbing, &scattering] {
        assert_eq!(BodyTerm::of(record, 0), BodyTerm::Absent);
    }
}

/// Water's sea at 288 K, from P14.T49.h's record about a Sun: its `r_d` at its index, 1.33–1.34,
/// with its body's light in the blue, 0.066–0.09 (decision-p14-t49c-split §2, against 0.020 at
/// normal incidence; a pure, deep, flat liquid's, recorded); without its body term, its `r_d`
/// alone. Over the host's whole spectrum: its first bin stands for the light below 380 nm, body
/// term and all, and the light above 760 nm takes `r_d` at its last bin's index alone.
#[test]
fn water_s_sea_is_its_diffuse_reflectance_and_a_little_of_its_body() {
    let sea = liquid_properties(id("H2O"), kelvin(288.0), Composition::Reference).unwrap();
    let sun = HostShade::sun();
    let bond = liquid_bond(&sea, &sun.bins);
    assert!((0.066..0.09).contains(&bond), "{bond}");
    let mut by_bin = [0.0; LIQUID_WEIGHTS];
    for (k, &n) in sea.refractive_index.iter().enumerate() {
        by_bin[k + 1] = liquid_bin_albedo(n, BodyTerm::of(&sea, k));
    }
    by_bin[0] = by_bin[1];
    let last = sea.refractive_index[LIQUID_BINS - 1];
    by_bin[LIQUID_BINS + 1] = diffuse_fresnel_reflectance(last);
    assert_same_bits(bond, band_weighted(&by_bin, &sun.bins));
    let body: Vec<String> = sea
        .refractive_index
        .iter()
        .enumerate()
        .map(|(k, &n)| {
            format!(
                "{:.4}",
                liquid_bin_albedo(n, BodyTerm::of(&sea, k)) - diffuse_fresnel_reflectance(n)
            )
        })
        .collect();
    eprintln!("water's body term by bin, 380 to 760 nm: {body:?}");
    let bare = LiquidProperties {
        absorption_per_m: None,
        ..sea.clone()
    };
    let surface = liquid_bond(&bare, &sun.bins);
    let mut by_index = [0.0; LIQUID_WEIGHTS];
    for (albedo, &n) in by_index[1..=LIQUID_BINS]
        .iter_mut()
        .zip(&sea.refractive_index)
    {
        *albedo = diffuse_fresnel_reflectance(n);
    }
    by_index[0] = by_index[1];
    by_index[LIQUID_BINS + 1] = diffuse_fresnel_reflectance(last);
    assert_same_bits(surface, band_weighted(&by_index, &sun.bins));
    assert!(bond > surface, "{bond} {surface}");
    let (below, above) = (sun.bins[0], sun.bins[LIQUID_BINS + 1]);
    eprintln!(
        "water's sea at 288 K about a Sun: {bond:.5} ({surface:.5} at its surface alone); the \
         Sun's light below 380 nm {below:.5}, above 760 nm {above:.5}"
    );
}

/// The share of a black body's light at `t` K below `nm`, (15 ÷ π⁴) Σₙ e^{−nx} (x³ ÷ n + 3x² ÷ n² +
/// 6x ÷ n³ + 6 ÷ n⁴), x = c₂ ÷ λT, the Planck integral's series summed to its 600th term
/// (`hyperion-fit`'s `blackbody_share_below`, P14.T49.i's), independent of the rule.
fn planck_series_below(nm: f64, t: f64) -> f64 {
    let x = SECOND_RADIATION_CONSTANT_M_K / (nm * 1e-9 * t);
    let sum = (1..=600_u32).fold(0.0, |sum, n| {
        let n = f64::from(n);
        sum + math::exp(-n * x)
            * (x * x * x / n
                + 3.0 * x * x / (n * n)
                + 6.0 * x / (n * n * n)
                + 6.0 / (n * n * n * n))
    });
    let pi_squared = core::f64::consts::PI * core::f64::consts::PI;
    15.0 / (pi_squared * pi_squared) * sum
}

/// The Gauss–Legendre rule integrates every monomial to degree 15, and every bin's share of a black
/// body's light from 2,000 to 40,000 K within 10⁻¹² of a 20,000-interval Simpson sum in x: the
/// optics' bins between their edges, their last from 3,500 nm to +∞ (x from 0), and the liquids'
/// bins. The shares outside, each the rest of the light (the optics' below 390 nm, the liquids'
/// below 380 nm) or summed from the optics' bins (the liquids' above 760 nm), are the Planck
/// series' within 10⁻¹³, and each set sums to 1. A Sun sends 0.447 of its light into 380–760 nm,
/// and, as P14.T49.i's record computes them from the series, 11% below 390 nm and 43% into
/// 0.78–5 µm.
#[test]
fn the_planck_rule_is_within_its_stated_error() {
    for degree in 0..=15_u32 {
        let monomial = |x: f64| (0..degree).fold(1.0, |p, _| p * x);
        let sum = GL8_NODES
            .iter()
            .zip(GL8_WEIGHTS)
            .fold(0.0, |sum, (&x, w)| sum + w * monomial(x));
        let exact = if degree % 2 == 0 {
            2.0 / f64::from(degree + 1)
        } else {
            0.0
        };
        assert!((sum - exact).abs() < 1e-15, "degree {degree}: {sum}");
    }
    let simpson = |t: f64, shortest: f64, longest: f64| {
        let x_at = |nm: f64| SECOND_RADIATION_CONSTANT_M_K / (nm * 1e-9 * t);
        let (low, high) = (x_at(longest), x_at(shortest));
        let intervals = 20_000_u32;
        let h = (high - low) / f64::from(intervals);
        // x³ ÷ (eˣ − 1) goes to 0 as x² at x = 0, the light at +∞ nm.
        let f = |x: f64| {
            if x > 0.0 {
                x * x * x / math::exp_m1(x)
            } else {
                0.0
            }
        };
        let inner = (1..intervals).fold(0.0, |sum, k| {
            let weight = if k % 2 == 1 { 4.0 } else { 2.0 };
            sum + weight * f(low + f64::from(k) * h)
        });
        let pi_squared = core::f64::consts::PI * core::f64::consts::PI;
        15.0 / (pi_squared * pi_squared) * (f(low) + inner + f(high)) * h / 3.0
    };
    let (mut worst, mut worst_open) = (0.0_f64, 0.0_f64);
    let last = OPTICS_BANDS - 1;
    for t in [
        2_000.0, 2_500.0, 3_200.0, 4_620.0, 5_772.0, 7_500.0, 10_000.0, 20_000.0, 40_000.0,
    ] {
        let optics = optics_shares(kelvin(t));
        for (bin, &share) in optics.iter().enumerate().skip(1) {
            let longest = if bin == last {
                f64::INFINITY
            } else {
                SPECTRAL_BIN_EDGES_NM[bin + 1]
            };
            let rule = planck_share(kelvin(t), SPECTRAL_BIN_EDGES_NM[bin], longest);
            assert_same_bits(share, rule);
            let fine = simpson(t, SPECTRAL_BIN_EDGES_NM[bin], longest);
            worst = worst.max((rule / fine - 1.0).abs());
        }
        let liquids = liquid_shares(kelvin(t), &optics);
        for (pair, &share) in LIQUID_BIN_EDGES_NM
            .windows(2)
            .zip(&liquids[1..=LIQUID_BINS])
        {
            let fine = simpson(t, pair[0], pair[1]);
            worst = worst.max((share / fine - 1.0).abs());
        }
        let longest = LIQUID_BIN_EDGES_NM[LIQUID_BINS];
        for (share, series) in [
            (optics[0], planck_series_below(SPECTRAL_BIN_EDGES_NM[1], t)),
            (liquids[0], planck_series_below(LIQUID_BIN_EDGES_NM[0], t)),
            (
                liquids[LIQUID_BINS + 1],
                1.0 - planck_series_below(longest, t),
            ),
        ] {
            worst_open = worst_open.max((share - series).abs());
        }
        for sum in [optics.iter().sum::<f64>(), liquids.iter().sum::<f64>()] {
            assert!((sum - 1.0).abs() < 1e-15, "{t} K: {sum}");
        }
    }
    assert!(worst < 1e-12, "{worst}");
    assert!(worst_open < 1e-13, "{worst_open}");
    let sun = optics_shares(kelvin(5_772.0));
    let visible = planck_share(kelvin(5_772.0), 380.0, 760.0);
    assert!((visible - 0.447).abs() < 0.001, "{visible}");
    assert!((sun[0] - 0.11).abs() < 0.005, "{}", sun[0]);
    let infrared =
        sun[5..].iter().sum::<f64>() - planck_share(kelvin(5_772.0), 5_000.0, f64::INFINITY);
    assert!((infrared - 0.43).abs() < 0.005, "{infrared}");
    assert_same_bits(planck_share(kelvin(0.0), 380.0, 760.0), 0.0);
    for t in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let optics = optics_shares(kelvin(t));
        for &share in optics.iter().chain(&liquid_shares(kelvin(t), &optics)) {
            assert_same_bits(share, 0.0);
        }
    }
    eprintln!(
        "the Planck rule's worst relative error {worst:.1e} in a bin, the open shares' worst \
         {worst_open:.1e}; a Sun's light by the optics' bins: {sun:.5?}"
    );
}

/// The weighting reads any number of bands: a synthetic fourth band, an infrared bin beyond R,
/// leaves B, V and R's shares as they are and weighs by its own light; a band of no light weighs
/// nothing; and light in no band weighs the bands equally.
#[test]
fn the_weighting_is_band_agnostic() {
    let three = [0.2, 0.5, 0.8];
    assert!((band_weighted(&three, &[1.0, 2.0, 1.0]) - 0.5).abs() < 1e-15);
    let four = [0.2, 0.5, 0.8, 0.1];
    assert_same_bits(
        band_weighted(&four, &[1.0, 2.0, 1.0, 0.0]),
        band_weighted(&three, &[1.0, 2.0, 1.0]),
    );
    assert!((band_weighted(&four, &[1.0, 2.0, 1.0, 4.0]) - 0.3).abs() < 1e-15);
    assert!((band_weighted(&three, &[0.0, 0.0, 0.0]) - 0.5).abs() < 1e-15);
    let sun = kelvin(5_772.0);
    let visible: [f64; 3] = planck_shares(sun, &[380.0, 495.0, 595.0, 760.0]);
    let with_infrared: [f64; 4] = planck_shares(sun, &[380.0, 495.0, 595.0, 760.0, 2_500.0]);
    for (band, share) in visible.iter().enumerate() {
        assert_same_bits(with_infrared[band], *share);
    }
    assert!(with_infrared[3] > 0.0 && with_infrared.iter().sum::<f64>() < 1.0);
}

/// Several hosts weigh by their flux: the light of two is the sum of each one's, each times its
/// flux, and their giant albedos the flux-weighted mean; one host's flux scales out of every
/// normalised weighting; a host of no flux or no temperature adds nothing, and a sky of none takes
/// a Sun's, labelled.
#[test]
fn several_hosts_weigh_by_their_flux() {
    let (sun, red) = (kelvin(5_772.0), kelvin(3_200.0));
    let flux = EarthFluxes::new;
    let mut both = HostShadeSum::EMPTY;
    both.add(flux(1.0), sun);
    both.add(flux(3.0), red);
    let both = both.finish();
    let (alone, dwarf) = (HostShade::of_host(sun), HostShade::of_host(red));
    for band in 0..OPTICS_BANDS {
        let sum = alone.bands[band] + 3.0 * dwarf.bands[band];
        assert!((both.bands[band] / sum - 1.0).abs() < 1e-15);
    }
    for class in 0..SUDARSKY_CLASSES {
        let mean = f64::midpoint(alone.giants[class], 3.0 * dwarf.giants[class]) / 2.0;
        assert!((both.giants[class] - mean).abs() < 1e-15);
    }
    let mean = f64::midpoint(alone.ice_giant, 3.0 * dwarf.ice_giant) / 2.0;
    assert!((both.ice_giant - mean).abs() < 1e-15);
    let mut bright = HostShadeSum::EMPTY;
    bright.add(flux(40.0), sun);
    let bright = bright.finish();
    let water = id("H2O");
    assert!(
        (solid_bond(water, &bright.bands).bond - solid_bond(water, &alone.bands).bond).abs()
            < 1e-15
    );
    let mut dark = HostShadeSum::EMPTY;
    dark.add(flux(0.0), sun);
    dark.add(flux(1.0), kelvin(0.0));
    dark.add(flux(f64::NAN), sun);
    assert_same_shade(&dark.finish(), &HostShade::sun());
    assert_same_shade(&HostShadeSum::EMPTY.finish(), &HostShade::sun());
}

/// Lever (d): a call chain's book holds each distinct luminous host's light once, each entry the
/// light `optics_shares`, `liquid_shares` and `class_bonds` give at its temperature, and the
/// shade's constants; a shade read off it is the shade its body computes for itself, to the bit,
/// for one host or several, a host the book holds or not, and a body no host lights; and a moon's
/// planet's glow added to it leaves the rest as it was.
#[test]
fn a_shade_off_its_book_is_its_own_to_the_bit() {
    let hosts = [
        2_600.0, 3_200.0, 4_620.0, 5_772.0, 7_500.0, 12_000.0, 40_000.0,
    ]
    .map(kelvin);
    let repeated = [
        hosts[3],
        hosts[0],
        kelvin(0.0),
        kelvin(-5.0),
        kelvin(f64::NAN),
    ];
    let book = ShadeBook::new(hosts.iter().copied().chain(repeated));
    assert_eq!(book.hosts.len(), hosts.len());
    for (&host, light) in hosts.iter().zip(&book.hosts) {
        let bands = optics_shares(host);
        let bins = liquid_shares(host, &bands);
        let (giants, ice_giant) = class_bonds(host);
        let pairs = light
            .bands
            .iter()
            .zip(&bands)
            .chain(light.bins.iter().zip(&bins))
            .chain(light.giants.iter().zip(&giants));
        for (&a, &b) in pairs {
            assert_same_bits(a, b);
        }
        assert_same_bits(light.ice_giant, ice_giant);
        assert_same_bits(light.temperature.value(), host.value());
    }
    assert_same_bits(book.water_cloud, water_cloud_albedo());
    assert_same_bits(book.cloud_column.value(), reference_cloud_column().value());
    let flux = EarthFluxes::new;
    let skies: [&[(f64, Kelvin)]; 6] = [
        &[(1.0, hosts[3])],
        &[(0.37, hosts[1]), (2.5e-4, hosts[6])],
        &[(12.0, hosts[0]), (0.8, hosts[2]), (3.0e-3, hosts[4])],
        &[(1.0, kelvin(5_000.0)), (0.2, hosts[5])],
        &[(0.0, hosts[3]), (1.0, kelvin(0.0))],
        &[],
    ];
    let shade_of = |sky: &[(f64, Kelvin)], book: Option<&ShadeBook>| {
        let mut sum = HostShadeSum::EMPTY;
        for &(f, host) in sky {
            sum.add_from(flux(f), host, book);
        }
        sum.finish_from(book)
    };
    for sky in skies {
        let mut own = HostShadeSum::EMPTY;
        for &(f, host) in sky {
            own.add(flux(f), host);
        }
        assert_same_shade(&shade_of(sky, Some(&book)), &own.finish());
        assert_same_shade(&shade_of(sky, None), &own.finish());
    }
    let glow = book.clone().with_host(kelvin(110.0));
    assert_eq!(glow.hosts.len(), hosts.len() + 1);
    assert_eq!(glow.hosts[..hosts.len()], book.hosts[..]);
    let moon = [(1.0, hosts[3]), (4.0e-6, kelvin(110.0))];
    assert_same_shade(&shade_of(&moon, Some(&glow)), &shade_of(&moon, None));
    assert_eq!(book.clone().with_host(hosts[2]), book);
}

/// Every one of Sudarsky, Burrows and Pinto's Fiducial albedos is reproduced at its tabled host and
/// its class's model planet, class I's times its stand-ins (decision-p14-t54a-ganymede §2–3): the
/// hazes' 0.48 ÷ 0.57 for a gas giant and a sub-Neptune, the ice giants' 0.32 ÷ 0.57 for an ice
/// giant, which classes II to V leave alone; the hosts interpolate linearly and clamp beyond M4V
/// and A8V; the planets clamp below class I's 130 K and above class V's 1,700 K.
#[test]
fn every_sudarsky_node_is_reproduced_at_each_tabled_host() {
    for (row, &host) in SUDARSKY_HOSTS_K.iter().enumerate() {
        let shade = HostShade::of_host(kelvin(host));
        for (class, &planet) in SUDARSKY_PLANETS_K.iter().enumerate() {
            for (body, ratio) in [
                (PlanetClass::GasGiant, CLASS_I_HAZE_RATIO),
                (PlanetClass::SubNeptune, CLASS_I_HAZE_RATIO),
                (PlanetClass::IceGiant, ICE_GIANT_CLASS_I_RATIO),
            ] {
                let bond = giant_bond(&shade, kelvin(planet), body);
                let table = SUDARSKY_BOND[row][class] * if class == 0 { ratio } else { 1.0 };
                assert!(
                    (bond - table).abs() < 1e-12,
                    "{host} K, class {class}, {body:?}: {bond}"
                );
            }
        }
    }
    // Their stated values: class I about M4V, G2V and A8V under the hazes and for an ice giant.
    for (row, hazy, ice) in [(0, 0.32, 0.213), (3, 0.48, 0.32), (5, 0.531, 0.354)] {
        let shade = HostShade::of_host(kelvin(SUDARSKY_HOSTS_K[row]));
        let at = |class| giant_bond(&shade, kelvin(SUDARSKY_PLANETS_K[0]), class);
        assert!((at(PlanetClass::GasGiant) - hazy).abs() < 5e-4, "{row}");
        assert!((at(PlanetClass::IceGiant) - ice).abs() < 5e-4, "{row}");
    }
    let between = sudarsky_bonds(kelvin(f64::midpoint(5_770.0, 6_240.0)));
    assert!((between[0] - 0.58).abs() < 1e-12, "{between:?}");
    assert!((between[1] - 0.825).abs() < 1e-12, "{between:?}");
    for (beyond, edge) in [(2_500.0, 3_200.0), (12_000.0, 7_500.0)] {
        let (beyond, edge) = (sudarsky_bonds(kelvin(beyond)), sudarsky_bonds(kelvin(edge)));
        for class in 0..SUDARSKY_CLASSES {
            assert_same_bits(beyond[class], edge[class]);
        }
    }
    let g2 = HostShade::of_host(kelvin(5_770.0));
    let giant = PlanetClass::GasGiant;
    assert!((giant_bond(&g2, kelvin(50.0), giant) - 0.48).abs() < 1e-12);
    assert!((giant_bond(&g2, kelvin(50.0), PlanetClass::IceGiant) - 0.32).abs() < 1e-12);
    assert!((giant_bond(&g2, kelvin(3_000.0), giant) - 0.55).abs() < 1e-12);
    assert!((giant_bond(&g2, kelvin(3_000.0), PlanetClass::IceGiant) - 0.55).abs() < 1e-12);
    // About a Sun, 5,772 K: the Fiducial 0.570 085 times each ratio, and a one-host shade's class I
    // is `class_i_bond`'s to the bit.
    let sun = kelvin(5_772.0);
    assert!((class_i_bond(sun, giant) - 0.570_085 * 0.48 / 0.57).abs() < 1e-6);
    assert!((class_i_bond(sun, PlanetClass::IceGiant) - 0.570_085 * 0.32 / 0.57).abs() < 1e-6);
    let shade = HostShade::of_host(sun);
    for class in [giant, PlanetClass::IceGiant] {
        assert_same_bits(
            giant_bond(&shade, kelvin(50.0), class),
            class_i_bond(sun, class),
        );
    }
}

/// An ice giant takes the ice giants' class I only at and below class I's 130 K model planet: above
/// it, its albedo runs to class II's as a gas giant's does, and at 250 K and above the two are the
/// same (decision-p14-t54a-ganymede §3).
#[test]
fn an_ice_giant_s_class_i_joins_class_ii() {
    let g2 = HostShade::of_host(kelvin(5_770.0));
    let (gas, ice) = (PlanetClass::GasGiant, PlanetClass::IceGiant);
    let at_190 = giant_bond(&g2, kelvin(190.0), ice);
    assert!(
        (at_190 - f64::midpoint(0.32, 0.81)).abs() < 1e-12,
        "{at_190}"
    );
    assert!(at_190 < giant_bond(&g2, kelvin(190.0), gas));
    for t in [250.0, 400.0, 1_500.0] {
        assert_same_bits(
            giant_bond(&g2, kelvin(t), ice),
            giant_bond(&g2, kelvin(t), gas),
        );
    }
}

/// A 1,200 K giant about a G2V star takes class IV's 0.03, between IV's and V's inclusive; a 1,500
/// K one lies strictly between 0.03 and 0.55; the albedo is continuous in the temperature.
#[test]
fn a_hot_giant_lies_between_its_classes() {
    let g2 = HostShade::of_host(kelvin(5_770.0));
    let giant = PlanetClass::GasGiant;
    let at_1200 = giant_bond(&g2, kelvin(1_200.0), giant);
    assert!((at_1200 - 0.03).abs() < 1e-12, "{at_1200}");
    assert!((0.03..=0.55).contains(&at_1200));
    let at_1500 = giant_bond(&g2, kelvin(1_500.0), giant);
    assert!(at_1500 > 0.03 && at_1500 < 0.55, "{at_1500}");
    for node in SUDARSKY_PLANETS_K {
        let (below, above) = (
            giant_bond(&g2, kelvin(node - 1e-9), giant),
            giant_bond(&g2, kelvin(node + 1e-9), giant),
        );
        assert!((above - below).abs() < 1e-9, "{node} K");
    }
}

/// A solid's Bond albedo is its spherical albedo by bin weighted by the light, never its normal
/// albedo; a row without condensed optics takes water frost's, labelled: `SiC`, `Kr`, `C2H6` and
/// `CH3OH`, which P14.T49.i left without optics (each checked here); every stand-in has a label.
#[test]
fn a_solid_without_optics_takes_water_frost_s_labelled() {
    use crate::substance::optics::BVR_BINS;

    let sun = sun_bands();
    let water = solid_bond(id("H2O"), &sun);
    assert_eq!(water.stand_in, None);
    let optics = id("H2O").substance().condensed_optics().unwrap();
    assert_same_bits(
        water.bond,
        band_weighted(&optics.spherical_albedo_bins(), &sun),
    );
    let normal = band_weighted(&optics.normal_albedo_bvr(), &BVR_BINS.map(|bin| sun[bin]));
    assert!(
        (water.bond - normal).abs() > 0.01,
        "{} {normal}",
        water.bond
    );
    for key in ["SiC", "Kr", "C2H6", "CH3OH"] {
        assert!(id(key).substance().condensed_optics().is_none(), "{key}");
        let stood = solid_bond(id(key), &sun);
        assert_eq!(
            stood.stand_in,
            Some(StandIn::FrostForAnUnmodelledSolid),
            "{key}"
        );
        assert_same_bits(stood.bond, water.bond);
    }
    for stand_in in [
        StandIn::FrostForAnUnmodelledSolid,
        StandIn::SolventForASolution,
        StandIn::IndexForAnUnrecordedLiquid,
        StandIn::RecordAtItsFirstMelt,
        StandIn::SpaceWeathering,
        StandIn::UnweatheredLava,
        StandIn::MoonForAnUnknownAirlessLid,
        StandIn::RowByMaterial,
    ] {
        assert!(stand_in.label().len() > 40, "{stand_in:?}");
    }
}

/// The space-weathering stand-in never brightens an airless silicate body of a known lid against
/// T13.c's 0.11, and lets a dark row go darker; an airless rocky body whose lid is not known takes
/// the Moon's 0.11 exactly, neither brightened nor darkened; under air a lid not known takes
/// `basalt`'s row and ice `H2O`'s frost, labelled; a known lid under air its row as it is; a
/// weathered regolith (P14.T54.b's seam, which P14.T51.d's tests reach) weighs its own bands.
#[test]
fn the_space_weathering_stand_in_never_brightens_an_airless_silicate_body() {
    let sun = sun_bands();
    let moon = SurfaceState::Airless.albedo(SurfaceMaterial::Rock).value();
    for (row, _) in crate::substance::optics::rows() {
        let bond = solid_bond(row, &sun).bond;
        let capped = airless_silicate_stand_in(bond);
        assert!(
            capped <= moon && capped <= bond,
            "{}",
            row.substance().key()
        );
    }
    let rock = Regolith::of_material(SurfaceMaterial::Rock);
    assert_eq!(rock.row, None);
    let basalt = solid_bond(id("basalt"), &sun).bond;
    let unknown = rock.bond(SurfaceState::Airless, &sun);
    assert_eq!(unknown.stand_in, Some(StandIn::MoonForAnUnknownAirlessLid));
    assert_same_bits(unknown.bond, moon);
    let land = rock.bond(SurfaceState::Temperate, &sun);
    assert_eq!(land.stand_in, Some(StandIn::RowByMaterial));
    assert_same_bits(land.bond, basalt);
    let lid = Regolith::with_lid(id("basalt"), SurfaceMaterial::Rock);
    assert_eq!(lid.row, Some(id("basalt")));
    let airless = lid.bond(SurfaceState::Airless, &sun);
    assert_eq!(airless.stand_in, Some(StandIn::SpaceWeathering));
    assert_same_bits(airless.bond, airless_silicate_stand_in(basalt));
    let under_air = lid.bond(SurfaceState::Temperate, &sun);
    assert_eq!(under_air.stand_in, None);
    assert_same_bits(under_air.bond, basalt);
    let ice = Regolith::of_material(SurfaceMaterial::Ice).bond(SurfaceState::Airless, &sun);
    assert_eq!(ice.stand_in, Some(StandIn::RowByMaterial));
    assert_same_bits(ice.bond, solid_bond(id("H2O"), &sun).bond);
    let bins = [
        0.05, 0.06, 0.07, 0.08, 0.09, 0.10, 0.11, 0.12, 0.13, 0.14, 0.15, 0.16,
    ];
    let weathered = Regolith::weathered(SurfaceMaterial::Rock, bins);
    assert_eq!(weathered.row, None);
    let weighed = weathered.bond(SurfaceState::Airless, &sun);
    assert_eq!(weighed.stand_in, None);
    assert_same_bits(weighed.bond, band_weighted(&bins, &sun));
    eprintln!(
        "basalt about a Sun: {basalt:.5}; an airless lid of basalt, by the stand-in, {:.5}; an \
         airless lid not known {:.5}",
        airless.bond, unknown.bond
    );
}

/// The space-weathering cap is keyed on the lid's row's family (P14.T54.a; decision-p14-t54a-passes
/// §3.4, the six points' item 4): on an airless body every lid T51.c names whose row is `Silicate`
/// takes at most the Moon's 0.11, whatever the bulk's material (Ceres's `phyllosilicate` on an icy
/// bulk, Mercury's `basalt_reduced`, a Vesta-like `basalt_fe`); an airless water-ice lid keeps its
/// frost on a rocky bulk (Europa's, a Rocky class at 8% water) and an `Organic` lid its own row (a
/// comet's `cometary`); under air every lid its row as it is. The fresh rows' Bond albedos about a
/// Sun are printed for the bless note's capped rows.
#[test]
fn the_airless_cap_is_keyed_on_the_lid_s_row() {
    use crate::planetary::hooks::crust::CrustLithology;

    let sun = sun_bands();
    let moon = SurfaceState::Airless.albedo(SurfaceMaterial::Rock).value();
    let mut capped = Vec::new();
    for lithology in CrustLithology::ALL {
        let row = lithology.substance();
        let fresh = solid_bond(row, &sun).bond;
        let silicate = row
            .substance()
            .condensed()
            .is_some_and(|c| c.mechanics() == MechanicsFamily::Silicate);
        for material in [SurfaceMaterial::Rock, SurfaceMaterial::Ice] {
            let lid = Regolith::with_lid(row, material);
            let airless = lid.bond(SurfaceState::Airless, &sun);
            assert_eq!(capped_lid(row, SurfaceState::Airless), silicate);
            if silicate {
                assert_eq!(
                    airless.stand_in,
                    Some(StandIn::SpaceWeathering),
                    "{lithology:?}"
                );
                assert_same_bits(airless.bond, airless_silicate_stand_in(fresh));
            } else {
                assert_eq!(airless.stand_in, None, "{lithology:?}");
                assert_same_bits(airless.bond, fresh);
            }
            for state in [SurfaceState::Temperate, SurfaceState::Snowball] {
                assert!(!capped_lid(row, state));
                assert_same_bits(lid.bond(state, &sun).bond, fresh);
            }
        }
        if silicate {
            capped.push(format!(
                "{} {fresh:.4} → {:.4}",
                lithology.key(),
                airless_silicate_stand_in(fresh)
            ));
        }
    }
    let at = |lithology: CrustLithology, material: SurfaceMaterial| {
        Regolith::with_lid(lithology.substance(), material)
            .bond(SurfaceState::Airless, &sun)
            .bond
    };
    // Mercury, Ceres and a Vesta-like basaltic body are held at the Moon's 0.11.
    for (lithology, material) in [
        (CrustLithology::ReducedBasalt, SurfaceMaterial::Rock),
        (CrustLithology::Phyllosilicate, SurfaceMaterial::Ice),
        (CrustLithology::IronRichBasalt, SurfaceMaterial::Rock),
    ] {
        assert_same_bits(at(lithology, material), moon);
    }
    // Europa's water ice on a rocky bulk keeps its frost, and a comet's dust its darkness.
    assert!(at(CrustLithology::Water, SurfaceMaterial::Rock) > 0.5);
    assert!(at(CrustLithology::Cometary, SurfaceMaterial::Ice) < 0.11);
    eprintln!(
        "the airless cap, fresh → capped, about a Sun: {}; H2O {:.4}, cometary {:.4} uncapped",
        capped.join(", "),
        at(CrustLithology::Water, SurfaceMaterial::Rock),
        at(CrustLithology::Cometary, SurfaceMaterial::Ice),
    );
}

/// `run`'s result with every landed hold of version 24 at its bump's rules
/// (decision-t24i-airless-state D3.3): the darkened rows (P14.T49.k), the read densities
/// (P14.T49.n), the compressed liquids, the unlit mean, and the escape's batch with the Rayleigh
/// floor it switches. The moon bulk's and the cold traps' rules are the system's and the record's,
/// which no piece of the area mix reads.
fn under_24_s_holds<R>(run: impl FnOnce() -> R) -> R {
    use crate::planetary::chem::escape::{EscapeRules, with_rules};
    use crate::planetary::derive::atmosphere::{UnlitMeanRules, with_unlit_mean};
    use crate::substance::optics::{RowDarkeningRules, with_row_darkening};
    use crate::substance::{
        CompressedLiquidsRules, ReadDensitiesRules, with_compressed_liquids, with_read_densities,
    };
    with_row_darkening(RowDarkeningRules::Darkened, || {
        with_read_densities(ReadDensitiesRules::Read, || {
            with_compressed_liquids(CompressedLiquidsRules::Compressed, || {
                with_unlit_mean(UnlitMeanRules::Floored, || {
                    with_rules(EscapeRules::Batch, run)
                })
            })
        })
    })
}

/// Under 24's rows, every landed hold of 24 in force (P14.T49.k; decision-p14-t49k Q5): the
/// darkened `basalt_reduced`, graphite-bearing, lies above the Moon's 0.11, so the cap still holds
/// Mercury's lid at 0.11, and a Vesta-like `basalt_fe` lid too, its row unchanged, whose `A_N` in V
/// is 23's to the bit (`reference.rs`'s 0.410); under air the two darkened rows are their own,
/// darker than 23's, stated with Mars by decision-pre-23-review item 13's terms (its `basalt_fe`
/// lid with `ferric` over 0.31 of the land, its ice and cloud aside).
#[test]
fn under_24_s_rows_the_airless_cap_holds_mercury_and_a_vesta_like_basalt() {
    use crate::planetary::hooks::crust::CrustLithology;

    let sun = sun_bands();
    let moon = SurfaceState::Airless.albedo(SurfaceMaterial::Rock).value();
    let read = || {
        let lid = |lithology: CrustLithology, state| {
            Regolith::with_lid(lithology.substance(), SurfaceMaterial::Rock).bond(state, &sun)
        };
        let basalt_fe = id("basalt_fe").substance().condensed_optics().unwrap();
        (
            solid_bond(id("basalt_reduced"), &sun).bond,
            solid_bond(id("ferric"), &sun).bond,
            solid_bond(id("basalt_fe"), &sun).bond,
            basalt_fe.normal_albedo_bvr()[1],
            lid(CrustLithology::ReducedBasalt, SurfaceState::Airless),
            lid(CrustLithology::IronRichBasalt, SurfaceState::Airless),
            lid(CrustLithology::ReducedBasalt, SurfaceState::Temperate),
        )
    };
    let held = read();
    let (reduced, ferric, basalt_fe, basalt_fe_v, mercury, vesta, under_air) =
        under_24_s_holds(read);
    assert!(
        reduced > moon && reduced < held.0,
        "basalt_reduced {reduced} against the cap {moon} and 23's {}",
        held.0
    );
    assert!(ferric < held.1, "ferric {ferric} against 23's {}", held.1);
    assert_same_bits(basalt_fe, held.2);
    assert_same_bits(basalt_fe_v, held.3);
    assert!((basalt_fe_v - 0.410).abs() < 5e-4, "{basalt_fe_v}");
    for (body, capped) in [("Mercury", mercury), ("a Vesta-like basalt_fe", vesta)] {
        assert_eq!(capped.stand_in, Some(StandIn::SpaceWeathering), "{body}");
        assert_same_bits(capped.bond, moon);
    }
    assert_eq!(under_air.stand_in, None);
    assert_same_bits(under_air.bond, reduced);
    let mars = |lid: f64, deposit: f64| 0.69 * lid + 0.31 * deposit;
    eprintln!(
        "under 24's rows, about a Sun: basalt_reduced {:.4} → {reduced:.4} (airless, capped, \
         {:.4}); ferric {:.4} → {ferric:.4}; basalt_fe {basalt_fe:.4} unchanged; Mars by item \
         13's terms, ice and cloud aside, {:.4} → {:.4}",
        held.0,
        mercury.bond,
        held.1,
        mars(held.2, held.1),
        mars(basalt_fe, ferric),
    );
}

/// The column weights of an air of `fractions` by mole at a surface pressure `pressure` (the
/// partition tests' rule): each x μ ÷ μ̄ of it.
fn weights_of(pressure: f64, fractions: &[(&str, f64)]) -> Vec<(SubstanceId, Pascals)> {
    let molar = |key: &str| id(key).substance().molar_mass_g_per_mol().unwrap();
    let sum = fractions.iter().fold(0.0, |s, &(_, x)| s + x);
    let mean = fractions
        .iter()
        .fold(0.0, |s, &(k, x)| s + x / sum * molar(k));
    fractions
        .iter()
        .map(|&(k, x)| (id(k), Pascals::new(pressure * x / sum * molar(k) / mean)))
        .collect()
}

/// The partition of `weights` over the span `cold` to `warm` about `mean`, at the gravity `g`.
fn partitioned(span: (f64, f64, f64), weights: Vec<(SubstanceId, Pascals)>, g: f64) -> Partition {
    let climate = ClimateSpan::new(kelvin(span.0), kelvin(span.1), kelvin(span.2)).unwrap();
    let inventory = ColumnInventory::new(weights).unwrap();
    partition(
        &climate,
        &Ground::without_basins(MetresPerSecondSquared::new(g)),
        &inventory,
    )
}

/// The cloud: Earth's 0.67, its water's column well above the reference's 47.9 kg m⁻²; Venus's
/// whole disc; a dry, thin-aired Mars-like world near none, its water its condensable at a tiny
/// column; airless and molten surfaces none. (A Mars with its carbon dioxide caps is T51.a's
/// partition fixture's: `a_mars_with_its_caps_takes_almost_no_cloud`.) A runaway solved without
/// its deck (P14.T53.b part 1 (b)) takes a temperate world's cloud over the same partition, at the
/// water cloud's albedo, and one under its deck Venus's 0.76 whatever the partition.
#[test]
fn the_cloud_follows_the_state_and_the_condensable_s_availability() {
    let reference = reference_cloud_column().value();
    assert!((reference - 47.9).abs() < 0.3, "{reference} kg m⁻²");
    let sun = HostShade::sun();
    assert_same_bits(sun.cloud_column.value(), reference);
    let earth_column = |kg: f64| kg * 9.82 / (4.0 * core::f64::consts::PI * 6.371e6 * 6.371e6);
    let earth = partitioned(
        (261.0, 288.0, 305.0),
        vec![
            (id("H2O"), Pascals::new(earth_column(1.4e21))),
            (id("N2"), Pascals::new(earth_column(3.87e18))),
            (id("CO2"), Pascals::new(42.0)),
            (id("Ar"), Pascals::new(earth_column(6.6e16))),
        ],
        9.82,
    );
    let temperate = (SurfaceState::Temperate, RunawayDeck::Kept);
    let runaway = (SurfaceState::RunawayGreenhouse, RunawayDeck::Kept);
    let deckless = (SurfaceState::RunawayGreenhouse, RunawayDeck::Deckless);
    assert_same_bits(
        cloud_fraction(temperate, &earth, &sun),
        SurfaceState::Temperate.cloud_fraction(),
    );
    assert_same_bits(cloud_fraction(runaway, &earth, &sun), 1.0);
    assert_same_bits(
        cloud_fraction(deckless, &earth, &sun),
        cloud_fraction(temperate, &earth, &sun),
    );
    for state in [SurfaceState::Airless, SurfaceState::MagmaOcean] {
        for deck in [RunawayDeck::Kept, RunawayDeck::Deckless] {
            assert_same_bits(cloud_fraction((state, deck), &earth, &sun), 0.0);
        }
    }
    let mut dry = weights_of(600.0, &[("N2", 0.98), ("Ar", 0.02)]);
    dry.push((id("H2O"), Pascals::new(2e4)));
    let dry = partitioned((150.0, 210.0, 240.0), dry, 3.72);
    let thin = cloud_fraction(temperate, &dry, &sun);
    assert!(thin < 0.02, "{thin}");
    assert_same_bits(cloud_fraction(deckless, &dry, &sun), thin);
    assert_same_bits(cloud_fraction(runaway, &dry, &sun), 1.0);
    assert_same_bits(cloud_albedo(runaway, &sun), 0.76);
    assert_same_bits(cloud_albedo(deckless, &sun), sun.water_cloud);
    assert_same_bits(cloud_albedo(deckless, &sun), cloud_albedo(temperate, &sun));
    eprintln!(
        "cloud: Earth {:.3}; a dry Mars-like world {thin:.2e}",
        cloud_fraction(temperate, &earth, &sun)
    );
}

/// The reference Earth's area mix gives NASA's 0.294 by its cloud's calibration, to 10⁻⁶: its
/// clear sky, its ocean at water's liquid albedo, its land ice at `H2O`'s frost and its land at
/// `basalt`'s, about a Sun, with the water cloud over 0.67 of it.
#[test]
fn the_reference_earth_takes_0_294_by_its_cloud_s_calibration() {
    let clear = reference_earth_clear_sky();
    let cloud = water_cloud_albedo();
    let earth = under_cloud(clear, SurfaceState::Temperate.cloud_fraction(), cloud);
    assert!((earth - 0.294).abs() < 1e-6, "{earth}");
    assert!((0.05..0.15).contains(&clear), "{clear}");
    assert!((0.3..0.5).contains(&cloud), "{cloud}");
    assert_same_bits(HostShade::sun().water_cloud, cloud);
    eprintln!("the reference Earth: A_clear {clear:.5}, A_cloud {cloud:.5}, A {earth:.6}");
}

/// A liquid takes its own record where it has one, unlabelled, at the temperature its condensate
/// lies at: Titan's seas, `ch4_c2h6_n2` (P14.T49.h step 3), and a 10-bar carbon dioxide world's
/// sea (step 2b). A solution without one takes its solvent's, labelled; a perchlorate brine, which
/// has no index yet, is one. A liquid without a record takes `r_d` at liquid nitrogen's 550 nm
/// index, labelled.
#[test]
fn a_solution_takes_its_own_record_or_its_solvent_s_and_an_unrecorded_liquid_nitrogen_s_index() {
    let sun = HostShade::sun();
    let titan = partitioned(
        (89.7, 93.65, 93.65),
        vec![
            (id("CH4"), Pascals::new(5.5e3)),
            (id("N2"), Pascals::new(1.418e5)),
            (id("H2O"), Pascals::new(1e9)),
        ],
        1.352,
    );
    let profile = |t: Kelvin| ((t.value() - 85.0) / 10.0).clamp(0.0, 1.0);
    let water = WaterCover {
        surface_water: crate::units::KilogramsPerSquareMetre::ZERO,
        potential_ocean: 0.0,
        ice_fraction: 0.0,
        ice_density: KilogramsPerCubicMetre::new(917.0),
        crust_water: CrustWater::Unlisted,
    };
    let cover = Cover {
        cold_area: &profile,
        state: SurfaceState::Snowball,
        water,
    };
    let published = titan.publish(&cover);
    let seas = published
        .entry(id("ch4_c2h6_n2"), CondensatePhase::Liquid)
        .unwrap();
    let solved = liquid_entry_bond(seas, &titan, &sun.bins);
    assert_eq!(solved.stand_in, None);
    let lies_at = titan.condensate(id("CH4")).unwrap().lies_at();
    let own = liquid_record(id("ch4_c2h6_n2"), lies_at, titan.surface_pressure())
        .in_range()
        .unwrap();
    assert_same_bits(solved.bond, liquid_bond(&own, &sun.bins));
    // The solvent's record remains the stand-in for a solution without one of its own.
    assert!(matches!(
        liquid_record(id("brine_naclo4"), kelvin(250.0), Pascals::ZERO),
        LiquidRecord::Unrecorded
    ));
    let sea = partitioned(
        (225.0, 250.0, 265.0),
        vec![(id("CO2"), Pascals::new(1e6))],
        9.82,
    );
    // Its annual mean runs from 225 K up, above its ice's edge: none of it freezes.
    let warm = |t: Kelvin| ((t.value() - 225.0) / 40.0).clamp(0.0, 1.0);
    let published = sea.publish(&Cover {
        cold_area: &warm,
        state: SurfaceState::Temperate,
        water,
    });
    // Liquid CO2 has a record of its own since P14.T49.h step 2b, R08.T5.k's file.
    let carbon_dioxide = published.entry(id("CO2"), CondensatePhase::Liquid).unwrap();
    let own_sea = liquid_entry_bond(carbon_dioxide, &sea, &sun.bins);
    assert_eq!(own_sea.stand_in, None);
    let lies_at = sea.condensate(id("CO2")).unwrap().lies_at();
    let record = liquid_record(id("CO2"), lies_at, sea.surface_pressure())
        .in_range()
        .unwrap();
    assert_same_bits(own_sea.bond, liquid_bond(&record, &sun.bins));
    // A liquid without a record takes liquid nitrogen's index at its normal boiling point.
    let stood = unrecorded_liquid_bond();
    assert!((0.04..0.05).contains(&stood), "{stood}");
    let boiling = nitrogen_normal_boiling_point();
    assert!((boiling.value() - 77.355).abs() < 0.01, "{boiling:?}");
    let nitrogen = liquid_record(id("N2"), boiling, Pascals::ZERO)
        .in_range()
        .unwrap();
    let index = nitrogen.refractive_index_at(STAND_IN_INDEX_WAVELENGTH_NM);
    assert_same_bits(stood, diffuse_fresnel_reflectance(index));
    eprintln!(
        "Titan's seas by their own record: {:.5}; a CO2 sea by its own: {:.5}; a liquid without a \
         record by nitrogen's index {index:.5} at its normal boiling point on the registry's \
         curve, {:.4} K: {stood:.5}",
        solved.bond,
        own_sea.bond,
        boiling.value(),
    );
}

/// The record of a liquid at its temperature names every case, so that a row with a record is
/// never read as having none: in its range, its record there; below its first melt (−∞ included)
/// its record at it; at or above its critical point (+∞ included) its record a part in a million
/// below it; at a temperature no bound holds (not a number, or above a range with no critical
/// point) its record at its first melt, labelled apart; and a row with no record, none.
#[test]
fn a_liquid_s_record_is_held_into_its_range_by_name() {
    let record = |key: &str, t: f64| liquid_properties(id(key), kelvin(t), Composition::Reference);
    let same = |held: &LiquidRecord, read: &LiquidProperties| {
        let held = match held {
            LiquidRecord::InRange(record) | LiquidRecord::AtFirstMelt(record) => record,
            LiquidRecord::Unrecorded => panic!("a record was expected"),
        };
        assert_eq!(format!("{held:?}"), format!("{read:?}"));
    };
    let mut beyond_no_critical_point = 0;
    for key in ["H2O", "CH4", "NH3", "N2", "C2H6", "H2SO4"] {
        let row = id(key).substance();
        let lowest = row.condensed().unwrap().transition().temperature().value();
        let Ok(at_lowest) = record(key, lowest) else {
            continue;
        };
        for below in [lowest - 10.0, f64::NEG_INFINITY] {
            let held = liquid_record(id(key), kelvin(below), Pascals::ZERO);
            assert!(
                matches!(held, LiquidRecord::InRange(_)),
                "{key} at {below} K"
            );
            same(&held, &at_lowest);
        }
        let nan = liquid_record(id(key), kelvin(f64::NAN), Pascals::ZERO);
        assert!(matches!(nan, LiquidRecord::AtFirstMelt(_)), "{key}");
        same(&nan, &at_lowest);
        // The range's upper bound as the record states it: the row's critical point, or for a
        // solution (the aqueous acid, P14.T49.h step 3) its components' lowest.
        let critical = match record(key, f64::INFINITY) {
            Err(ResolveLiquidError::OutOfRange { below, .. }) => below,
            other => panic!("{key} at +∞ K is out of its range: {other:?}"),
        };
        if let Some(critical) = critical {
            let near = record(key, critical.value() * (1.0 - 1e-6)).unwrap();
            for above in [critical.value(), critical.value() + 50.0, f64::INFINITY] {
                let held = liquid_record(id(key), kelvin(above), Pascals::ZERO);
                assert!(
                    matches!(held, LiquidRecord::InRange(_)),
                    "{key} at {above} K"
                );
                same(&held, &near);
            }
        } else {
            let infinite = liquid_record(id(key), kelvin(f64::INFINITY), Pascals::ZERO);
            assert!(matches!(infinite, LiquidRecord::AtFirstMelt(_)), "{key}");
            same(&infinite, &at_lowest);
            beyond_no_critical_point += 1;
        }
    }
    eprintln!("liquids with a record and no critical point: {beyond_no_critical_point}");
    let inside = record("H2O", 288.0).unwrap();
    same(
        &liquid_record(id("H2O"), kelvin(288.0), Pascals::ZERO),
        &inside,
    );
    // Rows with no liquid record (P14.T49.h's melts leave these out).
    for (key, t) in [
        ("ca_al_residue", 1_500.0),
        ("iron_nickel", 2_000.0),
        ("ca_al_residue", f64::NAN),
    ] {
        let none = liquid_record(id(key), kelvin(t), Pascals::ZERO);
        assert!(matches!(none, LiquidRecord::Unrecorded), "{key} at {t} K");
        assert!(none.in_range().is_none(), "{key} at {t} K");
    }
}

/// An Enceladus, the airless icy regolith's fresh water frost about a Sun (not a derived moon),
/// takes `H2O`'s frost by its material, labelled, near 0.8: within 0.7–0.9 (Howett et al. 2010's
/// 0.81 ± 0.04, from memory; Li et al. 2023's 0.76 ± 0.03, Icarus 394, 115429, its abstract read),
/// asserted since T51.b's area mix weights P14.T49.i's bins over the host's whole spectrum
/// (decision-p14-t49c-split §1.3; main's ruling, 2026-10-19), where it was visible-only, 0.98937.
/// It is T49.i's row-level check's weighting to 10⁻¹², each bin by the Planck series' light
/// between its edges, the first from 0 nm and the last to +∞ (`hyperion-fit`'s `bond_albedo`).
#[test]
fn an_enceladus_takes_its_fresh_frost_near_0_8() {
    let sun = HostShade::sun();
    let frost = Regolith::of_material(SurfaceMaterial::Ice).bond(SurfaceState::Airless, &sun.bands);
    assert_eq!(frost.stand_in, Some(StandIn::RowByMaterial));
    assert_same_bits(frost.bond, solid_bond(id("H2O"), &sun.bands).bond);
    eprintln!(
        "Enceladus by its fresh frost, over {OPTICS_BANDS} bins: {:.5}",
        frost.bond
    );
    assert!((0.7..=0.9).contains(&frost.bond), "{}", frost.bond);
    let bins = id("H2O")
        .substance()
        .condensed_optics()
        .unwrap()
        .spherical_albedo_bins();
    let below = |bin: usize| match bin {
        0 => 0.0,
        OPTICS_BANDS => 1.0,
        _ => planck_series_below(SPECTRAL_BIN_EDGES_NM[bin], SOLAR_EFFECTIVE_TEMPERATURE_K),
    };
    let by_series = bins.iter().enumerate().fold(0.0, |sum, (bin, &albedo)| {
        sum + albedo * (below(bin + 1) - below(bin))
    });
    assert!(
        (frost.bond - by_series).abs() < 1e-12,
        "{} against {by_series}",
        frost.bond
    );
}

/// E₁ by its series, −γ − ln x − Σ (−x)ᵏ ÷ (k k!) (Abramowitz and Stegun 1964, 5.1.11), summed to
/// convergence: an independent order to hold the floor's `E_n` to, by the recurrence n `E_{n+1}`(x)
/// = e^(−x) − x `E_n`(x) (5.1.14).
fn e1_by_series(x: f64) -> f64 {
    let (mut sum, mut term) = (0.0, 1.0);
    for k in 1..400 {
        let k = f64::from(k);
        term *= -x / k;
        let add = term / k;
        sum += add;
        if add.abs() < 1e-18 * sum.abs() {
            break;
        }
    }
    -core::f64::consts::EULER_GAMMA - math::ln(x) - sum
}

/// The floor's exponential integrals hold Abramowitz and Stegun's table at x = 1 (Table 5.1:
/// `E₃`(1) = 0.109 691 967, `E₄`(1) = 0.086 062 491, from `E₁`(1) = 0.219 383 934 by 5.1.14) and the
/// recurrence from an independent E₁ over 0.01–8, on both sides of the forms' switch at x = 1;
/// `E_n`(0) is 1 ÷ (n − 1), and both fall to 0 far out.
#[test]
fn the_floor_s_exponential_integrals_hold_their_table_and_recurrence() {
    use ExponentialOrder::{Four, Three};
    let (e3, e4) = (
        exponential_integral(Three, 1.0),
        exponential_integral(Four, 1.0),
    );
    assert!((e3 - 0.109_691_967).abs() < 1e-9, "E₃(1) = {e3}");
    assert!((e4 - 0.086_062_491).abs() < 1e-9, "E₄(1) = {e4}");
    assert_same_bits(exponential_integral(Three, 0.0), 0.5);
    assert_same_bits(exponential_integral(Four, 0.0), 1.0 / 3.0);
    assert_same_bits(exponential_integral(Three, 800.0), 0.0);
    for i in 1..=800 {
        let x = f64::from(i) * 0.01;
        let e = math::exp(-x);
        let e2 = e - x * e1_by_series(x);
        let e3 = (e - x * e2) / 2.0;
        let e4 = (e - x * e3) / 3.0;
        for (n, by_recurrence) in [(Three, e3), (Four, e4)] {
            let ours = exponential_integral(n, x);
            // The recurrence loses digits as x grows (each step subtracts nearly equal numbers),
            // so the bound is absolute, against values of order e^(−x) ÷ x.
            assert!(
                (ours - by_recurrence).abs() < 1e-11,
                "E_{}({x}): {ours} against {by_recurrence}",
                n.n()
            );
        }
    }
}

/// R(μ₀) of the Eddington conservative layer over black (Meador and Weaver 1980), the integrand of
/// [`conservative_spherical_albedo`].
fn eddington_reflectance(tau: f64, mu: f64) -> f64 {
    (0.75 * tau + (0.5 - 0.75 * mu) * (1.0 - math::exp(-tau / mu))) / (1.0 + 0.75 * tau)
}

/// The conservative layer's spherical albedo is its reflectance's sphere-weighted integral,
/// 2 ∫₀¹ R(μ₀) μ₀ dμ₀, by a fine midpoint sum, from thin to deep; it is τ in a thin layer, rises
/// with τ, and goes to 1 − 1 ÷ (¾τ) at depth.
#[test]
fn the_conservative_layer_s_albedo_is_its_reflectance_over_the_sphere() {
    let mut last = 0.0;
    for tau in [
        1e-4, 1e-3, 0.01, 0.0971, 0.3, 1.0, 3.0, 9.3, 34.0, 100.0, 1e3,
    ] {
        let closed = conservative_spherical_albedo(tau);
        let summed = cosine_weighted(|mu| eddington_reflectance(tau, mu), 200_000);
        assert!(
            (closed - summed).abs() < 1e-7 * closed.max(1e-3),
            "τ {tau}: {closed} against {summed}"
        );
        assert!(closed > last, "rises with τ at {tau}");
        last = closed;
    }
    let thin = conservative_spherical_albedo(1e-6);
    assert!((thin / 1e-6 - 1.0).abs() < 1e-4, "{thin}");
    let deep = conservative_spherical_albedo(1e4);
    assert!((deep - (1.0 - 1.0 / 7.5e3)).abs() < 1e-6, "{deep}");
    assert_same_bits(conservative_spherical_albedo(0.0), 0.0);
    assert_same_bits(conservative_spherical_albedo(f64::NAN), 0.0);
    assert_same_bits(conservative_spherical_albedo(f64::INFINITY), 1.0);
}

/// An air given by its gases' partial pressures, Pa, the rest none.
fn air(pressures: &[(Gas, f64)]) -> PartialPressures {
    PartialPressures::from_fn(|gas| {
        Pascals::new(
            pressures
                .iter()
                .find(|&&(g, _)| g == gas)
                .map_or(0.0, |&(_, p)| p),
        )
    })
}

/// Earth's air at sea level, 101,325 Pa: N₂, O₂ and Ar at their dry mole fractions with 360 ppm
/// of CO₂ (Bodhaine et al. 1999's), under 9.806 65 m s⁻².
fn earth_air() -> PartialPressures {
    let p = 101_325.0 / (0.780_84 + 0.209_46 + 0.009_34 + 0.000_36);
    air(&[
        (Gas::Nitrogen, 0.780_84 * p),
        (Gas::Oxygen, 0.209_46 * p),
        (Gas::Argon, 0.009_34 * p),
        (Gas::CarbonDioxide, 0.000_36 * p),
    ])
}

/// P14.T24.h.a's first test: a cloud-free N₂ column over a black surface takes the closed form, each
/// bin's depth at its longest wavelength by λ^−4.08 from N₂'s cross section at 550 nm over the
/// column p ÷ (g m), bin-weighted by the Sun's light, and none in the last bin; and Earth's column
/// gives `τ_R`(550) = 0.097 ± 0.002 (Bodhaine et al. 1999).
#[test]
fn a_cloud_free_nitrogen_column_over_black_takes_the_closed_form() {
    use crate::substance::rayleigh::rayleigh_cross_section;
    let g = MetresPerSecondSquared::new(9.806_65);
    let earth = rayleigh_optical_depth(&earth_air(), g);
    eprintln!("Earth's dry air at sea level: τ_R(550) = {earth:.5}");
    assert!((earth - 0.097).abs() <= 0.002, "{earth}");

    let sun = HostShade::sun();
    for bar in [0.1, 1.0, 10.0, 23.0, 100.0] {
        let p = bar * 1e5;
        let nitrogen = air(&[(Gas::Nitrogen, p)]);
        let sigma = rayleigh_cross_section(Gas::Nitrogen.substance())
            .unwrap()
            .cross_section()
            .value();
        let tau = p / (g.value() * Gas::Nitrogen.molecular_mass().value()) * sigma;
        let ratio = rayleigh_optical_depth(&nitrogen, g) / tau;
        assert!((ratio - 1.0).abs() < 1e-14, "{bar} bar: {ratio}");
        let (mut sum, mut weight) = (0.0, 0.0);
        for (bin, &light) in sun.bands.iter().enumerate() {
            let depth = if bin + 1 == OPTICS_BANDS {
                0.0
            } else {
                tau * math::powf(550.0 / SPECTRAL_BIN_EDGES_NM[bin + 1], 4.08)
            };
            sum += light * cosine_weighted(|mu| eddington_reflectance(depth, mu), 20_000);
            weight += light;
        }
        let by_hand = sum / weight;
        let floor = rayleigh_floor(&nitrogen, g, &sun);
        eprintln!(
            "{bar} bar of N₂ at 1 g about the Sun: τ_R(550) {tau:.4}, its own Bond {floor:.4}"
        );
        assert!((floor - by_hand).abs() < 1e-6, "{floor} against {by_hand}");
    }
    let earth_floor = rayleigh_floor(&earth_air(), g, &sun);
    eprintln!("Earth's air alone about the Sun: {earth_floor:.4}");
    assert!((0.03..0.07).contains(&earth_floor), "{earth_floor}");
}

/// The floor is a lower bound across each bin: with every bin's depth at its shortest wavelength in
/// place of its longest (the first bin's at 300 nm), the same column reflects more, at every depth;
/// and an air of no pressure, or under no gravity, has none.
#[test]
fn the_floor_lies_below_its_bins_shortest_wavelengths_and_is_none_without_air() {
    let sun = HostShade::sun();
    let g = MetresPerSecondSquared::new(9.806_65);
    for bar in [0.01, 0.3, 3.0, 30.0, 300.0] {
        let methane = air(&[(Gas::Methane, bar * 1e5)]);
        let tau = rayleigh_optical_depth(&methane, g);
        let upper: Vec<f64> = (0..OPTICS_BANDS)
            .map(|bin| {
                conservative_spherical_albedo(
                    tau * math::powf(550.0 / SPECTRAL_BIN_EDGES_NM[bin], 4.08),
                )
            })
            .collect();
        let floor = rayleigh_floor(&methane, g, &sun);
        assert!(floor < band_weighted(&upper, &sun.bands), "{bar} bar");
    }
    assert_same_bits(rayleigh_floor(&air(&[]), g, &sun), 0.0);
    let nitrogen = air(&[(Gas::Nitrogen, 1e5)]);
    assert_same_bits(
        rayleigh_floor(&nitrogen, MetresPerSecondSquared::new(0.0), &sun),
        0.0,
    );
}

/// The floor's pieces pinned to the bit (`planetary/rayleigh_floor.golden`), so that no rewrite
/// moves 24's Bond albedos unseen before P14.T54.b reads them: `E₃` and `E₄` on both sides of the
/// forms' switch at x = 1, the conservative layer's albedo over the tests' depths, and the optical
/// depth and the floor of Earth's air and of N₂ and CH₄ columns at 1 bar and 23 bar under 2.458
/// m s⁻² (`filler_c` e201's gravity), about a Sun and an F8 host (6,142 K). New at 23, an extension.
#[test]
fn the_floor_s_pieces_are_pinned() {
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;

    use crate::GENERATOR_VERSION;

    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    for x in [0.0, 1e-3, 0.1, 0.5, 1.0, 1.5, 4.0, 30.0, 700.0] {
        w.f64(
            &format!("E3({x})"),
            exponential_integral(ExponentialOrder::Three, x),
        );
        w.f64(
            &format!("E4({x})"),
            exponential_integral(ExponentialOrder::Four, x),
        );
    }
    for tau in [1e-4, 0.0971, 1.0, 9.3, 34.0, 1e3] {
        w.f64(
            &format!("conservative_spherical_albedo({tau})"),
            conservative_spherical_albedo(tau),
        );
    }
    let earth_g = MetresPerSecondSquared::new(9.806_65);
    let e201_g = MetresPerSecondSquared::new(2.458);
    let airs = [
        ("Earth's air at 1 g", earth_air(), earth_g),
        ("1 bar of N2 at 2.458", air(&[(Gas::Nitrogen, 1e5)]), e201_g),
        (
            "23 bar of N2 at 2.458",
            air(&[(Gas::Nitrogen, 2.3e6)]),
            e201_g,
        ),
        (
            "23 bar of CH4 at 2.458",
            air(&[(Gas::Methane, 2.3e6)]),
            e201_g,
        ),
    ];
    let hosts = [
        ("a Sun", HostShade::sun()),
        ("an F8", HostShade::of_host(kelvin(6_142.0))),
    ];
    for (name, partials, g) in &airs {
        w.f64(
            &format!("{name}: rayleigh_optical_depth"),
            rayleigh_optical_depth(partials, *g),
        );
        for (host, shade) in &hosts {
            w.f64(
                &format!("{name} about {host}: rayleigh_floor"),
                rayleigh_floor(partials, *g, shade),
            );
        }
    }
    golden!("planetary/rayleigh_floor", w.as_str());
}

/// The floor is held until P14.T54.b's bump: below it the rules are the area mix's alone, and the
/// tests' switch reaches the floor ahead of it.
#[test]
fn the_rayleigh_floor_is_held_only_until_the_24_bump() {
    assert!(
        GENERATOR_VERSION < RAYLEIGH_FLOOR_VERSION,
        "the 23 → 24 bump (P14.T54.b) puts the Rayleigh floor on the record: every body with a \
         surface takes the larger of its area mix and its air's own Rayleigh layer \
         (P14.T24.h.a). Delete `RAYLEIGH_FLOOR_VERSION`, `RayleighFloorRules` with its `Held` \
         arm, `with_rayleigh_floor` and its thread's rules, the `Held` branch of \
         `surface_mix::floored`, and this test; the floor's tests then run as the generator's"
    );
    assert_eq!(RayleighFloorRules::in_force(), RayleighFloorRules::Held);
    assert_eq!(
        with_rayleigh_floor(RayleighFloorRules::Floored, RayleighFloorRules::in_force),
        RayleighFloorRules::Floored
    );
    assert_eq!(RayleighFloorRules::in_force(), RayleighFloorRules::Held);
}

/// P14.T49.n's compression, held until 24 (`COMPRESSED_LIQUIDS_VERSION`): the area mix reads a
/// sea's record at the temperature its condensate lies at under its partition's surface pressure,
/// so that from 24 the Bond albedo's liquid and the partition's layer read one `ρ_l`(T, p)
/// (decision-p14-t54a-water-consistency's W4). By hand, a water sea at about 288 K under 20 bar of
/// nitrogen: under 23's rules its Bond albedo is the saturated record's, to the bit; under 24's its
/// record's density is the registry's law at its condensate's temperature and the surface
/// pressure, the density its published layer reads, to the bit, about 9 × 10⁻⁴ above the
/// saturated liquid's (`κ_T` Δp), and its Bond albedo that record's.
#[test]
fn under_compression_the_area_mix_s_sea_reads_its_layer_s_density() {
    use crate::substance::{CompressedLiquidsRules, with_compressed_liquids};

    let compressed = CompressedLiquidsRules::Compressed;
    let water = id("H2O");
    let sun = HostShade::sun();
    // By hand: Earth's water column under 20 bar of nitrogen, its sea Earth's surface water.
    let earth_column = |kg: f64| kg * 9.82 / (4.0 * core::f64::consts::PI * 6.371e6 * 6.371e6);
    let profile = |t: Kelvin| ((t.value() - 270.0) / 60.0).clamp(0.0, 1.0);
    let cover = Cover {
        cold_area: &profile,
        state: SurfaceState::Temperate,
        water: WaterCover {
            surface_water: KilogramsPerSquareMetre::new(2.7e6),
            potential_ocean: 0.7,
            ice_fraction: 0.0,
            ice_density: KilogramsPerCubicMetre::new(917.0),
            crust_water: CrustWater::Unlisted,
        },
    };
    let by_hand = || {
        let world = partitioned(
            (280.0, 288.0, 300.0),
            vec![
                (water, Pascals::new(earth_column(1.4e21))),
                (id("N2"), Pascals::new(2e6)),
            ],
            9.82,
        );
        let sea = *world
            .publish(&cover)
            .entry(water, CondensatePhase::Liquid)
            .expect("the world holds a sea");
        let lies_at = world.condensate(water).unwrap().lies_at();
        let pressure = world.surface_pressure();
        let bond = liquid_entry_bond(&sea, &world, &sun.bins);
        let record = liquid_record(water, lies_at, pressure).in_range().unwrap();
        (sea, (lies_at, pressure), bond, record)
    };
    let (_, (t, p), held, held_record) = by_hand();
    let (sea, state, moved, record) = with_compressed_liquids(compressed, by_hand);
    assert_eq!(
        held_record,
        liquid_properties(water, t, Composition::Reference).unwrap()
    );
    assert_same_bits(held.bond, liquid_bond(&held_record, &sun.bins));
    assert_same_bits(state.0.value(), t.value());
    assert_same_bits(state.1.value(), p.value());
    let law = water
        .substance()
        .compressed_liquid_density(t, p)
        .unwrap()
        .value();
    assert_same_bits(record.density_kg_m3, law);
    assert_same_bits(
        sea.reservoir_gel().unwrap().value(),
        sea.reservoir().value() / law,
    );
    assert_same_bits(moved.bond, liquid_bond(&record, &sun.bins));
    assert_eq!((held.stand_in, moved.stand_in), (None, None));
    assert!(
        (moved.bond - held.bond).abs() > 1e-6,
        "{moved:?} against {held:?}"
    );
    let relative = record.density_kg_m3 / held_record.density_kg_m3 - 1.0;
    assert!((5e-4..2e-3).contains(&relative), "{relative:e}");
    eprintln!(
        "a water sea at {:.2} K under {:.0} Pa: ρ {:.4} → {:.4} kg m⁻³ ({relative:.3e}), Bond \
         albedo {:.9} → {:.9}",
        t.value(),
        p.value(),
        held_record.density_kg_m3,
        record.density_kg_m3,
        held.bond,
        moved.bond
    );
}

/// P14.T49.n's compression, held until 24 (`COMPRESSED_LIQUIDS_VERSION`): the area mix's sea
/// record is the one the palette's liquid entry reads (`liquid_properties_at_pressure`), so that
/// from 24 the Bond albedo's liquid and the palette's read one `ρ_l`(T, p) at one state (W4). On
/// the record, `close_binary`'s `.0800`, a steam world's sea under 213 bar at about 583 K (the
/// partition's record test's body): read inside the switch, the palette's liquid entry's grain
/// density and the area mix's record at the entry's temperature under the record's surface pressure
/// are the same bits, and so is the whole record, more than 2% above the saturated liquid's; under
/// 23's rules both are the saturated liquid's. Each reader keeps its own temperature, the area mix
/// its condensate's `lies_at` and the palette the record's mean held at the boiling point
/// (decision-p14-t54a-water-consistency's Batch B follow-up), and its own surface pressure, the
/// partition's and the record's conditions'.
#[test]
fn under_compression_the_area_mix_s_sea_reads_the_palette_s_density() {
    use hyperion_surface::field::PaletteRole;

    use crate::Seed;
    use crate::galaxy::Galaxy;
    use crate::galaxy::params::GalaxyParams;
    use crate::id::SystemId;
    use crate::planetary::context::SystemContext;
    use crate::planetary::surface::{CoarseInputs, RECORD_TIME};
    use crate::planetary::system::generate;
    use crate::substance::{CompressedLiquidsRules, CondensedPhase, with_compressed_liquids};

    let compressed = CompressedLiquidsRules::Compressed;
    let water = id("H2O");
    // tests/planetary_golden.rs's `SYSTEMS_SEED` and close_binary's pinned ID.
    let galaxy = Galaxy::from_params(
        Seed::new(0x5eed_0000_0014_0032),
        GalaxyParams::milky_way_like(),
    )
    .expect("the Milky Way fixture's gas is mostly neutral");
    let system = SystemId::from_raw(0x4200_2cb2_0000_0009).expect("a pinned ID is well formed");
    let ctx = SystemContext::for_system(&galaxy, system).expect("a pinned ID names a system");
    let on_the_record = || {
        let snapshot = generate(galaxy.seed(), &ctx).snapshot_at(&ctx, RECORD_TIME);
        let record = snapshot
            .bodies()
            .iter()
            .find(|record| record.index().get() == 0x0800)
            .expect("close_binary holds its .0800");
        let surface = record.surface().ok().expect(".0800 has a surface");
        let pressure = surface.conditions().surface_pressure();
        let inputs = CoarseInputs::for_body(record, None, &ctx).expect(".0800 reads its inputs");
        let entry = inputs
            .palette()
            .entries()
            .iter()
            .find(|e| e.role == PaletteRole::Liquid && e.substance.as_str() == "H2O")
            .expect(".0800's palette holds its sea")
            .clone();
        let sea = entry.liquid.expect("a sea carries its record");
        let area_mix = liquid_record(water, sea.temperature, pressure)
            .in_range()
            .expect("the entry's temperature lies in water's range");
        (entry.grain_density_kg_m3, sea, area_mix, pressure)
    };
    let (held_grain, held_sea, held_mix, _) = on_the_record();
    let (grain, sea, mix, pressure) = with_compressed_liquids(compressed, on_the_record);
    let saturated = water
        .substance()
        .grain_density(CondensedPhase::Liquid, held_sea.temperature)
        .unwrap()
        .value();
    for (grain, sea, mix) in [(held_grain, &held_sea, &held_mix), (grain, &sea, &mix)] {
        assert_same_bits(mix.density_kg_m3, grain);
        assert_eq!(mix, sea);
    }
    assert_same_bits(held_grain, saturated);
    let law = water
        .substance()
        .compressed_liquid_density(sea.temperature, pressure)
        .unwrap()
        .value();
    assert_same_bits(grain, law);
    assert!(grain > 1.02 * saturated, "{grain} against {saturated}");
    eprintln!(
        "close_binary .0800 under {:.4} bar: the palette's sea at {:.3} K and the area mix's \
         record there, ρ {held_grain:.4} → {grain:.4} kg m⁻³",
        pressure.value() / 1e5,
        sea.temperature.value()
    );
}

/// The weathered area mix is held until P14.T54.b's bump (P14.T54.b.e): below it the rules are the
/// stand-in's, no pass reads its record's case (its closure is never called), and a known airless
/// silicate lid on the stand-in's arm is still capped at the Moon's 0.11; the tests' switch reaches
/// the weathered arm ahead of it.
#[test]
fn the_weathered_area_mix_is_held_only_until_the_24_bump() {
    use crate::planetary::derive::atmosphere::{NoMaturity, PassWeathering};

    assert_eq!(WEATHERED_AREA_MIX_VERSION, GeneratorVersion::new(24));
    assert!(
        GENERATOR_VERSION < WEATHERED_AREA_MIX_VERSION,
        "the 23 → 24 bump (P14.T54.b.l) weathers the area mix: every pass's known lid weathered at \
         its body's maturity by R10.T1.d's law, `basalt_reduced`'s kept at the cap, and `ferric`'s \
         deposit in the mix. Delete `WEATHERED_AREA_MIX_VERSION`, `AreaMixWeathering` with its \
         `Held` arm, `with_area_mix_weathering` and its thread's rules, `NoMaturity::Held` and \
         `PassWeathering::in_force`'s held arm, `planet_field_rules` and `reads_modifier_star`'s \
         held arms, its line in `with_every_24_hold`, the scan's `AreaMixWeathering` lines, and \
         this test; restamp the surface crate's `space_weathering` table `--since 24`"
    );
    assert_eq!(AreaMixWeathering::in_force(), AreaMixWeathering::Held);
    let held = PassWeathering::in_force(|| panic!("nothing of the weathering is computed"));
    assert_eq!(held.err(), Some(NoMaturity::Held));
    assert_eq!(
        with_area_mix_weathering(AreaMixWeathering::Weathered, AreaMixWeathering::in_force),
        AreaMixWeathering::Weathered
    );
    assert_eq!(AreaMixWeathering::in_force(), AreaMixWeathering::Held);
    let sun = sun_bands();
    let moon = SurfaceState::Airless.albedo(SurfaceMaterial::Rock).value();
    let capped = Regolith::with_lid(id("basalt_fe"), SurfaceMaterial::Rock)
        .bond(SurfaceState::Airless, &sun);
    assert_eq!(capped.stand_in, Some(StandIn::SpaceWeathering));
    assert_same_bits(capped.bond, moon);
}

/// The weathering law reads each optics bin at its midpoint, 345 to 4,250 nm, in bin order.
#[test]
fn the_law_reads_each_bin_at_its_midpoint() {
    let expected = [
        345.0, 442.5, 545.0, 647.5, 740.0, 890.0, 1_150.0, 1_500.0, 1_950.0, 2_500.0, 3_150.0,
        4_250.0,
    ];
    for (bin, &nm) in expected.iter().enumerate() {
        assert_same_bits(bin_wavelength_nm(bin), nm);
        assert!(SPECTRAL_BIN_EDGES_NM[bin] < nm && nm < SPECTRAL_BIN_EDGES_NM[bin + 1]);
    }
}

/// A known lid weathered at a maturity (P14.T54.b.e, `Regolith::weathered_lid`): each spherical
/// albedo by bin is R10.T1.d's law's at the bin's midpoint, under the row's law, at its palette
/// entry's grain density and `FeO_T`, to the bit, never above the fresh bin and never capped; at
/// m = 0, at `FeO_T` 0 (an ice) and for `mars_dust`'s observed, weathered albedo the row's own
/// bins to the bit, nothing darkening twice; a row whose `FeO_T` is not modelled (`phyllosilicate`,
/// the chondrites) keeps the stand-in's arm, capped where airless (decision-pre-23-review item
/// 16); the regolith's Bond albedo is its bins weighted by the light, as a solid's. Under 24's
/// rows.
#[test]
fn a_weathered_lid_is_the_law_s_by_bin_at_its_maturity() {
    use crate::planetary::testing::with_every_24_hold;

    with_every_24_hold(|| {
        let sun = sun_bands();
        let basalt_fe = id("basalt_fe");
        let optics = basalt_fe.substance().condensed_optics().unwrap();
        let fresh = optics.spherical_albedo_bins();
        let law = basalt_fe.substance().condensed().unwrap().phase_law();
        let t = kelvin(250.0);
        let density = basalt_fe
            .substance()
            .grain_density(CondensedPhase::Solid, held_at_first_melt(basalt_fe, t))
            .unwrap()
            .value();
        let feo = palette_feo_mass_fraction(basalt_fe).unwrap();
        assert!((feo - 0.202).abs() < 1e-3, "{feo}");
        for m in [0.25, 0.5, 1.0] {
            let regolith =
                Regolith::weathered_lid(basalt_fe, SurfaceMaterial::Rock, airless_at(m, t));
            assert!(regolith.is_weathered());
            let RegolithWeathering::Weathered { spherical_bins } = regolith.weathering else {
                panic!("basalt_fe is weathered");
            };
            for bin in 0..OPTICS_BANDS {
                let law = weathered_spherical_albedo(
                    fresh[bin],
                    bin_wavelength_nm(bin),
                    law,
                    density,
                    feo,
                    m,
                );
                assert_same_bits(spherical_bins[bin], law);
                assert!(spherical_bins[bin] <= fresh[bin]);
            }
            let bond = regolith.bond(SurfaceState::Airless, &sun);
            assert_eq!(bond.stand_in, None, "never capped");
            assert_same_bits(bond.bond, band_weighted(&spherical_bins, &sun));
        }
        let unweathered = |key: &str, material, m: f64| {
            let row = id(key);
            let regolith = Regolith::weathered_lid(row, material, airless_at(m, kelvin(120.0)));
            let fresh = row.substance().condensed_optics().unwrap();
            assert!(regolith.is_weathered(), "{key}");
            let RegolithWeathering::Weathered { spherical_bins } = regolith.weathering else {
                panic!("{key} takes the weathered arm");
            };
            for (a, b) in spherical_bins.iter().zip(fresh.spherical_albedo_bins()) {
                assert_same_bits(*a, b);
            }
        };
        unweathered("basalt_fe", SurfaceMaterial::Rock, 0.0);
        unweathered("H2O", SurfaceMaterial::Rock, 1.0);
        assert_eq!(palette_feo_mass_fraction(id("mars_dust")), None);
        unweathered("mars_dust", SurfaceMaterial::Rock, 1.0);
        // A row whose FeO_T is not modelled keeps the stand-in's arm, capped where airless.
        for key in ["phyllosilicate", "chondrite_ll", "chondrite_ci"] {
            let row = id(key);
            assert_eq!(row.substance().feo_mass_fraction(), None, "{key}");
            let regolith =
                Regolith::weathered_lid(row, SurfaceMaterial::Ice, airless_at(1.0, kelvin(160.0)));
            assert!(!regolith.is_weathered(), "{key}");
            assert_eq!(
                format!("{regolith:?}"),
                format!("{:?}", Regolith::with_lid(row, SurfaceMaterial::Ice)),
                "{key}"
            );
            let airless = regolith.bond(SurfaceState::Airless, &sun);
            assert_eq!(airless.stand_in, Some(StandIn::SpaceWeathering), "{key}");
        }
    });
}

/// decision-p14-t54b-e-findings §A.5's must-fix, every 24 hold in force: an airless `komatiite`
/// lid on a surface 100 yr old, at m ≈ 0, is a lava with no regolith yet and keeps T51.b's cap,
/// 0.11 exactly about a Sun (its fresh powder row lies above it), labelled
/// [`StandIn::UnweatheredLava`], under a tenuous air too; at 2 Myr, and at
/// [`REGOLITH_FORMATION_TIME`] itself, it is weathered and not capped; under air at 100 yr it is
/// weathered, as any lid under air; `basalt_reduced` keeps its own cap's label and an airless
/// `H2O` lid its frost, however young.
#[test]
fn an_airless_silicate_lid_younger_than_a_regolith_keeps_the_cap() {
    use crate::planetary::testing::with_every_24_hold;

    with_every_24_hold(|| {
        let sun = sun_bands();
        let moon = SurfaceState::Airless.albedo(SurfaceMaterial::Rock).value();
        let komatiite = id("komatiite");
        assert!(capped_lid(komatiite, SurfaceState::Airless));
        assert!(
            solid_bond(komatiite, &sun).bond > moon,
            "its fresh row lies above the cap"
        );
        assert_same_bits(REGOLITH_FORMATION_TIME.value(), 1.0e6);
        let at = |age: f64, state| LidWeathering {
            surface_age: Years::new(age),
            ..lid_at(1e-10, kelvin(140.0), state)
        };
        for state in [SurfaceState::Airless, SurfaceState::Tenuous] {
            let lava = Regolith::weathered_lid(komatiite, SurfaceMaterial::Rock, at(100.0, state));
            assert!(!lava.is_weathered(), "{state:?}");
            assert_eq!(lava.weathering, RegolithWeathering::UnweatheredLava);
            let bond = lava.bond(state, &sun);
            assert_eq!(bond.stand_in, Some(StandIn::UnweatheredLava), "{state:?}");
            assert_same_bits(bond.bond, moon);
            // The cold traps' bare regolith reads it airless: the same cap.
            assert_eq!(lava.bond(SurfaceState::Airless, &sun), bond);
        }
        for age in [2.0e6, REGOLITH_FORMATION_TIME.value()] {
            let at_age = at(age, SurfaceState::Airless);
            let regolith = Regolith::weathered_lid(komatiite, SurfaceMaterial::Rock, at_age);
            assert!(regolith.is_weathered(), "{age} yr");
            let bond = regolith.bond(SurfaceState::Airless, &sun);
            assert_eq!(bond.stand_in, None, "{age} yr: weathered, not capped");
            assert!(bond.bond > moon, "{age} yr: {}", bond.bond);
        }
        let under_air = at(100.0, SurfaceState::Temperate);
        assert!(
            Regolith::weathered_lid(komatiite, SurfaceMaterial::Rock, under_air).is_weathered()
        );
        let young = at(100.0, SurfaceState::Airless);
        let reduced = Regolith::weathered_lid(id("basalt_reduced"), SurfaceMaterial::Rock, young);
        assert_eq!(
            reduced.bond(SurfaceState::Airless, &sun).stand_in,
            Some(StandIn::SpaceWeathering)
        );
        let water = id("H2O");
        let frost = Regolith::weathered_lid(water, SurfaceMaterial::Ice, young);
        assert!(frost.is_weathered());
        assert_same_bits(
            frost.bond(SurfaceState::Airless, &sun).bond,
            solid_bond(water, &sun).bond,
        );
        let fresh = solid_bond(komatiite, &sun).bond;
        eprintln!(
            "an airless komatiite lid: {moon:.4} at 100 yr (its fresh row {fresh:.4}), {:.4} at \
             2 Myr, m 1e-10",
            Regolith::weathered_lid(
                komatiite,
                SurfaceMaterial::Rock,
                at(2.0e6, SurfaceState::Airless)
            )
            .bond(SurfaceState::Airless, &sun)
            .bond
        );
    });
}

/// Under the weathered area mix, every 24 hold in force (P14.T54.b.e; decision-p14-t49k Q5):
///
/// - **Mercury**, an airless `basalt_reduced` lid about a Sun at Mercury's maturity (its
///   k ≈ 8.9 over a surface of some Gyr: 1 to the double), keeps the cap's 0.11 as its Bond
///   albedo, within the allowance of 1.25 of Mallama 2017's 0.088 ± 0.003 (arXiv:1703.02670, the
///   spherical bolometric albedo over 0–4 µm, a preprint), 0.0704–0.1138; Domingue et al.'s 2011
///   refereed 0.081 over 0.43–1.01 µm is stated beside it. The band coverage: the optics' twelve
///   bins over 0.3–5 µm, the first and last standing for the light below and above them, so the
///   whole spectrum. Its V-band spherical albedo is stated beside Mallama et al. 2002's `p_V q_V`,
///   0.142 × 0.478 = 0.068, while the cap holds.
/// - **A Vesta-like `basalt_fe` lid** is weathered once at its maturity and never capped as well:
///   its row is 23's, unchanged, and its weathered Bond albedo lies below its fresh row's and is
///   not the cap's.
/// - **Mars** by decision-pre-23-review item 13's terms, its `basalt_fe` lid under its air (m +0)
///   with `ferric` over 0.31 of the land, its ice and cloud aside: stated against 0.20–0.30, not
///   asserted (P14.T49.k rides 24; the row swap and the assertion are P14.T54.b.l's).
#[test]
fn under_the_weathered_area_mix_mercury_keeps_the_cap_and_a_vesta_like_basalt_is_darkened_once() {
    use crate::planetary::testing::with_every_24_hold;

    let held_basalt_fe = solid_bond(id("basalt_fe"), &sun_bands()).bond;
    with_every_24_hold(|| {
        let sun = sun_bands();
        let moon = SurfaceState::Airless.albedo(SurfaceMaterial::Rock).value();
        let reduced = id("basalt_reduced");
        assert_eq!(CAPPED_ROWS, ["basalt_reduced"]);
        let mercury = Regolith::weathered_lid(
            reduced,
            SurfaceMaterial::Rock,
            airless_at(1.0, kelvin(440.0)),
        );
        assert!(!mercury.is_weathered());
        let bond = mercury.bond(SurfaceState::Airless, &sun);
        assert_eq!(bond.stand_in, Some(StandIn::SpaceWeathering));
        assert_same_bits(bond.bond, moon);
        let (mallama, allowance) = (0.088, 1.25);
        assert!(
            (mallama / allowance..=(mallama + 0.003) * allowance).contains(&bond.bond),
            "Mercury's {} against Mallama 2017's 0.088 ± 0.003 within 1.25",
            bond.bond
        );
        let fresh = reduced.substance().condensed_optics().unwrap();
        let fresh_bond = solid_bond(reduced, &sun).bond;
        let v = fresh.spherical_albedo_bins()[2];
        let uncapped = {
            let optics = reduced.substance().condensed_optics().unwrap();
            let law = reduced.substance().condensed().unwrap().phase_law();
            let density = reduced
                .substance()
                .grain_density(CondensedPhase::Solid, kelvin(440.0))
                .unwrap()
                .value();
            let feo = palette_feo_mass_fraction(reduced).unwrap();
            weathered_spherical_albedo(
                optics.spherical_albedo_bins()[2],
                bin_wavelength_nm(2),
                law,
                density,
                feo,
                1.0,
            )
        };
        eprintln!(
            "Mercury (basalt_reduced, airless, about a Sun): Bond {:.4} by the cap (Mallama 2017 \
             0.088 ± 0.003 over 0–4 µm; Domingue et al. 2011 0.081 over 0.43–1.01 µm), over the \
             twelve bins 0.3–5 µm, the first and last standing for the light outside them; its \
             fresh row {fresh_bond:.4}; its V-band spherical albedo fresh {v:.4}, under the cap's \
             scaling {:.4}, and by the law at m = 1 uncapped {uncapped:.4}, beside Mallama et al. \
             2002's p_V q_V 0.068",
            bond.bond,
            v * moon / fresh_bond
        );
        // A Vesta-like basalt_fe lid: weathered once, never capped.
        let basalt_fe = id("basalt_fe");
        assert_same_bits(solid_bond(basalt_fe, &sun).bond, held_basalt_fe);
        let vesta = Regolith::weathered_lid(
            basalt_fe,
            SurfaceMaterial::Rock,
            airless_at(0.9, kelvin(200.0)),
        );
        assert!(vesta.is_weathered());
        let weathered = vesta.bond(SurfaceState::Airless, &sun);
        assert_eq!(weathered.stand_in, None);
        assert!(weathered.bond < held_basalt_fe, "{}", weathered.bond);
        assert!((weathered.bond - moon).abs() > 1e-6, "not the cap's");
        // Mars, its basalt_fe lid under its air (m = +0: its row's own) and ferric over 0.31.
        let mars_lid = Regolith::weathered_lid(
            basalt_fe,
            SurfaceMaterial::Rock,
            lid_at(0.0, kelvin(210.0), SurfaceState::Temperate),
        )
        .bond(SurfaceState::Temperate, &sun)
        .bond;
        let ferric = solid_bond(id("ferric"), &sun).bond;
        let mars = 0.69 * mars_lid + 0.31 * ferric;
        eprintln!(
            "a Vesta-like basalt_fe at m = 0.9: Bond {:.4} against its fresh row's {:.4}, \
             uncapped; Mars by item 13's terms (basalt_fe {mars_lid:.4}, ferric {ferric:.4} over \
             0.31), ice and cloud aside: {mars:.4} against 0.20–0.30, stated",
            weathered.bond, held_basalt_fe
        );
    });
}

/// Under the weathered area mix (every 24 hold in force): an airless rocky body whose lid is not
/// known keeps the Moon's 0.11, labelled (a maturity comes only with a lid, `pass_regolith`), and
/// the table's Moon, derived over the passes' stand-in (its lid not known), keeps it as its Bond
/// albedo to the bit, its passes taking no maturity; Europa's airless `H2O` lid on a rocky bulk, of
/// `FeO_T` 0, keeps its frost to the bit. Its normal albedos in B, V and R, `H2O`'s fresh frost's,
/// are stated against Europa's normal reflectances 0.48–0.85 (Buratti and Veverka 1983, Icarus 55,
/// 93), which they miss above, a recorded miss pending P14.T49.m's icy regolith; its Bond albedo
/// about a Sun is stated beside them, a different quantity, not compared with that range.
#[test]
fn under_the_weathered_area_mix_the_moon_keeps_0_11_and_europa_its_frost() {
    use crate::planetary::derive::atmosphere::{NoMaturity, pass_regolith};
    use crate::planetary::hooks::surface::tests::table;
    use crate::planetary::testing::with_every_24_hold;

    with_every_24_hold(|| {
        let sun = sun_bands();
        let moon = SurfaceState::Airless.albedo(SurfaceMaterial::Rock).value();
        let unknown = pass_regolith(&Regolith::of_material, SurfaceMaterial::Rock, None)
            .bond(SurfaceState::Airless, &sun);
        assert_eq!(unknown.stand_in, Some(StandIn::MoonForAnUnknownAirlessLid));
        assert_same_bits(unknown.bond, moon);
        let table = table();
        let (_, the_moon, _) = table.iter().find(|(name, ..)| *name == "Moon").unwrap();
        assert_same_bits(the_moon.albedo().value(), moon);
        assert_eq!(
            the_moon.weathered().as_ref().err(),
            Some(&NoMaturity::NoRegolith)
        );
        let water = id("H2O");
        let europa =
            Regolith::weathered_lid(water, SurfaceMaterial::Rock, airless_at(0.5, kelvin(102.0)));
        let frost = water.substance().condensed_optics().unwrap();
        let RegolithWeathering::Weathered { spherical_bins } = europa.weathering else {
            panic!("an H2O lid takes the weathered arm");
        };
        for (a, b) in spherical_bins.iter().zip(frost.spherical_albedo_bins()) {
            assert_same_bits(*a, b);
        }
        let bond = europa.bond(SurfaceState::Airless, &sun).bond;
        eprintln!(
            "the Moon (the table's, its lid not known): {:.4}; Europa's H2O lid, FeO_T 0: its \
             normal albedos in B, V and R {:?}, H2O's fresh frost's, against Europa's normal \
             reflectances 0.48–0.85, a miss pending P14.T49.m; its Bond albedo about a Sun \
             {bond:.4}, stated",
            the_moon.albedo().value(),
            frost.normal_albedo_bvr()
        );
    });
}

/// The Moon's recorded miss (decision-p14-t51d-weathering, addendum 2026-10-19, D1–D3;
/// decision-r10-t1d-anchor-colour §3), stated and not asserted: the generated Moon's highlands
/// (`feldspathic`, `FeO_T` 0.051) and maria (`basalt_fe`, 0.202) at m = 1, their normal albedos in
/// B, V and R by R10.T1.d's law (`weathered_entry_bvr`, the palette's), against the plan's
/// 0.094/0.132/0.147 and 0.051/0.054/0.056, the maria's B/R against the anchors' 0.733, and the
/// highland-to-mare V contrast against the measured 1.58; and their weathered Bond albedos about a
/// Sun, the area mix's.
#[test]
fn the_moon_s_recorded_miss_is_stated() {
    use crate::planetary::testing::with_every_24_hold;
    use crate::substance::optics::resolve_palette_entry;
    use hyperion_surface::field::PaletteRole;
    use hyperion_surface::material::weathered_entry_bvr;

    with_every_24_hold(|| {
        let sun = sun_bands();
        let at_one = |key: &str| {
            let row = id(key);
            let mut entry =
                resolve_palette_entry(row, PaletteRole::PrimaryCrust, kelvin(250.0)).unwrap();
            entry.feo_mass_fraction = palette_feo_mass_fraction(row);
            let bvr = weathered_entry_bvr(&entry, 1.0);
            let bond =
                Regolith::weathered_lid(row, SurfaceMaterial::Rock, airless_at(1.0, kelvin(250.0)))
                    .bond(SurfaceState::Airless, &sun)
                    .bond;
            (entry.feo_mass_fraction, bvr, bond)
        };
        let (highland_feo, highlands, highland_bond) = at_one("feldspathic");
        let (mare_feo, maria, mare_bond) = at_one("basalt_fe");
        eprintln!(
            "the Moon's recorded miss at m = 1: highlands (feldspathic, FeO_T {highland_feo:?}) \
             A_N {highlands:.3?} against 0.094/0.132/0.147, Bond {highland_bond:.4}; maria \
             (basalt_fe, FeO_T {mare_feo:?}) A_N {maria:.3?} against 0.051/0.054/0.056, Bond \
             {mare_bond:.4}; the maria's B/R {:.3} against 0.733; the V contrast {:.2} against \
             the measured 1.58",
            maria[0] / maria[2],
            highlands[1] / maria[1]
        );
    });
}

/// The area mix takes a weathering deposit as a further solid at its row's optics (P14.T54.b.e):
/// with no deposit it is the condensates' and the regolith's to the bit; with `ferric` over 0.31
/// it is 0.69 of the regolith and 0.31 of `ferric`'s row, in that order; and areas summing above 1
/// are scaled to it.
#[test]
fn the_area_mix_takes_a_deposit_at_its_row() {
    let sun = HostShade::sun();
    let dry = partitioned(
        (150.0, 210.0, 240.0),
        weights_of(600.0, &[("N2", 0.98), ("Ar", 0.02)]),
        3.72,
    );
    let none = Condensates::default();
    let regolith = 0.137;
    assert_same_bits(surface_bond(&none, &dry, (regolith, &[]), &sun), regolith);
    let ferric = id("ferric");
    let deposit = solid_bond(ferric, &sun.bands).bond;
    let with = surface_bond(&none, &dry, (regolith, &[(ferric, 0.31)]), &sun);
    assert_same_bits(with, (1.0 - 0.31) * regolith + 0.31 * deposit);
    let over = surface_bond(
        &none,
        &dry,
        (regolith, &[(ferric, 0.8), (ferric, 0.4)]),
        &sun,
    );
    assert!((over - deposit).abs() < 1e-12, "{over} against {deposit}");
}
