#!/usr/bin/env python3
"""Tests of smart-ci's Rust graph (`rust_graph.py`): each trap of Rust's name resolution, on a small
workspace of its own, held to "a test that can reach the change runs, and one that cannot does not".

Run by `test_smart_ci.py`, which `just ci` runs (`_smart-ci-test`); or alone,
`python3 tools/smart-ci/test_rust_graph.py`. Standard library only.
"""

from __future__ import annotations

import sys
import unittest
from pathlib import Path
from typing import Dict, Iterable, List, Optional, Set, Tuple

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))

import rust_graph as rg  # noqa: E402

GOLDEN = ("tests/golden/", "crates/kit/src/golden.rs")


def crate(name: str, normal: Iterable[str] = (), dev: Iterable[str] = (), tests: Iterable[str] = (),
          lib: bool = True) -> rg.CrateInfo:
    targets = [rg.Target("lib", name, f"crates/{name}/src/lib.rs", True, True)] if lib else []
    targets += [rg.Target("test", t, f"crates/{name}/tests/{t}.rs", True) for t in tests]
    return rg.CrateInfo(name, f"crates/{name}", list(normal), list(dev), targets)


class Fixture:
    """A workspace of a few crates, its files' text now and at the base."""

    def __init__(self, crates: Iterable[rg.CrateInfo], files: Dict[str, str]) -> None:
        self.crates = {c.name: c for c in crates}
        self.files = dict(files)

    def graph(self, base: Optional[Dict[str, Optional[str]]] = None) -> rg.Graph:
        facts = rg.Facts(None, lambda p: self.files[p].encode() if p in self.files else None)
        old = {p: (rg.parse(text) if text is not None else None) for p, text in (base or {}).items()}
        return rg.Graph(self.crates, sorted(self.files), facts, old, [GOLDEN])

    def change(self, path: str, old: str, new: str) -> rg.Selection:
        """Replace `old` by `new` in `path` (`old` the base's text), and select."""
        base = self.files[path]
        assert base.count(old) == 1, (path, old)
        self.files[path] = base.replace(old, new)
        try:
            return self.graph({path: base}).select([(path, False)])
        finally:
            self.files[path] = base

    def touch(self, path: str) -> rg.Selection:
        """The path changed throughout (a new file, or data)."""
        return self.graph().select([(path, False)])


def picked(sel: rg.Selection) -> Set[str]:
    return {f"{unit} {name}" for unit, tests in sel.tests.items() for name in tests}


class Resolution(unittest.TestCase):
    """Paths: the module tree, imports, globs, re-exports, `super`, the crates' names."""

    def setUp(self) -> None:
        self.ws = Fixture([crate("a")], {
            "crates/a/src/lib.rs": "pub mod math;\npub mod far;\npub mod reexport;\npub use math::double;\n",
            "crates/a/src/math.rs": (
                "pub fn double(x: u32) -> u32 { x * 2 }\n"
                "pub fn triple(x: u32) -> u32 { x * 3 }\n"
                "#[cfg(test)]\nmod tests {\n    use super::*;\n"
                "    #[test]\n    fn doubles() { assert_eq!(double(2), 4); }\n"
                "    #[test]\n    fn triples() { assert_eq!(triple(2), 6); }\n}\n"),
            "crates/a/src/far.rs": (
                "use crate::math::triple;\n"
                "pub fn nine() -> u32 { triple(3) }\n"
                "#[cfg(test)]\nmod tests {\n"
                "    #[test]\n    fn qualified() { assert_eq!(crate::math::double(1), 2); }\n"
                "    #[test]\n    fn imported() { assert_eq!(super::nine(), 9); }\n"
                "    #[test]\n    fn through_the_root() { assert_eq!(crate::double(3), 6); }\n}\n"),
            "crates/a/src/reexport.rs": "pub use crate::math::*;\n",
        })

    def test_a_changed_function_selects_the_tests_that_reach_it_and_no_other(self) -> None:
        sel = self.ws.change("crates/a/src/math.rs", "x * 3", "x * 4")
        self.assertEqual(picked(sel), {"a math::tests::triples", "a far::tests::imported"})

    def test_qualified_paths_and_re_exports_reach_the_item(self) -> None:
        sel = self.ws.change("crates/a/src/math.rs", "x * 2", "x + x + 1")
        self.assertEqual(picked(sel), {"a math::tests::doubles", "a far::tests::qualified",
                                       "a far::tests::through_the_root"})

    def test_a_changed_re_export_reaches_whoever_names_the_name_through_it(self) -> None:
        sel = self.ws.change("crates/a/src/lib.rs", "pub use math::double;", "pub use math::triple as double;")
        self.assertIn("a far::tests::through_the_root", picked(sel))
        self.assertNotIn("a far::tests::imported", picked(sel))

    def test_a_comment_or_a_formatting_change_selects_nothing(self) -> None:
        sel = self.ws.change("crates/a/src/math.rs", "pub fn triple(x: u32) -> u32 { x * 3 }",
                             "/// Triples.\npub fn triple(x: u32) -> u32 {\n    x * 3 // here\n}")
        self.assertEqual(picked(sel), set())

    def test_a_test_reaches_itself(self) -> None:
        sel = self.ws.change("crates/a/src/far.rs", "assert_eq!(super::nine(), 9)", "assert_eq!(super::nine(), 10)")
        self.assertEqual(picked(sel), {"a far::tests::imported"})


class Methods(unittest.TestCase):
    """Calls that name no item: methods, operators, trait impls, blanket impls, generics."""

    def setUp(self) -> None:
        self.ws = Fixture([crate("a"), crate("b", normal=["a"])], {
            "crates/a/src/lib.rs": "pub mod shape;\npub mod impls;\npub mod ext;\npub mod generic;\n",
            "crates/a/src/shape.rs": (
                "pub struct Circle(pub f64);\n"
                "pub trait Area { fn area(&self) -> f64; }\n"
                "pub fn make() -> Circle { Circle(1.0) }\n"),
            "crates/a/src/impls.rs": (
                "use crate::shape::{Area, Circle};\n"
                "impl Area for Circle { fn area(&self) -> f64 { 3.14 * self.0 * self.0 } }\n"
                "impl std::ops::Add for Circle { type Output = Circle; fn add(self, o: Circle) -> Circle { Circle(self.0 + o.0) } }\n"),
            "crates/a/src/ext.rs": (
                "pub trait Half { fn half(self) -> Self; }\n"
                "impl Half for f64 { fn half(self) -> f64 { self / 2.0 } }\n"),
            "crates/a/src/generic.rs": (
                "use crate::shape::Area;\n"
                "pub fn total<T: Area>(items: &[T]) -> f64 { items.iter().map(|i| i.area()).sum() }\n"),
            "crates/b/src/lib.rs": (
                "#[cfg(test)]\nmod tests {\n"
                "    use a::shape::make;\n    use a::generic::total;\n"
                "    #[test]\n    fn method_on_a_value_it_made() { use a::shape::Area; assert!(make().area() > 3.0); }\n"
                "    #[test]\n    fn generic_over_the_trait() { assert!(total(&[make()]) > 3.0); }\n"
                "    #[test]\n    fn operator() { let c = make() + make(); let _ = c; }\n"
                "    #[test]\n    fn unrelated() { assert_eq!(1, 1); }\n}\n"
                # A trait in scope reaches every item of its module, any of which may call its methods.
                "#[cfg(test)]\nmod ext_tests {\n    use a::ext::Half;\n"
                "    #[test]\n    fn foreign_type() { assert_eq!(2.0_f64.half(), 1.0); }\n}\n"),
        })

    def test_an_impl_reaches_whoever_holds_its_type(self) -> None:
        # A workspace trait's method: the tests that hold a Circle and call `area`, here or generically.
        sel = self.ws.change("crates/a/src/impls.rs", "3.14 * self.0", "3.0 * self.0")
        self.assertEqual(picked(sel), {"b tests::method_on_a_value_it_made", "b tests::generic_over_the_trait"})

    def test_an_operator_impl_reaches_whoever_holds_its_type(self) -> None:
        self.assertIn("b tests::operator", picked(self.ws.change("crates/a/src/impls.rs", "self.0 + o.0", "self.0 - o.0")))

    def test_an_impl_for_a_foreign_type_reaches_whoever_has_its_trait_in_scope(self) -> None:
        sel = self.ws.change("crates/a/src/ext.rs", "self / 2.0", "self / 3.0")
        self.assertEqual(picked(sel), {"b ext_tests::foreign_type"})


class Members(unittest.TestCase):
    """An impl's members: a changed one runs the tests that both hold its type and name it."""

    def setUp(self) -> None:
        self.ws = Fixture([crate("a")], {
            "crates/a/src/lib.rs": (
                "pub mod orbit;\npub mod uses;\n"
                "macro_rules! period_of { ($o:expr) => { $o.period() }; }\npub(crate) use period_of;\n"),
            "crates/a/src/orbit.rs": (
                "pub struct Orbit(pub f64);\n"
                "impl Orbit {\n"
                "    pub fn new(a: f64) -> Self { Orbit(a) }\n"
                "    pub fn period(&self) -> f64 { self.0 * 2.0 }\n"
                "    pub fn speed(&self) -> f64 { self.0 / 2.0 }\n"
                "}\n"
                "impl std::fmt::Display for Orbit {\n"
                "    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, \"{}\", self.0) }\n"
                "}\n"
                "pub trait Shape { fn area(&self) -> f64; }\n"
                "impl Shape for Orbit { fn area(&self) -> f64 { self.0 * self.0 } }\n"
                "pub fn total<T: Shape>(t: &T) -> f64 { t.area() }\n"),
            "crates/a/src/uses.rs": (
                "#[cfg(test)]\nmod tests {\n    use crate::orbit::{total, Orbit};\n"
                "    #[test]\n    fn period() { assert_eq!(Orbit::new(1.0).period(), 2.0); }\n"
                "    #[test]\n    fn speed() { assert_eq!(Orbit::new(1.0).speed(), 0.5); }\n"
                "    #[test]\n    fn shown() { assert_eq!(Orbit::new(1.0).to_string(), \"1\"); }\n"
                "    #[test]\n    fn generic() { assert_eq!(total(&Orbit::new(2.0)), 4.0); }\n"
                "    #[test]\n    fn by_macro() { assert_eq!(crate::period_of!(Orbit::new(1.0)), 2.0); }\n}\n"),
        })

    def test_a_changed_method_runs_the_tests_that_call_it(self) -> None:
        sel = self.ws.change("crates/a/src/orbit.rs", "self.0 * 2.0", "self.0 * 3.0")
        self.assertEqual(picked(sel), {"a uses::tests::period", "a uses::tests::by_macro"})

    def test_a_new_method_that_nothing_calls_runs_nothing(self) -> None:
        sel = self.ws.change("crates/a/src/orbit.rs", "    pub fn speed", "    pub fn unused(&self) -> f64 { 0.0 }\n    pub fn speed")
        self.assertEqual(picked(sel), set())

    def test_a_removed_method_runs_its_old_callers(self) -> None:
        sel = self.ws.change("crates/a/src/orbit.rs", "    pub fn speed(&self) -> f64 { self.0 / 2.0 }\n", "")
        self.assertEqual(picked(sel), {"a uses::tests::speed"})

    def test_another_crate_s_trait_keeps_its_impl_whole(self) -> None:
        # `to_string` calls `fmt` through std's blanket impl: no code names `fmt`.
        sel = self.ws.change("crates/a/src/orbit.rs", 'write!(f, "{}", self.0)', 'write!(f, "{:.1}", self.0)')
        self.assertEqual(picked(sel), {"a uses::tests::period", "a uses::tests::speed", "a uses::tests::shown",
                                       "a uses::tests::generic", "a uses::tests::by_macro"})

    def test_a_workspace_trait_s_method_called_generically(self) -> None:
        sel = self.ws.change("crates/a/src/orbit.rs", "fn area(&self) -> f64 { self.0 * self.0 }",
                             "fn area(&self) -> f64 { self.0 * self.0 * 1.0 }")
        self.assertEqual(picked(sel), {"a uses::tests::generic"})


class TraitsOfOtherCrates(unittest.TestCase):
    """The orphan rule: an impl of a crate's own trait for another crate's type attaches to the trait."""

    def setUp(self) -> None:
        self.ws = Fixture([crate("base"), crate("sim", normal=["base"])], {
            "crates/base/src/lib.rs": (
                "pub struct Kelvin(pub f64);\n"
                "#[cfg(test)]\nmod tests {\n    #[test]\n    fn kelvin() { let _ = super::Kelvin(1.0); }\n}\n"),
            "crates/sim/src/lib.rs": "pub mod glow;\npub mod user;\n",
            "crates/sim/src/glow.rs": (
                "use base::Kelvin;\n"
                "pub trait Glow { fn glow(&self) -> f64; }\n"
                "impl Glow for Kelvin { fn glow(&self) -> f64 { self.0 * 2.0 } }\n"),
            "crates/sim/src/user.rs": (
                "#[cfg(test)]\nmod tests {\n    use crate::glow::Glow;\n"
                "    #[test]\n    fn glows() { assert_eq!(base::Kelvin(1.0).glow(), 2.0); }\n}\n"),
        })

    def test_the_trait_s_users_run_and_the_type_s_crate_does_not(self) -> None:
        sel = self.ws.change("crates/sim/src/glow.rs", "self.0 * 2.0", "self.0 * 3.0")
        self.assertEqual(picked(sel), {"sim user::tests::glows"})


class Macros(unittest.TestCase):
    """macro_rules!, #[macro_export], `$crate`, macros that call macros and define items."""

    def setUp(self) -> None:
        self.ws = Fixture([crate("kit"), crate("a", dev=["kit"])], {
            "crates/kit/src/lib.rs": (
                "pub mod golden;\npub fn helper() -> u32 { 7 }\n"
                "#[macro_export]\nmacro_rules! seven { () => { $crate::helper() }; }\n"
                "#[macro_export]\nmacro_rules! twice_seven { () => { $crate::seven!() * 2 }; }\n"),
            "crates/kit/src/golden.rs": (
                "pub fn check(dir: &str, name: &str, actual: &str) { let _ = (dir, name, actual); }\n"
                "#[macro_export]\nmacro_rules! golden { ($n:expr, $a:expr) => { $crate::golden::check(env!(\"CARGO_MANIFEST_DIR\"), $n, $a) }; }\n"),
            "crates/a/src/lib.rs": (
                "macro_rules! make_const { ($n:ident) => { pub const $n: u32 = 3; }; }\n"
                "make_const!(THREE);\n"
                "#[cfg(test)]\nmod tests {\n"
                "    #[test]\n    fn calls_the_macro() { assert_eq!(kit::seven!(), 7); }\n"
                "    #[test]\n    fn nested_macro() { assert_eq!(kit::twice_seven!(), 14); }\n"
                "    #[test]\n    fn macro_made_item() { assert_eq!(super::THREE, 3); }\n"
                "    #[test]\n    fn plain() { assert_eq!(2, 2); }\n}\n"),
            "crates/a/tests/goldens.rs": "#[test]\nfn compares() { kit::golden!(\"x/y\", \"text\"); }\n",
            "crates/a/tests/golden/x/y.golden": "text\n",
        })
        self.ws.crates["a"].targets.append(rg.Target("test", "goldens", "crates/a/tests/goldens.rs", True))

    def test_a_macro_s_body_is_its_caller_s_code(self) -> None:
        sel = self.ws.change("crates/kit/src/lib.rs", "pub fn helper() -> u32 { 7 }", "pub fn helper() -> u32 { 8 }")
        self.assertEqual(picked(sel) & {"a tests::calls_the_macro", "a tests::nested_macro"},
                         {"a tests::calls_the_macro", "a tests::nested_macro"})
        self.assertNotIn("a tests::plain", picked(sel))

    def test_an_item_a_macro_defines_is_reached(self) -> None:
        sel = self.ws.change("crates/a/src/lib.rs", "pub const $n: u32 = 3;", "pub const $n: u32 = 4;")
        self.assertIn("a tests::macro_made_item", picked(sel))

    def test_a_golden_runs_its_crate_s_tests_that_read_goldens(self) -> None:
        sel = self.ws.touch("crates/a/tests/golden/x/y.golden")
        self.assertEqual(picked(sel), {"a::goldens compares"})


class Configurations(unittest.TestCase):
    """cfg and features, consts, local items, aliases, doctests, dev-dependencies."""

    def setUp(self) -> None:
        self.ws = Fixture([crate("kit"), crate("a", dev=["kit"])], {
            "crates/kit/src/lib.rs": "pub fn assist() -> u32 { 1 }\n",
            "crates/a/src/lib.rs": (
                "pub mod wasm;\npub mod types;\n"
                "pub const LIMIT: u32 = 10;\n"
                "/// ```\n/// assert_eq!(a::wasm::lanes(), a::LIMIT);\n/// ```\n"
                "pub fn documented() {}\n"
                "#[cfg(test)]\nmod tests {\n"
                "    #[test]\n    fn limit() { assert_eq!(super::LIMIT, 10); }\n"
                "    #[test]\n    fn local_use() { use crate::wasm::lanes; assert!(lanes() > 0); }\n"
                "    #[test]\n    fn with_kit() { assert_eq!(kit::assist(), 1); }\n}\n"),
            "crates/a/src/wasm.rs": (
                "#[cfg(target_arch = \"wasm32\")]\npub fn lanes() -> u32 { 4 }\n"
                "#[cfg(not(target_arch = \"wasm32\"))]\npub fn lanes() -> u32 { 8 }\n"),
            "crates/a/src/types.rs": (
                "pub struct Real(pub f64);\npub type Alias = Real;\n"
                "pub trait Show { fn show(&self) -> String; }\n"
                "impl Show for Alias { fn show(&self) -> String { format!(\"{}\", self.0) } }\n"
                "#[cfg(test)]\nmod tests {\n    use super::*;\n"
                "    #[test]\n    fn shows() { assert_eq!(Real(1.0).show(), \"1\"); }\n}\n"),
        })

    def test_code_for_another_target_counts(self) -> None:
        sel = self.ws.change("crates/a/src/wasm.rs", "{ 4 }", "{ 5 }")
        self.assertEqual(picked(sel), {"a tests::local_use"})
        self.assertEqual(set(sel.doctests.get("a", {})), {"crates/a/src/lib.rs"})

    def test_a_const_and_a_doctest(self) -> None:
        sel = self.ws.change("crates/a/src/lib.rs", "LIMIT: u32 = 10", "LIMIT: u32 = 11")
        self.assertEqual(picked(sel), {"a tests::limit"})
        self.assertIn("crates/a/src/lib.rs", sel.doctests.get("a", {}))

    def test_an_impl_for_an_alias_reaches_the_aliased_type(self) -> None:
        sel = self.ws.change("crates/a/src/types.rs", 'format!("{}", self.0)', 'format!("{:.1}", self.0)')
        self.assertEqual(picked(sel), {"a types::tests::shows"})

    def test_test_code_reaches_a_dev_dependency(self) -> None:
        sel = self.ws.change("crates/kit/src/lib.rs", "{ 1 }", "{ 2 }")
        self.assertEqual(picked(sel), {"a tests::with_kit"})


class Widening(unittest.TestCase):
    """What the reading cannot place widens to the crate: unreadable files, orphans, crate attributes,
    and a changed `mod` declaration changes the files it names."""

    def setUp(self) -> None:
        self.ws = Fixture([crate("a"), crate("b", normal=["a"])], {
            "crates/a/src/lib.rs": "#![allow(dead_code)]\nmod one;\n",
            "crates/a/src/one.rs": "pub fn f() {}\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() { super::f(); }\n}\n",
            "crates/a/src/two.rs": "pub fn f() {}\n",
            "crates/a/src/stray.rs": "pub fn g() {}\n",
            "crates/b/src/lib.rs": "#[cfg(test)]\nmod tests {\n    #[test]\n    fn uses_nothing() {}\n}\n",
        })

    def test_an_unreadable_file_widens_to_its_crate(self) -> None:
        sel = self.ws.change("crates/a/src/one.rs", "pub fn f() {}", "pub fn f() { (]")
        self.assertIn("a", sel.whole)
        self.assertTrue(any("could not be read" in n for n in sel.notes))

    def test_an_orphan_widens_and_crate_attributes_widen(self) -> None:
        self.assertIn("a", self.ws.touch("crates/a/src/stray.rs").whole)
        self.assertIn("a", self.ws.change("crates/a/src/lib.rs", "#![allow(dead_code)]", "#![deny(dead_code)]").whole)

    def test_a_changed_mod_declaration_changes_the_files_it_names(self) -> None:
        sel = self.ws.change("crates/a/src/lib.rs", "mod one;", '#[path = "two.rs"]\nmod one;')
        self.assertEqual(sel.whole, {})
        self.assertIn("a one::tests::t", picked(self.ws.change("crates/a/src/lib.rs", "mod one;", "pub mod one;")))


class Removals(unittest.TestCase):
    """A removed item may leave its name to a glob: the module's other items, and its importers'."""

    def setUp(self) -> None:
        self.ws = Fixture([crate("a")], {
            "crates/a/src/lib.rs": "pub mod low;\npub mod high;\n",
            "crates/a/src/low.rs": "pub fn pick() -> u32 { 1 }\n",
            "crates/a/src/high.rs": (
                "use crate::low::*;\n"
                "pub fn pick() -> u32 { 2 }\n"
                "#[cfg(test)]\nmod tests {\n    use super::*;\n"
                "    #[test]\n    fn picks() { assert_eq!(pick(), 2); }\n}\n"),
        })

    def test_removing_an_item_reaches_what_now_resolves_through_a_glob(self) -> None:
        sel = self.ws.change("crates/a/src/high.rs", "pub fn pick() -> u32 { 2 }\n", "")
        self.assertIn("a high::tests::picks", picked(sel))


class Data(unittest.TestCase):
    """Files read as data: by a path's string, by a directory's name, by a directory's listing."""

    def setUp(self) -> None:
        self.ws = Fixture([crate("a", tests=["whole", "listing", "other"])], {
            "crates/a/src/lib.rs": "pub const TABLE: &str = include_str!(\"tables/t.txt\");\n",
            "crates/a/src/tables/t.txt": "1 2 3\n",
            "crates/a/tests/whole.rs": "#[test]\nfn reads() { let _ = std::fs::read(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/tests/data/set/d.csv\")); }\n",
            "crates/a/tests/listing.rs": "#[test]\nfn lists() { let _ = std::fs::read_dir(std::path::Path::new(\"x\").join(\"set\")); }\n",
            "crates/a/tests/other.rs": "#[test]\nfn table() { assert!(!a::TABLE.is_empty()); }\n#[test]\nfn nothing() {}\n",
            "crates/a/tests/data/set/d.csv": "x\n",
        })

    def test_include_str_and_paths_read_at_run_time(self) -> None:
        self.assertEqual(picked(self.ws.touch("crates/a/src/tables/t.txt")) - {"a::listing lists"},
                         {"a::other table"})
        sel = self.ws.touch("crates/a/tests/data/set/d.csv")
        self.assertTrue({"a::whole reads", "a::listing lists"} <= picked(sel))
        self.assertIn("a", sel.whole)  # data that is no golden: every test of its crate, too

    def test_a_listing_by_the_method_reads_its_crate_s_data(self) -> None:
        # `dir.read_dir()`, by a directory that no string of the data's path names.
        self.ws.files["crates/a/tests/listing.rs"] = (
            "#[test]\nfn lists() { let _ = std::path::Path::new(env!(\"CARGO_MANIFEST_DIR\")).join(\"elsewhere\")"
            ".read_dir(); }\n")
        self.assertIn("a::listing lists", picked(self.ws.touch("crates/a/tests/data/set/d.csv")))
        self.ws.files["crates/a/tests/listing.rs"] = "#[test]\nfn lists() { let _ = \"elsewhere\"; }\n"
        self.assertNotIn("a::listing lists", picked(self.ws.touch("crates/a/tests/data/set/d.csv")))


def readers(sel: rg.Selection) -> Set[str]:
    """The tests that read the Rust sources, which the selection picked apart from `tests`."""
    return {f"{unit} {name}" for unit, tests in sel.sources.items() for name in tests}


class SourceWalkers(unittest.TestCase):
    """Tests that read the Rust sources by listing a directory: every changed `.rs` file of a crate
    selects them, whatever its code reaches, in any crate."""

    WALKER = ("pub(crate) fn rust_files(dir: &std::path::Path, found: &mut Vec<std::path::PathBuf>) {\n"
              "    for entry in std::fs::read_dir(dir).unwrap() {\n"
              "        let path = entry.unwrap().path();\n"
              "        if path.is_dir() { rust_files(&path, found); }\n"
              "        else if path.extension().is_some_and(|e| e == \"rs\") { found.push(path); }\n"
              "    }\n}\n")

    def setUp(self) -> None:
        self.ws = Fixture([crate("a"), crate("b")], {
            "crates/a/src/lib.rs": "pub mod code;\npub mod thermal;\npub mod optics;\npub mod goldens;\n",
            "crates/a/src/code.rs": "pub fn f() -> u32 { 1 }\n" + TESTS
                                    + "    #[test]\n    fn calls_f() { assert_eq!(f(), 1); }\n}\n",
            "crates/a/src/thermal.rs": "pub fn g() -> u32 { 2 }\n#[cfg(test)]\nmod tests;\n",
            "crates/a/src/thermal/tests.rs": self.WALKER + (
                "#[test]\nfn nothing_reads_g_yet() {\n    let mut found = Vec::new();\n"
                "    rust_files(&std::path::Path::new(env!(\"CARGO_MANIFEST_DIR\")).join(\"src\"), &mut found);\n}\n"
                "#[test]\nfn plain() { assert_eq!(super::g(), 2); }\n"),
            # A walker called through an import from another module.
            "crates/a/src/optics.rs": "#[cfg(test)]\nmod tests {\n    use super::super::thermal::tests::rust_files;\n"
                                      "    #[test]\n    fn nothing_reads_the_columns_yet() {\n"
                                      "        let mut found = Vec::new();\n"
                                      "        rust_files(std::path::Path::new(\"src\"), &mut found);\n    }\n}\n",
            # A listing of goldens, which names no `.rs`: no walker.
            "crates/a/src/goldens.rs": "#[cfg(test)]\nmod tests {\n"
                                       "    fn goldens(dir: &std::path::Path) -> usize {\n"
                                       "        std::fs::read_dir(dir).unwrap().filter(|e| e.as_ref().unwrap().path()"
                                       ".extension().is_some_and(|x| x == \"golden\")).count()\n    }\n"
                                       "    #[test]\n    fn every_golden_parses() { goldens(std::path::Path::new(\"x\")); }\n}\n",
            "crates/a/data/table.bin": "",
            # Crate b, which does not depend on a, reads every crate's sources by a name's end.
            "crates/b/src/lib.rs": "#[cfg(test)]\nmod tests {\n"
                                   "    fn sources(dir: &std::path::Path, out: &mut Vec<String>) {\n"
                                   "        for entry in std::fs::read_dir(dir).unwrap() {\n"
                                   "            let name = entry.unwrap().file_name().into_string().unwrap();\n"
                                   "            if name.ends_with(\".rs\") { out.push(name); }\n        }\n    }\n"
                                   "    #[test]\n    fn every_crate_keeps_the_rule() {\n        let mut out = Vec::new();\n"
                                   "        sources(std::path::Path::new(env!(\"CARGO_MANIFEST_DIR\")).parent().unwrap(), "
                                   "&mut out);\n    }\n"
                                   "    #[test]\n    fn unrelated() { assert_eq!(1, 1); }\n}\n",
        })

    ALL = {"a thermal::tests::nothing_reads_g_yet", "a optics::tests::nothing_reads_the_columns_yet",
           "b tests::every_crate_keeps_the_rule"}

    def test_a_rust_change_its_code_does_not_reach_selects_every_walker_s_test(self) -> None:
        sel = self.ws.change("crates/a/src/code.rs", "{ 1 }", "{ 3 }")
        self.assertEqual(picked(sel), {"a code::tests::calls_f"})
        # Crate a's walkers' tests, and crate b's, which does not depend on a; apart from the code's.
        self.assertEqual(readers(sel), self.ALL)
        self.assertEqual(sel.sources["b"]["tests::every_crate_keeps_the_rule"], {"crates/a/src/code.rs"})
        # A file added (in no module tree, so widened to its crate) and one deleted, too.
        self.ws.files["crates/a/src/added.rs"] = "pub fn h() {}\n"
        added = self.ws.graph().select([("crates/a/src/added.rs", False)])
        del self.ws.files["crates/a/src/added.rs"]
        self.assertIn("a", added.whole)
        self.assertEqual(readers(added), self.ALL)
        deleted = self.ws.graph({"crates/a/src/gone.rs": "pub fn h() {}\n"}).select([("crates/a/src/gone.rs", True)])
        self.assertEqual(readers(deleted), self.ALL)
        # Data is no Rust source.
        self.assertEqual(readers(self.ws.touch("crates/a/data/table.bin")), set())

    def test_the_walkers_and_the_chain_to_each(self) -> None:
        graph = self.ws.graph()
        self.assertEqual(graph.source_walkers(), {
            "crates/a/src/thermal/tests.rs#fn::rust_files", "crates/b/src/lib.rs#fn:tests:sources"})
        sel = self.ws.change("crates/a/src/code.rs", "{ 1 }", "{ 3 }")
        # Reached through the import, the chain ends at the walker.
        chain = sel.via["a optics::tests::nothing_reads_the_columns_yet"]
        self.assertEqual(chain[0], "crates/a/src/optics.rs#fn:tests:nothing_reads_the_columns_yet")
        self.assertEqual(chain[-1], "crates/a/src/thermal/tests.rs#fn::rust_files")
        # A listing of goldens, and a test that holds a walker's module but not its call, read no source.
        self.assertNotIn("a goldens::tests::every_golden_parses", readers(sel))
        self.assertNotIn("a thermal::tests::plain", readers(sel))
        self.assertNotIn("b tests::unrelated", readers(sel))

    def test_a_comment_runs_them_and_a_test_the_code_reaches_is_in_both(self) -> None:
        # A comment changes no item, but it is in the text the walkers read.
        sel = self.ws.change("crates/a/src/code.rs", "pub fn f()", "/// One.\npub fn f()")
        self.assertEqual(sel.tests, {})
        self.assertEqual(readers(sel), self.ALL)
        sel = self.ws.change("crates/a/src/thermal/tests.rs", "found.push(path)", "found.insert(0, path)")
        self.assertIn("a thermal::tests::nothing_reads_g_yet", picked(sel))
        self.assertIn("a thermal::tests::nothing_reads_g_yet", readers(sel))

    def test_a_walker_that_lists_by_the_method_is_one(self) -> None:
        # `dir.read_dir()`, std's `Path::read_dir`, lists as `fs::read_dir(dir)` does. The lexer keeps
        # `read_dir` among the names after `.` or `::`: the method, or the path's last segment.
        method = self.WALKER.replace("std::fs::read_dir(dir)", "dir.read_dir()")
        for text in (self.WALKER, method):
            facts = rg.parse(text)
            index = next(i for i, item in enumerate(facts.items) if item[2] == "rust_files")
            self.assertIn("read_dir", facts.item_methods[index])
            self.assertTrue(rg.Graph._lists_directory(facts, index))
        self.ws.files["crates/a/src/thermal/tests.rs"] = self.ws.files["crates/a/src/thermal/tests.rs"].replace(
            self.WALKER, method)
        self.assertIn("crates/a/src/thermal/tests.rs#fn::rust_files", self.ws.graph().source_walkers())
        sel = self.ws.change("crates/b/src/lib.rs", "fn unrelated()", "fn unrelated_too()")
        self.assertEqual(readers(sel), {"b tests::every_crate_keeps_the_rule"})  # a's read only a
        sel = self.ws.change("crates/a/src/code.rs", "{ 1 }", "{ 3 }")
        self.assertEqual(readers(sel), self.ALL)

    def test_a_method_or_a_function_of_another_type_named_read_dir_is_a_listing_too(self) -> None:
        # A known limit: the reading cannot tell std's `Path::read_dir` from another type's method of
        # that name, nor from a function `read_dir` (the surface crate's `reading/tests.rs` has one),
        # so both count as listing a directory. That only adds: a listing with no `rs` string is no
        # source walker, and its crate's data files select its tests.
        facts = rg.parse("pub struct Shelf;\nimpl Shelf {\n    pub fn read_dir(&self) -> u32 { 1 }\n}\n"
                         "pub fn count() -> u32 { Shelf.read_dir() }\n"
                         "fn read_dir(dir: [f64; 3]) -> [f64; 3] { dir }\npub fn turn() -> [f64; 3] { read_dir([0.0; 3]) }\n")
        listing = {item[2] for i, item in enumerate(facts.items) if rg.Graph._lists_directory(facts, i)}
        self.assertTrue({"count", "turn"} <= listing, listing)


def scoped(sel: rg.Selection) -> Dict[str, Optional[List[str]]]:
    """The scope of each test that read the Rust sources: its crates, None for every crate."""
    return {key: crates for key, (crates, _) in sel.scopes.items()}


class SourceScopes(unittest.TestCase):
    """A test that reads the Rust sources runs for a changed `.rs` file of a crate its walker may list,
    read from the strings its code holds, and for any change its code reaches."""

    WALKER = SourceWalkers.WALKER.replace("\n", "\n    ").replace("pub(crate) fn", "    fn").rstrip(" ")

    def setUp(self) -> None:
        self.ws = Fixture([crate("a", normal=["c"]), crate("b"), crate("c")], {
            "crates/a/src/lib.rs": "pub fn f() -> u32 { 1 }\n" + TESTS + self.WALKER + (
                "    #[test]\n    fn own_sources() {\n        let mut found = Vec::new();\n"
                "        rust_files(&std::path::Path::new(env!(\"CARGO_MANIFEST_DIR\")).join(\"src\"), &mut found);\n"
                "    }\n"
                "    #[test]\n    fn own_and_b_sources() {\n"
                "        let manifest = std::path::Path::new(env!(\"CARGO_MANIFEST_DIR\"));\n"
                "        let mut found = Vec::new();\n"
                "        rust_files(&manifest.join(\"src\"), &mut found);\n"
                "        rust_files(&manifest.join(\"../b/src\"), &mut found);\n    }\n"
                # Computed, as the surface crate's tag check is: its `src` names its own file, skipped.
                "    #[test]\n    fn every_crate() {\n        let own = std::path::Path::new(\"src\").join(\"lib.rs\");\n"
                "        let mut found = Vec::new();\n"
                "        rust_files(std::path::Path::new(env!(\"CARGO_MANIFEST_DIR\")).parent().unwrap(), &mut found);\n"
                "        found.retain(|p| !p.ends_with(&own));\n    }\n"
                "    #[test]\n    fn own_sources_and_c() {\n        assert_eq!(c::g(), 2);\n"
                "        let mut found = Vec::new();\n        rust_files(std::path::Path::new(\"src\"), &mut found);\n"
                "    }\n}\n"),
            "crates/b/src/lib.rs": "pub fn b() -> u32 { 2 }\n",
            "crates/c/src/lib.rs": "pub fn g() -> u32 { 2 }\n",
        })

    def test_each_reader_runs_for_the_crates_it_lists(self) -> None:
        sel = self.ws.change("crates/a/src/lib.rs", "{ 1 }", "{ 3 }")
        self.assertEqual(scoped(sel), {"a tests::own_sources": ["a"], "a tests::own_and_b_sources": ["a", "b"],
                                       "a tests::every_crate": None, "a tests::own_sources_and_c": ["a"]})
        # (a) A crate the reader's code does not reach, and its strings do not name: not it.
        sel = self.ws.change("crates/b/src/lib.rs", "{ 2 }", "{ 4 }")
        self.assertEqual(readers(sel), {"a tests::own_and_b_sources", "a tests::every_crate"})  # (b), (c)
        self.assertEqual(sel.sources["a"]["tests::own_and_b_sources"], {"crates/b/src/lib.rs"})
        # A comment, which changes no item, the same.
        sel = self.ws.change("crates/b/src/lib.rs", "pub fn b()", "/// Two.\npub fn b()")
        self.assertEqual(readers(sel), {"a tests::own_and_b_sources", "a tests::every_crate"})

    def test_a_reader_the_change_reaches_through_code_runs_whatever_its_scope(self) -> None:
        # (d) c is in no reader's scope but the computed one's, and one reader's code calls c.
        sel = self.ws.change("crates/c/src/lib.rs", "{ 2 }", "{ 5 }")
        self.assertEqual(readers(sel), {"a tests::every_crate"})
        self.assertIn("a tests::own_sources_and_c", picked(sel))
        self.assertNotIn("a tests::own_sources", picked(sel))

    def scope(self, body: str, **files: str) -> Tuple[Optional[List[str]], str]:
        """The scope of a test `t` in crate a whose body is `body`, calling the walker."""
        ws = Fixture([crate("a", normal=["c"]), crate("b"), crate("c")], {
            "crates/a/src/lib.rs": TESTS + self.WALKER + "    #[test]\n    fn t() {\n        let mut found = Vec::new();\n"
                                   f"        {body}\n    }}\n}}\n",
            "crates/b/src/lib.rs": "", "crates/c/src/lib.rs": "", **{f"crates/{k}/src/lib.rs": v for k, v in files.items()}})
        graph = ws.graph()
        _, chains = graph.source_readers()
        return graph.source_scope("a", "tests::t", chains["a tests::t"][0])

    def test_what_names_a_crate_and_what_computes_the_directory(self) -> None:
        manifest = "std::path::Path::new(env!(\"CARGO_MANIFEST_DIR\"))"
        named = {
            # `concat!`'s form.
            f"rust_files(std::path::Path::new(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/src\")), &mut found);": ["a"],
            f"rust_files(std::path::Path::new(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/../b\")), &mut found);":
                ["a", "b"],
            f"rust_files(&{manifest}.join(\"tests\"), &mut found);": ["a"],  # its directory alone
            # A crate's directory's name, as one joined to a computed directory would be (`b/src`).
            "rust_files(&std::path::Path::new(\"src\").join(\"c\"), &mut found); let _ = \"b/src/lib.rs\";": ["a", "b"],
            # A file under `src/` and a pattern name no directory listed.
            f"rust_files(&{manifest}.join(\"src\"), &mut found); let _ = (\"substance/mod.rs\", \"pub fn \");": ["a"],
        }
        for body, crates in named.items():
            self.assertEqual(self.scope(body), (crates, ""), body)
        computed = {
            f"rust_files(&{manifest}.join(\"../..\"), &mut found);": "`..` that names no crate",
            f"rust_files(&{manifest}.join(\"src/../../b\"), &mut found);": "`..` that names no crate",
            f"rust_files(&{manifest}.join(\"crates\"), &mut found);": "the string \"crates\"",
            "rust_files(std::path::Path::new(env!(\"OUT_DIR\")), &mut found);": "the environment variable `OUT_DIR`",
            "rust_files(&std::path::PathBuf::from(std::env::var(\"HOME\").unwrap()), &mut found);":
                "the environment variable `HOME`",
            f"let root = {manifest}.ancestors().nth(2).unwrap(); rust_files(root, &mut found);": "`ancestors()`",
            f"let mut root = {manifest}.to_path_buf(); root.pop(); rust_files(&root, &mut found);": "`pop()`",
        }
        for body, why in computed.items():
            crates, reason = self.scope(body)
            self.assertIsNone(crates, body)
            self.assertIn(why, reason, body)
        # A directory that another crate's code computes, and one that nothing names.
        crates, reason = self.scope("rust_files(&c::root(), &mut found);",
                                    c="pub fn root() -> std::path::PathBuf {\n"
                                      "    std::path::Path::new(env!(\"CARGO_MANIFEST_DIR\")).parent().unwrap().into()\n}\n")
        self.assertIsNone(crates)
        self.assertIn("`parent()` in crates/c/src/lib.rs (fn root)", reason)
        self.assertEqual(self.scope("rust_files(&std::env::temp_dir(), &mut found);"),
                         (None, "no string of its code names the directory it lists"))
        # A `parent()` of another type, in code that works with no path, computes no directory.
        self.assertEqual(self.scope(f"rust_files(&{manifest}.join(\"src\"), &mut found); let _ = c::up(3);",
                                    c="pub struct Body(u32);\nimpl Body {\n    pub fn parent(&self) -> u32 { self.0 }\n}\n"
                                      "pub fn up(x: u32) -> u32 { Body(x).parent() }\n"),
                         (["a"], ""))


class Filters(unittest.TestCase):
    """nextest's filters: modules by their paths, tests by name, generated ones by their pattern."""

    TESTS = [rg.TestFn("n1", "a::tests::one", "a::tests", False), rg.TestFn("n2", "a::tests::two", "a::tests", False),
             rg.TestFn("n3", "b::tests::three", "b::tests", False), rg.TestFn("n4", "b::c::tests::four", "b::c::tests", False),
             rg.TestFn("n5", "root", "", False)]

    def test_all_of_a_binary_is_no_filter(self) -> None:
        self.assertIsNone(rg.unit_filter(self.TESTS, {t.name for t in self.TESTS}, []))

    def test_modules_by_path_and_tests_by_name(self) -> None:
        found = rg.unit_filter(self.TESTS, {"a::tests::one", "a::tests::two", "b::tests::three"}, [])
        self.assertEqual(found, "test(/^a::tests::/) | test(/^b::tests::/)")
        # A module whose own tests all run, though one below it does not: its own tests by its path.
        two_levels = self.TESTS + [rg.TestFn("n6", "b::tests::deep::five", "b::tests::deep", False)]
        found = rg.unit_filter(two_levels, {"b::tests::three"}, [])
        self.assertEqual(found, "test(/^b::tests::[^:]+$/)")
        found = rg.unit_filter(self.TESTS, {"a::tests::one", "root"}, ["test(/^gen::/)"])
        self.assertEqual(found, "test(/^[^:]+$/) | test(=a::tests::one) | test(/^gen::/)")
        self.assertEqual(rg.unit_filter(self.TESTS, {"a::tests::one"}, [], coarse=1), "test(/^a::tests::/)")


def lib(text: str, *more: rg.CrateInfo, files: Optional[Dict[str, str]] = None) -> Fixture:
    """A crate `a` whose library is `text`, beside other crates and files."""
    return Fixture([crate("a"), *more], {"crates/a/src/lib.rs": text, **(files or {})})


TESTS = "#[cfg(test)]\nmod tests {\n    use super::*;\n"


class Traps(unittest.TestCase):
    """The patterns an adversarial review of the graph found it missed (2026-10-10), each on a fixture
    of its own: where code reaches other code that its paths do not name."""

    ORBIT = (
        "pub struct Orbit(pub f64);\n"
        "pub trait Shape { fn area(&self) -> f64; }\n"
        "impl Orbit { pub fn new(a: f64) -> Self { Orbit(a) } pub fn period(&self) -> f64 { self.0 * 2.0 } }\n"
        "impl Shape for Orbit { fn area(&self) -> f64 { self.0 * 4.0 } }\n"
        "macro_rules! period_of { ($o:expr) => { $crate::Orbit::period(&$o) }; }\n"
        "macro_rules! area_of { ($t:ty, $o:expr) => { <$t as $crate::Shape>::area(&$o) }; }\n"
        "macro_rules! call { ($o:expr, $m:ident) => { $o.$m() }; }\n"
        + TESTS +
        "    #[test]\n    fn by_path() { assert_eq!(period_of!(Orbit::new(1.0)), 2.0); }\n"
        "    #[test]\n    fn qualified() { assert_eq!(area_of!(Orbit, Orbit::new(1.0)), 4.0); }\n"
        "    #[test]\n    fn by_argument() { assert_eq!(call!(Orbit::new(1.0), period), 2.0); }\n"
        "    #[test]\n    fn neither() { let _ = Orbit::new(1.0); }\n}\n")

    def test_a_member_that_a_macro_s_body_names_or_its_arguments_name(self) -> None:
        ws = lib(self.ORBIT)
        self.assertEqual(picked(ws.change("crates/a/src/lib.rs", "self.0 * 2.0", "self.0 * 3.0")),
                         {"a tests::by_path", "a tests::by_argument"})
        self.assertEqual(picked(ws.change("crates/a/src/lib.rs", "self.0 * 4.0", "self.0 * 5.0")),
                         {"a tests::qualified"})

    def test_a_macro_call_among_an_impl_s_members_keeps_it_whole(self) -> None:
        ws = lib("pub struct W(pub Vec<u32>);\n"
                 "macro_rules! getter { ($n:ident, $i:expr) => { pub fn $n(&self) -> u32 { self.0[$i] } }; }\n"
                 "impl W { getter!(first, 0); pub fn size(&self) -> usize { self.0.len() } }\n"
                 + TESTS + "    #[test]\n    fn reads_the_first() { assert_eq!(W(vec![1, 2]).first(), 1); }\n}\n")
        self.assertEqual(picked(ws.change("crates/a/src/lib.rs", "getter!(first, 0)", "getter!(first, 1)")),
                         {"a tests::reads_the_first"})

    def test_a_format_string_s_inline_argument(self) -> None:
        ws = lib("pub const ROWS: usize = 3;\n"
                 "pub fn header() -> String { format!(\"rows {ROWS:>2} {{ROWS}}\") }\n"
                 "pub fn width() -> String { format!(\"{:>8}\", 1) }\n"
                 + TESTS + "    #[test]\n    fn header_counts() { assert!(header().contains('3')); }\n"
                 "    #[test]\n    fn width_pads() { assert_eq!(width().len(), 8); }\n}\n")
        self.assertEqual(picked(ws.change("crates/a/src/lib.rs", "ROWS: usize = 3", "ROWS: usize = 4")),
                         {"a tests::header_counts"})

    def test_a_turbofish_s_types(self) -> None:
        ws = Fixture([crate("a"), crate("b", normal=["a"])], {
            "crates/a/src/lib.rs": "pub mod x;\npub trait Tr { fn m(&self) -> u32; }\n"
                                   "pub fn run<T: Default + Tr>() -> u32 { T::default().m() }\n",
            "crates/a/src/x.rs": "#[derive(Default)]\npub struct X;\nimpl crate::Tr for X { fn m(&self) -> u32 { 5 } }\n",
            "crates/b/src/lib.rs": "#[cfg(test)]\nmod tests {\n"
                                   "    #[test]\n    fn t() { assert_eq!(a::run::<a::x::X>(), 5); }\n"
                                   "    #[test]\n    fn u() { assert_eq!(1, 1); }\n}\n",
        })
        self.assertEqual(picked(ws.change("crates/a/src/x.rs", "{ 5 }", "{ 6 }")), {"b tests::t"})

    def test_a_changed_cfg_on_an_import(self) -> None:
        ws = lib("pub mod slow { pub fn compute() -> u32 { 1 } }\npub mod fast { pub fn compute() -> u32 { 2 } }\n"
                 "#[cfg(not(feature = \"fast\"))]\nuse slow::compute;\n#[cfg(feature = \"fast\")]\nuse fast::compute;\n"
                 "pub fn run() -> u32 { compute() }\n"
                 + TESTS + "    #[test]\n    fn runs() { assert!(run() > 0); }\n}\n")
        sel = ws.change("crates/a/src/lib.rs", "#[cfg(not(feature = \"fast\"))]\nuse slow", "#[cfg(feature = \"fast\")]\nuse slow")
        self.assertIn("a tests::runs", picked(sel))

    def test_a_path_from_the_extern_prelude(self) -> None:
        ws = Fixture([crate("a"), crate("b", normal=["a"])], {
            "crates/a/src/lib.rs": "pub fn helper() -> u32 { 1 }\n",
            "crates/b/src/lib.rs": "#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() { assert_eq!(::a::helper(), 1); }\n}\n",
        })
        self.assertEqual(picked(ws.change("crates/a/src/lib.rs", "{ 1 }", "{ 2 }")), {"b tests::t"})

    def test_an_impl_for_an_alias_of_a_foreign_type(self) -> None:
        ws = Fixture([crate("a"), crate("b", normal=["a"])], {
            "crates/a/src/lib.rs": "pub trait Tr { fn m(&self) -> f64; }\npub type Meters = f64;\n"
                                   "impl Tr for Meters { fn m(&self) -> f64 { *self * 2.0 } }\n",
            "crates/b/src/lib.rs": "#[cfg(test)]\nmod tests {\n    use a::Tr;\n"
                                   "    #[test]\n    fn t() { assert_eq!(2.0_f64.m(), 4.0); }\n}\n",
        })
        self.assertEqual(picked(ws.change("crates/a/src/lib.rs", "*self * 2.0", "*self * 3.0")), {"b tests::t"})

    def test_a_function_or_a_module_that_a_field_s_attribute_names(self) -> None:
        tests = {"crates/b/src/lib.rs": "#[cfg(test)]\nmod tests {\n"
                                        "    #[test]\n    fn t() { let _ = a::Dto { v: 1 }; }\n}\n"}
        by_function = Fixture([crate("a"), crate("b", normal=["a"])], {
            "crates/a/src/lib.rs": "pub mod ser { pub fn ser_v(v: &u32) -> u32 { *v + 1 } }\n"
                                   "#[derive(Serialize)]\npub struct Dto {\n"
                                   "    #[serde(serialize_with = \"ser::ser_v\")]\n    pub v: u32,\n}\n", **tests})
        self.assertEqual(picked(by_function.change("crates/a/src/lib.rs", "*v + 1", "*v + 2")), {"b tests::t"})
        by_module = Fixture([crate("a"), crate("b", normal=["a"])], {
            "crates/a/src/lib.rs": "pub mod ser { pub fn serialize(v: &u32) -> u32 { *v + 1 } pub fn deserialize() {} }\n"
                                   "#[derive(Serialize)]\npub struct Dto {\n"
                                   "    #[serde(with = \"ser\")]\n    pub v: u32,\n}\n", **tests})
        self.assertEqual(picked(by_module.change("crates/a/src/lib.rs", "*v + 1", "*v + 2")), {"b tests::t"})

    def test_a_macro_under_another_name_and_one_another_macro_makes(self) -> None:
        renamed = Fixture([crate("kit"), crate("a", dev=["kit"])], {
            "crates/kit/src/lib.rs": "pub fn helper() -> u32 { 7 }\n#[doc(hidden)]\n#[macro_export]\n"
                                     "macro_rules! __seven { () => { $crate::helper() }; }\npub use __seven as seven;\n",
            "crates/a/src/lib.rs": "#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() { assert_eq!(kit::seven!(), 7); }\n}\n",
        })
        self.assertEqual(picked(renamed.change("crates/kit/src/lib.rs", "{ 7 }", "{ 8 }")), {"a tests::t"})
        made = lib("pub fn base() -> u32 { 1 }\n"
                   "macro_rules! make_macro { ($n:ident) => { macro_rules! $n { () => { crate::base() }; } }; }\n"
                   "make_macro!(seven);\n"
                   "#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() { assert_eq!(seven!(), 1); }\n}\n")
        self.assertIn("a tests::t", picked(made.change("crates/a/src/lib.rs", "{ 1 }", "{ 2 }")))

    def test_an_item_that_a_macro_s_body_names_itself(self) -> None:
        # `domain_tags! { ... }` defines the registry's `ALL`, a name its arguments do not hold.
        ws = Fixture([crate("base"), crate("a", normal=["base"], tests=["pinned"])], {
            "crates/base/src/lib.rs": (
                "pub struct Tag(pub u64);\nimpl Tag { pub const fn registered(n: u64) -> Self { Tag(n + 1) } }\n"
                "#[macro_export]\nmacro_rules! tags { ($($name:ident = $n:literal;)*) => {\n"
                "    $(pub const $name: $crate::Tag = $crate::Tag::registered($n);)*\n"
                "    pub const ALL: &[$crate::Tag] = &[$($name),*];\n}; }\n"),
            "crates/a/src/lib.rs": "pub mod tags { base::tags! { ONE = 1; } }\n",
            "crates/a/tests/pinned.rs": "#[test]\nfn all_pinned() { assert_eq!(a::tags::ALL.len(), 1); }\n",
        })
        self.assertEqual(picked(ws.change("crates/base/src/lib.rs", "Tag(n + 1)", "Tag(n + 2)")),
                         {"a::pinned all_pinned"})

    def test_tests_that_a_macro_makes_through_another_or_with_a_path_attribute(self) -> None:
        nested = lib("pub fn f() -> u32 { 1 }\n"
                     "macro_rules! inner { ($n:ident) => { #[test] fn $n() { assert_eq!(crate::f(), 1); } }; }\n"
                     "macro_rules! outer { ($n:ident) => { inner!($n); }; }\n"
                     "#[cfg(test)]\nmod tests {\n    outer!(made);\n}\n")
        self.assertIn("test(/^tests::/)", nested.change("crates/a/src/lib.rs", "{ 1 }", "{ 2 }").generated.get("a", {}))
        tokio = lib("pub fn f() -> u32 { 1 }\n"
                    "macro_rules! cases { ($n:ident) => { #[tokio::test] async fn $n() { assert_eq!(crate::f(), 1); } }; }\n"
                    "#[cfg(test)]\nmod tests {\n    cases!(made);\n}\n")
        self.assertIn("test(/^tests::/)", tokio.change("crates/a/src/lib.rs", "{ 1 }", "{ 2 }").generated.get("a", {}))

    def test_test_attributes_under_cfg_attr_with_cases_and_outside_a_test_module(self) -> None:
        under = lib("pub fn f() -> u32 { 1 }\n#[cfg(test)]\nmod tests {\n"
                    "    #[cfg_attr(not(target_arch = \"wasm32\"), test)]\n    fn t() { assert_eq!(super::f(), 1); }\n}\n")
        self.assertEqual(picked(under.change("crates/a/src/lib.rs", "{ 1 }", "{ 2 }")), {"a tests::t"})
        cases = lib("pub fn compute(x: u32) -> u32 { x * 2 }\n#[cfg(test)]\nmod tests {\n    use test_case::test_case;\n"
                    "    #[test_case(super::compute(3) => 6 ; \"three\")]\n    fn t(x: u32) -> u32 { x }\n}\n")
        sel = cases.change("crates/a/src/lib.rs", "x * 2", "x * 3")
        self.assertEqual(picked(sel), {"a tests::t"})
        self.assertIn("test(/^tests::t::/)", sel.generated.get("a", {}))  # libtest's `tests::t::three`
        loose = Fixture([crate("kit"), crate("a", dev=["kit"])], {
            "crates/kit/src/lib.rs": "pub fn assist() -> u32 { 1 }\n",
            "crates/a/src/lib.rs": "#[test]\nfn t() { assert_eq!(kit::assist(), 1); }\n",
        })
        self.assertEqual(picked(loose.change("crates/kit/src/lib.rs", "{ 1 }", "{ 2 }")), {"a t"})

    def test_a_doctest_s_macro_body_and_glob_import(self) -> None:
        macro = Fixture([crate("kit")], {"crates/kit/src/lib.rs": (
            "pub mod h { pub fn helper() -> u32 { 7 } }\n/// ```\n/// assert_eq!(kit::seven!(), 7);\n/// ```\n"
            "#[macro_export]\nmacro_rules! seven { () => { $crate::h::helper() }; }\n")})
        self.assertIn("crates/kit/src/lib.rs",
                      macro.change("crates/kit/src/lib.rs", "{ 7 }", "{ 8 }").doctests.get("kit", {}))
        glob = Fixture([crate("a")], {
            "crates/a/src/lib.rs": "/// ```\n/// use a::math::*;\n/// assert_eq!(double(2), 4);\n/// ```\npub mod math;\n",
            "crates/a/src/math.rs": "pub fn double(x: u32) -> u32 { x * 2 }\n"})
        self.assertIn("crates/a/src/lib.rs", glob.change("crates/a/src/math.rs", "x * 2", "x + x").doctests.get("a", {}))

    def test_items_an_include_brings(self) -> None:
        ws = lib("include!(\"gen.rs\");\n" + TESTS + "    #[test]\n    fn t() { assert_eq!(generated(), 1); }\n}\n",
                 files={"crates/a/src/gen.rs": "pub fn generated() -> u32 { 1 }\n"})
        self.assertIn("a tests::t", picked(ws.change("crates/a/src/gen.rs", "{ 1 }", "{ 2 }")))

    def test_an_impl_inside_a_method_or_made_by_a_macro_in_a_body(self) -> None:
        local = lib("pub trait Tr { fn m(&self) -> u32; }\npub struct X;\npub struct Y;\n"
                    "impl X { pub fn register(&self) { impl Tr for Y { fn m(&self) -> u32 { 1 } } } }\n"
                    + TESTS + "    #[test]\n    fn t() { assert_eq!(Y.m(), 1); }\n}\n")
        self.assertEqual(picked(local.change("crates/a/src/lib.rs", "{ 1 }", "{ 2 }")), {"a tests::t"})
        made = lib("pub trait Tr { fn m(&self) -> u32; }\npub struct X;\n"
                   "macro_rules! imp { ($t:ty) => { impl Tr for $t { fn m(&self) -> u32 { 1 } } }; }\n"
                   "const _: () = { imp!(X); };\n"
                   + TESTS + "    #[test]\n    fn t() { assert_eq!(X.m(), 1); }\n}\n")
        self.assertEqual(picked(made.change("crates/a/src/lib.rs", "{ 1 }", "{ 2 }")), {"a tests::t"})

    def test_a_cycle_of_globs_resolves_in_any_order(self) -> None:
        for name in ("zz", "aa"):
            ws = lib(f"pub mod other;\npub mod {name};\npub use {name}::*;\nuse other::*;\nfn top() -> u32 {{ helper() }}\n",
                     files={"crates/a/src/other.rs": "pub fn helper() -> u32 { 1 }\n",
                            f"crates/a/src/{name}.rs": "use super::*;\n" + TESTS
                            + "    #[test]\n    fn t() { assert_eq!(helper(), 1); }\n}\n"})
            self.assertEqual(picked(ws.change("crates/a/src/other.rs", "{ 1 }", "{ 2 }")), {f"a {name}::tests::t"})

    def test_another_crate_s_data_read_through_its_directory(self) -> None:
        ws = Fixture([crate("a"), crate("b")], {
            "crates/a/src/lib.rs": "",
            "crates/a/data/x.bin": "",
            "crates/b/src/lib.rs": "#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() {\n"
                                   "        let dir = std::path::Path::new(env!(\"CARGO_MANIFEST_DIR\")).join(\"../a/data\");\n"
                                   "        let _ = dir.join(\"x.bin\");\n    }\n"
                                   "    #[test]\n    fn u() { assert_eq!(1, 1); }\n}\n",
        })
        sel = ws.touch("crates/a/data/x.bin")
        self.assertEqual(picked(sel), {"b tests::t"})
        self.assertIn("a", sel.whole)

    def test_a_range_in_a_list_of_arguments(self) -> None:
        ws = lib("pub const LIMIT: usize = 3;\npub fn pairs() -> usize { std::iter::zip(0..LIMIT, 0..10).count() }\n"
                 + TESTS + "    #[test]\n    fn t() { assert_eq!(pairs(), 3); }\n}\n")
        self.assertEqual(picked(ws.change("crates/a/src/lib.rs", "LIMIT: usize = 3", "LIMIT: usize = 4")), {"a tests::t"})

    def test_a_global_allocator_and_a_module_outside_the_crate(self) -> None:
        alloc = lib("mod alloc_impl;\npub fn f() -> u32 { 1 }\n" + TESTS + "    #[test]\n    fn t() { assert_eq!(f(), 1); }\n}\n",
                    files={"crates/a/src/alloc_impl.rs": "use std::alloc::System;\n#[global_allocator]\nstatic A: System = System;\n"})
        self.assertIn("a", alloc.change("crates/a/src/alloc_impl.rs", "static A", "static B").whole)
        shared = lib("#[path = \"../../../shared/common.rs\"]\nmod common;\n"
                     + TESTS + "    #[test]\n    fn t() { assert_eq!(common::f(), 1); }\n}\n",
                     files={"shared/common.rs": "pub fn f() -> u32 { 1 }\n"})
        self.assertIn("a", shared.change("shared/common.rs", "{ 1 }", "{ 2 }").whole)


class Lexing(unittest.TestCase):
    def test_strings_comments_lifetimes_and_raw_identifiers(self) -> None:
        lexed = rg.lex('let r#type = "a // b"; /* c /* nested */ d */ \'x\'; fn f<\'a>() {} r#"raw"#')
        self.assertEqual([v for k, v in zip(lexed.kinds, lexed.vals) if k == "i"], ["let", "type", "fn", "f"])
        self.assertIn("a // b", lexed.vals)
        self.assertIn("raw", lexed.vals)

    def test_doctests_are_read_from_rust_code_blocks_only(self) -> None:
        facts = rg.parse("/// ```\n/// use x::y;\n/// y::z();\n/// ```\n/// ```text\n/// not::code\n/// ```\n"
                         "/// ```ignore\n/// ignored::code\n/// ```\npub fn f() {}\n")
        self.assertEqual(len(facts.doctests), 1)
        self.assertEqual(facts.doctests[0][1], [["y", "x", "y"]])


if __name__ == "__main__":
    unittest.main(verbosity=1)
