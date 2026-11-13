//! The rough plains' cold-trap table (rendering plan R09, R09.T7.g and R09.T7.l): the committed
//! body is the rendering of its committed values, the surface crate's reader is the fit's own read
//! to the bit, and the task's smoke run goes end to end through the command line, the same bytes on
//! one thread and on four and at two chunk sizes.

mod common;

use std::fs;
use std::path::Path;

use clap::Parser;
use hyperion_fit::cli::{Cli, run};
use hyperion_fit::emit::{HeaderKind, TableText};
use hyperion_fit::manifest::Manifest;
use hyperion_fit::task::find;
use hyperion_fit::tasks::rough_plains_cold_traps::{Axes, Params, RoughPlainsFit, render};
use hyperion_sim::math;
use hyperion_sim::units::Radians;
use hyperion_surface::cold_trap::rough_plains;
use hyperion_surface::tables::rough_plains_cold_traps as table;
use hyperion_testkit::float::assert_same_bits;

/// The table the surface crate compiles, as committed.
const COMMITTED: &str =
    include_str!("../../hyperion-surface/src/tables/rough_plains_cold_traps.rs");

/// The task's manifest.
fn manifest() -> Manifest {
    Manifest::load(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("manifests/rough_plains_cold_traps.toml"),
    )
    .unwrap()
}

/// The committed table as a fit: the manifest's axes, which must be the table's to the bit, and
/// the table's values.
fn committed() -> RoughPlainsFit {
    let params = Params::of(&manifest()).unwrap();
    let axes = Axes::of(&params);
    for (built, table) in [
        (&axes.slopes, &table::SLOPE[..]),
        (&axes.colatitudes, &table::COLATITUDE_PER_SLOPE[..]),
        (&axes.limbs, &table::LIMB_PER_SLOPE[..]),
        (
            &axes.cold_colatitudes,
            &table::COLD_COLATITUDE_PER_SLOPE[..],
        ),
        (&axes.cold_limbs, &table::COLD_LIMB_PER_SLOPE[..]),
        (&axes.irradiances, &table::IRRADIANCE[..]),
        (&axes.instants, &table::TAN_ELEVATION_PER_SLOPE[..]),
    ] {
        assert_eq!(built.len(), table.len());
        assert!(built.iter().zip(table).all(|(a, b)| a.total_cmp(b).is_eq()));
    }
    assert_eq!(axes.cold_colatitude_indices, table::COLD_COLATITUDE_INDEX);
    assert_eq!(axes.cold_limb_indices, table::COLD_LIMB_INDEX);
    assert_same_bits(params.star_radius.value(), table::STAR_RADIUS);
    of_table(axes, params)
}

/// The committed table's values as a fit on `axes` and `params`.
fn of_table(axes: Axes, params: Params) -> RoughPlainsFit {
    RoughPlainsFit {
        axes,
        shadow: table::SHADOW.iter().map(|row| row.to_vec()).collect(),
        cold: table::COLD.iter().map(|row| row.to_vec()).collect(),
        instantaneous: table::INSTANTANEOUS.to_vec(),
        params,
    }
}

/// The committed table as a fit on its own axes, whatever the manifest holds: what the reader
/// reads.
fn as_read() -> RoughPlainsFit {
    let axes = Axes {
        slopes: table::SLOPE.to_vec(),
        colatitudes: table::COLATITUDE_PER_SLOPE.to_vec(),
        limbs: table::LIMB_PER_SLOPE.to_vec(),
        cold_colatitudes: table::COLD_COLATITUDE_PER_SLOPE.to_vec(),
        cold_limbs: table::COLD_LIMB_PER_SLOPE.to_vec(),
        cold_colatitude_indices: table::COLD_COLATITUDE_INDEX.to_vec(),
        cold_limb_indices: table::COLD_LIMB_INDEX.to_vec(),
        irradiances: table::IRRADIANCE.to_vec(),
        instants: table::TAN_ELEVATION_PER_SLOPE.to_vec(),
    };
    of_table(axes, Params::of(&manifest()).unwrap())
}

/// The committed body is the task's rendering of the committed values, byte for byte. The fit
/// itself takes about three hours, so it is not rerun here: `hyperion-fit check` holds the file to its
/// inputs hash and body hash, and the surface crate's tests hold its figures.
#[test]
fn the_committed_body_is_the_rendering_of_its_values() {
    let split = TableText::of_file(COMMITTED);
    assert_eq!(render(&committed()).body().unwrap(), split.body);
    let header = split.header().unwrap();
    assert_eq!(header.kind, Some(HeaderKind::Generated));
    assert_eq!(header.task.as_deref(), Some("rough_plains_cold_traps"));
    assert_eq!(
        header.manifest.as_deref(),
        Some("crates/hyperion-fit/manifests/rough_plains_cold_traps.toml")
    );
    let task = find("rough_plains_cold_traps").unwrap();
    assert_eq!(common::declared(&split.body), task.items());
}

#[test]
fn the_surface_crates_reader_is_the_fits_read_to_the_bit() {
    // On the table's own axes, so that the two reads are held together whatever the manifest says.
    let fit = as_read();
    let degrees = |x: f64| x.to_radians();
    let same =
        |a: f64, b: f64, what: &str| assert!(a.total_cmp(&b).is_eq(), "{what}: {a} against {b}");
    for w in [0.0, 0.02, 0.05, 0.08, 0.1, 0.15, 0.3, 0.45, 0.5, 0.9, 1.0] {
        let slope = math::atan(w);
        let w = math::tan(slope);
        for colatitude in [0.0, 0.3, 1.0, 2.5, 5.0, 8.0, 12.0, 20.0, 35.0, 60.0] {
            for limb in [0.0, 0.2711, 1.54, 1.81, 3.0, 4.1, 8.0, 20.0] {
                let (c, d) = (degrees(colatitude), degrees(limb));
                let read = fit.permanent_shadow(w, c, d).clamp(0.0, 1.0);
                let reader = rough_plains::permanent_shadow(
                    Radians::new(slope),
                    Radians::new(c),
                    Radians::new(d),
                );
                same(
                    read,
                    reader,
                    &format!("shadow at w {w}, {colatitude}°, {limb}°"),
                );
                for x in [-1.0, 0.0, 5e-5, 1e-4, 2e-3, 0.0137, 0.2, 1.0, 9.0, 50.0] {
                    let read = fit.cold_share(w, c, d, x).clamp(0.0, 1.0);
                    let reader = rough_plains::cold_share(
                        Radians::new(slope),
                        Radians::new(c),
                        Radians::new(d),
                        x,
                    );
                    same(
                        read,
                        reader,
                        &format!("cold at w {w}, {colatitude}°, {limb}°, {x}"),
                    );
                }
            }
        }
        for elevation in [0.0, 0.5, 2.0, 4.0, 10.0, 40.0] {
            let e = degrees(elevation);
            same(
                fit.instantaneous_shadow(w, e),
                rough_plains::instantaneous_shadow(Radians::new(slope), Radians::new(e)),
                &format!("instantaneous at w {w}, {elevation}°"),
            );
        }
    }
}

#[test]
fn the_smoke_run_is_the_same_on_any_thread_count_and_chunk_size() {
    let dir = tempfile::tempdir().unwrap();
    let smoke =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("manifests/rough_plains_cold_traps.smoke.toml");
    // The smoke manifest with its chunks of 4 rows and nodes cut to 1.
    let rechunked = dir.path().join("rough_plains_cold_traps.rechunked.toml");
    let text = fs::read_to_string(&smoke).unwrap();
    assert!(text.contains("\nchunk = 4\n"));
    fs::write(&rechunked, text.replace("\nchunk = 4\n", "\nchunk = 1\n")).unwrap();
    let mut bodies = Vec::new();
    for (threads, manifest) in [("1", None), ("4", None), ("2", Some(&rechunked))] {
        let out = dir.path().join(format!("rough_plains_{threads}.rs"));
        let mut args = vec![
            "hyperion-fit".to_owned(),
            "run".to_owned(),
            "rough_plains_cold_traps".to_owned(),
            "--threads".to_owned(),
            threads.to_owned(),
            "--out".to_owned(),
            out.to_str().unwrap().to_owned(),
        ];
        match manifest {
            Some(path) => args.extend(["--manifest".to_owned(), path.to_str().unwrap().to_owned()]),
            None => args.push("--smoke".to_owned()),
        }
        let cli = Cli::try_parse_from(args).unwrap();
        let mut log = Vec::new();
        run(&cli, &mut log).unwrap();
        let written = fs::read_to_string(&out).unwrap();
        let split = TableText::of_file(&written);
        let acceptance = split.header().unwrap().acceptance.unwrap_or_default();
        assert!(
            acceptance.contains("instantaneous shadow within"),
            "{written}"
        );
        // The direct trace at the Moon's D, R09.T7.l's acceptance, is in the header alone.
        assert!(acceptance.contains("against a direct trace"), "{written}");
        bodies.push((split.body, acceptance));
    }
    assert_eq!(bodies[0], bodies[1]);
    assert_eq!(bodies[0], bodies[2]);
    let task = find("rough_plains_cold_traps").unwrap();
    assert_eq!(common::declared(&bodies[0].0), task.items());
}
