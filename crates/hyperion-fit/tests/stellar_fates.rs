//! The fate table (plan 06, P06.T38.c): each committed panel is exactly what its task would write,
//! and a small fit is the same on one thread and on four.

use std::num::NonZeroUsize;

use hyperion_fit::emit::Workspace;
use hyperion_fit::pipeline::rerender;
use hyperion_fit::task::{find, registry};
use hyperion_fit::tasks::stellar_fates::{Panel, PanelGrid, PanelSpec, fe_h_nodes, fit};
use hyperion_sim::GENERATOR_VERSION;

/// The committed panels, as the sim compiles them.
const COMMITTED: [(&str, &str); 3] = [
    (
        "stellar_fates_low",
        include_str!("../../hyperion-sim/src/tables/stellar_fates_low.rs"),
    ),
    (
        "stellar_fates_mid",
        include_str!("../../hyperion-sim/src/tables/stellar_fates_mid.rs"),
    ),
    (
        "stellar_fates_high",
        include_str!("../../hyperion-sim/src/tables/stellar_fates_high.rs"),
    ),
];

/// Every panel is reproduced byte for byte: some 31,000 full tracks, a few seconds on four
/// threads in release and minutes in a debug build.
#[test]
#[ignore = "slow: the three panels' 31,000 full tracks"]
fn the_committed_panels_are_reproduced() {
    for (name, committed) in COMMITTED {
        let planned = rerender(
            registry(),
            find(name).expect("the panel's task is registered"),
            &Workspace::repository(),
            NonZeroUsize::new(4).expect("four"),
            GENERATOR_VERSION.get(),
        )
        .expect("the panel's task runs");
        if planned.text != committed {
            let line = planned
                .text
                .lines()
                .zip(committed.lines())
                .position(|(a, b)| a != b)
                .map_or_else(|| "the length".to_owned(), |n| format!("line {}", n + 1));
            panic!("tables/{name}.rs differs from the fit at {line}: run `just fit {name}`");
        }
        assert!(!planned.body_changed, "{name}");
    }
}

/// A small panel fits the same on one thread and on four: the chunking cannot move a node or a
/// bound.
#[test]
fn a_small_fit_is_the_same_on_any_thread_count() {
    let spec = PanelSpec {
        panel: Panel::Mid,
        fe_h: fe_h_nodes(0.6),
        grid: PanelGrid {
            log_mass_start: 0.3,
            log_mass_step: 0.05,
            masses: 5,
            etas: vec![-1.0, 0.0, 1.0],
        },
        safety: 3.0,
        max_bound: 0.02,
        max_bound_iron: 0.2,
    };
    let (one, v1) = fit(&spec, NonZeroUsize::MIN).unwrap();
    let (four, v4) = fit(&spec, NonZeroUsize::new(4).unwrap()).unwrap();
    assert_eq!(one, four);
    assert_eq!(v1, v4);
    assert_eq!(one.nodes.len(), 5 * 3 * spec.fe_h.len());
    assert_eq!(one.bounds.len(), 4 * (spec.fe_h.len() - 1));
}
