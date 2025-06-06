//! Ceres DSP Framework - Modular audio processing with parameter modulation

pub mod core;


// Re-export everything for clean imports
pub use core::*;
pub use ceres_macros::parameters;

// Convenience re-exports
pub use crate::core::{Builder, Runtime, ComponentFn};
pub use crate::core::{StateHandle, ModulatorHandle, ParameterHandle};
pub use crate::core::{Modulator, Parameters, ParameterRuntime};