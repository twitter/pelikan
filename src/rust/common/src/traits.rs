pub trait ExtendFromSlice<T> {
    fn extend(&mut self, src: &[T]);
}
