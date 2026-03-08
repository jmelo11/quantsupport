/// Key reference points for curve and surface construction.
///
/// The [`Pillars<T>`] trait provides methods to retrieve the pillars and their labels, as well as a
/// method to put the pillars on the tape for automatic differentiation purposes.
pub trait Pillars<T> {
    /// Returns an optional vector of tuples containing pillar labels and their corresponding values.
    fn pillars(&self) -> Option<Vec<(String, &T)>>;
    /// Returns an optional vector of pillar labels.
    fn pillar_labels(&self) -> Option<Vec<String>>;
    /// Puts the pillars on the tape for automatic differentiation purposes.
    fn put_pillars_on_tape(&mut self);
}
