//! The fate table (plan 06, P06.T38.c): a small fit is the same on one thread and on four.
//!
//! That each committed panel is exactly what its task would write is `hyperion-fit check
//! --rerun-fast`'s, which `just test-slow` runs: the panels are fast tasks.

use std::num::NonZeroUsize;

use hyperion_fit::tasks::stellar_fates::{Panel, PanelGrid, PanelSpec, fe_h_nodes, fit};

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
