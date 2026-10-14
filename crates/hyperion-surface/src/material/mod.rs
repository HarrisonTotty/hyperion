//! The authoritative ground: its material class, a form of one of the body's substances, and the
//! physical properties landing and the consoles read (plan R10, T1).
//!
//! The brainstorm makes the class authoritative ("The line between truth and decoration"): this
//! crate decides it at every point, beside the height and under the same determinism discipline,
//! and a shader only blends what a worker baked. A material is a form ([`MaterialClass`]) and a
//! substance, the index of an entry in the body's
//! [`MaterialPalette`](crate::field::MaterialPalette) ([`Material`]; decision-composition §1.8), so
//! that one set of forms serves every composition
//! the generator produces. Its properties ([`properties()`]) join the form's rows for the entry's
//! mechanics family ([`form_row`]) to the entry's values, which the server resolves from plan 14's
//! registry: the albedo in three bands, the photometric law ([`PhaseLaw`], whose codes this module
//! assigns), the grain density, the transition temperature. The bearing capacity is a function of
//! the footing and the gravity ([`bearing_capacity`]), and the pad friction of the temperature
//! where it depends on it ([`interface_friction`]).
//!
//! Every figure carries its source ([`Citation`]); a figure without one is absent, and a console
//! shows it missing, never invented (Design note 2).
//!
//! Nothing here draws from a stream, so the module opens no domain tag. Today it holds R10.T1.a's
//! class set and property table; the classifier, `material_at`, is R10.T1.b's.

mod bearing;
mod class;
mod properties;

pub use bearing::{BearingFactors, bearing_capacity, ultimate_bearing_capacity};
pub use class::{Material, MaterialClass};
pub use properties::{
    AlbedoSpan, Citation, Figure, FormRow, FrictionModel, LOMMEL_SEELIGER_NORMAL_ALBEDO_RATIO,
    MaterialProperties, MohrCoulomb, PhaseLaw, PhaseLawOrigin, ResolvePropertiesError, ShearLayer,
    UNASSIGNED_PHASE_ROW, directional_hemispherical_albedo, form_row, interface_friction,
    limb_parameter, properties,
};

use hyperion_base::units::Radians;

/// The slope above which loose material does not rest and the ground is exposed rock: 40°, the
/// angle loose slopes relax to, inside the 35–45° of the literature (plan R10, Design note 1).
///
/// Kleinhans et al. 2011 (JGR 116, E11004) found angular materials' time-averaged angle about 40°
/// (§40, p. 8) at 0.1, 0.38 and 1 g, their static angle rising about 5° and their dynamic angle
/// falling about 10° as gravity falls from 1 to 0.1 g (§39, p. 7), while Atwood-Stone and McEwen
/// 2013 (GRL 40, 2929, abstract) found the avalanche slopes of active Martian dunes at 30–34°,
/// against Earth's 30–35°; since the two disagree on gravity's effect, the threshold does not scale
/// with it. A slope may stand steeper than this up to the static angle, 41–55° in Kleinhans et
/// al.'s Fig. 9 (p. 10), until it fails, and relaxes to the dynamic angle, 26–42° there. The Lunar
/// Sourcebook gives a dumped pile of lunar soil nearly 40° and a compacted slope about 45°
/// (Carrier, Olhoeft and Mendell 1991, ch. 9, §9.1.10, p. 522).
pub const CLIFF_SLOPE: Radians = Radians::new(40.0_f64.to_radians());

/// The half-width of the visual blend either side of [`CLIFF_SLOPE`]: 5°.
///
/// The band, 35–45°, spans the literature's angles of repose (plan R10, Design note 1; the sources
/// of [`CLIFF_SLOPE`]). Presentation only: the class itself switches at [`CLIFF_SLOPE`].
pub const TRANSITION_HALF_WIDTH: Radians = Radians::new(5.0_f64.to_radians());

/// The slope above which snow does not lie and the ground is exposed rock: 60°.
///
/// "Terrain steeper than 60° is usually avalanche-free. This angle is greater than the angle of
/// repose of new snow. It sloughs off as soon as it is deposited" (Schaerer 1977, Can. Geotech. J.
/// 14, 281, p. 286; the same page notes a few avalanches on slopes steeper than 50°). Snow gives
/// way to `Bedrock` above it (plan R10, Design note 1). It is a terrestrial snow's angle; another
/// condensate's snow takes it too, unmeasured.
pub const SNOW_SHED_SLOPE: Radians = Radians::new(60.0_f64.to_radians());
