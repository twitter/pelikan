// Copyright (C) 2018-2020 Twitter, Inc.
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

use clap::{App, Arg};
use std::path::PathBuf;

fn main() {
    let matches = App::new("cdbgen")
        .version("0.1.0")
        .author("Jonathan Simms")
        .about("Creates a cdb file with n-byte keys for testing")
        .arg(
            Arg::with_name("OUTPUT")
                .help("path to write cdb to")
                .required(true)
                .index(1),
        )
        .get_matches();

    let output = PathBuf::from(matches.value_of("OUTPUT").unwrap());
    cdbgen::create(&output).unwrap();
}
