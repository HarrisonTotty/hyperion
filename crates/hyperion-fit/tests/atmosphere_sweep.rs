//! R08.T13.a's sweep end to end on its smoke (plan R08; `decision-r08-t13-scope.md`, ruling 6
//! and "#107's floor misses"): the same output at one and four threads, its timings masked (main's
//! reading 3), its walk and its floor scan as they fall, a run stopped after its first point and
//! run again giving a whole run's output, that point traced once, and the pilot stage alone the
//! same at one and four threads.

use std::num::NonZeroUsize;
use std::path::Path;

use hyperion_fit::atmosphere::sweep::{
    SMOKE_CASE, SweepCase, SweepMode, SweepOutcome, SweepRun, SweepSettings, run_sweep,
};
use serde_json::{Value, json};

/// The fields that differ by run: the times, and the CPU figures worked out from them.
const TIMINGS: [&str; 4] = ["cpuS", "wallS", "cpuPerVertexUs", "cpuPerSampleUs"];

/// An output with its times taken out.
fn masked(text: &str) -> Value {
    fn strip(value: &mut Value) {
        match value {
            Value::Object(map) => {
                for key in TIMINGS {
                    map.remove(key);
                }
                map.values_mut().for_each(strip);
            }
            Value::Array(items) => items.iter_mut().for_each(strip),
            _ => {}
        }
    }
    let mut value: Value = serde_json::from_str(text).expect("the output is JSON");
    strip(&mut value);
    value
}

/// The smoke in `mode` on `threads` threads, keeping its points in `work`, stopping after
/// `max_points`.
fn smoke_in(
    mode: SweepMode,
    threads: usize,
    work: Option<&Path>,
    max_points: Option<usize>,
) -> SweepOutcome {
    let case = SweepCase::from_json(SMOKE_CASE).expect("the smoke's case reads");
    let mut run = SweepRun::new(
        &case,
        SweepSettings::SMOKE,
        mode,
        NonZeroUsize::new(threads).expect("not zero"),
        "the test's",
    );
    if let Some(root) = work {
        run = run.with_work(root);
    }
    if let Some(most) = max_points {
        run = run.with_max_points(most);
    }
    run_sweep(&run, &mut Vec::new()).expect("the smoke runs")
}

/// The smoke's walk on `threads` threads.
fn smoke(threads: usize, work: Option<&Path>, max_points: Option<usize>) -> SweepOutcome {
    smoke_in(SweepMode::Walk, threads, work, max_points)
}

/// The smoke's floor scan as it falls (module documentation), and its rows.
fn check_floor_scan(cells: &[Value], points: &[Value]) {
    // The floor scan as it falls: the Rayleigh column misses nothing over any ground, A* 1; the
    // g 0.75 column's two ground views pass over the black ground and fail over a brighter one,
    // the isotropic ground bounce, from 0.02 and from the walk's own 0.3: A* 0.02.
    let fields = [
        "scanPoints",
        "fixedMisses",
        "groundMisses",
        "unsortedMisses",
        "albedoStar",
    ];
    let scan: Vec<Value> = cells
        .iter()
        .map(|c| {
            Value::Object(
                fields
                    .iter()
                    .map(|&f| (f.to_owned(), c[f].clone()))
                    .collect(),
            )
        })
        .collect();
    assert_eq!(
        scan,
        [
            json!({
                "scanPoints": [897, 898, 899, 900, 901, 902, 903],
                "fixedMisses": [],
                "groundMisses": [],
                "unsortedMisses": [],
                "albedoStar": 1.0,
            }),
            json!({
                "scanPoints": [925, 926, 927, 928, 929, 930, 931],
                "fixedMisses": [],
                "groundMisses": [
                    { "geometry": "ground-s30-v60-a180", "groundAlbedo": 0.02 },
                    { "geometry": "ground-s60-v60-a90", "groundAlbedo": 0.3 },
                ],
                "unsortedMisses": [],
                "albedoStar": 0.02,
            }),
        ]
    );
    // A traced row echoes its own twin, a scan row the scan's, at seven significant figures, and
    // names its failing geometries by index.
    let case: Value = serde_json::from_str(SMOKE_CASE).expect("the smoke's case is JSON");
    let rounded = |row: &Value| -> Value {
        Value::Array(
            row.as_array()
                .expect("a twin row")
                .iter()
                .map(|x| {
                    let x = x.as_f64().expect("a radiance");
                    json!(format!("{x:.6e}").parse::<f64>().expect("a number"))
                })
                .collect(),
        )
    };
    let row_of = |index: u64| {
        points
            .iter()
            .find(|p| p["index"] == index)
            .expect("the point's row")
    };
    let floor = row_of(375);
    assert_eq!(
        floor["traced"]["twin"],
        rounded(&case["columns"][1]["twin"][0])
    );
    assert_eq!(floor["traced"]["failing"], json!([0, 1]));
    assert_eq!(
        row_of(931)["traced"]["twin"],
        rounded(&case["columns"][1]["scanTwin"][6])
    );
    assert_ne!(row_of(931)["traced"]["twin"], floor["traced"]["twin"]);
    let scan_points: Vec<u64> = points
        .iter()
        .filter(|p| p["class"] == "scan")
        .map(|p| p["index"].as_u64().expect("an index"))
        .collect();
    assert_eq!(
        scan_points,
        [
            897, 898, 899, 900, 901, 902, 903, 925, 926, 927, 928, 929, 930, 931
        ]
    );
}

#[test]
fn atmosphere_sweep_smoke_is_the_same_at_one_and_four_threads() {
    let one = smoke(1, None, None)
        .into_text()
        .expect("a whole run's output");
    let four = smoke(4, None, None)
        .into_text()
        .expect("a whole run's output");
    assert_eq!(masked(&one), masked(&four));
    assert_eq!(one.lines().count(), four.lines().count());
    let output = masked(&one);
    let cells = output["cells"].as_array().expect("cells");
    let points = output["points"].as_array().expect("points");
    for cell in cells {
        let first = cell["points"][0].as_u64().expect("its floor point");
        let floor = points
            .iter()
            .find(|p| p["index"] == first)
            .expect("the floor's row");
        assert_eq!(floor["traced"]["samples"].as_array().map(Vec::len), Some(4));
    }
    // The walk as it falls on the smoke: the Rayleigh column climbs to its last octave and tops
    // out; the g 0.75 column, which runs VROOM, keeps its two ground views as floor misses, stops
    // at its first octave on two refusals and takes the half-octave below it.
    let indices = |value: &Value| -> Vec<u64> {
        value
            .as_array()
            .expect("indices")
            .iter()
            .map(|p| p.as_u64().expect("an index"))
            .collect()
    };
    let summary: Vec<(bool, &str, Vec<u64>, usize)> = cells
        .iter()
        .map(|c| {
            (
                c["vroom"].as_bool().expect("a flag"),
                c["kind"].as_str().expect("a kind"),
                indices(&c["points"]),
                c["floorMisses"].as_array().expect("its misses").len(),
            )
        })
        .collect();
    assert_eq!(
        summary,
        [
            (false, "top", vec![315, 317, 319], 0),
            (true, "reach", vec![375, 377, 376], 2),
        ]
    );
    check_floor_scan(cells, points);
}

#[test]
fn atmosphere_sweep_resumes_where_it_stopped() {
    let work = tempfile::tempdir().expect("a work directory");
    let stopped = smoke(4, Some(work.path()), Some(1));
    assert_eq!(
        (stopped.text(), stopped.computed(), stopped.resumed()),
        (None, 1, 0)
    );
    let kept: Vec<_> = std::fs::read_dir(work.path())
        .expect("the work directory")
        .flat_map(|entry| {
            std::fs::read_dir(entry.expect("its key's directory").path()).expect("readable")
        })
        .map(|entry| entry.expect("a point").path())
        .collect();
    assert_eq!(kept.len(), 1, "{kept:?}");
    let first: Value =
        serde_json::from_str(&std::fs::read_to_string(&kept[0]).expect("the point")).expect("JSON");
    let resumed = smoke(4, Some(work.path()), None);
    let whole = smoke(4, None, None);
    assert_eq!(resumed.resumed(), 1);
    assert_eq!(resumed.computed() + 1, whole.computed());
    let (resumed_text, whole_text) = (
        resumed.into_text().expect("the resumed run's output"),
        whole.into_text().expect("the whole run's output"),
    );
    assert_eq!(masked(&resumed_text), masked(&whole_text));
    // The resumed point keeps its first run's times.
    let output: Value = serde_json::from_str(&resumed_text).expect("JSON");
    let index = &first["index"];
    let row = output["points"]
        .as_array()
        .expect("points")
        .iter()
        .find(|p| p["index"] == *index)
        .expect("the first point's row");
    let pilot_wall = first["pilot"]["wallS"]
        .as_f64()
        .expect("its pilot's wall time");
    let traced_wall = first["traced"]["wallS"].as_f64().unwrap_or(0.0);
    let total = row["wallS"].as_f64().expect("its row's wall time");
    assert!(
        (total - (pilot_wall + traced_wall)).abs() <= 1e-6 * total.max(1e-9),
        "{total}"
    );
}

#[test]
fn atmosphere_sweep_pilots_alone_the_same_at_one_and_four_threads() {
    let points = || SweepMode::PilotOnly(vec![377, 315, 931]);
    let one = smoke_in(points(), 1, None, None)
        .into_text()
        .expect("a pilot's output");
    let four = smoke_in(points(), 4, None, None)
        .into_text()
        .expect("a pilot's output");
    assert_eq!(masked(&one), masked(&four));
    let output = masked(&one);
    assert_eq!(output["kind"], "pilot");
    let rows = output["points"].as_array().expect("the pilots");
    let indices: Vec<_> = rows.iter().map(|r| r["index"].as_u64()).collect();
    assert_eq!(
        indices,
        [Some(377), Some(315), Some(931)],
        "in the order asked"
    );
    for row in rows {
        assert_eq!(row["samples"], 1024, "the smoke's pilot count");
        for field in [
            "radiance",
            "standardError",
            "geometryVertices",
            "plannedSamples",
        ] {
            assert_eq!(row[field].as_array().map(Vec::len), Some(4), "{field}");
        }
        let planned = row["plannedSamples"].as_array().expect("the counts");
        assert!(
            planned
                .iter()
                .all(|n| n.as_u64().is_some_and(|n| n >= 1024)),
            "{planned:?}"
        );
        assert!(row["aggregate"]["vertices"].as_u64().is_some_and(|v| v > 0));
    }
    // Point 377 is the g 0.75 column's, which runs VROOM; 315 the Rayleigh column's; 931 the g
    // 0.75 column's floor over the scan's brightest ground.
    assert_eq!(
        (&rows[0]["vroom"], &rows[1]["vroom"], &rows[2]["vroom"]),
        (&Value::Bool(true), &Value::Bool(false), &Value::Bool(true))
    );
    assert_eq!(
        (&rows[0]["groundAlbedo"], &rows[2]["groundAlbedo"]),
        (&json!(0.3), &json!(0.8))
    );
    assert_eq!(rows[2]["tau"], json!(0.03125), "the floor's τ");
}
