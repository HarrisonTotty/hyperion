//! A replay of the checked-in capture on the default adapter, into offscreen textures with no
//! window or presentation, writes a results file in the descent spike's schema (R05.T15.c).
//!
//! It needs a GPU adapter (any wgpu backend), so it is ignored in `just ci`, which builds and runs
//! this tool's other tests, and run by `just test-gpu-replay`, in `just ci-slow`.

use std::path::{Path, PathBuf};

use gpu_replay::capture::Capture;
use gpu_replay::results::{RESULTS_SCHEMA, RESULTS_VERSION, write_results};
use gpu_replay::run::replay_offscreen;
use serde_json::Value;

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/small")
}

/// Every `{ value, reason }` figure is present with a null reason, or null with a reason, as the
/// client's `validateResults` requires.
fn figure_problems(node: &Value, path: &str, problems: &mut Vec<String>) {
    match node {
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                figure_problems(item, &format!("{path}[{i}]"), problems);
            }
        }
        Value::Object(map) => {
            if map.len() == 2 && map.contains_key("value") && map.contains_key("reason") {
                let value = &map["value"];
                let reason = &map["reason"];
                if value.is_null() && reason.as_str().is_none_or(str::is_empty) {
                    problems.push(format!("{path} is null without a reason"));
                }
                if !value.is_null() && !reason.is_null() {
                    problems.push(format!("{path} has both a value and a reason"));
                }
                figure_problems(value, &format!("{path}.value"), problems);
                return;
            }
            for (key, item) in map {
                figure_problems(item, &format!("{path}.{key}"), problems);
            }
        }
        _ => {}
    }
}

#[test]
#[ignore = "gpu: needs a GPU adapter; `just test-gpu-replay` runs it"]
fn an_offscreen_replay_writes_a_results_file_in_the_schema() {
    let capture = Capture::read(&fixture()).expect("the fixture is a capture");
    let figures = replay_offscreen(&capture, &fixture(), None).expect("the replay runs");
    assert!(!figures.presented());
    // The module that is invalid on purpose is refused by wgpu as by naga, and nothing else is:
    // not the canvas's sRGB view, which the engine's configuration allows.
    assert_eq!(figures.errors().len(), 1, "{:?}", figures.errors());
    assert!(
        figures.errors()[0].contains("broken on purpose"),
        "{:?}",
        figures.errors()
    );
    assert_eq!(figures.findings(), &[] as &[String]);
    let dir = std::env::temp_dir().join(format!("gpu-replay-test-{}", std::process::id()));
    let path = write_results(&dir, &figures).expect("the results are written");
    let text = std::fs::read_to_string(&path).expect("the results are readable");
    std::fs::remove_dir_all(&dir).expect("the test's directory is removed");
    let results: Value = serde_json::from_str(&text).expect("the results are JSON");

    assert_eq!(results["schema"], RESULTS_SCHEMA);
    assert_eq!(results["version"], RESULTS_VERSION);
    for key in [
        "startedAt",
        "platform",
        "launchMode",
        "setting",
        "seed",
        "timer",
    ] {
        assert!(results["run"][key].is_string(), "run.{key} is a string");
    }
    // The setting and seed come from the capture's `meta`.
    assert_eq!(results["run"]["setting"], "low");
    assert_eq!(results["run"]["seed"], "7");
    assert!(results["run"]["machine"]["loadAverage"].is_array());
    assert!(results["run"]["switches"].is_array());
    for key in ["levels", "streaming"] {
        assert!(results[key].is_array(), "{key} is a list");
    }
    for key in [
        "frames",
        "gpu",
        "mainThread",
        "memory",
        "criteria",
        "uploads",
        "pipelines",
    ] {
        assert!(results[key].is_object(), "{key} is present");
    }
    let rows = results["criteria"]["whole"]
        .as_array()
        .expect("criteria rows");
    assert!(!rows.is_empty());
    for row in rows {
        let verdict = row["verdict"].as_str().expect("a verdict");
        assert!(
            ["pass", "fail", "marginal", "not-measured"].contains(&verdict),
            "{verdict}"
        );
    }
    assert_eq!(
        results["run"]["periodMs"]["reason"],
        "an offscreen replay presents nothing"
    );
    assert_eq!(results["frames"]["source"], "gpu-completion");
    // Two frames, so one interval between their GPU ends.
    assert_eq!(results["frames"]["gpuCompletion"]["value"]["count"], 1);
    // Both frames' pass times are read back.
    assert_eq!(
        results["gpu"]["incompleteFrames"]["value"],
        serde_json::json!({ "dropped": 0, "partial": 0, "frames": 2 })
    );
    // The fixture's one pass is the terrain row's (its `passRows`), and the replay times it: the
    // rest of the frame is terrain alone, against the low setting's 14 + 4 ms.
    let joint = rows
        .iter()
        .find(|row| row["id"] == "terrain-atmosphere")
        .expect("the terrain-atmosphere row");
    assert!(joint["value"].is_number(), "{joint}");
    assert_eq!(joint["limit"], 18.0);
    let parts = &results["gpu"]["rows"];
    assert!(parts["terrain"]["p95Ms"]["value"].is_number(), "{parts}");
    assert_eq!(
        parts["atmosphere"]["p95Ms"]["reason"],
        "no timed atmosphere pass"
    );
    assert_eq!(parts["atmosphere"]["overEstimate"], Value::Null);
    let mut problems = Vec::new();
    figure_problems(&results, "", &mut problems);
    assert_eq!(problems, Vec::<String>::new());
}
