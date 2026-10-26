//! The fitting tasks, one module each. Each has a `fit()` that returns its result, a `render()`
//! that turns the result into its table's contents, and a unit struct implementing
//! [`FitTask`](crate::task::FitTask), which [`registry`](crate::task::registry) lists.

pub mod binary_cooling_floor;
pub mod binary_pair_light;
pub mod binary_reach;
pub mod bowl_permanent_shadow;
pub mod chabrier;
pub mod cluster_bh;
pub mod cluster_retention;
pub mod condensate_extinction;
pub mod displaced_forms;
pub mod equipartition;
pub mod giant_cooling;
pub mod helium;
pub mod kick_defaults;
pub mod kick_rank;
pub mod lifetime_bracket;
pub mod limb_darkening;
pub mod liquid_optics;
pub mod mge;
pub mod period_correction;
pub mod pulsars;
pub mod saturation_curves;
pub mod sky_binary_light;
pub mod sky_envelope;
pub mod sky_phase_envelope;
pub mod space_weathering;
pub mod star_colour;
pub mod stellar_fates;
pub mod streams;
pub mod stripping;
pub mod substance_albedos;
pub mod thermo_equilibrium;
pub mod type_ia_delay;
pub mod wd_cooling;
