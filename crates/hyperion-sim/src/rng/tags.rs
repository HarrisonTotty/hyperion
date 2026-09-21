//! The single registry of domain tags.
//!
//! Every domain tag in the crate is declared here and nowhere else, so that the collision check,
//! a `const` assertion over this one list, covers them all. The rules:
//!
//! - A tag is never renamed or removed. Its name is hashed into the key of every stream it opens,
//!   so a rename moves every value drawn under it.
//! - A new property group gets a new tag, so that adding it moves nothing already generated.
//! - Each plan adds its tags under its own heading, as `CONST_NAME: Scope = "tag.name";`, in the
//!   task that first opens a stream under them. Names match
//!   `[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)+`.
//! - The tag behind each event tag is an ordinary entry of scope `Event`, which the event-tag
//!   registry in `id/event_tags.rs` names by constant.

use super::domain_tag::domain_tags;

domain_tags! {
    // Plan 01: the determinism foundation.

    /// A stream for tests and golden files, never opened by a generator.
    SELFTEST_STREAM: SelfTest = "selftest.stream";

    /// The key derivation of event tag `0x0001`,
    /// [`SELF_TEST`](crate::id::event_tags::SELF_TEST), which no generator emits.
    EVENT_SELFTEST: Event = "event.selftest";

    // Plan 02: the galaxy model. Every tag has scope `Galaxy` and draws one parameter of
    // `galaxy::params`. Progenitor numbers: the dominant merger 0, the lesser progenitors 1–5,
    // the recent ones from 6.

    // Scalar parameters, each opened with `ObjectKey::galaxy()`.

    /// The galaxy's stellar mass, log-uniform on 3–10 × 10¹⁰ M☉.
    GALAXY_PARAMS_STELLAR_MASS: Galaxy = "galaxy.params.stellar_mass";

    /// The thick disc's share of systems.
    GALAXY_PARAMS_SHARE_THICK: Galaxy = "galaxy.params.share.thick";

    /// The combined share of the bulge and the long bar.
    GALAXY_PARAMS_SHARE_BULGE_BAR: Galaxy = "galaxy.params.share.bulge_bar";

    /// The long bar's part of the bulge-and-bar share.
    GALAXY_PARAMS_SHARE_BAR_OF_BULGE: Galaxy = "galaxy.params.share.bar_of_bulge";

    /// The nuclear disc's share of systems.
    GALAXY_PARAMS_SHARE_NUCLEAR_DISC: Galaxy = "galaxy.params.share.nuclear_disc";

    /// The halo's share of systems.
    GALAXY_PARAMS_SHARE_HALO: Galaxy = "galaxy.params.share.halo";

    /// The timescale of the thin disc's declining formation rate.
    GALAXY_PARAMS_SFH_TIMESCALE: Galaxy = "galaxy.params.sfh.timescale";

    /// The scatter of the thin disc's scale length about its mass coupling, dex.
    GALAXY_PARAMS_THIN_LENGTH_SCATTER: Galaxy = "galaxy.params.thin.length.scatter";

    /// The old thin disc's mean scale height.
    GALAXY_PARAMS_THIN_MEAN_HEIGHT: Galaxy = "galaxy.params.thin.mean_height";

    /// The young thin disc's scale height.
    GALAXY_PARAMS_YOUNG_HEIGHT: Galaxy = "galaxy.params.young.height";

    /// The thick disc's scale length over the thin disc's.
    GALAXY_PARAMS_THICK_LENGTH_RATIO: Galaxy = "galaxy.params.thick.length_ratio";

    /// The thick disc's scale height over the thin disc's mean.
    GALAXY_PARAMS_THICK_HEIGHT_RATIO: Galaxy = "galaxy.params.thick.height_ratio";

    /// The scatter of the bulge's long scale length about its mass coupling, dex.
    GALAXY_PARAMS_BULGE_LENGTH_SCATTER: Galaxy = "galaxy.params.bulge.length.scatter";

    /// The bulge's middle axis over its long axis.
    GALAXY_PARAMS_BULGE_B_OVER_A: Galaxy = "galaxy.params.bulge.b_over_a";

    /// The bulge's short axis over its long axis.
    GALAXY_PARAMS_BULGE_C_OVER_A: Galaxy = "galaxy.params.bulge.c_over_a";

    /// The bulge's vertical exponent.
    GALAXY_PARAMS_BULGE_BOXINESS: Galaxy = "galaxy.params.bulge.boxiness";

    /// The scatter of the long bar's half-length about its mass coupling, dex.
    GALAXY_PARAMS_BAR_LENGTH_SCATTER: Galaxy = "galaxy.params.bar.length.scatter";

    /// The long bar's width over its half-length.
    GALAXY_PARAMS_BAR_WIDTH_RATIO: Galaxy = "galaxy.params.bar.width_ratio";

    /// The long bar's scale height.
    GALAXY_PARAMS_BAR_HEIGHT: Galaxy = "galaxy.params.bar.height";

    /// The bar's corotation radius over its half-length.
    GALAXY_PARAMS_BAR_COROTATION_RATIO: Galaxy = "galaxy.params.bar.corotation_ratio";

    /// The scatter of the nuclear disc's scale length about its mass coupling, dex.
    GALAXY_PARAMS_NUCLEAR_LENGTH_SCATTER: Galaxy = "galaxy.params.nuclear.length.scatter";

    /// The nuclear disc's scale height over its scale length.
    GALAXY_PARAMS_NUCLEAR_HEIGHT_RATIO: Galaxy = "galaxy.params.nuclear.height_ratio";

    /// The scatter of the nuclear cluster's mass, dex.
    GALAXY_PARAMS_NUCLEAR_CLUSTER_MASS_SCATTER: Galaxy = "galaxy.params.nuclear_cluster.mass.scatter";

    /// The number of spiral arms, two or four.
    GALAXY_PARAMS_ARMS_COUNT: Galaxy = "galaxy.params.arms.count";

    /// The arms' pitch angle.
    GALAXY_PARAMS_ARMS_PITCH: Galaxy = "galaxy.params.arms.pitch";

    /// The young disc's arm width `σ_w`.
    GALAXY_PARAMS_ARMS_YOUNG_WIDTH: Galaxy = "galaxy.params.arms.young_width";

    /// The young disc's arm amplitude A.
    GALAXY_PARAMS_ARMS_YOUNG_FRACTION: Galaxy = "galaxy.params.arms.young_fraction";

    /// The old discs' arm amplitude a.
    GALAXY_PARAMS_ARMS_OLD_AMPLITUDE: Galaxy = "galaxy.params.arms.old_amplitude";

    /// The gas disc's mass over the thin disc's stellar mass.
    GALAXY_PARAMS_GAS_MASS_FRACTION: Galaxy = "galaxy.params.gas.mass_fraction";

    /// The gas disc's scale length over the thin disc's.
    GALAXY_PARAMS_GAS_LENGTH_RATIO: Galaxy = "galaxy.params.gas.length_ratio";

    /// The stellar-to-halo mass factor f★, log-uniform.
    GALAXY_PARAMS_DARK_F_STAR: Galaxy = "galaxy.params.dark.f_star";

    /// The scatter of the dark halo's concentration, dex.
    GALAXY_PARAMS_DARK_CONCENTRATION_SCATTER: Galaxy = "galaxy.params.dark.concentration.scatter";

    /// The scatter of the black hole's mass about the M–σ relation, dex.
    GALAXY_PARAMS_BH_SCATTER: Galaxy = "galaxy.params.bh.scatter";

    /// The discs' radial metallicity gradient.
    GALAXY_PARAMS_METALLICITY_GRADIENT: Galaxy = "galaxy.params.metallicity.gradient";

    /// The in-situ halo's share of the halo, before renormalising.
    GALAXY_PARAMS_HALO_IN_SITU_SHARE: Galaxy = "galaxy.params.halo.in_situ.share";

    /// The in-situ halo's axis ratio.
    GALAXY_PARAMS_HALO_IN_SITU_FLATTENING: Galaxy = "galaxy.params.halo.in_situ.flattening";

    /// The in-situ halo's core radius.
    GALAXY_PARAMS_HALO_IN_SITU_CORE: Galaxy = "galaxy.params.halo.in_situ.core";

    /// The dominant merger's share of the halo, before renormalising.
    GALAXY_PARAMS_HALO_DOMINANT_SHARE: Galaxy = "galaxy.params.halo.dominant.share";

    /// The dominant merger's axis ratio.
    GALAXY_PARAMS_HALO_DOMINANT_FLATTENING: Galaxy = "galaxy.params.halo.dominant.flattening";

    /// The dominant merger's core radius.
    GALAXY_PARAMS_HALO_DOMINANT_CORE: Galaxy = "galaxy.params.halo.dominant.core";

    /// The dominant merger's break radius.
    GALAXY_PARAMS_HALO_DOMINANT_BREAK_RADIUS: Galaxy = "galaxy.params.halo.dominant.break_radius";

    /// How much the dominant merger's slope steepens beyond its break.
    GALAXY_PARAMS_HALO_DOMINANT_BREAK_STEEPENING: Galaxy = "galaxy.params.halo.dominant.break_steepening";

    /// The number of lesser old progenitors, 2–5.
    GALAXY_PARAMS_HALO_LESSER_COUNT: Galaxy = "galaxy.params.halo.lesser.count";

    /// The lesser progenitors' share of the halo together, before renormalising.
    GALAXY_PARAMS_HALO_LESSER_SHARE_TOTAL: Galaxy = "galaxy.params.halo.lesser.share_total";

    /// The globular-born debris' share of the halo, before renormalising.
    GALAXY_PARAMS_HALO_DEBRIS_SHARE: Galaxy = "galaxy.params.halo.debris.share";

    /// The globular-born debris' power-law slope.
    GALAXY_PARAMS_HALO_DEBRIS_SLOPE: Galaxy = "galaxy.params.halo.debris.slope";

    /// The globular-born debris' core radius.
    GALAXY_PARAMS_HALO_DEBRIS_CORE: Galaxy = "galaxy.params.halo.debris.core";

    /// The halo's discrete share: streams and dwarf cores.
    GALAXY_PARAMS_HALO_DISCRETE_SHARE: Galaxy = "galaxy.params.halo.discrete_share";

    /// The time of the last major merger.
    GALAXY_PARAMS_ACCRETION_LAST_MAJOR_MERGER: Galaxy = "galaxy.params.accretion.last_major_merger";

    /// The number of recent progenitors, Poisson.
    GALAXY_PARAMS_ACCRETION_RECENT_COUNT: Galaxy = "galaxy.params.accretion.recent.count";

    /// The scatter of the globular cluster count, dex.
    GALAXY_PARAMS_ACCRETION_GLOBULAR_COUNT_SCATTER: Galaxy = "galaxy.params.accretion.globular_count.scatter";

    // List parameters, each opened with `ObjectKey::galaxy_item(n)`.

    /// A halo component's slope; n is its place: in situ 0, dominant 1, lesser 2–6.
    GALAXY_PARAMS_HALO_COMPONENT_SLOPE: Galaxy = "galaxy.params.halo.component.slope";

    /// A halo component's age range; n as above, debris 7.
    GALAXY_PARAMS_HALO_COMPONENT_AGE: Galaxy = "galaxy.params.halo.component.age";

    /// A lesser halo component's \[Fe/H\]; n as above.
    GALAXY_PARAMS_HALO_COMPONENT_FEH: Galaxy = "galaxy.params.halo.component.feh";

    /// A lesser progenitor's stick-breaking fraction; n is its number, 1–5.
    GALAXY_PARAMS_HALO_LESSER_SPLIT: Galaxy = "galaxy.params.halo.lesser.split";

    /// A lesser progenitor's axis ratio; n is its number, 1–5.
    GALAXY_PARAMS_HALO_LESSER_FLATTENING: Galaxy = "galaxy.params.halo.lesser.flattening";

    /// A recent progenitor's stellar mass; n is its progenitor number.
    GALAXY_PARAMS_ACCRETION_PROGENITOR_MASS: Galaxy = "galaxy.params.accretion.progenitor.mass";

    /// A progenitor's accretion time; n is its progenitor number.
    GALAXY_PARAMS_ACCRETION_PROGENITOR_TIME: Galaxy = "galaxy.params.accretion.progenitor.time";

    /// A progenitor's orbit: apocentre, pericentre, inclination, node, phase.
    GALAXY_PARAMS_ACCRETION_PROGENITOR_ORBIT: Galaxy = "galaxy.params.accretion.progenitor.orbit";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_two_tags_share_a_name_or_a_hash() {
        for (i, a) in ALL.iter().enumerate() {
            for b in &ALL[..i] {
                assert_ne!(a.name(), b.name());
                assert_ne!(a.hash(), b.hash(), "{} and {}", a.name(), b.name());
            }
        }
    }

    #[test]
    fn plan_01_registers_its_two_tags() {
        assert_eq!(SELFTEST_STREAM.name(), "selftest.stream");
        assert_eq!(SELFTEST_STREAM.scope(), crate::rng::TagScope::SelfTest);
        assert_eq!(EVENT_SELFTEST.name(), "event.selftest");
        assert_eq!(EVENT_SELFTEST.scope(), crate::rng::TagScope::Event);
        assert!(ALL.contains(&SELFTEST_STREAM) && ALL.contains(&EVENT_SELFTEST));
    }
}
