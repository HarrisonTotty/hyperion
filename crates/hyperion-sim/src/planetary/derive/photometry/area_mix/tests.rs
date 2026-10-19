//! P14.T51.b's area-mix Bond albedo: its pieces against their integrals and sources, the band
//! weighting and the hosts, Sudarsky, Burrows and Pinto's classes, the stand-ins and their labels,
//! the cloud, and the reference Earth's calibration. Every row's albedo is read from the registry,
//! never copied (main, 2026-10-16).

use hyperion_testkit::float::assert_same_bits;

use super::*;
use crate::planetary::chem::partition::{
    ClimateSpan, ColumnInventory, Cover, Ground, WaterCover, partition,
};
use crate::substance::diagram::PhaseDiagram;
use crate::units::{KilogramsPerCubicMetre, MetresPerSecondSquared};

fn id(key: &str) -> SubstanceId {
    by_key(key).unwrap_or_else(|| panic!("{key} is a row"))
}

fn kelvin(t: f64) -> Kelvin {
    Kelvin::new(t)
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
    for (x, y) in a.bands.iter().zip(&b.bands) {
        assert_same_bits(*x, *y);
    }
    for (x, y) in a.bins.iter().zip(&b.bins) {
        assert_same_bits(*x, *y);
    }
    for (x, y) in a.giants.iter().zip(&b.giants) {
        assert_same_bits(*x, *y);
    }
    assert_same_bits(a.water_cloud, b.water_cloud);
    assert_same_bits(a.cloud_column.value(), b.cloud_column.value());
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
/// alone.
#[test]
fn water_s_sea_is_its_diffuse_reflectance_and_a_little_of_its_body() {
    let sea = liquid_properties(id("H2O"), kelvin(288.0), Composition::Reference).unwrap();
    let sun = HostShade::sun();
    let bond = liquid_bond(&sea, &sun.bins);
    assert!((0.066..0.09).contains(&bond), "{bond}");
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
        ..sea
    };
    let surface = liquid_bond(&bare, &sun.bins);
    let by_bin: Vec<f64> = sea
        .refractive_index
        .iter()
        .map(|&n| diffuse_fresnel_reflectance(n))
        .collect();
    assert_same_bits(surface, band_weighted(&by_bin, &sun.bins));
    assert!(bond > surface, "{bond} {surface}");
    eprintln!("water's sea at 288 K about a Sun: {bond:.5} ({surface:.5} at its surface alone)");
}

/// The Gauss–Legendre rule integrates every monomial to degree 15, and every band's and bin's share
/// of a black body's light from 2,000 to 40,000 K within 10⁻¹² of a 20,000-interval Simpson sum in
/// x; a Sun sends 0.447 of its light into B, V and R.
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
        let f = |x: f64| x * x * x / math::exp_m1(x);
        let inner = (1..intervals).fold(0.0, |sum, k| {
            let weight = if k % 2 == 1 { 4.0 } else { 2.0 };
            sum + weight * f(low + f64::from(k) * h)
        });
        let pi_squared = core::f64::consts::PI * core::f64::consts::PI;
        15.0 / (pi_squared * pi_squared) * (f(low) + inner + f(high)) * h / 3.0
    };
    let mut worst = 0.0_f64;
    for t in [
        2_000.0, 2_500.0, 3_200.0, 4_620.0, 5_772.0, 7_500.0, 10_000.0, 20_000.0, 40_000.0,
    ] {
        for edges in [&VISIBLE_BAND_EDGES_NM[..], &LIQUID_BIN_EDGES_NM[..]] {
            for pair in edges.windows(2) {
                let rule = planck_share(kelvin(t), pair[0], pair[1]);
                let fine = simpson(t, pair[0], pair[1]);
                worst = worst.max((rule / fine - 1.0).abs());
            }
        }
    }
    assert!(worst < 1e-12, "{worst}");
    let sun: [f64; OPTICS_BANDS] = planck_shares(kelvin(5_772.0), &VISIBLE_BAND_EDGES_NM);
    let visible = sun.iter().sum::<f64>();
    assert!((visible - 0.447).abs() < 0.001, "{visible}");
    assert_same_bits(planck_share(kelvin(0.0), 380.0, 760.0), 0.0);
    eprintln!("the Planck rule's worst relative error: {worst:.1e}; a Sun's B, V, R: {sun:?}");
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
    let visible: [f64; 3] = planck_shares(sun, &VISIBLE_BAND_EDGES_NM);
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

/// Every one of Sudarsky, Burrows and Pinto's Fiducial albedos is reproduced at its tabled host and
/// its class's model planet; the hosts interpolate linearly and clamp beyond M4V and A8V; the
/// planets clamp below class I's 130 K and above class V's 1,700 K.
#[test]
fn every_sudarsky_node_is_reproduced_at_each_tabled_host() {
    for (row, &host) in SUDARSKY_HOSTS_K.iter().enumerate() {
        let shade = HostShade::of_host(kelvin(host));
        for (class, &planet) in SUDARSKY_PLANETS_K.iter().enumerate() {
            let bond = giant_bond(&shade, kelvin(planet));
            let table = SUDARSKY_BOND[row][class];
            assert!(
                (bond - table).abs() < 1e-12,
                "{host} K, class {class}: {bond}"
            );
        }
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
    assert!((giant_bond(&g2, kelvin(50.0)) - 0.57).abs() < 1e-12);
    assert!((giant_bond(&g2, kelvin(3_000.0)) - 0.55).abs() < 1e-12);
    assert!((class_i_bond(kelvin(5_772.0)) - 0.570_085).abs() < 1e-6);
}

/// A 1,200 K giant about a G2V star takes class IV's 0.03, between IV's and V's inclusive; a 1,500
/// K one lies strictly between 0.03 and 0.55; the albedo is continuous in the temperature.
#[test]
fn a_hot_giant_lies_between_its_classes() {
    let g2 = HostShade::of_host(kelvin(5_770.0));
    let at_1200 = giant_bond(&g2, kelvin(1_200.0));
    assert!((at_1200 - 0.03).abs() < 1e-12, "{at_1200}");
    assert!((0.03..=0.55).contains(&at_1200));
    let at_1500 = giant_bond(&g2, kelvin(1_500.0));
    assert!(at_1500 > 0.03 && at_1500 < 0.55, "{at_1500}");
    for node in SUDARSKY_PLANETS_K {
        let (below, above) = (
            giant_bond(&g2, kelvin(node - 1e-9)),
            giant_bond(&g2, kelvin(node + 1e-9)),
        );
        assert!((above - below).abs() < 1e-9, "{node} K");
    }
}

/// A solid's Bond albedo is its spherical albedo by band weighted by the light, never its normal
/// albedo; a row without condensed optics takes water frost's, labelled; every stand-in has a label.
#[test]
fn a_solid_without_optics_takes_water_frost_s_labelled() {
    let sun = sun_bands();
    let water = solid_bond(id("H2O"), &sun);
    assert_eq!(water.stand_in, None);
    let optics = id("H2O").substance().condensed_optics().unwrap();
    assert_same_bits(
        water.bond,
        band_weighted(&optics.spherical_albedo_bvr(), &sun),
    );
    let normal = band_weighted(&optics.normal_albedo_bvr(), &sun);
    assert!(
        (water.bond - normal).abs() > 0.01,
        "{} {normal}",
        water.bond
    );
    for key in ["feldspathic", "mars_dust", "graphite", "SO2"] {
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
    let weathered = Regolith::weathered(SurfaceMaterial::Rock, [0.06, 0.08, 0.10]);
    assert_eq!(weathered.row, None);
    let weighed = weathered.bond(SurfaceState::Airless, &sun);
    assert_eq!(weighed.stand_in, None);
    assert_same_bits(weighed.bond, band_weighted(&[0.06, 0.08, 0.10], &sun));
    eprintln!(
        "basalt about a Sun: {basalt:.5}; an airless lid of basalt, by the stand-in, {:.5}; an \
         airless lid not known {:.5}",
        airless.bond, unknown.bond
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
/// partition fixture's: `a_mars_with_its_caps_takes_almost_no_cloud`.)
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
    let temperate = SurfaceState::Temperate;
    assert_same_bits(
        cloud_fraction(temperate, &earth, &sun),
        SurfaceState::Temperate.cloud_fraction(),
    );
    assert_same_bits(
        cloud_fraction(SurfaceState::RunawayGreenhouse, &earth, &sun),
        1.0,
    );
    for state in [SurfaceState::Airless, SurfaceState::MagmaOcean] {
        assert_same_bits(cloud_fraction(state, &earth, &sun), 0.0);
    }
    let mut dry = weights_of(600.0, &[("N2", 0.98), ("Ar", 0.02)]);
    dry.push((id("H2O"), Pascals::new(2e4)));
    let dry = partitioned((150.0, 210.0, 240.0), dry, 3.72);
    let thin = cloud_fraction(temperate, &dry, &sun);
    assert!(thin < 0.02, "{thin}");
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

/// A solution without a record of its own takes its solvent's: Titan's seas, `ch4_c2h6_n2`, take
/// methane's record at the temperature their condensate lies at; a liquid without a record, a
/// 10-bar carbon dioxide world's sea, takes `r_d` at liquid nitrogen's 550 nm index; each labelled.
#[test]
fn a_solution_takes_its_solvent_s_record_and_an_unrecorded_liquid_nitrogen_s_index() {
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
    assert_eq!(solved.stand_in, Some(StandIn::SolventForASolution));
    let lies_at = titan.condensate(id("CH4")).unwrap().lies_at();
    let methane = liquid_record(id("CH4"), lies_at).in_range().unwrap();
    assert_same_bits(solved.bond, liquid_bond(&methane, &sun.bins));
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
    let carbon_dioxide = published.entry(id("CO2"), CondensatePhase::Liquid).unwrap();
    let stood = liquid_entry_bond(carbon_dioxide, &sea, &sun.bins);
    assert_eq!(stood.stand_in, Some(StandIn::IndexForAnUnrecordedLiquid));
    assert!((0.04..0.05).contains(&stood.bond), "{}", stood.bond);
    let boiling = nitrogen_normal_boiling_point();
    assert!((boiling.value() - 77.355).abs() < 0.01, "{boiling:?}");
    let nitrogen = liquid_record(id("N2"), boiling).in_range().unwrap();
    let index = nitrogen.refractive_index_at(STAND_IN_INDEX_WAVELENGTH_NM);
    assert_same_bits(stood.bond, diffuse_fresnel_reflectance(index));
    eprintln!(
        "Titan's seas by methane's record: {:.5}; a CO2 sea by nitrogen's index {index:.5} at its \
         normal boiling point on the registry's curve, {:.4} K: {:.5}",
        solved.bond,
        boiling.value(),
        stood.bond
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
            let held = liquid_record(id(key), kelvin(below));
            assert!(
                matches!(held, LiquidRecord::InRange(_)),
                "{key} at {below} K"
            );
            same(&held, &at_lowest);
        }
        let nan = liquid_record(id(key), kelvin(f64::NAN));
        assert!(matches!(nan, LiquidRecord::AtFirstMelt(_)), "{key}");
        same(&nan, &at_lowest);
        let critical = row.phase_diagram().map(PhaseDiagram::critical_temperature);
        if let Some(critical) = critical {
            let near = record(key, critical.value() * (1.0 - 1e-6)).unwrap();
            for above in [critical.value(), critical.value() + 50.0, f64::INFINITY] {
                let held = liquid_record(id(key), kelvin(above));
                assert!(
                    matches!(held, LiquidRecord::InRange(_)),
                    "{key} at {above} K"
                );
                same(&held, &near);
            }
        } else {
            let infinite = liquid_record(id(key), kelvin(f64::INFINITY));
            assert!(matches!(infinite, LiquidRecord::AtFirstMelt(_)), "{key}");
            same(&infinite, &at_lowest);
            beyond_no_critical_point += 1;
        }
    }
    eprintln!("liquids with a record and no critical point: {beyond_no_critical_point}");
    let inside = record("H2O", 288.0).unwrap();
    same(&liquid_record(id("H2O"), kelvin(288.0)), &inside);
    for (key, t) in [("CO2", 250.0), ("basalt", 1_500.0), ("CO2", f64::NAN)] {
        let none = liquid_record(id(key), kelvin(t));
        assert!(matches!(none, LiquidRecord::Unrecorded), "{key} at {t} K");
        assert!(none.in_range().is_none(), "{key} at {t} K");
    }
}

/// An Enceladus, the airless icy regolith's fresh water frost about a Sun (not a derived moon),
/// takes `H2O`'s frost by its material, labelled, at a finite albedo in [0, 1). Near 0.8 (Howett
/// et al. 2010, from memory; Li et al. 2023's 0.76 ± 0.03 through a secondary; both read at
/// P14.T49.i's build) is asserted from P14.T49.i's commit, which adds the infrared bands its frost
/// needs (decision-p14-t49c-split §1.3); until then its visible-only albedo is recorded.
#[test]
fn an_enceladus_takes_its_fresh_frost_until_the_infrared() {
    let sun = HostShade::sun();
    let frost = Regolith::of_material(SurfaceMaterial::Ice).bond(SurfaceState::Airless, &sun.bands);
    assert_eq!(frost.stand_in, Some(StandIn::RowByMaterial));
    assert!(
        frost.bond.is_finite() && (0.0..1.0).contains(&frost.bond),
        "{}",
        frost.bond
    );
    assert_same_bits(frost.bond, solid_bond(id("H2O"), &sun.bands).bond);
    eprintln!(
        "Enceladus by its fresh frost, over {OPTICS_BANDS} bands: {:.5}",
        frost.bond
    );
    if OPTICS_BANDS > 3 {
        assert!((0.7..=0.9).contains(&frost.bond), "{}", frost.bond);
    }
}
