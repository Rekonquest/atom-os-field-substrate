//! std-friendly facade that re-exports field-core.
//!
//! field-core itself is `#![cfg_attr(not(test), no_std)]`. When a std build
//! wants to consume it without dragging in `--cfg test` semantics, depend on
//! this crate instead: it links std unconditionally and re-exports the whole
//! field-core surface.

pub use field_core::{
    Config, CouplingMode, DisturbanceMode, Measurements, Spectrum, World,
};
