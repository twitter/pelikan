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

use cc_binding::{
    option, option_type, option_val, OPTION_TYPE_BOOL, OPTION_TYPE_FPN, OPTION_TYPE_STR,
    OPTION_TYPE_UINT,
};

use std::ffi::CStr;
use std::io::{Result, Write};

unsafe fn fmt_cstr(ptr: *const libc::c_char) -> &'static str {
    CStr::from_ptr(ptr).to_str().unwrap()
}

fn fmt_type(ty: option_type) -> &'static str {
    match ty {
        OPTION_TYPE_BOOL => "bool",
        OPTION_TYPE_UINT => "unsigned int",
        OPTION_TYPE_FPN => "double",
        OPTION_TYPE_STR => "string",
        _ => "<unknown>",
    }
}

fn fmt_value<W: Write>(writer: &mut W, val: option_val, ty: option_type) -> Result<()> {
    unsafe {
        match ty {
            OPTION_TYPE_BOOL => write!(writer, "{: <20}", if val.vbool { "yes" } else { "no" }),
            OPTION_TYPE_UINT => write!(writer, "{: <20}", val.vuint),
            OPTION_TYPE_FPN => write!(writer, "{: <20}", val.vfpn),
            OPTION_TYPE_STR => {
                if val.vstr.is_null() {
                    write!(writer, "{: <20}", "NULL")
                } else {
                    write!(writer, "{: <20}", fmt_cstr(val.vstr))
                }
            }
            _ => write!(writer, "{: <20}", "<unknown>"),
        }
    }
}

/// Print a single option.
///
/// # Safety
/// This function assumes that the description and name
/// pointers within the options always point to a valid
/// C string. It also assumes that the value for string
/// type options is either a valid C string or null.
pub unsafe fn option_print<W: Write>(writer: &mut W, opt: &option) -> Result<()> {
    write!(
        writer,
        "name: {: <31} type: {: <15} current: ",
        fmt_cstr(opt.name),
        fmt_type(opt.type_)
    )?;
    fmt_value(writer, opt.val, opt.type_)?;
    write!(writer, " (default: ")?;
    fmt_value(writer, opt.default_val, opt.type_)?;
    writeln!(writer, ")")
}

/// Print all options.
///
/// # Safety
/// This function assumes that the description and name
/// pointers within the options always point to a valid
/// C string. It also assumes that the value for string
/// type options is either a valid C string or null.
pub unsafe fn option_print_all<W: Write>(writer: &mut W, options: &[option]) -> Result<()> {
    for opt in options.iter() {
        option_print(writer, opt)?;
    }

    Ok(())
}

/// Describe a single option.
///
/// # Safety
/// This function assumes that the description and name
/// pointers within the options always point to a valid
/// C string. It also assumes that the value for string
/// type options is either a valid C string or null.
pub unsafe fn option_describe<W: Write>(writer: &mut W, opt: &option) -> Result<()> {
    let name = fmt_cstr(opt.name);
    let desc = fmt_cstr(opt.description);

    write!(writer, "{: <31} {: <15} ", name, fmt_type(opt.type_))?;
    fmt_value(writer, opt.default_val, opt.type_)?;
    writeln!(writer, " {}\n", desc)
}

/// Describe all options.
///
/// # Safety
/// This function assumes that the description and name
/// pointers within the options always point to a valid
/// C string. It also assumes that the value for string
/// type options is either a valid C string or null.
pub unsafe fn option_describe_all<W: Write>(writer: &mut W, options: &[option]) -> Result<()> {
    for opt in options.iter() {
        option_describe(writer, opt)?;
    }

    Ok(())
}
