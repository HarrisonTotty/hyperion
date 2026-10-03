//! The presented replay: a winit window at the captured resolution, FIFO presentation, one
//! captured frame a presentation (R05.T15.c). It shows a window, so on the development machine it
//! is run by hand by the owner (R05 Risks; the lanes never show a window on `:0`).

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
    PendingReadings, RunReplayError, collect_errors, device_for, frame_ranges, main_surface,
    map_for_reading, read_all, rows_of, upload_bytes,
};

/// The window's state once the event loop has resumed.
struct Running {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    replayer: Replayer,
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
    presented_at: Vec<Instant>,
    pending: PendingReadings,
    failure: Option<RunReplayError>,
}

impl App<'_> {
    fn start(&mut self, event_loop: &ActiveEventLoop) -> Result<Running, RunReplayError> {
        let main = main_surface(self.capture)
            .ok_or_else(|| RunReplayError::Window("the capture drew to no canvas".to_owned()))?;
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("gpu-replay")
                        .with_resizable(false)
                        .with_inner_size(PhysicalSize::new(main.width, main.height)),
                )
                .map_err(|error| RunReplayError::Window(error.to_string()))?,
        );
        let surface = self
            .instance
            .create_surface(Arc::clone(&window))
            .map_err(|error| RunReplayError::Window(error.to_string()))?;
        let adapter =
            pollster::block_on(self.instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                ..Default::default()
            }))
            .map_err(RunReplayError::NoAdapter)?;
        let (device, queue) = device_for(&adapter, self.capture)?;
        let mut config = surface
            .get_default_config(&adapter, main.width, main.height)
            .ok_or_else(|| RunReplayError::Window("the surface suits no adapter".to_owned()))?;
        config.present_mode = wgpu::PresentMode::Fifo;
        config.desired_maximum_frame_latency = 2;
        if let Some(format) =
            serde_json::from_value::<wgpu::TextureFormat>(Value::String(main.format.clone())).ok()
            && surface.get_capabilities(&adapter).formats.contains(&format)
        {
            config.format = format;
        }
        let capabilities = surface.get_capabilities(&adapter);
        config.usage = wgpu::TextureUsages::RENDER_ATTACHMENT
            | (capabilities.usages
                & (wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC));
        surface.configure(&device, &config);
        let errors = collect_errors(&device);
        let mut replayer = Replayer::new(device.clone(), queue.clone(), self.capture.surfaces());
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
            device,
            queue,
            replayer,
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
        let (wgpu::CurrentSurfaceTexture::Success(frame)
        | wgpu::CurrentSurfaceTexture::Suboptimal(frame)) = running.surface.get_current_texture()
        else {
            // A skipped acquisition is retried on the next redraw.
            return Ok(true);
        };
        running.replayer.replay(
            self.capture,
            range,
            &FrameTarget::Presented {
                surface: main,
                texture: &frame.texture,
            },
        )?;
        if let Some((staging, labels)) = running.replayer.finish_frame() {
            map_for_reading(&self.pending, &staging, labels);
        }
        running.window.pre_present_notify();
        running.queue.present(frame);
        self.presented_at.push(Instant::now());
        let _ = running.device.poll(wgpu::PollType::Poll);
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
    let event_loop = EventLoop::new().map_err(|error| RunReplayError::Window(error.to_string()))?;
    let mut app = App {
        capture,
        instance: wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle(Box::new(
            event_loop.owned_display_handle(),
        ))),
        running: None,
        ranges: frame_ranges(capture),
        next: 0,
        presented_at: Vec::new(),
        pending: Arc::new(Mutex::new(Vec::new())),
        failure: None,
    };
    event_loop
        .run_app(&mut app)
        .map_err(|error| RunReplayError::Window(error.to_string()))?;
    if let Some(error) = app.failure {
        return Err(error);
    }
    let running = app
        .running
        .ok_or_else(|| RunReplayError::Window("the window never opened".to_owned()))?;
    let _ = running.device.poll(wgpu::PollType::wait_indefinitely());
    let intervals_ms = app
        .presented_at
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).as_secs_f64() * 1000.0)
        .collect();
    let passes = read_all(&running.replayer, &app.pending);
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
        passes,
        rows: rows_of(capture),
        untimed_passes: running.replayer.untimed_passes(),
        upload_bytes: upload_bytes(capture),
        errors: running
            .errors
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone(),
    })
}
