//! The event tags of the event kinds, re-exported from the registry in
//! [`id::event_tags`](crate::id::event_tags).
//!
//! Plan 06's single stars hold `0x0100`–`0x0109`: flares, the glitches of Crab-like and of
//! Vela-like pulsars, a magnetar's episodes, bursts and giant flares, FU Orionis outbursts, the
//! giant eruptions of luminous blue variables, thermal pulses, and the cycle-keyed irregularity of
//! pulsating variables. Plans 09, 11 and 14 add theirs in the blocks the registry sets aside.

pub use crate::id::event_tags::{
    STAR_FLARE, STAR_FU_ORIONIS, STAR_GLITCH, STAR_GLITCH_CYCLE, STAR_LBV_ERUPTION,
    STAR_MAGNETAR_BURST, STAR_MAGNETAR_EPISODE, STAR_MAGNETAR_GIANT, STAR_THERMAL_PULSE,
    STAR_VARIABILITY_CYCLE,
};
