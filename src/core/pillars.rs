pub trait Pillars<T> {
    fn pillars(&self) -> Option<Vec<(String, &T)>>;
    fn pillar_labels(&self) -> Option<Vec<String>>;
    fn put_pillars_on_tape(&mut self);
}
