//! The surface crate's registry of domain tags: the `surface.*` tags, and `selftest.surface.*`.
//!
//! One of three registries (plan R04, Design note 4), beside `hyperion_base::rng::tags` and the
//! sim's `rng::tags`, which asserts all three disjoint. Every tag the surface crate opens is
//! declared here: the `surface.` prefix is reserved for this registry (the rendering plans' R09
//! adds `surface.coarse.*`, `surface.channel` and `surface.crater`, and reserves R11's
//! `surface.scatter`), and `selftest.surface.` for its tags of scope `SelfTest`, such as R05's
//! provisional test planet's. Tags the server derives in the sim, `body.surface` and
//! `body.surface.detail`, are the sim's, not this registry's.
//!
//! Empty until R05 and R09: the rules of every registry hold from the first entry (a tag is never
//! renamed or removed, a new property group gets a new tag).

hyperion_base::domain_tags! {}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every name begins `surface.`, or `selftest.surface.` for a tag of `SelfTest` scope.
    /// Vacuous until R05's first tag, and then binding.
    #[test]
    fn surface_tags_carry_the_surface_prefix() {
        for tag in ALL {
            let prefix = if tag.scope() == hyperion_base::rng::TagScope::SelfTest {
                "selftest.surface."
            } else {
                "surface."
            };
            assert!(
                tag.name().starts_with(prefix),
                "{} lacks {prefix}",
                tag.name()
            );
        }
    }
}
