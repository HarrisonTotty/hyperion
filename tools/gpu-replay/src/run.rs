//! Driving a replay: the device, the frames, their timing, offscreen or presented (R05.T15.c).

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Instant, SystemTime};

use serde_json::Value;

use crate::capture::{Capture, QUEUE_ID};
use crate::replay::{FramePassTimes, FrameTarget, ReplayCallError, Replayer};
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
    /// A call could not be replayed.
    #[error("the capture could not be replayed")]
    Call(#[from] ReplayCallError),
    /// The capture names no setting and none was given.
    #[error("the capture records no setting; pass --setting")]
    NoSetting,
    /// The window could not be made or drawn to.
    #[error("the window failed: {0}")]
    Window(String),
}

/// The WebGPU feature names the capture lists, as wgpu's features.
pub(crate) fn features_of(names: &[String]) -> wgpu::Features {
    names
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
                    _ => wgpu::Features::empty(),
                }
        })
}

/// The device a replay runs on: the adapter's limits, and the capture's features it has, with
/// `TIMESTAMP_QUERY` for the replay's own pass timer.
pub(crate) fn device_for(
    adapter: &wgpu::Adapter,
    capture: &Capture,
) -> Result<(wgpu::Device, wgpu::Queue), RunReplayError> {
    let wanted = features_of(capture.features()) | wgpu::Features::TIMESTAMP_QUERY;
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
            capture
                .blob(usize::try_from(blob).ok()?)
                .map(|bytes| bytes.len() as u64)
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

/// The pass-time readings in flight, collected as their mappings complete.
type Pending = Arc<Mutex<Vec<(wgpu::Buffer, Vec<String>, bool)>>>;

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

/// Replays a capture offscreen on the default high-performance adapter, every canvas an
/// offscreen texture, timing each frame by when the GPU completes it.
///
/// # Errors
///
/// When there is no adapter or device, or a call cannot be replayed.
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
    let (device, queue) = device_for(&adapter, capture)?;
    let errors = collect_errors(&device);
    let mut replayer = Replayer::new(device.clone(), queue.clone(), capture.surfaces());
    replayer.replay(capture, 0..capture.span_start(), &FrameTarget::Offscreen)?;
    let completions = Arc::new(Mutex::new(Vec::new()));
    let pending: Pending = Arc::new(Mutex::new(Vec::new()));
    for range in frame_ranges(capture) {
        replayer.replay(capture, range, &FrameTarget::Offscreen)?;
        let readback = replayer.finish_frame();
        let done = Arc::clone(&completions);
        queue.on_submitted_work_done(move || {
            done.lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(Instant::now());
        });
        if let Some((staging, labels)) = readback {
            map_for_reading(&pending, &staging, labels);
        }
        // Drives the callbacks without waiting, so that frames queue as the GPU allows.
        let _ = device.poll(wgpu::PollType::Poll);
    }
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    let completions = completions
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    let intervals_ms = completions
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).as_secs_f64() * 1000.0)
        .collect();
    let passes = read_all(&replayer, &pending);
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
        intervals_ms,
        passes,
        rows: rows_of(capture),
        untimed_passes: replayer.untimed_passes(),
        upload_bytes: upload_bytes(capture),
        errors: errors
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone(),
    })
}

pub(crate) fn map_for_reading(pending: &Pending, staging: &wgpu::Buffer, labels: Vec<String>) {
    let index = {
        let mut list = pending.lock().unwrap_or_else(PoisonError::into_inner);
        list.push((staging.clone(), labels, false));
        list.len() - 1
    };
    let ready = Arc::clone(pending);
    staging.map_async(wgpu::MapMode::Read, .., move |result| {
        if result.is_ok()
            && let Some(entry) = ready
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .get_mut(index)
        {
            entry.2 = true;
        }
    });
}

pub(crate) fn read_all(replayer: &Replayer, pending: &Pending) -> Vec<Vec<(String, f64)>> {
    pending
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .filter(|(_, _, mapped)| *mapped)
        .map(|(staging, labels, _)| {
            let FramePassTimes { passes } = replayer.read_pass_times(staging, labels);
            passes
        })
        .collect()
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

/// A frame's pending readings, for the window driver.
pub(crate) type PendingReadings = Pending;
