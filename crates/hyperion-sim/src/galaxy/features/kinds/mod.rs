//! The marks of each kind of feature (plan 09, P09.T4): bound open clusters, nurseries and clouds.
//!
//! Globular clusters' marks arrive with phase 3 (P09.T12).

pub mod cloud;
pub mod nursery;
pub mod open_cluster;

pub use cloud::CloudMarks;
pub use nursery::{NurseryMarks, NurseryStage, Superbubble};
pub use open_cluster::OpenClusterMarks;
