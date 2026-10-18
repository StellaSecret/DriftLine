//! Vector-field puzzle game core. Nothing in this crate touches rendering or UI,
//! so every module (and every gameplay mode) can be tested headlessly.
pub mod field;
pub mod level;
pub mod levels;
pub mod modes;
pub mod sim;
pub mod validate;

pub use field::{v2, Field, Layer, Term, V2};
pub use level::{Chapter, Challenge, Level, Reveal, Rules, Slot, Zone};
pub use sim::{simulate, Outcome, Rect, Region, RegionKind, SimConfig, Trajectory};
