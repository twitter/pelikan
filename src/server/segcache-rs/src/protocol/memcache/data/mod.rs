mod item;
mod request;
mod response;

pub use item::*;
pub use request::*;
pub use response::*;

use super::*;
use crate::*;

impl<T> Execute<MemcacheRequest, MemcacheResponse> for T
where
    T: MemcacheStorage + GetTtl,
{
    fn execute(&mut self, request: MemcacheRequest) -> MemcacheResponse {
        match request.command {
            MemcacheCommand::Get => self.get(&request.keys),
            MemcacheCommand::Gets => self.gets(&request.keys),
            MemcacheCommand::Set => {
                let ttl = self.get_ttl(request.expiry);
                self.set(
                    &request.keys[0],
                    request.value,
                    request.flags,
                    ttl,
                    request.noreply,
                )
            }
            MemcacheCommand::Add => {
                let ttl = self.get_ttl(request.expiry);
                self.add(
                    &request.keys[0],
                    request.value,
                    request.flags,
                    ttl,
                    request.noreply,
                )
            }
            MemcacheCommand::Replace => {
                let ttl = self.get_ttl(request.expiry);
                self.replace(
                    &request.keys[0],
                    request.value,
                    request.flags,
                    ttl,
                    request.noreply,
                )
            }
            MemcacheCommand::Delete => self.delete(&request.keys[0], request.noreply),
            MemcacheCommand::Cas => {
                let ttl = self.get_ttl(request.expiry);
                self.cas(
                    &request.keys[0],
                    request.value,
                    request.flags,
                    ttl,
                    request.noreply,
                    request.cas,
                )
            }
        }
    }
}
