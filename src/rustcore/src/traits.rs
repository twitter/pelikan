use pelikan::protocol::Protocol;

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Empty {}

///
pub trait Worker {
    type Protocol: Protocol;
    type State: Default;

    fn process_request(
        &self,
        req: &mut <Self::Protocol as Protocol>::Request,
        rsp: &mut <Self::Protocol as Protocol>::Response,
        state: &mut Self::State,
    ) -> WorkerAction;
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum WorkerAction {
    // Do nothing to the connection
    None,
    // Close the connection
    Close,

    #[doc(hidden)]
    __Nonexhaustive(Empty)
}

impl Default for WorkerAction {
    fn default() -> Self {
        Self::None
    }
}
