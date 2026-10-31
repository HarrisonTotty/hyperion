use super::*;
use hyperion_testkit::float::assert_same_bits;

/// A lag lies only over an ice bulk whose lid is known and not `H2O`, exactly where
/// `CrustWater::of_record` leaves the crust's water unlisted on an ice bulk, and its class is the
/// lid's: `cometary` a comet's, `chondrite_ci` and `chondrite_cm` carbonaceous, every other rock
/// Ceres's kind.
#[test]
fn a_lag_lies_over_an_ice_bulk_under_any_lid_but_water() {
    use crate::planetary::chem::partition::CrustWater;

    for lid in CrustLithology::ALL {
        let lag = DarkLag::of(SurfaceMaterial::Ice, Some(lid));
        assert_eq!(
            lag.is_some(),
            CrustWater::of_record(SurfaceMaterial::Ice, Some(lid)) == CrustWater::Unlisted,
            "{lid:?}"
        );
        let class = match lid {
            CrustLithology::Water => None,
            CrustLithology::Cometary => Some(DarkLag::Cometary),
            CrustLithology::ChondriteCi | CrustLithology::ChondriteCm => {
                Some(DarkLag::Carbonaceous)
            }
            CrustLithology::Graphite
            | CrustLithology::Feldspathic
            | CrustLithology::Basalt
            | CrustLithology::IronRichBasalt
            | CrustLithology::Komatiite
            | CrustLithology::ReducedBasalt
            | CrustLithology::Granite
            | CrustLithology::Ultramafic
            | CrustLithology::Phyllosilicate
            | CrustLithology::ChondriteCv
            | CrustLithology::ChondriteH
            | CrustLithology::ChondriteL
            | CrustLithology::ChondriteLl
            | CrustLithology::ChondriteE => Some(DarkLag::Rock),
        };
        assert_eq!(lag, class, "{lid:?}");
        assert_eq!(
            DarkLag::of(SurfaceMaterial::Rock, Some(lid)),
            None,
            "{lid:?}"
        );
    }
    assert_eq!(DarkLag::of(SurfaceMaterial::Ice, None), None);
    assert_eq!(DarkLag::of(SurfaceMaterial::Rock, None), None);
    assert_eq!(
        DarkLag::of(SurfaceMaterial::Ice, Some(CrustLithology::Phyllosilicate)),
        Some(DarkLag::Rock),
        "Ceres's lid"
    );
}

/// The measured exposed shares of each class's bodies, low and high, as the shares' doc comments
/// cite them: the comets' 10⁻⁴ (under Tempel 1's 1.5–3 × 10⁻⁴ of its observed side) to 10⁻² (an
/// order above 67P's detected 0.1%, Fornasier et al. 2023); Callisto's 0.03 (its albedo split,
/// Moore et al. 2004) to 0.3 (Calvin and Clark 1991's ice-bearing patches), its spectra's 10%
/// between (Spencer 1987); and Ceres's 1.3 × 10⁻⁶ (Juling's 3.6 km² alone, Raponi et al. 2018) to
/// 2.3 × 10⁻⁵ (Combe et al. 2019's nine sites at their 7 km² bound, over its 2.77 × 10⁶ km²).
fn sources_range(lag: DarkLag) -> (f64, f64) {
    match lag {
        DarkLag::Cometary => (1e-4, 1e-2),
        DarkLag::Carbonaceous => (0.03, 0.3),
        DarkLag::Rock => (1.3e-6, 2.3e-5),
    }
}

/// Each class's exposed share lies inside its sources' range, and the ranges are small and ordered:
/// Ceres's rare exposures under a comet's patches, under 1%, under Callisto's crest frost.
#[test]
fn each_lag_s_exposed_share_lies_in_its_sources_range() {
    for lag in [DarkLag::Cometary, DarkLag::Carbonaceous, DarkLag::Rock] {
        let (low, high) = sources_range(lag);
        let share = lag.exposed_share();
        assert!(
            0.0 < low && low <= share && share <= high && high < 1.0,
            "{lag:?}: {share} in [{low}, {high}]"
        );
    }
    assert!(sources_range(DarkLag::Rock).1 <= sources_range(DarkLag::Cometary).0);
    assert!(
        sources_range(DarkLag::Cometary).1 <= 0.01,
        "a comet's patches, under 1%"
    );
    assert!(sources_range(DarkLag::Cometary).1 <= sources_range(DarkLag::Carbonaceous).0);
}

/// The item is the lag's share held to the cover's ice area, and with no lag the area to the bit.
/// A NaN area stays a NaN and a negative zero keeps its sign.
#[test]
fn the_shown_ice_is_the_lag_s_share() {
    for ice in [0.0, 5e-6, 0.05, 0.25, 0.731, 1.0] {
        for lag in [
            None,
            Some(DarkLag::Cometary),
            Some(DarkLag::Carbonaceous),
            Some(DarkLag::Rock),
        ] {
            let exposed = shown(lag, ice);
            match lag {
                None => assert_same_bits(exposed, ice),
                Some(lag) => {
                    assert_same_bits(exposed, ice.min(lag.exposed_share()));
                    assert!(exposed <= sources_range(lag).1, "{lag:?} at {ice}");
                }
            }
        }
    }
    // A NaN stays a NaN, as `f64::min` would not keep it, and a negative zero its sign.
    for lag in [DarkLag::Cometary, DarkLag::Carbonaceous, DarkLag::Rock] {
        assert!(shown(Some(lag), f64::NAN).is_nan(), "{lag:?}");
        assert_same_bits(shown(Some(lag), -0.0), -0.0);
    }
}

/// The belt test asks for part of the surface: a cover within [`WHOLE_COVER_ROUNDING`] of 1, such
/// as one an ulp under 1, a list the surface scaled, is whole. No cover and a whole one are never
/// part.
#[test]
fn a_cover_the_surface_scaled_is_whole() {
    let ulp_under = 1.0 - f64::EPSILON / 2.0;
    assert!(ulp_under < 1.0);
    assert!(!covers_part(ulp_under));
    assert!(!covers_part(1.0 - WHOLE_COVER_ROUNDING));
    assert!(covers_part(1.0 - 2.0 * WHOLE_COVER_ROUNDING));
    assert!(covers_part(0.5) && covers_part(COMETARY_EXPOSED_SHARE));
    assert!(!covers_part(0.0) && !covers_part(1.0));
}
