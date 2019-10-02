//! Contains a collection of serializable types, which are typically returned
//! by API calls as JSON.

//use serde::{
//    Deserialize,
//    Deserializer,
//    Serialize,
//    Serializer
//};

use std::collections::HashMap;

/// Represents an API directory.
#[derive(Serialize, Deserialize, Debug)]
pub struct Directory {
    /// A brief description of the directory.
    pub desc: &'static str,

    /// A collection of possible sub-paths.
    pub endpoints: HashMap<&'static str, &'static str>
}

/// Represents the root API object.
///
/// This is what is returned when a user hits `/`.
#[derive(Serialize, Deserialize, Debug)]
pub struct APIRoot {
    /// The description of the server program.
    pub about: &'static str,

    /// The authors.
    pub authors: &'static str,

    /// The name of the server program.
    pub name: &'static str,
    
    /// A list of supported API versions, each one corresponding to a valid
    /// subpath.
    pub supported_apis: Vec<&'static str>,
    
    /// The version string of the Hyperion server process.
    pub version: &'static str,
}

/// Represents a generic message.
#[derive(Serialize, Deserialize, Debug)]
pub struct Message {
    /// The message contents.
    pub message: String
}
