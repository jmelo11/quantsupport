//! Compiler capture lifetimes for existing pricing and market interfaces.

use super::recorded::{Recorded, SavedRecording};

/// Controls construction of compiler IR. Derivatives execute as native programs.
pub struct Recording;

impl Recording {
    /// Starts a fresh pricing capture.
    pub fn start() {
        Recorded::begin_recording();
    }
    /// Finishes IR construction while keeping input derivative results available.
    pub fn stop() {
        Recorded::end_recording();
    }
    /// Returns whether a compiler capture is currently recording operations.
    #[must_use]
    pub fn is_active() -> bool {
        Recorded::recording_enabled()
    }
    /// Clears compiler IR while preserving the current recording state.
    pub fn clear() {
        Recorded::clear_recording();
    }
    /// Saves an immutable market/model setup prefix.
    pub fn checkpoint() {
        Recorded::checkpoint_recording();
    }
    /// Discards transient per-path IR after native differentiation.
    pub fn restart() {
        Recorded::restart_recording();
    }
    /// Releases the setup checkpoint. Native gradients already include setup dependencies.
    pub fn release_checkpoint() {
        Recorded::release_recording_checkpoint();
    }
    /// Clears the input derivatives from prior native executions.
    pub fn reset_derivatives() {
        Recorded::reset_derivatives();
    }
    /// Returns the operation count, checkpoint position and recording state.
    #[must_use]
    pub fn state() -> (usize, usize, bool) {
        Recorded::recording_state()
    }
    /// Returns the number of operations in the current capture.
    #[must_use]
    pub fn operation_count() -> usize {
        Recorded::recording_operation_count()
    }
}

/// Preserves an enclosing compiler capture across isolated market construction.
pub struct RecordingScope {
    saved: Option<SavedRecording>,
}
impl RecordingScope {
    /// Suspends the enclosing capture until this scope is dropped.
    #[must_use]
    pub fn new() -> Self {
        Self {
            saved: Some(Recorded::suspend_recording()),
        }
    }
}
impl Default for RecordingScope {
    /// Suspends the enclosing compiler capture for an isolated scope.
    fn default() -> Self {
        Self::new()
    }
}
impl Drop for RecordingScope {
    /// Restores the suspended capture when isolated construction finishes or unwinds.
    fn drop(&mut self) {
        if let Some(saved) = self.saved.take() {
            Recorded::restore_recording(saved);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::errors::Result;

    /// Checks native path gradients include setup dependencies once.
    #[test]
    fn native_path_gradients_include_setup_dependencies_once() -> Result<()> {
        let _scope = RecordingScope::new();
        Recording::start();
        let input = Recorded::new(2.0);
        let coefficient = input * input;
        Recording::checkpoint();
        for draw in [2.0, 3.0] {
            Recording::restart();
            (coefficient * draw).differentiate_accumulating()?;
        }
        Recording::release_checkpoint();
        assert!((input.derivative()?.value() - 20.0).abs() < 1e-12);
        Recording::stop();
        Ok(())
    }

    /// Checks isolated market construction restores the enclosing capture.
    #[test]
    fn isolated_market_construction_restores_the_enclosing_capture() -> Result<()> {
        let _scope = RecordingScope::new();
        Recording::start();
        let input = Recorded::new(2.0);
        let before = Recording::state();
        {
            let _market_scope = RecordingScope::new();
            Recording::start();
            let unrelated = Recorded::new(7.0);
            (unrelated * unrelated).differentiate()?;
        }
        assert_eq!(before, Recording::state());
        (input * input).differentiate()?;
        assert!((input.derivative()?.value() - 4.0).abs() < 1e-12);
        Ok(())
    }

    /// Checks calibrated input maps are native expressions.
    #[test]
    fn calibrated_input_maps_are_native_expressions() -> Result<()> {
        let _scope = RecordingScope::new();
        Recording::start();
        let quote = Recorded::new(0.03);
        let node = Recorded::new(0.9);
        node.set_linearization(&[quote], &[-0.8]);
        let price = Recorded::one() / node;
        price.differentiate()?;
        assert!((quote.derivative()?.value() - 0.8 / 0.81).abs() < 1e-12);
        let jacobian = Recorded::jacobian(&[quote], &[price])?;
        assert!((jacobian[0][0] - 0.8 / 0.81).abs() < 1e-12);
        assert!((quote.derivative()?.value() - jacobian[0][0]).abs() < 1e-12);
        Ok(())
    }
}
