//! The manifest of a bulk answer (rendering plan R03, R03.T10.a): what the terminal JSON response
//! of a request answered in binary frames says the frames held, so that the client can check what
//! arrived. The frames' header is laid out in the crate's documentation; a binary frame is not
//! JSON and has no type here.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// How many binary frames preceded a bulk answer's terminal response, and how many payload bytes
/// they held together, headers excluded. A kind answered in bulk (R06's sky, R09's coarse field)
/// carries it in its response body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BulkManifestDto {
    /// The frames sent, numbered from 0 in their headers; 0 for an empty payload.
    pub chunks: u32,
    /// The payload bytes, all frames together.
    ///
    /// A JSON number, not the crate's hexadecimal for a `u64`: a bulk payload is megabytes, far
    /// below the 2⁵³ a JavaScript number holds exactly.
    #[ts(type = "number")]
    pub bytes: u64,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::testing::assert_wire_form;

    #[test]
    fn bulk_manifest_wire_form() {
        assert_wire_form(
            &BulkManifestDto {
                chunks: 61,
                bytes: 15_728_640,
            },
            json!({ "chunks": 61, "bytes": 15_728_640 }),
        );
        assert_wire_form(
            &BulkManifestDto {
                chunks: 0,
                bytes: 0,
            },
            json!({ "chunks": 0, "bytes": 0 }),
        );
    }
}
