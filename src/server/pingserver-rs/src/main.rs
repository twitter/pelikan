#[macro_use]
extern crate log;

mod admin;
mod alloc;
mod data;
mod setting;
mod stats;
mod util;

use crate::admin::Handler;
use crate::setting::Settings;
use crate::stats::Metrics;

use pelikan::core::admin::Admin;

use libc::atexit;
use std::os::raw::c_char;

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
  in a typical production environment. \
"#;

const EXAMPLE_STR: &str = r#"
Example:
  pelikan_pingserver pingserver.conf
Sample config files can be found under the config dir.
"#;

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
    use crate::util::FileHandle;
    use ccommon::util::{AsMetricArray, AsOptionArray};
    use ccommon_sys::option_load_file;
    use pelikan_sys::{VERSION_MAJOR, VERSION_MINOR, VERSION_PATCH};

    let mut settings = Settings::new();
    let mut metrics = Metrics::new();

    let config = ccommon::log::LogConfigBuilder::default()
        // TODO(sean): Either rewrite the log adapter so this is not
        //             necessary or add code to create a temporary
        //             directory and use that.
        .path("".to_owned())
        .level(ccommon::log::Level::Info)
        .build()
        .map_err(|e| panic!("{}", e))
        .unwrap();
    let _handle = ccommon::log::log_setup(config);

    let args = build_args().get_matches();

    if args.is_present("version") {
        // TODO(sean): Figure out how to get this from cmake configuration
        println!(
            "Version: {}.{}.{}",
            VERSION_MAJOR, VERSION_MINOR, VERSION_PATCH
        );
        return;
    }

    if args.is_present("config") {
        unsafe { settings.describe_all() };
        return;
    }

    if args.is_present("stats") {
        unsafe { metrics.describe_all() };
        return;
    }

    let file = args
        .value_of("config-file")
        .map(|x| match FileHandle::open(x, "r+") {
            Ok(x) => x,
            Err(e) => panic!("Unable to open config: {}", e),
        });

    if let Some(ref file) = file {
        info!(
            "loading config from {}",
            args.value_of("config-file").unwrap()
        );
        let status = unsafe {
            let slice = settings.as_option_array_mut();

            option_load_file(
                file.handle(),
                slice.as_mut_ptr(),
                slice.len() as libc::c_uint,
            )
        };

        if status != ccommon_sys::CC_OK as libc::c_int {
            panic!("failed to load config");
        }
    }

    unsafe {
        // Ensure that cleanup happens in case of a non-standard exit
        atexit(atexit_handler);
    }

    let handler = ModuleRaiiHandler::new(&mut settings, &mut metrics).unwrap();
    unsafe {
        // This transmutes away the lifetime, it's definitely unsafe
        // but should still work since we make sure to clear RAII_HANDLER
        // before main's lifetime ends. (if atexit has called then
        // technically main's lifetime has not ended)
        RAII_HANDLER = Some(std::mem::transmute(handler));
    }

    let res = std::panic::catch_unwind(|| {
        ModuleRaiiHandler::run::<crate::data::PingDataProcessor>();
    });

    // Ensure that the handler is properly dropped in case of a panic,
    // not doing this would mean that RAII_HANDLER would outlive the
    // lifetime of settings and metrics.
    unsafe {
        let _ = RAII_HANDLER.take();
    }

    match res {
        Err(e) => std::panic::resume_unwind(e),
        Ok(_) => (),
    }
}

static mut RAII_HANDLER: Option<ModuleRaiiHandler<'static>> = None;

struct ModuleRaiiHandler<'a> {
    admin: Option<Admin<'a, Handler<'a>>>,
    fname: *const c_char,
}

impl<'a> ModuleRaiiHandler<'a> {
    unsafe fn _new(settings: &mut Settings, stats: &'a mut Metrics) -> Result<Self, String> {
        use ccommon_sys::*;
        use std::ptr::null_mut;

        use pelikan_sys::core::{core_admin_setup, core_server_setup, core_worker_setup};
        use pelikan_sys::protocol::ping::{compose_setup, parse_setup};
        use pelikan_sys::time::time_setup;
        use pelikan_sys::util::procinfo_setup;

        log_setup(&mut stats.log as *mut _);
        if debug_setup(&mut settings.debug as *mut _) < 0 {
            return Err("debug log setup failed".to_owned());
        }

        // Setup top-level application options
        if option_bool(&mut settings.pingserver.daemonize as *mut _) {
            pelikan_sys::util::daemonize();
        }

        let fname = option_str(&mut settings.pingserver.pid_filename as *mut _);
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

        let admin = Admin::new_global(Handler::new(stats)).unwrap();

        core_admin_setup(&settings.admin as *const _ as *mut _);
        core_server_setup(
            &settings.server as *const _ as *mut _,
            &stats.server as *const _ as *mut _,
        );
        core_worker_setup(
            &settings.worker as *const _ as *mut _,
            &stats.worker as *const _ as *mut _,
        );

        Ok(Self {
            admin: Some(admin),
            fname,
        })
    }

    pub fn new(settings: &'a mut Settings, metrics: &'a mut Metrics) -> Result<Self, String> {
        use ccommon_sys::{debug_log_flush, option_uint};
        use pelikan_sys::core::core_admin_register;
        use std::ptr::null_mut;

        unsafe {
            let handler = Self::_new(settings, metrics)?;

            let intvl = option_uint(&settings.pingserver.dlog_intvl as *const _ as *mut _);
            if core_admin_register(intvl, Some(debug_log_flush), null_mut()).is_null() {
                return Err("Could not register timed event to flush debug log".to_owned());
            }

            Ok(handler)
        }
    }

    pub fn run<DP>()
    where
        DP: pelikan::core::DataProcessor,
    {
        pelikan::core::core_run::<DP>();
    }
}

impl<'a> Drop for ModuleRaiiHandler<'a> {
    fn drop(&mut self) {
        use ccommon_sys::*;
        use pelikan_sys::{
            core::{core_admin_teardown, core_server_teardown, core_worker_teardown},
            protocol::ping::{compose_teardown, parse_teardown},
            time::time_teardown,
            util::{procinfo_teardown, remove_pidfile},
        };

        unsafe {
            remove_pidfile(self.fname);

            core_worker_teardown();
            core_server_teardown();
            core_admin_teardown();

            // Need to ensure drop order remains consistent
            let _ = self.admin.take();

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
