/// `Pillars`
///
/// Trait representing the concept of "pillars" in a financial context, which are key
/// reference points used in the construction of curves and surfaces. This
/// trait provides methods to retrieve the pillars and their labels, as well as a
/// method to put the pillars on the tape for automatic differentiation purposes.
pub trait Pillars<T> {
    /// Returns an optional vector of tuples containing pillar labels and their corresponding values.
    fn pillars(&self) -> Option<Vec<(String, &T)>>;
    /// Returns an optional vector of pillar labels.
    fn pillar_labels(&self) -> Option<Vec<String>>;
    /// Puts the pillars on the tape for automatic differentiation purposes.
    fn put_pillars_on_tape(&mut self);
}
