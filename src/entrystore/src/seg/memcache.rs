// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! This module defines how `Seg` storage will be used to execute `Memcache`
//! storage commands.

use super::*;
use protocol_common::*;

use protocol_memcache::*;

use std::time::Duration;

impl Execute<Request, Response> for Seg {
    fn execute(&mut self, request: &Request) -> Response {
        match request {
            Request::Get(get) => self.get(get),
            Request::Gets(gets) => self.gets(gets),
            Request::Set(set) => self.set(set),
            Request::Add(add) => self.add(add),
            Request::Replace(replace) => self.replace(replace),
            Request::Cas(cas) => self.cas(cas),
            Request::Incr(incr) => self.incr(incr),
            Request::Decr(decr) => self.decr(decr),
            Request::Append(append) => self.append(append),
            Request::Prepend(prepend) => self.prepend(prepend),
            Request::Delete(delete) => self.delete(delete),
            Request::FlushAll(flush_all) => self.flush_all(flush_all),
            Request::Quit(quit) => self.quit(quit),
        }
    }
}

impl Storage for Seg {
    fn get(&mut self, get: &Get) -> Response {
        let mut values = Vec::with_capacity(get.keys().len());
        for key in get.keys().iter() {
            if let Some(item) = self.data.get(key) {
                let o = item.optional().unwrap_or(&[0, 0, 0, 0]);
                let flags = u32::from_be_bytes([o[0], o[1], o[2], o[3]]);
                match item.value() {
                    seg::Value::Bytes(b) => {
                        values.push(Value::new(item.key(), flags, None, b));
                    }
                    seg::Value::U64(v) => {
                        values.push(Value::new(
                            item.key(),
                            flags,
                            None,
                            format!("{}", v).as_bytes(),
                        ));
                    }
                }
            } else {
                values.push(Value::none(key));
            }
        }
        Values::new(values.into_boxed_slice()).into()
    }

    fn gets(&mut self, get: &Gets) -> Response {
        let mut values = Vec::with_capacity(get.keys().len());
        for key in get.keys().iter() {
            if let Some(item) = self.data.get(key) {
                let o = item.optional().unwrap_or(&[0, 0, 0, 0]);
                let flags = u32::from_be_bytes([o[0], o[1], o[2], o[3]]);
                match item.value() {
                    seg::Value::Bytes(b) => {
                        values.push(Value::new(item.key(), flags, Some(item.cas().into()), b));
                    }
                    seg::Value::U64(v) => {
                        values.push(Value::new(
                            item.key(),
                            flags,
                            Some(item.cas().into()),
                            format!("{}", v).as_bytes(),
                        ));
                    }
                }
            } else {
                values.push(Value::none(key));
            }
        }
        Values::new(values.into_boxed_slice()).into()
    }

    fn set(&mut self, set: &Set) -> Response {
        let ttl = set.ttl().get().unwrap_or(0);

        if ttl < 0 {
            // immediate expire maps to a delete
            self.data.delete(set.key());
            Response::stored(set.noreply())
        } else if let Ok(s) = std::str::from_utf8(set.value()) {
            if let Ok(v) = s.parse::<u64>() {
                if self
                    .data
                    .insert(
                        set.key(),
                        v,
                        Some(&set.flags().to_be_bytes()),
                        Duration::from_secs(ttl as u64),
                    )
                    .is_ok()
                {
                    Response::stored(set.noreply())
                } else {
                    Response::server_error("")
                }
            } else if self
                .data
                .insert(
                    set.key(),
                    set.value(),
                    Some(&set.flags().to_be_bytes()),
                    Duration::from_secs(ttl as u64),
                )
                .is_ok()
            {
                Response::stored(set.noreply())
            } else {
                Response::server_error("")
            }
        } else if self
            .data
            .insert(
                set.key(),
                set.value(),
                Some(&set.flags().to_be_bytes()),
                Duration::from_secs(ttl as u64),
            )
            .is_ok()
        {
            Response::stored(set.noreply())
        } else {
            Response::server_error("")
        }
    }

    fn add(&mut self, add: &Add) -> Response {
        if self.data.get_no_freq_incr(add.key()).is_some() {
            return Response::not_stored(add.noreply());
        }

        let ttl = add.ttl().get().unwrap_or(0);

        if ttl < 0 {
            // immediate expire maps to a delete
            self.data.delete(add.key());
            Response::stored(add.noreply())
        } else if let Ok(s) = std::str::from_utf8(add.value()) {
            if let Ok(v) = s.parse::<u64>() {
                if self
                    .data
                    .insert(
                        add.key(),
                        v,
                        Some(&add.flags().to_be_bytes()),
                        Duration::from_secs(ttl as u64),
                    )
                    .is_ok()
                {
                    Response::stored(add.noreply())
                } else {
                    Response::server_error("")
                }
            } else if self
                .data
                .insert(
                    add.key(),
                    add.value(),
                    Some(&add.flags().to_be_bytes()),
                    Duration::from_secs(ttl as u64),
                )
                .is_ok()
            {
                Response::stored(add.noreply())
            } else {
                Response::server_error("")
            }
        } else if self
            .data
            .insert(
                add.key(),
                add.value(),
                Some(&add.flags().to_be_bytes()),
                Duration::from_secs(ttl as u64),
            )
            .is_ok()
        {
            Response::stored(add.noreply())
        } else {
            Response::server_error("")
        }
    }

    fn replace(&mut self, replace: &Replace) -> Response {
        if self.data.get_no_freq_incr(replace.key()).is_none() {
            return Response::not_stored(replace.noreply());
        }

        let ttl = replace.ttl().get().unwrap_or(0);

        if ttl < 0 {
            // immediate expire maps to a delete
            self.data.delete(replace.key());
            Response::stored(replace.noreply())
        } else if let Ok(s) = std::str::from_utf8(replace.value()) {
            if let Ok(v) = s.parse::<u64>() {
                if self
                    .data
                    .insert(
                        replace.key(),
                        v,
                        Some(&replace.flags().to_be_bytes()),
                        Duration::from_secs(ttl as u64),
                    )
                    .is_ok()
                {
                    Response::stored(replace.noreply())
                } else {
                    Response::server_error("")
                }
            } else if self
                .data
                .insert(
                    replace.key(),
                    replace.value(),
                    Some(&replace.flags().to_be_bytes()),
                    Duration::from_secs(ttl as u64),
                )
                .is_ok()
            {
                Response::stored(replace.noreply())
            } else {
                Response::server_error("")
            }
        } else if self
            .data
            .insert(
                replace.key(),
                replace.value(),
                Some(&replace.flags().to_be_bytes()),
                Duration::from_secs(ttl as u64),
            )
            .is_ok()
        {
            Response::stored(replace.noreply())
        } else {
            Response::server_error("")
        }
    }

    fn append(&mut self, _: &Append) -> Response {
        Response::error()
    }

    fn prepend(&mut self, _: &Prepend) -> Response {
        Response::error()
    }

    fn incr(&mut self, incr: &Incr) -> Response {
        match self.data.wrapping_add(incr.key(), incr.value()) {
            Ok(item) => match item.value() {
                seg::Value::U64(v) => Response::numeric(v, incr.noreply()),
                _ => Response::server_error(""),
            },
            Err(SegError::NotFound) => Response::not_found(incr.noreply()),
            Err(SegError::NotNumeric) => Response::error(),
            Err(_) => Response::server_error(""),
        }
    }

    fn decr(&mut self, decr: &Decr) -> Response {
        match self.data.saturating_sub(decr.key(), decr.value()) {
            Ok(item) => match item.value() {
                seg::Value::U64(v) => Response::numeric(v, decr.noreply()),
                _ => Response::server_error(""),
            },
            Err(SegError::NotFound) => Response::not_found(decr.noreply()),
            Err(SegError::NotNumeric) => Response::error(),
            Err(_) => Response::server_error(""),
        }
    }

    fn cas(&mut self, cas: &Cas) -> Response {
        // duration of zero is treated as no expiry. as we have
        // no way of checking the cas value without performing a cas
        // and checking the result, setting the shortest possible ttl
        // results in nearly immediate expiry
        let ttl = cas.ttl().get().unwrap_or(1);

        let ttl = if ttl < 0 {
            Duration::from_secs(0)
        } else {
            Duration::from_secs(ttl as u64)
        };

        if let Ok(s) = std::str::from_utf8(cas.value()) {
            if let Ok(v) = s.parse::<u64>() {
                match self.data.cas(
                    cas.key(),
                    v,
                    Some(&cas.flags().to_be_bytes()),
                    ttl,
                    cas.cas() as u32,
                ) {
                    Ok(_) => Response::stored(cas.noreply()),
                    Err(SegError::NotFound) => Response::not_found(cas.noreply()),
                    Err(SegError::Exists) => Response::exists(cas.noreply()),
                    Err(_) => Response::error(),
                }
            } else {
                match self.data.cas(
                    cas.key(),
                    cas.value(),
                    Some(&cas.flags().to_be_bytes()),
                    ttl,
                    cas.cas() as u32,
                ) {
                    Ok(_) => Response::stored(cas.noreply()),
                    Err(SegError::NotFound) => Response::not_found(cas.noreply()),
                    Err(SegError::Exists) => Response::exists(cas.noreply()),
                    Err(_) => Response::error(),
                }
            }
        } else {
            match self.data.cas(
                cas.key(),
                cas.value(),
                Some(&cas.flags().to_be_bytes()),
                ttl,
                cas.cas() as u32,
            ) {
                Ok(_) => Response::stored(cas.noreply()),
                Err(SegError::NotFound) => Response::not_found(cas.noreply()),
                Err(SegError::Exists) => Response::exists(cas.noreply()),
                Err(_) => Response::error(),
            }
        }
    }

    fn delete(&mut self, delete: &Delete) -> Response {
        if self.data.delete(delete.key()) {
            Response::deleted(delete.noreply())
        } else {
            Response::not_found(delete.noreply())
        }
    }

    fn flush_all(&mut self, _flush_all: &FlushAll) -> Response {
        Response::error()
    }

    fn quit(&mut self, _quit: &Quit) -> Response {
        Response::hangup()
    }
}
