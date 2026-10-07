//! The sky an observer sees: which stars the galaxy holds brighter than a limit, how bright and
//! what colour each appears at its retarded time, the unresolved band of the rest, and the
//! naked eye's limit per direction (rendering plan R06).
//!
//! - [`eye`]: the naked eye's threshold against a background (Crumey 2014, eq. 34, with its colour
//!   corrections) and the veiling glare of bright stars (CIE 146:2002), and [`MAX_CUT_V`], the
//!   deepest cut a sky is asked to.
//! - [`colour`]: a star's colour, photopic flux, scotopic ratio, camera band term, reddening and
//!   bake spectrum from its temperature and gravity (the fitted `star_colour` table).
//! - [`disc`]: the observer's own stars as limb-darkened discs (the fitted `limb_darkening`
//!   table).
//! - [`photometry`]: a star's absolute V and colour as the sky reads them, with plan 06's
//!   interims for protostars and white dwarfs.
//! - [`luminosity`]: the cumulative luminosity function per component and layer, the light and
//!   count of the stars a census does not list.
//! - [`binary_light`]: what pair evolution changes in those functions' light and counts, the
//!   fitted `sky_binary_light_*` tables and the sampling they are fitted from.
//! - [`envelope`]: the brightest V any star of a mass can reach, which bounds a system before its
//!   stars are generated.
//! - [`caps`]: how far out the census looks in each layer.
//! - [`census`]: the stars an observer sees brighter than a cut: the query, its plan and the
//!   per-cell cache.
//! - [`band`]: the light of the stars the census did not list, along rays from the observer: the
//!   unresolved band.
//! - [`limits`]: the naked eye's limit in every direction of the band, against its light and the
//!   listed stars' glare: the limit map.
//!
//! Everything here only reads the galaxy: nothing changes generated output, and nothing draws a
//! random word, except [`binary_light`]'s fit sampling, which generates systems of its own galaxy
//! offline, as the census generates the galaxy's.

pub mod band;
pub mod binary_light;
pub mod caps;
pub mod census;
pub mod colour;
pub mod disc;
pub mod envelope;
pub mod eye;
pub mod limits;
pub mod luminosity;
pub mod phase;
pub mod photometry;
#[cfg(test)]
mod testing;

pub use eye::{EyeObserver, MAX_CUT_V, REFERENCE_SP_RATIO};
