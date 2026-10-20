//! The laboratory rows' tests (plan 14, P14.T49.i; decision-p14-t49i-rock-mixtures §3): each
//! inversion's round trip, the mixtures' bands in the like quantity and their order, the anchors
//! space weathering does not set (Io, Titan's dunes, Mars's dust), and the method's check on
//! `basalt`.

use std::sync::Arc;

use hyperion_sim::substance::optics::{
    CondensedOptics, EndMember, FRESH_SNOW, Geometry, GrainScaling, LUNAR_REGOLITH, MixturePart,
    OpticsInput,
};
use hyperion_sim::substance::{self, by_key};

use super::lab::{EndMemberGrain, cosines};
use super::tests::{bond_albedo, fitted_row, manifest};
use super::*;

/// The allowance of every band, `Adopted`: a class passes within [lo ÷ 1.25, 1.25 × hi]
/// (decision-p14-t49i-rock-mixtures §3: the laboratory powders' grain range, their packing and the
/// photometry's ratio against each powder's own surge). The ruling sets it for the classes' bands;
/// the lane applies the same model allowance to Mars's bright dust and Titan's dunes, the model's
/// uncertainty being the same (the form's grains against the surface's, its packing, and the
/// reference photometry against the surface's own), and no other tolerance being ruled for them.
const ALLOWANCE: f64 = 1.25;

/// The optics of row `key`.
fn optics_of(key: &str) -> &'static CondensedOptics {
    by_key(key)
        .and_then(|id| id.substance().condensed_optics())
        .unwrap_or_else(|| panic!("{key} has optics"))
}

/// The grain of row `key`, read from the task's own manifest.
fn grain_of(key: &str) -> RowGrain {
    let manifest = manifest();
    let mut inputs = FitInputs::load(&manifest).unwrap();
    inputs.grain(by_key(key).unwrap(), optics_of(key)).unwrap()
}

/// Row `key`'s w at `nm`, at its own grains.
fn w_at(key: &str, nm: f64) -> f64 {
    grain_of(key)
        .as_spectral()
        .single_scattering_albedo(nm, optics_of(key).grain_diameter())
}

/// The reflectance factor at i 30°, e 0° (g 30°) of grains of albedo `w` under the regolith's
/// photometry: the 30° laboratory measurements' quantity (decision-p14-t49i-rock-mixtures §1).
fn reflectance_30(w: f64) -> f64 {
    let geometry = Geometry::new(30.0, 0.0, 0.0).unwrap();
    reflectance_factor(w, &LUNAR_REGOLITH, cosines(geometry))
}

/// Whether `value` lies inside the band [`lo`, `hi`] widened by [`ALLOWANCE`].
fn inside(value: f64, (lo, hi): (f64, f64)) -> bool {
    value >= lo / ALLOWANCE && value <= hi * ALLOWANCE
}

/// The end-members the rows read, each once, by name.
fn end_members() -> Vec<&'static EndMember> {
    let mut found: Vec<&'static EndMember> = Vec::new();
    let mut add = |e: &'static EndMember| {
        if !found.iter().any(|f| f.name() == e.name()) {
            found.push(e);
        }
    };
    for (_, optics) in substance::optics::rows() {
        match optics.input() {
            OpticsInput::Spectrum { end_member } => add(end_member),
            OpticsInput::Mixture { parts } => parts.iter().for_each(|p| add(p.end_member())),
            OpticsInput::Index { .. }
            | OpticsInput::SizedIndex { .. }
            | OpticsInput::OpaqueSlab { .. }
            | OpticsInput::Levels { .. } => {}
        }
    }
    found
}

/// The end-member named `name`.
fn end_member(name: &str) -> &'static EndMember {
    end_members()
        .into_iter()
        .find(|e| e.name() == name)
        .unwrap_or_else(|| panic!("{name} is an end-member"))
}

/// The spectra with clipped channels, (file, bright, dark): none. A channel at or above the model's
/// value at w = 1, or at or below 0, would take w = 1 or 0 and be stated here.
const CLIPPED: [(&str, usize, usize); 0] = [];

/// Each inversion reproduces its spectrum at its own geometry and grain: every channel of every
/// end-member's spectrum, inverted by the regolith's model at its spectrometer's geometry, gives
/// back its measured value to 10⁻¹², none is clipped but [`CLIPPED`]'s, and taken to its own
/// laboratory grain size its w is its inverted w, so the slab's rescale is the identity there.
#[test]
fn substance_albedos_each_spectrum_round_trips_at_its_geometry() {
    let manifest = manifest();
    let mut inputs = FitInputs::load(&manifest).unwrap();
    let mut spectra = 0;
    for e in end_members() {
        let grain: Arc<EndMemberGrain> = inputs.end_member(e).unwrap();
        for (spectrum, inverted) in e.spectra().iter().zip(grain.spectra()) {
            spectra += 1;
            let clipped = CLIPPED
                .iter()
                .find(|(file, _, _)| *file == spectrum.file())
                .map_or((0, 0), |c| (c.1, c.2));
            assert_eq!(
                (inverted.bright(), inverted.dark()),
                clipped,
                "{}: clipped channels",
                spectrum.file()
            );
            assert!(
                inverted.residual() < 1e-12,
                "{}: round trip {}",
                spectrum.file(),
                inverted.residual()
            );
        }
        if let GrainScaling::Slab {
            lab_grain_diameter, ..
        } = e.scaling()
        {
            let channels = grain.spectra()[0].channels();
            for &(nm, w) in channels.iter().step_by(37) {
                let back = grain.single_scattering_albedo(nm, lab_grain_diameter);
                assert!((back - w).abs() < 1e-12, "{}: {nm} nm", e.name());
            }
        }
    }
    assert_eq!(spectra, 40, "every spectrum the dataset commits is read");
}

/// The bands of the mixture classes in the like quantity (decision-p14-t49i-rock-mixtures §1 and
/// §3), each within [`ALLOWANCE`]; a row that misses one is a stand-in whose sources state the
/// miss, a row that meets all of its own is not:
///
/// - `chondrite_ci`: the reflectance factor at i 30°, e 0° at 550 nm in 0.020–0.035, the ruling's
///   band after Amano et al. 2023 (Sci. Adv. 9, eadi3789): its lower end Ryugu's global visible
///   reflectance, ~2%, from remote sensing (ONC-T and NIRS3), where Ryugu's returned grains measure
///   ~2 to 4% at v-band at i 30°, e 0° (g 30°), its powder aggregates inside that range and darker
///   than the coarse ones; its upper end Orgueil's preheated < 155 µm powder (~3.5% at v-band, the
///   same geometry);
/// - `chondrite_cm`: `A_N` at 550 nm in 0.047–0.085, CM's equivalent geometric albedo 0.066 ±
///   0.019, extrapolated to zero phase without the coherent backscatter (Beck et al. 2021, Icarus
///   354, 114066, §6.1), and the reflectance factor at 30° at 560 nm in 0.028–0.140 (Cloutis et al.
///   2011b, Icarus 216, 309, abstract: "from 2.8% to 14.0% at 0.56 μm");
/// - `chondrite_cv`: the reflectance factor at 30° at 560 nm in 0.05–0.13 (Cloutis et al. 2012,
///   Icarus 221, 328, abstract: "∼5% to 13% at 0.56 μm");
/// - `chondrite_h`, `_l`, `_ll`: the reflectance factor at 30° at 700 nm in 0.25–0.39, the falls
///   Chelyabinsk (LL5, 25%) and Viñales (L, 39%) at 45–90 µm (Bowen et al. 2023, PSJ 4, 52, p. 3),
///   measured at i 0°, e 30° (their lab's geometry, Cantillo et al. 2023, PSJ 4, 177, read as
///   arXiv:2401.10377, §3.2), the same by reciprocity;
/// - `basalt_fe`: `A_N` at 550 nm above the mature maria's 0.09 and at most the HEDs' 0.5 (Beck et
///   al. 2021), bounds rather than a band, with no allowance; it is a stand-in anyway, an upper
///   bound (§2.7).
#[test]
fn substance_albedos_the_mixtures_meet_their_bands() {
    let a_n = |key: &str| fitted_row(key)[4];
    let r30 = |key: &str, nm: f64| reflectance_30(w_at(key, nm));
    let checks: [(&str, &str, f64, (f64, f64)); 7] = [
        (
            "chondrite_ci",
            "Amano",
            r30("chondrite_ci", 550.0),
            (0.020, 0.035),
        ),
        ("chondrite_cm", "Beck", a_n("chondrite_cm"), (0.047, 0.085)),
        (
            "chondrite_cm",
            "Cloutis",
            r30("chondrite_cm", 560.0),
            (0.028, 0.140),
        ),
        (
            "chondrite_cv",
            "Cloutis",
            r30("chondrite_cv", 560.0),
            (0.05, 0.13),
        ),
        (
            "chondrite_h",
            "Bowen",
            r30("chondrite_h", 700.0),
            (0.25, 0.39),
        ),
        (
            "chondrite_l",
            "Bowen",
            r30("chondrite_l", 700.0),
            (0.25, 0.39),
        ),
        (
            "chondrite_ll",
            "Bowen",
            r30("chondrite_ll", 700.0),
            (0.25, 0.39),
        ),
    ];
    for key in [
        "chondrite_ci",
        "chondrite_cm",
        "chondrite_cv",
        "chondrite_h",
        "chondrite_l",
        "chondrite_ll",
    ] {
        let optics = optics_of(key);
        let own: Vec<_> = checks.iter().filter(|c| c.0 == key).collect();
        let passes = own.iter().all(|c| inside(c.2, c.3));
        assert_eq!(
            optics.is_stand_in(),
            !passes,
            "{key}: {:?}",
            own.iter().map(|c| (c.1, c.2)).collect::<Vec<_>>()
        );
        let band = optics
            .sources()
            .iter()
            .find(|s| s.covers() == "band" || s.covers() == "stand-in")
            .unwrap_or_else(|| panic!("{key} states its band"));
        for check in own {
            let stated = format!("{:.3}", check.2);
            assert!(
                band.citation().contains(&stated) && band.citation().contains(check.1),
                "{key}: its sources state {stated} against {}",
                check.1
            );
        }
    }
    let bfe = a_n("basalt_fe");
    assert!(bfe > 0.09 && bfe <= 0.5, "basalt_fe: A_N {bfe}");
    assert!(optics_of("basalt_fe").is_stand_in());
}

/// The classes' order in `A_N` at 550 nm: CI and CM below CV, below H, L and LL, the factor-of-two
/// gaps of the measured classes (Beck et al. 2021's order CK > CR > CV > CO > CM > CI; the
/// ordinary chondrites' powders far brighter; decision-p14-t49i-rock-mixtures §3). CI is not
/// ordered against CM, their measured means differing by less than the allowance.
#[test]
fn substance_albedos_the_chondrites_keep_their_order() {
    let a_n = |key: &str| fitted_row(key)[4];
    let cv = a_n("chondrite_cv");
    for dark in ["chondrite_ci", "chondrite_cm"] {
        assert!(a_n(dark) < cv, "{dark} {} against CV {cv}", a_n(dark));
    }
    for bright in ["chondrite_h", "chondrite_l", "chondrite_ll"] {
        assert!(cv < a_n(bright), "CV {cv} against {bright} {}", a_n(bright));
    }
}

/// `basalt`'s parts by Peck 1978's average Alae mode (USGS Professional Paper 935-B, p. B3), to
/// vol% at the Handbook of Mineralogy's densities, its apatite and glass left out (the row's
/// sources).
fn basalt_check_parts() -> [MixturePart; 5] {
    use hyperion_sim::substance::optics::GrainClass::Form;
    [
        MixturePart::new(end_member("olivine HS285.2B Fo80"), 0.0331, Form),
        MixturePart::new(end_member("augite NMNH120049"), 0.4212, Form),
        MixturePart::new(end_member("anorthite HS201.3B"), 0.5084, Form),
        MixturePart::new(end_member("ilmenite HS231.3B"), 0.0309, Form),
        MixturePart::new(end_member("magnetite HS195.3B"), 0.0064, Form),
    ]
}

/// The method's check on `basalt` (decision-p14-t49i-rock-mixtures §2.6): the mixture of clean
/// separates at Peck 1978's Alae mode lies above `basalt`'s whole-rock `A_N` in V, since
/// inclusions, glass, shock and fine groundmass opaques only darken an anhydrous rock, and the
/// ratio is the one `basalt`'s and `basalt_fe`'s sources state.
#[test]
fn substance_albedos_the_separates_bound_basalt_from_above() {
    let manifest = manifest();
    let mut inputs = FitInputs::load(&manifest).unwrap();
    let parts = basalt_check_parts();
    let mixture = lab::MixtureGrain::new(
        &parts,
        &inputs.library,
        &inputs.tables,
        &mut inputs.end_members,
    )
    .unwrap();
    let d = ReferenceForm::Regolith.grain_diameter();
    let w = mixture.single_scattering_albedo(550.0, d);
    let check = Medium::new(w, &LUNAR_REGOLITH).normal_albedo();
    let rock = fitted_row("basalt")[4];
    let ratio = check / rock;
    assert!(
        ratio > 1.0,
        "the separates' A_N {check} against the rock's {rock}"
    );
    let stated = format!("{ratio:.1} times");
    for key in ["basalt", "basalt_fe"] {
        assert!(
            optics_of(key)
                .sources()
                .iter()
                .any(|s| s.citation().contains(&stated)),
            "{key}'s sources state {stated} (A_N {check:.3})"
        );
    }
}

/// Io, as a bound: its bolometric Bond albedo, 0.50 ± 0.10 (Simonelli and Veverka 1984, Icarus
/// 59, 406, abstract), lies below both the `S8` and `SO2` rows' Bond albedos under a 5,772 K
/// blackbody, the whole of its range included, since the disc carries darker material than either
/// pure frost or powder.
#[test]
fn substance_albedos_io_lies_below_its_sulphur_and_frost() {
    for key in ["S8", "SO2"] {
        let row = fitted_row(key);
        let bond = bond_albedo(&row[SPECTRAL_BINS_OFFSET..DATA_REACH_OFFSET], 5_772.0);
        assert!(bond > 0.50 + 0.10, "{key}: Bond albedo {bond}");
    }
}

/// `SO2`'s levels at the other azimuth: with source and detector on the same side of the normal,
/// the phase angle is 40°, and the row's Bond albedo under a 5,772 K blackbody would be the one its
/// sources record, beside the adopted 80°'s.
#[test]
fn substance_albedos_so2_s_other_phase_is_recorded() {
    let OpticsInput::Levels { levels } = optics_of("SO2").input() else {
        panic!("SO2 is levels");
    };
    let opposite = levels.geometry();
    let same_side = hyperion_sim::substance::optics::ReflectanceLevels::new(
        levels.knots(),
        Geometry::new(opposite.incidence_deg(), opposite.emission_deg(), 0.0).unwrap(),
    );
    let grain = lab::LevelsGrain::new(&same_side, &FRESH_SNOW);
    let cache = SphericalAlbedoCache::new();
    let bins = spectral_bins(
        &grain,
        ReferenceForm::Frost.grain_diameter(),
        FRESH_SNOW.phase_function().asymmetry(),
        &cache,
        SPECTRAL_STEP_NM,
    );
    let bond = bond_albedo(&bins, 5_772.0);
    let stated = format!("{bond:.2}");
    assert!(
        optics_of("SO2")
            .sources()
            .iter()
            .any(|s| s.citation().contains("40°") && s.citation().contains(&stated)),
        "SO2's sources record its Bond albedo at 40°, {stated}"
    );
    assert!(grain.inverted().residual() < 1e-12);
}

/// Titan's dunes in its infrared windows (Brossier et al. 2018, JGR Planets 123, 1089, Fig. 7a, the
/// dune unit's values read by eye from the figure, 0.56–0.57 at 1.27 µm, 0.46 at 1.59 µm and 0.42
/// at 2.01 µm, its ordinate taken as single-scattering albedo, as the lane's preparation read it,
/// unconfirmed: the abstract calls its quantity a "surface albedo"): the `tholin` row's w at 1.27
/// µm, the window's wavelength as the figure gives it, interpolated between Khare et al.'s rows at
/// 1.192 and 1.357 µm (no node of its data or of the fit's grid falls at 1.27 µm), within the
/// lane's allowance of the dunes' value there, and brighter than the dunes at 1.59 and 2.01 µm,
/// the miss recorded: the dunes are not pure laboratory tholin (their Table 3 mixes 70–90% tholins
/// with water ice), so the anchor tests the dune material, not the substance, and `tholin` is no
/// stand-in for it. A reading by eye supports about ±0.01 (the
/// 1.27 µm value is read as the range 0.56–0.57, the others to two decimals), and the test asks no
/// finer: the allowance of 1.25 at 1.27 µm spans 0.448–0.713, and at 1.59 and 2.01 µm the row lies
/// 0.18 and 0.15 above the readings. These three values are facts with their citation, asserted
/// here only; nothing generated reads them, and no figure, digitised curve or data file of the
/// paper (CC BY-NC-ND) is committed (decision-composition §5).
#[test]
fn substance_albedos_tholin_meets_titan_s_dunes() {
    let at_1270 = w_at("tholin", 1_270.0);
    assert!(
        inside(at_1270, (0.56, 0.57)),
        "tholin's w at 1.27 µm {at_1270}"
    );
    eprintln!("tholin's w at 1.27 µm: {at_1270:.4}");
    for (nm, dunes) in [(1_590.0, 0.46), (2_010.0, 0.42)] {
        let w = w_at("tholin", nm);
        assert!(
            w > dunes,
            "tholin's w at {nm} nm {w} against the dunes' {dunes}"
        );
    }
    assert!(!optics_of("tholin").is_stand_in());
}

/// Mars's bright dust (decision-p14-t49c-split §1.3): `mars_dust`'s Bond albedo under a 5,772 K
/// blackbody against the bright regions' clear-sky bolometric Lambert albedo, 0.27 (Pleskot and
/// Miner 1981, Icarus 45, 179, abstract), within the allowance or, missing it, stated in its
/// stand-in's reason with the miss's direction; the row is a stand-in either way, its grains below
/// geometric optics.
#[test]
fn substance_albedos_mars_dust_is_held_against_the_bright_regions() {
    let row = fitted_row("mars_dust");
    let bond = bond_albedo(&row[SPECTRAL_BINS_OFFSET..DATA_REACH_OFFSET], 5_772.0);
    let optics = optics_of("mars_dust");
    assert!(optics.is_stand_in());
    let reason = optics
        .sources()
        .iter()
        .find(|s| s.covers() == "stand-in")
        .expect("mars_dust states its reason");
    assert!(reason.citation().contains(&format!("{bond:.2}")), "{bond}");
    let verdict = if inside(bond, (0.27, 0.27)) {
        "within"
    } else if bond > 0.27 {
        "brighter"
    } else {
        "darker"
    };
    assert!(reason.citation().contains(verdict), "{bond}: {verdict}");
}

/// The opaque slab's label: splib07a's iron powder SA-209309 lies the stated share below the slab
/// of `iron.json` at 550 nm, which makes `Cr`'s and `graphite`'s slabs smooth-grain upper bounds.
#[test]
fn substance_albedos_the_iron_powder_lies_below_its_slab() {
    let manifest = manifest();
    let file = manifest.load_material("iron.json").unwrap();
    let (n, k) = index_at(&file, 550.0).unwrap();
    let slab = lab::hemispherical_fresnel_reflectance(n, k);
    let powder = w_at("Fe", 550.0);
    let below = 100.0 * (1.0 - powder / slab);
    let stated = format!("{below:.0}% below");
    for key in ["Cr", "graphite"] {
        assert!(
            optics_of(key)
                .sources()
                .iter()
                .any(|s| s.citation().contains(&stated)),
            "{key} states {stated} (w {powder:.3} against {slab:.3})"
        );
    }
}

/// The rows whose w the regolith's energy bound holds somewhere in their data, with the count of
/// their channels it holds: halite's, `NaCl`'s and `KCl`'s, stated in their sources.
#[test]
fn substance_albedos_the_clamp_binds_where_it_is_stated() {
    let manifest = manifest();
    let mut inputs = FitInputs::load(&manifest).unwrap();
    let mut clamped = Vec::new();
    for (id, optics) in substance::optics::rows() {
        let grain = inputs.grain(id, optics).unwrap();
        let inner: &dyn super::spectral::SpectralGrain = match &grain {
            RowGrain::EndMember(g) => g.inner(),
            RowGrain::Mixture(g) => g.inner(),
            RowGrain::Index { .. } | RowGrain::Opaque { .. } | RowGrain::Levels(_) => continue,
        };
        let d = optics.grain_diameter();
        let over = inner
            .knots_nm()
            .into_iter()
            .filter(|&nm| {
                inner.single_scattering_albedo(nm, d) > REGOLITH_MAX_SINGLE_SCATTERING_ALBEDO
            })
            .count();
        if over > 0 {
            clamped.push((id.substance().key().as_str().to_owned(), over));
        }
    }
    let keys: Vec<&str> = clamped.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, CLAMPED_ROWS, "{clamped:?}");
    for (key, over) in &clamped {
        let stated = format!("{over} of its");
        assert!(
            optics_of(key)
                .sources()
                .iter()
                .any(|s| s.citation().contains(&stated)),
            "{key}: {stated}"
        );
    }
}

/// The rows the regolith's energy bound clamps.
const CLAMPED_ROWS: [&str; 2] = ["KCl", "NaCl"];

/// CO's noise floor (`infrared::CO_NOISE_FLOOR`): three times the rms of Gerakines et al. 2023's k
/// over its band-free window, to the floor's three digits, and every k the `CO` row reads is 0 or
/// at least the floor.
#[test]
fn substance_albedos_co_s_noise_floor_is_three_times_its_baseline_rms() {
    let manifest = manifest();
    let data = infrared::InfraredData::load(&manifest).unwrap();
    let text = data
        .files("gerakines_2023_co")
        .unwrap()
        .values()
        .next()
        .unwrap();
    let rows = infrared::table_rows("gerakines_2023_co", text, "wavenumber_per_cm,n,k").unwrap();
    let [lo, hi] = infrared::CO_NOISE_WINDOW_PER_CM;
    let window: Vec<f64> = rows
        .iter()
        .filter(|r| (lo..=hi).contains(&r[0]))
        .map(|r| r[2])
        .collect();
    assert_eq!(window.len(), 1_245);
    let n = f64::from(u32::try_from(window.len()).unwrap());
    let rms = (window.iter().map(|k| k * k).sum::<f64>() / n).sqrt();
    assert!(
        (3.0 * rms / infrared::CO_NOISE_FLOOR - 1.0).abs() < 5e-3,
        "3 × rms {} against the floor",
        3.0 * rms
    );
    let co = optics_of("CO");
    let file = manifest.load_material("carbon-monoxide-ice.json").unwrap();
    let spectral = infrared::spectral_index(co, "carbon-monoxide-ice.json", &file, &data).unwrap();
    // The table's own rows: its piece's interior knots, not the ends where the pieces meet.
    for nm in spectral
        .knots_nm()
        .into_iter()
        .filter(|&nm| nm > infrared::JOIN_NM && nm < infrared::REACH_END_NM)
    {
        let (_, k) = spectral.at(nm);
        assert!(
            k.total_cmp(&0.0).is_eq() || k >= infrared::CO_NOISE_FLOOR,
            "{nm} nm: {k}"
        );
    }
}
