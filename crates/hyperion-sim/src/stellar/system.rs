//! System assembly: a system's metallicity draw, its stars' models, and the summaries and briefs
//! the server sends for a system and for each row of a range query (plan 06, phase G).
//!
//! This is the only part of the stellar stage that reads plan 03's placement: everything else is a
//! pure function of mass, composition, the star's draws and age.
