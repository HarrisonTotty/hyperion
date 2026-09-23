//! What a body query returns, and how it degrades for the Knowledge overlay: `BodyRecord`,
//! `DetailLevel` and `BodyKind` (plan 14, design note 16, P14.T34).
//!
//! A record will be nested so that each detail level is a prefix of the next: `Contact`
//! (identity, kind unknown, position), `MassAndOrbit`, `Bulk`, `Surface` and `Full`. Degrading a
//! record clears sections and never blurs a number.
//!
//! Not built yet.
