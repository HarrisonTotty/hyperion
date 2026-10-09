//! The surface crate's registry of domain tags: the `surface.*` tags, and `selftest.surface.*`.
//!
//! One of three registries (plan R04, Design note 4), beside `hyperion_base::rng::tags` and the
//! sim's `rng::tags`, which asserts all three disjoint. Every tag the surface crate opens is
//! declared here: the `surface.` prefix is reserved for this registry (R11's `surface.scatter` is
//! reserved by R09 and not yet declared), and `selftest.surface.` for its tags of scope
//! `SelfTest`, such as R05's provisional test planet's. Tags the server derives in the sim,
//! `body.surface` and `body.surface.detail`, are the sim's, not this registry's.
//!
//! The rules of every registry hold: a tag is never renamed or removed, and a new property group
//! gets a new tag.
//!
//! # Surface tags and their keys
//!
//! A `surface.coarse.*` tag has scope
//! [`SurfaceCoarse`](hyperion_base::rng::TagScope::SurfaceCoarse) and is opened only from a body's
//! surface seed ([`SurfaceSeed::stream`](hyperion_base::rng::SurfaceSeed::stream)), by the
//! server's coarse pass in the sim; every other `surface.*` tag has scope
//! [`SurfaceDetail`](hyperion_base::rng::TagScope::SurfaceDetail) and is opened only from its
//! detail seed ([`DetailSeed::stream`](hyperion_base::rng::DetailSeed::stream)), by the local
//! synthesis here. The body is in the seed, so the key names a cell of its cube sphere
//! ([`ObjectKey::surface_cell`](hyperion_base::rng::ObjectKey::surface_cell)) or an item
//! ([`ObjectKey::surface_item`](hyperion_base::rng::ObjectKey::surface_item)), and since a surface
//! item's word can equal a low surface cell's, each tag is used with the one key form this table
//! gives and never the other (the rendering plans' R09, Provides and Design note 2). The tests
//! hold the table to the registry, and every call site to the table: each `stream` call, as a
//! method or by path, that names a surface tag's constant names its documented key constructor in
//! the same call. A tag passed in as a parameter or held under another name escapes that check,
//! so a call site names both.
//!
//! | Tag                      | Scope           | Key            | Draws                                                          |
//! | ------------------------ | --------------- | -------------- | -------------------------------------------------------------- |
//! | `surface.coarse.plates`  | `SurfaceCoarse` | `surface_item` | plate count, seeds, Euler poles, crust type per plate          |
//! | `surface.coarse.warp`    | `SurfaceCoarse` | `surface_cell` | the low-frequency boundary warp's lattice                      |
//! | `surface.coarse.relief`  | `SurfaceCoarse` | `surface_cell` | coarse landform noise, volcanic provinces                      |
//! | `surface.coarse.crater`  | `SurfaceCoarse` | `surface_item` | craters of the boundary diameter and wider: count, place, age, morphology |
//! | `surface.coarse.erosion` | `SurfaceCoarse` | `surface_cell` | random receivers and the multigrid's jitter                    |
//! | `surface.relief`         | `SurfaceDetail` | `surface_item` | structural noise: a 3D lattice corner's word, as R05's [`noise`](crate::noise) keys it |
//! | `surface.channel`        | `SurfaceDetail` | `surface_cell` | Dendry key points per cell and level                           |
//! | `surface.crater`         | `SurfaceDetail` | `surface_cell` | craters below the boundary diameter, per octave cell           |

hyperion_base::domain_tags! {
    // Plan R05: the terrain geometry and the descent spike.

    /// The provisional test planet's lattice-corner gradients and octave offsets (R05, Design
    /// note 13), outside every universe; R09's real height function replaces it.
    TEST_PLANET: SelfTest = "selftest.surface.test_planet";

    // Plan R09: the surface generator (R09.T1.a), each with the key form of the table above.

    /// The coarse pass's plates (R09, Design note 6), keyed by `surface_item(k)` for plate k: the
    /// plate count, each plate's seed point, Euler pole and crust type.
    SURFACE_COARSE_PLATES: SurfaceCoarse = "surface.coarse.plates";

    /// The coarse pass's low-frequency warp of the plate boundaries (R09, Design note 6), keyed by
    /// `surface_cell` of the warp lattice's cells.
    SURFACE_COARSE_WARP: SurfaceCoarse = "surface.coarse.warp";

    /// The coarse pass's landform noise and volcanic provinces (R09, Design notes 6 and 7), keyed
    /// by `surface_cell` of the coarse cells.
    SURFACE_COARSE_RELIEF: SurfaceCoarse = "surface.coarse.relief";

    /// The coarse pass's craters of the boundary diameter D_b and wider (R09, Design note 10),
    /// keyed by `surface_item(k)` for crater slot k: their count, places, ages and morphologies.
    SURFACE_COARSE_CRATER: SurfaceCoarse = "surface.coarse.crater";

    /// The coarse pass's erosion (R09, Design note 9), keyed by `surface_cell` of the coarse
    /// cells: each cell's random receiver, and the multigrid upsample's jitter.
    SURFACE_COARSE_EROSION: SurfaceCoarse = "surface.coarse.erosion";

    /// The synthesis's structural octaves (R09, Design note 13), keyed by `surface_item` of a 3D
    /// lattice corner's word, i << 32 | j of the corner's 32-bit two's complements, with the draw
    /// number octave << 32 | k, as R05's [`noise`](crate::noise) keys its corners.
    SURFACE_RELIEF: SurfaceDetail = "surface.relief";

    /// The synthesis's channel network (R09, Design note 13), keyed by `surface_cell` of a
    /// Dendry level's cells: each cell's jittered key point.
    SURFACE_CHANNEL: SurfaceDetail = "surface.channel";

    /// The synthesis's craters below the boundary diameter (R09, Design note 13), keyed by
    /// `surface_cell` of an octave's quadtree cells: each cell's count, places and diameters.
    SURFACE_CRATER: SurfaceDetail = "surface.crater";
}

#[cfg(test)]
mod tests {
    use hyperion_base::rng::TagScope;

    use super::*;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    /// This file's text, whose module documentation holds the table of key forms.
    const SOURCE: &str = include_str!("tags.rs");

    /// The two key forms a surface tag may be documented with.
    const KEY_FORMS: [&str; 2] = ["surface_cell", "surface_item"];

    /// Every name begins `surface.`, or `selftest.surface.` for a tag of `SelfTest` scope.
    #[test]
    fn surface_tags_carry_the_surface_prefix() {
        for tag in ALL {
            let prefix = if tag.scope() == TagScope::SelfTest {
                "selftest.surface."
            } else {
                "surface."
            };
            assert!(
                tag.name().starts_with(prefix),
                "{} lacks {prefix}",
                tag.name()
            );
        }
    }

    /// The rows of the table of key forms in the module documentation: tag name, scope and key
    /// form, their backticks removed.
    fn documented_rows() -> Vec<[&'static str; 3]> {
        SOURCE
            .lines()
            .filter(|line| line.starts_with("//! | `"))
            .map(|line| {
                let cells: Vec<&str> = line
                    .trim_start_matches("//!")
                    .split('|')
                    .map(|cell| cell.trim().trim_matches('`'))
                    .collect();
                // A leading empty cell, then tag, scope, key and draws.
                [cells[1], cells[2], cells[3]]
            })
            .collect()
    }

    /// The constant a tag is declared as: its name upper-cased, each dot an underscore.
    fn constant_of(name: &str) -> String {
        name.to_uppercase().replace('.', "_")
    }

    /// Each tag of the registry but its self-test ones has exactly one row, whose scope is the
    /// tag's and whose key is one of the two forms; every row is a registered tag; the coarse
    /// pass's tags, and only they, have scope `SurfaceCoarse`; and each is declared as the
    /// constant its name gives, which the call-site check below looks for.
    #[test]
    fn every_surface_tag_has_one_documented_key_form() {
        let rows = documented_rows();
        assert!(!rows.is_empty(), "the module documentation has no table");
        for [name, scope, key] in &rows {
            let tag = ALL
                .iter()
                .find(|tag| tag.name() == *name)
                .unwrap_or_else(|| panic!("{name} is documented but not registered"));
            assert_eq!(format!("{:?}", tag.scope()), *scope, "{name}");
            assert!(
                KEY_FORMS.contains(key),
                "{name}: {key} is not a surface key form"
            );
            let declaration = format!("{}: {scope} = \"{name}\";", constant_of(name));
            assert!(
                SOURCE.contains(&declaration),
                "{name} is not declared as {declaration}"
            );
        }
        for tag in ALL {
            let documented = rows.iter().filter(|[name, ..]| *name == tag.name()).count();
            match tag.scope() {
                TagScope::SelfTest => assert_eq!(documented, 0, "{}", tag.name()),
                TagScope::SurfaceCoarse => {
                    assert_eq!(documented, 1, "{}", tag.name());
                    assert!(tag.name().starts_with("surface.coarse."), "{}", tag.name());
                }
                TagScope::SurfaceDetail => {
                    assert_eq!(documented, 1, "{}", tag.name());
                    assert!(!tag.name().starts_with("surface.coarse."), "{}", tag.name());
                }
                TagScope::Galaxy
                | TagScope::Cell
                | TagScope::Feature
                | TagScope::System
                | TagScope::Body
                | TagScope::Event => panic!("{} has no surface scope", tag.name()),
            }
        }
    }

    /// The constant and documented key form of every tag in the table.
    fn documented_forms() -> Vec<(String, &'static str)> {
        documented_rows()
            .into_iter()
            .map(|[name, _, key]| (constant_of(name), key))
            .collect()
    }

    /// Whether `word` occurs in `text` as a whole identifier, not inside a longer one.
    fn names(text: &str, word: &str) -> bool {
        let is_ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
        text.match_indices(word).any(|(at, _)| {
            let before = text[..at].chars().next_back();
            let after = text[at + word.len()..].chars().next();
            !before.is_some_and(is_ident) && !after.is_some_and(is_ident)
        })
    }

    /// The text of the argument lists of every call `head` begins, such as `.stream(`, up to its
    /// matching parenthesis, each with the byte offset of its head.
    fn calls<'t>(text: &'t str, head: &str) -> Vec<(usize, &'t str)> {
        text.match_indices(head)
            .map(|(at, _)| {
                let start = at + head.len();
                let mut depth = 0_u32;
                let mut end = text.len();
                for (offset, c) in text[start..].char_indices() {
                    match c {
                        '(' => depth += 1,
                        ')' if depth == 0 => {
                            end = start + offset;
                            break;
                        }
                        ')' => depth -= 1,
                        _ => {}
                    }
                }
                (at, &text[start..end])
            })
            .collect()
    }

    /// What breaks the rule in one source text: a `stream` call, as a method or by path, naming a
    /// surface tag without its documented key constructor, or with the other form's; and any
    /// `Stream::open` naming one, which panics.
    fn call_site_faults(text: &str, forms: &[(String, &str)]) -> Vec<String> {
        let line = |at: usize| text[..at].matches('\n').count() + 1;
        let mut faults = Vec::new();
        let streams = calls(text, ".stream(")
            .into_iter()
            .chain(calls(text, "::stream("));
        for (at, args) in streams {
            for (constant, form) in forms {
                if !names(args, constant) {
                    continue;
                }
                let used: Vec<&str> = KEY_FORMS
                    .into_iter()
                    .filter(|f| args.contains(&format!("{f}(")))
                    .collect();
                if used != [*form] {
                    faults.push(format!(
                        "line {}: {constant} is keyed by {form}, but its call names {used:?}",
                        line(at)
                    ));
                }
            }
        }
        for (at, args) in calls(text, "Stream::open(") {
            for (constant, _) in forms {
                if names(args, constant) {
                    faults.push(format!(
                        "line {}: {constant} opened by Stream::open, not by its seed",
                        line(at)
                    ));
                }
            }
        }
        faults
    }

    /// The check itself, on call sites written for it: the right form passes, a wrong, missing or
    /// doubled form fails, as does `Stream::open`, and a longer constant is not mistaken for one.
    #[test]
    fn the_call_site_check_tells_a_documented_key_from_any_other() {
        let forms = documented_forms();
        let good = [
            "seed.stream(tags::SURFACE_RELIEF, ObjectKey::surface_item(corner))",
            "detail\n    .stream(\n        SURFACE_CHANNEL,\n        ObjectKey::surface_cell(f, l, i, j, n)?,\n    )",
            "seed.stream(tags::SURFACE_RELIEF_EXTRA, key)",
            "seed.scope(SURFACE_CRATER)",
            "seed.stream(tags::SURFACE_COARSE_PLATES, ObjectKey::surface_item(cells(k)))",
            "DetailSeed::stream(seed, SURFACE_CRATER, ObjectKey::surface_cell(0, 9, 3, 4, 1)?)",
        ];
        for text in good {
            assert_eq!(
                call_site_faults(text, &forms),
                Vec::<String>::new(),
                "{text}"
            );
        }
        let bad = [
            "seed.stream(tags::SURFACE_RELIEF, ObjectKey::surface_cell(0, 0, 0, 0, 0)?)",
            "seed.stream(tags::SURFACE_CRATER, key)",
            "seed.stream(SURFACE_CRATER, ObjectKey::surface_item(surface_cell(1)))",
            "SurfaceSeed::stream(seed, tags::SURFACE_COARSE_WARP, ObjectKey::surface_item(0))",
            "Stream::open(seed, tags::SURFACE_COARSE_WARP, ObjectKey::surface_item(0))",
        ];
        for text in bad {
            assert_eq!(call_site_faults(text, &forms).len(), 1, "{text}");
        }
    }

    /// What reads the workspace's sources, which only a native or WASI run can (plan R04, Design
    /// note 12).
    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    mod native_only {
        use std::path::{Path, PathBuf};

        use super::{call_site_faults, documented_forms};

        /// Every `.rs` file under `dir`, recursively, in name order.
        fn rust_files(dir: &Path, files: &mut Vec<PathBuf>) {
            let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
                .unwrap_or_else(|e| panic!("cannot list {}: {e}", dir.display()))
                .map(|entry| entry.expect("a directory entry is readable").path())
                .collect();
            entries.sort();
            for path in entries {
                if path.is_dir() {
                    rust_files(&path, files);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    files.push(path);
                }
            }
        }

        /// Every call site in the workspace's crates, the sim's coarse pass and this crate's
        /// synthesis among them, keys each surface tag by its documented form. This registry's
        /// own file, whose tests write call sites that break the rule on purpose, is skipped.
        #[test]
        fn each_surface_tag_s_call_sites_use_its_documented_key_form() {
            let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
            let crates = manifest.parent().expect("the crate sits in crates/");
            let this = manifest.join("src").join("tags.rs");
            let mut files = Vec::new();
            rust_files(crates, &mut files);
            let forms = documented_forms();
            let mut faults = Vec::new();
            for path in files.iter().filter(|path| **path != this) {
                let text = std::fs::read_to_string(path)
                    .unwrap_or_else(|e| panic!("{} is readable: {e}", path.display()));
                for fault in call_site_faults(&text, &forms) {
                    faults.push(format!("{}: {fault}", path.display()));
                }
            }
            assert!(
                files.iter().any(|p| p.ends_with("hyperion-sim/src/lib.rs")),
                "the sim's sources were not read from {}",
                crates.display()
            );
            assert_eq!(faults, Vec::<String>::new());
        }
    }
}
