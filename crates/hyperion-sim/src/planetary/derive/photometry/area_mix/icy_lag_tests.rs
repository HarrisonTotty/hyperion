//! P14.T49.m's icy lid's regolith: its hold, the two-size mixture against its rows and the
//! similarity relation, the lag's calibration on the measured Ganymede, the resurfacing step, and
//! the measured moons' windows at the area mix (decision-p14-t54a-ganymede §1.5).

use hyperion_testkit::float::assert_same_bits;

use super::*;

fn id(key: &str) -> SubstanceId {
    by_key(key).unwrap_or_else(|| panic!("{key} is a row"))
}

fn optics(key: &str) -> &'static CondensedOptics {
    id(key)
        .substance()
        .condensed_optics()
        .unwrap_or_else(|| panic!("{key} has condensed optics"))
}

/// An old airless surface's lid weathering at maturity 0.5 and 100 K.
fn airless() -> LidWeathering {
    LidWeathering {
        maturity: 0.5,
        temperature: Kelvin::new(100.0),
        surface_age: Years::new(4.5e9),
        state: SurfaceState::Airless,
    }
}

/// Every `H2O` regolith the passes build, known and not: the stand-in's row for ice, a lid's row
/// by the stand-in, and a lid's row at a maturity, on a rocky and an icy bulk.
fn h2o_regoliths() -> [Regolith; 5] {
    let water = id("H2O");
    [
        Regolith::of_material(SurfaceMaterial::Ice),
        Regolith::with_lid(water, SurfaceMaterial::Ice),
        Regolith::with_lid(water, SurfaceMaterial::Rock),
        Regolith::weathered_lid(water, SurfaceMaterial::Ice, airless()),
        Regolith::weathered_lid(water, SurfaceMaterial::Rock, airless()),
    ]
}

/// A body's resurfacing by its tide `flux` alone, its ice holding no volatile.
fn tide(flux: WattsPerSquareMetre) -> IcyResurfacing {
    IcyResurfacing::new(Some(flux), VolatileFrost::None)
}

/// The tidal heat flux of the measured Enceladus over its sphere, W m⁻²: its south polar terrain's
/// 15.8 GW (Howett et al. 2011, JGR 116, E03003; from memory) over 4π (252.1 km)².
fn enceladus_tidal_flux() -> WattsPerSquareMetre {
    let r = 252.1e3;
    WattsPerSquareMetre::new(15.8e9 / (4.0 * core::f64::consts::PI * r * r))
}

/// Under the frost, the tests' oracle, every `H2O` regolith is T49.i's fresh frost to the bit, as
/// at 26, whatever its pass knows of its resurfacing, and the weathered area mix's `H2O` lid keeps
/// its weathered arm.
#[test]
fn under_the_frost_every_h2o_regolith_is_t49i_s_frost_to_the_bit() {
    let frost = solid_bond(id("H2O"), &sun_bands()).bond;
    with_icy_regolith(IcyRegolithRules::Frost, || {
        for regolith in h2o_regoliths() {
            for resurfacing in [
                IcyResurfacing::NOT_KNOWN,
                IcyResurfacing::new(Some(WattsPerSquareMetre::new(0.0)), VolatileFrost::None),
                IcyResurfacing::new(Some(RESURFACING_TIDAL_FLUX), VolatileFrost::None),
            ] {
                let regolith = regolith.with_resurfacing(resurfacing);
                for state in [SurfaceState::Airless, SurfaceState::Tenuous] {
                    assert_same_bits(regolith.bond(state, &sun_bands()).bond, frost);
                }
            }
        }
        assert!(
            Regolith::weathered_lid(id("H2O"), SurfaceMaterial::Rock, airless()).is_weathered()
        );
    });
}

/// The mixture of one row gives that row back: the frost's bins within rounding, and
/// `chondrite_ci`'s, under the regolith form, within the tables' interpolation (under 10⁻³); and
/// that interpolation is the similarity relation's, the regolith form's tabled spherical albedo
/// matching the frost form's at the w its asymmetry takes to the same isotropic medium.
#[test]
fn a_mixture_of_one_row_is_that_row() {
    let (frost, rock) = (optics("H2O"), optics(ICY_LAG_ROCK));
    let alone = two_size_mixture((frost, 1.0), (rock, 0.0));
    for (mixed, own) in alone.iter().zip(frost.spherical_albedo_bins()) {
        assert!((mixed - own).abs() < 1e-12, "{mixed} against {own}");
    }
    let alone = two_size_mixture((rock, 1.0), (frost, 0.0));
    for (mixed, own) in alone.iter().zip(rock.spherical_albedo_bins()) {
        assert!((mixed - own).abs() < 1e-3, "{mixed} against {own}");
    }
    let regolith = form_spherical(ReferenceForm::Regolith);
    let xi_f = ReferenceForm::Frost
        .photometry()
        .phase_function()
        .asymmetry();
    let xi_r = ReferenceForm::Regolith
        .photometry()
        .phase_function()
        .asymmetry();
    let mut worst: f64 = 0.0;
    for i in 0..=1_000 {
        let w = f64::from(i) / 1_000.0;
        let isotropic = w * (1.0 - xi_r) / (1.0 - w * xi_r);
        let on_frost = isotropic / (1.0 - xi_f + isotropic * xi_f);
        let gap = (spherical_of_grain(regolith, w)
            - spherical_of_grain(form_spherical(ReferenceForm::Frost), on_frost))
        .abs();
        if gap > worst {
            worst = gap;
        }
    }
    eprintln!(
        "the regolith form's A_S against the frost form's by the relation: at most {worst:.3e}"
    );
    assert!(worst < 1e-3, "{worst}");
    // Inverting then reading a column returns the albedo, within rounding.
    for albedo in [0.01, 0.2, 0.5, 0.83, 0.99] {
        let w = grain_albedo_of(regolith, albedo);
        assert!((spherical_of_grain(regolith, w) - albedo).abs() < 1e-12);
    }
    assert_same_bits(grain_albedo_of(regolith, 0.0), 0.0);
    assert_same_bits(grain_albedo_of(regolith, 1.5), 1.0);
}

/// The lag's share is the measured Ganymede's: about a Sun the lag gives 0.3500, inside 0.32–0.38
/// (0.35 ± 0.03, Squyres and Veverka 1982; Buratti 1991), and the share the mixture inverts to
/// 0.35 lies within 10⁻³ of [`ICY_LAG_NON_ICE_SHARE`]; the lag darkens monotonically with its
/// share, from the frost's 0.832 at none to the rock's own at all. Its Bond albedo about other
/// hosts and its bins are stated.
#[test]
fn the_lag_s_share_gives_the_measured_ganymede() {
    let sun = sun_bands();
    let lag = band_weighted(&icy_lag_bins(), &sun);
    eprintln!("the lag about a Sun: {lag:.4}, against Ganymede's 0.35 ± 0.03");
    assert!((0.32..=0.38).contains(&lag), "{lag}");
    let (frost, rock) = (optics("H2O"), optics(ICY_LAG_ROCK));
    let at =
        |share: f64| band_weighted(&two_size_mixture((frost, 1.0 - share), (rock, share)), &sun);
    assert_same_bits(at(ICY_LAG_NON_ICE_SHARE), lag);
    let invert = |target: f64| {
        let (mut low, mut high) = (0.0, 1.0);
        for _ in 0..60 {
            let middle = f64::midpoint(low, high);
            if at(middle) > target {
                low = middle;
            } else {
                high = middle;
            }
        }
        low
    };
    let ganymede = invert(0.35);
    eprintln!(
        "shares: Ganymede's 0.35 at {ganymede:.4} (0.38 at {:.4}, 0.32 at {:.4}); Callisto's 0.11 \
         at {:.4}; Europa's 0.64 at {:.4}",
        invert(0.38),
        invert(0.32),
        invert(0.11),
        invert(0.64)
    );
    assert!(
        (ganymede - ICY_LAG_NON_ICE_SHARE).abs() < 1e-3,
        "{ganymede}"
    );
    let mut last = f64::INFINITY;
    for i in 0..=20 {
        let albedo = at(f64::from(i) / 20.0);
        assert!(albedo < last, "{i}: {albedo}");
        last = albedo;
    }
    assert!((at(0.0) - solid_bond(id("H2O"), &sun).bond).abs() < 1e-12);
    assert!((at(1.0) - solid_bond(id(ICY_LAG_ROCK), &sun).bond).abs() < 1e-3);
    for host in [3_200.0, 4_620.0, 5_530.0, 6_240.0, 7_500.0] {
        let shade = HostShade::of_host(Kelvin::new(host));
        eprintln!(
            "the lag about a {host} K host: {:.4} (the frost {:.4})",
            band_weighted(&icy_lag_bins(), shade.bands()),
            solid_bond(id("H2O"), shade.bands()).bond
        );
    }
    eprintln!("the lag's bins: {:.4?}", icy_lag_bins());
    let cm = two_size_mixture(
        (frost, 1.0 - ICY_LAG_NON_ICE_SHARE),
        (optics("chondrite_cm"), ICY_LAG_NON_ICE_SHARE),
    );
    eprintln!(
        "chondrite_cm's lag at the same share: {:.4}",
        band_weighted(&cm, &sun)
    );
}

/// Resurfacing is a tidal step at [`RESURFACING_TIDAL_FLUX`], at or above it fresh frost, under it,
/// not known or not a number the lag; or a volatile air the body's ice holds, at any tide.
#[test]
fn resurfacing_is_a_tide_of_at_least_10_mw_m2_or_a_volatile_frost() {
    let flux = WattsPerSquareMetre::new;
    assert!(!IcyResurfacing::NOT_KNOWN.resurfaced());
    assert!(!tide(flux(0.0)).resurfaced());
    assert!(!tide(flux(0.009_999)).resurfaced());
    assert!(!tide(flux(f64::NAN)).resurfaced());
    assert!(tide(RESURFACING_TIDAL_FLUX).resurfaced());
    assert!(tide(enceladus_tidal_flux()).resurfaced());
    for tidal in [None, Some(flux(0.0)), Some(flux(f64::NAN))] {
        assert!(IcyResurfacing::new(tidal, VolatileFrost::Held).resurfaced());
    }
}

/// Under the mixture (P14.T49.m's tests, decision-p14-t54a-ganymede §1.5), at the area mix about a
/// Sun, each moon's `H2O` regolith at its own resurfacing:
///
/// - Ganymede, its tide under 1 mW m⁻², takes the lag: 0.3500, inside 0.32–0.38 (asserted);
/// - Enceladus, its 19.8 mW m⁻², keeps its frost: 0.8320, inside 0.7–0.9 (E2, asserted), and so
///   does a body whose ice holds a volatile air, Triton's ground (E2's 0.72–0.92);
/// - Callisto, Ganymede's twin in the generator's inputs, takes the same lag, against 0.09–0.13;
///   and Europa, resurfaced, its frost, against 0.58–0.70: both missed, pinned as they stand
///   (deferred findings).
///
/// Every `H2O` regolith takes the same lag, a stand-in's row keeping its label; under an air, and
/// for any other row, nothing moves.
#[test]
fn under_the_mixture_the_measured_moons_take_their_windows() {
    let sun = sun_bands();
    let frost = solid_bond(id("H2O"), &sun).bond;
    with_icy_regolith(IcyRegolithRules::Mixture, || {
        let lag = band_weighted(&icy_lag_bins(), &sun);
        let at = |resurfacing: IcyResurfacing| {
            Regolith::of_material(SurfaceMaterial::Ice)
                .with_resurfacing(resurfacing)
                .bond(SurfaceState::Airless, &sun)
        };
        let quiet = tide(WattsPerSquareMetre::new(1e-4));
        let ganymede = at(quiet);
        assert_eq!(ganymede.stand_in, Some(StandIn::RowByMaterial));
        assert_same_bits(ganymede.bond, lag);
        let callisto = at(IcyResurfacing::NOT_KNOWN).bond;
        let europa = at(tide(WattsPerSquareMetre::new(0.03))).bond;
        let enceladus = at(tide(enceladus_tidal_flux())).bond;
        let triton = at(IcyResurfacing::new(
            Some(WattsPerSquareMetre::new(0.0)),
            VolatileFrost::Held,
        ))
        .bond;
        assert_same_bits(triton, frost);
        eprintln!(
            "Ganymede {:.4} (0.32–0.38), Callisto {callisto:.4} (0.09–0.13, missed), Europa \
             {europa:.4} (0.58–0.70, missed), Enceladus {enceladus:.4} (0.7–0.9), the frost \
             {frost:.4}",
            ganymede.bond
        );
        assert!((0.32..=0.38).contains(&ganymede.bond), "{}", ganymede.bond);
        assert!((0.7..=0.9).contains(&enceladus), "{enceladus}");
        // The misses, pinned as they stand.
        assert_same_bits(callisto, lag);
        assert_same_bits(europa, frost);
        assert!((callisto - 0.3500).abs() < 5e-5, "{callisto}");
        for regolith in h2o_regoliths() {
            let lagged = regolith.with_resurfacing(quiet);
            for state in [SurfaceState::Airless, SurfaceState::Tenuous] {
                assert_same_bits(lagged.bond(state, &sun).bond, lag);
            }
            assert_same_bits(lagged.bond(SurfaceState::Snowball, &sun).bond, frost);
        }
        let known =
            Regolith::with_lid(id("H2O"), SurfaceMaterial::Ice).bond(SurfaceState::Airless, &sun);
        assert_eq!(known.stand_in, None);
        assert!(
            !Regolith::weathered_lid(id("H2O"), SurfaceMaterial::Rock, airless()).is_weathered()
        );
        // Every other row, as at 25.
        let moon = SurfaceState::Airless.albedo(SurfaceMaterial::Rock).value();
        assert_same_bits(
            Regolith::of_material(SurfaceMaterial::Rock)
                .bond(SurfaceState::Airless, &sun)
                .bond,
            moon,
        );
        for key in ["CO2", "N2", "cometary", "chondrite_ci", "basalt_fe"] {
            let row = Regolith::with_lid(id(key), SurfaceMaterial::Ice)
                .with_resurfacing(quiet)
                .bond(SurfaceState::Airless, &sun);
            let held = with_icy_regolith(IcyRegolithRules::Frost, || {
                Regolith::with_lid(id(key), SurfaceMaterial::Ice).bond(SurfaceState::Airless, &sun)
            });
            assert_eq!(row, held, "{key}");
        }
    });
}
