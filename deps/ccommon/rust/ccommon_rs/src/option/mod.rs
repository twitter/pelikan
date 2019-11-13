// ccommon - a cache common library.
// Copyright (C) 2019 Twitter, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Types and methods for dealing with ccommon options.

use std::ffi::CStr;

use cc_binding::option;
use ccommon_backend::option::{
    option_describe_all, option_load, option_load_default, option_print_all, ParseError,
};

// Sealed trait to prevent SingleOption from ever being implemented
// from outside of this crate.
mod private {
    pub trait Sealed {}
}

use self::private::Sealed;

mod boolean;
mod fpn;
mod string;
mod uint;

pub use self::boolean::Bool;
pub use self::fpn::Float;
pub use self::string::Str;
pub use self::uint::UInt;

/// A single option.
///
/// This trait is sealed and cannot be implemented outside
/// of ccommon_rs.
pub unsafe trait SingleOption: self::private::Sealed {
    /// The type of the value contained within this option.
    type Value;

    /// Create an option with the given description, name,
    /// and default value.
    ///
    /// Normally, this should only be called by the derive
    /// macro for `Options`.
    fn new(default: Self::Value, name: &'static CStr, desc: &'static CStr) -> Self;

    /// Create an option with the given description, name,
    /// and `Default::default()` as the default value.
    ///
    /// The only exception is that [`Str`](crate::option::Str)
    /// uses `std::ptr::null_mut()` as it's default value since
    /// pointers do not implement `Default`.
    ///
    /// Normally, this should only be called by the derive macro
    /// for `Options`.
    fn defaulted(name: &'static CStr, desc: &'static CStr) -> Self;

    /// The name of this option
    fn name(&self) -> &'static CStr;
    /// A C string describing this option
    fn desc(&self) -> &'static CStr;
    /// The current value of this option
    fn value(&self) -> Self::Value;
    /// The default value for this option
    fn default(&self) -> Self::Value;
    /// Whether the option has been set externally
    fn is_set(&self) -> bool;

    /// Overwrite the current value for the option.
    ///
    /// This will always set `is_set` to true.
    fn set_value(&mut self, val: Self::Value);
}

/// A type that can be safely viewed as a contiguous
/// array of [`option`s][0].
///
/// See [`OptionsExt`] for more useful functions on
/// `Options`.
///
/// This trait should normally only be implemented through
/// `#[derive(Options)]`. However, it must be implemented
/// manually for C types which have been bound using bindgen.
///
/// [0]: ../../cc_binding/struct.option.html
pub unsafe trait Options: Sized {
    fn new() -> Self;
}

pub trait OptionExt: Options {
    /// The number of options in this object when it
    /// is interpreted as an array.
    ///
    /// # Panics
    /// Panics if the size of this type is not a multiple
    /// of thie size of `option`.
    fn num_options() -> usize {
        use std::mem::size_of;

        // If this assert fails then there was no way that
        // options upholds it's safety requirements so it's
        // better to fail here.
        assert!(size_of::<Self>() % size_of::<option>() == 0);

        // If this assert fails then we'll pass an invalid
        // size to several ccommon methods.
        assert!(size_of::<Self>() / size_of::<option>() < std::u32::MAX as usize);

        size_of::<Self>() / size_of::<option>()
    }

    /// Get `self` as a const pointer to an array of `option`s.
    ///
    /// # Panics
    /// Panics if the size of this type is not a multiple
    /// of thie size of `option`.
    fn as_ptr(&self) -> *const option {
        self as *const _ as *const option
    }

    /// Get `self` as a mutable pointer to an array of `option`s.
    ///
    /// # Panics
    /// Panics if the size of this type is not a multiple
    /// of thie size of `option`.
    fn as_mut_ptr(&mut self) -> *mut option {
        self as *mut _ as *mut option
    }

    /// Get `self` as a slice of `option`s.
    fn as_slice(&self) -> &[option] {
        use std::slice;

        // Safe because implementing the trait means that layout
        // is guaranteed.
        unsafe { slice::from_raw_parts(self.as_ptr(), Self::num_options()) }
    }

    /// Get `self` as a mutable slice of `option`s.
    fn as_mut_slice(&mut self) -> &mut [option] {
        use std::slice;

        // Safe because implementing the trait means that layout
        // is guaranteed.
        unsafe { slice::from_raw_parts_mut(self.as_mut_ptr(), Self::num_options()) }
    }

    /// Print a description of all options in the current object
    /// given using the default value, name, and description.
    ///
    /// Internally this calls out to `option_describe_all`.
    fn describe_all(&self) {
        unsafe {
            option_describe_all(&mut std::io::stdout(), self.as_slice())
                .expect("Failed to write to stdout");
        }
    }

    /// Print out the values of all options.
    ///
    /// Internally this calls out to `option_print_all`.
    fn print_all(&self) {
        unsafe {
            option_print_all(&mut std::io::stdout(), self.as_slice())
                .expect("Failed to write to stdout")
        }
    }

    /// Load default values for all options.
    ///
    /// Internally this calls `option_load_default`
    fn load_default(&mut self) -> Result<(), crate::Error> {
        unsafe { option_load_default(self.as_mut_slice()).map_err(|_| crate::Error::ENoMem) }
    }

    /// Load options from a file.
    #[deprecated(note = "Use load instead - this interface is unsafe")]
    fn load_from_libc_file(&mut self, file: *mut libc::FILE) -> Result<(), crate::Error> {
        use ccommon_backend::compat::CFileRef;
        use std::io::BufReader;

        let cfile = unsafe { CFileRef::from_ptr_mut(file) };
        self.load(&mut BufReader::new(cfile))
            .map_err(|_| crate::Error::EOther)
    }

    /// Load options from a reader implementing `BufRead`.
    fn load<R: std::io::BufRead>(&mut self, input: &mut R) -> Result<(), ParseError<'static>> {
        option_load(self.as_mut_slice(), input)
    }
}

impl<T: Options> OptionExt for T {}

/// Implementations of Options for cc_bindings types
mod impls {
    use super::Options;
    use cc_binding::*;

    macro_rules! c_str {
        ($s:expr) => {
            concat!($s, "\0").as_bytes().as_ptr() as *const i8 as *mut _
        };
    }

    macro_rules! initialize_option_value {
        (OPTION_TYPE_BOOL, $default:expr) => {
            option_val_u { vbool: $default }
        };
        (OPTION_TYPE_UINT, $default:expr) => {
            option_val_u {
                vuint: $default.into(),
            }
        };
        (OPTION_TYPE_FPN, $default:expr) => {
            option_val_u { vfpn: $default }
        };
        (OPTION_TYPE_STR, $default:expr) => {
            option_val_u { vstr: $default }
        };
    }

    macro_rules! impl_options {
        {
            $(
                impl Options for $metrics_ty:ty {
                    $(
                        ACTION( $field:ident, $type:ident, $default:expr, $desc:expr )
                    )*
                }
            )*
        } => {
            $(
                unsafe impl Options for $metrics_ty {
                    fn new() -> Self {
                        Self {
                            $(
                                $field: option {
                                    name: c_str!($desc),
                                    set: false,
                                    type_: $type,
                                    default_val: initialize_option_value!($type, $default),
                                    val: initialize_option_value!($type, $default),
                                    description: c_str!($desc)
                                },
                            )*
                        }
                    }
                }
            )*
        }
    }

    impl_options! {
        impl Options for buf_options_st {
            ACTION( buf_init_size,  OPTION_TYPE_UINT,   BUF_DEFAULT_SIZE,   "init buf size incl header" )
            ACTION( buf_poolsize,   OPTION_TYPE_UINT,   BUF_POOLSIZE,       "buf pool size"             )
        }

        impl Options for dbuf_options_st {
            ACTION( dbuf_max_power,      OPTION_TYPE_UINT,  DBUF_DEFAULT_MAX,   "max number of doubles")
        }

        impl Options for pipe_options_st {
            ACTION( pipe_poolsize,      OPTION_TYPE_UINT,   PIPE_POOLSIZE,  "pipe conn pool size" )
        }

        impl Options for tcp_options_st {
            ACTION( tcp_backlog,    OPTION_TYPE_UINT,   TCP_BACKLOG,    "tcp conn backlog limit" )
            ACTION( tcp_poolsize,   OPTION_TYPE_UINT,   TCP_POOLSIZE,   "tcp conn pool size"     )
        }

        impl Options for sockio_options_st {
            ACTION( buf_sock_poolsize,  OPTION_TYPE_UINT,   BUFSOCK_POOLSIZE,   "buf_sock limit" )
        }

        impl Options for array_options_st {
            ACTION( array_nelem_delta,  OPTION_TYPE_UINT,   NELEM_DELTA,      "max nelem delta during expansion" )
        }

        impl Options for debug_options_st {
            ACTION( debug_log_level, OPTION_TYPE_UINT, DEBUG_LOG_LEVEL,  "debug log level"     )
            ACTION( debug_log_file,  OPTION_TYPE_STR,  DEBUG_LOG_FILE,   "debug log file"      )
            ACTION( debug_log_nbuf,  OPTION_TYPE_UINT, DEBUG_LOG_NBUF,   "debug log buf size"  )
        }

        impl Options for stats_log_options_st {
            ACTION( stats_log_file, OPTION_TYPE_STR,  std::ptr::null_mut(), "file storing stats"   )
            ACTION( stats_log_nbuf, OPTION_TYPE_UINT, STATS_LOG_NBUF,       "stats log buf size"   )
        }
    }
}

use crate::Options;

#[cfg(test)]
mod test {
    use super::*;
    use crate::Options;

    #[derive(Options)]
    #[repr(C)]
    struct TestOptions {
        #[option(desc = "The first test option")]
        opt1: Bool,

        #[option(desc = "The second test option", default = 5)]
        opt2: UInt,

        #[option(desc = "The third test option", default = 35.0)]
        opt3: Float,
    }

    #[test]
    fn test_option_properties() {
        assert_eq!(TestOptions::num_options(), 3);
    }

    #[test]
    fn test_option_defaults() {
        let options = TestOptions::new();
        let ptr =
            unsafe { std::slice::from_raw_parts(options.as_ptr(), TestOptions::num_options()) };

        unsafe {
            assert_eq!(ptr[0].default_val.vbool, false);
            assert_eq!(ptr[0].set, false);

            assert_eq!(ptr[1].default_val.vuint, 5);
            assert_eq!(ptr[1].set, false);

            assert_eq!(ptr[2].default_val.vfpn, 35.0);
            assert_eq!(ptr[2].set, false);
        }
    }

    #[test]
    fn option_size_sanity() {
        // Protect against a bad bindgen run
        assert!(std::mem::size_of::<option>() != 0);
    }
}
