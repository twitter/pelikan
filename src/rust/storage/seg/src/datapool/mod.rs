// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Datapools are an abstraction of contiguous byte allocations.

//! Each datapool implementation should implement the `Datapool` trait to allow
//! easy interoperability across different backing stores.

mod file;
mod memory;

pub use file::File;
pub use memory::Memory;

/// The datapool trait defines the abstraction that each datapool implementation
/// should conform to.
pub trait Datapool: Send {
    /// Immutable borrow of the data within the datapool
    fn as_slice(&self) -> &[u8];

    /// Mutable borrow of the data within the datapool
    fn as_mut_slice(&mut self) -> &mut [u8];

    /// Performs any actions necessary to persist the data to the backing store.
    /// This may be a no-op for datapools which cannot persist data.
    fn flush(&self) -> Result<(), std::io::Error>;

}
