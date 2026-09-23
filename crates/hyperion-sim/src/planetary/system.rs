//! The assembled planetary generator: `generate`, `generate_planets` and the `PlanetarySystem`
//! they return (plan 14, P14.T30).
//!
//! A system will be generated as it was born (design note 1): per orbit host its disc, class and
//! zone, its bodies sorted by [`BodyIndex`](super::BodyIndex), its belts and its cometary halo, and
//! the queries `snapshot_at`, `body_at`, `position_at`, `events_between` and `habitable_zone_at`,
//! each a pure function of seed, ID and time.
//!
//! Not built yet: it needs the context (P14.T1.d), the architecture classes (T4, T5), placement
//! (T6–T9) and derivation (T11–T16).
