//! The gases' formation constants' golden (plan 14, P14.T49.d).
//!
//! `tests/golden/substance/formation.golden` pins, by their bits at the generator version, what
//! `substance::formation` gives for every gas row, in registry order: a row with no record by
//! that fact; for the others, [`formation_range`]'s ends, and [`log10_formation_constant`] at
//! those ends, at 500, 1,000, 2,500 and 5,000 K, and at each condensed reference state's switch
//! to its gas (`tables::thermo`'s `<EL>_BOILING_K`) and the next `f64` above it, where the sim
//! branches between the condensed phases and the gas. Then [`Reaction::log10_equilibrium_constant`]
//! of the module test's four reactions, each in its declared order, at 500, 1,500, 3,000 and
//! 5,000 K. A temperature outside a row's range pins `none`. Nothing on the record reads these
//! functions until P14.T52.a (decision-bump-plan-after-22 §1.3), so until then a refit of
//! `thermo_equilibrium` re-blesses this golden at the version in force
//! (decision-p14-t49h-liquids §5), each re-bless citing that.

use hyperion_sim::GENERATOR_VERSION;
use hyperion_sim::substance::{
    self, Reaction, SubstanceId, by_key, formation_range, log10_formation_constant,
};
use hyperion_sim::tables::thermo;
use hyperion_sim::units::Kelvin;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

/// The fixed temperatures every row is pinned at, K.
const TEMPERATURES: [f64; 4] = [500.0, 1_000.0, 2_500.0, 5_000.0];

/// The condensed reference states' switches, in `Element::ALL`'s order: each item's name and
/// value, K.
const SWITCHES: [(&str, f64); 15] = [
    ("NA_BOILING_K", thermo::NA_BOILING_K),
    ("MG_BOILING_K", thermo::MG_BOILING_K),
    ("AL_BOILING_K", thermo::AL_BOILING_K),
    ("SI_BOILING_K", thermo::SI_BOILING_K),
    ("P_BOILING_K", thermo::P_BOILING_K),
    ("S_BOILING_K", thermo::S_BOILING_K),
    ("K_BOILING_K", thermo::K_BOILING_K),
    ("CA_BOILING_K", thermo::CA_BOILING_K),
    ("TI_BOILING_K", thermo::TI_BOILING_K),
    ("V_BOILING_K", thermo::V_BOILING_K),
    ("CR_BOILING_K", thermo::CR_BOILING_K),
    ("MN_BOILING_K", thermo::MN_BOILING_K),
    ("FE_BOILING_K", thermo::FE_BOILING_K),
    ("NI_BOILING_K", thermo::NI_BOILING_K),
    ("ZN_BOILING_K", thermo::ZN_BOILING_K),
];

/// `substance::formation`'s test's reactions, each in its declared order: methanation, ammonia's
/// synthesis, sodium's ionisation and H₂S's oxidation by water.
const REACTIONS: [&[(&str, i32)]; 4] = [
    &[("CO", -1), ("H2", -3), ("CH4", 1), ("H2O", 1)],
    &[("N2", -1), ("H2", -3), ("NH3", 2)],
    &[("Na", -1), ("Na+", 1), ("e-", 1)],
    &[("H2S", -1), ("H2O", -2), ("SO2", 1), ("H2", 3)],
];

/// The reactions' temperatures, K.
const REACTION_TEMPERATURES: [f64; 4] = [500.0, 1_500.0, 3_000.0, 5_000.0];

/// Pins `value` under `label`, or `none`.
fn pin(w: &mut GoldenWriter, label: &str, value: Option<f64>) {
    match value {
        Some(value) => w.f64(label, value),
        None => w.line(&format!("{label}: none")),
    }
}

/// Pins row `id`'s formation constant at `t` K under `label`.
fn pin_kf(w: &mut GoldenWriter, label: &str, id: SubstanceId, t: f64) {
    pin(w, label, log10_formation_constant(id, Kelvin::new(t)));
}

#[test]
fn formation_constants_are_pinned() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    w.line("[switches]");
    for (name, value) in SWITCHES {
        w.f64(name, value);
    }
    w.line("[formation]");
    for id in substance::ids().filter(|id| id.substance().gas().is_some()) {
        let key = id.substance().key();
        let Some((lower, upper)) = formation_range(id) else {
            w.line(&format!("{key}: no record"));
            continue;
        };
        let (lower, upper) = (lower.value(), upper.value());
        w.f64(&format!("{key} range lower_k"), lower);
        w.f64(&format!("{key} range upper_k"), upper);
        pin_kf(&mut w, &format!("{key} log10_kf at lower"), id, lower);
        pin_kf(&mut w, &format!("{key} log10_kf at upper"), id, upper);
        for t in TEMPERATURES {
            pin_kf(&mut w, &format!("{key} log10_kf at {t} K"), id, t);
        }
        for (name, value) in SWITCHES {
            pin_kf(&mut w, &format!("{key} log10_kf at {name}"), id, value);
            pin_kf(
                &mut w,
                &format!("{key} log10_kf above {name}"),
                id,
                value.next_up(),
            );
        }
    }
    w.line("[reactions]");
    for species in REACTIONS {
        let ids: Vec<(SubstanceId, i32)> = species
            .iter()
            .map(|&(key, nu)| (by_key(key).expect("the reactions' species are rows"), nu))
            .collect();
        let reaction = Reaction::new(&ids).expect("the reactions balance");
        let name = species
            .iter()
            .map(|(key, nu)| format!("{nu:+} {key}"))
            .collect::<Vec<_>>()
            .join(" ");
        for t in REACTION_TEMPERATURES {
            pin(
                &mut w,
                &format!("{name} = 0: log10_k at {t} K"),
                reaction.log10_equilibrium_constant(Kelvin::new(t)),
            );
        }
    }
    let text = w.finish();
    golden!("substance/formation", &text);
}
