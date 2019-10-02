//! Contains all of the server route method definitions.

use crate::json;
use rocket_contrib::json::Json;

/// The root server route.
#[get("/")]
pub fn root() -> Json<json::APIRoot> {
    use clap:: {
        crate_authors,
        crate_description,
        crate_name,
        crate_version
    };
    Json(json::APIRoot {
        about: crate_description!(),
        authors: crate_authors!(),
        name: crate_name!(),
        supported_apis: vec!["v1"],
        version: crate_version!()
    })
}

/// The root API endpoint for v1.
#[get("/v1")]
pub fn v1() -> Json<json::Directory> {
    Json(json::Directory {
        desc: "Hyperion server API version 1",
        endpoints: [
            ("create-ship", "[POST] Create a new player ship with the specified name and password."),
            ("get-ship-id", "[POST] Gets the ID of a player ship from its name or registration."),
            ("ships/", "Interact with player ships.")
        ].iter().cloned().collect()
    })
}
