pub enum RecvError {
    Empty,
}

pub enum SendError<T> {
    Full(T),
}
