//! The replay's results file, in the descent spike's schema (`hyperion.descent-spike.results`
//! version 4, `apps/hyperion/src/main/results.ts`), so that a replay and a browser run read the
//! same way (R05 Design notes 18, 21 and 22).
//!
//! A native replay has no trace, no `requestAnimationFrame`, no GPU process and no memory
//! readings: those figures are null with the reason. With no trace there are no trace windows, so
//! no frame is left out at their boundaries (`frames.excludedFrames` is 0) and the engine's
//! sampled time (`mainThread.engine`) is null (decision-r05-trace-windows.md). A replay's frame
//! intervals are the presentation intervals of a presented replay, or, offscreen, the intervals
//! between the GPU's completions of successive frames (`frames.gpuCompletion`, a replay-only
//! field), which say what the GPU sustains with nothing presented. Percentiles are by nearest rank
//! and the criterion's rows are Design note 21's, as the client's writer reads them.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

/// The schema name the client's results files carry.
pub const RESULTS_SCHEMA: &str = "hyperion.descent-spike.results";

/// The schema's version, the client's `RESULTS_VERSION`.
///
/// Version 2 stores the memory series as columns of whole KiB (decision-r05-results-size.md),
/// version 3 takes the client's trace in windows (decision-r05-trace-windows.md), and version 4
/// (decision-r05-trace-windows-2.md) adds the trace's format, counts `mainThread.split`'s our code
/// by the per-frame `spike.frame` spans, lists only the GPU-process slices of the recorded
/// categories, and fails a window whose frame spans disagree with the renderer's frames. A replay
/// has no trace, so it writes `run.trace`, `mainThread.split` and `gpu.gpuProcess` as null and
/// meets none of those checks.
pub const RESULTS_VERSION: u64 = 4;

/// Why a native replay has no memory figure.
const NO_MEMORY: &str = "the native replay does not measure memory";

/// Why a native replay has no figure read from a trace.
const NO_TRACE: &str = "a native replay has no trace";

/// The memory series' reading columns (the client's `MemorySeries`), each null all run here.
const MEMORY_COLUMNS: [&str; 8] = [
    "appKiB",
    "gpuProcessKiB",
    "tracingKiB",
    "rendererPrivateKiB",
    "drmResidentKiB",
    "drmTotalKiB",
    "nvidiaDeviceKiB",
    "nvidiaGpuProcessKiB",
];

/// A native replay's memory series: no sample, and every reading null with the reason.
#[must_use]
fn memory_series() -> Value {
    let mut series = serde_json::Map::new();
    series.insert("tMs".to_owned(), json!([]));
    for column in MEMORY_COLUMNS {
        series.insert(column.to_owned(), missing(NO_MEMORY));
    }
    Value::Object(series)
}

/// A figure, or `null` with the reason, as the schema writes it.
#[must_use]
#[expect(
    clippy::needless_pass_by_value,
    reason = "every caller builds the value for this figure alone"
)]
pub fn measured(value: Value) -> Value {
    json!({ "value": value, "reason": null })
}

/// A missing figure and why.
#[must_use]
pub fn missing(reason: &str) -> Value {
    json!({ "value": null, "reason": reason })
}

/// Frame intervals summarised as Design note 21 reads them.
#[derive(Debug, Clone, PartialEq)]
pub struct FrameStats {
    /// Intervals.
    pub(crate) count: usize,
    /// The 50th, 95th and 99th percentiles by nearest rank, and the largest, ms.
    pub(crate) p50_ms: f64,
    /// See `p50_ms`.
    pub(crate) p95_ms: f64,
    /// See `p50_ms`.
    pub(crate) p99_ms: f64,
    /// See `p50_ms`.
    pub(crate) max_ms: f64,
    /// Intervals above 1.5 T, `None` without T.
    pub(crate) missed: Option<usize>,
    /// Intervals above 3 T, `None` without T.
    pub(crate) hitches: Option<usize>,
}

/// The `p`th percentile of ascending `sorted` by nearest rank, or `None` for no values.
#[must_use]
pub fn nearest_rank(sorted: &[f64], p: f64) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "a rank within a list's length, which is far below 2^52"
    )]
    let rank = (p * sorted.len() as f64).ceil() as usize;
    sorted.get(rank.clamp(1, sorted.len()) - 1).copied()
}

/// Summarises `intervals_ms` against the period T, or `None` for no intervals.
#[must_use]
pub fn frame_stats(intervals_ms: &[f64], period_ms: Option<f64>) -> Option<FrameStats> {
    let mut sorted = intervals_ms.to_vec();
    sorted.sort_by(f64::total_cmp);
    Some(FrameStats {
        count: sorted.len(),
        p50_ms: nearest_rank(&sorted, 0.5)?,
        p95_ms: nearest_rank(&sorted, 0.95)?,
        p99_ms: nearest_rank(&sorted, 0.99)?,
        max_ms: *sorted.last()?,
        missed: period_ms.map(|t| sorted.iter().filter(|ms| **ms > 1.5 * t).count()),
        hitches: period_ms.map(|t| sorted.iter().filter(|ms| **ms > 3.0 * t).count()),
    })
}

fn stats_json(stats: &FrameStats) -> Value {
    #[expect(
        clippy::cast_precision_loss,
        reason = "frame counts are far below 2^52"
    )]
    let fraction = stats
        .missed
        .map(|missed| missed as f64 / stats.count as f64);
    json!({
        "count": stats.count,
        "p50Ms": stats.p50_ms,
        "p95Ms": stats.p95_ms,
        "p99Ms": stats.p99_ms,
        "maxMs": stats.max_ms,
        "missed": stats.missed,
        "missedFraction": fraction,
        "hitches": stats.hitches,
    })
}

/// The quality setting a run is judged at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Setting {
    /// 1080p, T the vsync period.
    High,
    /// 720p30, T twice the vsync period.
    Low,
}

impl Setting {
    fn name(self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Low => "low",
        }
    }
}

/// Which of Design note 21's GPU rows a pass counts towards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassRow {
    /// The terrain row.
    Terrain,
    /// The atmosphere row.
    Atmosphere,
    /// Neither.
    Other,
}

/// What a replay measured and how it ran.
#[derive(Debug, Clone)]
pub struct ReplayFigures {
    /// When the replay started.
    pub(crate) started_at: SystemTime,
    /// The capture's directory.
    pub(crate) capture: PathBuf,
    /// The setting the capture was taken at.
    pub(crate) setting: Setting,
    /// The capture's seed, as it recorded it, or `"unknown"`.
    pub(crate) seed: String,
    /// Whether frames were presented in a window (FIFO) or replayed offscreen.
    pub(crate) presented: bool,
    /// The display's refresh rate, Hz, for a presented replay.
    pub(crate) display_hz: Option<f64>,
    /// The adapter replayed on.
    pub(crate) adapter: wgpu::AdapterInfo,
    /// Whether passes were timed (`TIMESTAMP_QUERY`).
    pub(crate) timed: bool,
    /// The main canvas's size, pixels.
    pub(crate) canvas: (u32, u32),
    /// Intervals between presentations (presented) or GPU completions (offscreen), ms.
    pub(crate) intervals_ms: Vec<f64>,
    /// Each frame's passes, label and GPU time, ms.
    pub(crate) passes: Vec<Vec<(String, f64)>>,
    /// The row of each pass label, from the capture.
    pub(crate) rows: BTreeMap<String, PassRow>,
    /// Passes beyond the timer's capacity.
    pub(crate) untimed_passes: usize,
    /// Bytes written by the span's uploads.
    pub(crate) upload_bytes: u64,
    /// Validation errors wgpu reported during the replay.
    pub(crate) errors: Vec<String>,
    /// What the replay could not do as captured: the capture's own problems, features the
    /// adapter lacks, a canvas format the window cannot present.
    pub(crate) findings: Vec<String>,
}

impl ReplayFigures {
    /// Whether frames were presented in a window.
    #[must_use]
    pub fn presented(&self) -> bool {
        self.presented
    }

    /// wgpu's validation errors during the replay.
    #[must_use]
    pub fn errors(&self) -> &[String] {
        &self.errors
    }

    /// What the replay could not do as captured.
    #[must_use]
    pub fn findings(&self) -> &[String] {
        &self.findings
    }
}

/// One row of the criterion.
fn row(
    id: &str,
    criterion: &str,
    limit: Option<f64>,
    unit: &str,
    value: Result<f64, String>,
    note: Option<&str>,
) -> Value {
    let (value, reason) = match value {
        Ok(value) => (
            Some(value),
            limit.is_none().then(|| "no period T".to_owned()),
        ),
        Err(reason) => (None, Some(reason)),
    };
    let verdict = match (value, limit) {
        (Some(value), Some(limit)) if value <= limit => "pass",
        (Some(_), Some(_)) => "fail",
        _ => "not-measured",
    };
    json!({
        "id": id,
        "criterion": criterion,
        "limit": limit,
        "unit": unit,
        "value": value,
        "tolerance": 0,
        "verdict": verdict,
        "note": reason.or(note.map(str::to_owned)),
    })
}

fn p95(values: &mut [f64]) -> Option<f64> {
    values.sort_by(f64::total_cmp);
    nearest_rank(values, 0.95)
}

/// The machine's facts the schema records, read from Linux's `/proc` and `/sys` where present.
fn machine(adapter: &wgpu::AdapterInfo) -> Value {
    let read = |path: &str| fs::read_to_string(path).ok();
    let name: String = read("/proc/sys/kernel/hostname")
        .unwrap_or_default()
        .trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    let cpu = read("/proc/cpuinfo")
        .and_then(|text| {
            text.lines()
                .find(|line| line.starts_with("model name"))
                .and_then(|line| line.split(':').nth(1))
                .map(|model| model.trim().to_owned())
        })
        .unwrap_or_else(|| "unknown".to_owned());
    let memory = read("/proc/meminfo")
        .and_then(|text| {
            text.lines()
                .find(|line| line.starts_with("MemTotal:"))
                .and_then(|line| line.split_whitespace().nth(1))
                .and_then(|kib| kib.parse::<u64>().ok())
        })
        .map_or(0, |kib| kib * 1024);
    let load: Vec<f64> = read("/proc/loadavg")
        .map(|text| {
            text.split_whitespace()
                .take(3)
                .filter_map(|v| v.parse().ok())
                .collect()
        })
        .unwrap_or_default();
    let governor = read("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor").map_or_else(
        || missing("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor could not be read"),
        |text| measured(json!(text.trim())),
    );
    let cores = std::thread::available_parallelism().map_or(0, std::num::NonZero::get);
    json!({
        "name": if name.is_empty() { "machine".to_owned() } else { name },
        "cpu": cpu,
        "logicalCores": cores,
        "memoryBytes": memory,
        "governor": governor,
        "loadAverage": [load.first().copied().unwrap_or(0.0), load.get(1).copied().unwrap_or(0.0), load.get(2).copied().unwrap_or(0.0)],
        "gpu": measured(json!({
            "vendorId": adapter.vendor,
            "deviceId": adapter.device,
            "driverVersion": format!("{} {}", adapter.driver, adapter.driver_info),
            "description": adapter.name,
        })),
    })
}

/// The date and time of `time` as ISO 8601 in UTC, to the second.
fn iso8601(time: SystemTime) -> String {
    let seconds = time.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let days = i64::try_from(seconds / 86_400).unwrap_or(0);
    let rest = seconds % 86_400;
    // Howard Hinnant's civil-from-days algorithm.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.000Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

/// The results file of a replay, as JSON in the client's schema.
#[must_use]
#[expect(
    clippy::too_many_lines,
    reason = "the schema's parts, written in its order"
)]
pub fn results_json(figures: &ReplayFigures) -> Value {
    let vsync_ms = figures
        .display_hz
        .filter(|hz| *hz > 0.0)
        .map(|hz| 1000.0 / hz);
    let period_ms = if figures.presented {
        vsync_ms.map(|t| {
            if figures.setting == Setting::Low {
                2.0 * t
            } else {
                t
            }
        })
    } else {
        None
    };
    let period_reason = if figures.presented {
        "the display reports no refresh rate"
    } else {
        "an offscreen replay presents nothing"
    };
    let stats = frame_stats(&figures.intervals_ms, period_ms);
    let no_interval = if figures.presented || figures.timed {
        "no frame interval"
    } else {
        "an offscreen replay times frames by timestamp-query, which the adapter lacks"
    };
    let stats_figure = stats
        .as_ref()
        .map_or_else(|| missing(no_interval), |s| measured(stats_json(s)));
    let (presentation, completion) = if figures.presented {
        (
            stats_figure.clone(),
            missing("a presented replay times presentations"),
        )
    } else {
        (
            missing("an offscreen replay presents nothing"),
            stats_figure.clone(),
        )
    };

    let mut by_label: BTreeMap<&str, Vec<f64>> = BTreeMap::new();
    let mut sums = Vec::new();
    let mut terrain = Vec::new();
    let mut atmosphere = Vec::new();
    for frame in &figures.passes {
        // A row's sum counts only frames with a pass of that row, as the client's writer does.
        let (mut sum, mut t, mut a) = (0.0, None::<f64>, None::<f64>);
        for (label, ms) in frame {
            by_label.entry(label.as_str()).or_default().push(*ms);
            sum += ms;
            match figures.rows.get(label).copied().unwrap_or(PassRow::Other) {
                PassRow::Terrain => t = Some(t.unwrap_or(0.0) + ms),
                PassRow::Atmosphere => a = Some(a.unwrap_or(0.0) + ms),
                PassRow::Other => {}
            }
        }
        if !frame.is_empty() {
            sums.push(sum);
        }
        terrain.extend(t);
        atmosphere.extend(a);
    }
    let timer_reason = "the adapter has no timestamp-query";
    let passes: Vec<Value> = by_label
        .iter_mut()
        .map(|(label, times)| {
            times.sort_by(f64::total_cmp);
            json!({
                "label": label,
                "row": match figures.rows.get(*label).copied().unwrap_or(PassRow::Other) {
                    PassRow::Terrain => "terrain",
                    PassRow::Atmosphere => "atmosphere",
                    PassRow::Other => "other",
                },
                "frames": times.len(),
                "p50Ms": nearest_rank(times, 0.5),
                "p95Ms": nearest_rank(times, 0.95),
                "p99Ms": nearest_rank(times, 0.99),
            })
        })
        .collect();
    let gpu_value = |values: &mut Vec<f64>, what: &str| -> Result<f64, String> {
        if !figures.timed {
            return Err(timer_reason.to_owned());
        }
        p95(values).ok_or_else(|| format!("no timed {what}"))
    };
    let sum_p95 = gpu_value(&mut sums, "pass");
    let (terrain_limit, atmosphere_limit, p95_limit, memory_limit) = match figures.setting {
        Setting::High => (5.0, 1.0, period_ms.map(|t| t + 1.0), 3e9),
        Setting::Low => (14.0, 4.0, Some(35.0), 1e9),
    };
    let frame_value = |pick: &dyn Fn(&FrameStats) -> Option<f64>| -> Result<f64, String> {
        let Some(stats) = &stats else {
            return Err("no frame interval".to_owned());
        };
        pick(stats).ok_or_else(|| period_reason.to_owned())
    };
    #[expect(
        clippy::cast_precision_loss,
        reason = "frame counts are far below 2^52"
    )]
    let whole = vec![
        row(
            "p50",
            "50th percentile ≤ T + 0.5 ms",
            period_ms.map(|t| t + 0.5),
            "ms",
            frame_value(&|s| Some(s.p50_ms)),
            None,
        ),
        row(
            "p95",
            if figures.setting == Setting::High {
                "95th percentile ≤ T + 1 ms"
            } else {
                "95th percentile ≤ 35 ms"
            },
            if period_ms.is_some() { p95_limit } else { None },
            "ms",
            frame_value(&|s| Some(s.p95_ms)),
            None,
        ),
        row(
            "p99",
            "99th percentile ≤ 2T",
            period_ms.map(|t| 2.0 * t),
            "ms",
            frame_value(&|s| Some(s.p99_ms)),
            None,
        ),
        row(
            "missed",
            "≤ 1% of intervals above 1.5 T",
            Some(0.01),
            "fraction",
            frame_value(&|s| s.missed.map(|m| m as f64 / s.count as f64)),
            None,
        ),
        row(
            "hitches",
            "none above 3 T",
            Some(0.0),
            "count",
            frame_value(&|s| s.hitches.map(|h| h as f64)),
            None,
        ),
        row(
            "headroom-main",
            "main thread ≤ 0.8 T at the 95th percentile",
            period_ms.map(|t| 0.8 * t),
            "ms",
            Err("a native replay has no main-thread figure".to_owned()),
            None,
        ),
        row(
            "headroom-gpu",
            "GPU pass sum ≤ 0.8 T at the 95th percentile",
            period_ms.map(|t| 0.8 * t),
            "ms",
            sum_p95.clone(),
            None,
        ),
        row(
            "terrain",
            &format!("terrain GPU time ≤ {terrain_limit} ms at the 95th percentile"),
            Some(terrain_limit),
            "ms",
            gpu_value(&mut terrain, "terrain pass"),
            None,
        ),
        row(
            "atmosphere",
            &format!("atmosphere GPU time ≤ {atmosphere_limit} ms at the 95th percentile"),
            Some(atmosphere_limit),
            "ms",
            gpu_value(&mut atmosphere, "atmosphere pass"),
            None,
        ),
        row(
            "memory",
            if figures.setting == Setting::High {
                "GPU resident ≤ 3 GB"
            } else {
                "GPU resident ≤ 1 GB"
            },
            Some(memory_limit),
            "bytes",
            Err(NO_MEMORY.to_owned()),
            None,
        ),
    ];
    let overall = {
        let verdicts: Vec<&str> = whole
            .iter()
            .filter_map(|r| r.get("verdict").and_then(Value::as_str))
            .collect();
        if verdicts.contains(&"fail") {
            "fail"
        } else if verdicts.contains(&"not-measured") {
            "not-measured"
        } else {
            "pass"
        }
    };
    let load = machine(&figures.adapter);
    let load_1 = load
        .get("loadAverage")
        .and_then(|l| l.get(0))
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    json!({
        "schema": RESULTS_SCHEMA,
        "version": RESULTS_VERSION,
        "run": {
            "startedAt": iso8601(figures.started_at),
            "machine": load,
            "versions": {
                "app": format!("gpu-replay {}", env!("CARGO_PKG_VERSION")),
                "electron": "none (native replay)",
                "chromium": "none (native replay)",
                "node": "none (native replay)",
                "v8": "none (native replay)",
            },
            "platform": std::env::consts::OS,
            "launchMode": "native-replay",
            "setting": figures.setting.name(),
            "seed": figures.seed,
            "options": {
                "capture": figures.capture.display().to_string(),
                "presented": figures.presented,
                "backend": format!("{:?}", figures.adapter.backend).to_lowercase(),
                "validationErrors": figures.errors.len(),
            },
            "switches": [],
            "timer": if figures.timed { "full" } else { "absent" },
            "shown": figures.presented,
            "periodMs": period_ms.map_or_else(|| missing(period_reason), |t| measured(json!(t))),
            "warmupS": 0,
            "canvas": { "widthPx": figures.canvas.0, "heightPx": figures.canvas.1 },
            "quiet": {
                "provisional": load_1 >= 1.0,
                "note": (load_1 >= 1.0).then(|| format!("load average {load_1:.2} at the start (Design note 27 asks under 1): provisional")),
            },
            "trace": missing(NO_TRACE),
        },
        "levels": [],
        "frames": {
            "source": if figures.presented { "presentation" } else { "gpu-completion" },
            "presentation": presentation,
            "raf": missing("a native replay has no requestAnimationFrame"),
            "gpuCompletion": completion,
            "segments": [],
            "dropped": missing(NO_TRACE),
            "excludedFrames": 0,
        },
        "gpu": {
            "timer": if figures.timed { "full" } else { "absent" },
            "tolerancePerPassMs": 0,
            "untimedPasses": figures.untimed_passes,
            "passes": if figures.timed { measured(json!(passes)) } else { missing(timer_reason) },
            "sumP95Ms": sum_p95.map_or_else(|reason| missing(&reason), |v| measured(json!(v))),
            "gpuProcess": missing("a native replay has no GPU process"),
        },
        "mainThread": {
            "ourCodeP95Ms": missing("a native replay has no main-thread figure"),
            "split": missing(NO_TRACE),
            "engine": missing(NO_TRACE),
            "gc": missing(NO_TRACE),
        },
        "streaming": [],
        "uploads": { "bytes": figures.upload_bytes },
        "pipelines": { "late": [] },
        "memory": {
            "series": memory_series(),
            "gpuHeadline": missing(NO_MEMORY),
            "peakAppBytes": missing(NO_MEMORY),
            "peakTracingBytes": missing(NO_TRACE),
            "peakRendererPrivateBytes": missing("a native replay has no renderer"),
            "peakDrmResidentBytes": missing(NO_MEMORY),
            "peakNvidiaDeviceLessBaselineBytes": missing(NO_MEMORY),
            "peakNvidiaGpuProcessBytes": missing(NO_MEMORY),
            "adapterPeakBytes": 0,
        },
        "criteria": { "whole": whole, "segments": [], "overall": overall },
        "replay": {
            "validationErrors": figures.errors,
            "findings": figures.findings,
        },
    })
}

/// Writes a replay's results into `dir` as `<date>-<machine>-<setting>-replay.json`, numbered
/// `-2`, `-3` and so on beside an earlier one, and returns the path.
///
/// # Errors
///
/// When the directory cannot be made or the file cannot be written.
pub fn write_results(dir: &Path, figures: &ReplayFigures) -> std::io::Result<PathBuf> {
    let results = results_json(figures);
    fs::create_dir_all(dir)?;
    let date = iso8601(figures.started_at);
    let machine = results
        .pointer("/run/machine/name")
        .and_then(Value::as_str)
        .unwrap_or("machine")
        .to_owned();
    let stem = format!(
        "{}-{machine}-{}-replay",
        &date[..10],
        figures.setting.name()
    );
    let mut name = format!("{stem}.json");
    let mut n = 2;
    while dir.join(&name).exists() {
        name.clear();
        let _ = write!(name, "{stem}-{n}.json");
        n += 1;
    }
    let path = dir.join(name);
    let mut text = serde_json::to_string_pretty(&results).map_err(std::io::Error::other)?;
    text.push('\n');
    fs::write(&path, text)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_rank_matches_the_clients() {
        let values = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        assert_eq!(nearest_rank(&values, 0.5), Some(5.0));
        assert_eq!(nearest_rank(&values, 0.95), Some(10.0));
        assert_eq!(nearest_rank(&values, 0.1), Some(1.0));
        assert_eq!(nearest_rank(&[], 0.5), None);
    }

    #[test]
    fn frame_stats_count_missed_frames_and_hitches() {
        let stats = frame_stats(
            &[16.7, 16.7, 16.7, 26.0, 30.0, 51.0, 16.6, 16.7],
            Some(16.68),
        )
        .expect("stats");
        assert_eq!((stats.missed, stats.hitches), (Some(3), Some(1)));
        assert!((stats.p50_ms - 16.7).abs() < 1e-12);
    }

    fn figures(rows: BTreeMap<String, PassRow>) -> ReplayFigures {
        ReplayFigures {
            started_at: UNIX_EPOCH,
            capture: PathBuf::from("capture"),
            setting: Setting::High,
            seed: "7".to_owned(),
            presented: false,
            display_hz: None,
            adapter: wgpu::AdapterInfo {
                name: "test".to_owned(),
                vendor: 0,
                device: 0,
                device_type: wgpu::DeviceType::Other,
                device_pci_bus_id: String::new(),
                driver: String::new(),
                driver_info: String::new(),
                backend: wgpu::Backend::Noop,
                subgroup_min_size: 0,
                subgroup_max_size: 0,
                transient_saves_memory: None,
                limit_bucket: None,
            },
            timed: true,
            canvas: (1920, 1080),
            intervals_ms: vec![16.7, 16.7],
            passes: vec![vec![("terrain".to_owned(), 4.0), ("tone".to_owned(), 0.5)]; 4],
            rows,
            untimed_passes: 0,
            upload_bytes: 0,
            errors: Vec::new(),
            findings: Vec::new(),
        }
    }

    fn row<'a>(results: &'a Value, id: &str) -> &'a Value {
        results["criteria"]["whole"]
            .as_array()
            .and_then(|rows| rows.iter().find(|row| row["id"] == id))
            .expect("the row")
    }

    #[test]
    fn a_row_with_no_pass_of_its_own_is_not_measured() {
        let results = results_json(&figures(BTreeMap::new()));
        assert_eq!(row(&results, "terrain")["verdict"], "not-measured");
        assert_eq!(row(&results, "terrain")["note"], "no timed terrain pass");
    }

    #[test]
    fn a_rows_passes_are_judged_against_its_limit() {
        let rows = BTreeMap::from([("terrain".to_owned(), PassRow::Terrain)]);
        let results = results_json(&figures(rows));
        assert_eq!(row(&results, "terrain")["value"], 4.0);
        assert_eq!(row(&results, "terrain")["verdict"], "pass");
        assert_eq!(row(&results, "headroom-gpu")["verdict"], "not-measured");
        assert_eq!(results["gpu"]["sumP95Ms"]["value"], 4.5);
    }

    #[test]
    fn the_memory_series_has_no_sample_and_every_reading_null_with_its_reason() {
        let results = results_json(&figures(BTreeMap::new()));
        // The client's version and column names, written out: its `validateResults` reads them.
        assert_eq!(results["version"], 4);
        let none = json!({ "value": null, "reason": "the native replay does not measure memory" });
        assert_eq!(
            results["memory"]["series"],
            json!({
                "tMs": [],
                "appKiB": none,
                "gpuProcessKiB": none,
                "tracingKiB": none,
                "rendererPrivateKiB": none,
                "drmResidentKiB": none,
                "drmTotalKiB": none,
                "nvidiaDeviceKiB": none,
                "nvidiaGpuProcessKiB": none,
            })
        );
        assert_eq!(results["memory"].get("samples"), None);
    }

    #[test]
    fn the_file_is_version_4_with_no_trace_window_and_no_engine_figure() {
        let results = results_json(&figures(BTreeMap::new()));
        let no_trace = json!({ "value": null, "reason": "a native replay has no trace" });
        assert_eq!(results["version"], 4);
        assert_eq!(results["run"]["trace"], no_trace);
        assert_eq!(results["mainThread"]["engine"], no_trace);
        // Version 4's split and GPU-process slices are the trace's, so a replay writes neither.
        assert_eq!(results["mainThread"]["split"], no_trace);
        assert_eq!(
            results["gpu"]["gpuProcess"],
            json!({ "value": null, "reason": "a native replay has no GPU process" })
        );
        assert_eq!(results["frames"]["excludedFrames"], 0);
    }

    #[test]
    fn dates_are_iso_8601() {
        let time = UNIX_EPOCH + std::time::Duration::from_hours(497_490);
        assert_eq!(iso8601(time), "2026-10-02T18:00:00.000Z");
    }
}
