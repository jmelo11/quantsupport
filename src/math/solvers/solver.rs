use crate::errors::errors::LibError;

pub trait Solver {
    fn solve(&self) -> Result<SolverResults, LibError>;
}

#[derive(Debug)]
pub enum SolverStatus {
    FoundSolution,
    MaxIterReached,
}

#[derive(Debug)]
pub struct SolverResults {
    pub f: f64,
    pub x: f64,
    pub status: SolverStatus,
    pub iters: i16,
}
