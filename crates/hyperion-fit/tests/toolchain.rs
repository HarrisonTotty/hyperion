//! The emitter, the lock file and the staleness check against toy tasks in a temporary workspace
//! (plan 15, P15.T2), and the check on the repository's own tables.

use std::fs;
use std::num::NonZeroUsize;
use std::sync::Mutex;

use hyperion_fit::check::{CheckInputs, CheckReport, Problem, Rerun, check};
use hyperion_fit::emit::{
    Destination, EmitTableError, RustTable, TableItem, Workspace, write_table,
};
use hyperion_fit::manifest::{LockFile, Manifest, SimFingerprint};
use hyperion_fit::pipeline::{ManifestKind, manifest_path, prepare};
use hyperion_fit::task::{FitTask, RunTaskError, TaskClass, TaskOutput, registry};
use hyperion_fit::{RunFitError, emit};
use hyperion_sim::GENERATOR_VERSION;
use hyperion_sim::tables::{MANIFEST, TableInfo};
use tempfile::TempDir;

/// A toy fit: its table is `[value, 2 × value]` from its manifest, and its fingerprint one probe
/// the test can move.
#[derive(Debug)]
struct Toy {
    name: &'static str,
    path: &'static str,
    probe: Mutex<f64>,
}

impl FitTask for Toy {
    fn name(&self) -> &'static str {
        self.name
    }

    fn class(&self) -> TaskClass {
        TaskClass::Fast
    }

    fn table_path(&self) -> &'static str {
        self.path
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
            provisional: None,
        })
    }
}

/// A toy task that lives as long as the test binary.
fn toy(name: &'static str, path: &'static str) -> &'static Toy {
    Box::leak(Box::new(Toy {
        name,
        path,
        probe: Mutex::new(1.5),
    }))
}

/// A workspace with a `tables/mod.rs` holding the manifest's markers and each toy's manifest.
fn workspace(toys: &[&Toy]) -> (TempDir, Workspace) {
    let root = tempfile::tempdir().unwrap();
    let ws = Workspace::under(root.path());
    fs::create_dir_all(&ws.tables_dir).unwrap();
    fs::create_dir_all(&ws.manifests_dir).unwrap();
    fs::create_dir_all(&ws.data_dir).unwrap();
    fs::write(
        ws.tables_mod(),
        format!(
            "//! Tables.\n\npub mod toy;\n\n{}\npub const MANIFEST: &[TableInfo] = &[];\n{}\n",
            emit::MANIFEST_BEGIN,
            emit::MANIFEST_END
        ),
    )
    .unwrap();
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
            since,
            current,
        },
    )?)
}

/// `tables::MANIFEST` as the workspace's lock file says it should be.
fn manifest_of(ws: &Workspace) -> Vec<TableInfo> {
    LockFile::read(&ws.lock_path)
        .unwrap()
        .entries()
        .iter()
        .map(|e| TableInfo {
            name: Box::leak(e.name.clone().into_boxed_str()),
            revision: e.revision,
            since_generator_version: e.since_generator_version,
            provisional: e.provisional,
        })
        .collect()
}

/// The staleness check on the workspace, the generator at `current`.
fn check_at(ws: &Workspace, tasks: &[&'static dyn FitTask], current: u32) -> CheckReport {
    let manifest = manifest_of(ws);
    check(
        &CheckInputs {
            workspace: ws,
            tasks,
            manifest: &manifest,
            current,
        },
        Rerun::Fast,
        NonZeroUsize::MIN,
    )
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
    let module = fs::read_to_string(ws.tables_mod()).unwrap();
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
    let table = fs::read_to_string(ws.tables_dir.join("toy.rs")).unwrap();
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
    let path = ws.tables_dir.join("toy.rs");
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
    let manifest = manifest_of(&ws);
    let within = check(
        &CheckInputs {
            workspace: &ws,
            tasks: &tasks,
            manifest: &manifest,
            current: 11,
        },
        Rerun::None,
        NonZeroUsize::MIN,
    );
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
    fs::remove_file(ws.tables_dir.join("alpha.rs")).unwrap();
    assert!(problems(&check_at(&ws, &[a], 11)).contains(&Problem::Missing));
    let stale = check(
        &CheckInputs {
            workspace: &ws,
            tasks: &[a],
            manifest: &[],
            current: 11,
        },
        Rerun::None,
        NonZeroUsize::MIN,
    );
    assert!(
        stale
            .findings
            .iter()
            .any(|f| f.table == "tables::MANIFEST" && matches!(f.problem, Problem::Disagrees(_))),
        "{stale}"
    );
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
    let before = fs::read_to_string(ws.tables_dir.join("toy.rs")).unwrap();
    set_value(&ws, t, 0.75);
    match run(&ws, &tasks, t, Some(11), 11) {
        Err(RunFitError::Emit(EmitTableError::ChangedBodyWithoutNewSince { table, since: 11 })) => {
            assert_eq!(table, "toy");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(
        fs::read_to_string(ws.tables_dir.join("toy.rs")).unwrap(),
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
    let path = ws.tables_dir.join("shared.rs");
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

/// The repository's own tables are fresh: what `just fit-check` checks, here without the reruns.
#[test]
fn the_committed_tables_are_fresh() {
    let report = check(
        &CheckInputs {
            workspace: &Workspace::repository(),
            tasks: registry(),
            manifest: MANIFEST,
            current: GENERATOR_VERSION.get(),
        },
        Rerun::None,
        NonZeroUsize::MIN,
    );
    assert!(report.passed(), "{report}");
    assert_eq!(
        report.fresh.len() + report.warnings.len(),
        registry().len(),
        "{report}"
    );
}

/// Every committed table declares its task's items, in order.
#[test]
fn every_committed_table_declares_its_tasks_items() {
    let ws = Workspace::repository();
    for task in registry() {
        let text = fs::read_to_string(ws.tables_dir.join(task.table_path())).unwrap();
        let body = hyperion_fit::emit::TableText::of_file(&text).body;
        let declared: Vec<&str> = body
            .lines()
            .filter_map(|l| {
                l.strip_prefix("pub const ")
                    .or_else(|| l.strip_prefix("pub static "))
            })
            .filter_map(|l| l.split(':').next())
            .collect();
        assert_eq!(declared, task.items(), "{}", task.name());
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
    for name in ["hyperion-sim", "hyperion-protocol", "hyperion-server"] {
        let text = fs::read_to_string(format!("{crates}/{name}/Cargo.toml")).unwrap();
        let manifest: toml::Value = toml::from_str(&text).unwrap();
        assert!(
            !names(&manifest),
            "{name} names hyperion-fit in its Cargo.toml"
        );
    }
}
