use pelikan::protocol::Protocol;

pub trait Worker {
    type Protocol: Protocol;
    type State: Default;

    fn process_request(
        &mut self,
        req: &mut <Self::Protocol as Protocol>::Request,
        rsp: &mut <Self::Protocol as Protocol>::Response,
        state: &mut Self::State,
    );
}
