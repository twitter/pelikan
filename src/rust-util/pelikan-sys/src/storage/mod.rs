#[cfg(feature = "cuckoo")]
pub mod cuckoo {
    use crate::time::{delta_time_i, proc_time_i};
    use ccommon_sys::{bstring, metric, option, rstatus_i};

    include!(concat!(env!("OUT_DIR"), "/cuckoo.rs"));
}

#[cfg(feature = "slab")]
pub mod slab;

#[cfg(feature = "cdb")]
pub mod cdb {
    use ccommon_sys::bstring;

    include!(concat!(env!("OUT_DIR"), "/cdb.rs"));
}
