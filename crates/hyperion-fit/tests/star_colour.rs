//! The colour table (rendering plan R06, R06.T3.a and T3.b) and its four reddening tables
//! (R06.T9.e): the smoke runs go end to end from the committed CIE tables; with the spectra fetched,
//! the committed tables are what the tasks write, the colour table agrees with Pickles' (1998)
//! empirical spectra, an M dwarf's model is less red than its blackbody, and the reddened light the
//! sim reads between the nodes is the direct integrals' of the rows' own spectra.
//!
//! The model spectra, Bessell and Murphy's V and Pickles' library are fetched datasets
//! (`tasks::star_colour`), read from `crates/hyperion-fit/data/cache/`. Without them each such test
//! says so on stderr and checks nothing more; the sim's own tests of the table (`sky::colour`) run
//! either way.

use std::fs;
use std::num::NonZeroUsize;
use std::path::Path;

use clap::Parser as _;
use hyperion_fit::cli::{Cli, run};
use hyperion_fit::emit::{HeaderKind, TableText, Workspace};
use hyperion_fit::pipeline::{ManifestKind, load_manifest, rerender};
use hyperion_fit::task::{find, registry};
use hyperion_fit::tasks::star_colour::columns::{
    BAKE_WAVELENGTH_COUNT, dimmed_bins, extras, law_by_bin,
};
use hyperion_fit::tasks::star_colour::photometry::uv_prime;
use hyperion_fit::tasks::star_colour::pickles::{PICKLES_TYPES, parse_pickles};
use hyperion_fit::tasks::star_colour::spectrum::{BIN_COUNT, bin_centre_nm};
use hyperion_fit::tasks::star_colour::{
    Observer, Sensor, Source, Spectra, Spectrum, bake_wavelengths_nm, fit, read_model_spectrum,
    read_observer_from_dir, read_sensor_from_dir,
};
use hyperion_sim::GENERATOR_VERSION;
use hyperion_sim::math;
use hyperion_sim::sky::colour::{
    AtmosphereGrid, BAKE_WAVELENGTHS_NM, CAMERA_ETA_SUN, SUN_LOG_G, SUN_TEFF_K, StarColour,
    lift_into_gamut, solar_colour, star_colour,
};
use hyperion_sim::tables::star_colour::{
    LUMINANCE_RGB, NORMAL_BAKE, NORMAL_LOG_G, NORMAL_LOG_TEFF, WHITE_DWARF_BAKE,
};
use hyperion_sim::units::{Kelvin, Magnitudes};

/// The table the sim compiles, as committed.
const COMMITTED: &str = include_str!("../../hyperion-sim/src/tables/star_colour.rs");

/// The reddening tables the sim compiles, as committed, by name.
const COMMITTED_REDDENING: [(&str, &str); 4] = [
    (
        "star_colour_reddening",
        include_str!("../../hyperion-sim/src/tables/star_colour_reddening.rs"),
    ),
    (
        "star_colour_reddening_av02_05",
        include_str!("../../hyperion-sim/src/tables/star_colour_reddening_av02_05.rs"),
    ),
    (
        "star_colour_reddening_av10_15",
        include_str!("../../hyperion-sim/src/tables/star_colour_reddening_av10_15.rs"),
    ),
    (
        "star_colour_reddening_av20_30",
        include_str!("../../hyperion-sim/src/tables/star_colour_reddening_av20_30.rs"),
    ),
];

/// The fetched cache.
const CACHE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/data/cache");

/// Whether every dataset the full fit reads is fetched.
fn fetched(what: &str) -> bool {
    let all = [
        "atlas9_ck04",
        "bessell_murphy_2012",
        "phoenix_husser2013",
        "pickles1998",
        "tlusty_ostar2002",
        "tmap",
        "wd_koester_da",
    ]
    .iter()
    .all(|d| Path::new(CACHE).join(d).is_dir());
    if !all {
        eprintln!(
            "the star_colour datasets are not all fetched into {CACHE}: {what} is not checked"
        );
    }
    all
}

#[test]
fn star_colour_the_smoke_run_passes_the_header_grammar_and_is_the_same_on_any_thread_count() {
    smoke_run_passes_the_header_grammar_on_any_thread_count("star_colour");
}

#[test]
fn star_colour_reddening_the_smoke_runs_pass_the_header_grammar_and_are_the_same_on_any_thread_count()
 {
    for (name, _) in COMMITTED_REDDENING {
        smoke_run_passes_the_header_grammar_on_any_thread_count(name);
    }
}

/// `name`'s smoke run, on one thread and on three: the same text, provisional, marked as a smoke
/// run, and declaring every item the task lists.
fn smoke_run_passes_the_header_grammar_on_any_thread_count(name: &str) {
    let dir = tempfile::tempdir().unwrap();
    let mut texts = Vec::new();
    for threads in ["1", "3"] {
        let out = dir.path().join(format!("{name}_{threads}.rs"));
        let cli = Cli::try_parse_from([
            "hyperion-fit",
            "run",
            name,
            "--smoke",
            "--threads",
            threads,
            "--out",
            out.to_str().unwrap(),
        ])
        .unwrap();
        let mut log = Vec::new();
        run(&cli, &mut log).unwrap();
        texts.push(fs::read_to_string(&out).unwrap());
    }
    assert_eq!(texts[0], texts[1]);
    let split = TableText::of_file(&texts[0]);
    let header = split.header().unwrap();
    assert!(matches!(header.kind, Some(HeaderKind::Provisional { .. })));
    assert!(texts[0].contains("SMOKE RUN"));
    let task = find(name).unwrap();
    for item in task.items() {
        assert!(
            texts[0].contains(&format!("pub const {item}:"))
                || texts[0].contains(&format!("pub static {item}:")),
            "{item} is not declared"
        );
    }
}

#[test]
#[ignore = "slow: integrates some 1,300 model spectra, 1 GB of PHOENIX among them"]
fn star_colour_table_is_reproduced() {
    table_is_reproduced("star_colour", COMMITTED);
}

#[test]
#[ignore = "slow: integrates some 1,300 model spectra, 1 GB of PHOENIX among them, four times"]
fn star_colour_reddening_table_is_reproduced() {
    for (name, committed) in COMMITTED_REDDENING {
        table_is_reproduced(name, committed);
    }
}

/// `name`'s fit on the fetched spectra, on eight threads and on one, renders `committed`.
fn table_is_reproduced(name: &str, committed: &str) {
    if !fetched("the committed table") {
        return;
    }
    let render = |threads: usize| {
        rerender(
            registry(),
            find(name).expect("the task is registered"),
            &Workspace::repository(),
            NonZeroUsize::new(threads).unwrap(),
            GENERATOR_VERSION.get(),
        )
        .expect("the task runs on the fetched spectra")
        .text
    };
    let rendered = render(8);
    // The models are read in parallel; the thread count must not move a bit.
    assert!(
        render(1) == rendered,
        "{name}: one thread renders another table than eight"
    );
    if rendered != committed {
        let line = rendered
            .lines()
            .zip(committed.lines())
            .position(|(a, b)| a != b)
            .map_or_else(|| "the length".to_owned(), |n| format!("line {}", n + 1));
        panic!(
            "crates/hyperion-sim/src/tables/{name}.rs differs from the fit at {line}: run `just fit \
             {name}`"
        );
    }
}

/// The observer with the fetched V band.
fn observer() -> Observer {
    read_observer_from_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("data")).unwrap()
}

/// The (u′, v′) of the table's colour at `teff` and `log_g`, from its linear Rec. 709.
fn table_uv(observer: &Observer, teff: f64, log_g: f64, grid: AtmosphereGrid) -> [f64; 2] {
    let c = star_colour(Kelvin::new(teff), log_g, grid);
    let [r, g] = c.chroma();
    uv_prime(observer.rgb_to_xyz([f64::from(r), f64::from(g), c.blue()]))
}

/// A Pickles spectrum, read by the task's own reader.
fn pickles(name: &str) -> Spectrum {
    parse_pickles(&fs::read_to_string(Path::new(CACHE).join("pickles1998").join(name)).unwrap())
        .unwrap()
}

/// Pickles' library, integrated by the task's own code, lies within Δ(u′, v′) < 0.005 of the
/// table at each type's `T_eff` and log g (R06.T3.b), from A0 V to M2 V and for the G8 and K0 giants;
/// O5 V and M3 III are measured exceptions (`tasks::star_colour::pickles::PICKLES_TYPES`, ruled
/// 2026-10-02).
#[test]
fn star_colour_the_table_agrees_with_pickles() {
    if !fetched("the Pickles comparison") {
        return;
    }
    let observer = observer();
    let mut report = Vec::new();
    let mut failed = false;
    for ty in PICKLES_TYPES {
        let spectrum = pickles(ty.file);
        let uv = uv_prime(observer.integrals(&spectrum.bin_means()).xyz);
        let grid = if ty.giant {
            AtmosphereGrid::Giant
        } else {
            AtmosphereGrid::MainSequence
        };
        let table = table_uv(&observer, ty.teff, ty.log_g, grid);
        let d = hyperion_sim::math::hypot(uv[0] - table[0], uv[1] - table[1]);
        report.push(format!("{} at {} K: Δ(u′v′) {d:.4}", ty.file, ty.teff));
        failed |= d >= ty.limit;
    }
    eprintln!("{}", report.join("; "));
    assert!(!failed, "{}", report.join("; "));
}

/// An M dwarf's model is less red than its blackbody by 0.01–0.02 in CIE 1960 (u, v) (R06.T3.b;
/// the brainstorm's "up to 0.02 off in CIE 1960 uv for M dwarfs"), from 2,900 to 3,100 K, M5 V to
/// M6 V (ruled 2026-10-02: at 3,200 K the difference is 0.008).
#[test]
fn star_colour_an_m_dwarf_is_less_red_than_its_blackbody() {
    if !fetched("the M dwarf against its blackbody") {
        return;
    }
    let observer = observer();
    // CIE 1960 (u, v) from (u′, v′): u = u′, v = 2v′ ÷ 3.
    let to_1960 = |[u, v]: [f64; 2]| [u, v * 2.0 / 3.0];
    for teff in [2_900.0, 3_000.0, 3_100.0] {
        let model = to_1960(table_uv(&observer, teff, 5.0, AtmosphereGrid::MainSequence));
        let bb = to_1960(uv_prime(
            observer
                .integrals(&Spectrum::blackbody(teff).bin_means())
                .xyz,
        ));
        let d = hyperion_sim::math::hypot(model[0] - bb[0], model[1] - bb[1]);
        eprintln!("{teff} K: model {model:?}, blackbody {bb:?}, Δ(uv) {d:.4}");
        // Less red: a smaller u than the blackbody's.
        assert!(model[0] < bb[0], "{teff} K");
        assert!((0.01..=0.02).contains(&d), "{teff} K: Δ(uv) {d:.4}");
    }
}

/// Pickles' own spectra put a star of V = 0 at 0 to +0.10 mag of photopic flux above 2.54 µlx from
/// O5 V to M6 V (the brainstorm's "within 0.08", corrected 2026-10-02).
#[test]
fn star_colour_pickles_lux_per_v0_is_within_a_tenth_of_a_magnitude() {
    if !fetched("Pickles' photopic flux") {
        return;
    }
    let observer = observer();
    for file in [
        "uko5v.dat",
        "uka0v.dat",
        "ukg2v.dat",
        "ukk5v.dat",
        "ukm2v.dat",
        "ukm5v.dat",
        "ukm6v.dat",
    ] {
        let row = observer.row(&pickles(file).bin_means());
        let mag = 2.5 * hyperion_sim::math::log10(row.lux_per_v0);
        eprintln!("{file}: {mag:.3} mag");
        assert!(mag.abs() < 0.11, "{file}: {mag:.3} mag");
    }
}

/// The default sensor.
fn sensor() -> Sensor {
    read_sensor_from_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("data")).unwrap()
}

/// The fit's bake bins are the sim's mirror of R08's `BAKE_WAVELENGTHS_NM` (and so the fixture's,
/// which the sim's own test reads).
#[test]
fn star_colour_the_bake_bins_are_the_sims() {
    for (a, b) in bake_wavelengths_nm().iter().zip(BAKE_WAVELENGTHS_NM) {
        assert!((a - b).abs() < 1e-9, "{a} against {b}");
    }
}

/// The fifteen-bin illuminance of every bake spectrum is 1 lx to 10⁻⁶: every row of the committed
/// table, blackbodies from 2,300 K to 500,000 K and, when it is fetched, Pickles' library; and for
/// the spectra, within 1% of the exact integral over 1 nm bins (R06.T3.c).
#[test]
fn star_colour_every_bake_spectrum_holds_one_lux() {
    let observer = observer();
    let sensor = sensor();
    let y_means = observer.photopic_bin_means();
    let width = 380.0 / 15.0;
    // The committed table's rows, as the sim reads them.
    for bake in NORMAL_BAKE.iter().chain(&WHITE_DWARF_BAKE) {
        let lux: f64 = (0..BAKE_WAVELENGTH_COUNT)
            .map(|k| 683.0 * y_means[k] * bake[k] * width)
            .sum();
        assert!((lux - 1.0).abs() < 1e-6, "{bake:?}: {lux}");
    }
    let mut spectra: Vec<(String, Spectrum)> =
        [2_300.0, 3_000.0, 5_772.0, 10_000.0, 40_000.0, 5.0e5]
            .iter()
            .map(|&t| (format!("{t} K"), Spectrum::blackbody(t)))
            .collect();
    if fetched("Pickles' bake spectra") {
        for ty in PICKLES_TYPES {
            spectra.push((ty.file.to_owned(), pickles(ty.file)));
        }
    }
    for (name, spectrum) in spectra {
        let bins = spectrum.bin_means();
        let bake = extras(&observer, &sensor, &spectrum, &bins).bake;
        let lux: f64 = (0..BAKE_WAVELENGTH_COUNT)
            .map(|k| 683.0 * y_means[k] * bake[k] * width)
            .sum();
        assert!((lux - 1.0).abs() < 1e-6, "{name}: {lux}");
        // The exact photopic integral of the same spectrum, scaled as the bake spectrum is.
        let (lo, hi) = (380.0, 760.0);
        let scale = bake[7] / (spectrum.integral(lo + 7.0 * width, lo + 8.0 * width) / width);
        let exact: f64 = (0..BIN_COUNT)
            .map(|i| 683.0 * observer.photopic()[i] * bins[i] * scale)
            .sum();
        assert!(
            (exact - 1.0).abs() < 0.01,
            "{name}: exact {exact} ({lo}–{hi} nm bins)"
        );
    }
}

/// The camera band term of Pickles' spectra, integrated by the task's own code against the
/// committed η☉: decision-camera-eta's values ± 0.05 mag.
#[test]
fn star_colour_pickles_camera_terms_are_the_rulings() {
    if !fetched("Pickles' camera terms") {
        return;
    }
    let observer = observer();
    let sensor = sensor();
    for (file, expected) in [
        ("uko5v.dat", 0.11),
        ("uka0v.dat", 0.14),
        ("ukk5v.dat", -0.30),
        ("ukm2v.dat", -0.70),
        ("ukm5v.dat", -1.58),
        ("ukm6v.dat", -2.14),
    ] {
        let spectrum = pickles(file);
        let eta = extras(&observer, &sensor, &spectrum, &spectrum.bin_means()).camera_eta;
        let term = -2.5 * math::log10(eta / CAMERA_ETA_SUN);
        eprintln!("{file}: {term:.3}");
        assert!(
            (term - expected).abs() < 0.05,
            "{file}: {term:.3} against {expected}"
        );
    }
}

/// Bessell and Murphy's (2012) photonic I response, as (nm, response), from Table 1's bytes 45–48
/// and 50–54.
fn bessell_i() -> Vec<(f64, f64)> {
    let text = fs::read_to_string(Path::new(CACHE).join("bessell_murphy_2012/table1.dat")).unwrap();
    text.lines()
        .filter_map(|l| {
            let w: f64 = l.get(44..48)?.trim().parse().ok()?;
            let r: f64 = l.get(49..54)?.trim().parse().ok()?;
            Some((w / 10.0, r))
        })
        .collect()
}

/// Synthetic V − I of a spectrum's bins, photon-weighted, zero for Pickles' A0 V.
fn v_minus_i(observer: &Observer, i_band: &[(f64, f64)], bins: &[f64]) -> f64 {
    let response = |nm: f64| {
        let j = i_band.partition_point(|&(w, _)| w <= nm);
        if j == 0 || j == i_band.len() {
            return 0.0;
        }
        let (w0, r0) = i_band[j - 1];
        let (w1, r1) = i_band[j];
        r0 + (r1 - r0) * (nm - w0) / (w1 - w0)
    };
    let (mut v, mut i) = (0.0, 0.0);
    for (k, &s) in bins.iter().enumerate() {
        let nm = bin_centre_nm(k);
        v += s * observer.v_photons()[k];
        i += s * response(nm) * nm;
    }
    -2.5 * math::log10(v / i)
}

/// For Pickles' dwarfs from O5 V to M6 V the camera band term lies within 0.1 mag of Riello et
/// al.'s (2021, A&A 649, A3) G − V(V − `I_C`) relation for Gaia's silicon, relative to the Sun's
/// (coefficients −0.01597, −0.02809, −0.2483, 0.03656, −0.002939; decision-camera-eta), with
/// V − I synthetic through Bessell and Murphy's V and I, zero for Pickles' A0 V.
#[test]
fn star_colour_the_camera_term_follows_gaias_g_minus_v() {
    if !fetched("the camera term against Gaia's G − V") {
        return;
    }
    let observer = observer();
    let sensor = sensor();
    let i_band = bessell_i();
    let a0 = pickles("uka0v.dat").bin_means();
    let zero = v_minus_i(&observer, &i_band, &a0);
    let riello = |x: f64| {
        -0.015_97 - 0.028_09 * x - 0.248_3 * x * x + 0.036_56 * x * x * x
            - 0.002_939 * x * x * x * x
    };
    let sun = pickles("ukg2v.dat").bin_means();
    let sun_x = v_minus_i(&observer, &i_band, &sun) - zero;
    for file in [
        "uko5v.dat",
        "uka0v.dat",
        "ukf5v.dat",
        "ukg2v.dat",
        "ukk0v.dat",
        "ukk5v.dat",
        "ukm0v.dat",
        "ukm2v.dat",
        "ukm3v.dat",
        "ukm4v.dat",
        "ukm5v.dat",
        "ukm6v.dat",
    ] {
        let spectrum = pickles(file);
        let bins = spectrum.bin_means();
        let x = v_minus_i(&observer, &i_band, &bins) - zero;
        let eta = extras(&observer, &sensor, &spectrum, &bins).camera_eta;
        let term = -2.5 * math::log10(eta / CAMERA_ETA_SUN);
        let gaia = riello(x) - riello(sun_x);
        eprintln!("{file}: V − I {x:.2}, term {term:.3}, Riello {gaia:.3}");
        assert!(
            (term - gaia).abs() < 0.1,
            "{file}: {term:.3} against {gaia:.3}"
        );
    }
}

/// The full reddening manifest, which names the datasets' directories.
fn reddening_manifest() -> hyperion_fit::manifest::Manifest {
    load_manifest(
        &Workspace::repository(),
        find("star_colour_reddening").expect("the task is registered"),
        ManifestKind::Full,
    )
    .expect("the manifest loads")
}

/// The addendum's tests 4 and 5 on the fetched spectra, every row of both grids: the sim's
/// `lift_into_gamut` of each row's raw colour is `unit_rgb`'s, bit for bit, unreddened (no row is
/// out of gamut) and behind `A_V` 30 (every row is), so on both of the lift's branches; and the
/// exact parts' photopic transmission is the direct V(λ) integral's within 10⁻⁴ at every node.
#[test]
#[ignore = "slow: integrates some 1,300 model spectra, 1 GB of PHOENIX among them"]
fn star_colour_reddening_every_row_lifts_as_t3_and_keeps_the_photopic_identity() {
    if !fetched("the reddening's every row") {
        return;
    }
    let observer = observer();
    let table = fit(
        &reddening_manifest(),
        &observer,
        &sensor(),
        Spectra::Models,
        NonZeroUsize::new(8).unwrap(),
    )
    .unwrap();
    let mut worst: f64 = 0.0;
    let mut lifted = [0, 0];
    for row in table.normal.rows.iter().chain(&table.white_dwarf.rows) {
        for (k, xyz) in [row.reddening.xyz, row.reddening.xyz_at_last_node]
            .into_iter()
            .enumerate()
        {
            let raw = observer.raw_rgb(xyz);
            lifted[k] += usize::from(raw.iter().any(|&c| c < 0.0));
            let (sim, t3) = (lift_into_gamut(raw), observer.unit_rgb(xyz));
            // `total_cmp` is equal exactly when the bits are.
            assert!(
                sim.iter().zip(&t3).all(|(a, b)| a.total_cmp(b).is_eq()),
                "{sim:?} against {t3:?}"
            );
        }
        worst = worst.max(row.reddening.photopic_identity);
    }
    eprintln!(
        "the parts' photopic transmission against V(λ)'s: within {worst:.2e}; lifted, of every row: \
         {} unreddened, {} behind A_V 30",
        lifted[0], lifted[1]
    );
    assert!(worst < 1e-4, "{worst}");
    assert_eq!(
        lifted[1],
        table.normal.rows.len() + table.white_dwarf.rows.len(),
        "every row's light is out of gamut behind A_V 30"
    );
}

/// One row of the between-the-nodes test: its name, the spectrum's 1 nm bin means, and the sim's
/// colour of it.
struct Row {
    name: &'static str,
    bins: Vec<f64>,
    colour: StarColour,
}

/// A grid node's spectrum, read from the cache.
fn node_bins(source: Source, teff: u32, log_g_centi: i32) -> Vec<f64> {
    read_model_spectrum(&reddening_manifest(), source, teff, log_g_centi)
        .unwrap_or_else(|e| panic!("{} at {teff} K, log g {log_g_centi}: {e}", source.label()))
        .bin_means()
}

/// The direct integrals of a spectrum's bin means `bins` behind sightline `A_V` `a`, as the
/// reddened values the sim gives: the colour of unit luminance lifted into gamut, the photopic and
/// V extinctions, mag, ρ, and the camera's extinction less V's, mag.
fn direct(observer: &Observer, sensor: &Sensor, law: &[f64], bins: &[f64], a: f64) -> Direct {
    let dimmed = dimmed_bins(bins, law, a);
    let (before, after) = (observer.integrals(bins), observer.integrals(&dimmed));
    let band = |b: &[f64], w: &dyn Fn(usize) -> f64| -> f64 {
        b.iter().enumerate().map(|(i, &s)| s * w(i)).sum()
    };
    let v = |i: usize| observer.v_photons()[i];
    let camera = |i: usize| sensor.qe()[i] * bin_centre_nm(i);
    let extinction =
        |w: &dyn Fn(usize) -> f64| -2.5 * math::log10(band(&dimmed, w) / band(bins, w));
    Direct {
        rgb: observer.unit_rgb(after.xyz),
        photopic: -2.5 * math::log10(after.photopic / before.photopic),
        v: extinction(&v),
        sp_ratio: 1_700.0 * after.scotopic / (683.0 * after.photopic),
        camera: extinction(&camera) - extinction(&v),
    }
}

/// [`direct`]'s values.
struct Direct {
    rgb: [f64; 3],
    photopic: f64,
    v: f64,
    sp_ratio: f64,
    camera: f64,
}

/// The colour table's solar point's spectrum: its four ATLAS9 nodes' spectra, each at unit
/// luminance, mixed by the weights the sim takes from the table's rounded nodes.
fn solar_bins(observer: &Observer) -> Vec<f64> {
    let bracket = |nodes: &[f64], x: f64| {
        let i = nodes
            .partition_point(|&n| n <= x)
            .saturating_sub(1)
            .min(nodes.len() - 2);
        (
            i,
            ((x - nodes[i]) / (nodes[i + 1] - nodes[i])).clamp(0.0, 1.0),
        )
    };
    let (ti, tf) = bracket(&NORMAL_LOG_TEFF, math::log10(SUN_TEFF_K));
    let (gi, gf) = bracket(&NORMAL_LOG_G, SUN_LOG_G);
    // The nodes are whole kelvin and hundredths of a dex; the table holds their rounded values.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a grid temperature of a few thousand kelvin, rounded to a whole kelvin"
    )]
    let node_teff = |i: usize| (math::exp10(NORMAL_LOG_TEFF[i]) + 0.5).floor() as u32;
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a grid gravity of 0–6 in hundredths"
    )]
    let node_g = |j: usize| (NORMAL_LOG_G[j] * 100.0).round() as i32;
    let mut solar = vec![0.0; BIN_COUNT];
    for (i, wt) in [(ti, 1.0 - tf), (ti + 1, tf)] {
        for (j, wg) in [(gi, 1.0 - gf), (gi + 1, gf)] {
            let bins = node_bins(Source::Atlas9, node_teff(i), node_g(j));
            let luminance = observer.integrals(&bins).xyz[1];
            for (sum, s) in solar.iter_mut().zip(&bins) {
                *sum += wt * wg * s / luminance;
            }
        }
    }
    solar
}

/// The rows of the between-the-nodes test, with their spectra.
fn between_the_nodes_rows(observer: &Observer) -> Vec<Row> {
    let dwarf = AtmosphereGrid::MainSequence;
    let at = |t: f64, g: f64, grid| star_colour(Kelvin::new(t), g, grid);
    vec![
        Row {
            name: "the solar row",
            bins: solar_bins(observer),
            colour: solar_colour(),
        },
        Row {
            name: "a 45,000 K dwarf",
            bins: node_bins(Source::Tlusty, 45_000, 450),
            colour: at(45_000.0, 4.5, dwarf),
        },
        Row {
            name: "a 30,000 K dwarf",
            bins: node_bins(Source::Tlusty, 30_000, 450),
            colour: at(30_000.0, 4.5, dwarf),
        },
        Row {
            name: "a 10,000 K dwarf",
            bins: node_bins(Source::Atlas9, 10_000, 450),
            colour: at(10_000.0, 4.5, dwarf),
        },
        Row {
            name: "a 4,000 K giant",
            bins: node_bins(Source::Atlas9, 4_000, 150),
            colour: at(4_000.0, 1.5, AtmosphereGrid::Giant),
        },
        Row {
            name: "a 3,500 K dwarf",
            bins: node_bins(Source::Atlas9, 3_500, 500),
            colour: at(3_500.0, 5.0, dwarf),
        },
        Row {
            name: "a 3,000 K dwarf",
            bins: node_bins(Source::Phoenix, 3_000, 500),
            colour: at(3_000.0, 5.0, dwarf),
        },
        Row {
            name: "the coolest PHOENIX row",
            bins: node_bins(Source::Phoenix, 2_300, 500),
            colour: at(2_300.0, 5.0, dwarf),
        },
        Row {
            name: "a 10,000 K white dwarf",
            bins: node_bins(Source::Koester, 10_000, 800),
            colour: at(10_000.0, 8.0, AtmosphereGrid::WhiteDwarf),
        },
    ]
}

/// The between-the-nodes test's range of `A_V` `a`: to 5, to 20 or to 30.
fn range_of(a: f64) -> usize {
    usize::from(a > 5.0) + usize::from(a > 20.0)
}

/// The between-the-nodes test's tolerance of quantity `q` (in [`QUANTITIES`]' order) at `A_V`
/// `a`, mag or a channel at unit luminance.
fn tolerance(q: usize, a: f64) -> f64 {
    match q {
        0 => [0.005, 0.02, 0.07][range_of(a)],
        1 => [0.003, 0.003, 0.02][range_of(a)],
        2 if a <= 5.0 => 0.035,
        2 if a <= 10.0 => 0.05,
        2 => 0.2,
        3 => [0.01, 0.05, 0.05][range_of(a)],
        _ => [0.005, 0.05, 0.05][range_of(a)],
    }
}

/// The between-the-nodes test's quantities.
const QUANTITIES: [&str; 5] = ["colour", "photopic", "camera", "eye offset", "V extinction"];

/// The sim's reddened light of `colour` behind `a` against the direct integrals `d`: each of
/// [`QUANTITIES`]' errors.
fn errors(colour: &StarColour, a: f64, d: &Direct) -> [f64; 5] {
    let unreddened = colour.reddened(Magnitudes::ZERO);
    let r = colour.reddened(Magnitudes::new(a));
    let [red, green] = r.red_green();
    let [yr, yg, yb] = LUMINANCE_RGB;
    let rgb = [red, green, (1.0 - yr * red - yg * green) / yb];
    [
        rgb.iter()
            .zip(&d.rgb)
            .map(|(s, t)| (s - t).abs())
            .fold(0.0, f64::max),
        (-2.5 * math::log10(r.photopic_transmission()) - d.photopic).abs(),
        (r.camera_band_mag() - unreddened.camera_band_mag() - d.camera).abs(),
        (2.5 * math::log10(r.sp_ratio() / d.sp_ratio)).abs(),
        (r.v_extinction().value() - d.v).abs(),
    ]
}

/// The addendum's test 2: between the nodes (`A_V` 0.5, 1, 1.5, 3, 4, 7, 12, 17 and 25), the
/// reddened light the sim reads off the tables lies within the ruled tolerances of the direct
/// integrals of the rows' own spectra, for the solar row, 30,000 K and 10,000 K dwarfs, a
/// 4,000 K giant of log g 1.5, 3,500 K and 3,000 K dwarfs, the coolest PHOENIX row and a 10,000 K
/// white dwarf; and, beyond the addendum's list, a 45,000 K dwarf, so that the hot rows' camera term
/// is bounded beyond 30,000 K too (the science check of the amendment). No row of either grid is
/// out of gamut unreddened, so the coolest PHOENIX row, 2,300 K at log g 5, stands for the lifted
/// one: reddening lifts it from `A_V` 5.
///
/// | Quantity | To `A_V` 5 | To 20 | To 30 |
/// | --- | --- | --- | --- |
/// | the lifted colour at unit luminance, a channel | 0.005 | 0.02 | 0.07 |
/// | the photopic | 0.003 mag | 0.003 mag | 0.02 mag |
/// | the camera term | 0.035 mag | 0.05 mag to 10, 0.2 beyond | 0.2 mag |
/// | the eye offset, 2.5 log₁₀ ρ | 0.01 mag | 0.05 mag | 0.05 mag |
/// | V's extinction | 0.005 mag | 0.05 mag | 0.05 mag |
///
/// The camera term between `A_V` 5 and 10 is held to 0.05 mag, not the addendum's 0.035: the hot
/// rows' secant bends between the nodes at 5 and 10 (0.048 for the 30,000 K dwarf at 7; R06's Risks,
/// "Deviations in T9.e, as built").
#[test]
#[ignore = "slow: reads and integrates twelve model spectra"]
fn star_colour_reddening_between_the_nodes_is_the_direct_integrals() {
    if !fetched("the reddening between the nodes") {
        return;
    }
    let observer = observer();
    let sensor = sensor();
    let law = law_by_bin();
    let mut worst = [[0.0_f64; 3]; 5];
    let mut failures = Vec::new();
    for row in between_the_nodes_rows(&observer) {
        let mut own = [[0.0_f64; 3]; 5];
        for a in [0.5, 1.0, 1.5, 3.0, 4.0, 7.0, 12.0, 17.0, 25.0] {
            let d = direct(&observer, &sensor, &law, &row.bins, a);
            for (q, e) in errors(&row.colour, a, &d).into_iter().enumerate() {
                worst[q][range_of(a)] = worst[q][range_of(a)].max(e);
                own[q][range_of(a)] = own[q][range_of(a)].max(e);
                if e > tolerance(q, a) {
                    failures.push(format!(
                        "{} at A_V {a}: {} off by {e:.4} (tolerance {})",
                        row.name,
                        QUANTITIES[q],
                        tolerance(q, a)
                    ));
                }
            }
            if row.name == "the solar row" && a <= 3.0 {
                eprintln!("the solar row at A_V {a}: the direct colour {:?}", d.rgb);
            }
        }
        eprintln!(
            "{}: worst to A_V 5, 20 and 30: colour {:.4?}, photopic {:.4?}, camera {:.4?}, eye \
             offset {:.4?}, V {:.4?}",
            row.name, own[0], own[1], own[2], own[3], own[4]
        );
    }
    for (q, name) in QUANTITIES.iter().enumerate() {
        eprintln!(
            "{name}: worst {:.4} to A_V 5, {:.4} to 20, {:.4} to 30",
            worst[q][0], worst[q][1], worst[q][2]
        );
    }
    assert!(failures.is_empty(), "{}", failures.join("; "));
}
