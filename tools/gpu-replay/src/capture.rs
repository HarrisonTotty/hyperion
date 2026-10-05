//! Reading a capture written by the client's capture shim (`view/spike/capture.ts`, R05.T15.a).
//!
//! A capture is a directory holding `capture.json`, the call log, and `capture.bin`, the bytes
//! its blobs name. Each call is the object it was made on (by ID: 0 the device, 1 its queue), the
//! method's name, its arguments as JSON (`{"$ref": id}` for an object, `{"$blob": n, "$type": …}`
//! for bytes, `{"$undefined": true}` for an absent argument) and the ID of what it returned. The
//! log needs no schema per call: a replayer maps each method to its own API.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Deserialize;
use serde_json::Value;

/// The schema name every capture carries.
pub const CAPTURE_SCHEMA: &str = "hyperion.gpu-capture";
/// The schema version this reader reads.
pub const CAPTURE_VERSION: u32 = 1;
/// The call log's file name in a capture's directory.
pub const CAPTURE_JSON: &str = "capture.json";
/// The blobs' file name in a capture's directory.
pub const CAPTURE_DATA: &str = "capture.bin";
/// The methods whose results a replay makes before the span only when something it replays
/// uses them ([`Capture::setup_calls`]): what the engine makes many of and drops.
pub const DROPPED_BEFORE_THE_SPAN: [&str; 3] =
    ["createView", "createBindGroup", "getBindGroupLayout"];
/// The device's ID in every capture.
pub const DEVICE_ID: u64 = 0;
/// The queue's ID in every capture.
pub const QUEUE_ID: u64 = 1;

/// Why a capture could not be read.
#[derive(Debug, thiserror::Error)]
pub enum ReadCaptureError {
    /// A file of the capture could not be read.
    #[error("cannot read {path}")]
    Io {
        /// The file.
        path: PathBuf,
        /// The cause.
        #[source]
        source: std::io::Error,
    },
    /// The call log is not JSON of the capture's shape.
    #[error("{path} is not a capture")]
    Json {
        /// The file.
        path: PathBuf,
        /// The cause.
        #[source]
        source: serde_json::Error,
    },
    /// The capture is of another schema or version.
    #[error(
        "the capture is {schema} version {version}, not {CAPTURE_SCHEMA} version {CAPTURE_VERSION}"
    )]
    Schema {
        /// The schema it names.
        schema: String,
        /// The version it names.
        version: u32,
    },
    /// A blob lies outside the bytes.
    #[error("blob {index} lies outside the capture's {length} bytes")]
    Blob {
        /// The blob's index.
        index: usize,
        /// The bytes the capture holds.
        length: usize,
    },
}

/// A canvas the capture drew to.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Surface {
    /// Its ID in the call log.
    pub id: u64,
    /// Its size in pixels.
    pub width: u32,
    /// Its size in pixels.
    pub height: u32,
    /// Its texture format, as WebGPU names it.
    pub format: String,
}

/// A resource the capture's snapshot could not read.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Skipped {
    /// Its ID in the call log.
    pub id: u64,
    /// Why.
    pub reason: String,
}

/// Where a blob's bytes lie in `capture.bin`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
struct BlobSpan {
    offset: usize,
    length: usize,
}

/// One logged call.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Call {
    /// The ID of the object it was made on.
    pub target: u64,
    /// The method's name, as WebGPU's JavaScript API names it.
    pub op: String,
    /// Its arguments as the capture wrote them.
    pub args: Vec<Value>,
    /// The ID of what it returned, if it returned an object.
    pub result: Option<u64>,
}

impl Call {
    /// Pushes the IDs of the objects the call names: its target and every `$ref` in its
    /// arguments.
    fn named_objects(&self, out: &mut Vec<u64>) {
        out.push(self.target);
        for arg in &self.args {
            push_refs(arg, out);
        }
    }
}

/// Pushes the ID of every `{"$ref": id}` in `value`.
fn push_refs(value: &Value, out: &mut Vec<u64>) {
    match value {
        Value::Object(map) => {
            if let Some(id) = map.get("$ref").and_then(Value::as_u64) {
                out.push(id);
            }
            for item in map.values() {
                push_refs(item, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                push_refs(item, out);
            }
        }
        _ => {}
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CaptureFile {
    schema: String,
    version: u32,
    adapter: BTreeMap<String, String>,
    features: Vec<String>,
    limits: BTreeMap<String, f64>,
    surfaces: Vec<Surface>,
    skipped: Vec<Skipped>,
    span_start: usize,
    frames: Vec<usize>,
    blobs: Vec<BlobSpan>,
    calls: Vec<Call>,
    /// What the client knew of the run: its `setting`, `seed` and the row of each pass label
    /// (`passRows`).
    #[serde(default)]
    meta: serde_json::Map<String, Value>,
    /// What the capture could not log faithfully.
    #[serde(default)]
    problems: Vec<String>,
}

/// A WGSL module the capture made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShaderModule<'a> {
    /// Its ID in the call log.
    pub id: u64,
    /// Its label, or empty.
    pub label: &'a str,
    /// Its WGSL source.
    pub code: &'a str,
}

/// A capture read from its directory.
#[derive(Debug)]
pub struct Capture {
    file: CaptureFile,
    data: Vec<u8>,
    /// [`Capture::setup_calls`], found once.
    setup: OnceLock<Vec<usize>>,
}

impl Capture {
    /// Reads the capture in `dir`.
    ///
    /// # Errors
    ///
    /// When a file cannot be read, the log is not a capture of this schema and version, or a blob
    /// lies outside the bytes.
    pub fn read(dir: &Path) -> Result<Self, ReadCaptureError> {
        let json_path = dir.join(CAPTURE_JSON);
        let data_path = dir.join(CAPTURE_DATA);
        let text = fs::read_to_string(&json_path).map_err(|source| ReadCaptureError::Io {
            path: json_path.clone(),
            source,
        })?;
        let data = fs::read(&data_path).map_err(|source| ReadCaptureError::Io {
            path: data_path,
            source,
        })?;
        Self::parse(&text, data).map_err(|error| match error {
            ParseError::Json(source) => ReadCaptureError::Json {
                path: json_path,
                source,
            },
            ParseError::Read(error) => error,
        })
    }

    pub(crate) fn parse(text: &str, data: Vec<u8>) -> Result<Self, ParseError> {
        let file: CaptureFile = serde_json::from_str(text).map_err(ParseError::Json)?;
        if file.schema != CAPTURE_SCHEMA || file.version != CAPTURE_VERSION {
            return Err(ParseError::Read(ReadCaptureError::Schema {
                schema: file.schema,
                version: file.version,
            }));
        }
        for (index, span) in file.blobs.iter().enumerate() {
            if span
                .offset
                .checked_add(span.length)
                .is_none_or(|end| end > data.len())
            {
                return Err(ParseError::Read(ReadCaptureError::Blob {
                    index,
                    length: data.len(),
                }));
            }
        }
        Ok(Self {
            file,
            data,
            setup: OnceLock::new(),
        })
    }

    /// Every call, in order.
    #[must_use]
    pub fn calls(&self) -> &[Call] {
        &self.file.calls
    }

    /// The index in [`Capture::calls`] where the captured span starts, after the snapshot.
    #[must_use]
    pub fn span_start(&self) -> usize {
        self.file.span_start
    }

    /// The indices in [`Capture::calls`] where each captured frame starts.
    #[must_use]
    pub fn frames(&self) -> &[usize] {
        &self.file.frames
    }

    /// The canvases drawn to.
    #[must_use]
    pub fn surfaces(&self) -> &[Surface] {
        &self.file.surfaces
    }

    /// The resources the snapshot could not read.
    #[must_use]
    pub fn skipped(&self) -> &[Skipped] {
        &self.file.skipped
    }

    /// The capturing adapter's description, by field.
    #[must_use]
    pub fn adapter(&self) -> &BTreeMap<String, String> {
        &self.file.adapter
    }

    /// The device's features.
    #[must_use]
    pub fn features(&self) -> &[String] {
        &self.file.features
    }

    /// The device's limits, by WebGPU's names.
    #[must_use]
    pub fn limits(&self) -> &BTreeMap<String, f64> {
        &self.file.limits
    }

    /// A blob's bytes, or `None` for an index the capture lacks.
    #[must_use]
    pub fn blob(&self, index: usize) -> Option<&[u8]> {
        let span = self.file.blobs.get(index)?;
        self.data.get(span.offset..span.offset + span.length)
    }

    /// What the client recorded of the run: `setting`, `seed`, `passRows`.
    #[must_use]
    pub fn meta(&self) -> &serde_json::Map<String, Value> {
        &self.file.meta
    }

    /// What the capture could not log faithfully; empty for a capture a replay can trust.
    #[must_use]
    pub fn problems(&self) -> &[String] {
        &self.file.problems
    }

    /// The number of blobs.
    #[must_use]
    pub fn blob_count(&self) -> usize {
        self.file.blobs.len()
    }

    /// The indices of the calls before the span that a replay makes: every one, less the
    /// views, bind groups and pipelines' bind-group layouts ([`DROPPED_BEFORE_THE_SPAN`]) that
    /// neither the span nor anything else kept uses, directly or through what it was made from.
    ///
    /// The log keeps every creation before the span, but the engine drops most of the views and
    /// bind groups it makes (each frame's), which the browser frees. The first capture of the
    /// descent, 1,085 s in, held 781,049 views, 180,260 bind groups and 180,239 read-back layouts
    /// made before its span, of which it kept 303 calls in all; a replay that made them all could
    /// submit nothing (R05.T15, 2026-10-04).
    #[must_use]
    pub fn setup_calls(&self) -> &[usize] {
        self.setup.get_or_init(|| self.find_setup_calls())
    }

    fn find_setup_calls(&self) -> Vec<usize> {
        let calls = &self.file.calls;
        let (setup, span) = calls.split_at(self.file.span_start.min(calls.len()));
        let droppable = |call: &Call| DROPPED_BEFORE_THE_SPAN.contains(&call.op.as_str());
        let mut made: HashMap<u64, &Call> = HashMap::with_capacity(setup.len());
        made.extend(setup.iter().filter_map(|call| Some((call.result?, call))));
        let mut pending = Vec::new();
        for call in span
            .iter()
            .chain(setup.iter().filter(|call| !droppable(call)))
        {
            call.named_objects(&mut pending);
        }
        let mut needed = HashSet::with_capacity(pending.len());
        while let Some(id) = pending.pop() {
            if needed.insert(id)
                && let Some(call) = made.get(&id)
            {
                call.named_objects(&mut pending);
            }
        }
        setup
            .iter()
            .enumerate()
            .filter(|(_, call)| {
                !droppable(call) || call.result.is_some_and(|id| needed.contains(&id))
            })
            .map(|(index, _)| index)
            .collect()
    }

    /// Every WGSL module the capture made, in order.
    pub fn shader_modules(&self) -> impl Iterator<Item = ShaderModule<'_>> {
        self.file.calls.iter().filter_map(|call| {
            if call.target != DEVICE_ID || call.op != "createShaderModule" {
                return None;
            }
            let descriptor = call.args.first()?;
            Some(ShaderModule {
                id: call.result?,
                label: descriptor
                    .get("label")
                    .and_then(Value::as_str)
                    .unwrap_or(""),
                code: descriptor.get("code")?.as_str()?,
            })
        })
    }
}

/// [`Capture::parse`]'s errors, before the file's path is known.
#[derive(Debug)]
pub(crate) enum ParseError {
    Json(serde_json::Error),
    Read(ReadCaptureError),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal(schema: &str, blobs: &str) -> String {
        format!(
            r#"{{"schema":"{schema}","version":1,"adapter":{{}},"features":[],"limits":{{}},
            "surfaces":[],"skipped":[],"spanStart":0,"frames":[],"blobs":{blobs},"calls":[]}}"#
        )
    }

    #[test]
    fn refuses_another_schema() {
        let error = Capture::parse(&minimal("other", "[]"), Vec::new()).expect_err("wrong schema");
        assert!(matches!(
            error,
            ParseError::Read(ReadCaptureError::Schema { .. })
        ));
    }

    #[test]
    fn refuses_a_blob_outside_the_bytes() {
        let text = minimal(CAPTURE_SCHEMA, r#"[{"offset":2,"length":4}]"#);
        let error = Capture::parse(&text, vec![0; 5]).expect_err("blob past the end");
        assert!(matches!(
            error,
            ParseError::Read(ReadCaptureError::Blob {
                index: 0,
                length: 5
            })
        ));
    }

    /// A capture whose span starts at call `span_start` of `calls`, with one four-byte blob.
    fn capture_of(calls: &str, span_start: usize) -> Capture {
        let text = format!(
            r#"{{"schema":"{CAPTURE_SCHEMA}","version":1,"adapter":{{}},"features":[],"limits":{{}},
            "surfaces":[],"skipped":[],"spanStart":{span_start},"frames":[{span_start}],
            "blobs":[{{"offset":0,"length":4}}],"calls":{calls}}}"#
        );
        Capture::parse(&text, vec![0; 4]).expect("a valid capture")
    }

    #[test]
    fn the_setup_drops_the_views_and_bind_groups_nothing_uses() {
        // Before the span: two buffers, a texture with three views, a pipeline whose layout is
        // read back twice, a bind group over each of the first two views, writes and a
        // destruction. The span uses the first bind group; a kept pipeline layout uses the
        // first read-back layout and the third view is used by nothing.
        let calls = r#"[
            {"target":0,"op":"createBuffer","args":[{"size":16,"usage":72}],"result":3},
            {"target":0,"op":"createBuffer","args":[{"size":16,"usage":72}],"result":4},
            {"target":0,"op":"createTexture","args":[{"size":[4,4],"format":"rgba8unorm","usage":4}],"result":5},
            {"target":5,"op":"createView","args":[{"label":"used"}],"result":6},
            {"target":5,"op":"createView","args":[{"label":"dropped with its bind group"}],"result":7},
            {"target":5,"op":"createView","args":[{"label":"never used"}],"result":15},
            {"target":0,"op":"createComputePipeline","args":[{"layout":"auto","compute":{"module":{"$ref":2}}}],"result":12},
            {"target":12,"op":"getBindGroupLayout","args":[0],"result":13},
            {"target":12,"op":"getBindGroupLayout","args":[0],"result":14},
            {"target":0,"op":"createPipelineLayout","args":[{"bindGroupLayouts":[{"$ref":13}]}],"result":16},
            {"target":0,"op":"createBindGroup","args":[{"layout":{"$ref":14},"entries":[{"binding":0,"resource":{"$ref":6}},{"binding":1,"resource":{"buffer":{"$ref":3}}}]}],"result":8},
            {"target":0,"op":"createBindGroup","args":[{"layout":{"$ref":14},"entries":[{"binding":0,"resource":{"$ref":7}},{"binding":1,"resource":{"buffer":{"$ref":4}}}]}],"result":9},
            {"target":1,"op":"writeBuffer","args":[{"$ref":3},0,{"$blob":0,"$type":"Uint8Array"},0,4]},
            {"target":1,"op":"writeBuffer","args":[{"$ref":4},0,{"$blob":0,"$type":"Uint8Array"},0,4]},
            {"target":4,"op":"destroy","args":[]},
            {"target":0,"op":"createCommandEncoder","args":[{}],"result":10},
            {"target":10,"op":"beginComputePass","args":[{}],"result":11},
            {"target":11,"op":"setBindGroup","args":[0,{"$ref":8}]}
        ]"#;
        let capture = capture_of(calls, 15);
        assert_eq!(
            capture.setup_calls(),
            &[0, 1, 2, 3, 6, 7, 8, 9, 10, 12, 13, 14]
        );
    }

    #[test]
    fn calls_other_than_views_and_bind_groups_are_kept_when_unused() {
        let calls = r#"[
            {"target":0,"op":"createBuffer","args":[{"size":16,"usage":72}],"result":2},
            {"target":0,"op":"createShaderModule","args":[{"code":"unused"}],"result":3},
            {"target":1,"op":"writeBuffer","args":[{"$ref":2},0,{"$blob":0,"$type":"Uint8Array"},0,4]},
            {"target":1,"op":"submit","args":[[]]}
        ]"#;
        assert_eq!(capture_of(calls, 3).setup_calls(), &[0, 1, 2]);
    }

    #[test]
    fn slices_a_blob() {
        let text = minimal(CAPTURE_SCHEMA, r#"[{"offset":1,"length":3}]"#);
        let capture = Capture::parse(&text, vec![9, 1, 2, 3, 9]).expect("a valid capture");
        assert_eq!(capture.blob(0), Some(&[1, 2, 3][..]));
        assert_eq!(capture.blob(1), None);
    }
}
