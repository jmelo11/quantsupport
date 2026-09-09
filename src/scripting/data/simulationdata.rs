use crate::scripting::{
    utils::errors::{Result, ScriptingError},
    NumericType,
};
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SimulationData {
    numeraire: NumericType,
    dfs: Vec<NumericType>,
    fwds: Vec<NumericType>,
    fxs: Vec<NumericType>,
    spots: Vec<NumericType>,
}

impl SimulationData {
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

    pub fn numeraire(&self) -> NumericType {
        self.numeraire
    }

    pub fn dfs(&self) -> &[NumericType] {
        &self.dfs
    }

    pub fn fwds(&self) -> &[NumericType] {
        &self.fwds
    }

    pub fn fxs(&self) -> &[NumericType] {
        &self.fxs
    }

    pub fn get_df(&self, index: usize) -> Result<NumericType> {
        self.dfs
            .get(index)
            .cloned()
            .ok_or(ScriptingError::NotFoundError(format!(
                "df at index {}",
                index
            )))
    }

    pub fn get_fwd(&self, index: usize) -> Result<NumericType> {
        self.fwds
            .get(index)
            .cloned()
            .ok_or(ScriptingError::NotFoundError(format!(
                "fwd at index {}",
                index
            )))
    }
    pub fn get_fx(&self, index: usize) -> Result<NumericType> {
        self.fxs
            .get(index)
            .cloned()
            .ok_or(ScriptingError::NotFoundError(format!(
                "fx at index {}",
                index
            )))
    }

    pub fn spots(&self) -> &[NumericType] {
        &self.spots
    }

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

pub type Scenario = Vec<SimulationData>;
