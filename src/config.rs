//! Contains functions and definitions related to the primary hyperion data configuration file.

//use serde::{
//    Deserialize,
//    Deserializer,
//    Serialize,
//    Serializer
//};

/// Represents the primary configuration file.
#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    /// `resources::DeltaTime`
    #[serde(default = "default_dt", rename = "simulation.dt")]
    pub dt: f64,

    /// Where to find faction definition files.
    #[serde(default = "default_faction_files", rename = "data.factions")]
    pub faction_files: Vec<String>, 
    
    /// `resources::DynamicsLimits.maximum_acceleration`
    #[serde(default = "default_max_acceleration", rename = "limits.max_acceleration")]
    pub max_acceleration: f64, 

    /// `resources::OrientationLimits.maximum_angular_acceleration`
    #[serde(default = "default_max_angular_acceleration", rename = "limits.max_angular_acceleration")]
    pub max_angular_acceleration: f64,

    /// `resources::OrientationLimits.maximum_angular_velocity`
    #[serde(default = "default_max_angular_velocity", rename = "limits.max_angular_velocity")]
    pub max_angular_velocity: f64,
    
    /// `resources::CollisionLimits.maximum_detection_theshold`
    #[serde(default = "default_max_collision_detection_threshold", rename = "limits.max_collision_detection_theshold")]
    pub max_collision_detection_threshold: f64,

    /// `resources::DynamicsLimits.maximum_position`
    #[serde(default = "default_max_position", rename = "limits.max_position")]
    pub max_position: f64,

    /// `resources::DynamicsLimits.maximum_velocity`
    #[serde(default = "default_max_velocity", rename = "limits.max_velocity")]
    pub max_velocity: f64,

    /// `resources::DynamicsLimits.minimum_acceleration`
    #[serde(default = "default_min_acceleration", rename = "limits.min_acceleration")]
    pub min_acceleration: f64,

    /// `resources::OrientationLimits.minimum_angular_acceleration`
    #[serde(default = "default_min_angular_acceleration", rename = "limits.min_angular_acceleration")]
    pub min_angular_acceleration: f64,

    /// `resources::OrientationLimits.minimum_angular_velocity`
    #[serde(default = "default_min_angular_velocity", rename = "limits.min_angular_velocity")]
    pub min_angular_velocity: f64,

    /// `resources::CollisionLimits.minimum_detection_theshold`
    #[serde(default = "default_min_collision_detection_threshold", rename = "limits.min_collision_detection_theshold")]
    pub min_collision_detection_threshold: f64,

    /// `resources::DynamicsLimits.minimum_position`
    #[serde(default = "default_min_position", rename = "limits.min_position")]
    pub min_position: f64,

    /// `resources::DynamicsLimits.minimum_velocity`
    #[serde(default = "default_min_velocity", rename = "limits.min_velocity")]
    pub min_velocity: f64,

    /// Where to find ship class definition files.
    #[serde(default = "default_ship_class_files", rename = "data.ship_classes")]
    pub ship_class_files: Vec<String>,

    /// Where to find ship module definition files.
    #[serde(default = "default_ship_module_files", rename = "data.ship_modules")]
    pub ship_module_files: Vec<String>,
}

/// A function that returns the list of matching files fiven a file
/// specification.
pub fn files_from_spec(spec: String) -> Vec<String> {
    let full_spec = match spec.starts_with("/") {
        true => spec,
        false => match spec.starts_with("~") {
            true => spec, // placeholder
            false => spec // placeholder
        }
    };
    if !full_spec.contains("*") && !full_spec.contains("[") && !full_spec.contains("]") {
        return vec![full_spec];
    } else if full_spec.contains("*") {
        return vec![full_spec]; // placeholder
    }
    vec![full_spec] // placeholder
}


/// A function that returns the list of matching files given a list of file
/// specifications.
pub fn files_from_specs(specs: Vec<String>) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();
    for spec in specs {
        result.extend(files_from_spec(spec).iter().cloned());
    }
    result
}

// ----- Functions for setting defaults -----
fn default_dt() -> f64 { 1.0 }
fn default_faction_files() -> Vec<String> { vec![String::from("factions/*.yaml")] }
fn default_max_acceleration() -> f64 { std::f64::INFINITY }
fn default_max_angular_acceleration() -> f64 { std::f64::INFINITY }
fn default_max_angular_velocity() -> f64 { std::f64::INFINITY }
fn default_max_collision_detection_threshold() -> f64 { 100.0 }
fn default_max_position() -> f64 { std::f64::INFINITY }
fn default_max_velocity() -> f64 { std::f64::INFINITY }
fn default_min_acceleration() -> f64 { 0.0 }
fn default_min_angular_acceleration() -> f64 { 0.0 }
fn default_min_angular_velocity() -> f64 { 0.0 }
fn default_min_collision_detection_threshold() -> f64 { 1.0 }
fn default_min_position() -> f64 { 0.0 }
fn default_min_velocity() -> f64 { 0.0 }
fn default_ship_class_files() -> Vec<String> { vec![String::from("ship_classes/*.yaml")] }
fn default_ship_module_files() -> Vec<String> { vec![String::from("ship_modules/*.yaml")] }
