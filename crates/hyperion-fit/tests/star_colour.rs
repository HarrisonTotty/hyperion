//! The colour table (rendering plan R06, R06.T3.a and T3.b): the smoke run goes end to end from the
//! committed CIE tables; with the spectra fetched, the committed table is what the task writes, it
//! agrees with Pickles' (1998) empirical spectra, and an M dwarf's model is less red than its
//! blackbody.
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
use hyperion_fit::pipeline::rerender;
use hyperion_fit::task::{find, registry};
use hyperion_fit::tasks::star_colour::columns::{BAKE_WAVELENGTH_COUNT, extras};
use hyperion_fit::tasks::star_colour::photometry::uv_prime;
use hyperion_fit::tasks::star_colour::pickles::{PICKLES_TYPES, parse_pickles};
use hyperion_fit::tasks::star_colour::spectrum::{BIN_COUNT, bin_centre_nm};
use hyperion_fit::tasks::star_colour::{
    Observer, Sensor, Spectrum, bake_wavelengths_nm, read_observer_from_dir, read_sensor_from_dir,
};
use hyperion_sim::GENERATOR_VERSION;
use hyperion_sim::math;
use hyperion_sim::sky::colour::{AtmosphereGrid, BAKE_WAVELENGTHS_NM, CAMERA_ETA_SUN, star_colour};
use hyperion_sim::tables::star_colour::{NORMAL_BAKE, WHITE_DWARF_BAKE};
use hyperion_sim::units::Kelvin;

/// The table the sim compiles, as committed.
const COMMITTED: &str = include_str!("../../hyperion-sim/src/tables/star_colour.rs");

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
    let dir = tempfile::tempdir().unwrap();
    let mut texts = Vec::new();
    for threads in ["1", "3"] {
        let out = dir.path().join(format!("star_colour_{threads}.rs"));
        let cli = Cli::try_parse_from([
            "hyperion-fit",
            "run",
            "star_colour",
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
    let task = find("star_colour").unwrap();
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
    if !fetched("the committed table") {
        return;
    }
    let render = |threads: usize| {
        rerender(
            registry(),
            find("star_colour").expect("star_colour is registered"),
            &Workspace::repository(),
            NonZeroUsize::new(threads).unwrap(),
            GENERATOR_VERSION.get(),
        )
        .expect("the star_colour task runs on the fetched spectra")
        .text
    };
    let rendered = render(8);
    // The models are read in parallel; the thread count must not move a bit.
    assert!(
        render(1) == rendered,
        "one thread renders another table than eight"
    );
    if rendered != COMMITTED {
        let line = rendered
            .lines()
            .zip(COMMITTED.lines())
            .position(|(a, b)| a != b)
            .map_or_else(|| "the length".to_owned(), |n| format!("line {}", n + 1));
        panic!(
            "crates/hyperion-sim/src/tables/star_colour.rs differs from the fit at {line}: run \
             `just fit star_colour`"
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
