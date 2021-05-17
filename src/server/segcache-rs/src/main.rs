#[macro_use]
extern crate rustcommon_logger;

#[macro_use]
extern crate rustcommon_fastmetrics;

mod buffer;
mod common;
mod event_loop;
mod protocol;
mod session;
mod storage;
mod threads;

pub use protocol::*;
pub use storage::*;

fn main() {
    println!("Hello, world!");
}
