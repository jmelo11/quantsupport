/// Differentiable state variables exposed by a constructed market object.
///
/// Pillar values are registered on the automatic-differentiation tape, and
/// their labels identify the adjoints returned in sensitivity reports. When a
/// surface or cube also stores calibration instrument identifiers, those
/// identifiers describe the source option contracts available to a model's
/// calibration basket. Pillars describe the variables that receive risk.
pub trait Pillars<T> {
    /// Returns the sensitivity label and differentiable value of each pillar.
    fn pillars(&self) -> Option<Vec<(String, &T)>>;
    /// Returns the labels used to identify pillar adjoints in reports.
    fn pillar_labels(&self) -> Option<Vec<String>>;
    /// Registers each pillar as a leaf on the automatic-differentiation tape.
    fn put_pillars_on_tape(&mut self);
}
