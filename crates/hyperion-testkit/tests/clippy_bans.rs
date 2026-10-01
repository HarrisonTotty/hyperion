//! Holds every `clippy.toml` of the workspace to one ban list.
//!
//! Clippy takes the nearest `clippy.toml` walking up from a crate's manifest and does not merge
//! them, so each determinism crate carries a self-contained file of its own, and a ban added to
//! one file and forgotten in another would silently not bind there. These tests read the files as
//! text and assert that each holds its set: the root's and every crate's ban the platform
//! transcendentals, `mul_add` and the `algebraic_*` methods; every crate's file also bans reading
//! float bits, which the root's omits on purpose (the testkit prints bits and the server compares
//! them); and every crate other than those the root's file binds has a file of its own, so a new
//! determinism crate cannot fall back to the root's by accident (plan R04, Design note 9).
//!
//! The tests read files, so they are compiled out on `wasm32-unknown-unknown`, inside a module
//! named `native_only` (plan R04, Design note 12).

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
mod native_only {
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};

    /// The float methods that go through the determinism crates' `math` instead, on `f64` and
    /// `f32` alike: the platform's versions differ between systems, and `mul_add` calls the
    /// platform's `fma` where the CPU has none.
    const TRANSCENDENTALS: [&str; 27] = [
        "sin", "cos", "tan", "sin_cos", "asin", "acos", "atan", "atan2", "sinh", "cosh", "tanh",
        "asinh", "acosh", "atanh", "exp", "exp2", "exp_m1", "ln", "log", "log2", "log10", "ln_1p",
        "powf", "powi", "cbrt", "hypot", "mul_add",
    ];

    /// The float methods whose results the compiler may fuse or reassociate, so that "the same
    /// inputs may produce different results even within a single program run" (Rust's
    /// documentation, "Algebraic operators", rustc 1.98.1).
    const ALGEBRAIC: [&str; 5] = [
        "algebraic_add",
        "algebraic_sub",
        "algebraic_mul",
        "algebraic_div",
        "algebraic_rem",
    ];

    /// The float-bit conversions, banned in every crate's own file so that no float is hashed.
    const FLOAT_BITS: [&str; 4] = ["to_bits", "to_le_bytes", "to_be_bytes", "to_ne_bytes"];

    /// The crates the root `clippy.toml` binds; every other crate needs a file of its own.
    const ROOT_CONFIGURED: [&str; 3] = ["hyperion-protocol", "hyperion-server", "hyperion-testkit"];

    fn on_both_widths(methods: &[&str]) -> Vec<String> {
        ["f64", "f32"]
            .iter()
            .flat_map(|ty| methods.iter().map(move |m| format!("{ty}::{m}")))
            .collect()
    }

    /// The list every `clippy.toml` bans.
    fn shared_list() -> Vec<String> {
        let mut list = on_both_widths(&TRANSCENDENTALS);
        list.extend(on_both_widths(&ALGEBRAIC));
        list
    }

    /// The paths a `clippy.toml`'s text bans: every `path = "…"` inside its `disallowed-methods`
    /// array and outside a comment (no ban's path or reason contains a `#`).
    fn banned_paths(text: &str) -> BTreeSet<String> {
        text.lines()
            .skip_while(|line| !line.starts_with("disallowed-methods = ["))
            .take_while(|line| !line.starts_with(']'))
            .map(|line| line.split('#').next().unwrap_or_default())
            .flat_map(|line| line.split("path = \"").skip(1))
            .filter_map(|rest| rest.split('"').next())
            .map(str::to_owned)
            .collect()
    }

    /// The required paths the text does not ban, in the order given.
    fn missing(text: &str, required: &[String]) -> Vec<String> {
        let banned = banned_paths(text);
        required
            .iter()
            .filter(|path| !banned.contains(*path))
            .cloned()
            .collect()
    }

    /// The crates, by directory name, that neither have a `clippy.toml` of their own nor are
    /// bound by the root's.
    fn crates_without_own_file(crates: &[(String, bool)]) -> Vec<String> {
        crates
            .iter()
            .filter(|(name, has_file)| !has_file && !ROOT_CONFIGURED.contains(&name.as_str()))
            .map(|(name, _)| name.clone())
            .collect()
    }

    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("the testkit sits two levels below the workspace root")
    }

    /// Every crate under `crates/`, by directory name, and whether it has its own `clippy.toml`.
    fn workspace_crates() -> Vec<(String, bool)> {
        let dir = workspace_root().join("crates");
        let mut crates: Vec<(String, bool)> = std::fs::read_dir(&dir)
            .expect("crates/ is readable")
            .map(|entry| entry.expect("crates/ entry is readable").path())
            .filter(|path| path.join("Cargo.toml").is_file())
            .map(|path| {
                let name = path
                    .file_name()
                    .expect("a crate directory has a name")
                    .to_string_lossy()
                    .into_owned();
                (name, path.join("clippy.toml").is_file())
            })
            .collect();
        crates.sort();
        crates
    }

    /// Each crate's own `clippy.toml`, as (workspace-relative path, text).
    fn crate_files() -> Vec<(String, String)> {
        workspace_crates()
            .into_iter()
            .filter(|(_, has_file)| *has_file)
            .map(|(name, _)| {
                let rel = format!("crates/{name}/clippy.toml");
                let text = std::fs::read_to_string(workspace_root().join(&rel))
                    .unwrap_or_else(|e| panic!("{rel} is readable: {e}"));
                (rel, text)
            })
            .collect()
    }

    /// The root's `clippy.toml` and every crate's, as (workspace-relative path, text).
    fn every_file() -> Vec<(String, String)> {
        let root = std::fs::read_to_string(workspace_root().join("clippy.toml"))
            .expect("the root clippy.toml is readable");
        let mut files = vec![("clippy.toml".to_owned(), root)];
        files.extend(crate_files());
        files
    }

    /// Each required entry, deleted from a copy of the text, is reported missing, so the check
    /// would catch its loss from the real file.
    fn assert_each_deletion_is_caught(rel: &str, text: &str, required: &[String]) {
        for path in required {
            let entry = format!("path = \"{path}\"");
            let copy = text
                .lines()
                .filter(|line| !line.contains(&entry))
                .collect::<Vec<_>>()
                .join("\n");
            assert_eq!(
                missing(&copy, required),
                vec![path.clone()],
                "deleting {path} from a copy of {rel} is caught"
            );
        }
    }

    #[test]
    fn every_clippy_toml_bans_the_shared_list() {
        let shared = shared_list();
        let files = every_file();
        assert!(
            files.len() >= 3,
            "the root's, the sim's and the fitting crate's files at least: {:?}",
            files.iter().map(|(rel, _)| rel).collect::<Vec<_>>()
        );
        for (rel, text) in &files {
            assert_eq!(
                missing(text, &shared),
                Vec::<String>::new(),
                "{rel} bans the shared list"
            );
            assert_each_deletion_is_caught(rel, text, &shared);
        }
    }

    #[test]
    fn crate_files_also_ban_float_bits() {
        let bits = on_both_widths(&FLOAT_BITS);
        for (rel, text) in crate_files() {
            assert_eq!(
                missing(&text, &bits),
                Vec::<String>::new(),
                "{rel} bans float bits"
            );
            assert_each_deletion_is_caught(&rel, &text, &bits);
        }
    }

    #[test]
    fn every_determinism_crate_has_its_own_clippy_toml() {
        let crates = workspace_crates();
        assert_eq!(
            crates_without_own_file(&crates),
            Vec::<String>::new(),
            "every crate but {ROOT_CONFIGURED:?} has a clippy.toml of its own"
        );
        for name in ["hyperion-sim", "hyperion-fit"] {
            assert!(
                crates.contains(&(name.to_owned(), true)),
                "{name} has a clippy.toml of its own"
            );
        }
        // A sixth crate added without a file of its own is caught.
        let mut added = crates.clone();
        added.push(("hyperion-new".to_owned(), false));
        assert_eq!(
            crates_without_own_file(&added),
            vec!["hyperion-new".to_owned()]
        );
        // A crate's own file removed is caught.
        let removed: Vec<(String, bool)> = crates
            .iter()
            .map(|(name, has_file)| (name.clone(), *has_file && name != "hyperion-sim"))
            .collect();
        assert_eq!(
            crates_without_own_file(&removed),
            vec!["hyperion-sim".to_owned()]
        );
    }

    #[test]
    fn a_commented_out_ban_does_not_count() {
        let text = "disallowed-methods = [\n    # { path = \"f64::sin\", reason = \"x\" },\n    \
                    { path = \"f64::cos\", reason = \"x\" }, # { path = \"f64::tan\" }\n]\n\
                    disallowed-types = [\n    { path = \"f64::exp\", reason = \"x\" },\n]\n";
        assert_eq!(banned_paths(text), BTreeSet::from(["f64::cos".to_owned()]));
    }

    /// The crates the client ships as WebAssembly, which ban relaxed SIMD (Design note 10).
    const CLIENT_CRATES: [&str; 2] = ["hyperion-base", "hyperion-surface"];

    /// The crates whose sources may carry no `target_feature` attribute: the client's two and the
    /// sim, which links the surface crate.
    const NO_TARGET_FEATURE_CRATES: [&str; 3] =
        ["hyperion-base", "hyperion-surface", "hyperion-sim"];

    /// The relaxed-SIMD intrinsics of `core::arch::wasm32` on rustc 1.98.1: safe functions, so
    /// callable without `unsafe`, whose NaN, signed-zero and rounding results the engine decides.
    const RELAXED_INTRINSICS: [&str; 20] = [
        "i8x16_relaxed_swizzle",
        "i32x4_relaxed_trunc_f32x4",
        "u32x4_relaxed_trunc_f32x4",
        "i32x4_relaxed_trunc_f64x2_zero",
        "u32x4_relaxed_trunc_f64x2_zero",
        "f32x4_relaxed_madd",
        "f32x4_relaxed_nmadd",
        "f64x2_relaxed_madd",
        "f64x2_relaxed_nmadd",
        "i8x16_relaxed_laneselect",
        "i16x8_relaxed_laneselect",
        "i32x4_relaxed_laneselect",
        "i64x2_relaxed_laneselect",
        "f32x4_relaxed_min",
        "f32x4_relaxed_max",
        "f64x2_relaxed_min",
        "f64x2_relaxed_max",
        "i16x8_relaxed_q15mulr",
        "i16x8_relaxed_dot_i8x16_i7x16",
        "i32x4_relaxed_dot_i8x16_i7x16_add",
    ];

    /// The crate-level guard, with every space removed.
    const GUARD: &str = "#[cfg(target_feature=\"relaxed-simd\")]";

    fn read(rel: &str) -> String {
        std::fs::read_to_string(workspace_root().join(rel))
            .unwrap_or_else(|e| panic!("{rel} is readable: {e}"))
    }

    fn relaxed_list() -> Vec<String> {
        RELAXED_INTRINSICS
            .iter()
            .map(|name| format!("core::arch::wasm32::{name}"))
            .collect()
    }

    /// Whether a source text enables a target feature for a function, or names relaxed SIMD in a
    /// `target_feature` other than the crate-level guard: `target_feature(enable …)`, inside a
    /// `cfg_attr` as well, or `target_feature = "…relaxed…"`. Whitespace, line breaks included, is
    /// ignored, so an attribute split over lines is still seen.
    fn enables_a_target_feature(text: &str) -> bool {
        let squashed: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        let squashed = squashed.replace(GUARD, "");
        squashed.contains("target_feature(enable")
            || squashed.split("target_feature=\"").skip(1).any(|rest| {
                rest.split('"')
                    .next()
                    .is_some_and(|v| v.contains("relaxed"))
            })
    }

    /// Every `.rs` file under `dir`, recursively.
    fn rust_files(dir: &Path, files: &mut Vec<PathBuf>) {
        let entries =
            std::fs::read_dir(dir).unwrap_or_else(|e| panic!("cannot list {}: {e}", dir.display()));
        for entry in entries {
            let path = entry.expect("a directory entry is readable").path();
            if path.is_dir() {
                rust_files(&path, files);
            } else if path.extension().is_some_and(|e| e == "rs") {
                files.push(path);
            }
        }
    }

    #[test]
    fn relaxed_intrinsics_are_banned() {
        let relaxed = relaxed_list();
        for name in CLIENT_CRATES {
            let rel = format!("crates/{name}/clippy.toml");
            let text = read(&rel);
            assert_eq!(
                missing(&text, &relaxed),
                Vec::<String>::new(),
                "{rel} bans them"
            );
            for path in &relaxed {
                let entry = format!("path = \"{path}\"");
                let line = text
                    .lines()
                    .find(|line| line.contains(&entry))
                    .expect("each entry is on a line");
                assert!(
                    line.contains("allow-invalid = true"),
                    "{rel}: {path} needs allow-invalid, since the host cannot resolve it"
                );
            }
            assert_each_deletion_is_caught(&rel, &text, &relaxed);
        }
    }

    #[test]
    fn both_client_crates_refuse_relaxed_simd() {
        for name in CLIENT_CRATES {
            let rel = format!("crates/{name}/src/lib.rs");
            let squashed: String = read(&rel).chars().filter(|c| !c.is_whitespace()).collect();
            let Some((_, after)) = squashed.split_once(GUARD) else {
                panic!("{rel} carries no relaxed-SIMD guard");
            };
            assert!(
                after.starts_with("compile_error!(\"relaxedSIMDisbannedin")
                    && after
                        .split_once(')')
                        .is_some_and(|(message, _)| message.contains(name)),
                "{rel}'s guard is a compile_error! naming {name}"
            );
        }
    }

    #[test]
    fn no_target_feature_attributes() {
        let mut offending = Vec::new();
        for name in NO_TARGET_FEATURE_CRATES {
            let mut files = Vec::new();
            rust_files(&workspace_root().join("crates").join(name), &mut files);
            assert!(!files.is_empty(), "{name} has sources");
            for path in files {
                let text = std::fs::read_to_string(&path)
                    .unwrap_or_else(|e| panic!("{} is readable: {e}", path.display()));
                if enables_a_target_feature(&text) {
                    offending.push(path.display().to_string());
                }
            }
        }
        assert_eq!(
            offending,
            Vec::<String>::new(),
            "files enabling a target feature"
        );
        // The check itself: it catches the attribute, inside `cfg_attr` and over lines, and passes
        // on the guard.
        for bad in [
            "#[target_feature(enable = \"relaxed-simd\")]\nfn f() {}",
            "#[target_feature(enable = \"simd128\")]\nfn f() {}",
            "#[cfg_attr(target_arch = \"wasm32\", target_feature(enable = \"relaxed-simd\"))]",
            "#[target_feature(\n    enable = \"relaxed-simd\",\n)]",
            "#[cfg(target_feature = \"relaxed-simd\")]\n#[cfg_attr(x, target_feature = \"relaxed-simd\")]",
        ] {
            assert!(enables_a_target_feature(bad), "{bad:?} is caught");
        }
        for good in [
            "#[cfg(target_feature = \"relaxed-simd\")]\ncompile_error!(\"relaxed SIMD is banned\");",
            "#[cfg(target_feature = \"simd128\")]\nfn f() {}",
            "// The `target_feature` attributes are rejected by a source test.",
        ] {
            assert!(!enables_a_target_feature(good), "{good:?} passes");
        }
    }
}
