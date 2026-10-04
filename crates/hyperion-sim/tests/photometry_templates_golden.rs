//! The phase-curve templates' laws (plan 14, P14.T47.d), pinned in
//! `golden/photometry/templates.golden` for the client's `lawFor` and `discIntegratedPhase`
//! (rendering plan R07.T4.b), which are tested against it.
//!
//! For every template of `PHASE_TEMPLATES`, in its order, and for each Lommel–Seeliger share L in
//! {0, 0.5, 1}, the file holds the law's phase function `Φ_c(α)` ([`channel_phase`]) at every whole
//! degree from 0° to 180° and its phase integral q ([`phase_integral`]), each in B, V and R at the
//! template's exponents s.
//!
//! # Line format
//!
//! After the `# generator_version` header and a comment line:
//!
//! ```text
//! template <name> share=<the row's L> valid_to=<rad> s=<b>,<v>,<r>
//! l=<L> q=<b>,<v>,<r>
//! a=<degrees> phi=<b>,<v>,<r>
//! ```
//!
//! one `template` line, then for each L one `l` line and 181 `a` lines. A name is the wire's
//! `PhaseTemplateDto` string (`airless_ice`). Every float is Rust's shortest round-trip decimal,
//! which `parseFloat` reads back to the same `f64`; α in radians is degrees × π ÷ 180.

use std::f64::consts::PI;

use hyperion_sim::GENERATOR_VERSION;
use hyperion_sim::planetary::derive::photometry::{
    Bands, PHASE_TEMPLATES, PhaseTemplate, channel_phase, phase_integral,
};
use hyperion_sim::units::Radians;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

/// The shares pinned.
const SHARES: [f64; 3] = [0.0, 0.5, 1.0];

/// A template's wire name.
fn name(template: PhaseTemplate) -> &'static str {
    match template {
        PhaseTemplate::Moon => "moon",
        PhaseTemplate::Mercury => "mercury",
        PhaseTemplate::Mars => "mars",
        PhaseTemplate::Venus => "venus",
        PhaseTemplate::Earth => "earth",
        PhaseTemplate::Jupiter => "jupiter",
        PhaseTemplate::Saturn => "saturn",
        PhaseTemplate::Uranus => "uranus",
        PhaseTemplate::Neptune => "neptune",
        PhaseTemplate::AirlessIce => "airless_ice",
        PhaseTemplate::Snowball => "snowball",
        PhaseTemplate::Magma => "magma",
    }
}

/// `x` as the file writes it: finite, so that its shortest round-trip decimal stands for its bits
/// alone (a NaN's would not).
fn float(x: f64) -> String {
    assert!(x.is_finite(), "a pinned value is finite: {x}");
    format!("{x:?}")
}

/// `f` in each band, as the file writes three values.
fn bands(s: Bands, f: impl Fn(f64) -> f64) -> String {
    format!("{},{},{}", float(f(s.b)), float(f(s.v)), float(f(s.r)))
}

/// P14.T47.d: every template's law is pinned at 1° steps for L in {0, 0.5, 1}, with its q, and
/// at the row's own L the pinned q is the row's literal to 10⁻⁹ (decision-p14-phase-j, A6).
#[test]
fn the_phase_templates_are_pinned_for_the_client() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    w.line("# every template's law at 1 degree steps for L in {0, 0.5, 1}, and its q, in B, V, R");
    for row in &PHASE_TEMPLATES {
        assert!(
            SHARES
                .iter()
                .any(|l| l.total_cmp(&row.lunar_lambert_share).is_eq()),
            "{:?}'s share {} is pinned, so its q is checked against the row's",
            row.template,
            row.lunar_lambert_share
        );
        let template = row.template;
        let s = row.exponents;
        w.line(&format!(
            "template {} share={} valid_to={} s={}",
            name(template),
            float(row.lunar_lambert_share),
            float(row.valid_to.value()),
            bands(s, |s| s)
        ));
        for l in SHARES {
            let q = |s: f64| phase_integral(template, l, s);
            if l.total_cmp(&row.lunar_lambert_share).is_eq() {
                for (pinned, literal) in [
                    (q(s.b), row.phase_integral.b),
                    (q(s.v), row.phase_integral.v),
                    (q(s.r), row.phase_integral.r),
                ] {
                    assert!(
                        (pinned - literal).abs() < 1e-9,
                        "{template:?}: q {pinned} against the row's {literal}"
                    );
                }
            }
            w.line(&format!("l={} q={}", float(l), bands(s, q)));
            for degrees in 0..=180_u32 {
                let alpha = Radians::new(f64::from(degrees) * PI / 180.0);
                let phi = |s: f64| channel_phase(template, l, s, alpha);
                w.line(&format!("a={degrees} phi={}", bands(s, phi)));
            }
        }
    }
    golden!("photometry/templates", w.as_str());
}
