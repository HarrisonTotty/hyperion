//! The presented replay: a winit window at the captured resolution, FIFO presentation, one
//! captured frame a presentation (R05.T15.c). It shows a window, so on the development machine it
//! is run by hand by the owner (R05 Risks; the lanes never show a window on `:0`).
//!
//! A frame's time is taken when `present` returns: under FIFO with at most two frames queued,
//! `present` blocks until a swapchain image is free, so the intervals follow the display's
//! cadence, as the browser's presentation times do.

use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Instant, SystemTime};

use serde_json::Value;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

use crate::capture::Capture;
use crate::replay::{FrameTarget, Replayer};
use crate::results::{ReplayFigures, Setting};
use crate::run::{
    Frames, RunReplayError, collect_errors, device_for, frame_ranges, main_surface, rows_of,
    upload_bytes,
};

/// Acquisitions that may fail in a row before the replay gives up.
const ACQUIRE_RETRIES: u32 = 8;

/// The window's state once the event loop has resumed.
struct Running {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    queue: wgpu::Queue,
    replayer: Replayer,
    frames: Frames,
    adapter: wgpu::AdapterInfo,
    errors: Arc<Mutex<Vec<String>>>,
    display_hz: Option<f64>,
}

struct App<'a> {
    capture: &'a Capture,
    instance: wgpu::Instance,
    running: Option<Running>,
    ranges: Vec<std::ops::Range<usize>>,
    next: usize,
    failed_acquisitions: u32,
    presented_at: Vec<Instant>,
    findings: Vec<String>,
    failure: Option<RunReplayError>,
}

impl App<'_> {
    fn start(&mut self, event_loop: &ActiveEventLoop) -> Result<Running, RunReplayError> {
        let main = main_surface(self.capture)
            .ok_or_else(|| RunReplayError::window("the capture drew to no canvas"))?;
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("gpu-replay")
                        .with_resizable(false)
                        .with_inner_size(PhysicalSize::new(main.width, main.height)),
                )
                .map_err(RunReplayError::window)?,
        );
        let surface = self
            .instance
            .create_surface(Arc::clone(&window))
            .map_err(RunReplayError::window)?;
        let adapter =
            pollster::block_on(self.instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                ..Default::default()
            }))
            .map_err(RunReplayError::NoAdapter)?;
        let (device, queue) = device_for(&adapter, self.capture, &mut self.findings)?;
        let mut config = surface
            .get_default_config(&adapter, main.width, main.height)
            .ok_or_else(|| RunReplayError::window("the surface suits no adapter"))?;
        config.present_mode = wgpu::PresentMode::Fifo;
        config.desired_maximum_frame_latency = 2;
        let capabilities = surface.get_capabilities(&adapter);
        let wanted =
            serde_json::from_value::<wgpu::TextureFormat>(Value::String(main.format.clone()));
        match wanted {
            Ok(format) if capabilities.formats.contains(&format) => config.format = format,
            _ => self.findings.push(format!(
                "the window cannot present the capture's canvas format {}; it presents {:?}, so \
                 pipelines drawing to the canvas fail validation",
                main.format, config.format
            )),
        }
        config.usage = wgpu::TextureUsages::RENDER_ATTACHMENT
            | (capabilities.usages
                & (wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC));
        surface.configure(&device, &config);
        let errors = collect_errors(&device);
        let mut replayer = Replayer::new(device.clone(), queue.clone(), self.capture.surfaces())?;
        replayer.replay(
            self.capture,
            0..self.capture.span_start(),
            &FrameTarget::Offscreen,
        )?;
        let display_hz = window
            .current_monitor()
            .and_then(|monitor| monitor.refresh_rate_millihertz())
            .map(|millihertz| f64::from(millihertz) / 1000.0);
        Ok(Running {
            window,
            surface,
            config,
            device,
            queue,
            replayer,
            frames: Frames::default(),
            adapter: adapter.get_info(),
            errors,
            display_hz,
        })
    }

    /// Replays and presents the next frame; `false` once every frame is presented.
    fn frame(&mut self) -> Result<bool, RunReplayError> {
        let Some(range) = self.ranges.get(self.next).cloned() else {
            return Ok(false);
        };
        let Some(running) = self.running.as_mut() else {
            return Ok(true);
        };
        let main = main_surface(self.capture).map_or(0, |surface| surface.id);
        let frame = match running.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            other => {
                self.failed_acquisitions += 1;
                if self.failed_acquisitions > ACQUIRE_RETRIES {
                    return Err(RunReplayError::window(format!(
                        "the window's surface gave no texture {ACQUIRE_RETRIES} times running \
                         ({other:?})"
                    )));
                }
                // An outdated or lost surface is configured again; the frame is retried on the
                // next redraw.
                running.surface.configure(&running.device, &running.config);
                return Ok(true);
            }
        };
        self.failed_acquisitions = 0;
        running.replayer.replay(
            self.capture,
            range,
            &FrameTarget::Presented {
                surface: main,
                texture: &frame.texture,
            },
        )?;
        running.window.pre_present_notify();
        running.queue.present(frame);
        self.presented_at.push(Instant::now());
        running
            .frames
            .end_frame(&mut running.replayer, &running.device, &running.queue)?;
        self.next += 1;
        Ok(true)
    }
}

impl ApplicationHandler for App<'_> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.running.is_some() {
            return;
        }
        match self.start(event_loop) {
            Ok(running) => {
                running.window.request_redraw();
                self.running = Some(running);
            }
            Err(error) => {
                self.failure = Some(error);
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => match self.frame() {
                Ok(true) => {
                    if let Some(running) = &self.running {
                        running.window.request_redraw();
                    }
                }
                Ok(false) => event_loop.exit(),
                Err(error) => {
                    self.failure = Some(error);
                    event_loop.exit();
                }
            },
            _ => {}
        }
    }
}

/// Runs the presented replay to its last frame.
pub(crate) fn run(
    capture: &Capture,
    capture_dir: &Path,
    setting: Setting,
) -> Result<ReplayFigures, RunReplayError> {
    let started_at = SystemTime::now();
    let event_loop = EventLoop::new().map_err(RunReplayError::window)?;
    let mut app = App {
        capture,
        instance: wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle(Box::new(
            event_loop.owned_display_handle(),
        ))),
        running: None,
        ranges: frame_ranges(capture),
        next: 0,
        failed_acquisitions: 0,
        presented_at: Vec::new(),
        findings: capture.problems().to_vec(),
        failure: None,
    };
    event_loop
        .run_app(&mut app)
        .map_err(RunReplayError::window)?;
    if let Some(error) = app.failure {
        return Err(error);
    }
    let running = app
        .running
        .ok_or_else(|| RunReplayError::window("the window never opened"))?;
    let times = running.frames.finish(&running.replayer, &running.device)?;
    let intervals_ms = app
        .presented_at
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).as_secs_f64() * 1000.0)
        .collect();
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
        presented: true,
        display_hz: running.display_hz,
        adapter: running.adapter.clone(),
        timed: running.replayer.times_passes(),
        canvas: main.map_or((0, 0), |s| (s.width, s.height)),
        intervals_ms,
        passes: times.into_iter().map(|frame| frame.passes).collect(),
        rows: rows_of(capture),
        untimed_passes: running.replayer.untimed_passes(),
        upload_bytes: upload_bytes(capture),
        errors: running
            .errors
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone(),
        findings: app.findings,
    })
}
