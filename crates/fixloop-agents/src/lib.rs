//! Fixloop's agents. Phase 2 ships the Reviewer; the Fixer and Bug Fixer
//! follow in Phase 4.

pub mod diff;
pub mod error;
pub mod reviewer;

pub use error::ReviewError;
