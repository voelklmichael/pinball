//! Pinball engine: the table model, table files and physics. Has no
//! window or drawing; the game (`proto`) and the scenario tests
//! (`scenario`) build on it.

pub mod layout;
pub mod physics;
pub mod svg;
pub mod tablefile;

#[cfg(test)]
mod tests;

/// The physics engine, so users of this crate get the same version.
pub use rapier2d;
