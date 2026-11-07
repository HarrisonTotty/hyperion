//! The passes' surface mix: P14.T24.i's unlit mean, and the stand-in's cover.

use hyperion_testkit::float::assert_same_bits;

/// A Earth-mass body about a dark host, which has no light (P14.T24.i): derived over the passes'
/// stand-in, whose climate reads no heat flow.
fn dark_body() -> crate::planetary::derive::DerivedBody {
    use crate::planetary::derive::solar::{SOLAR_AGE, orbit, solar_disc};
    use crate::planetary::derive::{BodyHosts, HostLight, PlacedBody, derive_body};
    use crate::stellar::Composition;
    use crate::stellar::draws::UnitUniform;
    use crate::time::UniverseTime;
    use crate::units::consts::SOLAR_MASS_KG;
    use crate::units::{EarthMasses, Kelvin, Kilograms, SolarLuminosities, SolarRadii};

    let disc = solar_disc();
    let dark = [HostLight::new(SolarLuminosities::ZERO, Kelvin::ZERO, SolarRadii::ZERO).unwrap()];
    let hosts = BodyHosts::new(
        Kilograms::new(SOLAR_MASS_KG),
        Composition::SOLAR,
        &dark,
        &[],
    )
    .unwrap();
    let o = orbit(1.0, 0.0);
    let placed = PlacedBody::new(
        EarthMasses::new(1.0),
        o,
        o.semi_major_axis(),
        UnitUniform::HALF,
    )
    .unwrap();
    derive_body(&placed, &hosts, &disc, SOLAR_AGE, UniverseTime::EPOCH).unwrap()
}

/// P14.T24.i and its follow-up (decision-t24i-airless-state D1.1, D1.3): an unlit airless body's
/// mean surface temperature is its night floor from 24, where version 23 published its equilibrium
/// temperature, 0 K: the passes' stand-in, which reads no heat flow, gives the cosmic background's
/// 2.725 K, the body airless.
#[test]
fn an_unlit_airless_body_takes_its_night_floor() {
    use crate::planetary::derive::atmosphere::SurfaceState;
    use crate::planetary::hooks::surface::COSMIC_BACKGROUND_TEMPERATURE;

    let floored = dark_body();
    assert_eq!(floored.atmosphere().state(), SurfaceState::Airless);
    assert_same_bits(
        floored.atmosphere().surface_temperature().value(),
        COSMIC_BACKGROUND_TEMPERATURE.value(),
    );
    assert_same_bits(floored.equilibrium_temperature().value(), 0.0);
}

/// P14.T53.b part 1 (a) at the passes' stand-in cover (the batch review's fixes): the stand-in's
/// partition (`given_partition`) gives its water's cover (`pass_water_cover`) the state the air's
/// water follows. Over [`past_only_runaways`]' cases:
///
/// - each past-only runaway, published `Temperate` or `Snowball`, holds no surface water and no
///   ice in its pass's cover, and its published condensates no `H2O`;
/// - over its control's own partition and climate a runaway's water state alone takes the water
///   away: no surface water, no potential ocean and no ice, where the control, whose water is its
///   own, holds its surface water and, as a `Snowball`, its uncapped cold area as ice; so the
///   stand-in's partition of the control's air under a runaway's water publishes no `H2O`, where
///   the control's publishes its liquid.
///
/// [`past_only_runaways`]: crate::planetary::derive::atmosphere::tests::past_only_runaways
#[test]
fn under_every_24_hold_a_past_only_runaway_s_stand_in_cover_holds_no_water_or_ice() {
    use super::super::{Atmosphere, ClimateOf, Gas, SurfaceState};
    use super::{PassLight, PassSurface, given_partition, isothermal_climate, pass_water_cover};
    use crate::planetary::chem::partition::{CondensatePhase, Ground, partition};
    use crate::planetary::derive::atmosphere::tests::past_only_runaways;
    use crate::units::MetresPerSecondSquared;
    use crate::units::consts::GRAVITATIONAL_CONSTANT;
    let surface = PassSurface::stand_in(PassLight::HottestHost);
    let climate_of: &ClimateOf<'_> = &isothermal_climate;
    let water = Gas::Water.substance();
    for case in past_only_runaways() {
        let (m, r) = (case.inputs.mass.value(), case.inputs.radius.value());
        let gravity = MetresPerSecondSquared::new(GRAVITATIONAL_CONSTANT * m / (r * r));
        // The cover of the partition of `air`'s column at its climate, under `water_state`.
        let cover_of = |air: &Atmosphere, water_state: SurfaceState| {
            let climate = climate_of(air.surface_temperature, &air.partials);
            let ground = Ground::new(gravity, surface.basins, surface.water_at_surface);
            let solved = partition(&climate, &ground, &air.column_inventory());
            pass_water_cover(
                &solved,
                (air.state, water_state, case.inputs.material),
                &surface,
                &climate,
            )
        };
        let published = |air: &Atmosphere| {
            given_partition(air, &case.inputs, &surface, (climate_of, gravity)).1
        };
        let h2o = |air: &Atmosphere| {
            let condensates = published(air);
            let area = |phase| {
                condensates
                    .entry(water, phase)
                    .map(crate::planetary::chem::partition::CondensateEntry::area_fraction)
            };
            (area(CondensatePhase::Solid), area(CondensatePhase::Liquid))
        };
        let now = cover_of(&case.air, case.air.water_state);
        let gated = cover_of(&case.control, SurfaceState::RunawayGreenhouse);
        let control = cover_of(&case.control, case.control.water_state);
        let dried = Atmosphere {
            water_state: SurfaceState::RunawayGreenhouse,
            ..case.control
        };
        eprintln!(
            "a past-only runaway published {:?}: its cover {now:?}, its H2O {:?}; over its \
             control's partition under a runaway's water {gated:?}, its H2O {:?}; the \
             control's {control:?}, its H2O {:?}",
            case.published,
            h2o(&case.air),
            h2o(&dried),
            h2o(&case.control),
        );
        for cover in [&now, &gated] {
            assert_same_bits(cover.surface_water.value(), 0.0);
            assert_same_bits(cover.potential_ocean, 0.0);
            assert_same_bits(cover.ice_fraction, 0.0);
        }
        assert_eq!(h2o(&case.air), (None, None), "{:?}", case.published);
        assert!(control.surface_water.value() > 0.0, "{control:?}");
        // The control publishes its water (a liquid entry, covering no area in the stand-in,
        // which knows no basins); under a runaway's water the same air publishes none.
        assert!(h2o(&case.control).1.is_some(), "{:?}", case.published);
        assert_eq!(h2o(&dried), (None, None), "{:?}", case.published);
        if case.published == SurfaceState::Snowball {
            assert_same_bits(control.ice_fraction, 1.0);
        }
    }
}

/// The surface colour (from generator version 30): each band's disc lies under its air's layer in
/// that band's light alone. Earth's clear air over a grey ground of 0.1 is bluer in B than in V and
/// in V than in R, each the conservative layer in its bin added over the ground, to the bit; a
/// Titan haze over it reddens it, its B below its R; with no air the disc is its own in every band.
#[test]
fn each_band_s_disc_lies_under_its_own_layer() {
    use super::{Disc, added, by_band};
    use crate::planetary::derive::atmosphere::PartialPressures;
    use crate::planetary::derive::photometry::area_mix::{
        self, ColourBand, HostShade, reference_earth_air,
    };
    use crate::planetary::hooks::aerosols::TITAN_HAZE_OPTICAL_DEPTH;
    use crate::units::MetresPerSecondSquared;

    let sun = HostShade::sun();
    let air = reference_earth_air();
    let gravity = MetresPerSecondSquared::new(9.82);
    let disc = Disc::Clouded {
        ground: 0.1,
        cloud_fraction: 0.0,
        cloud_albedo: 0.0,
    };
    let clear = by_band(|band| added(disc, &air, gravity, (&sun.in_band(band), 0.0)));
    assert!(
        clear.b > clear.v && clear.v > clear.r && clear.r > 0.1,
        "{clear:?}"
    );
    let layers = area_mix::rayleigh_bins(&air, gravity).expect("Earth's air has a layer");
    assert_same_bits(
        clear.v,
        area_mix::rayleigh_added(layers[ColourBand::V.bin()], 0.1),
    );
    let hazy = by_band(|band| {
        added(
            disc,
            &air,
            gravity,
            (&sun.in_band(band), TITAN_HAZE_OPTICAL_DEPTH),
        )
    });
    assert!(hazy.b < hazy.r, "{hazy:?}");
    let none = by_band(|band| {
        added(
            disc,
            &PartialPressures::default(),
            gravity,
            (&sun.in_band(band), 0.0),
        )
    });
    for x in [none.b, none.v, none.r] {
        assert_same_bits(x, 0.1);
    }
}
