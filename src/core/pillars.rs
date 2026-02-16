pub trait Pillars<T> {
    fn pillars(&self) -> Option<Vec<(String, T)>>;
}
