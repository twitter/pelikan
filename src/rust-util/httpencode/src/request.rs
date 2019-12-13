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

use crate::{HttpBuilder, Method, Result, Uri, Version};
use bytes::BufMut;

pub fn get<B: BufMut>(buf: B, path: Uri) -> Result<HttpBuilder<B>> {
    HttpBuilder::request(buf, Method::Get, Version::Http11, path)
}

pub fn post<B: BufMut>(buf: B, path: Uri) -> Result<HttpBuilder<B>> {
    HttpBuilder::request(buf, Method::Post, Version::Http11, path)
}

pub fn put<B: BufMut>(buf: B, path: Uri) -> Result<HttpBuilder<B>> {
    HttpBuilder::request(buf, Method::Put, Version::Http11, path)
}

pub fn head<B: BufMut>(buf: B, path: Uri) -> Result<HttpBuilder<B>> {
    HttpBuilder::request(buf, Method::Head, Version::Http11, path)
}

pub fn options<B: BufMut>(buf: B, path: Uri) -> Result<HttpBuilder<B>> {
    HttpBuilder::request(buf, Method::Options, Version::Http11, path)
}

pub fn patch<B: BufMut>(buf: B, path: Uri) -> Result<HttpBuilder<B>> {
    HttpBuilder::request(buf, Method::Patch, Version::Http11, path)
}

pub fn delete<B: BufMut>(buf: B, path: Uri) -> Result<HttpBuilder<B>> {
    HttpBuilder::request(buf, Method::Delete, Version::Http11, path)
}

pub fn trace<B: BufMut>(buf: B, path: Uri) -> Result<HttpBuilder<B>> {
    HttpBuilder::request(buf, Method::Trace, Version::Http11, path)
}
