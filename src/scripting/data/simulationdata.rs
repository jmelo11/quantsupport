/*
This file is part of QuantSupport's Rust rewrite and adaptation of the
derivatives scripting code written by Antoine Savine in 2018.

The original code is the strict intellectual property of Antoine Savine.

A license to use and alter the original code for personal and commercial
applications is freely granted to any person or company that purchased a copy
of the book:

Modern Computational Finance: Scripting for Derivatives and XVA
Jesper Andreasen and Antoine Savine
Wiley, 2018

This attribution and license notice must be preserved at the top of this file.
*/

use crate::scripting::{
    utils::errors::{Result, ScriptingError},
    NumericType,
};
#[derive(Debug, Clone, PartialEq, Default)]
/// Market observations available while evaluating one scripted event.
pub struct SimulationData {
    numeraire: NumericType,
    dfs: Vec<NumericType>,
    fwds: Vec<NumericType>,
    fxs: Vec<NumericType>,
    spots: Vec<NumericType>,
}

impl SimulationData {
    /// Creates an event data set from its numeraire and indexed observations.
    pub fn new(
        numeraire: NumericType,
        dfs: Vec<NumericType>,
        fwds: Vec<NumericType>,
        fxs: Vec<NumericType>,
        spots: Vec<NumericType>,
    ) -> SimulationData {
        SimulationData {
            numeraire,
            dfs,
            fwds,
            fxs,
            spots,
        }
    }

    /// Returns the event numeraire.
    pub fn numeraire(&self) -> NumericType {
        self.numeraire
    }

    /// Returns all indexed discount factors.
    pub fn dfs(&self) -> &[NumericType] {
        &self.dfs
    }

    /// Returns all indexed forward rates.
    pub fn fwds(&self) -> &[NumericType] {
        &self.fwds
    }

    /// Returns all indexed FX rates.
    pub fn fxs(&self) -> &[NumericType] {
        &self.fxs
    }

    /// Returns the discount factor at `index`.
    ///
    /// # Errors
    /// Returns an error when the index is outside the discount-factor vector.
    pub fn get_df(&self, index: usize) -> Result<NumericType> {
        self.dfs
            .get(index)
            .cloned()
            .ok_or(ScriptingError::NotFoundError(format!(
                "df at index {}",
                index
            )))
    }

    /// Returns the forward rate at `index`.
    ///
    /// # Errors
    /// Returns an error when the index is outside the forward-rate vector.
    pub fn get_fwd(&self, index: usize) -> Result<NumericType> {
        self.fwds
            .get(index)
            .cloned()
            .ok_or(ScriptingError::NotFoundError(format!(
                "fwd at index {}",
                index
            )))
    }
    /// Returns the FX rate at `index`.
    ///
    /// # Errors
    /// Returns an error when the index is outside the FX-rate vector.
    pub fn get_fx(&self, index: usize) -> Result<NumericType> {
        self.fxs
            .get(index)
            .cloned()
            .ok_or(ScriptingError::NotFoundError(format!(
                "fx at index {}",
                index
            )))
    }

    /// Returns all indexed spot observations.
    pub fn spots(&self) -> &[NumericType] {
        &self.spots
    }

    /// Returns the spot observation at `index`.
    ///
    /// # Errors
    /// Returns an error when the index is outside the spot vector.
    pub fn get_spot(&self, index: usize) -> Result<NumericType> {
        self.spots
            .get(index)
            .cloned()
            .ok_or(ScriptingError::NotFoundError(format!(
                "spot at index {}",
                index
            )))
    }
}

/// Chronological market data supplied to every event in one simulated path.
pub type Scenario = Vec<SimulationData>;
