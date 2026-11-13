//! The space-weathering fit's tests on synthetic and committed inputs: the slab against
//! `substance_albedos`', the nodes, the columns' monotony and bound, iron's join, the inversion of
//! a row's own albedo, the recovery of a known Λ, and the fit's law against the surface crate's to
//! the bit. The tests of the real fit against the Moon's anchors are the crate's
//! `tests/space_weathering.rs`.

use std::sync::OnceLock;

use hyperion_sim::units::Metres;
use hyperion_surface::material::{PhaseLaw, weathered_normal_albedo};
use hyperion_surface::tables::space_weathering as committed;
use hyperion_testkit::float::{assert_same_bits, bits};

use super::*;
use crate::emit::Workspace;
use crate::tasks::substance_albedos::single_scattering_albedo;

/// Asserts that `actual` and `expected` hold the same values to the bit, element by element.
fn assert_same_bits_each(actual: &[f64], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len(), "the lengths");
    for (&a, &e) in actual.iter().zip(expected) {
        assert_same_bits(a, e);
    }
}

/// The task's committed manifest.
fn repository_manifest() -> Manifest {
    Manifest::load(
        &Workspace::repository()
            .manifests_dir
            .join("space_weathering.toml"),
    )
    .expect("the committed manifest loads")
}

/// Its parameters.
fn params() -> &'static Params {
    static PARAMS: OnceLock<Params> = OnceLock::new();
    PARAMS.get_or_init(|| Params::of(&repository_manifest()).expect("the manifest parses"))
}

/// The committed iron datasets, Johnson and Christy's and Ordal et al.'s.
fn indices() -> &'static (IronIndex, IronIndex) {
    static INDICES: OnceLock<(IronIndex, IronIndex)> = OnceLock::new();
    INDICES.get_or_init(|| {
        let manifest = repository_manifest();
        (
            load_index(&manifest, NEAR_DATASET, NEAR_FILE).expect("Johnson and Christy read"),
            load_index(&manifest, FAR_DATASET, FAR_FILE).expect("Ordal et al. read"),
        )
    })
}

/// The tables on the manifest's grids, computed once (a few seconds).
fn tables() -> &'static Tables {
    static TABLES: OnceLock<Tables> = OnceLock::new();
    TABLES.get_or_init(|| {
        let (near, far) = indices();
        Tables::new(params().nodes, params().grid, near, far).expect("the tables compute")
    })
}

/// The slab here, at k = 0, with `substance_albedos`' Θ and mean path, gives its w to 10⁻¹² for
/// grains whose k is small enough that k² leaves R(0) unmoved (k ≤ 3 × 10⁻⁷, k² ≤ 10⁻¹³): the two
/// tasks share Hapke 2012's fits and the mean path.
#[test]
fn space_weathering_slab_gives_substance_albedos_w() {
    for n in [1.31, 1.5, 1.7, 2.1] {
        let [s_e, s_i] = slab(n);
        for k in [0.0, 1e-8, 1e-7, 3e-7] {
            for wavelength_nm in [440.0, 550.0, 640.0, 2_200.0] {
                for diameter_m in [70e-6, 1e-3, 1e-2] {
                    let alpha = 4.0 * core::f64::consts::PI * k / (wavelength_nm * 1.0e-9);
                    let theta = math::exp(-alpha * mean_path(n, diameter_m));
                    let w = s_e + (1.0 - s_e) * (1.0 - s_i) * theta / (1.0 - s_i * theta);
                    let theirs =
                        single_scattering_albedo(n, k, wavelength_nm, Metres::new(diameter_m));
                    assert!(
                        (w - theirs).abs() < 1e-12,
                        "n {n}, k {k}, {wavelength_nm} nm, D {diameter_m} m: {w} against {theirs}"
                    );
                }
            }
        }
    }
    // The regolith's host index gives the ruling's slab (decision-p14-t51d-weathering's addendum:
    // S_e 0.1165 and S_i 0.6943 at n_h 1.7) and its 63.5 µm mean path for 70 µm grains.
    let [s_e, s_i] = slab(REGOLITH_HOST_INDEX);
    assert!(
        (s_e - 0.1165).abs() < 5e-5 && (s_i - 0.6943).abs() < 5e-5,
        "{s_e} {s_i}"
    );
    let path = form_mean_path(ReferenceForm::Regolith);
    assert!((path - 63.5e-6).abs() < 0.05e-6, "{path}");
}

/// The nodes run from 0 to 1 exactly, rising, at equal steps in √(1 − w).
#[test]
fn space_weathering_nodes_are_uniform_in_gamma() {
    let w = nodes(129);
    assert_eq!(w.len(), 129);
    assert_same_bits(w[0], 0.0);
    assert_same_bits(w[128], 1.0);
    assert!(w.windows(2).all(|pair| pair[0] < pair[1]));
    for pair in w.windows(2) {
        let step = (1.0 - pair[0]).sqrt() - (1.0 - pair[1]).sqrt();
        assert!((step - 1.0 / 128.0).abs() < 1e-15, "{step}");
    }
}

/// Both forms' columns rise strictly from 0 and the energy albedos stay within 1, so the law can
/// invert each; a column that does not rise, or passes 1, is refused.
#[test]
fn space_weathering_columns_rise_and_stay_within_one() {
    for (form, tables) in [
        (ReferenceForm::Regolith, &tables().regolith),
        (ReferenceForm::Frost, &tables().frost),
    ] {
        let columns = &tables.columns;
        assert_eq!(check_columns(form, columns), Ok(()), "{form:?}");
        assert_same_bits(columns.normal[0], 0.0);
        assert_same_bits(columns.spherical[0], 0.0);
        assert!(columns.spherical.iter().all(|&a| (0.0..=1.0).contains(&a)));
        assert!(
            columns
                .hemispherical
                .iter()
                .flatten()
                .all(|&a| (0.0..=1.0).contains(&a))
        );
        eprintln!(
            "{form:?}: A_N at w = 1 {:.4}, A_S {:.4}, A_h {:?}",
            columns.normal[128], columns.spherical[128], columns.hemispherical[128]
        );
    }
    let mut flat = tables().regolith.columns.clone();
    flat.normal[40] = flat.normal[39];
    assert!(matches!(
        check_columns(ReferenceForm::Regolith, &flat),
        Err(FitWeatheringError::NotIncreasing { node: 40, .. })
    ));
    let mut nan = tables().regolith.columns.clone();
    nan.spherical[7] = f64::NAN;
    assert!(matches!(
        check_columns(ReferenceForm::Regolith, &nan),
        Err(FitWeatheringError::NotIncreasing { .. })
    ));
    let mut bright = tables().frost.columns.clone();
    bright.hemispherical[128][3] = 1.000_001;
    assert!(matches!(
        check_columns(ReferenceForm::Frost, &bright),
        Err(FitWeatheringError::AboveOne { node: 128, .. })
    ));
}

/// The reader takes the committed files' rows, Johnson and Christy's 49 over 188–1,937 nm and
/// Ordal et al.'s 52 over 667–286,000 nm, and refuses a file it cannot read.
#[test]
fn space_weathering_reads_the_iron_files_and_refuses_bad_ones() {
    let (near, far) = indices();
    assert_eq!(near.len(), 49);
    assert_eq!(far.len(), 52);
    assert!((near.first_nm() - 188.0).abs() < 1e-9 && (near.last_nm() - 1_937.0).abs() < 1e-9);
    assert!((far.first_nm() - 667.0).abs() < 1e-9 && (far.last_nm() - 286_000.0).abs() < 1e-6);
    // A row's own values at its wavelength; ln k linear between rows.
    let (n, k) = near.at(near.last_nm()).unwrap();
    assert!(
        (n - 3.17).abs() < 1e-12 && (k - 6.12).abs() < 1e-12,
        "{n} {k}"
    );
    let (n, k) = near.at(f64::midpoint(1_393.0, 1_610.0)).unwrap();
    assert!((n - 3.10).abs() < 1e-12, "{n}");
    assert!((k - (4.83_f64 * 5.39).sqrt()).abs() < 1e-12, "{k}");
    assert_eq!(near.at(100.0), None);
    assert_eq!(near.at(2_000.0), None);
    let read = |text| IronIndex::from_refractiveindex_yml(text, NEAR_DATASET);
    let why = |text| match read(text) {
        Err(FitWeatheringError::ReadIndex { why, .. }) => why,
        other => panic!("{other:?}"),
    };
    assert_eq!(
        why("DATA:\n  - type: tabulated nk\n"),
        "it has no `data: |` block"
    );
    assert_eq!(
        why("DATA:\n  data: |\n    0.4 2.0 x\n    0.5 2.1 3.0\n"),
        "a row is not numbers"
    );
    assert_eq!(
        why("DATA:\n  data: |\n    0.4 2.0\n    0.5 2.1 3.0\n"),
        "a row is not λ, n and k"
    );
    assert_eq!(
        why("DATA:\n  data: |\n    0.4 2.0 0.0\n    0.5 2.1 3.0\n"),
        "a row's values are not finite, or its k is not positive"
    );
    assert_eq!(
        why("DATA:\n  data: |\n    0.5 2.0 3.0\n    0.4 2.1 3.0\n"),
        "its wavelengths do not rise"
    );
    assert_eq!(
        why("DATA:\n  data: |\n    0.5 2.0 3.0\n  other: 1\n"),
        "it holds fewer than two rows"
    );
    // The block ends where its indentation does.
    let two =
        read("DATA:\n  data: |\n    0.4 2.0 3.0\n\n    0.5 2.1 3.1\n  other: 0.6 1 1\n").unwrap();
    assert_eq!(two.len(), 2);
}

/// Iron's z is continuous where Ordal et al.'s scaled values meet Johnson and Christy's, the join
/// factor is the stated one (about 1.18 at the regolith's index), and z ÷ λ falls from B to R, so
/// the law reddens.
#[test]
fn space_weathering_the_z_grid_is_continuous_at_the_join() {
    let (near, far) = indices();
    for (form, own) in [
        (ReferenceForm::Regolith, &tables().regolith),
        (ReferenceForm::Frost, &tables().frost),
    ] {
        let n_h = host_index(form);
        let join = own.iron.join_nm;
        let below = z_of(near, NEAR_DATASET, n_h, join).unwrap();
        let above = z_of(far, FAR_DATASET, n_h, join).unwrap() * own.iron.join_factor;
        assert!(
            ((above - below) / below).abs() < 1e-12,
            "{form:?}: {below} against {above}"
        );
        assert!(
            (1.1..1.3).contains(&own.iron.join_factor),
            "{form:?}: {}",
            own.iron.join_factor
        );
        assert!(own.iron.z.iter().all(|z| z.is_finite() && *z > 0.0));
        let law = Law {
            nodes: &tables().nodes,
            grid: tables().grid,
            form: own,
            lambda_m: 1e-6,
        };
        let per_nm = |nm: f64| law.iron_z_at(nm) / nm;
        assert!(per_nm(440.0) > per_nm(550.0) && per_nm(550.0) > per_nm(640.0));
        // The ends are held.
        assert_same_bits(law.iron_z_at(200.0), own.iron.z[0]);
        assert_same_bits(law.iron_z_at(9_000.0), own.iron.z[own.iron.z.len() - 1]);
        eprintln!(
            "{form:?}: join at {join} nm × {:.6}; 36π z ÷ λ at 440/550/640 nm {:.4e}/{:.4e}/{:.4e} \
             m⁻¹",
            own.iron.join_factor,
            36.0 * core::f64::consts::PI * per_nm(440.0) * 1e9,
            36.0 * core::f64::consts::PI * per_nm(550.0) * 1e9,
            36.0 * core::f64::consts::PI * per_nm(640.0) * 1e9,
        );
    }
}

/// The largest difference allowed between a grain's own w and the w its `A_N` inverts to on the
/// tables away from the regolith form's knee: 10⁻⁴, above the linear interpolation's error between
/// the nodes there (measured below, about 3 × 10⁻⁵ at most).
const INVERSION_TOLERANCE: f64 = 1e-4;

/// The same next to the knee, where the opposition amplitude B₀ = S(0) ÷ (w p(0)) leaves its cap of
/// 1 at w = S(0) ÷ p(0) = 0.459, so that `A_N`(w) turns there between two nodes and the line
/// between them misses it: 10⁻³, above the measured 5.5 × 10⁻⁴. The law's ratio `A_N`(w) ÷
/// `A_N`(w₀) shares most of it.
const KNEE_INVERSION_TOLERANCE: f64 = 1e-3;

/// A grain's own `A_N` inverts to its own w within the grid's interpolation error: the sim's
/// regolith-form rows, and grains of w 0.05 to 0.999 by the form's own Hapke solution.
#[test]
fn space_weathering_a_row_s_own_normal_albedo_inverts_to_its_own_w() {
    let law = tables().regolith_law(1e-6);
    let photometry = ReferenceForm::Regolith.photometry();
    let knee = match photometry.opposition() {
        OppositionEffect::ShadowHiding { surface_share, .. } => {
            surface_share / photometry.phase_function().at(1.0)
        }
        OppositionEffect::None => unreachable!("the lunar regolith shadow-hides"),
    };
    let nodes = &tables().nodes;
    let above = nodes.partition_point(|&w| w <= knee);
    let (knee_low, knee_high) = (nodes[above - 1], nodes[above]);
    let tolerance = |w: f64| {
        if (knee_low..=knee_high).contains(&w) {
            KNEE_INVERSION_TOLERANCE
        } else {
            INVERSION_TOLERANCE
        }
    };
    let (mut worst, mut worst_at_knee): (f64, f64) = (0.0, 0.0);
    let mut check = |w: f64, back: f64, what: &str| {
        let error = (back - w).abs();
        assert!(error < tolerance(w), "{what}: w {w}, inverted {back}");
        if (knee_low..=knee_high).contains(&w) {
            worst_at_knee = worst_at_knee.max(error);
        } else {
            worst = worst.max(error);
        }
    };
    for (id, row) in optics::rows() {
        if row.reference_form() != ReferenceForm::Regolith {
            continue;
        }
        for (band, name) in BANDS.iter().enumerate() {
            let w = row.single_scattering_albedo_bvr()[band];
            let back = law.grain_albedo_of(row.normal_albedo_bvr()[band]);
            check(w, back, &format!("`{}` in {name}", id.substance().key()));
        }
    }
    for i in 0..=400_u32 {
        let w = 0.05 + 0.949 * f64::from(i) / 400.0;
        let back = law.grain_albedo_of(Medium::new(w, photometry).normal_albedo());
        check(w, back, "a grain");
    }
    eprintln!(
        "the worst inversion: {worst:.2e}, and {worst_at_knee:.2e} between the nodes {knee_low} \
         and {knee_high} about the knee at w {knee:.4}"
    );
}

/// Two synthetic terrains whose anchors are the law's own albedos at a known Λ: the fit recovers
/// that Λ to 10⁻⁹, relative.
#[test]
fn space_weathering_the_fit_recovers_a_known_lambda() {
    let tables = tables();
    let synthetic = |name: &'static str, w: [f64; 3], density_kg_m3, feo_mass_fraction| {
        let law = tables.regolith_law(1e-6);
        Sample {
            terrain: Terrain {
                name,
                row: format!("synthetic {name}"),
                feo_mass_fraction,
                centres: [0.0; 3],
                p10: [0.0; 3],
                p90: [0.0; 3],
            },
            row: FreshRow {
                key: format!("synthetic {name}"),
                w,
                normal: w.map(|w| law.normal_at(w)),
                density_kg_m3,
            },
        }
    };
    for truth in [2.19e-6, 4.0e-7, 1.5e-5] {
        let law = tables.regolith_law(truth);
        let mut samples = [
            synthetic("highland", [0.82, 0.86, 0.88], 2_900.0, 0.055),
            synthetic("mare", [0.55, 0.61, 0.64], 3_310.0, 0.16),
        ];
        for sample in &mut samples {
            let centres = mature(&law, sample);
            sample.terrain.centres = centres;
            sample.terrain.p10 = centres.map(|c| 0.85 * c);
            sample.terrain.p90 = centres.map(|c| 1.15 * c);
        }
        let search = Search {
            lambda_min_m: 1e-8,
            lambda_max_m: 1e-4,
            iterations: 100,
        };
        let found = fit_lambda(tables, &samples, 2.563, search);
        assert!(
            (found / truth - 1.0).abs() < 1e-9,
            "{found} against {truth}"
        );
    }
}

/// The fit's restated law is the surface crate's to the bit at the committed table's Λ, on tables
/// the committed table holds (so this also holds the table's columns, slab and z to the fit's).
#[test]
fn space_weathering_the_fit_s_law_is_the_surface_s_to_the_bit() {
    let tables = tables();
    assert_same_bits_each(&tables.nodes, &committed::W);
    assert_same_bits_each(
        &tables.regolith.columns.normal,
        &committed::REGOLITH_NORMAL_ALBEDO,
    );
    assert_same_bits_each(&tables.regolith.slab, &committed::REGOLITH_SLAB);
    assert_same_bits_each(&tables.regolith.iron.z, &committed::REGOLITH_IRON_Z);
    assert_same_bits(tables.grid.first_nm, committed::IRON_Z_FIRST_NM);
    assert_same_bits(tables.grid.step_nm, committed::IRON_Z_STEP_NM);
    let law = tables.regolith_law(committed::REGOLITH_LAMBDA_M);
    for fresh in [0.0, 0.01, 0.05, 0.093, 0.2, 0.41, 0.667, 1.0, 1.3] {
        for wavelength_nm in [250.0, 300.0, 440.0, 455.5, 550.0, 640.0, 2_200.0, 5_000.0] {
            for feo_mass_fraction in [0.0, 0.055, 0.16, 1.0] {
                for maturity in [0.0, 0.3, 1.0] {
                    for density in [916.7, 3_310.0] {
                        assert_same_bits(
                            law.weathered_normal_albedo(
                                fresh,
                                wavelength_nm,
                                density,
                                feo_mass_fraction,
                                maturity,
                            ),
                            weathered_normal_albedo(
                                fresh,
                                wavelength_nm,
                                PhaseLaw::Regolith,
                                density,
                                feo_mass_fraction,
                                maturity,
                            ),
                        );
                    }
                }
            }
        }
    }
}

/// The committed manifest names the ruled inputs (decision-p14-t51d-weathering's addendum), and
/// the parameters refuse an extra key and an anchor outside its bounds.
#[test]
fn space_weathering_the_manifest_names_the_ruled_inputs() {
    let p = params();
    assert_eq!(p.highland.row, "feldspathic");
    assert_eq!(p.mare.row, "basalt_fe");
    assert_same_bits(p.highland.feo_mass_fraction, 0.055);
    assert_same_bits(p.mare.feo_mass_fraction, 0.16);
    // The anchors are Dollfus and Bowell's 600 nm values times (λ ÷ 600)^0.83, their Eq. 11's
    // slope, from one exponent and with no rounding: decision-r10-t1d-anchor-colour §3's table,
    // to its five places.
    assert_same_bits(p.colour.anchor_wavelength_nm, 600.0);
    assert_same_bits(p.colour.exponent, 0.83);
    let close = |values: [f64; 3], expected: [f64; 3]| {
        for (value, expected) in values.into_iter().zip(expected) {
            assert!(
                (value - expected).abs() < 5e-6,
                "{value} against {expected}"
            );
        }
    };
    close(p.mare.centres, [0.05566, 0.06698, 0.07596]);
    close(p.mare.p10, [0.04793, 0.05768, 0.06541]);
    close(p.mare.p90, [0.06339, 0.07629, 0.08651]);
    close(p.highland.centres, [0.08813, 0.10606, 0.12027]);
    close(p.highland.p10, [0.07421, 0.08931, 0.10128]);
    close(p.highland.p90, [0.10977, 0.13211, 0.14981]);
    for (band, wavelength_nm) in BAND_WAVELENGTHS_NM.into_iter().enumerate() {
        let ratio = math::powf(wavelength_nm / 600.0, 0.83);
        for (terrain, [p10, median, p90]) in [
            (&p.mare, [0.062, 0.072, 0.082]),
            (&p.highland, [0.096, 0.114, 0.142]),
        ] {
            assert_same_bits(terrain.centres[band], median * ratio);
            assert_same_bits(terrain.p10[band], p10 * ratio);
            assert_same_bits(terrain.p90[band], p90 * ratio);
        }
    }
    assert_eq!((p.nodes, p.grid.count), (129, 471));
    // Eq. 4's 3.2 × 10⁻⁴ per FeO is 4.117 × 10⁻⁴ per FeO-iron, 0.0309 at X = 75 (0.030876).
    assert_same_bits(p.checks.morris_slope, 3.2e-4);
    let centre = p.checks.yield_centre();
    assert!((centre - 0.0309).abs() < 5e-5, "{centre}");
    let text = String::from_utf8(repository_manifest().bytes().to_vec()).unwrap();
    let edited = |from: &str, to: &str| {
        let text = text.replacen(from, to, 1);
        assert_ne!(
            text,
            String::from_utf8(repository_manifest().bytes().to_vec()).unwrap()
        );
        Manifest::from_bytes(std::path::Path::new("edited.toml"), text.into_bytes())
            .map(|m| Params::of(&m))
            .unwrap()
    };
    assert_eq!(
        edited("nodes = 129", "nodes = 129\nnodez = 1"),
        Err(ManifestParamError::new(
            "nodez",
            "absent: the task reads no such key"
        ))
    );
    assert_eq!(
        edited("mare_median = 0.072", "mare_median = 0.09"),
        Err(ManifestParamError::new(
            "mare_median",
            "inside its P10 and P90"
        ))
    );
    assert_eq!(
        edited("colour_exponent = 0.83", "colour_exponent = nan"),
        Err(ManifestParamError::new("colour_exponent", "finite"))
    );
}

/// The fingerprint probes every constant it names, each finite: a row without a fixed density
/// probes [`NO_FIXED_DENSITY`].
#[test]
fn space_weathering_the_fingerprint_probes_the_law_s_constants() {
    let fingerprint = SpaceWeatheringTask.fingerprint();
    let probe = |name: &str| {
        fingerprint
            .probes()
            .iter()
            .find(|(n, _)| n == name)
            .map(|&(_, v)| bits(v))
    };
    assert_eq!(
        probe("regolith host index"),
        Some(bits(REGOLITH_HOST_INDEX))
    );
    assert_eq!(probe("frost host index"), Some(bits(ICE_HOST_INDEX)));
    assert_eq!(probe("iron density kg m-3"), Some(bits(IRON_DENSITY_KG_M3)));
    assert_eq!(probe("basalt_fe solid density kg m-3"), Some(bits(3_310.0)));
    assert!(probe("basalt_fe A_N in V").is_some());
    assert!(
        probe("H2O A_N in V").is_none(),
        "a frost row is not the fit's"
    );
    // Every probe is finite, a row's missing density included, so a lock file can match it.
    assert!(fingerprint.probes().iter().all(|(_, v)| v.is_finite()));
    assert!(
        fingerprint
            .probes()
            .iter()
            .filter(|(n, _)| n.ends_with("solid density kg m-3"))
            .all(|&(_, v)| v > 0.0 || bits(v) == bits(NO_FIXED_DENSITY))
    );
}
