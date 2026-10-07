//! The fitting tasks, one module each. Each has a `fit()` that returns its result, a `render()`
//! that turns the result into its table's contents, and a unit struct implementing
//! [`FitTask`](crate::task::FitTask), which [`registry`](crate::task::registry) lists.

pub mod binary_reach;
pub mod chabrier;
pub mod cluster_bh;
pub mod cluster_retention;
pub mod displaced_forms;
pub mod equipartition;
pub mod giant_cooling;
pub mod kick_rank;
pub mod limb_darkening;
pub mod mge;
pub mod period_correction;
pub mod pulsars;
pub mod sky_binary_light;
pub mod sky_envelope;
pub mod star_colour;
pub mod stellar_fates;
pub mod stripping;
pub mod type_ia_delay;
pub mod wd_cooling;
