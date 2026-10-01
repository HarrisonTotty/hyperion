//! The flush-to-zero probes fail on a thread whose floating-point mode flushes subnormals, and the
//! CPU pool then refuses to start (the rendering brainstorm's "Testing", the flush-to-zero probes;
//! plan R04, task T9.b and design note 15).
//!
//! Turning flushing on needs `unsafe`, which the workspace forbids, so the mode is set from outside,
//! the way a native library would set it: the parent test builds a shared object with `cc` whose
//! constructor ORs FTZ (MXCSR bit 15, `0x8000`), DAZ (bit 6, `0x0040`) or both into MXCSR (Intel
//! SDM vol. 1 §10.2.3), and re-runs this test binary with it in `LD_PRELOAD`, naming only the child
//! test. The constructor runs on the child's main thread before `main`, and every thread created
//! after it, the test harness's among them, inherits the mode. A fourth shared object is an empty
//! file built with `-mdaz-ftz`, which links in `crtfastmath.o`: the fast-math startup code that
//! older toolchains linked into every `-ffast-math` shared object, and the exact hazard the
//! brainstorm describes.
//!
//! Rust code run under a preloaded flushing mode is formally undefined behaviour (`core::arch`'s
//! documentation of MXCSR), so this shows what the compiled probe does in that state, not what the
//! language guarantees; that undefinedness is why the server refuses to generate.
//!
//! Linux on x86-64 with glibc only (a static musl binary ignores `LD_PRELOAD`). An `AArch64` twin would set the FPCR's FZ in the same way, and waits until
//! something runs on Arm.
#![cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]

use std::env;
use std::fs;
use std::io::ErrorKind;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;

use hyperion_server::compute::{CpuPool, FlushProbe, StartPoolError, probe_flush_to_zero};

/// Set in the child's environment to the mode it runs under; unset, the child test does nothing.
const MARKER: &str = "HYPERION_FLUSH_TO_ZERO_CHILD";

/// The child test's name, as `--exact` matches it.
const CHILD: &str = "under_a_preloaded_mode_the_probes_fail_and_the_pool_refuses_to_start";

/// The modes the parent preloads: the marker's value, and what the probe must report.
const MODES: [(&str, FlushProbe); 4] = [
    ("ftz", FlushProbe::FLUSHES_OUTPUTS),
    ("daz", FlushProbe::FLUSHES_INPUTS),
    ("ftz-daz", FlushProbe::FLUSHES_BOTH),
    ("crtfastmath", FlushProbe::FLUSHES_BOTH),
];

fn expected_probe(mode: &str) -> FlushProbe {
    MODES
        .iter()
        .find_map(|&(name, probe)| (name == mode).then_some(probe))
        .unwrap_or_else(|| panic!("unknown mode {mode:?} in {MARKER}"))
}

#[test]
fn under_a_preloaded_mode_the_probes_fail_and_the_pool_refuses_to_start() {
    let Ok(mode) = env::var(MARKER) else {
        // Run directly, not by the parent: there is no preloaded mode to check.
        return;
    };
    let expected = expected_probe(&mode);
    assert_eq!(probe_flush_to_zero(), expected, "on the test's thread");
    let spawned = thread::spawn(probe_flush_to_zero).join().unwrap();
    assert_eq!(spawned, expected, "on a spawned thread");
    let n = |value| NonZeroUsize::new(value).unwrap();
    match CpuPool::new(n(2), n(4), n(4)) {
        Err(StartPoolError::FloatingPointMode { worker, probe }) => {
            assert_eq!((worker, probe), (0, expected));
        }
        other => panic!("the pool started or failed otherwise under {mode}: {other:?}"),
    }
}

/// C source whose constructor ORs `mask` into MXCSR when the library loads.
fn constructor_source(mask: u32) -> String {
    format!(
        "#include <xmmintrin.h>\n\
         __attribute__((constructor)) static void hyperion_set_flush_mode(void) {{\n\
         \x20   _mm_setcsr(_mm_getcsr() | {mask:#06x}u);\n\
         }}\n"
    )
}

/// Builds `source` into a shared object in `dir` with `cc -shared -fPIC` and `flags`.
fn build_shared_object(dir: &Path, name: &str, source: &str, flags: &[&str]) -> PathBuf {
    let source_path = dir.join(format!("{name}.c"));
    let object = dir.join(format!("lib{name}.so"));
    fs::write(&source_path, source).unwrap();
    let output = Command::new("cc")
        .args(["-shared", "-fPIC"])
        .args(flags)
        .arg("-o")
        .arg(&object)
        .arg(&source_path)
        .output()
        .unwrap_or_else(|error| match error.kind() {
            ErrorKind::NotFound => panic!(
                "this test needs a C compiler named `cc` (rustc's default linker on \
                 x86_64-unknown-linux-gnu) to build the preloaded library, and none was found"
            ),
            _ => panic!("failed to run `cc`: {error}"),
        });
    assert!(
        output.status.success(),
        "cc failed building {name} (`-mdaz-ftz` needs GCC 13 or later, or Clang 19 or later): {}",
        String::from_utf8_lossy(&output.stderr)
    );
    object
}

#[test]
fn the_probes_fail_on_threads_whose_mode_flushes_subnormals() {
    let dir = tempfile::tempdir().unwrap();
    let objects = [
        build_shared_object(dir.path(), "ftz", &constructor_source(0x8000), &["-O2"]),
        build_shared_object(dir.path(), "daz", &constructor_source(0x0040), &["-O2"]),
        build_shared_object(dir.path(), "ftz-daz", &constructor_source(0x8040), &["-O2"]),
        build_shared_object(dir.path(), "crtfastmath", "", &["-mdaz-ftz"]),
    ];
    let test_binary = env::current_exe().unwrap();
    for ((mode, _), object) in MODES.iter().zip(&objects) {
        // The dynamic loader splits `LD_PRELOAD` at spaces and colons.
        let preload = object
            .to_str()
            .expect("the temporary directory's path is UTF-8");
        assert!(
            !preload.contains([' ', ':']),
            "the preloaded library's path {preload:?} holds a space or a colon, which LD_PRELOAD \
             cannot carry; set TMPDIR to a plain path"
        );
        let output = Command::new(&test_binary)
            .args(["--exact", CHILD, "--nocapture", "--test-threads=1"])
            .env(MARKER, mode)
            .env("LD_PRELOAD", preload)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success(),
            "the child failed under {mode}:\n{stdout}\n{stderr}"
        );
        assert!(
            stdout.contains("test result: ok. 1 passed"),
            "the child ran no test under {mode}:\n{stdout}"
        );
    }
}
