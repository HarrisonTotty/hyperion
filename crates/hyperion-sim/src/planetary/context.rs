//! Everything the planetary stage reads from the stages above: a system's `SystemContext`
//! (plan 14, P14.T1.d).
//!
//! The context will hold the system's ID and host kind (a star, a free-floating brown dwarf or a
//! rogue planet), its stars with their tracks and plan 11's hierarchy, \[Fe/H\] and \[α/Fe\], its
//! age at the epoch, its sphere of influence (the galactic tidal radius, until plan 09's pericentre
//! rule) and its encounter environment (`None` until plan 09). `SystemContext::for_system` will
//! resolve an ID through plan 03 and build the stars through plans 06 and 11; its builder makes the
//! synthetic hosts of the tests and tools.
//!
//! Not built yet. Until it is, the pieces that exist take their inputs as plain arguments: the
//! disc takes a [`DiscHost`](super::disc::DiscHost), its lifetime and its truncation radii.
