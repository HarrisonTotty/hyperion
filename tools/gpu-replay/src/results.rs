//! The replay's results file, in the descent spike's schema (`hyperion.descent-spike.results`
//! version 5, `apps/hyperion/src/main/results.ts`), so that a replay and a browser run read the
//! same way (R05 Design notes 18, 21 and 22).
//!
//! A native replay has no trace, no `requestAnimationFrame`, no GPU process and no memory
//! readings: those figures are null with the reason. The GPU's clocks are read before the first
//! frame, once a second and after the last (`clocks.rs`), on the memory series' times, whose
//! memory readings are all null; a GPU row measured at a median clock below 90% of the maximum
//! says so, as the client's do (decision-r05-trace-windows-2.md, addendum B). With no trace there
//! are no trace windows, so
//! no frame is left out at their boundaries (`frames.excludedFrames` is 0) and the engine's
//! sampled time (`mainThread.engine`) is null (decision-r05-trace-windows.md). A replay's frame
//! intervals are the presentation intervals of a presented replay, or, offscreen, the intervals
//! between the GPU's completions of successive frames (`frames.gpuCompletion`, a replay-only
//! field), which say what the GPU sustains with nothing presented. Percentiles are by nearest rank
//! and the criterion's rows are Design note 21's, as the client's writer reads them. A frame whose
//! pass times were never read back is left out of the GPU rows and counted, and bounds their
//! verdicts as the client's incomplete frames do (decision-r05-trace-windows-2.md, addendum B).

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use crate::clocks::{ClockReadings, ClockSample, Reading};

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
/// meets none of those checks. Version 5 (the same decision's addendum B) counts the frames whose
/// pass times are incomplete (`gpu.incompleteFrames`), bounds the GPU rows' verdicts by them, and
/// adds `gpu.clocks`, the GPU's clocks on the memory series' times (R05.T14.k).
pub const RESULTS_VERSION: u64 = 5;

/// Why a replay with no clock sample has no clocks.
const NO_CLOCK_SAMPLE: &str = "no clock sample";

/// The fraction of the maximum graphics clock below which a GPU row says what it was measured at,
/// the client's `CLOCK_NOTE_FRACTION`.
const CLOCK_NOTE_FRACTION: f64 = 0.9;

/// The rows read from per-frame GPU times, which a clock note may sit on.
const GPU_ROW_IDS: [&str; 3] = ["headroom-gpu", "terrain", "atmosphere"];

/// The percentile every GPU row reads (Design note 21).
const GPU_ROW_PERCENTILE: f64 = 0.95;

/// Why a native replay has no memory figure.
const NO_MEMORY: &str = "the native replay does not measure memory";

/// Why a native replay has no figure read from a trace.
const NO_TRACE: &str = "a native replay has no trace";

/// Why a replay on an adapter without `TIMESTAMP_QUERY` has no pass time.
const TIMER_REASON: &str = "the adapter has no timestamp-query";

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

/// A native replay's memory series: the clock samples' times, and every memory reading null with
/// the reason.
#[must_use]
fn memory_series(clocks: &[ClockSample]) -> Value {
    let mut series = serde_json::Map::new();
    let t_ms: Vec<u64> = clocks.iter().map(|sample| sample.t_ms).collect();
    series.insert("tMs".to_owned(), json!(t_ms));
    for column in MEMORY_COLUMNS {
        series.insert(column.to_owned(), missing(NO_MEMORY));
    }
    Value::Object(series)
}

/// The distinct reasons of `reasons`, in their first order, joined.
#[must_use]
fn joined<'a>(reasons: impl IntoIterator<Item = &'a str>) -> String {
    let mut distinct: Vec<&str> = Vec::new();
    for reason in reasons {
        if !distinct.contains(&reason) {
            distinct.push(reason);
        }
    }
    distinct.join("; ")
}

/// A column of readings in the client's `MemoryColumn` form: each reading, -1 within its gaps,
/// and the gaps, consecutive samples of one reason being one; null with every reason when no
/// sample has a reading. `readings` is not empty.
#[must_use]
fn reading_column(readings: impl IntoIterator<Item = Reading>) -> Value {
    let mut samples: Vec<i64> = Vec::new();
    let mut gaps: Vec<(usize, usize, String)> = Vec::new();
    for (i, reading) in readings.into_iter().enumerate() {
        match reading {
            Ok(value) => samples.push(i64::from(value)),
            Err(reason) => {
                samples.push(-1);
                match gaps.last_mut() {
                    Some((_, to, last)) if *to + 1 == i && *last == reason => *to = i,
                    _ => gaps.push((i, i, reason)),
                }
            }
        }
    }
    if samples.iter().all(|reading| *reading == -1) {
        return missing(&joined(gaps.iter().map(|(_, _, reason)| reason.as_str())));
    }
    let gaps: Vec<Value> = gaps
        .into_iter()
        .map(|(from, to, reason)| json!({ "from": from, "to": to, "reason": reason }))
        .collect();
    measured(json!({ "samples": samples, "gaps": gaps }))
}

/// The readings of the first sample that has any, and their source's maximum: the largest any of
/// its samples read.
#[must_use]
fn source_readings(clocks: &[ClockSample]) -> Option<(&ClockReadings, Option<u32>)> {
    let first = clocks
        .iter()
        .find_map(|sample| sample.readings.as_ref().ok())?;
    let max = clocks
        .iter()
        .filter_map(|sample| sample.readings.as_ref().ok())
        .filter(|readings| readings.source == first.source)
        .filter_map(|readings| readings.max_graphics_mhz.as_ref().ok().copied())
        .max();
    Some((first, max))
}

/// The replay's `gpu.clocks`, as the client's `gpuClocksOf` builds it from its samples: the first
/// reading's source, a column a reading on the samples' times, and the largest maximum read;
/// without a graphics clock at any sample, null with that reason.
#[must_use]
fn clocks_json(clocks: &[ClockSample]) -> Value {
    if clocks.is_empty() {
        return missing(NO_CLOCK_SAMPLE);
    }
    let Some((first, max)) = source_readings(clocks) else {
        return missing(&joined(clocks.iter().filter_map(|sample| {
            sample.readings.as_ref().err().map(String::as_str)
        })));
    };
    let source = first.source;
    let column = |pick: fn(&ClockReadings) -> &Reading| {
        reading_column(clocks.iter().map(|sample| match &sample.readings {
            Ok(readings) if readings.source == source => pick(readings).clone(),
            Ok(readings) => Err(format!(
                "read from {}, not the replay's {}",
                readings.source.name(),
                source.name()
            )),
            Err(reason) => Err(reason.clone()),
        }))
    };
    let graphics = column(|readings| &readings.graphics_mhz);
    if graphics["value"].is_null() {
        return graphics;
    }
    let max_graphics = max.map_or_else(
        || {
            missing(
                first
                    .max_graphics_mhz
                    .as_ref()
                    .err()
                    .map_or("no maximum", String::as_str),
            )
        },
        |mhz| measured(json!(mhz)),
    );
    measured(json!({
        "source": source.name(),
        "maxGraphicsMHz": max_graphics,
        "graphicsMHz": graphics,
        "memoryMHz": column(|readings| &readings.memory_mhz),
        "performanceState": column(|readings| &readings.performance_state),
    }))
}

/// The note a GPU row carries when the replay's median graphics clock was below
/// [`CLOCK_NOTE_FRACTION`] of the maximum, as the client's `clockNote` words it; a replay has no
/// warm-up, so every sample counts.
#[must_use]
fn clock_note(clocks: &[ClockSample]) -> Option<String> {
    let (first, max) = source_readings(clocks)?;
    let max = max?;
    let mut graphics: Vec<u32> = clocks
        .iter()
        .filter_map(|sample| sample.readings.as_ref().ok())
        .filter(|readings| readings.source == first.source)
        .filter_map(|readings| readings.graphics_mhz.as_ref().ok().copied())
        .collect();
    graphics.sort_unstable();
    let median = *graphics.get(rank_index(graphics.len(), 0.5))?;
    (f64::from(median) < CLOCK_NOTE_FRACTION * f64::from(max)).then(|| {
        format!("measured at a median {median} of {max} MHz (the driver's choice at this load)")
    })
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

/// The index [`nearest_rank`] reads among `len` ascending values at percentile `p`, from 0; the
/// GPU rows' bounds read the same, as the client's `rankIndex` does. `len` is at least 1.
#[must_use]
fn rank_index(len: usize, p: f64) -> usize {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "a rank within a list's length, which is far below 2^52"
    )]
    let rank = (p * len as f64).ceil() as usize;
    rank.clamp(1, len.max(1)) - 1
}

/// The `p`th percentile of ascending `sorted` by nearest rank, or `None` for no values.
#[must_use]
pub fn nearest_rank(sorted: &[f64], p: f64) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    sorted.get(rank_index(sorted.len(), p)).copied()
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
    /// Frames whose resolved pass times were never read back, which `passes` leaves out.
    pub(crate) unread_frames: usize,
    /// Frames that timed no pass, which `passes` leaves out too.
    pub(crate) untimed_frames: usize,
    /// Each read-back frame's passes, label and GPU time, ms.
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
    /// The GPU's clocks, read before the first frame, once a second and after the last.
    pub(crate) clocks: Vec<ClockSample>,
}

impl ReplayFigures {
    /// Every frame replayed: those read back, those never read back and those that timed no pass,
    /// so that the unread are never more than the frames.
    #[must_use]
    pub fn frames(&self) -> usize {
        self.passes.len() + self.unread_frames + self.untimed_frames
    }

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
    nearest_rank(values, GPU_ROW_PERCENTILE)
}

/// Why a GPU row is not measured when the frames whose pass times were not read back could carry
/// its verdict either way.
#[must_use]
fn unread_reason(count: usize) -> String {
    let frames = if count == 1 { "frame's" } else { "frames'" };
    format!("{count} {frames} pass times could not be read back")
}

/// How a replay timed its passes, which its GPU rows read.
#[derive(Debug, Clone, Copy)]
struct PassTiming {
    /// Whether the adapter timed passes (`TIMESTAMP_QUERY`).
    timed: bool,
    /// Frames whose resolved pass times were never read back.
    unread: usize,
}

impl PassTiming {
    /// The 95th percentile of `values`, which it sorts, or why there is none.
    fn p95(self, values: &mut [f64], what: &str) -> Result<f64, String> {
        if !self.timed {
            return Err(TIMER_REASON.to_owned());
        }
        p95(values).ok_or_else(|| {
            if self.unread > 0 {
                unread_reason(self.unread)
            } else {
                format!("no timed {what}")
            }
        })
    }

    /// A GPU row: the 95th percentile of `values`, its frames read back, against `limit`, judged
    /// so that its verdict holds whatever times the unread frames had, as the client's
    /// `boundedGpuRow` does (decision-r05-trace-windows-2.md, addendum B, ruling 1).
    ///
    /// The row passes only if it still passes with every unread frame placed above its limit, and
    /// fails only if it still fails with every one placed below it; otherwise it is not measured,
    /// with their count as its reason. With no unread frame it is [`row`]'s.
    #[must_use]
    fn row(
        self,
        id: &str,
        criterion: &str,
        limit: Option<f64>,
        values: &mut [f64],
        what: &str,
    ) -> Value {
        let value = self.p95(values, what);
        let mut judged = row(id, criterion, limit, "ms", value.clone(), None);
        let (Some(limit), Ok(_), true) = (limit, &value, self.unread > 0) else {
            return judged;
        };
        // The percentile's index among the read values and the unread placed after them (above)
        // or before them (below); `values` is sorted now.
        let rank = rank_index(values.len() + self.unread, GPU_ROW_PERCENTILE);
        let above = values.get(rank).copied().unwrap_or(f64::INFINITY);
        let below = rank
            .checked_sub(self.unread)
            .and_then(|i| values.get(i))
            .copied()
            .unwrap_or(f64::NEG_INFINITY);
        let judge = |ms: f64| if ms <= limit { "pass" } else { "fail" };
        let (verdict, note) = if judge(above) == judge(below) {
            let frames = if self.unread == 1 { "frame" } else { "frames" };
            (
                judge(above),
                format!(
                    "{} {frames} with incomplete pass times left out; the verdict holds whatever their times",
                    self.unread
                ),
            )
        } else {
            ("not-measured", unread_reason(self.unread))
        };
        judged["verdict"] = json!(verdict);
        judged["note"] = json!(note);
        judged
    }
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
    let timing = PassTiming {
        timed: figures.timed,
        unread: figures.unread_frames,
    };
    let sum_p95 = timing.p95(&mut sums, "pass");
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
        timing.row(
            "headroom-gpu",
            "GPU pass sum ≤ 0.8 T at the 95th percentile",
            period_ms.map(|t| 0.8 * t),
            &mut sums,
            "pass",
        ),
        timing.row(
            "terrain",
            &format!("terrain GPU time ≤ {terrain_limit} ms at the 95th percentile"),
            Some(terrain_limit),
            &mut terrain,
            "terrain pass",
        ),
        timing.row(
            "atmosphere",
            &format!("atmosphere GPU time ≤ {atmosphere_limit} ms at the 95th percentile"),
            Some(atmosphere_limit),
            &mut atmosphere,
            "atmosphere pass",
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
    let mut whole = whole;
    if let Some(note) = clock_note(&figures.clocks) {
        for entry in &mut whole {
            let gpu_row = entry["id"]
                .as_str()
                .is_some_and(|id| GPU_ROW_IDS.contains(&id));
            if gpu_row && !entry["value"].is_null() {
                entry["note"] = json!(match entry["note"].as_str() {
                    Some(own) => format!("{own}; {note}"),
                    None => note.clone(),
                });
            }
        }
    }
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
            "passes": if figures.timed { measured(json!(passes)) } else { missing(TIMER_REASON) },
            "sumP95Ms": sum_p95.map_or_else(|reason| missing(&reason), |v| measured(json!(v))),
            // A replay resolves once a frame, so a frame's pass times are whole or none.
            "incompleteFrames": if figures.timed {
                measured(json!({
                    "dropped": figures.unread_frames,
                    "partial": 0,
                    "frames": figures.frames(),
                }))
            } else {
                missing(TIMER_REASON)
            },
            "gpuProcess": missing("a native replay has no GPU process"),
            "clocks": clocks_json(&figures.clocks),
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
            "series": memory_series(&figures.clocks),
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
            unread_frames: 0,
            untimed_frames: 0,
            passes: vec![vec![("terrain".to_owned(), 4.0), ("tone".to_owned(), 0.5)]; 4],
            rows,
            untimed_passes: 0,
            upload_bytes: 0,
            errors: Vec::new(),
            findings: Vec::new(),
            clocks: Vec::new(),
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

    /// Readings from `nvidia-smi` of a 2,115 MHz GPU at `graphics_mhz` in P2.
    fn nvidia_at(graphics_mhz: u32) -> ClockReadings {
        ClockReadings {
            source: crate::clocks::ClockSourceKind::NvidiaSmi,
            max_graphics_mhz: Ok(2115),
            graphics_mhz: Ok(graphics_mhz),
            memory_mhz: Ok(9501),
            performance_state: Ok(2),
        }
    }

    #[test]
    fn the_clocks_are_written_from_their_samples_on_the_memory_series_times() {
        let rows = BTreeMap::from([("terrain".to_owned(), PassRow::Terrain)]);
        let mut stubbed = figures(rows);
        stubbed.clocks = vec![
            ClockSample::new(12, Ok(nvidia_at(1110))),
            ClockSample::new(1013, Err("nvidia-smi failed: timed out".to_owned())),
            ClockSample::new(2014, Ok(nvidia_at(1320))),
            ClockSample::new(2400, Ok(nvidia_at(1965))),
        ];
        let results = results_json(&stubbed);
        assert_eq!(
            results["memory"]["series"]["tMs"],
            json!([12, 1013, 2014, 2400])
        );
        let gap = json!([{ "from": 1, "to": 1, "reason": "nvidia-smi failed: timed out" }]);
        assert_eq!(
            results["gpu"]["clocks"],
            json!({
                "value": {
                    "source": "nvidia-smi",
                    "maxGraphicsMHz": { "value": 2115, "reason": null },
                    "graphicsMHz": {
                        "value": { "samples": [1110, -1, 1320, 1965], "gaps": gap },
                        "reason": null,
                    },
                    "memoryMHz": {
                        "value": { "samples": [9501, -1, 9501, 9501], "gaps": gap },
                        "reason": null,
                    },
                    "performanceState": {
                        "value": { "samples": [2, -1, 2, 2], "gaps": gap },
                        "reason": null,
                    },
                },
                "reason": null,
            })
        );
        // The median of 1,110, 1,320 and 1,965 MHz is 62% of the maximum: the GPU rows with a
        // value say so, the others not.
        let note = "measured at a median 1320 of 2115 MHz (the driver's choice at this load)";
        assert_eq!(row(&results, "terrain")["note"], note);
        assert_eq!(
            row(&results, "atmosphere")["note"],
            "no timed atmosphere pass"
        );
        // A frame row with a value keeps its own note alone.
        assert_eq!(row(&results, "p50")["note"], "no period T");
    }

    #[test]
    fn a_median_clock_near_the_maximum_puts_no_note() {
        let rows = BTreeMap::from([("terrain".to_owned(), PassRow::Terrain)]);
        let mut stubbed = figures(rows);
        stubbed.clocks = vec![ClockSample::new(0, Ok(nvidia_at(1965))); 3];
        assert_eq!(row(&results_json(&stubbed), "terrain")["note"], Value::Null);
    }

    #[test]
    fn clocks_with_no_reading_are_null_with_the_reason() {
        let mut stubbed = figures(BTreeMap::new());
        let reason = "no unprivileged GPU clock reading on macos; powermetrics needs root";
        stubbed.clocks = vec![ClockSample::new(0, Err(reason.to_owned())); 2];
        assert_eq!(results_json(&stubbed)["gpu"]["clocks"], missing(reason));
        stubbed.clocks.clear();
        assert_eq!(
            results_json(&stubbed)["gpu"]["clocks"],
            missing("no clock sample")
        );
    }

    #[test]
    fn the_memory_series_has_no_sample_and_every_reading_null_with_its_reason() {
        let results = results_json(&figures(BTreeMap::new()));
        // The client's version and column names, written out: its `validateResults` reads them.
        assert_eq!(results["version"], 5);
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
    fn the_file_is_version_5_with_no_trace_window_and_no_engine_figure() {
        let results = results_json(&figures(BTreeMap::new()));
        let no_trace = json!({ "value": null, "reason": "a native replay has no trace" });
        assert_eq!(results["version"], 5);
        assert_eq!(results["run"]["trace"], no_trace);
        assert_eq!(results["mainThread"]["engine"], no_trace);
        // Version 4's split and GPU-process slices are the trace's, so a replay writes neither.
        assert_eq!(results["mainThread"]["split"], no_trace);
        assert_eq!(
            results["gpu"]["gpuProcess"],
            json!({ "value": null, "reason": "a native replay has no GPU process" })
        );
        assert_eq!(results["frames"]["excludedFrames"], 0);
        assert_eq!(
            results["gpu"]["incompleteFrames"],
            json!({ "value": { "dropped": 0, "partial": 0, "frames": 4 }, "reason": null })
        );
        assert_eq!(
            results["gpu"]["clocks"],
            json!({ "value": null, "reason": "no clock sample" })
        );
    }

    /// `figures` with `count` frames of a terrain pass taking `terrain_ms`, `unread` more of them
    /// never read back.
    fn unread_figures(count: usize, terrain_ms: f64, unread: usize) -> ReplayFigures {
        let mut figures = figures(BTreeMap::from([("terrain".to_owned(), PassRow::Terrain)]));
        figures.passes = vec![vec![("terrain".to_owned(), terrain_ms)]; count];
        figures.unread_frames = unread;
        figures
    }

    #[test]
    fn frames_never_read_back_are_counted_as_dropped() {
        let results = results_json(&unread_figures(4, 4.0, 1));
        assert_eq!(
            results["gpu"]["incompleteFrames"]["value"],
            json!({ "dropped": 1, "partial": 0, "frames": 5 })
        );
    }

    #[test]
    fn a_row_its_unread_frames_could_carry_over_its_limit_is_not_measured() {
        // Four frames at 4 ms of the high setting's 5, and a fifth unread: placed above the limit,
        // it is the 95th percentile.
        let results = results_json(&unread_figures(4, 4.0, 1));
        let terrain = row(&results, "terrain");
        let value = terrain["value"].as_f64().expect("a value");
        assert!((value - 4.0).abs() < 1e-12, "{value}");
        assert_eq!(terrain["verdict"], "not-measured");
        assert_eq!(
            terrain["note"],
            "1 frame's pass times could not be read back"
        );
    }

    #[test]
    fn a_row_passes_or_fails_whatever_its_unread_frames_took() {
        let passing = results_json(&unread_figures(100, 4.0, 1));
        assert_eq!(row(&passing, "terrain")["verdict"], "pass");
        assert_eq!(
            row(&passing, "terrain")["note"],
            "1 frame with incomplete pass times left out; the verdict holds whatever their times"
        );
        let failing = results_json(&unread_figures(100, 6.0, 3));
        assert_eq!(row(&failing, "terrain")["verdict"], "fail");
        assert_eq!(
            row(&failing, "terrain")["note"],
            "3 frames with incomplete pass times left out; the verdict holds whatever their times"
        );
    }

    #[test]
    fn a_row_with_only_unread_frames_states_their_count() {
        let results = results_json(&unread_figures(0, 4.0, 2));
        let terrain = row(&results, "terrain");
        assert_eq!(terrain["verdict"], "not-measured");
        assert_eq!(
            terrain["note"],
            "2 frames' pass times could not be read back"
        );
        assert_eq!(
            results["gpu"]["sumP95Ms"]["reason"],
            "2 frames' pass times could not be read back"
        );
        assert_eq!(results["gpu"]["incompleteFrames"]["value"]["frames"], 2);
    }

    #[test]
    fn frames_are_those_read_back_unread_and_untimed() {
        let mut figures = unread_figures(3, 4.0, 1);
        figures.untimed_frames = 2;
        assert_eq!(figures.frames(), 6);
    }

    #[test]
    fn a_replay_without_a_timer_states_why_it_counts_no_frame() {
        let mut untimed = unread_figures(4, 4.0, 0);
        untimed.timed = false;
        let results = results_json(&untimed);
        assert_eq!(
            results["gpu"]["incompleteFrames"],
            json!({ "value": null, "reason": "the adapter has no timestamp-query" })
        );
    }

    #[test]
    fn dates_are_iso_8601() {
        let time = UNIX_EPOCH + std::time::Duration::from_hours(497_490);
        assert_eq!(iso8601(time), "2026-10-02T18:00:00.000Z");
    }
}
