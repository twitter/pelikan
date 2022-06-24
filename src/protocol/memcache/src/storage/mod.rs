// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;

pub trait Storage {
    fn add(&mut self, request: &Add) -> Response;
    fn append(&mut self, request: &Append) -> Response;
    fn cas(&mut self, request: &Cas) -> Response;
    fn decr(&mut self, request: &Decr) -> Response;
    fn delete(&mut self, request: &Delete) -> Response;
    fn flush_all(&mut self, request: &FlushAll) -> Response;
    fn get(&mut self, request: &Get) -> Response;
    fn gets(&mut self, request: &Gets) -> Response;
    fn incr(&mut self, request: &Incr) -> Response;
    fn prepend(&mut self, request: &Prepend) -> Response;
    fn quit(&mut self, request: &Quit) -> Response;
    fn replace(&mut self, request: &Replace) -> Response;
    fn set(&mut self, request: &Set) -> Response;
}
