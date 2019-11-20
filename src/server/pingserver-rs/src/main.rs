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

#[macro_use]
extern crate log;
#[macro_use]
extern crate ccommon;

mod admin;
mod alloc;
mod data;
mod setting;
mod stats;

use crate::admin::Handler;
use crate::setting::Settings;
use crate::stats::Metrics;

use libc::atexit;

use std::fs::File;
use std::os::raw::c_char;
use std::panic::AssertUnwindSafe;

use ccommon::metric::{MetricExt, Metrics as _};
use ccommon::option::{OptionExt, Options as _, SingleOption};

// Override the global allocator to use _cc_alloc and _cc_free everywhere.
// This allows for a buffer to be allocated on the C side and freed on
// the Rust side or vice-versa.
#[global_allocator]
pub static ALLOC: crate::alloc::LoggedAlloc = crate::alloc::LoggedAlloc;

const LONG_USE_STR: &str = r#"\
  pelikan_pingserver is, arguably, the most over-engineered 
  ping server. 
  
  The purpose is to demonstrate how to create an otherwise 
  minimal service with the libraries and modules provided by
  Pelikan, which meets stringent requirements on latencies,
  observability, configurability, and other valuable traits
  in a typical production environment.\
"#;

const EXAMPLE_STR: &str = r#"
Example:
  pelikan_pingserver pingserver.conf
Sample config files can be found under the config dir.
"#;

#[no_mangle]
unsafe extern "C" fn malloc_error_break() {
    libc::printf("Detected heap corruption. Exiting now!\n\0".as_ptr() as *const libc::c_char);
    libc::exit(1);
}

fn build_args() -> clap::App<'static, 'static> {
    use clap::{App, Arg};

    App::new("pelikan_pingserver")
        .about("pelikan_pingserver is, arguably the most over-engineered\nping server.")
        .long_about(LONG_USE_STR)
        .after_help(EXAMPLE_STR)
        .arg_from_usage("-v, --version 'show version number'")
        .arg_from_usage("-c, --config  'list and describe all options in config'")
        .arg_from_usage("-s, --stats   'list and describe all metrics in stats'")
        .arg(
            Arg::with_name("config-file")
                .index(1)
                .help("a config file")
                .required(false),
        )
}

fn main() {
    use pelikan_sys::{VERSION_MAJOR, VERSION_MINOR, VERSION_PATCH};

    let mut settings = Settings::new();
    let metrics = Box::new(Metrics::new());
    // Need metrics to live forever
    let metrics: &'static mut Metrics = Box::leak(metrics);

    // Setup rust logging shim
    ccommon::log::init().expect("Failed to initialize logging shim");

    let args = build_args().get_matches();

    if args.is_present("version") {
        println!(
            "Version: {}.{}.{}",
            VERSION_MAJOR, VERSION_MINOR, VERSION_PATCH
        );
        return;
    }

    if args.is_present("config") {
        settings.describe_all();
        return;
    }

    if args.is_present("stats") {
        metrics.describe_all();
        return;
    }

    let file = args.value_of("config-file").map(|x| match File::open(x) {
        Ok(x) => std::io::BufReader::new(x),
        Err(e) => panic!("Unable to open config: {}", e),
    });

    if let Some(mut file) = file {
        info!(
            "loading config from {}",
            args.value_of("config-file").unwrap()
        );
        settings.load(&mut file).expect("Failed to load config");
    }

    unsafe {
        // Ensure that cleanup happens in case of a non-standard exit
        atexit(atexit_handler);
    }

    let handler = ModuleRaiiHandler::new(&mut settings, metrics).unwrap();
    unsafe {
        // This transmutes away the lifetime, it's definitely unsafe
        // but should still work since we make sure to clear RAII_HANDLER
        // before main's lifetime ends. (if atexit has called then
        // technically main's lifetime has not ended)
        RAII_HANDLER = Some(handler);
    }

    let admin = Handler::new(metrics);
    let worker = crate::data::PingWorker;
    let res = std::panic::catch_unwind(AssertUnwindSafe({
        let worker_metrics: &'static rustcore::CoreMetrics = &metrics.core;
        move || {
            let res = rustcore::core_run_tcp(
                &settings.admin,
                &settings.server,
                &worker_metrics,
                admin,
                worker,
            );

            match res {
                Ok(()) => (),
                Err(e) => {
                    panic!("An IO error occurrred: {}", e);
                }
            }
        }
    }));

    // Ensure that the handler is properly dropped in case of a panic,
    // not doing this would mean that RAII_HANDLER would outlive the
    // lifetime of settings and metrics.
    unsafe {
        let _ = RAII_HANDLER.take();
    }

    match res {
        Err(e) => std::panic::resume_unwind(e),
        Ok(()) => (),
    }
}

static mut RAII_HANDLER: Option<ModuleRaiiHandler> = None;

struct ModuleRaiiHandler {
    fname: *const c_char,
}

impl ModuleRaiiHandler {
    unsafe fn _new(settings: &mut Settings, stats: &mut Metrics) -> Result<Self, String> {
        use ccommon_sys::*;
        use std::ptr::null_mut;

        use pelikan_sys::protocol::ping::{compose_setup, parse_setup};
        use pelikan_sys::time::time_setup;
        use pelikan_sys::util::procinfo_setup;

        log_setup(&mut stats.log as *mut _);
        if debug_setup(&mut settings.debug as *mut _) < 0 {
            return Err("debug log setup failed".to_owned());
        }

        // Setup top-level application options
        if settings.pingserver.daemonize.value() {
            pelikan_sys::util::daemonize();
        }

        let fname = settings.pingserver.pid_filename.value();
        if !fname.is_null() {
            /* to get the correct pid, call create_pidfile after daemonize */
            pelikan_sys::util::create_pidfile(fname);
        }

        // Setup library modules
        buf_setup(&mut settings.buf as *mut _, &mut stats.buf as *mut _);
        dbuf_setup(&mut settings.dbuf as *mut _, &mut stats.dbuf as *mut _);
        event_setup(&mut stats.event as *mut _);
        sockio_setup(&mut settings.sockio as *mut _, &mut stats.sockio as *mut _);
        tcp_setup(&mut settings.tcp as *mut _, &mut stats.tcp as *mut _);
        timing_wheel_setup(&mut stats.timing_wheel as *mut _);

        // Setup pelikan modules
        time_setup(&mut settings.time as *mut _);
        procinfo_setup(&mut stats.procinfo as *mut _);
        parse_setup(&mut stats.parse_req as *mut _, null_mut());
        compose_setup(null_mut(), &mut stats.compose_rsp as *mut _);

        Ok(Self { fname })
    }

    pub fn new(settings: &mut Settings, metrics: &mut Metrics) -> Result<Self, String> {
        unsafe { Self::_new(settings, metrics) }
    }
}

impl Drop for ModuleRaiiHandler {
    fn drop(&mut self) {
        use ccommon_sys::*;
        use pelikan_sys::{
            core::{core_admin_teardown, core_server_teardown, core_worker_teardown},
            protocol::ping::{compose_teardown, parse_teardown},
            time::time_teardown,
            util::{procinfo_teardown, remove_pidfile},
        };

        unsafe {
            if !self.fname.is_null() {
                remove_pidfile(self.fname);
            }

            core_worker_teardown();
            core_server_teardown();
            core_admin_teardown();

            compose_teardown();
            parse_teardown();
            procinfo_teardown();
            time_teardown();

            timing_wheel_teardown();
            tcp_teardown();
            sockio_teardown();
            event_teardown();
            dbuf_teardown();
            buf_teardown();

            debug_teardown();
            log_teardown();
        }
    }
}

extern "C" fn atexit_handler() {
    unsafe {
        let _ = RAII_HANDLER.take();
    }
}
