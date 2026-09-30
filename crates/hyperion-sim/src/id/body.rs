//! Body IDs: a system and a 16-bit body index.

use super::designation::Designation;
use super::system::SystemId;

/// A body of a system: a star, planet, moon, ring, belt or anything else a system holds.
///
/// A body's ID is wider than 64 bits. Every `u16` is a valid index here; what the indices mean
/// belongs to the stellar and planetary stages. The text form is the system's 16 hexadecimal
/// digits, a full stop and the index as four hexadecimal digits: `0123456789abcdef.0003`.
///
/// # Examples
///
/// ```
/// use hyperion_sim::id::{BodyId, SystemId};
///
/// let star = BodyId::new(SystemId::from_raw(0x0200_0800_2000_0000)?, 0);
/// assert_eq!(star.to_string(), "0200080020000000.0000");
/// assert_eq!("0200080020000000.0000".parse::<BodyId>()?, star);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BodyId {
    system: SystemId,
    body_index: u16,
}

impl BodyId {
    /// Body `body_index` of `system`.
    #[must_use]
    pub const fn new(system: SystemId, body_index: u16) -> Self {
        Self { system, body_index }
    }

    /// The system.
    #[must_use]
    pub const fn system(self) -> SystemId {
        self.system
    }

    /// The body index.
    #[must_use]
    pub const fn body_index(self) -> u16 {
        self.body_index
    }

    /// The body's designation: the system's designation and ` /<body index>`.
    #[must_use]
    pub const fn designation(self) -> Designation {
        Designation::of_body(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bodies_order_by_system_then_index() {
        let a = SystemId::from_raw(0x0200_0800_2000_0000).unwrap();
        let b = SystemId::from_raw(0x0200_0800_2000_0001).unwrap();
        let mut bodies = [
            BodyId::new(b, 0),
            BodyId::new(a, u16::MAX),
            BodyId::new(a, 1),
        ];
        bodies.sort_unstable();
        assert_eq!(
            bodies,
            [
                BodyId::new(a, 1),
                BodyId::new(a, u16::MAX),
                BodyId::new(b, 0)
            ]
        );
        assert_eq!((bodies[1].system(), bodies[1].body_index()), (a, u16::MAX));
    }
}
