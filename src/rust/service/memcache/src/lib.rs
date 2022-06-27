use std::sync::Arc;
use session::Session;
use protocol_common::*;
use protocol_memcache::*;

pub enum RecvError {
    Invalid,
    Incomplete,
    NoPending,
    Pending,
    Unknown,
}

impl From<ParseError> for RecvError {
    fn from(other: ParseError) -> Self {
        match other {
            ParseError::Invalid => Self::Invalid,
            ParseError::Incomplete => Self::Incomplete,
            ParseError::Unknown => Self::Unknown,
        }
    }
}

pub enum SendError {
    NoPending,
    Pending,
}

pub struct Server {
    parser: RequestParser,
    pending: Option<Arc<Request>>,
}

impl Server {
    pub fn recv(&mut self, src: &[u8]) -> Result<ParseOk<Arc<Request>>, RecvError> {
        if self.pending.is_some() {
            return Err(RecvError::Pending);
        }

        match self.parser.parse(src) {
            Ok(r) => {
                let consumed = r.consumed();
                let request = r.into_inner();
                let request = Arc::new(request);
                self.pending = Some(request.clone());

                Ok(ParseOk::new(request, consumed))
            }
            Err(e) => {
                Err(RecvError::from(e))
            }
        }
    }

    pub fn send(&mut self, dst: &mut Session, response: Response) -> Result<(), SendError> {
        if self.pending.is_none() {
            return Err(SendError::NoPending);
        }

        match &*self.pending.take().unwrap() {
            Request::Get(req) => match response {
                Response::Values(ref res) => {
                    let total_keys = req.keys().len();
                    let hit_keys = res.values().len();
                    let miss_keys = total_keys - hit_keys;
                    COMPOSE_GET_KEY_HIT.add(hit_keys as _);
                    COMPOSE_GET_KEY_MISS.add(miss_keys as _);
                }
                _ => {
                    return Ok(Error::new().compose(dst));
                }
            }
            Request::Set(_) => match response {
                Response::NotStored(_) => {
                    COMPOSE_SET_NOT_STORED.increment();
                }
                Response::Stored(_) => {
                    COMPOSE_SET_STORED.increment();
                }
                _ => {
                    return Ok(Error::new().compose(dst));
                }
            }
            _ => todo!()
        }
        Ok(response.compose(dst))
    }
}

pub struct Client {
    parser: ResponseParser,
    pending: Option<Request>,
}

impl Client {
    pub fn recv(&mut self, src: &[u8]) -> Result<ParseOk<Response>, RecvError> {
        if self.pending.is_none() {
            return Err(RecvError::NoPending);
        }

        match self.parser.parse(src) {
            Ok(result) => {
                let consumed = result.consumed();
                let response = result.into_inner();

                let request = self.pending.take().unwrap();

                match request {
                    Request::Get(ref req) => match response {
                        Response::Values(ref res) => {
                            let total_keys = req.keys().len();
                            let hit_keys = res.values().len();
                            let miss_keys = total_keys - hit_keys;
                            PARSE_GET_KEY_HIT.add(hit_keys as _);
                            PARSE_GET_KEY_MISS.add(miss_keys as _);
                        }
                        _ => {
                            return Err(RecvError::Invalid);
                        }
                    }
                    Request::Gets(ref req) => match response {
                        Response::Values(ref res) => {
                            let total_keys = req.keys().len();
                            let hit_keys = res.values().len();
                            let miss_keys = total_keys - hit_keys;
                            PARSE_GETS_KEY_HIT.add(hit_keys as _);
                            PARSE_GETS_KEY_MISS.add(miss_keys as _);
                        }
                        _ => {
                            return Err(RecvError::Invalid);
                        }
                    }
                    _ => todo!()
                    
                }

                Ok(ParseOk::new(response, consumed))
            }
            Err(e) => {
                Err(RecvError::from(e))
            }
        }
    }

    pub fn send(&mut self, dst: &mut Session, request: Request) -> Result<(), SendError> {
        if self.pending.is_some() {
            return Err(SendError::Pending);
        }

        request.compose(dst);

        self.pending = Some(request);

        Ok(())
    }
}



#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
