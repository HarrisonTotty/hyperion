//! The fit's fixture, GJ 1214 b: its bright haze at the fitted optical depth against the brief's
//! targets.

use hyperion_testkit::float::assert_same_bits;

use super::*;
use crate::planetary::chem::HostAbundances;
use crate::planetary::chem::photochem::BRIGHT_HAZE_OPTICAL_DEPTH;
use crate::planetary::testing::{EnvelopeByHand, envelope_by_hand};
use crate::tables::warm_haze::COAGULATED_HAZE_OPTICAL_DEPTH;
use crate::units::{Dex, Watts};

/// The fixture's record envelope by hand at equilibrium temperature `t_eq` (K): its mass, radius,
/// gravity and fractions, protosolar about a host whose snow line lies at 0.15 au (the builder's,
/// labelled), formed where it orbits.
fn by_hand(t_eq: f64) -> crate::planetary::record::Envelope {
    let fixture = GJ_1214_B;
    let inputs = fixture
        .inputs(Kelvin::new(t_eq))
        .expect("the fixture's inputs");
    let mass = EarthMasses::new(fixture.mass_earth);
    let fractions = composition(
        mass,
        EarthRadii::new(fixture.radius_earth),
        SnowLineSide::Inside,
        fixture.flux(),
    )
    .expect("a composition")
    .fractions();
    let radius = inputs.radius.value();
    envelope_by_hand(&EnvelopeByHand {
        mass,
        fractions,
        radius: inputs.radius,
        gravity: MetresPerSecondSquared::new(
            GRAVITATIONAL_CONSTANT * inputs.mass.value() / (radius * radius),
        ),
        equilibrium_temperature: Kelvin::new(t_eq),
        internal_luminosity: Watts::ZERO,
        age: Years::new(fixture.age_years),
        formation_distance: Metres::new(fixture.semi_major_axis_au * METRES_PER_AU),
        snow_line: Metres::new(0.15 * METRES_PER_AU),
        fe_h: Dex::ZERO,
        alpha_fe: Dex::ZERO,
        host: HostAbundances::protosolar(),
        primordial_helium_per_hydrogen: crate::substance::Element::He
            .protosolar_abundance_per_hydrogen(),
        starlit: true,
    })
    .expect("the fixture's envelope")
}

/// The keys of the photochemical hazes' materials among an envelope's modes.
fn hazes_of(envelope: &crate::planetary::record::Envelope) -> Vec<(&'static str, f64)> {
    envelope
        .atmosphere()
        .modes()
        .iter()
        .filter_map(|mode| {
            let key = mode.material().substance().key();
            let key: &'static str = ["soot", "tholin", "S8"]
                .into_iter()
                .find(|k| *k == key.as_str())?;
            Some((key, mode.optical_depth().map_or(f64::NAN, |d| d.value())))
        })
        .collect()
}

/// The fixture's record carries the bright haze alone, its slab and its coagulated mode at their
/// τ(550), whole below 600 K and above 550 K, and its closed form meets
/// the brief's: its Bond albedo within the measured 0.39–0.63 (pinned at the fit's), B/V 0.6–0.7
/// and R/V 1.1–1.6 in its column's spherical albedo, `p_V` 0.15–0.25, its q the bright haze's.
/// The slab sets the visible; the coagulated mode, read in each bin to its own τ of 10, the
/// infrared, where two thirds of a 3,250 K host's light lies (research-leans-52).
#[test]
fn gj_1214_b_draws_its_bright_haze() {
    let light = GJ_1214_B
        .light(COAGULATED_HAZE_OPTICAL_DEPTH)
        .expect("the fixture's column");
    assert!((0.39..=0.63).contains(&light.bond_albedo), "{light:?}");
    let [b, v, r] = light.spherical_albedo;
    assert!((0.6..=0.7).contains(&(b / v)), "{light:?}");
    assert!((1.1..=1.6).contains(&(r / v)), "{light:?}");
    assert!(
        (0.15..=0.25).contains(&light.geometric_albedo[1]),
        "{light:?}"
    );
    for (q, bright) in light.phase_integral.into_iter().zip([
        crate::planetary::derive::photometry::BRIGHT_HAZE_PHASE_INTEGRAL.b,
        crate::planetary::derive::photometry::BRIGHT_HAZE_PHASE_INTEGRAL.v,
        crate::planetary::derive::photometry::BRIGHT_HAZE_PHASE_INTEGRAL.r,
    ]) {
        assert_same_bits(q, bright);
    }
    assert!((light.bond_albedo - 0.5100).abs() < 5e-4, "{light:?}");
    // By hand the given T_eq is the zero-albedo one, which sets the hazes' shares.
    let envelope = by_hand(light.zero_albedo_temperature_k);
    let hazes = hazes_of(&envelope);
    assert_eq!(hazes.len(), 2, "{hazes:?}");
    for ((key, tau), want) in hazes
        .iter()
        .zip([BRIGHT_HAZE_OPTICAL_DEPTH, COAGULATED_HAZE_OPTICAL_DEPTH])
    {
        assert_eq!(*key, "tholin");
        assert!((tau / want - 1.0).abs() < 1e-9, "{hazes:?}");
    }
}

/// The fixture's flux is its host's black body at its distance, and its zero-albedo `T_eq` the
/// planet's 595 K (Cloutier et al. 2021, recalled: 596 ± 19 K).
#[test]
fn the_fixture_s_flux_is_its_host_s() {
    let t0 = equilibrium_temperature(GJ_1214_B.flux(), BondAlbedo::from_fraction(0.0)).value();
    let expected = GJ_1214_B.host_temperature_k
        * (GJ_1214_B.host_radius_solar * SOLAR_RADIUS_M
            / (2.0 * GJ_1214_B.semi_major_axis_au * METRES_PER_AU))
            .sqrt();
    assert!(
        (t0 / expected - 1.0).abs() < 1e-12,
        "{t0} against {expected}"
    );
    assert!((t0 - 595.3).abs() < 0.1, "{t0}");
}
