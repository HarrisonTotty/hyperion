//! Alerts: transients seen from a distance, each in the window its light arrives in (plan 12,
//! P12.T5; brainstorm, "Alerts").
//!
//! Only [`AlertBand`] exists so far, added ahead of P12.T5.a because the server's Knowledge store
//! (P12.T7) records the band of every sighting. The rest of P12.T5.a (the sensor horizon, the
//! detection and census types, reach and flux) is that task's.

use crate::galaxy::gas::ccm::Band;

/// The band a detection is made in (plan 12's Provides; Design note 14).
///
/// Photometric bands are plan 07's [`Band`], from U to radio. X-rays are a band of their own here
/// and not a value of [`Band`]: plan 07's extinction runs through the Cardelli law, which stops at
/// 0.1 µm, while X-rays are absorbed photoelectrically by the metals that make the dust, which
/// P12.T5.a models from `A_V`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AlertBand {
    /// One of plan 07's photometric bands.
    Photometric(Band),
    /// X-rays, at a few keV.
    XRay,
}
