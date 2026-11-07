//! The surface colour's area mix by band (from generator version 31; `photometry`'s module
//! documentation, "The colour from the body's own surface"): a band's light alone, the regolith's
//! and the cloud's albedos in a band, and whether space weathering's reddening reaches the airless
//! silicate lids.

use std::fmt::Write as _;

use hyperion_testkit::float::assert_same_bits;

use super::tests::{id, kelvin};
use super::*;

/// The Moon's colour, its albedo over its albedo at 550 nm, (λ ÷ 550 nm)^0.83 at 440 and 640 nm:
/// Dollfus and Bowell 1971's whole-Moon law (A&A 10, 29, eq. 11, p. 50), the colour R10.T1.d's
/// anchors take (decision-r10-t1d-anchor-colour §2).
fn moon_colour() -> (f64, f64) {
    let law = |nm: f64| math::powf(nm / 550.0, 0.83);
    (law(440.0), law(640.0))
}

/// What a pass on an old airless surface gives its lid's weathering at the maturity `m`.
fn airless_at(m: f64) -> LidWeathering {
    LidWeathering {
        maturity: m,
        temperature: kelvin(250.0),
        surface_age: Years::new(4.5e9),
        state: SurfaceState::Airless,
    }
}

/// A band's light alone makes the area mix's Bond albedo the bin's: a solid's spherical albedo in
/// the bin to the bit, a liquid's the mean of its bins over the band, weighted by their overlap,
/// whose weights sum to the band's width; the rest of the light's values are the shade's own.
#[test]
fn a_band_s_light_alone_gives_each_part_its_albedo_in_the_bin() {
    let sun = HostShade::sun();
    for key in ["basalt", "basalt_fe", "H2O", "ferric", "tholin"] {
        let row = id(key);
        let bins = row
            .substance()
            .condensed_optics()
            .unwrap()
            .spherical_albedo_bins();
        for band in ColourBand::ALL {
            let alone = sun.in_band(band);
            assert_same_bits(solid_bond(row, alone.bands()).bond, bins[band.bin()]);
            assert_same_bits(alone.water_cloud, sun.water_cloud);
        }
    }
    let ocean = liquid_record(water(), kelvin(288.0), Pascals::ZERO)
        .in_range()
        .expect("water's record at 288 K");
    for (band, width) in ColourBand::ALL.into_iter().zip([105.0, 100.0, 105.0]) {
        let weights = band.liquid_weights();
        let total = weights.iter().fold(0.0, |sum, &w| sum + w);
        assert!((total - width).abs() < 1e-9, "{band:?}: {total}");
        assert_same_bits(weights[0], 0.0);
        assert_same_bits(weights[LIQUID_WEIGHTS - 1], 0.0);
        let (mut sum, mut by) = (0.0, 0.0);
        for (k, &w) in weights[1..=LIQUID_BINS].iter().enumerate() {
            let albedo = liquid_bin_albedo(ocean.refractive_index[k], BodyTerm::of(&ocean, k));
            sum += w * albedo;
            by += w;
        }
        let banded = liquid_bond(&ocean, &sun.in_band(band).bins);
        assert!((banded - sum / by).abs() < 1e-15, "{band:?}: {banded}");
    }
    assert_same_bits(ColourBand::V.analogue_colour(PhaseTemplate::Mercury), 1.0);
}

/// The regolith's albedo in a band is its Bond albedo's in the bin, but where a stand-in gives one
/// for every bin: the space-weathering cap takes Mercury's measured colour at the cap, so that its
/// V is the cap; a lava younger than a regolith its fresh row's colour at the cap; an unknown
/// airless rocky lid the Moon's 0.11 in Mercury's colour; a weathered lid its weathered bins; a
/// lid under air, or one darker than the cap, its own row's bins.
#[test]
fn the_regolith_s_band_albedo_is_its_bond_s_in_the_bin() {
    let sun = HostShade::sun();
    let (rock, airless) = (SurfaceMaterial::Rock, SurfaceState::Airless);
    let cap = moon_bond();
    let mercury = |band: ColourBand| band.analogue_colour(PhaseTemplate::Mercury);
    let basalt_fe = id("basalt_fe");
    let bins = basalt_fe
        .substance()
        .condensed_optics()
        .unwrap()
        .spherical_albedo_bins();
    let capped = Regolith::with_lid(basalt_fe, rock);
    let unknown = Regolith::of_material(rock);
    let weathered = Regolith::weathered_lid(basalt_fe, rock, airless_at(0.3));
    let RegolithWeathering::Weathered { spherical_bins } = weathered.weathering else {
        panic!("basalt_fe weathers");
    };
    let lava = Regolith::weathered_lid(
        basalt_fe,
        rock,
        LidWeathering {
            surface_age: Years::new(1.0e5),
            ..airless_at(0.3)
        },
    );
    assert_eq!(lava.weathering, RegolithWeathering::UnweatheredLava);
    let fresh_bond = solid_bond(basalt_fe, &sun.bands).bond;
    assert!(fresh_bond > cap, "{fresh_bond}");
    for band in ColourBand::ALL {
        let bin = band.bin();
        assert_same_bits(capped.band_albedo(airless, &sun, band), cap * mercury(band));
        assert_same_bits(
            unknown.band_albedo(airless, &sun, band),
            cap * mercury(band),
        );
        assert_same_bits(
            weathered.band_albedo(airless, &sun, band),
            spherical_bins[bin],
        );
        assert_same_bits(
            lava.band_albedo(airless, &sun, band),
            bins[bin] * cap / fresh_bond,
        );
        assert_same_bits(
            capped.band_albedo(SurfaceState::Temperate, &sun, band),
            bins[bin],
        );
    }
    assert_same_bits(capped.band_albedo(airless, &sun, ColourBand::V), cap);
    // basalt's fresh row lies below the cap, which then does not act: its own bins.
    let basalt = id("basalt");
    let own = basalt
        .substance()
        .condensed_optics()
        .unwrap()
        .spherical_albedo_bins();
    assert!(solid_bond(basalt, &sun.bands).bond < cap);
    for band in ColourBand::ALL {
        assert_same_bits(
            Regolith::with_lid(basalt, rock).band_albedo(airless, &sun, band),
            own[band.bin()],
        );
    }
}

/// An ice regolith's band albedo: under the icy regolith (from generator version 27) an airless
/// body's is its rock's lag in the bin, and under air its row by material's, `H2O`'s frost.
#[test]
fn an_ice_regolith_takes_its_lag_s_or_its_frost_s_bin() {
    let sun = HostShade::sun();
    let ice = Regolith::of_material(SurfaceMaterial::Ice);
    let lag = icy_lag_bins();
    let frost = water()
        .substance()
        .condensed_optics()
        .unwrap()
        .spherical_albedo_bins();
    assert_eq!(IcyRegolithRules::in_force(), IcyRegolithRules::Mixture);
    for band in ColourBand::ALL {
        assert_same_bits(
            ice.band_albedo(SurfaceState::Airless, &sun, band),
            lag[band.bin()],
        );
        assert_same_bits(
            ice.band_albedo(SurfaceState::Temperate, &sun, band),
            frost[band.bin()],
        );
    }
}

/// Venus's deck is Venus's measured colour at its 0.76, the water cloud white, and an airless
/// body has none.
#[test]
fn the_deck_is_venus_s_colour_and_the_water_cloud_white() {
    let sun = HostShade::sun();
    let deck = (SurfaceState::RunawayGreenhouse, RunawayDeck::Kept);
    let water = (SurfaceState::Temperate, RunawayDeck::Kept);
    let venus = PhaseTemplate::Venus.row().albedo;
    for band in ColourBand::ALL {
        assert_same_bits(
            cloud_band_albedo(deck, &sun, band),
            cloud_albedo(deck, &sun) * band.analogue_colour(PhaseTemplate::Venus),
        );
        assert_same_bits(cloud_band_albedo(water, &sun, band), sun.water_cloud);
        assert_same_bits(
            cloud_band_albedo((SurfaceState::Airless, RunawayDeck::Kept), &sun, band),
            0.0,
        );
    }
    let b =
        cloud_band_albedo(deck, &sun, ColourBand::B) / cloud_band_albedo(deck, &sun, ColourBand::V);
    assert!((b - venus.b / venus.v).abs() < 1e-15);
}

/// The brief's check that space weathering's reddening is applied where it should be, now that the
/// disc reads it: R10.T1.d's law reddens every airless silicate lid (B ÷ V below and R ÷ V above
/// its fresh row's at m = 0.3, and B below V below R at every maturity), and fresh basalt is near
/// grey, its R ÷ V − B ÷ V below the Moon's (Dollfus and Bowell's 0.30). At m = 1 the law's
/// terrain colour spread shows (decision-r10-t1d-anchor-colour, D1–D2: the form's dark end and the
/// stand-in fresh rows): mature Fe-rich rows greyer (B ÷ V about 0.95) and `feldspathic` redder
/// (about 0.71) than the Moon's mean 0.83 (Dollfus and Bowell 1971, eq. 11), which Gehrels et al.
/// 1964 (Table XIII) find the maria share within 1%; stated, not asserted.
#[test]
fn space_weathering_reddens_the_airless_silicate_lids() {
    let (moon_b, moon_r) = moon_colour();
    let ratio = |a: &[f64; OPTICS_BANDS]| {
        let v = a[ColourBand::V.bin()];
        (a[ColourBand::B.bin()] / v, a[ColourBand::R.bin()] / v)
    };
    let basalt = id("basalt")
        .substance()
        .condensed_optics()
        .unwrap()
        .spherical_albedo_bins();
    let (b, r) = ratio(&basalt);
    assert!(
        r - b < moon_r - moon_b,
        "fresh basalt: B/V {b:.3}, R/V {r:.3}"
    );
    for key in [
        "basalt",
        "basalt_fe",
        "ultramafic",
        "komatiite",
        "feldspathic",
    ] {
        let row = id(key);
        let fresh = ratio(
            &row.substance()
                .condensed_optics()
                .unwrap()
                .spherical_albedo_bins(),
        );
        let mut line = format!("{key}: fresh B/V {:.3} R/V {:.3}", fresh.0, fresh.1);
        for m in [0.1, 0.3, 0.6, 1.0] {
            let RegolithWeathering::Weathered { spherical_bins } =
                Regolith::weathered_lid(row, SurfaceMaterial::Rock, airless_at(m)).weathering
            else {
                panic!("{key} weathers");
            };
            let (b, r) = ratio(&spherical_bins);
            assert!(b < 1.0 && r > 1.0, "{key} at m = {m}: B/V {b}, R/V {r}");
            if (m - 0.3).abs() < 1e-12 {
                assert!(
                    b < fresh.0 && r > fresh.1,
                    "{key} at m = 0.3: B/V {b}, R/V {r}"
                );
            }
            write!(line, "; m {m}: {b:.3} {r:.3}").unwrap();
        }
        eprintln!("{line}; the Moon's {moon_b:.3} {moon_r:.3}");
    }
}
