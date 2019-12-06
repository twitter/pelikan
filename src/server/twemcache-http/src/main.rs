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
extern crate thiserror;
#[macro_use]
extern crate log;
#[macro_use]
extern crate pelikan_sys;

mod admin;
mod http;
mod memcached;
mod metrics;
mod options;
mod worker;

use crate::admin::Handler;
use crate::metrics::Metrics;
use crate::options::Options;
use crate::worker::Worker;

use ccommon::option::SingleOption;
use rustcore::Core;
use tokio::sync::mpsc::channel;

use std::cell::RefCell;
use std::fs::File;
use std::rc::Rc;

const LONG_USE_STR: &str = r#"
  pelikan_dual is one of the unified cache backends.
  In addition to supporting everything that pelikan_twemcache
  does, it also supports a basic HTTP REST protocol.
"#;

const EXAMPLE_STR: &str = r#"
Example:
  pelikan_dual pelikan.conf
"#;

fn build_args() -> clap::App<'static, 'static> {
    use clap::{App, Arg};

    App::new("pelikan_dual")
        .about("pelikan_dual is a unified cache backend supporting\n both memcached and HTTP protocols")
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

fn setup(opts: &mut Options, metrics: &mut Metrics) {
    use crate::memcached::sys::process_setup;
    use ccommon_sys::*;
    use pelikan_sys::{
        hotkey::hotkey_setup, protocol::memcache::*, storage::slab::*, time::time_setup,
        util::procinfo_setup,
    };
    use std::ptr::null_mut;

    unsafe {
        log_setup(&mut metrics.log);
        if debug_setup(&mut opts.debug) < 0 {
            panic!("debug log setup failed");
        }

        // Setup top-level application options
        if opts.server.daemonize.value() {
            pelikan_sys::util::daemonize();
        }

        let fname = opts.server.pid_filename.value();
        if !fname.is_null() {
            /* to get the correct pid, call create_pidfile after daemonize */
            pelikan_sys::util::create_pidfile(fname);
        }

        // Setup library modules.
        stats_log_setup(&mut opts.stats_log);
        buf_setup(&mut opts.buf, &mut metrics.buf);
        dbuf_setup(&mut opts.dbuf, &mut metrics.dbuf);
        sockio_setup(&mut opts.sockio, &mut metrics.sockio);

        // Setup pelikan modules
        time_setup(&mut opts.time);
        procinfo_setup(&mut metrics.procinfo);
        request_setup(&mut opts.request, &mut metrics.request);
        response_setup(&mut opts.response, &mut metrics.response);
        parse_setup(&mut metrics.parse_req, null_mut());
        compose_setup(null_mut(), &mut metrics.compose_rsp);
        klog_setup(&mut opts.klog, &mut metrics.klog);
        hotkey_setup(&mut opts.hotkey);
        slab_setup(&mut opts.slab, &mut metrics.slab);

        // Setup memcached server
        process_setup(&mut opts.process, &mut metrics.process);
    }
}

fn main() {
    use ccommon::{
        metric::{MetricExt, Metrics as CCMetrics},
        option::{OptionExt, Options as CCOptions},
    };
    use pelikan_sys::{VERSION_MAJOR, VERSION_MINOR, VERSION_PATCH};

    let mut options = Options::new();
    let metrics = Box::new(Metrics::new());
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
        options.describe_all();
        return;
    }

    if args.is_present("stats") {
        metrics.describe_all();
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
        options.load(&mut file).expect("Failed to load config");
    }

    setup(&mut options, metrics);
    let metrics = &*metrics;

    let worker = Rc::new(RefCell::new(Worker::new()));

    let admin_addr = options.admin.addr().expect("Invalid admin socket");
    let admin_dlog = options.admin.dlog_intvl();
    let handler = Handler::new(metrics);
    let mut core = Core::new(move || {
        rustcore::admin::admin_tcp(admin_addr, handler, admin_dlog, &metrics.admin)
    })
    .expect("Unable to setup rustcore");

    let (http_send, http_recv) = channel(1024);
    let (twem_send, twem_recv) = channel(1024);

    let http_addr = options.http.addr().expect("Invalid HTTP Address");
    let twem_addr = options.twemcache.addr().expect("Invalid Twemcache Address");

    core.listener(async move {
        rustcore::tcp_listener(http_addr, http_send, &metrics.listener)
            .await
            .expect("HTTP Listener panicked");
    });
    core.worker(http_recv, worker, &metrics.worker, crate::http::http_worker);

    core.listener(async move {
        rustcore::tcp_listener(twem_addr, twem_send, &metrics.listener)
            .await
            .expect("Twemcache Listener panicked");
    });
    core.worker(
        twem_recv,
        Rc::new(()),
        &metrics.worker,
        crate::memcached::memcached_worker,
    );

    core.run().expect("Failed to run core");
}
