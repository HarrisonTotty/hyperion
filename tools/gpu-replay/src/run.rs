//! Driving a replay: the device, the frames, their timing, offscreen or presented (R05.T15.c).

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, SystemTime};

use serde_json::Value;

use crate::capture::{Capture, QUEUE_ID};
use crate::replay::{FramePassTimes, FrameTarget, ReplayCallError, Replayer, UnknownFormatError};
use crate::results::{PassRow, ReplayFigures, Setting};

/// Why a replay could not run.
#[derive(Debug, thiserror::Error)]
pub enum RunReplayError {
    /// No adapter.
    #[error("no GPU adapter")]
    NoAdapter(#[source] wgpu::RequestAdapterError),
    /// The device could not be made.
    #[error("the device could not be made")]
    NoDevice(#[source] wgpu::RequestDeviceError),
    /// A canvas's format is not one wgpu knows.
    #[error(transparent)]
    Format(#[from] UnknownFormatError),
    /// A call could not be replayed.
    #[error("the capture could not be replayed")]
    Call(#[from] ReplayCallError),
    /// Waiting for the GPU failed or timed out.
    #[error("waiting for the GPU failed")]
    Poll(#[source] wgpu::PollError),
    /// The capture names no setting and none was given.
    #[error("the capture records no setting; pass --setting")]
    NoSetting,
    /// The window could not be made or drawn to.
    #[error("the window failed: {reason}")]
    Window {
        /// What failed.
        reason: String,
    },
}

impl RunReplayError {
    pub(crate) fn window(reason: impl std::fmt::Display) -> Self {
        Self::Window {
            reason: reason.to_string(),
        }
    }
}

/// How long one wait for the GPU may take before the replay gives up.
const GPU_WAIT: Duration = Duration::from_secs(30);

/// Frames the replay lets the GPU queue before it waits for the oldest, as a browser does.
const FRAMES_IN_FLIGHT: usize = 2;

/// The WebGPU feature names the capture lists, as wgpu's features, and the names wgpu lacks.
pub(crate) fn features_of(names: &[String]) -> (wgpu::Features, Vec<String>) {
    let mut unknown = Vec::new();
    let features = names
        .iter()
        .fold(wgpu::Features::empty(), |features, name| {
            features
                | match name.as_str() {
                    "timestamp-query" => wgpu::Features::TIMESTAMP_QUERY,
                    "shader-f16" => wgpu::Features::SHADER_F16,
                    "subgroups" => wgpu::Features::SUBGROUP,
                    "float32-filterable" => wgpu::Features::FLOAT32_FILTERABLE,
                    "depth32float-stencil8" => wgpu::Features::DEPTH32FLOAT_STENCIL8,
                    "rg11b10ufloat-renderable" => wgpu::Features::RG11B10UFLOAT_RENDERABLE,
                    "bgra8unorm-storage" => wgpu::Features::BGRA8UNORM_STORAGE,
                    "indirect-first-instance" => wgpu::Features::INDIRECT_FIRST_INSTANCE,
                    "depth-clip-control" => wgpu::Features::DEPTH_CLIP_CONTROL,
                    "texture-compression-bc" => wgpu::Features::TEXTURE_COMPRESSION_BC,
                    "dual-source-blending" => wgpu::Features::DUAL_SOURCE_BLENDING,
                    _ => {
                        unknown.push(name.clone());
                        wgpu::Features::empty()
                    }
                }
        });
    (features, unknown)
}

/// The device a replay runs on: the adapter's limits, and the capture's features it has, with
/// `TIMESTAMP_QUERY` for the replay's own pass timer. A capture feature the adapter or wgpu lacks
/// is a finding, recorded in `findings`.
pub(crate) fn device_for(
    adapter: &wgpu::Adapter,
    capture: &Capture,
    findings: &mut Vec<String>,
) -> Result<(wgpu::Device, wgpu::Queue), RunReplayError> {
    let (captured, unknown) = features_of(capture.features());
    for name in unknown {
        findings.push(format!(
            "the capture's feature {name} is not one the replay maps"
        ));
    }
    let missing = captured - adapter.features();
    if !missing.is_empty() {
        findings.push(format!(
            "the adapter lacks the capture's features {missing:?}"
        ));
    }
    let wanted = captured | wgpu::Features::TIMESTAMP_QUERY;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("gpu-replay"),
        required_features: wanted & adapter.features(),
        required_limits: adapter.limits(),
        ..Default::default()
    }))
    .map_err(RunReplayError::NoDevice)
}

/// The setting the capture was taken at, from `--setting` or the capture's `meta.setting`.
pub(crate) fn setting_of(
    capture: &Capture,
    given: Option<Setting>,
) -> Result<Setting, RunReplayError> {
    if let Some(setting) = given {
        return Ok(setting);
    }
    match capture.meta().get("setting").and_then(Value::as_str) {
        Some("high") => Ok(Setting::High),
        Some("low") => Ok(Setting::Low),
        _ => Err(RunReplayError::NoSetting),
    }
}

pub(crate) fn rows_of(capture: &Capture) -> BTreeMap<String, PassRow> {
    capture
        .meta()
        .get("passRows")
        .and_then(Value::as_object)
        .map(|map| {
            map.iter()
                .map(|(label, row)| {
                    let row = match row.as_str() {
                        Some("terrain") => PassRow::Terrain,
                        Some("atmosphere") => PassRow::Atmosphere,
                        _ => PassRow::Other,
                    };
                    (label.clone(), row)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The bytes the span's writes upload.
pub(crate) fn upload_bytes(capture: &Capture) -> u64 {
    capture.calls()[capture.span_start().min(capture.calls().len())..]
        .iter()
        .filter(|call| {
            call.target == QUEUE_ID && (call.op == "writeBuffer" || call.op == "writeTexture")
        })
        .filter_map(|call| {
            let blob = call
                .args
                .iter()
                .find_map(|arg| arg.get("$blob")?.as_u64())?;
            let bytes = capture.blob(usize::try_from(blob).ok()?)?;
            u64::try_from(bytes.len()).ok()
        })
        .sum()
}

/// The main canvas: the largest the capture drew to.
pub(crate) fn main_surface(capture: &Capture) -> Option<&crate::capture::Surface> {
    capture
        .surfaces()
        .iter()
        .max_by_key(|surface| u64::from(surface.width) * u64::from(surface.height))
}

/// The calls of each captured frame, after the setup and snapshot.
pub(crate) fn frame_ranges(capture: &Capture) -> Vec<std::ops::Range<usize>> {
    let frames = capture.frames();
    let end = capture.calls().len();
    frames
        .iter()
        .enumerate()
        .map(|(i, start)| *start..frames.get(i + 1).copied().unwrap_or(end))
        .collect()
}

pub(crate) fn collect_errors(device: &wgpu::Device) -> Arc<Mutex<Vec<String>>> {
    let errors = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&errors);
    device.on_uncaptured_error(Arc::new(move |error: wgpu::Error| {
        sink.lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(error.to_string());
    }));
    errors
}

/// One frame's pass-time read-back: its staging buffer, labels and whether it has mapped.
#[derive(Debug)]
struct Reading {
    staging: wgpu::Buffer,
    labels: Vec<String>,
    mapped: Arc<Mutex<bool>>,
}

/// The frames' pass-time read-backs and the submissions that mark each frame's end.
#[derive(Debug, Default)]
pub(crate) struct Frames {
    readings: Vec<Reading>,
    markers: Vec<wgpu::SubmissionIndex>,
}

impl Frames {
    /// Ends a replayed frame: maps its pass times for reading, marks its end on the queue, and
    /// waits for the frame [`FRAMES_IN_FLIGHT`] before it, so that the GPU never has more queued.
    pub(crate) fn end_frame(
        &mut self,
        replayer: &mut Replayer,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<(), RunReplayError> {
        if let Some((staging, labels, _)) = replayer.finish_frame() {
            let mapped = Arc::new(Mutex::new(false));
            let flag = Arc::clone(&mapped);
            staging.map_async(wgpu::MapMode::Read, .., move |result| {
                *flag.lock().unwrap_or_else(PoisonError::into_inner) = result.is_ok();
            });
            self.readings.push(Reading {
                staging,
                labels,
                mapped,
            });
        }
        self.markers.push(queue.submit(std::iter::empty()));
        if let Some(oldest) = self
            .markers
            .len()
            .checked_sub(FRAMES_IN_FLIGHT + 1)
            .and_then(|i| self.markers.get(i))
        {
            device
                .poll(wgpu::PollType::Wait {
                    submission_index: Some(oldest.clone()),
                    timeout: Some(GPU_WAIT),
                })
                .map_err(RunReplayError::Poll)?;
        }
        Ok(())
    }

    /// Waits for every frame, then reads the pass times of each frame that mapped.
    pub(crate) fn finish(
        self,
        replayer: &Replayer,
        device: &wgpu::Device,
    ) -> Result<Vec<FramePassTimes>, RunReplayError> {
        device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(GPU_WAIT),
            })
            .map_err(RunReplayError::Poll)?;
        Ok(self
            .readings
            .iter()
            .filter(|reading| {
                *reading
                    .mapped
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
            })
            .map(|reading| replayer.read_pass_times(&reading.staging, &reading.labels))
            .collect())
    }
}

/// The intervals between successive frames' GPU ends, ms: what the GPU sustains offscreen.
pub(crate) fn gpu_intervals_ms(frames: &[FramePassTimes], period_ns: f32) -> Vec<f64> {
    let ends: Vec<u64> = frames.iter().filter_map(|frame| frame.end_ticks).collect();
    ends.windows(2)
        .map(|pair| {
            #[expect(
                clippy::cast_precision_loss,
                reason = "an interval's ticks are far below 2^52, so the f64 is exact"
            )]
            let ticks = pair[1].saturating_sub(pair[0]) as f64;
            ticks * f64::from(period_ns) / 1e6
        })
        .collect()
}

/// Replays a capture offscreen on the default high-performance adapter, every canvas an
/// offscreen texture, timing each frame by the GPU's timestamp at the end of its last pass.
///
/// # Errors
///
/// When there is no adapter or device, a canvas's format is unknown, a call cannot be replayed,
/// or waiting for the GPU fails.
pub fn replay_offscreen(
    capture: &Capture,
    capture_dir: &Path,
    setting: Option<Setting>,
) -> Result<ReplayFigures, RunReplayError> {
    let setting = setting_of(capture, setting)?;
    let started_at = SystemTime::now();
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .map_err(RunReplayError::NoAdapter)?;
    let mut findings = capture.problems().to_vec();
    let (device, queue) = device_for(&adapter, capture, &mut findings)?;
    let errors = collect_errors(&device);
    let mut replayer = Replayer::new(device.clone(), queue.clone(), capture.surfaces())?;
    replayer.replay(capture, 0..capture.span_start(), &FrameTarget::Offscreen)?;
    let mut frames = Frames::default();
    for range in frame_ranges(capture) {
        replayer.replay(capture, range, &FrameTarget::Offscreen)?;
        frames.end_frame(&mut replayer, &device, &queue)?;
    }
    let times = frames.finish(&replayer, &device)?;
    let main = main_surface(capture);
    Ok(ReplayFigures {
        started_at,
        capture: capture_dir.to_path_buf(),
        setting,
        seed: capture
            .meta()
            .get("seed")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_owned(),
        presented: false,
        display_hz: None,
        adapter: adapter.get_info(),
        timed: replayer.times_passes(),
        canvas: main.map_or((0, 0), |s| (s.width, s.height)),
        intervals_ms: gpu_intervals_ms(&times, replayer.timestamp_period_ns()),
        passes: times.into_iter().map(|frame| frame.passes).collect(),
        rows: rows_of(capture),
        untimed_passes: replayer.untimed_passes(),
        upload_bytes: upload_bytes(capture),
        errors: errors
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone(),
        findings,
    })
}

/// Replays a capture in a window with FIFO presentation at the captured resolution, the main
/// canvas presented and the others offscreen, timing each frame by its presentation.
///
/// # Errors
///
/// When there is no adapter, device or window, or a call cannot be replayed.
pub fn replay_presented(
    capture: &Capture,
    capture_dir: &Path,
    setting: Option<Setting>,
) -> Result<ReplayFigures, RunReplayError> {
    crate::window::run(capture, capture_dir, setting_of(capture, setting)?)
}
