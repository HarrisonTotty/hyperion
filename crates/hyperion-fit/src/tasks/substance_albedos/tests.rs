use hyperion_sim::substance::optics::{
    BVR_BINS, FRESH_SNOW, LUNAR_REGOLITH, REGOLITH_MAX_SINGLE_SCATTERING_ALBEDO,
};
use hyperion_sim::substance::{self, by_key};
use hyperion_surface::material::{BRIGHT_FROST_NORMAL_ALBEDO, PARTICULATE_NORMAL_ALBEDO_CEILING};

use super::*;

/// Chandrasekhar's exact H-function for isotropic scattering, H(μ) at single-scattering albedo w,
/// as Kawabata 2016 tabulates it to 15 digits (arXiv 1611.10197, Tables 2, 3 and 6): (μ, w, H).
const KAWABATA_H: [(f64, f64, f64); 7] = [
    (0.5, 0.5, 1.187_735_132_670_431),
    (1.0, 0.5, 1.251_259_563_383_223),
    (0.5, 0.9, 1.556_033_802_021_363),
    (1.0, 0.9, 1.850_098_516_769_812),
    (1.0, 0.99, 2.472_792_828_397_026),
    (0.5, 1.0, 2.012_778_769_997_181),
    (1.0, 1.0, 2.907_810_529_078_606),
];

/// The H-function is Chandrasekhar's: Kawabata's exact values to 10⁻⁸, and its zeroth moment
/// ∫₀¹ H dμ = (2 ÷ w)(1 − √(1 − w)) to 10⁻⁸, conservative scattering included.
#[test]
fn substance_albedos_h_function_is_chandrasekhar_s() {
    for (mu, w, expected) in KAWABATA_H {
        let h = h_function(w, mu);
        assert!(
            (h - expected).abs() < 1e-8,
            "H({mu}, {w}) = {h}, not {expected}"
        );
    }
    for w in [0.3, 0.9, 0.999, 1.0] {
        let moment: f64 = mu_nodes()
            .into_iter()
            .map(|(mu, weight)| weight * h_function(w, mu))
            .sum();
        let expected = 2.0 / w * (1.0 - (1.0 - w).sqrt());
        assert!(
            (moment - expected).abs() < 1e-8,
            "w {w}: {moment} against {expected}"
        );
    }
    assert!((h_function(0.7, 0.0) - 1.0).abs() < f64::EPSILON);
}

/// Isotropic scatterers without an opposition effect reflect 1 − √(1 − w) H(μ₀) of a beam at μ₀,
/// exactly, as the H-function's own equation gives it: the hemisphere's quadrature holds that to
/// 10⁻⁷ at each incidence the table holds, a lossless layer reflecting all of it.
#[test]
fn substance_albedos_the_hemisphere_s_quadrature_is_exact_for_isotropic_grains() {
    let isotropic = PhaseFunction::Legendre { b: 0.0, c: 0.0 };
    for w in [0.2, 0.9, 0.999, 1.0] {
        let medium = Medium::from_parts(w, isotropic, 0.0, 1.0);
        for mu0 in HEMISPHERICAL_INCIDENCE_COSINES {
            let exact = 1.0 - (1.0 - w).sqrt() * h_function(w, mu0);
            let quadrature = medium.directional_hemispherical_albedo(mu0);
            assert!(
                (quadrature - exact).abs() < 1e-7,
                "w {w}, mu0 {mu0}: {quadrature} against {exact}"
            );
        }
        let exact_normal = w / 8.0 * h_function(w, 1.0) * h_function(w, 1.0);
        assert!((medium.normal_albedo() - exact_normal).abs() < 1e-15);
    }
    let lossless = Medium::from_parts(1.0, isotropic, 0.0, 1.0);
    assert!((lossless.spherical_albedo() - 1.0).abs() < 1e-7);
}

/// Hapke 1981's H-function, H(x) = (1 + 2x) ÷ (1 + 2x √(1 − w)), which Hapke et al. 2012 fitted
/// the Moon with (JGR 117, E00H15, eq. 4).
fn h_1981(w: f64, x: f64) -> f64 {
    (1.0 + 2.0 * x) / (1.0 + 2.0 * x * (1.0 - w).sqrt())
}

/// Hapke 2002's H-function, H(x) ≈ 1 ÷ {1 − w x [r₀ + (1 − 2 r₀ x) ln((1 + x) ÷ x) ÷ 2]}, r₀ = (1 −
/// γ) ÷ (1 + γ), γ = √(1 − w) (Lapotre et al. 2017, JGR Planets 122, 983, eq. 2, "following Hapke
/// [2002]"), with which decision-r10-limb computed Enceladus's normal albedo.
fn h_2002(w: f64, x: f64) -> f64 {
    let gamma = (1.0 - w).sqrt();
    let r0 = (1.0 - gamma) / (1.0 + gamma);
    1.0 / (1.0 - w * x * (r0 + (1.0 - 2.0 * r0 * x) / 2.0 * math::ln((1.0 + x) / x)))
}

/// The normal albedo the model's closed form gives for a published Hapke solution, single-lobe
/// Henyey–Greenstein of mean cosine `xi` (forward for ξ > 0), shadow-hiding amplitude `shadow`
/// (`B_S0`), coherent-backscatter amplitude `coherent` (`B_C0`) and an H-function `h1` at x = 1.
fn published_normal_albedo(w: f64, xi: f64, shadow: f64, coherent: f64, h1: f64) -> f64 {
    let p0 = PhaseFunction::HenyeyGreenstein { asymmetry: xi }.at(1.0);
    w / 8.0 * (p0 * (1.0 + shadow) + h1 * h1 - 1.0) * (1.0 + coherent)
}

/// The closed form reproduces the Moon's normal albedos from their own Hapke solution: Hapke et al.
/// 2012's highlands (Table 1: w, b, `B_S0` and `B_C0` at 566, 604 and 643 nm; their single lobe,
/// eq. 3 with c = 1, backscattering for b > 0; their eq. 4's IMSA with Hapke 1981's H), whose `A_N`
/// are 0.240, 0.254 and 0.276 ± 10%, within 2%, their tabulated quantities being averages over two
/// end-member models (¶30).
#[test]
fn substance_albedos_reproduces_the_lunar_normal_albedos() {
    for (w, b, shadow, coherent, a_n) in [
        (0.333, 0.294, 0.399, 0.456, 0.240),
        (0.357, 0.298, 0.358, 0.456, 0.254),
        (0.387, 0.304, 0.325, 0.456, 0.276),
    ] {
        let model = published_normal_albedo(w, -b, shadow, coherent, h_1981(w, 1.0));
        assert!(
            (model / a_n - 1.0).abs() < 0.02,
            "w {w}: A_N {model} against {a_n}"
        );
    }
}

/// The closed form reproduces R10's bright-frost anchor: Verbiscer and Veverka 1994's Enceladus
/// (w 0.998, g −0.399, `B₀` 0.21) gives `A_N` 1.351 with Hapke 2002's H, decision-r10-limb's
/// computation of `BRIGHT_FROST_NORMAL_ALBEDO`, 1.35.
#[test]
fn substance_albedos_reproduces_enceladus_s_normal_albedo() {
    let w = 0.998;
    let model = published_normal_albedo(w, -0.399, 0.21, 0.0, h_2002(w, 1.0));
    assert!((model - 1.351).abs() < 1e-3, "{model}");
    assert!((model - BRIGHT_FROST_NORMAL_ALBEDO).abs() < 0.01);
}

/// The equivalent slab's limits: a grain with no absorption scatters everything; absorption and
/// size each lower w; and an opaque grain keeps only its surface's reflection, `S_e`.
#[test]
fn substance_albedos_the_slab_has_its_limits() {
    let d = Metres::new(100.0e-6);
    assert!((single_scattering_albedo(1.5, 0.0, 550.0, d) - 1.0).abs() < 1e-15);
    let mut last = 1.0;
    for k in [1e-6, 1e-5, 1e-4, 1e-3, 1e-2] {
        let w = single_scattering_albedo(1.5, k, 550.0, d);
        assert!(w < last && w > 0.0, "k {k}: {w}");
        last = w;
    }
    let small = single_scattering_albedo(1.5, 1e-4, 550.0, Metres::new(10.0e-6));
    let large = single_scattering_albedo(1.5, 1e-4, 550.0, Metres::new(1.0e-3));
    assert!(small > large);
    let k = 0.05;
    let r0 = (0.25 + k * k) / (6.25 + k * k);
    let s_e = 0.0587 + 0.8543 * r0 + 0.0870 * r0 * r0;
    let opaque = single_scattering_albedo(1.5, k, 550.0, Metres::new(1.0e-2));
    assert!((opaque - s_e).abs() < 1e-9, "{opaque} against S_e {s_e}");
}

/// The lunar photometry conserves energy up to its limit: at w =
/// `REGOLITH_MAX_SINGLE_SCATTERING_ALBEDO` every directional-hemispherical albedo is at most 1, and
/// past it, at w = 0.9998, the one at normal incidence is above 1, which is why a transparent grain
/// is not modelled under it. Fresh snow's conserves it at every w, a lossless grain included.
#[test]
fn substance_albedos_the_photometries_conserve_energy_inside_their_limits() {
    let edge = Medium::new(REGOLITH_MAX_SINGLE_SCATTERING_ALBEDO, &LUNAR_REGOLITH);
    for mu0 in HEMISPHERICAL_INCIDENCE_COSINES {
        let a_h = edge.directional_hemispherical_albedo(mu0);
        assert!(a_h <= 1.0, "mu0 {mu0}: {a_h}");
    }
    assert!(Medium::new(0.9998, &LUNAR_REGOLITH).directional_hemispherical_albedo(1.0) > 1.0);
    for w in [0.1, 0.5, 0.9, 0.99, 0.9999, 1.0] {
        let snow = Medium::new(w, &FRESH_SNOW);
        for mu0 in HEMISPHERICAL_INCIDENCE_COSINES {
            assert!(
                snow.directional_hemispherical_albedo(mu0) <= 1.0,
                "w {w}, mu0 {mu0}"
            );
        }
        assert!(snow.normal_albedo() <= PARTICULATE_NORMAL_ALBEDO_CEILING);
    }
}

/// The task's own manifest's path.
pub(super) fn manifest_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("manifests/substance_albedos.toml")
}

/// The task's own manifest, whose material directory is R08's.
pub(super) fn manifest() -> Manifest {
    Manifest::load(&manifest_path()).unwrap()
}

/// The task's fit of its own manifest, computed once for the tests that read it.
pub(super) fn fitted() -> &'static [(SubstanceId, AlbedoRow)] {
    static FITTED: std::sync::OnceLock<Vec<(SubstanceId, AlbedoRow)>> = std::sync::OnceLock::new();
    FITTED.get_or_init(|| fit(&manifest()).unwrap())
}

/// The fitted row of `key`.
pub(super) fn fitted_row(key: &str) -> AlbedoRow {
    let id = by_key(key).unwrap();
    fitted()
        .iter()
        .find(|(r, _)| *r == id)
        .unwrap_or_else(|| panic!("{key} is fitted"))
        .1
}

/// `water-ice.json` with `edit` applied, read back as a material file from a temporary directory.
fn edited_water_ice(edit: impl FnOnce(&mut serde_json::Value)) -> MaterialFile {
    let text = std::fs::read_to_string(manifest().materials_dir().join("water-ice.json")).unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
    edit(&mut value);
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("edited.json"),
        serde_json::to_string(&value).unwrap(),
    )
    .unwrap();
    MaterialFile::read(dir.path(), "edited.json").unwrap()
}

/// The node of `file`'s grid at `wavelength_nm`.
fn node(file: &MaterialFile, wavelength_nm: f64) -> usize {
    file.wavelengths_nm()
        .iter()
        .position(|w| w.total_cmp(&wavelength_nm).is_eq())
        .unwrap_or_else(|| panic!("{wavelength_nm} nm is a node of {}", file.key()))
}

/// The index is found by wavelength on the file's own grid: a node's own values; between two
/// nodes n linear and k linear in ln k (at the midpoint, the nodes' geometric mean), and k linear
/// where a node's k is 0; and nothing off the grid.
#[test]
fn substance_albedos_the_index_is_found_by_wavelength_on_the_file_s_grid() {
    let water = manifest().load_material("water-ice.json").unwrap();
    let at = node(&water, 440.0);
    let (grid, n, k) = (water.wavelengths_nm(), water.n(), water.k());
    let (n_node, k_node) = index_at(&water, 440.0).unwrap();
    assert!(n_node.total_cmp(&n[at]).is_eq() && k_node.total_cmp(&k[at]).is_eq());
    let between = f64::midpoint(grid[at], grid[at + 1]);
    let (n_between, k_between) = index_at(&water, between).unwrap();
    assert!(
        k[at] > 0.0 && k[at + 1] > 0.0,
        "water ice absorbs at 440 nm"
    );
    assert!((n_between - f64::midpoint(n[at], n[at + 1])).abs() < 1e-12);
    assert!(
        (k_between / (k[at] * k[at + 1]).sqrt() - 1.0).abs() < 1e-12,
        "k {k_between} against the nodes' geometric mean"
    );
    assert!(index_at(&water, grid[0] - 1.0).is_none());
    assert!(index_at(&water, grid[grid.len() - 1] + 1.0).is_none());
    let zeroed = edited_water_ice(|v| v["k"][at + 1] = serde_json::Value::from(0.0));
    let (_, k_zeroed) = index_at(&zeroed, between).unwrap();
    assert!(
        (k_zeroed / (0.5 * k[at]) - 1.0).abs() < 1e-12,
        "k {k_zeroed} linear to a zero node"
    );
}

/// A file whose grid does not reach a band, or whose n at a band is below 1, is refused by the
/// band; and a manifest whose `materials` are not the rows' files is refused before any is read.
#[test]
fn substance_albedos_an_index_off_its_grid_or_below_unity_is_refused() {
    let water = by_key("H2O").unwrap();
    let file = manifest().load_material("water-ice.json").unwrap();
    let from_500 = node(&file, 500.0);
    let short = edited_water_ice(|v| {
        for array in ["wavelengthsNm", "n", "k"] {
            v[array].as_array_mut().unwrap().drain(..from_500);
        }
    });
    assert!(matches!(
        row_index(water, &short),
        Err(FitAlbedoError::OffGrid { file: "water-ice.json", wavelength_nm })
            if wavelength_nm.total_cmp(&440.0).is_eq()
    ));
    let at_550 = node(&file, 550.0);
    let thin = edited_water_ice(|v| v["n"][at_550] = serde_json::Value::from(0.9));
    assert!(matches!(
        row_index(water, &thin),
        Err(FitAlbedoError::IndexBelowUnity { file: "water-ice.json", wavelength_nm, n })
            if wavelength_nm.total_cmp(&550.0).is_eq() && n.total_cmp(&0.9).is_eq()
    ));
    let text = std::fs::read_to_string(manifest_path()).unwrap();
    let without_water = text.replace("    \"water-ice.json\",\n", "");
    assert_ne!(without_water, text);
    let manifest = Manifest::from_bytes(&manifest_path(), without_water.into_bytes()).unwrap();
    assert!(matches!(fit(&manifest), Err(RunTaskError::Input { .. })));
}

/// The table's rows are the registry's rows with condensed optics, in registry order, each read
/// from its own solid file by wavelength, the manifest listing those files; a row's provenance is
/// at least as strict as its file's flag; and a row with no optics, a row whose optics read no
/// index file, or a file not its own is refused.
#[test]
fn substance_albedos_the_items_are_the_registry_s_optics_rows() {
    let manifest = manifest();
    assert_eq!(manifest.materials(), row_files().as_slice());
    let names: Vec<String> = fitted().iter().map(|(id, _)| item_name(*id)).collect();
    assert_eq!(names, ITEMS);
    let table = render(fitted().to_vec());
    assert_eq!(table.item_names(), ITEMS);
    for (id, optics) in substance::optics::rows() {
        let Some(name) = optics.index_file() else {
            continue;
        };
        let file = manifest.load_material(name).unwrap();
        assert!(row_index(id, &file).is_ok());
        if file.provenance() == "standIn" {
            assert!(optics.is_stand_in(), "{}", file.key());
        }
    }
    let nitrogen = manifest.load_material("nitrogen-ice.json").unwrap();
    assert!(matches!(
        row_index(by_key("He").unwrap(), &nitrogen),
        Err(FitAlbedoError::NoOptics { .. })
    ));
    assert_eq!(
        row_index(by_key("S8").unwrap(), &nitrogen),
        Err(FitAlbedoError::NoIndexFile {
            key: "S8".to_owned()
        })
    );
    assert_eq!(
        row_index(by_key("H2O").unwrap(), &nitrogen),
        Err(FitAlbedoError::NotItsFile {
            key: "H2O".to_owned(),
            file: "water-ice.json",
            found_key: "N2".to_owned(),
            found_phase: "solid".to_owned(),
        })
    );
}

/// The acceptance line is written only when it is true: the fit's rows pass, and a row whose
/// `A_N` passes 1.35 or whose `A_h` passes 1 is refused, as is no row at all.
#[test]
fn substance_albedos_the_acceptance_refuses_a_fit_past_its_bounds() {
    let rows = fitted().to_vec();
    let line = acceptance(&rows).unwrap();
    assert!(
        line.starts_with("55 rows (11 frosts, 44 regoliths)"),
        "{line}"
    );
    let mut bright = rows.clone();
    bright[0].1[4] = 1.4;
    assert!(matches!(
        acceptance(&bright),
        Err(RunTaskError::Input { .. })
    ));
    let mut over = rows.clone();
    over[0].1[9] = 1.01;
    assert!(matches!(acceptance(&over), Err(RunTaskError::Input { .. })));
    let mut binned = rows;
    binned[0].1[SPECTRAL_BINS_OFFSET + 7] = 1.01;
    assert!(matches!(
        acceptance(&binned),
        Err(RunTaskError::Input { .. })
    ));
    assert!(matches!(acceptance(&[]), Err(RunTaskError::Input { .. })));
}

/// The committed table is this fit, bit for bit, row by row, every value of each row through the
/// sim's accessors.
#[test]
fn substance_albedos_the_committed_table_is_the_fit() {
    for &(id, values) in fitted() {
        let optics = id.substance().condensed_optics().unwrap();
        let committed: Vec<f64> = [
            optics.single_scattering_albedo_bvr(),
            optics.normal_albedo_bvr(),
            optics.spherical_albedo_bvr(),
        ]
        .into_iter()
        .flatten()
        .chain((0..4).flat_map(|i| optics.directional_hemispherical_albedo_bvr(i).unwrap()))
        .chain(optics.spherical_albedo_bins())
        .chain(optics.data_reach_nm())
        .collect();
        assert_eq!(committed.len(), values.len());
        for (a, b) in committed.into_iter().zip(values) {
            assert!(
                a.total_cmp(&b).is_eq(),
                "{}: {a} against {b}",
                id.substance().key()
            );
        }
    }
}

/// Every row conserves energy (decision-r10-limb §4, as decision-p14-t49c-split §1.2 keeps it): its
/// IMSA directional-hemispherical albedo, from the same w, phase function and opposition amplitude
/// as its `A_N`, is at most 1 at the four incidences in every band, and its `A_N` at most 1.35,
/// R10's particulate ceiling; its tabled energy albedos, the similarity relation's, are in (0, 1];
/// each w is in [0, 1], and a regolith's at most the lunar photometry's limit.
#[test]
fn substance_albedos_every_row_conserves_energy() {
    for &(id, values) in fitted() {
        let key = id.substance().key();
        let form = id.substance().condensed_optics().unwrap().reference_form();
        for band in 0..3 {
            let w = values[band];
            assert!((0.0..=1.0).contains(&w), "{key}: w {w}");
            if form == ReferenceForm::Regolith {
                assert!(w <= REGOLITH_MAX_SINGLE_SCATTERING_ALBEDO, "{key}: w {w}");
            }
            let imsa = Medium::new(w, form.photometry());
            for mu0 in HEMISPHERICAL_INCIDENCE_COSINES {
                let a_h = imsa.directional_hemispherical_albedo(mu0);
                assert!(a_h <= 1.0, "{key}: the IMSA's A_h {a_h} at mu0 {mu0}");
            }
            let a_n = values[3 + band];
            assert!(
                a_n > 0.0 && a_n <= PARTICULATE_NORMAL_ALBEDO_CEILING,
                "{key}: A_N {a_n}"
            );
            let sphere = values[6 + band];
            assert!(sphere > 0.0 && sphere <= 1.0, "{key}: spherical {sphere}");
            for incidence in 0..4 {
                let a_h = values[9 + 3 * incidence + band];
                assert!(
                    a_h > 0.0 && a_h <= 1.0,
                    "{key}: A_h {a_h} at incidence {incidence}"
                );
            }
        }
    }
}

/// The energy albedos conserve energy and match the measured snow: a lossless row's tabled
/// directional-hemispherical and spherical albedos are 1 to 10⁻⁶ at every incidence, `H2O`'s in V
/// at μ₀ = 0.5 lies within 0.95–1.0 (fresh snow measures 0.96–0.98, Grenfell, Warren and Mullen
/// 1994, abstract, as R10's `FRESH_SNOW_V` cites it; pure ice a little above, Wiscombe and Warren
/// 1980's abstract putting their pure-snow model up to 15% above measured snow, from trace
/// impurities), and an isotropic grain's similar medium is itself.
#[test]
fn substance_albedos_the_energy_albedos_match_lossless_and_measured_snow() {
    let row = fitted_row;
    for key in ["CH4", "NH3", "N2", "O2", "Ar", "CO", "H2S", "NH4SH"] {
        let values = row(key);
        for (offset, &value) in values.iter().enumerate().take(BVR_ROW_LEN).skip(6) {
            assert!(
                (value - 1.0).abs() < 1e-6,
                "{key}: value {offset} is {value}"
            );
        }
    }
    for key in ["O2", "Ar", "H2S", "NH4SH"] {
        let values = row(key);
        for (offset, &value) in values[SPECTRAL_BINS_OFFSET..DATA_REACH_OFFSET]
            .iter()
            .enumerate()
        {
            assert!((value - 1.0).abs() < 1e-6, "{key}: bin {offset} is {value}");
        }
    }
    let water = row("H2O");
    let a_h_v_half = water[9 + 3 + 1];
    assert!(
        (0.95..=1.0).contains(&a_h_v_half),
        "H2O's A_h in V at 0.5: {a_h_v_half}"
    );
    for w in [0.3, 0.9] {
        assert!(similar_isotropic_albedo(w, 0.0).total_cmp(&w).is_eq());
        let (plane, _) = energy_albedos(w, 0.0);
        let imsa = Medium::from_parts(w, PhaseFunction::Legendre { b: 0.0, c: 0.0 }, 0.0, 1.0);
        for (mu0, a_h) in HEMISPHERICAL_INCIDENCE_COSINES.into_iter().zip(plane) {
            assert!((imsa.directional_hemispherical_albedo(mu0) - a_h).abs() < 1e-7);
        }
    }
    assert!(similar_isotropic_albedo(1.0, 0.503).total_cmp(&1.0).is_eq());
    assert!(similar_isotropic_albedo(0.9, 0.503) < 0.9);
    assert!(similar_isotropic_albedo(0.9, -0.11) > 0.9);
}

/// Van de Hulst's exact spherical albedos of semi-infinite media (1974, A&A 35, 209, Table 1,
/// p. 212, isotropic, and Table 2, p. 213, example B's isotropic column): (a, A*).
const VAN_DE_HULST_ISOTROPIC: [(f64, f64); 5] = [
    (0.975, 0.69500),
    (0.9, 0.47802),
    (0.5, 0.14652),
    (0.1, 0.02170),
    (0.96, 0.6306),
];

/// Van de Hulst's exact spherical albedos of Henyey–Greenstein media (1974, Table 1, p. 212, and
/// Table 2's examples A and B, p. 213): (a, g, A*).
const VAN_DE_HULST_SPHERICAL: [(f64, f64, f64); 4] = [
    (0.9, 1.0 / 3.0, 0.40897),
    (0.6, 0.25, 0.1580),
    (0.9, 0.875, 0.1568),
    (0.99, 0.75, 0.6348),
];

/// Van de Hulst's exact plane albedos of the same media (1974, Table 2, p. 213): (a, g, r(1),
/// r(0.5), the bound this task's similar plane albedo is held to at both).
const VAN_DE_HULST_PLANE: [(f64, f64, f64, f64, f64); 3] = [
    (0.6, 0.25, 0.1125, 0.1781, 0.015),
    (0.9, 0.875, 0.0865, 0.1836, 0.045),
    (0.99, 0.75, 0.5592, 0.6710, 0.02),
];

/// The similarity relation meets van de Hulst 1974's exact media: the isotropic plane and
/// spherical albedos within 6 × 10⁻⁵ of his printed values (Table 2's example B at a = 0.96: r(1)
/// 0.5713, r(0.5) 0.6593; his Table 1's 0.14652 at a = 0.5 is 2.4 × 10⁻⁵ below the exact
/// 0.146544), and a Henyey–Greenstein medium's spherical albedo within 0.004, the sum of his two
/// "at most 0.002" (p. 209), since the relation here reads the exact isotropic A* at the same s
/// where he fits both to one z(s) (here +0.0009, +0.0016, +0.0028 and +0.0001). His plane albedo
/// is not similar (§4, p. 213), and this task's directional-hemispherical albedo of an absorbing
/// anisotropic grain carries the miss: r(1) +0.014 and +0.040 (46%) and r(0.5) −0.004 and −0.009
/// in the far example A, at g 0.25 and 0.875, and in the nearly conservative example B, at a 0.99
/// and g 0.75, r(1) +0.017 (3%) and r(0.5) −0.0075 (1%), growing toward grazing incidence (−0.040
/// at μ 0.1 and −0.060 at 0, which this test does not hold).
#[test]
fn substance_albedos_the_similarity_relation_meets_van_de_hulst_s_tables() {
    for (a, sphere) in VAN_DE_HULST_ISOTROPIC {
        let (_, ours) = energy_albedos(a, 0.0);
        assert!(
            (ours - sphere).abs() < 6e-5,
            "isotropic a {a}: A* {ours} against {sphere}"
        );
    }
    let (plane, _) = energy_albedos(0.96, 0.0);
    assert!((plane[0] - 0.5713).abs() < 6e-5, "r(1) {}", plane[0]);
    assert!((plane[1] - 0.6593).abs() < 6e-5, "r(0.5) {}", plane[1]);
    for (a, g, sphere) in VAN_DE_HULST_SPHERICAL {
        let (_, ours) = energy_albedos(a, g);
        assert!(
            (ours - sphere).abs() < 0.004,
            "a {a}, g {g}: A* {ours} against {sphere}"
        );
    }
    for (a, g, at_one, at_half, bound) in VAN_DE_HULST_PLANE {
        let (plane, _) = energy_albedos(a, g);
        assert!(
            (plane[0] - at_one).abs() < bound,
            "a {a}, g {g}: r(1) {}",
            plane[0]
        );
        assert!(
            (plane[1] - at_half).abs() < bound,
            "a {a}, g {g}: r(0.5) {}",
            plane[1]
        );
    }
}

/// The fingerprint probes the bands, the incidences, every reference form's constants, evaluated
/// photometry and asymmetry, and every row's form, all finite, so that a changed constant or
/// formula of the sim's finds the table stale.
#[test]
fn substance_albedos_the_fingerprint_reads_the_sim_s_constants() {
    let fingerprint = SubstanceAlbedosTask.fingerprint();
    let rows = substance::optics::rows().count();
    assert_eq!(
        fingerprint.probes().len(),
        3 + 4 + 8 + 11 + rows + SPECTRAL_BIN_EDGES_NM.len() + rows + 6 + 4
    );
    assert!(fingerprint.values().iter().all(|v| v.is_finite()));
    let probe = |name: &str| {
        fingerprint
            .probes()
            .iter()
            .find(|(p, _)| p == name)
            .map_or_else(|| panic!("{name} is probed"), |&(_, v)| v)
    };
    assert!(probe("H2O reference form").total_cmp(&0.0).is_eq());
    assert!(probe("basalt reference form").total_cmp(&1.0).is_eq());
    assert!((probe("Regolith asymmetry") + 0.11).abs() < 1e-12);
    assert!(probe("bin edge 5 nm").total_cmp(&780.0).is_eq());
    assert!(probe("H2O infrared set").total_cmp(&1.0).is_eq());
    assert!(probe("O2 infrared set").total_cmp(&0.0).is_eq());
    assert!(probe("mars_dust infrared set").total_cmp(&13.0).is_eq());
    assert!((probe("fine grain diameter m") - 1.0e-6).abs() < 1e-18);
    for word in 0..4 {
        let value = probe(&format!("row inputs hash word {word}"));
        assert!((1.0..=65_536.0).contains(&value), "{value}");
    }
    let mut descriptions: Vec<String> = substance::optics::rows()
        .map(|(_, optics)| describe_input(optics.input()))
        .collect();
    assert!(
        descriptions.iter().all(|d| !d.contains("Kokaly")),
        "no prose"
    );
    let rows_described = descriptions.len();
    descriptions.dedup();
    assert!(descriptions.len() > rows_described / 2);
    for (place, set) in INFRARED_SETS.into_iter().enumerate() {
        assert!(
            !INFRARED_SETS[..place].contains(&set),
            "{set:?} is listed once"
        );
    }
}

/// The manifest's datasets are the rows' infrared sets', in the order the rows first name them;
/// another list is refused before any is read.
#[test]
fn substance_albedos_the_datasets_are_the_rows_infrared_sets() {
    assert_eq!(manifest().datasets(), row_datasets().as_slice());
    let text = std::fs::read_to_string(manifest_path()).unwrap();
    let without = text.replace("    \"grundy_1993_n2\",\n", "");
    assert_ne!(without, text);
    let manifest = Manifest::from_bytes(&manifest_path(), without.into_bytes()).unwrap();
    assert!(matches!(fit(&manifest), Err(RunTaskError::Input { .. })));
}

/// The cache of the isotropic spherical albedo is within 10⁻⁷ of `energy_albedos`'s at any w, for
/// both reference forms' asymmetries, between its nodes as at them, and exact at w = 1.
#[test]
fn substance_albedos_the_spherical_albedo_cache_is_exact_to_its_bound() {
    let cache = spectral::SphericalAlbedoCache::new();
    let mut worst: f64 = 0.0;
    for form in [ReferenceForm::Frost, ReferenceForm::Regolith] {
        let xi = form.photometry().phase_function().asymmetry();
        for j in 0..=400_u32 {
            // Uniform in s = √(1 − w), where the cache's grid is, and so between its nodes.
            let s = (f64::from(j) + 0.37) / 400.37;
            let w = 1.0 - s * s;
            let exact = energy_albedos(w, xi).1;
            worst = worst.max((cache.spherical_albedo(w, xi) - exact).abs());
        }
        assert!(cache.spherical_albedo(1.0, xi).total_cmp(&1.0).is_eq());
    }
    assert!(worst < 1e-7, "the cache's largest error {worst}");
}

/// The bins converge on the grid: halving its step moves no bin of any row by more than 10⁻⁴.
#[test]
fn substance_albedos_the_bins_converge_on_the_grid() {
    let manifest = manifest();
    let coarse = fit_on_grid(&manifest, spectral::SPECTRAL_STEP_NM).unwrap();
    let fine = fit_on_grid(&manifest, spectral::SPECTRAL_STEP_NM / 2.0).unwrap();
    let mut worst = (0.0, String::new(), 0);
    for ((id, a), (_, b)) in coarse.iter().zip(&fine) {
        for bin in 0..SPECTRAL_BINS {
            let moved = (a[SPECTRAL_BINS_OFFSET + bin] - b[SPECTRAL_BINS_OFFSET + bin]).abs();
            if moved > worst.0 {
                worst = (moved, id.substance().key().as_str().to_owned(), bin);
            }
        }
    }
    assert!(worst.0 < 1e-4, "halving the step moves {worst:?}");
}

/// Each row's index, as the bins read it, is its R08 file's own over 380–780 nm: the file's piece
/// gives `index_at`'s n and k at and between the file's nodes, to the bit.
#[test]
fn substance_albedos_the_visible_piece_is_the_file_s_index() {
    let manifest = manifest();
    for (_, optics) in substance::optics::rows() {
        let Some(name) = optics.index_file() else {
            continue;
        };
        let file = manifest.load_material(name).unwrap();
        let piece = infrared::visible_piece(name, &file).unwrap();
        for nm in [380.0, 437.5, 550.0, 641.25, 777.0, 780.0] {
            let (n, k) = index_at(&file, nm).unwrap();
            let (pn, pk) = piece.at(nm);
            assert!(
                pn.total_cmp(&n).is_eq() && pk.total_cmp(&k).is_eq(),
                "{name} at {nm} nm"
            );
        }
    }
}

/// The step in k where each infrared set joins its file at 780 nm, the set's k over the file's,
/// where the file's k is not 0: 1 where the set is the file's own source, and Warren 1986's
/// impure dry ice over Hansen 2005's continuum for `CO2` (the row's source). Where the file's k is
/// 0, the set's k at 780 nm is stated instead (`CH4`'s, 3.54 × 10⁻⁸, in Grundy et al. 2002's 0.79
/// µm band).
const STEPS_AT_780_NM: [(&str, f64); 9] = [
    ("H2O", 1.0),
    ("CO2", 2_371.0),
    ("ZnS", 1.0),
    ("MgSiO3", 1.0),
    ("Mg2SiO4", 1.0),
    ("tholin", 1.0),
    ("basalt", 1.0),
    ("lava_glass", 1.0),
    ("mars_dust", 1.0),
];

/// Each infrared set at 780 nm: its n within 1% of its R08 file's, and its k continuous with the
/// file's, or its step at the stated size ([`STEPS_AT_780_NM`]); where the file's k is 0, the set's
/// is 0 too (`N2`, `CO`, `NH3`) or `CH4`'s stated 3.54 × 10⁻⁸. `H2O`'s joins inside its set, where
/// Grundy and Schmitt 1998's 100 K ice meets Warren and Brandt 2008's 266 K ice, are at their
/// stated sizes: k 2.64 times Warren and Brandt's at 962.3 nm, and 0.561 times at 2,742 nm. `CO`'s
/// n steps by −0.46% at 3,333.5 nm, where Gerakines et al.'s table begins.
#[test]
fn substance_albedos_each_infrared_set_joins_its_file_at_780_nm() {
    let manifest = manifest();
    let data = infrared::InfraredData::load(&manifest).unwrap();
    let ratio = |a: f64, b: f64| a / b;
    for (id, optics) in substance::optics::rows() {
        let key = id.substance().key();
        let Some(name) = optics.index_file() else {
            continue;
        };
        let file = manifest.load_material(name).unwrap();
        let spectral = infrared::spectral_index(optics, name, &file, &data).unwrap();
        let (n_file, k_file) = index_at(&file, 780.0).unwrap();
        let Some(_) = optics.infrared_source() else {
            assert_eq!(spectral.pieces().len(), 1, "{key}");
            continue;
        };
        let (n_set, k_set) = spectral.at(780.0);
        assert!(
            (n_set / n_file - 1.0).abs() < 0.01,
            "{key}: n {n_set} against {n_file}"
        );
        if k_file > 0.0 {
            let step = STEPS_AT_780_NM
                .iter()
                .find(|(k, _)| *k == key.as_str())
                .map_or_else(|| panic!("{key}'s step is stated"), |s| s.1);
            let found = ratio(k_set, k_file);
            assert!(
                (found / step - 1.0).abs() < 1e-3,
                "{key}: k steps by {found}, not {step}"
            );
        } else if key.as_str() == "CH4" {
            assert!((k_set / 3.544e-8 - 1.0).abs() < 1e-3, "{key}: k {k_set}");
        } else {
            assert!(k_set.total_cmp(&0.0).is_eq(), "{key}: k {k_set}");
        }
    }
    let water = by_key("H2O")
        .unwrap()
        .substance()
        .condensed_optics()
        .unwrap();
    let file = manifest.load_material("water-ice.json").unwrap();
    let spectral = infrared::spectral_index(water, "water-ice.json", &file, &data).unwrap();
    let pieces = spectral.pieces();
    // The file's piece, then Warren and Brandt's, Grundy and Schmitt's, and Warren and Brandt's.
    for (join, expected) in [(pieces[2].lo_nm(), 2.64), (pieces[3].lo_nm(), 0.561)] {
        let found = ratio(pieces[2].at(join).1, pieces[1].at(join).1);
        assert!(
            (found / expected - 1.0).abs() < 2e-3,
            "H2O at {join} nm: {found}"
        );
    }
    let co = by_key("CO")
        .unwrap()
        .substance()
        .condensed_optics()
        .unwrap();
    let file = manifest.load_material("carbon-monoxide-ice.json").unwrap();
    let spectral = infrared::spectral_index(co, "carbon-monoxide-ice.json", &file, &data).unwrap();
    // The file's piece, then k = 0, then Gerakines et al.'s.
    let join = spectral.pieces()[2].lo_nm();
    let step = spectral.at(join).0 / spectral.pieces()[1].at(join).0 - 1.0;
    assert!(
        (step + 0.0046).abs() < 1e-4,
        "CO's n steps by {step} at {join} nm"
    );
}

/// The rows whose data put n below 1, inside a strong band, where the slab's extension
/// ([`spectral::spectral_single_scattering_albedo`]) reads them: the ices' strong fundamentals
/// alone, `H2O`'s and `NH3`'s 3 µm bands, `CO2`'s 4.27 µm band and `CO`'s 4.67 µm band, and only
/// beyond 2.8 µm.
#[test]
fn substance_albedos_an_index_below_unity_lies_in_the_ices_fundamentals() {
    let manifest = manifest();
    let data = infrared::InfraredData::load(&manifest).unwrap();
    let mut keys = Vec::new();
    for (id, optics) in substance::optics::rows() {
        let Some(name) = optics.index_file() else {
            continue;
        };
        let file = manifest.load_material(name).unwrap();
        let spectral = infrared::spectral_index(optics, name, &file, &data).unwrap();
        for nm in spectral::SpectralGrain::knots_nm(&spectral) {
            if spectral.at(nm).0 < 1.0 {
                assert!(nm > 2_800.0, "{} at {nm} nm", id.substance().key());
                keys.push(id.substance().key().as_str().to_owned());
            }
        }
    }
    keys.dedup();
    assert_eq!(keys, ["H2O", "NH3", "CO2", "CO"]);
}

/// The share of a blackbody's emission at temperature `teff` K below `nm`: (15 ÷ π⁴) Σₙ e^{−nx}
/// (x³ ÷ n + 3x² ÷ n² + 6x ÷ n³ + 6 ÷ n⁴), x = hc ÷ (λ k T), the series of the Planck integral (its
/// terms summed to n = 600, past e⁻²⁹⁰ at 5 µm and 5,772 K), with hc ÷ k from CODATA 2018's exact
/// h, c and `k_B`, as `star_colour::spectrum::planck` takes it.
pub(super) fn blackbody_share_below(nm: f64, teff: f64) -> f64 {
    const HC_OVER_K_NM_K: f64 = 6.626_070_15e-34 * 299_792_458.0 / 1.380_649e-23 * 1.0e9;
    let x = HC_OVER_K_NM_K / (nm * teff);
    let mut sum = 0.0;
    for n in 1..=600_u32 {
        let n = f64::from(n);
        sum += math::exp(-n * x)
            * (x * x * x / n
                + 3.0 * x * x / (n * n)
                + 6.0 * x / (n * n * n)
                + 6.0 / (n * n * n * n));
    }
    15.0 / math::powi(core::f64::consts::PI, 4) * sum
}

/// The Bond albedo of `bins` under a blackbody at `teff` K: each bin weighted by the flux between
/// its edges, the first bin by all the flux below 390 nm and the last by all above 3,500 nm (the
/// first and last bins standing for the light outside 0.3–5 µm).
pub(super) fn bond_albedo(bins: &[f64], teff: f64) -> f64 {
    let edges = SPECTRAL_BIN_EDGES_NM;
    (0..SPECTRAL_BINS)
        .map(|bin| {
            let lo = if bin == 0 {
                0.0
            } else {
                blackbody_share_below(edges[bin], teff)
            };
            let hi = if bin == SPECTRAL_BINS - 1 {
                1.0
            } else {
                blackbody_share_below(edges[bin + 1], teff)
            };
            bins[bin] * (hi - lo)
        })
        .sum()
}

/// The blackbody's shares are the Planck function's: none below 10 nm, and a band's share is
/// `star_colour::spectrum::planck` integrated over it against the whole (the trapezoid rule, 1 nm
/// steps, to 10⁻³).
#[test]
fn substance_albedos_the_blackbody_shares_are_planck_s() {
    use crate::tasks::star_colour::spectrum::planck;
    assert!(blackbody_share_below(10.0, 5_772.0) < 1e-12);
    let (lo, hi) = (390.0, 5_000.0);
    let band: f64 = (0..4_610_u32)
        .map(|i| {
            let a = lo + f64::from(i);
            f64::midpoint(planck(a, 5_772.0), planck(a + 1.0, 5_772.0))
        })
        .sum();
    let total: f64 = (1..2_000_000_u32)
        .map(|i| planck(f64::from(i) * 0.05 + 50.0, 5_772.0) * 0.05)
        .sum();
    let share = blackbody_share_below(hi, 5_772.0) - blackbody_share_below(lo, 5_772.0);
    assert!(
        (band / total - share).abs() < 1e-3,
        "{} against {share}",
        band / total
    );
}

/// The anchor the infrared already sets at the row (plan 14, P14.T49.i; decision-p14-t49c-split
/// §1.3): `H2O`'s Bond albedo under a 5,772 K blackbody, the Sun's effective temperature (0.832),
/// against Enceladus's measured bolometric Bond albedo, 0.81 ± 0.04 (Howett et al. 2010, from
/// memory: its abstract gives no number, and the paper was not reached) and 0.76 ± 0.03 (Li et al.
/// 2023, Icarus 394, 115429, its abstract read in ADS, 2023Icar..39415429L): within Howett et al.'s
/// range as remembered, and not below Li et al.'s, pure fresh frost of 100 µm grains being an upper
/// bound on an icy satellite's, whose grains, impurities and weathering darken it (P14.T51.b
/// asserts the body's). Under a 3,000 K blackbody, an M dwarf's, the same frost's is 0.528: most of
/// the light is then beyond 1 µm, where ice absorbs (decision-p14-t49c-split §5.2). The row's B, V
/// and R bins stay above 0.98.
#[test]
fn substance_albedos_water_frost_s_bond_albedo_meets_enceladus_s() {
    let water = fitted_row("H2O");
    let bins = &water[SPECTRAL_BINS_OFFSET..DATA_REACH_OFFSET];
    let sun = bond_albedo(bins, 5_772.0);
    assert!((0.77..=0.85).contains(&sun), "H2O's Bond albedo {sun}");
    assert!(sun >= 0.76 - 0.03, "H2O's Bond albedo {sun}");
    let m_dwarf = bond_albedo(bins, 3_000.0);
    assert!(
        m_dwarf < sun - 0.2,
        "H2O's Bond albedo at 3,000 K {m_dwarf}"
    );
    assert!(BVR_BINS.iter().all(|&bin| bins[bin] > 0.98), "{bins:?}");
}

/// T51.a's by-hand Pluto and Triton, checked at the rows (plan 14, P14.T49.i;
/// decision-p14-t49c-split §1.3): each of the ices their fixtures give by hand, `N2`, `CH4` and
/// `CO`, has a Bond albedo under a 5,772 K blackbody at or above the top of both bodies' ranges,
/// Pluto's 0.6–0.8 (Buratti et al. 2017's 0.72 ± 0.07) and Triton's 0.72–0.92 (Hillier et al.
/// 1990's 0.82 ± 0.05; both read in decision-triton-and-e101 §1.6), so that the frosts' infrared
/// leaves both reachable. Pure fresh frost of 100 µm grains is an upper bound on a body's: a mix of
/// these three alone lies above Pluto's range, and P14.T51.b asserts the bodies, with what darkens
/// them.
#[test]
fn substance_albedos_the_outer_frosts_bound_pluto_and_triton_from_above() {
    for key in ["N2", "CH4", "CO"] {
        let row = fitted_row(key);
        let sun = bond_albedo(&row[SPECTRAL_BINS_OFFSET..DATA_REACH_OFFSET], 5_772.0);
        for (body, top) in [("Pluto", 0.8), ("Triton", 0.92)] {
            assert!(
                sun >= top,
                "{key}'s Bond albedo {sun} against {body}'s range's top {top}"
            );
        }
    }
}
