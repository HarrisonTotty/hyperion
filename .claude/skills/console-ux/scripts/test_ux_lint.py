#!/usr/bin/env python3
"""Tests for ux_lint.py's typography and substance rules (signoff-4 item 7, signoff-5 item 8,
decision-substance-names §7): `python3 -B .claude/skills/console-ux/scripts/test_ux_lint.py`.

Standard library only. Each test writes a small renderer file into a temporary checkout and lints
it there. The fonts are stood in for by the coverage the UX guide states (test_glyphs.STATED), so
the tests run without `pnpm install`.
"""

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))

import glyphs  # noqa: E402  (the script's directory is put on the path first)
import ux_lint  # noqa: E402
from test_glyphs import STATED  # noqa: E402


class Fixture(unittest.TestCase):
    """A temporary checkout with a renderer, linted on the guide's stated font coverage."""

    def setUp(self) -> None:
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        patch = mock.patch.object(glyphs, "coverage", lambda font: STATED)
        patch.start()
        self.addCleanup(patch.stop)

    def lint(self, source: str, name: str = "displays/probe.ts") -> list[ux_lint.Finding]:
        path = self.root / ux_lint.RENDERER / "src" / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(source, encoding="utf-8")
        unchecked: dict[str, str] = {}
        findings = ux_lint.lint(path, self.root, unchecked)
        self.assertEqual(unchecked, {})
        return findings

    def rules(self, source: str, name: str = "displays/probe.ts") -> list[tuple[str, str]]:
        """(rule, severity) of each finding, in line order."""
        return [(f.rule, f.severity) for f in sorted(self.lint(source, name), key=lambda f: (f.line, f.rule))]

    def label(self, text: str) -> list[tuple[str, str]]:
        """The findings for one displayed label."""
        return self.rules(f'export const LABEL = "{text}";\n')


class Formulas(Fixture):
    def test_a_parenthesised_count_is_an_error(self) -> None:
        for text in ("CO(2) PARTIAL PRESSURE", "H(2)O ICE", "Mg(2)SiO(4) DUST"):
            with self.subTest(text):
                self.assertEqual(self.label(text), [("formula", "error")])

    def test_the_message_writes_the_formula_on_the_line_and_cites_typography(self) -> None:
        (finding,) = self.lint('export const LABEL = "CO(2) ICE";\n')
        self.assertIn(": CO2.", finding.message)
        self.assertEqual(finding.section, "Typography")

    def test_one_symbol_with_a_parenthesised_digit_is_a_symbols_subscript(self) -> None:
        for text in ("4-0 S(1) LINE", "A(V) 0.31 mag", "E(B-V) 0.10 mag"):
            with self.subTest(text):
                self.assertEqual(self.label(text), [])

    def test_a_subscript_count_is_one_formula_error_and_no_glyph_error(self) -> None:
        findings = self.lint('export const LABEL = "CO₂ ICE";\n')
        self.assertEqual([(f.rule, f.severity) for f in findings], [("formula", "error")])
        self.assertIn(": CO2.", findings[0].message)

    def test_a_subscript_that_counts_no_atoms_stays_a_glyph_error(self) -> None:
        self.assertEqual(self.label("N₀ SCALE"), [("glyph", "error")])

    def test_formulas_on_the_line_pass(self) -> None:
        for text in ("CO2 ICE", "Mg2SiO4 DUST", "NH4SH CLOUD", "NaCl BRINE", "Mg(ClO4)2 BRINE", "Fe+ LINES", "H− OPACITY", "e− DENSITY"):
            with self.subTest(text):
                self.assertEqual(self.label(text), [])

    def test_a_raised_sign_goes_on_the_line(self) -> None:
        findings = self.lint('export const LABEL = "Fe⁺ LINES · e⁻ DENSITY";\n')
        self.assertEqual(sorted(f.rule for f in findings), ["formula", "formula"])
        self.assertIn(": Fe+.", " ".join(f.message for f in findings))
        self.assertIn("e−", " ".join(f.message for f in findings))

    def test_a_unit_exponent_is_no_ion(self) -> None:
        self.assertEqual(self.label("RATE K⁻¹"), [("glyph", "error")])


class Ions(Fixture):
    def test_a_raised_charge_number_is_an_error(self) -> None:
        self.assertEqual(self.label("Fe²⁺ IONS"), [("ion-charge", "error")])

    def test_a_charge_number_on_the_line_is_a_check(self) -> None:
        (finding,) = self.lint('export const LABEL = "Fe3+ LINES";\n')
        self.assertEqual((finding.rule, finding.severity, finding.section), ("ion-charge", "check", "Voice and nomenclature"))
        self.assertIn("3 atoms of Fe", finding.message)

    def test_singly_charged_molecular_ions_pass(self) -> None:
        for text in ("H3+ EMISSION", "H2+ LINES", "O2− COLUMN", "N2+ BANDS"):
            with self.subTest(text):
                self.assertEqual(self.label(text), [])

    def test_a_doubled_sign_is_a_check(self) -> None:
        self.assertEqual(self.label("Ca++ LINES"), [("ion-charge", "check")])


class Isotopes(Fixture):
    def test_a_raised_mass_number_is_an_error(self) -> None:
        (finding,) = self.lint('export const LABEL = "³He FRACTION";\n')
        self.assertEqual((finding.rule, finding.severity), ("isotope", "error"))
        self.assertIn("He-3", finding.message)

    def test_a_mass_number_b612_lacks_is_named_once(self) -> None:
        self.assertEqual(self.label("⁴He FRACTION"), [("isotope", "error")])

    def test_the_hyphen_form_and_a_unit_exponent_pass(self) -> None:
        for text in ("He-3 FRACTION", "C-14 AGE", "AREA 12 km²", "FLUX 3.20E-12 W/m²"):
            with self.subTest(text):
                self.assertEqual(self.label(text), [])


class StateSymbols(Fixture):
    def test_a_state_symbol_after_a_formula_is_an_error(self) -> None:
        for text in ("CH4(l) SEAS", "NaCl(aq) BRINE", "H2O(s) CAP"):
            with self.subTest(text):
                self.assertEqual(self.label(text), [("state-symbol", "error")])

    def test_one_symbol_may_carry_a_subscript_so_it_is_a_check(self) -> None:
        self.assertEqual(self.label("P(s) SURFACE"), [("state-symbol", "check")])

    def test_a_phase_in_words_passes(self) -> None:
        self.assertEqual(self.label("liquid CH4 seas"), [])


class FormulaCase(Fixture):
    def test_a_formula_whose_case_was_changed_is_a_check(self) -> None:
        for text in ("MG2SIO4 DUST", "FE2O3 RICH", "the H2o column", "Ch4 haze"):
            with self.subTest(text):
                self.assertEqual(self.label(text), [("formula-case", "check")])

    def test_co_for_carbon_monoxide_is_a_check(self) -> None:
        for text in ("Co2 ice", "Co ICE"):
            with self.subTest(text):
                self.assertEqual(self.label(text), [("formula-case", "check")])

    def test_cobalt_and_designations_pass(self) -> None:
        for text in ("Fe, Ni and Co", "B612 MONO", "HIP12345", "RTX3080", "UHD620", "R08.T5.c", "CO2 ICE"):
            with self.subTest(text):
                self.assertEqual(self.label(text), [])

    def test_upper_casing_a_substances_name_in_code_is_a_check(self) -> None:
        for source in ("const t = substanceName(key).toUpperCase();\n", "const t = layer.material.toUpperCase();\n"):
            with self.subTest(source):
                self.assertEqual(self.rules(source), [("formula-case", "check")])

    def test_the_label_form_passes(self) -> None:
        self.assertEqual(self.rules('const t = substanceName(key, "upper");\n'), [])


class Fractions(Fixture):
    def test_a_qualified_per_cent_is_an_error(self) -> None:
        for text in ("aqueous H2SO4, 75 wt%", "H2SO4 75 % w/w", "H2SO4 75% by mass", "CH4 1.8 ppmv", "75 percent by volume"):
            with self.subTest(text):
                self.assertEqual(self.label(text), [("fraction", "error")])

    def test_the_message_cites_voice_and_nomenclature(self) -> None:
        (finding,) = self.lint('export const LABEL = "75 wt% acid";\n')
        self.assertEqual(finding.section, "Voice and nomenclature")
        self.assertIn("mass fraction 75 %", finding.message)

    def test_a_per_cent_with_no_space_is_a_check(self) -> None:
        self.assertEqual(self.rules("export const note = (n: number) => `crystals carry ${n}% of the light`;\n"), [("fraction", "check")])

    def test_a_named_fraction_and_css_lengths_pass(self) -> None:
        self.assertEqual(self.label("aqueous H2SO4, mass fraction 75 %"), [])
        self.assertEqual(self.rules('export const across = "calc(100% - 2rem) translate(-50%)";\n'), [])


class LettersAndNotation(Fixture):
    def test_a_symbol_spelled_letter_by_letter_is_a_check(self) -> None:
        for text in ("delta phi 0.2 rad", "tau 0.5 at 550 nm", "gamma = 1.3", "DELTA T 12 K"):
            with self.subTest(text):
                self.assertEqual(self.label(text), [("letter-name", "check")])

    def test_the_literatures_own_letter_names_pass(self) -> None:
        for text in ("DELTA-V REMAINING", "delta v 12 m/s", "H-alpha 656.3 nm", "oxygen's gamma band", "Alpha Centauri"):
            with self.subTest(text):
                self.assertEqual(self.label(text), [])

    def test_a_caret_power_and_a_typed_relation_are_checks(self) -> None:
        self.assertEqual(self.label("1.2 x 10^-4 W"), [("notation", "check")])
        self.assertEqual(self.label("RANGE <= 12 km"), [("notation", "check")])


class Spaces(Fixture):
    def test_a_thin_space_is_a_space_error(self) -> None:
        (finding,) = self.lint('export const LABEL = "12\u2009km";\n')
        self.assertEqual((finding.rule, finding.severity), ("space", "error"))
        self.assertIn("THIN SPACE", finding.message)
        self.assertIn("U+0020", finding.message)

    def test_an_entity_in_jsx_text_is_decoded_first(self) -> None:
        self.assertEqual(self.rules("export const A = () => <p>12&thinsp;km</p>;\n", "displays/Probe.tsx"), [("space", "error")])

    def test_the_no_break_space_passes(self) -> None:
        self.assertEqual(self.label("12\u00a0km"), [])


class Emoji(Fixture):
    def test_the_message_follows_the_drawn_glyph_rule(self) -> None:
        (finding,) = self.lint('export const LABEL = "\U0001f680 LAUNCH";\n')
        self.assertEqual(finding.rule, "emoji")
        self.assertIn("name the thing in words", finding.message)
        self.assertIn("unit symbol or a key's own sign", finding.message)
        self.assertNotIn("draw symbols as SVG", finding.message)


class DeveloperText(Fixture):
    def test_the_smoke_pages_skip_the_typography_rules(self) -> None:
        for name in ("smoke/probe.ts", "smoke/spike.ts"):
            with self.subTest(name):
                self.assertEqual(self.rules('export const LABEL = "CO(2) at 75 wt%";\n', name), [])

    def test_the_descent_spike_is_a_display_and_is_read(self) -> None:
        self.assertEqual(self.rules('export const LABEL = "CO(2) ICE";\n', "view/spike/probe.ts"), [("formula", "error")])

    def test_the_older_rules_still_read_the_developer_pages(self) -> None:
        self.assertEqual(self.rules('export const LABEL = "CO₂ ICE";\n', "smoke/probe.ts"), [("glyph", "error")])

    def test_provenance_fields_skip_them(self) -> None:
        sources = (
            'const a = { source: "CO(2) at 75 wt% (Smith 2020)" };\n',
            'const a = { basis: "tau 0.5, ³He from Smith 2020" };\n',
            'const a = {\n  evidence:\n    "the first part " +\n    "CO(2) at 75 wt%",\n};\n',
        )
        for source in sources:
            with self.subTest(source):
                self.assertEqual(self.rules(source), [])

    def test_identifier_fields_and_shader_sources_skip_them(self) -> None:
        sources = (
            'const a = { key: "H2+e-", what: "the free-free lines" };\n',
            "const wgsl = `@group(0) @binding(0) var<storage, read_write> data : array<u32>; @compute @workgroup_size(64) fn main(@builtin(global_invocation_id) id : vec3<u32>) {}`;\n",
        )
        for source in sources:
            with self.subTest(source):
                self.assertEqual(self.rules(source), [])

    def test_a_developer_messages_chain_is_skipped(self) -> None:
        self.assertEqual(self.rules('console.warn(\n  "the first part " +\n    "CO(2) at 75 wt%",\n);\n'), [])

    def test_comments_and_test_names_are_never_read(self) -> None:
        self.assertEqual(self.rules('/** CO(2) at 75 wt%, ³He. */\nit("CO(2) at 75 wt%", () => {});\n'), [])


class SubstanceKeys(Fixture):
    def test_a_key_printed_in_a_displayed_string_is_a_check(self) -> None:
        (finding,) = self.lint("export const note = (material: string) => `no ${material} on file`;\n")
        self.assertEqual((finding.rule, finding.severity, finding.section), ("substance-key", "check", "Voice and nomenclature"))
        self.assertIn("'${material}'", finding.message)

    def test_every_kind_of_name_is_read(self) -> None:
        source = "export const n = `${layer.material}: a ${absorber.species} and ${row?.substance} drawn`;\n"
        self.assertEqual(self.rules(source), [("substance-key", "check")] * 3)

    def test_names_indexes_comparisons_and_counts_pass(self) -> None:
        sources = (
            "export const n = `${substanceName(layer.material)}: a fallback`;\n",
            'export const n = `${substanceName(layer.material, "upper")} ICE`;\n',
            "export const n = `${PHASE_WORDS[layer.material]} phase`;\n",
            'export const n = `${layer.material === "H2O" ? "ice" : "dust"} mode`;\n',
            "export const n = `${layers.map((species) => substanceName(species)).join(\", \")} drawn`;\n",
            "export const n = `${a.species.length} species drawn`;\n",
        )
        for source in sources:
            with self.subTest(source):
                self.assertEqual(self.rules(source), [])

    def test_developer_messages_keys_and_developer_pages_pass(self) -> None:
        self.assertEqual(self.rules("throw new Error(`no ${material} on file`);\n"), [])
        self.assertEqual(self.rules("export const k = `${material}-${phase}`;\n"), [])
        self.assertEqual(self.rules("export const n = `no ${material} on file`;\n", "smoke/probe.ts"), [])
        self.assertEqual(self.rules("export const a = { source: `the ${species} table of Smith 2020` };\n"), [])

    def test_a_key_through_a_string_method_is_still_printed(self) -> None:
        source = "export const n = `${layer.material.toUpperCase()} ICE drawn`;\n"
        self.assertEqual(self.rules(source), [("formula-case", "check"), ("substance-key", "check")])

    def test_a_nested_template_is_read_once(self) -> None:
        source = "export const r = `${note ?? `${layer.material}: bare`} (at 550 nm)`;\n"
        self.assertEqual(self.rules(source), [("substance-key", "check")])

    def test_a_jsx_child_is_read(self) -> None:
        name = "displays/Probe.tsx"
        self.assertEqual(self.rules("export const A = () => <td>{row.species}</td>;\n", name), [("substance-key", "check")])
        self.assertEqual(self.rules("export const A = () => <td>{substanceName(row.species)}</td>;\n", name), [])
        keyed = "export const A = () => <ul>{rows.map((row) => <li key={row.species}>{substanceName(row.species)}</li>)}</ul>;\n"
        self.assertEqual(self.rules(keyed, name), [])

    def test_it_runs_before_lib_substances_exists_and_says_so(self) -> None:
        (before,) = self.lint("export const n = `no ${material} on file`;\n")
        self.assertIn("once it lands", before.message)
        module = self.root / ux_lint.SUBSTANCE_MODULE
        module.parent.mkdir(parents=True, exist_ok=True)
        module.write_text('export function substanceName(key: string): string {\n  return key;\n}\n', encoding="utf-8")
        (after,) = self.lint("export const n = `no ${material} on file`;\n")
        self.assertIn("substanceName() (lib/substances.ts)", after.message)
        self.assertNotIn("once it lands", after.message)


class ReviewedForms(Fixture):
    """The forms the first review found misread or missed."""

    def test_ratios_of_like_units_pass(self) -> None:
        for text in ("GAIN 12 V/V", "EFFICIENCY 0.42 W/W", "STRAIN 0.002 m/m", "RATIO 3 mol/mol"):
            with self.subTest(text):
                self.assertEqual(self.label(text), [])

    def test_more_qualified_per_cents_are_errors(self) -> None:
        for text in ("75 weight percent acid", "75 mass % acid", "CH4 2 ppm by volume", "75 % m/m acid"):
            with self.subTest(text):
                self.assertEqual(self.label(text), [("fraction", "error")])

    def test_an_unspaced_per_cent_shows_its_whole_number(self) -> None:
        (finding,) = self.lint('export const LABEL = "raise it 47.5% above that";\n')
        self.assertIn("'47.5%'", finding.message)

    def test_an_underscore_subscript_is_a_check_and_not_a_key(self) -> None:
        (finding,) = self.lint('export const LABEL = "its c_p near 100 K";\n')
        self.assertEqual((finding.rule, finding.severity), ("notation", "check"))
        self.assertIn("c(p)", finding.message)

    def test_one_symbol_with_a_subscript_is_offered_both_forms(self) -> None:
        (finding,) = self.lint('export const LABEL = "V\N{SUBSCRIPT TWO} 12 m/s";\n')
        self.assertEqual((finding.rule, finding.severity), ("formula", "error"))
        self.assertIn("(V2)", finding.message)
        self.assertIn("(V(2))", finding.message)

    def test_formulas_inside_parentheses_are_read(self) -> None:
        cases = {
            "partial pressure (CO(2))": [("formula", "error")],
            "seas (CH4(l))": [("state-symbol", "error")],
            "column (CO\N{SUBSCRIPT TWO} and H\N{SUBSCRIPT TWO}O)": [("formula", "error"), ("formula", "error")],
            "ions (Fe\N{SUPERSCRIPT TWO}\N{SUPERSCRIPT PLUS SIGN})": [("ion-charge", "error")],
            "lines (Fe3+)": [("ion-charge", "check")],
        }
        for text, expected in cases.items():
            with self.subTest(text):
                self.assertEqual(self.label(text), expected)

    def test_a_key_mapped_by_an_arrow_or_joined_is_printed(self) -> None:
        for source in (
            'export const n = `${layers.map((l) => l.material).join(", ")} drawn`;\n',
            'export const n = `${absorber.species.join(", ")} drawn`;\n',
            "export const n = `${String(layer.material)} drawn`;\n",
            "export const n = `no ${materialKey} on file`;\n",
        ):
            with self.subTest(source):
                self.assertEqual(self.rules(source), [("substance-key", "check")])

    def test_a_key_turned_into_words_by_a_call_passes(self) -> None:
        for source in ("export const n = `${phaseWord(layer.material)} phase`;\n", "export const n = `${WORDS.get(layer.material)} phase`;\n"):
            with self.subTest(source):
                self.assertEqual(self.rules(source), [])

    def test_a_displayed_jsx_attribute_is_read(self) -> None:
        name = "displays/Probe.tsx"
        self.assertEqual(self.rules("export const A = () => <td title={row.species}>x</td>;\n", name), [("substance-key", "check")])
        self.assertEqual(self.rules("export const A = () => <td aria-label={substanceName(row.species)}>x</td>;\n", name), [])

    def test_a_key_with_an_underscore_is_an_error(self) -> None:
        (finding,) = self.lint('export const LABEL = "no mars_dust on file";\n')
        self.assertEqual((finding.rule, finding.severity, finding.section), ("code-name", "error", "Voice and nomenclature"))

    def test_a_mass_number_on_the_line_before_the_symbol_is_a_check(self) -> None:
        for text in ("3He FRACTION", "40Ar AGE"):
            with self.subTest(text):
                self.assertEqual(self.label(text), [("isotope", "check")])
        for text in ("4K DISPLAY", "12V BUS", "B9.5Iab STAR", "G2V STAR"):
            with self.subTest(text):
                self.assertEqual(self.label(text), [])

    def test_an_ion_minus_typed_as_a_hyphen_is_a_check(self) -> None:
        self.assertEqual(self.label("H- OPACITY"), [("ion-charge", "check")])
        for text in ("Fe- and Mg-rich basalt", "T-00:04:12", "X-RAY BINARY"):
            with self.subTest(text):
                self.assertEqual(self.label(text), [])

    def test_other_spellings_of_a_power_of_ten_are_checks(self) -> None:
        for text in ("1.2 x 10-6 W", "10\N{SUPERSCRIPT THREE} km", "1e-3 at 550 nm"):
            with self.subTest(text):
                self.assertEqual(self.label(text), [("notation", "check")])

    def test_abbreviations_that_are_letters_names_pass(self) -> None:
        for text in ("ETA 00:04:12", "PSI 30 TANK"):
            with self.subTest(text):
                self.assertEqual(self.label(text), [])

    def test_designators_pass_and_the_registrys_formulas_are_held_to_their_case(self) -> None:
        for text in ("TANK2 PRESSURE", "BAY3 OPEN", "HE FRACTION"):
            with self.subTest(text):
                self.assertEqual(self.label(text), [])
        fixture = self.root / ux_lint.SUBSTANCE_FIXTURE
        fixture.parent.mkdir(parents=True, exist_ok=True)
        fixture.write_text('{"substances": [{"key": "He"}, {"key": "CH4"}, {"key": "mars_dust"}]}\n', encoding="utf-8")
        ux_lint.registry_formulas.cache_clear()
        self.addCleanup(ux_lint.registry_formulas.cache_clear)
        self.assertEqual(self.label("HE FRACTION"), [("formula-case", "check")])
        self.assertEqual(self.label("He FRACTION"), [])


class SignsAndTemperatures(Fixture):
    """decision-ux-numbers-names, rulings 1 and 2: checks until the guide's text lands."""

    def test_the_minus_sign_before_a_number_is_a_check(self) -> None:
        (finding,) = self.lint('export const LABEL = "within +0.081 and \N{MINUS SIGN}0.022";\n')
        self.assertEqual((finding.rule, finding.severity, finding.section), ("sign", "check", "Numbers, units and time"))
        self.assertIn("decision-ux-numbers-names", finding.message)

    def test_the_minus_sign_of_an_ion_an_operator_or_a_key_passes(self) -> None:
        for text in ("H\N{MINUS SIGN} OPACITY", "e\N{MINUS SIGN} DENSITY", "|m \N{MINUS SIGN} 1| 0.2", "+/\N{MINUS SIGN} ZOOM"):
            with self.subTest(text):
                self.assertEqual(self.label(text), [])

    def test_a_temperature_in_celsius_is_a_check(self) -> None:
        (finding,) = self.lint('export const LABEL = "liquid H2O at 25 \N{DEGREE SIGN}C";\n')
        self.assertEqual((finding.rule, finding.severity), ("temperature", "check"))

    def test_kelvin_and_angles_pass(self) -> None:
        for text in ("liquid H2O at 298 K", "BEARING 047\N{DEGREE SIGN} COREWARD", "FOV 60\N{DEGREE SIGN}"):
            with self.subTest(text):
                self.assertEqual(self.label(text), [])


class Helpers(unittest.TestCase):
    def test_the_natural_elements(self) -> None:
        self.assertEqual(len(ux_lint.ELEMENTS), 92)
        self.assertNotIn("No", ux_lint.ELEMENTS)

    def test_formula_symbols(self) -> None:
        self.assertEqual(ux_lint.formula_symbols("Mg2SiO4", recased=False), 3)
        self.assertEqual(ux_lint.formula_symbols("MG2SIO4", recased=False), 0)
        self.assertGreaterEqual(ux_lint.formula_symbols("MG2SIO4", recased=True), 2)
        self.assertEqual(ux_lint.formula_symbols("H20", recased=False), 1)
        self.assertEqual(ux_lint.formula_symbols("H02", recased=False), 0)  # a count has no leading zero


if __name__ == "__main__":
    unittest.main()
