//! Replaying a capture's calls in wgpu (R05.T15.c, Design note 22, step 2).
//!
//! Each logged call is mapped to wgpu's own: the objects the capture names by ID are kept in a
//! table, the descriptors are read out of the JSON the capture wrote (WebGPU's names, which
//! wgpu's types also deserialise from), and the passes are kept open across calls with
//! `forget_lifetime`. The capture's own timestamp queries are not replayed; the replayer times
//! every pass itself where the adapter has `TIMESTAMP_QUERY`, labelled as the capture labelled it.
//!
//! Buffers and textures gain `COPY_DST`, which the snapshot's writes need (the capture's usage is
//! the engine's own). Canvases are offscreen textures, except the one a [`FrameTarget`] presents.

use std::collections::HashMap;
use std::num::NonZeroU64;

use serde::de::DeserializeOwned;
use serde_json::Value;
use wgpu::util::DeviceExt as _;

use crate::capture::{Call, Capture, DEVICE_ID, QUEUE_ID, Surface};

/// Why a call could not be replayed.
#[derive(Debug, thiserror::Error)]
pub enum ReplayCallError {
    /// A call names an object the replay does not hold.
    #[error("call {index} ({op}) names object {id}, which the replay does not hold")]
    MissingObject {
        /// The call's index.
        index: usize,
        /// The call's method.
        op: String,
        /// The object.
        id: u64,
    },
    /// A call's target is not the kind of object its method belongs to.
    #[error("call {index}: object {id} has no method {op}")]
    WrongTarget {
        /// The call's index.
        index: usize,
        /// The call's method.
        op: String,
        /// The object.
        id: u64,
    },
    /// A call's arguments are not what its method takes.
    #[error("call {index} ({op}): {reason}")]
    BadArguments {
        /// The call's index.
        index: usize,
        /// The call's method.
        op: String,
        /// What was wrong.
        reason: String,
    },
}

/// An object the replay holds, by the capture's ID.
#[derive(Debug)]
enum Object {
    Buffer(wgpu::Buffer),
    Texture(wgpu::Texture),
    View(wgpu::TextureView),
    Sampler(wgpu::Sampler),
    Shader(wgpu::ShaderModule),
    BindGroupLayout(wgpu::BindGroupLayout),
    PipelineLayout(wgpu::PipelineLayout),
    BindGroup(wgpu::BindGroup),
    RenderPipeline(wgpu::RenderPipeline),
    ComputePipeline(wgpu::ComputePipeline),
    Encoder(wgpu::CommandEncoder),
    RenderPass(Box<wgpu::RenderPass<'static>>),
    ComputePass(Box<wgpu::ComputePass<'static>>),
    CommandBuffer(wgpu::CommandBuffer),
    /// A canvas: an offscreen texture, or the presented one's slot.
    Surface(u64),
    /// Something the replay deliberately drops: the capture's query sets.
    Skipped,
}

/// Where a frame's main canvas goes.
#[derive(Debug)]
pub enum FrameTarget<'a> {
    /// Every canvas is an offscreen texture.
    Offscreen,
    /// The given surface draws into this texture, which the caller presents.
    Presented {
        /// The capture's ID of the presented canvas.
        surface: u64,
        /// The window's texture for this frame.
        texture: &'a wgpu::Texture,
    },
}

/// The replay's own pass timer: one query set, resolved after every frame.
#[derive(Debug)]
struct PassTimer {
    queries: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    capacity: u32,
    /// The labels of this frame's passes, in query order.
    labels: Vec<String>,
}

/// A frame's pass times, read back.
#[derive(Debug, Clone, PartialEq)]
pub struct FramePassTimes {
    /// Each pass's label and its GPU time, ms.
    pub(crate) passes: Vec<(String, f64)>,
    /// The GPU's timestamp at the end of the frame's last timed pass, in its ticks, or `None`
    /// without one: successive frames' ends give the interval the GPU sustains.
    pub(crate) end_ticks: Option<u64>,
}

/// Holds a capture's objects on a device and replays its calls.
#[derive(Debug)]
pub struct Replayer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    objects: HashMap<u64, Object>,
    offscreen: HashMap<u64, wgpu::Texture>,
    timer: Option<PassTimer>,
    period_ns: f32,
    untimed: usize,
}

/// The passes a frame may time; more go untimed.
const TIMED_PASSES: u32 = 256;

impl Replayer {
    /// A replayer on `device`, with an offscreen texture for every canvas of `surfaces`.
    ///
    /// # Errors
    ///
    /// When a canvas's format is not one wgpu knows.
    pub fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        surfaces: &[Surface],
    ) -> Result<Self, UnknownFormatError> {
        let mut offscreen = HashMap::new();
        let mut objects = HashMap::new();
        for surface in surfaces {
            let format = texture_format(&surface.format).ok_or_else(|| UnknownFormatError {
                format: surface.format.clone(),
            })?;
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("offscreen canvas"),
                size: wgpu::Extent3d {
                    width: surface.width.max(1),
                    height: surface.height.max(1),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &canvas_view_formats(format),
            });
            offscreen.insert(surface.id, texture);
            objects.insert(surface.id, Object::Surface(surface.id));
        }
        let timer = device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY)
            .then(|| PassTimer {
                queries: device.create_query_set(&wgpu::QuerySetDescriptor {
                    label: Some("replay pass timer"),
                    ty: wgpu::QueryType::Timestamp,
                    count: TIMED_PASSES * 2,
                }),
                resolve: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("replay pass timer resolve"),
                    size: u64::from(TIMED_PASSES) * 2 * 8,
                    usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                }),
                capacity: TIMED_PASSES,
                labels: Vec::new(),
            });
        let period_ns = queue.get_timestamp_period();
        Ok(Self {
            device,
            queue,
            objects,
            offscreen,
            timer,
            period_ns,
            untimed: 0,
        })
    }

    /// The length of one timestamp tick, ns.
    #[must_use]
    pub fn timestamp_period_ns(&self) -> f32 {
        self.period_ns
    }

    /// Whether the replay times its passes (the adapter has `TIMESTAMP_QUERY`).
    #[must_use]
    pub fn times_passes(&self) -> bool {
        self.timer.is_some()
    }

    /// Passes that went untimed, past the timer's capacity a frame.
    #[must_use]
    pub fn untimed_passes(&self) -> usize {
        self.untimed
    }

    /// The device the replay runs on.
    #[must_use]
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// Replays calls `range` of the capture, the main canvas going to `target`.
    ///
    /// # Errors
    ///
    /// The first call that cannot be replayed.
    pub fn replay(
        &mut self,
        capture: &Capture,
        range: std::ops::Range<usize>,
        target: &FrameTarget<'_>,
    ) -> Result<(), ReplayCallError> {
        for index in range {
            let Some(call) = capture.calls().get(index) else {
                break;
            };
            self.call(capture, index, call, target)?;
        }
        Ok(())
    }

    /// Replays the calls before the span ([`Capture::setup_calls`]): every one, less the views,
    /// bind groups and pipelines' bind-group layouts the engine made and dropped before it.
    ///
    /// # Errors
    ///
    /// The first call that cannot be replayed.
    pub fn replay_setup(&mut self, capture: &Capture) -> Result<(), ReplayCallError> {
        for &index in capture.setup_calls() {
            if let Some(call) = capture.calls().get(index) {
                self.call(capture, index, call, &FrameTarget::Offscreen)?;
            }
        }
        Ok(())
    }

    /// Resolves this frame's pass timestamps and returns a buffer to read them from after the
    /// frame's work is done, with the labels in order and the submission that resolves them;
    /// `None` without a timer or a timed pass.
    pub fn finish_frame(&mut self) -> Option<(wgpu::Buffer, Vec<String>, wgpu::SubmissionIndex)> {
        let timer = self.timer.as_mut()?;
        if timer.labels.is_empty() {
            return None;
        }
        let labels = std::mem::take(&mut timer.labels);
        let count = u32::try_from(labels.len()).ok()? * 2;
        let bytes = u64::from(count) * 8;
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("replay pass timer staging"),
            size: bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("replay pass timer"),
            });
        encoder.resolve_query_set(&timer.queries, 0..count, &timer.resolve, 0);
        encoder.copy_buffer_to_buffer(&timer.resolve, 0, &staging, 0, bytes);
        let submission = self.queue.submit([encoder.finish()]);
        Some((staging, labels, submission))
    }

    /// Reads back a frame's pass times from a buffer [`Replayer::finish_frame`] gave, once its
    /// mapping has completed.
    ///
    /// # Errors
    ///
    /// When the mapped range cannot be read: the frame's pass times are then incomplete
    /// (R05.T14.j).
    pub fn read_pass_times(
        &self,
        staging: &wgpu::Buffer,
        labels: &[String],
    ) -> Result<FramePassTimes, wgpu::MapRangeError> {
        let data = staging.slice(..).get_mapped_range()?;
        let stamps: Vec<u64> = data
            .as_chunks::<8>()
            .0
            .iter()
            .map(|chunk| u64::from_le_bytes(*chunk))
            .collect();
        drop(data);
        staging.unmap();
        let end_ticks = (0..labels.len())
            .filter_map(|i| stamps.get(2 * i + 1).copied())
            .max();
        let passes = labels
            .iter()
            .enumerate()
            .map(|(i, label)| {
                let begin = stamps.get(2 * i).copied().unwrap_or(0);
                let end = stamps.get(2 * i + 1).copied().unwrap_or(begin);
                let ticks = end.saturating_sub(begin);
                #[expect(
                    clippy::cast_precision_loss,
                    reason = "a pass's ticks are far below 2^52, so the f64 is exact"
                )]
                let ns = ticks as f64 * f64::from(self.period_ns);
                (label.clone(), ns / 1e6)
            })
            .collect();
        Ok(FramePassTimes { passes, end_ticks })
    }

    fn missing(index: usize, call: &Call, id: u64) -> ReplayCallError {
        ReplayCallError::MissingObject {
            index,
            op: call.op.clone(),
            id,
        }
    }

    fn bad(index: usize, call: &Call, reason: impl Into<String>) -> ReplayCallError {
        ReplayCallError::BadArguments {
            index,
            op: call.op.clone(),
            reason: reason.into(),
        }
    }

    fn call(
        &mut self,
        capture: &Capture,
        index: usize,
        call: &Call,
        target: &FrameTarget<'_>,
    ) -> Result<(), ReplayCallError> {
        let args = Args {
            capture,
            values: &call.args,
            index,
            call,
        };
        let made = match (call.target, call.op.as_str()) {
            (DEVICE_ID, "createBuffer") => Some(Object::Buffer(self.create_buffer(&args)?)),
            (DEVICE_ID, "createTexture") => Some(Object::Texture(self.create_texture(&args)?)),
            (DEVICE_ID, "createSampler") => Some(Object::Sampler(self.create_sampler(&args))),
            (DEVICE_ID, "createShaderModule") => {
                let code = args.field_str(0, "code")?;
                let label = args.label(0);
                Some(Object::Shader(self.device.create_shader_module(
                    wgpu::ShaderModuleDescriptor {
                        label: label.as_deref(),
                        source: wgpu::ShaderSource::Wgsl(code.into()),
                    },
                )))
            }
            (DEVICE_ID, "createBindGroupLayout") => Some(Object::BindGroupLayout(
                self.create_bind_group_layout(&args)?,
            )),
            (DEVICE_ID, "createPipelineLayout") => {
                Some(Object::PipelineLayout(self.create_pipeline_layout(&args)?))
            }
            (DEVICE_ID, "createBindGroup") => {
                Some(Object::BindGroup(self.create_bind_group(&args)?))
            }
            (DEVICE_ID, "createRenderPipeline") => {
                Some(Object::RenderPipeline(self.create_render_pipeline(&args)?))
            }
            (DEVICE_ID, "createComputePipeline") => Some(Object::ComputePipeline(
                self.create_compute_pipeline(&args)?,
            )),
            (DEVICE_ID, "createQuerySet") => Some(Object::Skipped),
            (DEVICE_ID, "createCommandEncoder") => Some(Object::Encoder(
                self.device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: args.label(0).as_deref(),
                    }),
            )),
            (QUEUE_ID, "writeBuffer") => {
                let buffer = self.buffer(&args, args.reference(0)?)?;
                let offset = args.u64_at(1)?;
                let bytes = args.blob(2)?;
                self.queue.write_buffer(buffer, offset, bytes);
                None
            }
            (QUEUE_ID, "writeTexture") => {
                let destination = args.get(0)?;
                let texture = self.texture_of(&args, destination)?;
                let bytes = args.blob(1)?;
                let layout = buffer_layout(args.get(2)?);
                let size = extent(args.get(3)?);
                self.queue.write_texture(
                    texel_copy_texture(texture, destination),
                    bytes,
                    layout,
                    size,
                );
                None
            }
            (QUEUE_ID, "submit") => {
                let list = args
                    .get(0)?
                    .as_array()
                    .ok_or_else(|| Self::bad(index, call, "no list"))?;
                let mut buffers = Vec::with_capacity(list.len());
                for item in list {
                    let id =
                        ref_of(item).ok_or_else(|| Self::bad(index, call, "not a reference"))?;
                    match self.objects.remove(&id) {
                        Some(Object::CommandBuffer(buffer)) => buffers.push(buffer),
                        _ => return Err(Self::missing(index, call, id)),
                    }
                }
                self.queue.submit(buffers);
                None
            }
            (id, op) => return self.call_on_object(&args, id, op, target),
        };
        if let (Some(object), Some(result)) = (made, call.result) {
            self.objects.insert(result, object);
        }
        Ok(())
    }

    fn call_on_object(
        &mut self,
        args: &Args<'_>,
        id: u64,
        op: &str,
        target: &FrameTarget<'_>,
    ) -> Result<(), ReplayCallError> {
        let (index, call) = (args.index, args.call);
        let Some(object) = self.objects.remove(&id) else {
            return Err(Self::missing(index, call, id));
        };
        let wrong = || ReplayCallError::WrongTarget {
            index,
            op: op.to_owned(),
            id,
        };
        let (kept, made) = match (object, op) {
            (Object::Surface(surface), "getCurrentTexture") => {
                let texture = match target {
                    FrameTarget::Presented {
                        surface: presented,
                        texture,
                    } if *presented == surface => (*texture).clone(),
                    _ => self
                        .offscreen
                        .get(&surface)
                        .cloned()
                        .ok_or_else(|| Self::missing(index, call, surface))?,
                };
                (
                    Some(Object::Surface(surface)),
                    Some(Object::Texture(texture)),
                )
            }
            (Object::Texture(texture), "createView") => {
                let view = Self::create_view(args, &texture);
                (Some(Object::Texture(texture)), Some(Object::View(view)))
            }
            (Object::Texture(texture), "destroy") => {
                texture.destroy();
                (None, None)
            }
            (Object::Buffer(buffer), "destroy") => {
                buffer.destroy();
                (None, None)
            }
            (Object::Skipped, _) => (Some(Object::Skipped), None),
            (Object::RenderPipeline(pipeline), "getBindGroupLayout") => {
                let layout = pipeline.get_bind_group_layout(args.u32_at(0)?);
                (
                    Some(Object::RenderPipeline(pipeline)),
                    Some(Object::BindGroupLayout(layout)),
                )
            }
            (Object::ComputePipeline(pipeline), "getBindGroupLayout") => {
                let layout = pipeline.get_bind_group_layout(args.u32_at(0)?);
                (
                    Some(Object::ComputePipeline(pipeline)),
                    Some(Object::BindGroupLayout(layout)),
                )
            }
            (Object::Encoder(encoder), op) => self.encoder_call(args, encoder, op)?,
            (Object::RenderPass(pass), op) => (self.render_pass_call(args, *pass, op)?, None),
            (Object::ComputePass(pass), op) => (self.compute_pass_call(args, *pass, op)?, None),
            (object, _) => {
                self.objects.insert(id, object);
                return Err(wrong());
            }
        };
        if let Some(kept) = kept {
            self.objects.insert(id, kept);
        }
        if let (Some(made), Some(result)) = (made, call.result) {
            self.objects.insert(result, made);
        }
        Ok(())
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one arm an encoder method: the dispatch reads best as one table"
    )]
    fn encoder_call(
        &mut self,
        args: &Args<'_>,
        mut encoder: wgpu::CommandEncoder,
        op: &str,
    ) -> Result<(Option<Object>, Option<Object>), ReplayCallError> {
        let made = match op {
            "beginRenderPass" => Some(Object::RenderPass(Box::new(
                self.begin_render_pass(args, &mut encoder)?,
            ))),
            "beginComputePass" => {
                let label = args.label(0);
                let timestamp_writes = self.timestamp_slot(label.as_deref());
                let pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: label.as_deref(),
                    timestamp_writes: timestamp_writes.as_ref().map(|(set, begin)| {
                        wgpu::ComputePassTimestampWrites {
                            query_set: set,
                            beginning_of_pass_write_index: Some(*begin),
                            end_of_pass_write_index: Some(begin + 1),
                        }
                    }),
                });
                Some(Object::ComputePass(Box::new(pass.forget_lifetime())))
            }
            "copyBufferToBuffer" => {
                // WebGPU allows (source, destination, size?) beside the five-argument form.
                if args.values.len() >= 4 {
                    let source = self.buffer(args, args.reference(0)?)?;
                    let destination = self.buffer(args, args.reference(2)?)?;
                    let size = args.optional_u64(4);
                    encoder.copy_buffer_to_buffer(
                        source,
                        args.u64_at(1)?,
                        destination,
                        args.u64_at(3)?,
                        size,
                    );
                } else {
                    let source = self.buffer(args, args.reference(0)?)?;
                    let destination = self.buffer(args, args.reference(1)?)?;
                    encoder.copy_buffer_to_buffer(source, 0, destination, 0, args.optional_u64(2));
                }
                None
            }
            "copyBufferToTexture" => {
                let source = args.get(0)?;
                let buffer = self.buffer(
                    args,
                    field_ref(source, "buffer")
                        .ok_or_else(|| Self::bad(args.index, args.call, "no buffer"))?,
                )?;
                let destination = args.get(1)?;
                let texture = self.texture_of(args, destination)?;
                encoder.copy_buffer_to_texture(
                    wgpu::TexelCopyBufferInfo {
                        buffer,
                        layout: buffer_layout(source),
                    },
                    texel_copy_texture(texture, destination),
                    extent(args.get(2)?),
                );
                None
            }
            "copyTextureToBuffer" => {
                let source = args.get(0)?;
                let texture = self.texture_of(args, source)?;
                let destination = args.get(1)?;
                let buffer = self.buffer(
                    args,
                    field_ref(destination, "buffer")
                        .ok_or_else(|| Self::bad(args.index, args.call, "no buffer"))?,
                )?;
                encoder.copy_texture_to_buffer(
                    texel_copy_texture(texture, source),
                    wgpu::TexelCopyBufferInfo {
                        buffer,
                        layout: buffer_layout(destination),
                    },
                    extent(args.get(2)?),
                );
                None
            }
            "copyTextureToTexture" => {
                let source = args.get(0)?;
                let destination = args.get(1)?;
                let from = self.texture_of(args, source)?;
                let to = self.texture_of(args, destination)?;
                encoder.copy_texture_to_texture(
                    texel_copy_texture(from, source),
                    texel_copy_texture(to, destination),
                    extent(args.get(2)?),
                );
                None
            }
            "clearBuffer" => {
                let buffer = self.buffer(args, args.reference(0)?)?;
                encoder.clear_buffer(
                    buffer,
                    args.optional_u64(1).unwrap_or(0),
                    args.optional_u64(2),
                );
                None
            }
            // The capture's own queries are not replayed; the replay times the passes itself.
            "resolveQuerySet" => None,
            "pushDebugGroup" => {
                encoder.push_debug_group(args.str_at(0).unwrap_or(""));
                None
            }
            "popDebugGroup" => {
                encoder.pop_debug_group();
                None
            }
            "insertDebugMarker" => {
                encoder.insert_debug_marker(args.str_at(0).unwrap_or(""));
                None
            }
            "finish" => return Ok((None, Some(Object::CommandBuffer(encoder.finish())))),
            _ => {
                return Err(ReplayCallError::WrongTarget {
                    index: args.index,
                    op: op.to_owned(),
                    id: args.call.target,
                });
            }
        };
        Ok((Some(Object::Encoder(encoder)), made))
    }

    /// The next pair of timestamp queries for a pass labelled `label`, or `None` once full or
    /// without a timer.
    fn timestamp_slot(&mut self, label: Option<&str>) -> Option<(wgpu::QuerySet, u32)> {
        let timer = self.timer.as_mut()?;
        let next = u32::try_from(timer.labels.len()).ok()?;
        if next >= timer.capacity {
            self.untimed += 1;
            return None;
        }
        timer.labels.push(label.unwrap_or("").to_owned());
        Some((timer.queries.clone(), next * 2))
    }

    #[expect(
        clippy::too_many_lines,
        reason = "a render pass's descriptor has that many parts"
    )]
    fn begin_render_pass(
        &mut self,
        args: &Args<'_>,
        encoder: &mut wgpu::CommandEncoder,
    ) -> Result<wgpu::RenderPass<'static>, ReplayCallError> {
        let descriptor = args.get(0)?;
        let label = args.label(0);
        let slot = self.timestamp_slot(label.as_deref());
        let mut colour = Vec::new();
        for attachment in descriptor
            .get("colorAttachments")
            .and_then(Value::as_array)
            .map_or(&[][..], Vec::as_slice)
        {
            if attachment.is_null() {
                colour.push(None);
                continue;
            }
            let view = self.view(
                args,
                field_ref(attachment, "view")
                    .ok_or_else(|| Self::bad(args.index, args.call, "an attachment has no view"))?,
            )?;
            let resolve = match field_ref(attachment, "resolveTarget") {
                Some(id) => Some(self.view(args, id)?),
                None => None,
            };
            let load = if attachment.get("loadOp").and_then(Value::as_str) == Some("clear") {
                wgpu::LoadOp::Clear(colour_of(attachment.get("clearValue")))
            } else {
                wgpu::LoadOp::Load
            };
            colour.push(Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: attachment
                    .get("depthSlice")
                    .and_then(Value::as_u64)
                    .and_then(|v| u32::try_from(v).ok()),
                resolve_target: resolve,
                ops: wgpu::Operations {
                    load,
                    store: store_of(attachment.get("storeOp")),
                },
            }));
        }
        let depth = match descriptor.get("depthStencilAttachment") {
            Some(attachment) if attachment.is_object() => {
                let view = self.view(
                    args,
                    field_ref(attachment, "view").ok_or_else(|| {
                        Self::bad(args.index, args.call, "the depth attachment has no view")
                    })?,
                )?;
                let depth_ops = attachment.get("depthLoadOp").map(|load| wgpu::Operations {
                    load: if load.as_str() == Some("clear") {
                        #[expect(
                            clippy::cast_possible_truncation,
                            reason = "a depth clear value is in [0, 1], exact in f32"
                        )]
                        let value = attachment
                            .get("depthClearValue")
                            .and_then(Value::as_f64)
                            .unwrap_or(0.0) as f32;
                        wgpu::LoadOp::Clear(value)
                    } else {
                        wgpu::LoadOp::Load
                    },
                    store: store_of(attachment.get("depthStoreOp")),
                });
                let stencil_ops = attachment
                    .get("stencilLoadOp")
                    .map(|load| wgpu::Operations {
                        load: if load.as_str() == Some("clear") {
                            wgpu::LoadOp::Clear(
                                attachment
                                    .get("stencilClearValue")
                                    .and_then(Value::as_u64)
                                    .and_then(|v| u32::try_from(v).ok())
                                    .unwrap_or(0),
                            )
                        } else {
                            wgpu::LoadOp::Load
                        },
                        store: store_of(attachment.get("stencilStoreOp")),
                    });
                Some(wgpu::RenderPassDepthStencilAttachment {
                    view,
                    depth_ops,
                    stencil_ops,
                })
            }
            _ => None,
        };
        let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: label.as_deref(),
            color_attachments: &colour,
            depth_stencil_attachment: depth,
            timestamp_writes: slot
                .as_ref()
                .map(|(set, begin)| wgpu::RenderPassTimestampWrites {
                    query_set: set,
                    beginning_of_pass_write_index: Some(*begin),
                    end_of_pass_write_index: Some(begin + 1),
                }),
            occlusion_query_set: None,
            multiview_mask: None,
        });
        Ok(pass.forget_lifetime())
    }

    fn render_pass_call(
        &self,
        args: &Args<'_>,
        mut pass: wgpu::RenderPass<'static>,
        op: &str,
    ) -> Result<Option<Object>, ReplayCallError> {
        match op {
            "setPipeline" => match self.objects.get(&args.reference(0)?) {
                Some(Object::RenderPipeline(pipeline)) => pass.set_pipeline(pipeline),
                _ => return Err(Self::missing(args.index, args.call, args.reference(0)?)),
            },
            "setBindGroup" => {
                let group = match ref_of(args.get(1)?) {
                    Some(id) => Some(self.bind_group(args, id)?),
                    None => None,
                };
                pass.set_bind_group(args.u32_at(0)?, group, &args.dynamic_offsets()?);
            }
            "setVertexBuffer" => {
                if let Some(id) = ref_of(args.get(1)?) {
                    let buffer = self.buffer(args, id)?;
                    let slice = args.slice(buffer, 2)?;
                    pass.set_vertex_buffer(args.u32_at(0)?, slice);
                }
            }
            "setIndexBuffer" => {
                let buffer = self.buffer(args, args.reference(0)?)?;
                let format = enum_of::<wgpu::IndexFormat>(args.get(1)?)
                    .ok_or_else(|| Self::bad(args.index, args.call, "an unknown index format"))?;
                pass.set_index_buffer(args.slice(buffer, 2)?, format);
            }
            "draw" => {
                let vertices = args.range(args.optional_u32(2).unwrap_or(0), args.u32_at(0)?)?;
                let instances = args.range(
                    args.optional_u32(3).unwrap_or(0),
                    args.optional_u32(1).unwrap_or(1),
                )?;
                pass.draw(vertices, instances);
            }
            "drawIndexed" => {
                let indices = args.range(args.optional_u32(2).unwrap_or(0), args.u32_at(0)?)?;
                let base_vertex = args.optional_i32(3).unwrap_or(0);
                let instances = args.range(
                    args.optional_u32(4).unwrap_or(0),
                    args.optional_u32(1).unwrap_or(1),
                )?;
                pass.draw_indexed(indices, base_vertex, instances);
            }
            "drawIndirect" => {
                pass.draw_indirect(self.buffer(args, args.reference(0)?)?, args.u64_at(1)?);
            }
            "drawIndexedIndirect" => {
                pass.draw_indexed_indirect(self.buffer(args, args.reference(0)?)?, args.u64_at(1)?);
            }
            "setViewport" => {
                let f = |i: usize| -> Result<f32, ReplayCallError> {
                    #[expect(
                        clippy::cast_possible_truncation,
                        reason = "viewport coordinates are pixels and depths in [0, 1]"
                    )]
                    let value = args.f64_at(i)? as f32;
                    Ok(value)
                };
                pass.set_viewport(f(0)?, f(1)?, f(2)?, f(3)?, f(4)?, f(5)?);
            }
            "setScissorRect" => {
                pass.set_scissor_rect(
                    args.u32_at(0)?,
                    args.u32_at(1)?,
                    args.u32_at(2)?,
                    args.u32_at(3)?,
                );
            }
            "setBlendConstant" => pass.set_blend_constant(colour_of(args.values.first())),
            "setStencilReference" => pass.set_stencil_reference(args.u32_at(0)?),
            "pushDebugGroup" => pass.push_debug_group(args.str_at(0).unwrap_or("")),
            "popDebugGroup" => pass.pop_debug_group(),
            "insertDebugMarker" => pass.insert_debug_marker(args.str_at(0).unwrap_or("")),
            "end" => return Ok(None),
            _ => {
                return Err(ReplayCallError::WrongTarget {
                    index: args.index,
                    op: op.to_owned(),
                    id: args.call.target,
                });
            }
        }
        Ok(Some(Object::RenderPass(Box::new(pass))))
    }

    fn compute_pass_call(
        &self,
        args: &Args<'_>,
        mut pass: wgpu::ComputePass<'static>,
        op: &str,
    ) -> Result<Option<Object>, ReplayCallError> {
        match op {
            "setPipeline" => match self.objects.get(&args.reference(0)?) {
                Some(Object::ComputePipeline(pipeline)) => pass.set_pipeline(pipeline),
                _ => return Err(Self::missing(args.index, args.call, args.reference(0)?)),
            },
            "setBindGroup" => {
                let group = match ref_of(args.get(1)?) {
                    Some(id) => Some(self.bind_group(args, id)?),
                    None => None,
                };
                pass.set_bind_group(args.u32_at(0)?, group, &args.dynamic_offsets()?);
            }
            "dispatchWorkgroups" => pass.dispatch_workgroups(
                args.u32_at(0)?,
                args.optional_u32(1).unwrap_or(1),
                args.optional_u32(2).unwrap_or(1),
            ),
            "dispatchWorkgroupsIndirect" => {
                pass.dispatch_workgroups_indirect(
                    self.buffer(args, args.reference(0)?)?,
                    args.u64_at(1)?,
                );
            }
            "pushDebugGroup" => pass.push_debug_group(args.str_at(0).unwrap_or("")),
            "popDebugGroup" => pass.pop_debug_group(),
            "insertDebugMarker" => pass.insert_debug_marker(args.str_at(0).unwrap_or("")),
            "end" => return Ok(None),
            _ => {
                return Err(ReplayCallError::WrongTarget {
                    index: args.index,
                    op: op.to_owned(),
                    id: args.call.target,
                });
            }
        }
        Ok(Some(Object::ComputePass(Box::new(pass))))
    }

    fn buffer(&self, args: &Args<'_>, id: u64) -> Result<&wgpu::Buffer, ReplayCallError> {
        match self.objects.get(&id) {
            Some(Object::Buffer(buffer)) => Ok(buffer),
            _ => Err(Self::missing(args.index, args.call, id)),
        }
    }

    fn view(&self, args: &Args<'_>, id: u64) -> Result<&wgpu::TextureView, ReplayCallError> {
        match self.objects.get(&id) {
            Some(Object::View(view)) => Ok(view),
            _ => Err(Self::missing(args.index, args.call, id)),
        }
    }

    fn bind_group(&self, args: &Args<'_>, id: u64) -> Result<&wgpu::BindGroup, ReplayCallError> {
        match self.objects.get(&id) {
            Some(Object::BindGroup(group)) => Ok(group),
            _ => Err(Self::missing(args.index, args.call, id)),
        }
    }

    fn texture_of(&self, args: &Args<'_>, copy: &Value) -> Result<&wgpu::Texture, ReplayCallError> {
        let id = field_ref(copy, "texture")
            .ok_or_else(|| Self::bad(args.index, args.call, "no texture"))?;
        match self.objects.get(&id) {
            Some(Object::Texture(texture)) => Ok(texture),
            _ => Err(Self::missing(args.index, args.call, id)),
        }
    }

    fn create_buffer(&self, args: &Args<'_>) -> Result<wgpu::Buffer, ReplayCallError> {
        let descriptor = args.get(0)?;
        let mut usage =
            wgpu::BufferUsages::from_bits_truncate(field_u32(descriptor, "usage").unwrap_or(0));
        if !usage.contains(wgpu::BufferUsages::MAP_WRITE) {
            usage |= wgpu::BufferUsages::COPY_DST;
        }
        let size = descriptor
            .get("size")
            .and_then(Value::as_u64)
            .ok_or_else(|| Self::bad(args.index, args.call, "no size"))?;
        let label = args.label(0);
        if descriptor.get("mappedAtCreation").and_then(Value::as_bool) == Some(true) {
            // Its contents were written through a mapping the capture did not see; zeros stand in.
            return Ok(self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: label.as_deref(),
                    contents: &vec![0; usize::try_from(size).unwrap_or(0)],
                    usage,
                }));
        }
        Ok(self.device.create_buffer(&wgpu::BufferDescriptor {
            label: label.as_deref(),
            size,
            usage,
            mapped_at_creation: false,
        }))
    }

    fn create_texture(&self, args: &Args<'_>) -> Result<wgpu::Texture, ReplayCallError> {
        let descriptor = args.get(0)?;
        let format = descriptor
            .get("format")
            .and_then(enum_of::<wgpu::TextureFormat>)
            .ok_or_else(|| Self::bad(args.index, args.call, "an unknown texture format"))?;
        let mut usage =
            wgpu::TextureUsages::from_bits_truncate(field_u32(descriptor, "usage").unwrap_or(0));
        if field_u32(descriptor, "sampleCount").unwrap_or(1) == 1 {
            usage |= wgpu::TextureUsages::COPY_DST;
        }
        let view_formats: Vec<wgpu::TextureFormat> = descriptor
            .get("viewFormats")
            .and_then(Value::as_array)
            .map_or(Some(Vec::new()), |list| list.iter().map(enum_of).collect())
            .ok_or_else(|| Self::bad(args.index, args.call, "an unknown view format"))?;
        let label = args.label(0);
        Ok(self.device.create_texture(&wgpu::TextureDescriptor {
            label: label.as_deref(),
            size: extent(descriptor.get("size").unwrap_or(&Value::Null)),
            mip_level_count: field_u32(descriptor, "mipLevelCount").unwrap_or(1),
            sample_count: field_u32(descriptor, "sampleCount").unwrap_or(1),
            dimension: descriptor
                .get("dimension")
                .and_then(enum_of)
                .unwrap_or(wgpu::TextureDimension::D2),
            format,
            usage,
            view_formats: &view_formats,
        }))
    }

    fn create_view(args: &Args<'_>, texture: &wgpu::Texture) -> wgpu::TextureView {
        let descriptor = args.values.first().filter(|value| value.is_object());
        let field = |key: &str| descriptor.and_then(|d| d.get(key));
        let label = args.label(0);
        texture.create_view(&wgpu::TextureViewDescriptor {
            label: label.as_deref(),
            format: field("format").and_then(enum_of),
            dimension: field("dimension").and_then(enum_of),
            usage: None,
            aspect: field("aspect")
                .and_then(enum_of)
                .unwrap_or(wgpu::TextureAspect::All),
            base_mip_level: descriptor
                .and_then(|d| field_u32(d, "baseMipLevel"))
                .unwrap_or(0),
            mip_level_count: descriptor.and_then(|d| field_u32(d, "mipLevelCount")),
            base_array_layer: descriptor
                .and_then(|d| field_u32(d, "baseArrayLayer"))
                .unwrap_or(0),
            array_layer_count: descriptor.and_then(|d| field_u32(d, "arrayLayerCount")),
        })
    }

    fn create_sampler(&self, args: &Args<'_>) -> wgpu::Sampler {
        let empty = Value::Object(serde_json::Map::new());
        let descriptor = args
            .values
            .first()
            .filter(|value| value.is_object())
            .unwrap_or(&empty);
        let address = |key: &str| {
            descriptor
                .get(key)
                .and_then(enum_of)
                .unwrap_or(wgpu::AddressMode::ClampToEdge)
        };
        let filter = |key: &str| {
            descriptor
                .get(key)
                .and_then(enum_of)
                .unwrap_or(wgpu::FilterMode::Nearest)
        };
        let label = args.label(0);
        #[expect(
            clippy::cast_possible_truncation,
            reason = "level-of-detail clamps are small, exact in f32"
        )]
        let lod = |key: &str, default: f64| {
            descriptor
                .get(key)
                .and_then(Value::as_f64)
                .unwrap_or(default) as f32
        };
        self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: label.as_deref(),
            address_mode_u: address("addressModeU"),
            address_mode_v: address("addressModeV"),
            address_mode_w: address("addressModeW"),
            mag_filter: filter("magFilter"),
            min_filter: filter("minFilter"),
            mipmap_filter: descriptor
                .get("mipmapFilter")
                .and_then(enum_of)
                .unwrap_or(wgpu::MipmapFilterMode::Nearest),
            lod_min_clamp: lod("lodMinClamp", 0.0),
            lod_max_clamp: lod("lodMaxClamp", 32.0),
            compare: descriptor.get("compare").and_then(enum_of),
            anisotropy_clamp: descriptor
                .get("maxAnisotropy")
                .and_then(Value::as_u64)
                .and_then(|v| u16::try_from(v).ok())
                .unwrap_or(1),
            border_color: None,
        })
    }

    fn create_bind_group_layout(
        &self,
        args: &Args<'_>,
    ) -> Result<wgpu::BindGroupLayout, ReplayCallError> {
        let descriptor = args.get(0)?;
        let mut entries = Vec::new();
        for entry in descriptor
            .get("entries")
            .and_then(Value::as_array)
            .map_or(&[][..], Vec::as_slice)
        {
            let binding = field_u32(entry, "binding")
                .ok_or_else(|| Self::bad(args.index, args.call, "an entry has no binding"))?;
            let visibility =
                wgpu::ShaderStages::from_bits_truncate(field_u32(entry, "visibility").unwrap_or(0));
            let ty = binding_type(entry).ok_or_else(|| {
                Self::bad(
                    args.index,
                    args.call,
                    format!("entry {binding} has no binding type"),
                )
            })?;
            entries.push(wgpu::BindGroupLayoutEntry {
                binding,
                visibility,
                ty,
                count: None,
            });
        }
        let label = args.label(0);
        Ok(self
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: label.as_deref(),
                entries: &entries,
            }))
    }

    fn create_pipeline_layout(
        &self,
        args: &Args<'_>,
    ) -> Result<wgpu::PipelineLayout, ReplayCallError> {
        let descriptor = args.get(0)?;
        let mut layouts = Vec::new();
        for item in descriptor
            .get("bindGroupLayouts")
            .and_then(Value::as_array)
            .map_or(&[][..], Vec::as_slice)
        {
            match ref_of(item) {
                Some(id) => match self.objects.get(&id) {
                    Some(Object::BindGroupLayout(layout)) => layouts.push(Some(layout)),
                    _ => return Err(Self::missing(args.index, args.call, id)),
                },
                None => layouts.push(None),
            }
        }
        let label = args.label(0);
        Ok(self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: label.as_deref(),
                bind_group_layouts: &layouts,
                immediate_size: 0,
            }))
    }

    fn create_bind_group(&self, args: &Args<'_>) -> Result<wgpu::BindGroup, ReplayCallError> {
        let descriptor = args.get(0)?;
        let layout_id = field_ref(descriptor, "layout")
            .ok_or_else(|| Self::bad(args.index, args.call, "no layout"))?;
        let Some(Object::BindGroupLayout(layout)) = self.objects.get(&layout_id) else {
            return Err(Self::missing(args.index, args.call, layout_id));
        };
        let mut entries = Vec::new();
        for entry in descriptor
            .get("entries")
            .and_then(Value::as_array)
            .map_or(&[][..], Vec::as_slice)
        {
            let binding = field_u32(entry, "binding")
                .ok_or_else(|| Self::bad(args.index, args.call, "an entry has no binding"))?;
            let resource = entry
                .get("resource")
                .ok_or_else(|| Self::bad(args.index, args.call, "an entry has no resource"))?;
            let resource = if let Some(id) = ref_of(resource) {
                match self.objects.get(&id) {
                    Some(Object::Sampler(sampler)) => wgpu::BindingResource::Sampler(sampler),
                    Some(Object::View(view)) => wgpu::BindingResource::TextureView(view),
                    Some(Object::Buffer(buffer)) => buffer.as_entire_binding(),
                    _ => return Err(Self::missing(args.index, args.call, id)),
                }
            } else {
                let id = field_ref(resource, "buffer").ok_or_else(|| {
                    Self::bad(
                        args.index,
                        args.call,
                        "a resource is neither an object nor a buffer binding",
                    )
                })?;
                wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: self.buffer(args, id)?,
                    offset: resource.get("offset").and_then(Value::as_u64).unwrap_or(0),
                    size: resource
                        .get("size")
                        .and_then(Value::as_u64)
                        .and_then(NonZeroU64::new),
                })
            };
            entries.push(wgpu::BindGroupEntry { binding, resource });
        }
        let label = args.label(0);
        Ok(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: label.as_deref(),
            layout,
            entries: &entries,
        }))
    }

    fn layout_of(
        &self,
        args: &Args<'_>,
        descriptor: &Value,
    ) -> Result<Option<&wgpu::PipelineLayout>, ReplayCallError> {
        match field_ref(descriptor, "layout") {
            None => Ok(None),
            Some(id) => match self.objects.get(&id) {
                Some(Object::PipelineLayout(layout)) => Ok(Some(layout)),
                _ => Err(Self::missing(args.index, args.call, id)),
            },
        }
    }

    fn shader(
        &self,
        args: &Args<'_>,
        stage: &Value,
    ) -> Result<&wgpu::ShaderModule, ReplayCallError> {
        let id = field_ref(stage, "module")
            .ok_or_else(|| Self::bad(args.index, args.call, "a stage has no module"))?;
        match self.objects.get(&id) {
            Some(Object::Shader(module)) => Ok(module),
            _ => Err(Self::missing(args.index, args.call, id)),
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "a render pipeline's descriptor has that many parts"
    )]
    fn create_render_pipeline(
        &self,
        args: &Args<'_>,
    ) -> Result<wgpu::RenderPipeline, ReplayCallError> {
        let descriptor = args.get(0)?;
        let layout = self.layout_of(args, descriptor)?;
        let vertex = descriptor
            .get("vertex")
            .ok_or_else(|| Self::bad(args.index, args.call, "no vertex stage"))?;
        let vertex_module = self.shader(args, vertex)?;
        let vertex_constants = constants_of(vertex).ok_or_else(|| {
            Self::bad(args.index, args.call, "a pipeline constant is not a number")
        })?;
        let vertex_constant_refs: Vec<(&str, f64)> = vertex_constants
            .iter()
            .map(|(k, v)| (k.as_str(), *v))
            .collect();
        let mut attributes: Vec<Vec<wgpu::VertexAttribute>> = Vec::new();
        let mut strides = Vec::new();
        let buffers_json = vertex
            .get("buffers")
            .and_then(Value::as_array)
            .map_or(&[][..], Vec::as_slice);
        for layout in buffers_json {
            if layout.is_null() {
                attributes.push(Vec::new());
                strides.push(None);
                continue;
            }
            let list = layout
                .get("attributes")
                .and_then(Value::as_array)
                .map_or(&[][..], Vec::as_slice)
                .iter()
                .map(|attribute| {
                    Some(wgpu::VertexAttribute {
                        format: attribute.get("format").and_then(enum_of)?,
                        offset: attribute.get("offset").and_then(Value::as_u64).unwrap_or(0),
                        shader_location: field_u32(attribute, "shaderLocation")?,
                    })
                })
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| {
                    Self::bad(args.index, args.call, "a vertex attribute does not parse")
                })?;
            attributes.push(list);
            strides.push(Some((
                layout
                    .get("arrayStride")
                    .and_then(Value::as_u64)
                    .unwrap_or(0),
                layout
                    .get("stepMode")
                    .and_then(enum_of)
                    .unwrap_or(wgpu::VertexStepMode::Vertex),
            )));
        }
        let buffers: Vec<Option<wgpu::VertexBufferLayout<'_>>> = strides
            .iter()
            .zip(&attributes)
            .map(|(stride, attributes)| {
                stride.map(|(array_stride, step_mode)| wgpu::VertexBufferLayout {
                    array_stride,
                    step_mode,
                    attributes,
                })
            })
            .collect();
        let primitive = descriptor.get("primitive");
        let pfield = |key: &str| primitive.and_then(|p| p.get(key));
        let primitive = wgpu::PrimitiveState {
            topology: pfield("topology")
                .and_then(enum_of)
                .unwrap_or(wgpu::PrimitiveTopology::TriangleList),
            strip_index_format: pfield("stripIndexFormat").and_then(enum_of),
            front_face: pfield("frontFace")
                .and_then(enum_of)
                .unwrap_or(wgpu::FrontFace::Ccw),
            cull_mode: pfield("cullMode")
                .and_then(Value::as_str)
                .and_then(|mode| match mode {
                    "front" => Some(wgpu::Face::Front),
                    "back" => Some(wgpu::Face::Back),
                    _ => None,
                }),
            unclipped_depth: pfield("unclippedDepth")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        };
        let depth_stencil = descriptor
            .get("depthStencil")
            .filter(|d| d.is_object())
            .and_then(depth_stencil_of);
        let multisample = descriptor.get("multisample");
        let mfield = |key: &str| multisample.and_then(|m| m.get(key));
        let multisample = wgpu::MultisampleState {
            count: mfield("count")
                .and_then(Value::as_u64)
                .and_then(|v| u32::try_from(v).ok())
                .unwrap_or(1),
            mask: mfield("mask").and_then(Value::as_u64).unwrap_or(u64::MAX),
            alpha_to_coverage_enabled: mfield("alphaToCoverageEnabled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        };
        let fragment = descriptor.get("fragment").filter(|f| f.is_object());
        let fragment_module = match fragment {
            Some(stage) => Some(self.shader(args, stage)?),
            None => None,
        };
        let fragment_constants =
            fragment
                .map_or(Some(Vec::new()), constants_of)
                .ok_or_else(|| {
                    Self::bad(args.index, args.call, "a pipeline constant is not a number")
                })?;
        let fragment_constant_refs: Vec<(&str, f64)> = fragment_constants
            .iter()
            .map(|(k, v)| (k.as_str(), *v))
            .collect();
        let targets: Vec<Option<wgpu::ColorTargetState>> = fragment
            .and_then(|f| f.get("targets"))
            .and_then(Value::as_array)
            .map_or(&[][..], Vec::as_slice)
            .iter()
            .map(colour_target_of)
            .collect();
        let vertex_entry = vertex.get("entryPoint").and_then(Value::as_str);
        let fragment_entry = fragment
            .and_then(|f| f.get("entryPoint"))
            .and_then(Value::as_str);
        let label = args.label(0);
        Ok(self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: label.as_deref(),
                layout,
                vertex: wgpu::VertexState {
                    module: vertex_module,
                    entry_point: vertex_entry,
                    compilation_options: wgpu::PipelineCompilationOptions {
                        constants: &vertex_constant_refs,
                        zero_initialize_workgroup_memory: true,
                    },
                    buffers: &buffers,
                },
                primitive,
                depth_stencil,
                multisample,
                fragment: fragment_module.map(|module| wgpu::FragmentState {
                    module,
                    entry_point: fragment_entry,
                    compilation_options: wgpu::PipelineCompilationOptions {
                        constants: &fragment_constant_refs,
                        zero_initialize_workgroup_memory: true,
                    },
                    targets: &targets,
                }),
                multiview_mask: None,
                cache: None,
            }))
    }

    fn create_compute_pipeline(
        &self,
        args: &Args<'_>,
    ) -> Result<wgpu::ComputePipeline, ReplayCallError> {
        let descriptor = args.get(0)?;
        let layout = self.layout_of(args, descriptor)?;
        let stage = descriptor
            .get("compute")
            .ok_or_else(|| Self::bad(args.index, args.call, "no compute stage"))?;
        let module = self.shader(args, stage)?;
        let constants = constants_of(stage).ok_or_else(|| {
            Self::bad(args.index, args.call, "a pipeline constant is not a number")
        })?;
        let constant_refs: Vec<(&str, f64)> =
            constants.iter().map(|(k, v)| (k.as_str(), *v)).collect();
        let label = args.label(0);
        Ok(self
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: label.as_deref(),
                layout,
                module,
                entry_point: stage.get("entryPoint").and_then(Value::as_str),
                compilation_options: wgpu::PipelineCompilationOptions {
                    constants: &constant_refs,
                    zero_initialize_workgroup_memory: true,
                },
                cache: None,
            }))
    }
}

/// A call's arguments, with what is needed to report a bad one.
#[derive(Debug, Clone, Copy)]
struct Args<'a> {
    capture: &'a Capture,
    values: &'a [Value],
    index: usize,
    call: &'a Call,
}

impl<'a> Args<'a> {
    fn bad(&self, reason: impl Into<String>) -> ReplayCallError {
        Replayer::bad(self.index, self.call, reason)
    }

    fn get(&self, i: usize) -> Result<&'a Value, ReplayCallError> {
        self.values
            .get(i)
            .filter(|value| !is_undefined(value))
            .ok_or_else(|| self.bad(format!("no argument {i}")))
    }

    fn reference(&self, i: usize) -> Result<u64, ReplayCallError> {
        ref_of(self.get(i)?).ok_or_else(|| self.bad(format!("argument {i} is not an object")))
    }

    fn u64_at(&self, i: usize) -> Result<u64, ReplayCallError> {
        self.get(i)?
            .as_u64()
            .ok_or_else(|| self.bad(format!("argument {i} is not a whole number")))
    }

    fn u32_at(&self, i: usize) -> Result<u32, ReplayCallError> {
        u32::try_from(self.u64_at(i)?)
            .map_err(|_| self.bad(format!("argument {i} is out of range")))
    }

    fn f64_at(&self, i: usize) -> Result<f64, ReplayCallError> {
        self.get(i)?
            .as_f64()
            .ok_or_else(|| self.bad(format!("argument {i} is not a number")))
    }

    fn optional_u64(&self, i: usize) -> Option<u64> {
        self.values.get(i).and_then(Value::as_u64)
    }

    fn optional_u32(&self, i: usize) -> Option<u32> {
        self.optional_u64(i).and_then(|v| u32::try_from(v).ok())
    }

    fn optional_i32(&self, i: usize) -> Option<i32> {
        self.values
            .get(i)
            .and_then(Value::as_i64)
            .and_then(|v| i32::try_from(v).ok())
    }

    /// `first..first + count`, refused where it would overflow.
    fn range(&self, first: u32, count: u32) -> Result<std::ops::Range<u32>, ReplayCallError> {
        first
            .checked_add(count)
            .map(|end| first..end)
            .ok_or_else(|| self.bad("a range past the largest index"))
    }

    /// The part of `buffer` from argument `i` (offset) and `i + 1` (size), refused past its end,
    /// where WebGPU reports a validation error and wgpu would panic.
    fn slice<'b>(
        &self,
        buffer: &'b wgpu::Buffer,
        i: usize,
    ) -> Result<wgpu::BufferSlice<'b>, ReplayCallError> {
        let start = self.optional_u64(i).unwrap_or(0);
        let end = match self.optional_u64(i + 1) {
            Some(size) => start.checked_add(size),
            None => Some(buffer.size()),
        };
        match end {
            Some(end) if start <= end && end <= buffer.size() => Ok(buffer.slice(start..end)),
            _ => Err(self.bad("a buffer range past the buffer's end")),
        }
    }

    fn str_at(&self, i: usize) -> Option<&'a str> {
        self.values.get(i).and_then(Value::as_str)
    }

    fn field_str(&self, i: usize, key: &str) -> Result<&'a str, ReplayCallError> {
        self.get(i)?
            .get(key)
            .and_then(Value::as_str)
            .ok_or_else(|| self.bad(format!("argument {i} has no {key}")))
    }

    fn label(&self, i: usize) -> Option<String> {
        self.values
            .get(i)?
            .get("label")?
            .as_str()
            .map(str::to_owned)
    }

    /// A blob argument's bytes.
    fn blob(&self, i: usize) -> Result<&'a [u8], ReplayCallError> {
        let index = self
            .get(i)?
            .get("$blob")
            .and_then(Value::as_u64)
            .and_then(|v| usize::try_from(v).ok())
            .ok_or_else(|| self.bad(format!("argument {i} is not a blob")))?;
        self.capture
            .blob(index)
            .ok_or_else(|| self.bad(format!("blob {index} is missing")))
    }

    /// `setBindGroup`'s dynamic offsets: a list, or a `Uint32Array` blob with a start and a length.
    fn dynamic_offsets(&self) -> Result<Vec<u32>, ReplayCallError> {
        let Some(value) = self.values.get(2).filter(|v| !is_undefined(v)) else {
            return Ok(Vec::new());
        };
        if let Some(list) = value.as_array() {
            return list
                .iter()
                .map(|v| {
                    v.as_u64()
                        .and_then(|v| u32::try_from(v).ok())
                        .ok_or_else(|| self.bad("a dynamic offset is not a u32"))
                })
                .collect();
        }
        let bytes = self.blob(2)?;
        let all: Vec<u32> = bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|chunk| u32::from_le_bytes(*chunk))
            .collect();
        let start = self
            .optional_u64(3)
            .and_then(|v| usize::try_from(v).ok())
            .unwrap_or(0);
        let length = self
            .optional_u64(4)
            .and_then(|v| usize::try_from(v).ok())
            .unwrap_or(all.len().saturating_sub(start));
        start
            .checked_add(length)
            .and_then(|end| all.get(start..end))
            .map(<[u32]>::to_vec)
            .ok_or_else(|| self.bad("the dynamic offsets lie outside their array"))
    }
}

fn is_undefined(value: &Value) -> bool {
    value.get("$undefined").and_then(Value::as_bool) == Some(true)
}

fn ref_of(value: &Value) -> Option<u64> {
    value.get("$ref")?.as_u64()
}

fn field_ref(value: &Value, key: &str) -> Option<u64> {
    ref_of(value.get(key)?)
}

fn field_u32(value: &Value, key: &str) -> Option<u32> {
    value.get(key)?.as_u64().and_then(|v| u32::try_from(v).ok())
}

/// An enum of wgpu's from WebGPU's name for it.
fn enum_of<T: DeserializeOwned>(value: &Value) -> Option<T> {
    // An unknown name is `None`, which each caller turns into an error or WebGPU's default.
    T::deserialize(value).ok()
}

/// The view formats a canvas's texture allows besides its own: its sRGB form, if it has one.
///
/// The engine configures every canvas so (`view/engine/webgpu/view.ts`, `srgbViewFormat`) and
/// draws its display pass through the sRGB view; the capture logs a canvas's size and format but
/// not its configuration.
#[must_use]
pub(crate) fn canvas_view_formats(format: wgpu::TextureFormat) -> Vec<wgpu::TextureFormat> {
    let srgb = format.add_srgb_suffix();
    if srgb == format {
        Vec::new()
    } else {
        vec![srgb]
    }
}

/// A texture format from WebGPU's name for it.
fn texture_format(name: &str) -> Option<wgpu::TextureFormat> {
    enum_of(&Value::String(name.to_owned()))
}

/// A canvas format wgpu does not know.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("the capture's canvas format {format} is not one wgpu knows")]
pub struct UnknownFormatError {
    /// The format's WebGPU name.
    pub format: String,
}

fn extent(value: &Value) -> wgpu::Extent3d {
    if let Some(list) = value.as_array() {
        let at = |i: usize, default: u32| {
            list.get(i)
                .and_then(Value::as_u64)
                .and_then(|v| u32::try_from(v).ok())
                .unwrap_or(default)
        };
        return wgpu::Extent3d {
            width: at(0, 1),
            height: at(1, 1),
            depth_or_array_layers: at(2, 1),
        };
    }
    wgpu::Extent3d {
        width: field_u32(value, "width").unwrap_or(1),
        height: field_u32(value, "height").unwrap_or(1),
        depth_or_array_layers: field_u32(value, "depthOrArrayLayers").unwrap_or(1),
    }
}

fn origin(value: Option<&Value>) -> wgpu::Origin3d {
    let Some(value) = value else {
        return wgpu::Origin3d::ZERO;
    };
    if let Some(list) = value.as_array() {
        let at = |i: usize| {
            list.get(i)
                .and_then(Value::as_u64)
                .and_then(|v| u32::try_from(v).ok())
                .unwrap_or(0)
        };
        return wgpu::Origin3d {
            x: at(0),
            y: at(1),
            z: at(2),
        };
    }
    wgpu::Origin3d {
        x: field_u32(value, "x").unwrap_or(0),
        y: field_u32(value, "y").unwrap_or(0),
        z: field_u32(value, "z").unwrap_or(0),
    }
}

fn buffer_layout(value: &Value) -> wgpu::TexelCopyBufferLayout {
    wgpu::TexelCopyBufferLayout {
        offset: value.get("offset").and_then(Value::as_u64).unwrap_or(0),
        bytes_per_row: field_u32(value, "bytesPerRow"),
        rows_per_image: field_u32(value, "rowsPerImage"),
    }
}

fn texel_copy_texture<'a>(
    texture: &'a wgpu::Texture,
    value: &Value,
) -> wgpu::TexelCopyTextureInfo<'a> {
    wgpu::TexelCopyTextureInfo {
        texture,
        mip_level: field_u32(value, "mipLevel").unwrap_or(0),
        origin: origin(value.get("origin")),
        aspect: value
            .get("aspect")
            .and_then(enum_of)
            .unwrap_or(wgpu::TextureAspect::All),
    }
}

fn colour_of(value: Option<&Value>) -> wgpu::Color {
    let Some(value) = value else {
        return wgpu::Color::TRANSPARENT;
    };
    if let Some(list) = value.as_array() {
        let at = |i: usize| list.get(i).and_then(Value::as_f64).unwrap_or(0.0);
        return wgpu::Color {
            r: at(0),
            g: at(1),
            b: at(2),
            a: at(3),
        };
    }
    let at = |key: &str| value.get(key).and_then(Value::as_f64).unwrap_or(0.0);
    wgpu::Color {
        r: at("r"),
        g: at("g"),
        b: at("b"),
        a: at("a"),
    }
}

fn store_of(value: Option<&Value>) -> wgpu::StoreOp {
    if value.and_then(Value::as_str) == Some("discard") {
        wgpu::StoreOp::Discard
    } else {
        wgpu::StoreOp::Store
    }
}

/// A stage's pipeline-overridable constants, or `None` if one is not a number.
fn constants_of(stage: &Value) -> Option<Vec<(String, f64)>> {
    stage
        .get("constants")
        .and_then(Value::as_object)
        .map_or(Some(Vec::new()), |map| {
            map.iter()
                .map(|(k, v)| Some((k.clone(), v.as_f64()?)))
                .collect()
        })
}

fn binding_type(entry: &Value) -> Option<wgpu::BindingType> {
    if let Some(buffer) = entry.get("buffer").filter(|b| b.is_object()) {
        let ty = match buffer
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("uniform")
        {
            "uniform" => wgpu::BufferBindingType::Uniform,
            "storage" => wgpu::BufferBindingType::Storage { read_only: false },
            "read-only-storage" => wgpu::BufferBindingType::Storage { read_only: true },
            _ => return None,
        };
        return Some(wgpu::BindingType::Buffer {
            ty,
            has_dynamic_offset: buffer
                .get("hasDynamicOffset")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            min_binding_size: buffer
                .get("minBindingSize")
                .and_then(Value::as_u64)
                .and_then(NonZeroU64::new),
        });
    }
    if let Some(sampler) = entry.get("sampler").filter(|s| s.is_object()) {
        return Some(wgpu::BindingType::Sampler(
            sampler
                .get("type")
                .and_then(enum_of)
                .unwrap_or(wgpu::SamplerBindingType::Filtering),
        ));
    }
    if let Some(texture) = entry.get("texture").filter(|t| t.is_object()) {
        let sample_type = match texture
            .get("sampleType")
            .and_then(Value::as_str)
            .unwrap_or("float")
        {
            "unfilterable-float" => wgpu::TextureSampleType::Float { filterable: false },
            "depth" => wgpu::TextureSampleType::Depth,
            "sint" => wgpu::TextureSampleType::Sint,
            "uint" => wgpu::TextureSampleType::Uint,
            "float" => wgpu::TextureSampleType::Float { filterable: true },
            _ => return None,
        };
        return Some(wgpu::BindingType::Texture {
            sample_type,
            view_dimension: texture
                .get("viewDimension")
                .and_then(enum_of)
                .unwrap_or(wgpu::TextureViewDimension::D2),
            multisampled: texture
                .get("multisampled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        });
    }
    if let Some(storage) = entry.get("storageTexture").filter(|s| s.is_object()) {
        return Some(wgpu::BindingType::StorageTexture {
            access: storage
                .get("access")
                .and_then(enum_of)
                .unwrap_or(wgpu::StorageTextureAccess::WriteOnly),
            format: storage.get("format").and_then(enum_of)?,
            view_dimension: storage
                .get("viewDimension")
                .and_then(enum_of)
                .unwrap_or(wgpu::TextureViewDimension::D2),
        });
    }
    None
}

fn depth_stencil_of(value: &Value) -> Option<wgpu::DepthStencilState> {
    let face = |key: &str| {
        let face = value.get(key);
        let field = |name: &str| face.and_then(|f| f.get(name));
        wgpu::StencilFaceState {
            compare: field("compare")
                .and_then(enum_of)
                .unwrap_or(wgpu::CompareFunction::Always),
            fail_op: field("failOp")
                .and_then(enum_of)
                .unwrap_or(wgpu::StencilOperation::Keep),
            depth_fail_op: field("depthFailOp")
                .and_then(enum_of)
                .unwrap_or(wgpu::StencilOperation::Keep),
            pass_op: field("passOp")
                .and_then(enum_of)
                .unwrap_or(wgpu::StencilOperation::Keep),
        }
    };
    #[expect(
        clippy::cast_possible_truncation,
        reason = "depth-bias slopes and clamps are small numbers, exact enough in f32"
    )]
    let float = |key: &str| value.get(key).and_then(Value::as_f64).unwrap_or(0.0) as f32;
    Some(wgpu::DepthStencilState {
        format: value.get("format").and_then(enum_of)?,
        depth_write_enabled: value.get("depthWriteEnabled").and_then(Value::as_bool),
        depth_compare: value.get("depthCompare").and_then(enum_of),
        stencil: wgpu::StencilState {
            front: face("stencilFront"),
            back: face("stencilBack"),
            read_mask: field_u32(value, "stencilReadMask").unwrap_or(u32::MAX),
            write_mask: field_u32(value, "stencilWriteMask").unwrap_or(u32::MAX),
        },
        bias: wgpu::DepthBiasState {
            constant: value
                .get("depthBias")
                .and_then(Value::as_i64)
                .and_then(|v| i32::try_from(v).ok())
                .unwrap_or(0),
            slope_scale: float("depthBiasSlopeScale"),
            clamp: float("depthBiasClamp"),
        },
    })
}

fn colour_target_of(value: &Value) -> Option<wgpu::ColorTargetState> {
    if value.is_null() {
        return None;
    }
    let component = |blend: &Value, key: &str| {
        let part = blend.get(key);
        let field = |name: &str| part.and_then(|p| p.get(name));
        wgpu::BlendComponent {
            src_factor: field("srcFactor")
                .and_then(enum_of)
                .unwrap_or(wgpu::BlendFactor::One),
            dst_factor: field("dstFactor")
                .and_then(enum_of)
                .unwrap_or(wgpu::BlendFactor::Zero),
            operation: field("operation")
                .and_then(enum_of)
                .unwrap_or(wgpu::BlendOperation::Add),
        }
    };
    Some(wgpu::ColorTargetState {
        format: value.get("format").and_then(enum_of)?,
        blend: value
            .get("blend")
            .filter(|b| b.is_object())
            .map(|blend| wgpu::BlendState {
                color: component(blend, "color"),
                alpha: component(blend, "alpha"),
            }),
        write_mask: wgpu::ColorWrites::from_bits_truncate(
            field_u32(value, "writeMask").unwrap_or(0xF),
        ),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// A capture of one call, `op` on the queue with `args`, and a blob of `blob`.
    fn capture_of(args: &Value, blob: &[u8]) -> Capture {
        let text = json!({
            "schema": "hyperion.gpu-capture",
            "version": 1,
            "adapter": {},
            "features": [],
            "limits": {},
            "surfaces": [],
            "skipped": [],
            "spanStart": 0,
            "frames": [],
            "blobs": [{ "offset": 0, "length": blob.len() }],
            "calls": [{ "target": QUEUE_ID, "op": "test", "args": args }],
        })
        .to_string();
        Capture::parse(&text, blob.to_vec()).expect("a valid capture")
    }

    fn with_args<T>(args: &Value, blob: &[u8], check: impl FnOnce(&Args<'_>) -> T) -> T {
        let capture = capture_of(args, blob);
        let call = &capture.calls()[0];
        check(&Args {
            capture: &capture,
            values: &call.args,
            index: 0,
            call,
        })
    }

    #[test]
    fn a_canvas_allows_its_srgb_view_as_the_engine_configures_it() {
        assert_eq!(
            canvas_view_formats(wgpu::TextureFormat::Rgba8Unorm),
            vec![wgpu::TextureFormat::Rgba8UnormSrgb]
        );
        assert_eq!(
            canvas_view_formats(wgpu::TextureFormat::Bgra8Unorm),
            vec![wgpu::TextureFormat::Bgra8UnormSrgb]
        );
        assert!(canvas_view_formats(wgpu::TextureFormat::Bgra8UnormSrgb).is_empty());
        assert!(canvas_view_formats(wgpu::TextureFormat::Rgba16Float).is_empty());
    }

    #[test]
    fn extents_read_both_webgpu_forms_with_their_defaults() {
        let from_list = extent(&json!([64, 32]));
        let from_dict = extent(&json!({ "width": 64, "height": 32, "depthOrArrayLayers": 6 }));
        assert_eq!(
            (
                from_list.width,
                from_list.height,
                from_list.depth_or_array_layers
            ),
            (64, 32, 1)
        );
        assert_eq!(from_dict.depth_or_array_layers, 6);
    }

    #[test]
    fn binding_types_map_webgpu_names_and_refuse_unknown_ones() {
        assert!(matches!(
            binding_type(&json!({ "buffer": { "type": "read-only-storage" } })),
            Some(wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                ..
            })
        ));
        assert!(matches!(
            binding_type(&json!({ "buffer": {} })),
            Some(wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                ..
            })
        ));
        assert!(binding_type(&json!({ "buffer": { "type": "mystery" } })).is_none());
        assert!(binding_type(&json!({ "texture": { "sampleType": "mystery" } })).is_none());
        assert!(matches!(
            binding_type(
                &json!({ "texture": { "sampleType": "depth", "viewDimension": "2d-array" } })
            ),
            Some(wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Depth,
                view_dimension: wgpu::TextureViewDimension::D2Array,
                multisampled: false,
            })
        ));
    }

    #[test]
    fn draw_ranges_are_refused_where_they_would_overflow() {
        with_args(&json!([]), &[], |args| {
            assert_eq!(args.range(4, 3).expect("in range"), 4..7);
            assert!(matches!(
                args.range(u32::MAX, 2),
                Err(ReplayCallError::BadArguments { .. })
            ));
        });
    }

    #[test]
    fn dynamic_offsets_come_from_a_list_or_a_typed_array_slice() {
        with_args(&json!([0, { "$ref": 2 }, [256, 512]]), &[], |args| {
            assert_eq!(args.dynamic_offsets().expect("a list"), [256, 512]);
        });
        let bytes: Vec<u8> = [1u32, 2, 3, 4]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let blob = json!({ "$blob": 0, "$type": "Uint32Array" });
        with_args(&json!([0, { "$ref": 2 }, blob, 1, 2]), &bytes, |args| {
            assert_eq!(args.dynamic_offsets().expect("a slice"), [2, 3]);
        });
        with_args(&json!([0, { "$ref": 2 }, blob, 3, 2]), &bytes, |args| {
            assert!(matches!(
                args.dynamic_offsets(),
                Err(ReplayCallError::BadArguments { .. })
            ));
        });
    }

    #[test]
    fn an_absent_argument_is_webgpus_undefined() {
        with_args(&json!([3, { "$undefined": true }]), &[], |args| {
            assert_eq!(args.u32_at(0).expect("a count"), 3);
            assert!(args.get(1).is_err());
            assert_eq!(args.optional_u32(1), None);
        });
    }
}
