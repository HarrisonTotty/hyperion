//! The emitter, the lock file and the staleness check against toy tasks in a temporary workspace
//! (plan 15, P15.T2) whose tables live in two crates (the crate split, CS.T1), and the check on the
//! repository's own tables.

mod common;

use std::fs;
use std::num::NonZeroUsize;
use std::sync::Mutex;

use hyperion_fit::check::{CheckInputs, CheckReport, Problem, REPOSITORY_MANIFESTS, Rerun, check};
use hyperion_fit::emit::{
    Destination, EmitTableError, Placement, RustTable, TableItem, Workspace, write_table,
};
use hyperion_fit::list::{ListRow, ListState, listing};
use hyperion_fit::manifest::{LockEntry, LockFile, Manifest, SimFingerprint};
use hyperion_fit::pipeline::{ManifestKind, manifest_path, placement_among, prepare};
use hyperion_fit::task::{FitTask, RunTaskError, TableCrate, TaskClass, TaskOutput, registry};
use hyperion_fit::{RunFitError, emit};
use hyperion_sim::GENERATOR_VERSION;
use hyperion_sim::tables::TableInfo;
use tempfile::TempDir;

/// A toy fit: its table is `[value, 2 × value]` from its manifest, and its fingerprint one probe
/// the test can move.
#[derive(Debug)]
struct Toy {
    name: &'static str,
    table_crate: TableCrate,
    path: &'static str,
    shares_its_file: bool,
    provisional: Option<&'static str>,
    probe: Mutex<f64>,
}

impl FitTask for Toy {
    fn name(&self) -> &'static str {
        self.name
    }

    fn class(&self) -> TaskClass {
        TaskClass::Fast
    }

    fn table_crate(&self) -> TableCrate {
        self.table_crate
    }

    fn table_path(&self) -> &'static str {
        self.path
    }

    fn shares_its_file(&self) -> bool {
        self.shares_its_file
    }

    fn revision(&self) -> u32 {
        2
    }

    fn items(&self) -> &'static [&'static str] {
        &["VALUES"]
    }

    fn fingerprint(&self) -> SimFingerprint {
        SimFingerprint::new(vec![(
            "probe".to_owned(),
            *self.probe.lock().expect("no test panics holding it"),
        )])
    }

    fn run(&self, manifest: &Manifest, _threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        let value = manifest.f64("value")?;
        Ok(TaskOutput {
            table: RustTable {
                summary: vec![format!("The toy table `{}`.", self.name)],
                notes: vec!["A test's.".to_owned()],
                items: vec![TableItem::Array {
                    name: "VALUES".to_owned(),
                    doc: vec!["The values.".to_owned()],
                    values: vec![value, 2.0 * value],
                }],
            },
            source: "a test".to_owned(),
            acceptance: format!("value {value}"),
            provisional: self.provisional,
        })
    }
}

/// A toy task of the sim's tables that lives as long as the test binary.
fn toy(name: &'static str, path: &'static str) -> &'static Toy {
    toy_in(TableCrate::Sim, name, path)
}

/// A toy task of `table_crate`'s tables that lives as long as the test binary.
fn toy_in(table_crate: TableCrate, name: &'static str, path: &'static str) -> &'static Toy {
    Box::leak(Box::new(Toy {
        name,
        table_crate,
        path,
        shares_its_file: false,
        provisional: None,
        probe: Mutex::new(1.5),
    }))
}

/// A toy task of the sim's tables whose table is a scratch stand-in that plan task `by` replaces.
fn toy_provisional(name: &'static str, path: &'static str, by: &'static str) -> &'static Toy {
    Box::leak(Box::new(Toy {
        name,
        table_crate: TableCrate::Sim,
        path,
        shares_its_file: false,
        provisional: Some(by),
        probe: Mutex::new(1.5),
    }))
}

/// A toy task of the sim's tables whose block shares its file with code of its own.
fn toy_beside_code(name: &'static str, path: &'static str) -> &'static Toy {
    Box::leak(Box::new(Toy {
        name,
        table_crate: TableCrate::Sim,
        path,
        shares_its_file: true,
        provisional: None,
        probe: Mutex::new(1.5),
    }))
}

/// A workspace whose two crates' `tables/mod.rs` hold the manifest's markers, with each toy's
/// manifest.
fn workspace(toys: &[&Toy]) -> (TempDir, Workspace) {
    let root = tempfile::tempdir().unwrap();
    let ws = Workspace::under(root.path());
    for table_crate in TableCrate::ALL {
        fs::create_dir_all(ws.tables_dir(table_crate)).unwrap();
        fs::write(
            ws.tables_mod(table_crate),
            format!(
                "//! Tables.\n\npub mod toy;\n\n{}\npub const MANIFEST: &[TableInfo] = &[];\n{}\n",
                emit::MANIFEST_BEGIN,
                emit::MANIFEST_END
            ),
        )
        .unwrap();
    }
    fs::create_dir_all(&ws.manifests_dir).unwrap();
    fs::create_dir_all(&ws.data_dir).unwrap();
    for t in toys {
        set_value(&ws, t, 0.25);
    }
    (root, ws)
}

/// Writes toy `t`'s manifest with `value`.
fn set_value(ws: &Workspace, t: &Toy, value: f64) {
    fs::write(
        manifest_path(ws, t, ManifestKind::Full),
        format!("task = \"{}\"\n[params]\nvalue = {value:?}\n", t.name),
    )
    .unwrap();
}

/// `run <toy> --since <since>` into the workspace, the generator at `current`.
fn run(
    ws: &Workspace,
    tasks: &[&'static dyn FitTask],
    t: &'static Toy,
    since: Option<u32>,
    current: u32,
) -> Result<emit::Written, RunFitError> {
    let manifest = Manifest::load(&manifest_path(ws, t, ManifestKind::Full))?;
    let prepared = prepare(tasks, t, &manifest, ws, NonZeroUsize::MIN)?;
    Ok(write_table(
        &prepared.emission(),
        Destination::Tables {
            workspace: ws,
            tasks,
            since,
            current,
        },
    )?)
}

/// The rows of `tables::MANIFEST` as the emitter wrote them into `table_crate`'s `tables/mod.rs`.
fn manifest_in(ws: &Workspace, table_crate: TableCrate) -> Vec<TableInfo> {
    let text = fs::read_to_string(ws.tables_mod(table_crate)).unwrap();
    text.split("    TableInfo {\n")
        .skip(1)
        .map(|row| {
            let field = |key: &str| {
                row.lines()
                    .find_map(|line| line.trim().strip_prefix(key))
                    .and_then(|value| value.strip_suffix(','))
                    .unwrap_or_else(|| panic!("no `{key}` in {row}"))
            };
            TableInfo {
                name: Box::leak(field("name: ").trim_matches('"').into()),
                revision: field("revision: ").parse().unwrap(),
                since_generator_version: field("since_generator_version: ").parse().unwrap(),
                provisional: field("provisional: ").parse().unwrap(),
            }
        })
        .collect()
}

/// Each crate's `tables::MANIFEST` as the emitter wrote it.
fn manifests_of(ws: &Workspace) -> Vec<(TableCrate, Vec<TableInfo>)> {
    TableCrate::ALL
        .into_iter()
        .map(|table_crate| (table_crate, manifest_in(ws, table_crate)))
        .collect()
}

/// The staleness check on the workspace against `manifests`, the generator at `current`.
fn check_with(
    ws: &Workspace,
    tasks: &[&'static dyn FitTask],
    manifests: &[(TableCrate, Vec<TableInfo>)],
    current: u32,
    rerun: Rerun,
) -> CheckReport {
    let manifests: Vec<(TableCrate, &[TableInfo])> = manifests
        .iter()
        .map(|(table_crate, rows)| (*table_crate, rows.as_slice()))
        .collect();
    check(
        &CheckInputs {
            workspace: ws,
            tasks,
            manifests: &manifests,
            current,
        },
        rerun,
        NonZeroUsize::MIN,
    )
}

/// The staleness check on the workspace, with the fast reruns, the generator at `current`.
fn check_at(ws: &Workspace, tasks: &[&'static dyn FitTask], current: u32) -> CheckReport {
    check_with(ws, tasks, &manifests_of(ws), current, Rerun::Fast)
}

/// The problems the check reports.
fn problems(report: &CheckReport) -> Vec<Problem> {
    report.findings.iter().map(|f| f.problem.clone()).collect()
}

#[test]
fn a_fresh_table_passes_and_its_manifest_row_is_written() {
    let t = toy("toy", "toy.rs");
    let (_root, ws) = workspace(&[t]);
    let tasks: [&'static dyn FitTask; 1] = [t];
    let written = run(&ws, &tasks, t, Some(11), 11).unwrap();
    assert_eq!(written.since, 11);
    let report = check_at(&ws, &tasks, 11);
    assert!(report.passed(), "{report}");
    assert_eq!(report.fresh.len(), 1, "{report}");
    let module = fs::read_to_string(ws.tables_mod(TableCrate::Sim)).unwrap();
    assert!(
        module.contains(
            "pub const MANIFEST: &[TableInfo] = &[\n    TableInfo {\n        name: \"toy\",\n        \
             revision: 2,\n        since_generator_version: 11,\n        provisional: false,\n    \
             },\n];\n"
        ),
        "{module}"
    );
    assert!(
        module.starts_with("//! Tables.\n\npub mod toy;\n\n"),
        "{module}"
    );
    let table = fs::read_to_string(ws.tables_dir(TableCrate::Sim).join("toy.rs")).unwrap();
    assert!(
        table.contains("//! since-generator-version: 11\n"),
        "{table}"
    );
    assert!(table.contains("pub const VALUES: [f64; 2] = [\n    0.25,\n    0.5,\n];\n"));
    // A rerun that changes nothing keeps the table's version even after a bump.
    let again = run(&ws, &tasks, t, Some(12), 12).unwrap();
    assert!(!again.body_changed);
    assert_eq!(again.since, 11);
}

#[test]
fn check_reports_hand_edited_table() {
    let t = toy("toy", "toy.rs");
    let (_root, ws) = workspace(&[t]);
    let tasks: [&'static dyn FitTask; 1] = [t];
    run(&ws, &tasks, t, Some(11), 11).unwrap();
    let path = ws.tables_dir(TableCrate::Sim).join("toy.rs");
    let text = fs::read_to_string(&path).unwrap().replace("0.5,", "0.6,");
    fs::write(&path, text).unwrap();
    let report = check_at(&ws, &tasks, 11);
    assert_eq!(problems(&report), [Problem::HandEdited], "{report}");
    assert!(
        report
            .to_string()
            .contains("STALE    toy: the body differs"),
        "{report}"
    );
}

#[test]
fn check_reports_changed_manifest() {
    let t = toy("toy", "toy.rs");
    let (_root, ws) = workspace(&[t]);
    let tasks: [&'static dyn FitTask; 1] = [t];
    run(&ws, &tasks, t, Some(11), 11).unwrap();
    set_value(&ws, t, 0.75);
    let report = check_at(&ws, &tasks, 11);
    assert!(
        problems(&report).contains(&Problem::ChangedInputs),
        "{report}"
    );
}

#[test]
fn check_reports_fingerprint_drift() {
    let t = toy("toy", "toy.rs");
    let (_root, ws) = workspace(&[t]);
    let tasks: [&'static dyn FitTask; 1] = [t];
    run(&ws, &tasks, t, Some(11), 11).unwrap();
    *t.probe.lock().unwrap() = 1.5 * (1.0 + 1e-12);
    let within = check_with(&ws, &tasks, &manifests_of(&ws), 11, Rerun::None);
    // Within the tolerance the table is fresh; a rerun would write the new probes' hash into the
    // header, so `--rerun-fast` asks for one.
    assert!(within.passed(), "{within}");
    assert_eq!(
        problems(&check_at(&ws, &tasks, 11)),
        [Problem::RerunDiffers]
    );
    *t.probe.lock().unwrap() = 1.5 * (1.0 + 1e-6);
    let report = check_at(&ws, &tasks, 11);
    assert!(
        problems(&report).contains(&Problem::FingerprintDrift {
            probe: "probe".to_owned()
        }),
        "{report}"
    );
}

#[test]
fn check_rejects_since_version_above_current() {
    let t = toy("toy", "toy.rs");
    let (_root, ws) = workspace(&[t]);
    let tasks: [&'static dyn FitTask; 1] = [t];
    run(&ws, &tasks, t, Some(12), 12).unwrap();
    let report = check_at(&ws, &tasks, 11);
    assert!(
        problems(&report).contains(&Problem::SinceAhead {
            since: 12,
            current: 11
        }),
        "{report}"
    );
}

/// A provisional table fails the check from P15.T12 on, as it only warned before, and the failure
/// names the table, the task that must fit it and the plan task its header says made it.
#[test]
fn check_fails_on_a_provisional_table_naming_it_and_its_task() {
    let t = toy_provisional("toy", "toy.rs", "P15.T99");
    let (_root, ws) = workspace(&[t]);
    let tasks: [&'static dyn FitTask; 1] = [t];
    run(&ws, &tasks, t, Some(11), 11).unwrap();
    assert!(
        LockFile::read(&ws.lock_path)
            .unwrap()
            .get("toy")
            .unwrap()
            .provisional
    );
    for rerun in [Rerun::None, Rerun::Fast] {
        let report = check_with(&ws, &tasks, &manifests_of(&ws), 11, rerun);
        assert!(!report.passed(), "{report}");
        assert!(report.warnings.is_empty(), "{report}");
        let tables: Vec<&str> = report.findings.iter().map(|f| f.table.as_str()).collect();
        assert_eq!(tables, ["toy"], "{report}");
        assert_eq!(
            problems(&report),
            [Problem::Provisional {
                task: Some("toy".to_owned()),
                by: Some("P15.T99".to_owned()),
                since: 11
            }],
            "{report}"
        );
        assert!(
            report.to_string().contains(
                "STALE    toy: provisional, a scratch stand-in by P15.T99 since generator version \
                 11: task `toy` must fit it, as no table may stay provisional (P15.T12)\n0 fresh, \
                 0 warnings, 1 failures"
            ),
            "{report}"
        );
    }
}

/// The listing gives each task's table its lock entry's revision and figures and its state, and
/// then lists each table file whose own header no task writes: a hand-made header beside a task's
/// block, or a file of its own, in the grammar or not; a file whose comment holds no header is no
/// table.
#[test]
fn the_listing_shows_provisional_and_unregistered_tables() {
    let alpha = toy("alpha", "alpha.rs");
    let beta = toy_provisional("beta", "beta.rs", "P15.T99");
    let gamma = toy_beside_code("gamma", "beside.rs");
    let (_root, ws) = workspace(&[alpha, beta, gamma]);
    let tasks: [&'static dyn FitTask; 3] = [alpha, beta, gamma];
    let tables = ws.tables_dir(TableCrate::Sim);
    fs::write(
        tables.join("beside.rs"),
        "//! Shapes of a table.\n//!\n//! @provisional: hand-made scratch values.\n\npub const \
         SCRATCH: f64 = 1.0;\n\n// @begin-table gamma\n// @end-table gamma\n",
    )
    .unwrap();
    for t in [alpha, beta, gamma] {
        run(&ws, &tasks, t, Some(11), 11).unwrap();
    }
    fs::write(
        tables.join("hand.rs"),
        "//! A constant.\n//!\n//! @static.\n//! since-generator-version: 3\n//! source: a test\n\n\
         pub const X: f64 = 1.0;\n",
    )
    .unwrap();
    fs::write(
        tables.join("plain.rs"),
        "//! Not a table.\n\npub const Y: f64 = 2.0;\n",
    )
    .unwrap();
    let manifests = manifests_of(&ws);
    let manifests: Vec<(TableCrate, &[TableInfo])> = manifests
        .iter()
        .map(|(table_crate, rows)| (*table_crate, rows.as_slice()))
        .collect();
    let listed = listing(&CheckInputs {
        workspace: &ws,
        tasks: &tasks,
        manifests: &manifests,
        current: 11,
    })
    .unwrap();
    let task_row = |name: &str, path: &str, state| ListRow {
        name: name.to_owned(),
        revision: Some(2),
        class: Some(TaskClass::Fast),
        state,
        path: path.to_owned(),
        acceptance: Some("value 0.25".to_owned()),
    };
    let unregistered = |name: &str, kind: &str| ListRow {
        name: name.to_owned(),
        revision: None,
        class: None,
        state: ListState::Unregistered(kind.to_owned()),
        path: format!("{name}.rs"),
        acceptance: None,
    };
    assert_eq!(
        listed.rows,
        [
            task_row("alpha", "alpha.rs", ListState::Fresh),
            task_row("beta", "beta.rs", ListState::Provisional),
            task_row("gamma", "beside.rs", ListState::Fresh),
            unregistered("beside", "provisional"),
            unregistered("hand", "static"),
        ]
    );
    let text = listed.to_string();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        lines[0],
        "name    revision  class  runtime  state                     table      acceptance"
    );
    assert_eq!(
        lines[2],
        "beta    2         fast   < 1 min  provisional               beta.rs    value 0.25"
    );
    assert_eq!(
        lines[5],
        "hand    -         -      -        unregistered static       hand.rs    -"
    );
}

/// A lock entry that no task makes is listed once, from its lock entry and not again as an
/// unregistered file, and fails the check while provisional; a stale table is listed as stale.
#[test]
fn the_listing_shows_a_lock_entry_no_task_makes_once_and_a_stale_table() {
    let alpha = toy("alpha", "alpha.rs");
    let (_root, ws) = workspace(&[alpha]);
    let tasks: [&'static dyn FitTask; 1] = [alpha];
    let tables = ws.tables_dir(TableCrate::Sim);
    let scratch = "//! A scratch table.\n//!\n//! @provisional by P11.T15. Do not edit.\n//! \
                   since-generator-version: 9\n//! source: a test\n\npub const Z: f64 = 3.0;\n";
    fs::write(tables.join("scratch.rs"), scratch).unwrap();
    let mut lock = LockFile::default();
    lock.upsert(LockEntry {
        name: "scratch".to_owned(),
        path: "scratch.rs".to_owned(),
        task: None,
        revision: 0,
        inputs_sha256: None,
        body_sha256: emit::TableText::of_file(scratch).body_sha256(),
        fingerprint: Vec::new(),
        since_generator_version: 9,
        provisional: true,
        acceptance: None,
    });
    lock.write(&ws.lock_path).unwrap();
    run(&ws, &tasks, alpha, Some(11), 11).unwrap();
    let path = tables.join("alpha.rs");
    let text = fs::read_to_string(&path).unwrap().replace("0.5,", "0.6,");
    fs::write(&path, text).unwrap();
    let manifests = manifests_of(&ws);
    let report = check_with(&ws, &tasks, &manifests, 11, Rerun::None);
    let findings: Vec<(&str, &Problem)> = report
        .findings
        .iter()
        .map(|f| (f.table.as_str(), &f.problem))
        .collect();
    assert_eq!(
        findings,
        [
            ("alpha", &Problem::HandEdited),
            (
                "scratch",
                &Problem::Provisional {
                    task: None,
                    by: Some("P11.T15".to_owned()),
                    since: 9
                }
            ),
        ],
        "{report}"
    );
    assert!(
        report.to_string().contains(
            "STALE    scratch: provisional, a scratch stand-in by P11.T15 since generator version \
             9: no registered task fits it"
        ),
        "{report}"
    );
    let manifests: Vec<(TableCrate, &[TableInfo])> = manifests
        .iter()
        .map(|(table_crate, rows)| (*table_crate, rows.as_slice()))
        .collect();
    let listed = listing(&CheckInputs {
        workspace: &ws,
        tasks: &tasks,
        manifests: &manifests,
        current: 11,
    })
    .unwrap();
    assert_eq!(
        listed.rows,
        [
            ListRow {
                name: "alpha".to_owned(),
                revision: Some(2),
                class: Some(TaskClass::Fast),
                state: ListState::Stale,
                path: "alpha.rs".to_owned(),
                acceptance: Some("value 0.25".to_owned()),
            },
            ListRow {
                name: "scratch".to_owned(),
                revision: Some(0),
                class: None,
                state: ListState::Provisional,
                path: "scratch.rs".to_owned(),
                acceptance: None,
            },
        ]
    );
}

#[test]
fn check_reports_a_missing_table_an_unlocked_task_and_a_disagreeing_manifest() {
    let a = toy("alpha", "alpha.rs");
    let b = toy("beta", "beta.rs");
    let (_root, ws) = workspace(&[a, b]);
    let tasks: [&'static dyn FitTask; 2] = [a, b];
    run(&ws, &tasks, a, Some(11), 11).unwrap();
    let report = check_at(&ws, &tasks, 11);
    assert!(
        matches!(&problems(&report)[..], [Problem::NotRegistered(why)] if why.contains("beta")),
        "{report}"
    );
    fs::remove_file(ws.tables_dir(TableCrate::Sim).join("alpha.rs")).unwrap();
    assert!(problems(&check_at(&ws, &[a], 11)).contains(&Problem::Missing));
    let stale = check_with(&ws, &[a], &[], 11, Rerun::None);
    assert!(
        stale
            .findings
            .iter()
            .any(|f| f.table == "hyperion_sim::tables::MANIFEST"
                && matches!(f.problem, Problem::Disagrees(_))),
        "{stale}"
    );
}

/// A task whose table lives in a second crate (the sky's, from the crate split) writes its table
/// and its `MANIFEST` row into that crate's `tables/`, and the check finds both there. A file of
/// one name in each crate is two files, each its own task's, and the lock file names a table's
/// file alone, whichever crate holds it.
#[test]
fn a_second_crates_tables_and_manifest_rows_stay_in_that_crate() {
    let alpha = toy("alpha", "alpha.rs");
    let beta = toy_in(TableCrate::Sky, "beta", "beta.rs");
    let gamma = toy("gamma", "same.rs");
    let delta = toy_in(TableCrate::Sky, "delta", "same.rs");
    let (_root, ws) = workspace(&[alpha, beta, gamma, delta]);
    let tasks: [&'static dyn FitTask; 4] = [alpha, beta, gamma, delta];
    for t in [alpha, beta, gamma, delta] {
        run(&ws, &tasks, t, Some(11), 11).unwrap();
    }
    let (sim, sky) = (
        ws.tables_dir(TableCrate::Sim),
        ws.tables_dir(TableCrate::Sky),
    );
    assert!(sim.join("alpha.rs").exists() && !sky.join("alpha.rs").exists());
    assert!(sky.join("beta.rs").exists() && !sim.join("beta.rs").exists());
    for dir in [sim, sky] {
        let same = fs::read_to_string(dir.join("same.rs")).unwrap();
        assert!(!same.contains(emit::BLOCK_BEGIN), "{same}");
    }
    let names = |table_crate| -> Vec<&str> {
        manifest_in(&ws, table_crate)
            .iter()
            .map(|row| row.name)
            .collect()
    };
    assert_eq!(names(TableCrate::Sim), ["alpha", "gamma"]);
    assert_eq!(names(TableCrate::Sky), ["beta", "delta"]);
    let lock = LockFile::read(&ws.lock_path).unwrap();
    assert_eq!(lock.get("beta").unwrap().path, "beta.rs");
    assert_eq!(lock.get("delta").unwrap().path, "same.rs");
    let report = check_at(&ws, &tasks, 11);
    assert!(report.passed(), "{report}");
    assert_eq!(report.fresh.len(), 4, "{report}");

    // Every row in the sim's `MANIFEST`, in name order, as a move that left the sky's rows behind
    // would have it: both crates' `MANIFEST`s disagree with the lock file.
    let mut rows: Vec<TableInfo> = manifests_of(&ws)
        .into_iter()
        .flat_map(|(_, rows)| rows)
        .collect();
    rows.sort_by_key(|row| row.name);
    let misplaced = check_with(&ws, &tasks, &[(TableCrate::Sim, rows)], 11, Rerun::None);
    let tables: Vec<&str> = misplaced
        .findings
        .iter()
        .map(|f| f.table.as_str())
        .collect();
    assert_eq!(
        tables,
        [
            "hyperion_sim::tables::MANIFEST",
            "hyperion_sky::tables::MANIFEST"
        ],
        "{misplaced}"
    );
}

/// A crate that holds no locked table is left alone, so a table is written, and checked, with no
/// `tables/` of the other crate at all: the repository's state until the crate split's move
/// (CS.T2) creates the sky's.
#[test]
fn a_crate_without_tables_needs_no_tables_directory() {
    let t = toy("toy", "toy.rs");
    let (_root, ws) = workspace(&[t]);
    fs::remove_dir_all(ws.tables_dir(TableCrate::Sky)).unwrap();
    let tasks: [&'static dyn FitTask; 1] = [t];
    run(&ws, &tasks, t, Some(11), 11).unwrap();
    let sim = manifest_in(&ws, TableCrate::Sim);
    assert_eq!(sim.iter().map(|row| row.name).collect::<Vec<_>>(), ["toy"]);
    assert!(!ws.tables_dir(TableCrate::Sky).exists());
    let report = check_with(&ws, &tasks, &[(TableCrate::Sim, sim)], 11, Rerun::Fast);
    assert!(report.passed(), "{report}");
}

#[test]
fn emitter_refuses_changed_body_without_new_since() {
    let t = toy("toy", "toy.rs");
    let (_root, ws) = workspace(&[t]);
    let tasks: [&'static dyn FitTask; 1] = [t];
    assert!(matches!(
        run(&ws, &tasks, t, None, 11),
        Err(RunFitError::Emit(EmitTableError::SinceRequired))
    ));
    assert!(matches!(
        run(&ws, &tasks, t, Some(10), 11),
        Err(RunFitError::Emit(EmitTableError::SinceNotCurrent {
            since: 10,
            current: 11
        }))
    ));
    run(&ws, &tasks, t, Some(11), 11).unwrap();
    let before = fs::read_to_string(ws.tables_dir(TableCrate::Sim).join("toy.rs")).unwrap();
    set_value(&ws, t, 0.75);
    match run(&ws, &tasks, t, Some(11), 11) {
        Err(RunFitError::Emit(EmitTableError::ChangedBodyWithoutNewSince { table, since: 11 })) => {
            assert_eq!(table, "toy");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(
        fs::read_to_string(ws.tables_dir(TableCrate::Sim).join("toy.rs")).unwrap(),
        before,
        "a refused write leaves the table alone"
    );
    let written = run(&ws, &tasks, t, Some(12), 12).unwrap();
    assert!(written.body_changed);
    assert_eq!(written.since, 12);
    assert!(check_at(&ws, &tasks, 12).passed());
}

#[test]
fn two_tasks_share_a_file_without_touching_each_other() {
    let a = toy("alpha", "shared.rs");
    let b = toy("beta", "shared.rs");
    let (_root, ws) = workspace(&[a, b]);
    let tasks: [&'static dyn FitTask; 2] = [a, b];
    let path = ws.tables_dir(TableCrate::Sim).join("shared.rs");
    fs::write(&path, "//! Two tasks' table.\n").unwrap();
    run(&ws, &tasks, a, Some(11), 11).unwrap();
    run(&ws, &tasks, b, Some(11), 11).unwrap();
    let both = fs::read_to_string(&path).unwrap();
    assert!(both.starts_with("//! Two tasks' table.\n\n// @begin-table alpha\n// The toy"));
    let block_of = |text: &str, name: &str| {
        let begin = text.find(&format!("// @begin-table {name}\n")).unwrap();
        let end_marker = format!("// @end-table {name}\n");
        let end = text.find(&end_marker).unwrap() + end_marker.len();
        text[begin..end].to_owned()
    };
    let beta_before = block_of(&both, "beta");
    assert!(
        beta_before.contains("// @generated by hyperion-fit"),
        "{beta_before}"
    );
    assert!(
        check_at(&ws, &tasks, 11).passed(),
        "{}",
        check_at(&ws, &tasks, 11)
    );
    set_value(&ws, a, 0.75);
    run(&ws, &tasks, a, Some(12), 12).unwrap();
    let after = fs::read_to_string(&path).unwrap();
    assert_eq!(block_of(&after, "beta"), beta_before);
    assert!(block_of(&after, "alpha").contains("    0.75,\n    1.5,\n"));
    let lock = LockFile::read(&ws.lock_path).unwrap();
    assert_eq!(lock.entries().len(), 2);
    assert_ne!(
        lock.get("alpha").unwrap().body_sha256,
        lock.get("beta").unwrap().body_sha256
    );
    let report = check_at(&ws, &tasks, 12);
    assert!(report.passed(), "{report}");
}

/// A task alone in writing a file that holds code of its own writes its block and nothing else:
/// into its markers where they stand, before the code after them, and the check reads the block.
#[test]
fn a_lone_task_beside_code_writes_only_its_block() {
    let t = toy_beside_code("gamma", "beside.rs");
    let (_root, ws) = workspace(&[t]);
    let tasks: [&'static dyn FitTask; 1] = [t];
    assert_eq!(placement_among(&tasks, t), Placement::Block);
    let path = ws.tables_dir(TableCrate::Sim).join("beside.rs");
    let before =
        "//! Shapes of a table, and its scratch values.\n\npub const SCRATCH: f64 = 1.0;\n\n";
    let after = "\n#[cfg(test)]\nmod tests {}\n";
    fs::write(
        &path,
        format!("{before}// @begin-table gamma\n// @end-table gamma\n{after}"),
    )
    .unwrap();
    run(&ws, &tasks, t, Some(11), 11).unwrap();
    let text = fs::read_to_string(&path).unwrap();
    assert!(
        text.starts_with(&format!("{before}// @begin-table gamma\n// The toy")),
        "{text}"
    );
    assert!(
        text.ends_with(&format!("// @end-table gamma\n{after}")),
        "{text}"
    );
    assert!(text.contains("    0.25,\n    0.5,\n"), "{text}");
    let report = check_at(&ws, &tasks, 11);
    assert!(report.passed(), "{report}");
}

/// The repository's own tables are fresh: what `just fit-check` checks, here without the reruns.
#[test]
fn the_committed_tables_are_fresh() {
    let report = check(
        &CheckInputs {
            workspace: &Workspace::repository(),
            tasks: registry(),
            manifests: REPOSITORY_MANIFESTS,
            current: GENERATOR_VERSION.get(),
        },
        Rerun::None,
        NonZeroUsize::MIN,
    );
    assert!(report.passed(), "{report}");
    assert_eq!(report.fresh.len(), registry().len(), "{report}");
}

/// Every committed table declares its task's items, in order: a file's, or its block's where
/// tasks share the file.
#[test]
fn every_committed_table_declares_its_tasks_items() {
    let ws = Workspace::repository();
    for task in registry() {
        let text =
            fs::read_to_string(ws.tables_dir(task.table_crate()).join(task.table_path())).unwrap();
        let body = match hyperion_fit::pipeline::placement(*task) {
            Placement::File => hyperion_fit::emit::TableText::of_file(&text).body,
            Placement::Block => {
                hyperion_fit::emit::TableText::of_block(&text, task.name())
                    .expect("a shared file's blocks parse")
                    .expect("a shared file holds a block of each of its tasks")
                    .body
            }
        };
        assert_eq!(common::declared(&body), task.items(), "{}", task.name());
    }
}

/// Nothing may depend on the fitting crate (plan 15, Design note 1).
#[test]
fn no_runtime_crate_depends_on_hyperion_fit() {
    /// Whether any key of `value`, at any depth, is `hyperion-fit`.
    fn names(value: &toml::Value) -> bool {
        match value {
            toml::Value::Table(table) => table
                .iter()
                .any(|(key, v)| key == "hyperion-fit" || names(v)),
            toml::Value::Array(items) => items.iter().any(names),
            _ => false,
        }
    }
    let crates = concat!(env!("CARGO_MANIFEST_DIR"), "/..");
    for name in [
        "hyperion-base",
        "hyperion-sim",
        "hyperion-surface",
        "hyperion-protocol",
        "hyperion-server",
    ] {
        let text = fs::read_to_string(format!("{crates}/{name}/Cargo.toml")).unwrap();
        let manifest: toml::Value = toml::from_str(&text).unwrap();
        assert!(
            !names(&manifest),
            "{name} names hyperion-fit in its Cargo.toml"
        );
    }
}
