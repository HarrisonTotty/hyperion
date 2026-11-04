//! The pins harness against real files, in Cargo's per-target temporary directory.
//!
//! It reads and writes files, so it runs natively and on wasip1 only (plan R04, Design note 12);
//! the tests that run threads, natively alone.

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
mod native_only {
    use std::path::{Path, PathBuf};

    use hyperion_testkit::golden::Mode;
    use hyperion_testkit::pins::{HEADER, PinFile, Pins, Tolerance, Value};

    /// A fresh fake crate directory for one test.
    fn scratch(test: &str) -> PathBuf {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join("pins-harness")
            .join(test);
        if dir.exists() {
            std::fs::remove_dir_all(&dir).expect("scratch directory is removable");
        }
        dir
    }

    fn pins_file(dir: &Path, name: &str) -> PathBuf {
        dir.join("tests/pins").join(format!("{name}.pins"))
    }

    fn read(dir: &Path, name: &str) -> String {
        std::fs::read_to_string(pins_file(dir, name)).expect("the pins file exists")
    }

    /// Writes the pins file `name` with `lines` after the header, as given.
    fn write_text(dir: &Path, name: &str, lines: &[&str]) {
        let path = pins_file(dir, name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut text = format!("{HEADER}\n");
        for line in lines {
            text.push_str(line);
            text.push('\n');
        }
        std::fs::write(path, text).unwrap();
    }

    /// Writes the pins file `name` with `lines`, which must be canonical.
    fn write(dir: &Path, name: &str, lines: &[&str]) {
        write_text(dir, name, lines);
        PinFile::parse(&read(dir, name)).expect("the fixture is canonical");
    }

    /// The census a fake test pins: a count, a tuple and a float.
    fn pin_census(pins: &mut Pins, count: u32, median: f64) {
        pins.exact("count", count);
        pins.exact("bands", (count / 4, count % 4));
        pins.within("median", median, Tolerance::absolute(1e-12));
    }

    fn open(mode: Mode, dir: &Path, test: &str) -> Pins {
        Pins::open_in_mode(mode, dir, "c", test)
    }

    #[test]
    fn blessing_writes_the_file_and_a_comparison_then_passes() {
        let dir = scratch("bless");
        let mut pins = Pins::open_in_mode(Mode::Bless, &dir, "census/compact", "a_census");
        pin_census(&mut pins, 471, 2.5);
        assert_eq!(
            read(&dir, "census/compact"),
            format!(
                "{HEADER}\na_census.bands = [117, 3]\na_census.count = 471\na_census.median = 2.5\n"
            )
        );
        let mut pins = Pins::open_in_mode(Mode::Compare, &dir, "census/compact", "a_census");
        pin_census(&mut pins, 471, 2.5);
    }

    #[test]
    #[should_panic(expected = "a_census.ratio is stale\n  \
                               pinned:   [0.25, 3]\n  \
                               received: [0.2501, 3]\n\
                               to match within 1e-6; if the change is intended, \
                               re-pin it with `just repin a_census`\n  file: ")]
    fn a_stale_pin_fails_naming_its_key_the_pinned_value_and_the_received() {
        let dir = scratch("stale");
        write(
            &dir,
            "c",
            &["a_census.count = 471", "a_census.ratio = [0.25, 3]"],
        );
        let mut pins = open(Mode::Compare, &dir, "a_census");
        pins.exact("count", 471_u32);
        pins.within("ratio", (0.250_1, 3_u8), Tolerance::absolute(1e-6));
    }

    #[test]
    #[should_panic(expected = "a_census.count is stale\n  pinned:   471\n  received: 471.0\n")]
    fn an_integer_never_matches_a_float() {
        let dir = scratch("kinds");
        write(&dir, "c", &["a_census.count = 471"]);
        open(Mode::Compare, &dir, "a_census").within("count", 471.0, Tolerance::absolute(1.0));
    }

    #[test]
    fn blessing_keeps_a_figure_within_its_tolerance_and_writes_one_outside_it_exactly() {
        let dir = scratch("tolerance");
        write(&dir, "c", &["a_census.share = 0.0348"]);
        let tolerance = Tolerance::relative(0.01);
        let within = 0.034_812_345_678_9;
        open(Mode::Compare, &dir, "a_census").within("share", within, tolerance);
        open(Mode::Bless, &dir, "a_census").within("share", within, tolerance);
        assert_eq!(
            read(&dir, "c"),
            format!("{HEADER}\na_census.share = 0.0348\n")
        );
        let outside = 0.035_212_345_678_9;
        open(Mode::Bless, &dir, "a_census").within("share", outside, tolerance);
        assert_eq!(
            read(&dir, "c"),
            format!("{HEADER}\na_census.share = 0.0352123456789\n")
        );
        open(Mode::Compare, &dir, "a_census").within("share", outside, tolerance);
    }

    #[test]
    fn blessing_rewrites_only_the_keys_the_test_reads() {
        let dir = scratch("only");
        write(
            &dir,
            "c",
            &[
                "a_census.bands = [1, 1]",
                "a_census.count = 5",
                "a_census.median = 1.0",
                "a_census.retired = 9",
                "b_census.count = 5",
                "c_census.median = 7.5",
            ],
        );
        let mut pins = open(Mode::Bless, &dir, "a_census");
        pins.exact("count", 6_u8);
        pins.exact("added", [1_i8, -1]);
        assert_eq!(
            read(&dir, "c"),
            format!(
                "{HEADER}\na_census.added = [1, -1]\na_census.bands = [1, 1]\na_census.count = 6\n\
                 a_census.median = 1.0\na_census.retired = 9\nb_census.count = 5\n\
                 c_census.median = 7.5\n"
            )
        );
    }

    #[test]
    fn a_second_run_blessed_or_not_is_a_no_op() {
        let dir = scratch("again");
        let bless = || pin_census(&mut open(Mode::Bless, &dir, "a_census"), 471, 0.1 + 0.2);
        bless();
        let path = pins_file(&dir, "c");
        let modified = || path.metadata().unwrap().modified().unwrap();
        let (text, before) = (read(&dir, "c"), modified());
        bless();
        pin_census(&mut open(Mode::Compare, &dir, "a_census"), 471, 0.1 + 0.2);
        assert_eq!(read(&dir, "c"), text);
        assert_eq!(modified(), before, "nothing was written");
        assert!(
            text.contains("a_census.median = 0.30000000000000004\n"),
            "{text}"
        );
    }

    #[test]
    #[should_panic(expected = "a_census.other is not pinned: received 1; \
                               pin it with `just repin a_census`\n  file: ")]
    fn a_missing_key_fails() {
        let dir = scratch("missing-key");
        write(&dir, "c", &["a_census.count = 1"]);
        let mut pins = open(Mode::Compare, &dir, "a_census");
        pins.exact("count", 1_u8);
        pins.exact("other", 1_u8);
    }

    #[test]
    #[should_panic(expected = "a_census.count is not pinned: received 1")]
    fn a_missing_file_fails() {
        let dir = scratch("missing-file");
        open(Mode::Compare, &dir, "a_census").exact("count", 1_u8);
    }

    #[test]
    fn blessing_adds_a_missing_key() {
        let dir = scratch("added");
        write(&dir, "c", &["a_census.count = 1"]);
        open(Mode::Bless, &dir, "a_census").exact("other", 2_u8);
        let mut pins = open(Mode::Compare, &dir, "a_census");
        pins.exact("count", 1_u8);
        pins.exact("other", 2_u8);
    }

    #[test]
    #[should_panic(expected = "c.pins: line 3: the key is not after the line before's")]
    fn a_hand_edit_out_of_the_canonical_form_fails_to_read() {
        let dir = scratch("hand-edit");
        write_text(&dir, "c", &["b.x = 1", "a.x = 2"]);
        drop(open(Mode::Compare, &dir, "a"));
    }

    #[test]
    #[should_panic(expected = "c.pins: line 2: the value is not written canonically")]
    fn blessing_refuses_a_hand_edit_out_of_the_canonical_form() {
        let dir = scratch("hand-edit-bless");
        write_text(&dir, "c", &["a.x = 1.50"]);
        open(Mode::Bless, &dir, "a").exact("y", 2_u8);
    }

    #[test]
    #[should_panic(expected = "refusing to bless")]
    fn blessing_under_ci_panics() {
        let dir = scratch("ci");
        drop(open(Mode::BlessForbidden, &dir, "a_census"));
    }

    #[test]
    fn the_log_lists_every_pin_with_its_file() {
        let dir = scratch("log");
        write(
            &dir,
            "c",
            &[
                "a_census.bands = [117, 3]",
                "a_census.count = 471",
                "a_census.median = 2.5",
            ],
        );
        let log = dir.join("pins.log");
        pin_census(
            &mut open(Mode::Compare, &dir, "a_census").logging_to(&log),
            471,
            2.5,
        );
        let file = pins_file(&dir, "c").display().to_string();
        assert_eq!(
            std::fs::read_to_string(&log).unwrap(),
            format!("{file}\ta_census.count\n{file}\ta_census.bands\n{file}\ta_census.median\n")
        );
    }

    /// libtest and nextest name a test's thread by its path; WebAssembly has no threads.
    #[cfg(not(target_family = "wasm"))]
    #[test]
    #[should_panic(
        expected = "the test native_only::a_test_may_not_open_pins_under_another_s_name \
                               opens pins as `another_test`"
    )]
    fn a_test_may_not_open_pins_under_another_s_name() {
        let dir = scratch("other-name").display().to_string();
        drop(Pins::open(&dir, "c", "another_test"));
    }

    #[cfg(not(target_family = "wasm"))]
    #[test]
    fn a_test_and_its_own_threads_open_pins_under_its_name() {
        let dir = scratch("own-name").display().to_string();
        let test = "a_test_and_its_own_threads_open_pins_under_its_name";
        drop(Pins::open(&dir, "c", test));
        std::thread::scope(|s| {
            s.spawn(|| drop(Pins::open(&dir, "c", test)));
        });
    }

    #[cfg(not(target_family = "wasm"))]
    #[test]
    fn tests_blessing_one_file_at_once_lose_none_of_each_others_pins() {
        let dir = scratch("together");
        let tests: Vec<String> = (0..8).map(|t| format!("census_{t}")).collect();
        std::thread::scope(|s| {
            for test in &tests {
                let dir = &dir;
                s.spawn(move || {
                    let mut pins = open(Mode::Bless, dir, test);
                    for key in 0..16_u32 {
                        pins.exact(&format!("k{key:02}"), key);
                    }
                });
            }
        });
        let file = PinFile::parse(&read(&dir, "c")).unwrap();
        assert_eq!(file.keys().count(), 8 * 16);
        assert_eq!(file.get("census_7.k15"), Some(&Value::from(15_u32)));
    }
}
