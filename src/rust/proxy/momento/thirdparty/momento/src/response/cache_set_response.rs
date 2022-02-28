#[derive(Debug)]
pub enum MomentoSetStatus {
    OK,
    ERROR,
}
#[derive(Debug)]
pub struct MomentoSetResponse {
    pub result: MomentoSetStatus,
}
