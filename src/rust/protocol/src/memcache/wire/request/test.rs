// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::memcache::MemcacheRequest;
use crate::*;

#[test]
fn get() {
    // keysets which are used for requests
    let keysets = vec![
        vec!["0"],
        vec!["0", "1"],
        vec!["0", "1", "2"],
        vec!["espresso"],
        vec!["ice", "cofffee"],
    ];

    // covers get on single and multiple keys
    for keyset in keysets {
        println!("keyset: {:?}", keyset);
        let request = MemcacheRequest::parse(format!("get {}\r\n", keyset.join(" ")).as_bytes())
            .expect("parse failure");
        if let MemcacheRequest::Get { keys } = request.message {
            println!("keys: {:?}", keys);
            assert_eq!(keys.len(), keyset.len());
            for (id, key) in keyset.iter().enumerate() {
                assert_eq!(keys[id].as_ref(), key.as_bytes());
            }
        } else {
            panic!("invalid parse result");
        }
    }

    let request = MemcacheRequest::parse(b"get 0\r\n1 ").expect("parse failure");
    if let MemcacheRequest::Get { keys } = request.message {
        println!("keys: {:?}", keys);
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].as_ref(), b"0");
    } else {
        panic!("invalid parse result");
    }
}

#[test]
fn gets() {
    // keysets which are used for requests
    let keysets = vec![
        vec!["0"],
        vec!["0", "1"],
        vec!["0", "1", "2"],
        vec!["espresso"],
        vec!["ice", "cofffee"],
    ];

    // covers get on single and multiple keys
    for keyset in keysets {
        println!("keyset: {:?}", keyset);
        let request = MemcacheRequest::parse(format!("gets {}\r\n", keyset.join(" ")).as_bytes())
            .expect("parse failure");
        if let MemcacheRequest::Gets { keys } = request.message {
            println!("keys: {:?}", keys);
            assert_eq!(keys.len(), keyset.len());
            for (id, key) in keyset.iter().enumerate() {
                assert_eq!(keys[id].as_ref(), key.as_bytes());
            }
        } else {
            panic!("invalid parse result");
        }
    }
}

#[test]
fn set() {
    // keysets which are used for requests
    let keys = vec!["0", "1", "espresso"];

    let values = vec!["0", "1", "coffee is important"];

    for key in &keys {
        for value in &values {
            let request_str = format!("set {} 0 0 {}\r\n{}\r\n", key, value.len(), value);
            println!("request: {}", request_str);
            let request = MemcacheRequest::parse(request_str.as_bytes()).expect("parse failure");
            if let MemcacheRequest::Set { entry, noreply } = request.message {
                assert_eq!(entry.key(), key.as_bytes());
                assert_eq!(entry.value(), value.as_bytes());
                assert_eq!(entry.cas(), None);
                assert!(!noreply);
            } else {
                panic!("invalid parse result");
            }
        }
    }

    let request = MemcacheRequest::parse(b"set 0 0 0 1 noreply\r\n0\r\n").expect("parse failure");
    if let MemcacheRequest::Set { entry, noreply } = request.message {
        assert_eq!(entry.key(), b"0");
        assert_eq!(entry.value(), b"0");
        assert!(noreply);
    } else {
        panic!("invalid parse result");
    }
}

#[test]
fn cas() {
    let request = MemcacheRequest::parse(b"cas 0 0 0 1 0\r\n0\r\n").expect("parse failure");
    if let MemcacheRequest::Cas { entry, noreply } = request.message {
        assert_eq!(entry.key(), b"0");
        assert_eq!(entry.value(), b"0");
        assert_eq!(entry.cas(), Some(0));
        assert!(!noreply);
    } else {
        panic!("invalid parse result");
    }
}

#[test]
fn incomplete() {
    // incomplete
    if let Err(e) = MemcacheRequest::parse(b"get partial") {
        if e != ParseError::Incomplete {
            panic!("invalid parse result");
        }
    } else {
        panic!("invalid parse result");
    }
}

#[test]
fn trailing_whitespace() {
    // get
    let request = MemcacheRequest::parse(b"get key \r\n").expect("parse failure");
    if let MemcacheRequest::Get { keys } = request.message {
        println!("keys: {:?}", keys);
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].as_ref(), b"key");
    } else {
        panic!("invalid parse result");
    }

    // set
    let request = MemcacheRequest::parse(b"set 0 0 0 1 \r\n1\r\n").expect("parse failure");
    if let MemcacheRequest::Set { entry, noreply } = request.message {
        assert_eq!(entry.key(), b"0");
        assert_eq!(entry.value(), b"1");
        assert!(!noreply);
    } else {
        panic!("invalid parse result");
    }

    // set + noreply
    let request = MemcacheRequest::parse(b"set 0 0 0 1 noreply \r\n1\r\n").expect("parse failure");
    if let MemcacheRequest::Set { entry, noreply } = request.message {
        assert_eq!(entry.key(), b"0");
        assert_eq!(entry.value(), b"1");
        assert!(noreply);
    } else {
        panic!("invalid parse result");
    }
}

#[test]
fn invalid() {
    // invalid
    for request in &[
        "get \r\n",
        "get this     is    malformed\r\n",
        "get\r\n",
        "get lorem_ipsum_dolor_sit_amet_consectetur_adipiscing_elit_sed_do_\
        eiusmod_tempor_incididunt_ut_labore_et_dolore_magna_aliqua_Ut_enim_ad_\
        minim_veniam_quis_nostrud_exercitation_ullamco_laboris_nisi_ut_aliquip\
        _ex_ea_commodo_consequat_Duis_aute_irure_dolor_in_reprehenderit\r\n",
        "get lorem_ipsum_dolor_sit_amet_consectetur_adipiscing_elit_sed_do_\
        eiusmod_tempor_incididunt_ut_labore_et_dolore_magna_aliqua_Ut_enim_ad_\
        minim_veniam_quis_nostrud_exercitation_ullamco_laboris_nisi_ut_aliquip\
        _ex_ea_commodo_consequat_Duis_aute_irure_dolor_in_reprehenderit_\
        eiusmod_tempor_incididunt_ut_labore_et_dolore_magna_aliqua_Ut_enim_ad_\
        minim_veniam_quis_nostrud_exercitation_ullamco_laboris_nisi_ut_aliquip\
        _ex_ea_commodo_consequat_Duis_aute_irure_dolor_in_reprehenderit_",
        "lorem_ipsum_dolor_sit_amet_consectetur_adipiscing_elit_sed_do_eiusmod_\
        tempor_incididunt_ut_labore_et_dolore_magna_aliqua_Ut_enim_ad_minim_\
        veniam_quis_nostrud_exercitation_ullamco_laboris_nisi_ut_aliquip_ex_ea_\
        commodo_consequat_Duis_aute_irure_dolor_in_reprehenderit"
    ] {
        if let Err(e) = MemcacheRequest::parse(request.as_bytes()) {
            if e != ParseError::Invalid {
                panic!("invalid parse result");
            }
        } else {
            panic!("invalid parse result");
        }
    }
}

#[test]
fn pipelined() {
    let request = MemcacheRequest::parse(b"get 0\r\nget 1\r\n").expect("parse failure");
    if let MemcacheRequest::Get { keys } = request.message {
        assert!(keys.len() == 1);
        assert_eq!(keys[0].as_ref(), b"0");
    } else {
        panic!("invalid parse result");
    }
}