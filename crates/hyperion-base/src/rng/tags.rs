//! The foundation's registry of domain tags: the tags this crate itself opens.
//!
//! One of three registries (plan R04, Design note 4). This one holds only what the foundation
//! opens, today the self-test stream its tests and goldens draw from; `hyperion_surface::tags`
//! holds the `surface.*` tags; the sim's `rng::tags` holds every other stage's, and asserts all
//! three disjoint, so the collision check still covers every tag. A tag belongs here only if code
//! in this crate opens it: a registry here would otherwise change, and re-run the client's checks,
//! with every sim plan's tags.
//!
//! The rules of every registry hold: a tag is never renamed or removed, a new property group gets
//! a new tag, and names match `[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)+`.

crate::domain_tags! {
    // Plan 01: the determinism foundation.

    /// A stream for tests and golden files, never opened by a generator.
    SELFTEST_STREAM: SelfTest = "selftest.stream";
}
