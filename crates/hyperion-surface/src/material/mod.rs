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
//! assigns, and which a consolidated form overrides with Lambert's: [`PhotometricForm`]), the grain
//! density, the transition temperature. The bearing capacity is a function of the footing, the
//! gravity and the ground's slope ([`bearing_capacity`] on level ground,
//! [`bearing_capacity_on_slope`]), and the pad friction of the temperature where it depends on it
//! ([`interface_friction`]).
//!
//! Every figure carries its source ([`Citation`]); a figure without one is absent, and a console
//! shows it missing, never invented (Design note 2).
//!
//! The classifier ([`material_at`], R10.T1.b) decides the material at a point from the same inputs
//! as the height: the finest level's slope, the coarse cell's record and climate, and the header's
//! palette, by an ordered list of rules (Design note 1). The class weights a patch bakes
//! ([`weights_at`], R10.T2) are that class at the finest level and, at a coarser one, the expected
//! share of each finest-level class over a texel's footprint (Design note 3). The class photometry
//! ([`albedo_scale`], R10.T10.a) is the one scale on a body's class albedos that makes its scaled
//! classes, over their area fractions ([`class_fractions`]), give its geometric albedo (Design
//! note 8). The space-weathering maturity's time law ([`maturity_after`], [`body_maturity`],
//! [`LUNAR_MATURATION_TIME`]; plan 14's P14.T51.d, decision-p14-t51d-weathering §2.1) is the one
//! the sim's surface modifiers and R10.T1.c's crater fading share; nothing on the record calls it
//! until P14.T54.b. The darkening that maturity drives ([`weathered_normal_albedo`],
//! [`weathered_spherical_albedo`], [`weathered_hemispherical_albedos`], [`weathered_entry_bvr`],
//! [`npfe_optical_depth`]; R10.T1.d, decision-p14-t51d-weathering §1) is Hapke 2001's
//! nanophase-iron absorption inside the equivalent slab, applied to an entry's fresh albedo in its
//! reference form by `hyperion-fit`'s fitted table, the one law the server's albedo scale, the
//! sim's area mix and R10.T1.c share. R10.T1.c applies it (decision-r10-t1c-maturity): a
//! material's exposed albedo ([`exposed_properties`]) is its properties' weathered to the ground's
//! exposure ([`GroundExposure`]) and blended with the organic cover where its entry takes one
//! ([`takes_organic_cover`]), the one function the server's solve ([`exposed_albedo_scale`]) and
//! the worker's photometry, class map and point query call; and the maturity at a point
//! ([`maturity_at`]) is the header's less the largest deficit a young crater's fresh ejecta leave
//! there ([`crater_maturity`]), at its fresh share of the crater's wear, its excavated share
//! ([`excavated_share`], [`reworked_depth`], [`EXCAVATION_DEPTH_RATIO`],
//! [`REWORKING_DEPTH_AT_ONE_MYR`], [`REWORKING_EXPONENT`]) and its halo ([`halo_share`],
//! [`HALO_LENGTH_RIM_RADII`], [`HALO_EXTENT_RIM_RADII`]).
//!
//! Nothing here draws from a stream, so the module opens no domain tag.

mod albedo;
mod bearing;
mod class;
mod classify;
#[cfg(test)]
mod golden;
mod maturity;
mod properties;
mod weathering;
mod weights;

pub use albedo::{
    ALBEDO_SCALE_FLOOR, AlbedoScale, BuildClassFractionsError, ClassFractions,
    SolveAlbedoScaleError, albedo_scale, class_fractions, exposed_albedo_scale, scale_range,
    scaled_normal_albedo,
};
pub use bearing::{
    BearingFactors, bearing_capacity, bearing_capacity_on_slope, ultimate_bearing_capacity,
    ultimate_bearing_capacity_on_slope,
};
pub use class::{Material, MaterialClass, PhotometricForm};
pub(crate) use classify::IceCuts;
pub use classify::{GroundMaterial, MASSIVE_ICE_SHARE, THIN_ATMOSPHERE_PRESSURE, material_at};
// One line, unwrapped: until P14.T54.b deletes it, the sim's tripwire
// (`modifiers_nothing_on_the_record_reads_the_modifiers_yet`) allows the time law's names outside
// `material/maturity.rs` on this module's one re-export line of that module alone.
#[rustfmt::skip]
pub use maturity::{EXCAVATION_DEPTH_RATIO, GroundExposure, HALO_EXTENT_RIM_RADII, HALO_LENGTH_RIM_RADII, LUNAR_MATURATION_TIME, REWORKING_DEPTH_AT_ONE_MYR, REWORKING_EXPONENT, body_maturity, crater_maturity, excavated_share, exposed_properties, halo_share, maturity_after, maturity_at, reworked_depth, takes_organic_cover};
pub use properties::{
    AlbedoSpan, BRIGHT_FROST_LIMB_PARAMETER, BRIGHT_FROST_NORMAL_ALBEDO, Citation, Figure, FormRow,
    FrictionModel, LOMMEL_SEELIGER_NORMAL_ALBEDO_RATIO, LUNAR_LIMB_ONSET_NORMAL_ALBEDO,
    MaterialProperties, MohrCoulomb, PARTICULATE_NORMAL_ALBEDO_CEILING, PhaseLaw, PhaseLawOrigin,
    ResolvePropertiesError, ShearLayer, UNASSIGNED_PHASE_ROW, directional_hemispherical_albedo,
    form_row, interface_friction, limb_parameter, properties,
};
pub use weathering::{
    FEO_MOLAR_MASS_G_PER_MOL, ICE_HOST_INDEX, IRON_DENSITY_KG_M3, IRON_MOLAR_MASS_G_PER_MOL,
    REGOLITH_HOST_INDEX, npfe_optical_depth, weathered_entry_bvr, weathered_hemispherical_albedos,
    weathered_normal_albedo, weathered_spherical_albedo,
};
pub use weights::{ClassWeights, MAX_CLASSES, SLOPE_LAG, WEIGHT_TOTAL, weights_at};
pub(crate) use weights::{kept_omitted_rms, weights_with_sample};

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

/// The half-width of the visual blend either side of [`CLIFF_SLOPE`] and of [`SNOW_SHED_SLOPE`]: 5°
/// ([`weights_at`] at the finest level, R10.T2).
///
/// The cliff's band, 35–45°, spans the literature's angles of repose (plan R10, Design note 1; the
/// sources of [`CLIFF_SLOPE`]). The snow's, 55–65°, is presentation alone, loosely Schaerer 1977's
/// few avalanches above 50° and none as a rule above 60° (p. 286). Presentation only: the class
/// itself switches at each threshold.
pub const TRANSITION_HALF_WIDTH: Radians = Radians::new(5.0_f64.to_radians());

/// The slope above which snow does not lie and the ground is exposed rock: 60°.
///
/// "Terrain steeper than 60° is usually avalanche-free. This angle is greater than the angle of
/// repose of new snow. It sloughs off as soon as it is deposited" (Schaerer 1977, Can. Geotech. J.
/// 14, 281, p. 286; the same page notes a few avalanches on slopes steeper than 50°). Snow gives
/// way to `Bedrock` above it (plan R10, Design note 1). It is a terrestrial snow's angle; another
/// condensate's snow takes it too, unmeasured.
pub const SNOW_SHED_SLOPE: Radians = Radians::new(60.0_f64.to_radians());

// The classifier's steepness nests the two thresholds (ground above the snow's is above the
// cliff's), and the weights' two blends do not meet: both hold only while the snow's threshold is
// at least two half-widths steeper than the cliff's.
const _: () = assert!(
    SNOW_SHED_SLOPE.value() - CLIFF_SLOPE.value() >= 2.0 * TRANSITION_HALF_WIDTH.value(),
    "the slope thresholds' blends meet"
);
