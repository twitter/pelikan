#[derive(Debug)]
pub enum MomentoGetStatus {
    HIT,
    MISS,
    ERROR,
}

#[derive(Debug)]
pub struct MomentoGetResponse {
    pub result: MomentoGetStatus,
    pub value: Vec<u8>,
}

impl MomentoGetResponse {
    pub fn as_string(&self) -> &str {
        return std::str::from_utf8(self.value.as_slice()).unwrap_or_default();
    }
}
