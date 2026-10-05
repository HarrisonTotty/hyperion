//! The capture reader and the naga report against the checked-in capture in
//! `tests/fixtures/small`: a triangle drawn into a canvas in two frames (the first also clearing
//! it through its sRGB view), a buffer written by the snapshot, a multisampled texture the
//! snapshot skipped with a view of it that nothing uses, and one module that is invalid on
//! purpose.

use std::path::{Path, PathBuf};

use gpu_replay::capture::{Capture, DEVICE_ID, QUEUE_ID, ReadCaptureError, Surface};
use gpu_replay::validate::validate_capture;

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/small")
}

#[test]
fn reads_the_calls_span_frames_and_surfaces() {
    let capture = Capture::read(&fixture()).expect("the fixture is a capture");
    assert_eq!(capture.calls().len(), 28);
    assert_eq!(capture.span_start(), 7);
    assert_eq!(capture.frames(), &[7, 19]);
    assert_eq!(
        capture.surfaces(),
        &[Surface {
            id: 9,
            width: 64,
            height: 32,
            format: "bgra8unorm".to_owned()
        }]
    );
    assert_eq!(capture.skipped()[0].reason, "multisampled");
    assert_eq!(capture.features(), &["timestamp-query".to_owned()]);
    assert_eq!(capture.meta()["setting"], "low");
    assert_eq!(capture.meta()["passRows"]["triangle"], "terrain");
    let first = &capture.calls()[0];
    assert_eq!(
        (first.target, first.op.as_str(), first.result),
        (DEVICE_ID, "createShaderModule", Some(2))
    );
    let submit = capture.calls().last().expect("calls");
    assert_eq!((submit.target, submit.op.as_str()), (QUEUE_ID, "submit"));
}

#[test]
fn the_setup_leaves_out_the_view_nothing_uses() {
    let capture = Capture::read(&fixture()).expect("the fixture is a capture");
    assert_eq!(capture.calls()[6].op, "createView");
    assert_eq!(capture.setup_calls(), &[0, 1, 2, 3, 4, 5]);
}

#[test]
fn reads_the_bytes_of_a_blob() {
    let capture = Capture::read(&fixture()).expect("the fixture is a capture");
    let bytes = capture.blob(0).expect("blob 0");
    let floats: Vec<f32> = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| f32::from_le_bytes(*chunk))
        .collect();
    assert_eq!(floats, [1.0, 2.0, 3.0, 4.0]);
    assert_eq!(capture.blob_count(), 1);
}

#[test]
fn lists_the_shader_modules() {
    let capture = Capture::read(&fixture()).expect("the fixture is a capture");
    let labels: Vec<&str> = capture
        .shader_modules()
        .map(|module| module.label)
        .collect();
    assert_eq!(labels, ["triangle", "broken on purpose"]);
}

#[test]
fn the_naga_report_lists_the_invalid_module() {
    let capture = Capture::read(&fixture()).expect("the fixture is a capture");
    let report = validate_capture(&capture);
    assert_eq!(report.modules, 2);
    assert_eq!(report.rejected.len(), 1);
    assert_eq!(report.rejected[0].id, 3);
    assert_eq!(report.rejected[0].label, "broken on purpose");
    assert!(report.to_text().contains("module 3 (broken on purpose)"));
}

#[test]
fn a_missing_capture_names_its_file() {
    let error = Capture::read(Path::new("/nonexistent/capture")).expect_err("no such capture");
    assert!(matches!(error, ReadCaptureError::Io { .. }));
    assert!(error.to_string().contains("capture.json"), "{error}");
}
