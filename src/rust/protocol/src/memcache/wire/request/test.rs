// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::memcache::MemcacheRequest;
use crate::memcache::MemcacheRequestParser;
use crate::*;

#[test]
fn get() {
    let parser = MemcacheRequestParser::default();

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
        let request = parser
            .parse(format!("get {}\r\n", keyset.join(" ")).as_bytes())
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

    let request = parser.parse(b"get 0\r\n1 ").expect("parse failure");
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
    let parser = MemcacheRequestParser::default();

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
        let request = parser
            .parse(format!("gets {}\r\n", keyset.join(" ")).as_bytes())
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
    let parser = MemcacheRequestParser::default();

    // keysets which are used for requests
    let keys = vec!["0", "1", "espresso"];

    let values = vec!["0", "1", "coffee is important"];

    for key in &keys {
        for value in &values {
            let request_str = format!("set {} 0 0 {}\r\n{}\r\n", key, value.len(), value);
            println!("request: {}", request_str);
            let request = parser.parse(request_str.as_bytes()).expect("parse failure");
            if let MemcacheRequest::Set { entry, noreply } = request.message {
                assert_eq!(entry.key(), key.as_bytes());
                assert_eq!(entry.value(), Some(value.as_bytes()));
                assert_eq!(entry.cas(), None);
                assert!(!noreply);
            } else {
                panic!("invalid parse result");
            }
        }
    }

    let request = parser
        .parse(b"set 0 0 0 1 noreply\r\n0\r\n")
        .expect("parse failure");
    if let MemcacheRequest::Set { entry, noreply } = request.message {
        assert_eq!(entry.key(), b"0");
        assert_eq!(entry.value(), Some("0".as_bytes()));
        assert!(noreply);
    } else {
        panic!("invalid parse result");
    }
}

#[test]
fn cas() {
    let parser = MemcacheRequestParser::default();

    let request = parser
        .parse(b"cas 0 0 0 1 0\r\n0\r\n")
        .expect("parse failure");
    if let MemcacheRequest::Cas { entry, noreply } = request.message {
        assert_eq!(entry.key(), b"0");
        assert_eq!(entry.value(), Some("0".as_bytes()));
        assert_eq!(entry.cas(), Some(0));
        assert!(!noreply);
    } else {
        panic!("invalid parse result");
    }

    let request = parser
        .parse(b"cas 0 0 0 1 0 noreply\r\n0\r\n")
        .expect("parse failure");
    if let MemcacheRequest::Cas { entry, noreply } = request.message {
        assert_eq!(entry.key(), b"0");
        assert_eq!(entry.value(), Some("0".as_bytes()));
        assert_eq!(entry.cas(), Some(0));
        assert!(noreply);
    } else {
        panic!("invalid parse result");
    }
}

#[test]
fn delete() {
    let parser = MemcacheRequestParser::default();

    // keysets which are used for requests
    let keys = vec!["0", "1", "espresso"];

    // covers delete with reply
    for k in &keys {
        println!("key: {:?}", k);
        let request = parser
            .parse(format!("delete {}\r\n", k).as_bytes())
            .expect("parse failure");
        if let MemcacheRequest::Delete { key, noreply } = request.message {
            assert_eq!(key.as_ref(), k.as_bytes());
            assert!(!noreply);
        } else {
            panic!("invalid parse result");
        }
    }

    // covers delete with noreply
    for k in keys {
        println!("key: {:?}", k);
        let request = parser
            .parse(format!("delete {} noreply\r\n", k).as_bytes())
            .expect("parse failure");
        if let MemcacheRequest::Delete { key, noreply } = request.message {
            assert_eq!(key.as_ref(), k.as_bytes());
            assert!(noreply);
        } else {
            panic!("invalid parse result");
        }
    }
}

#[test]
fn flush_all() {
    let parser = MemcacheRequestParser::default();

    let request = parser.parse(b"flush_all\r\n").expect("parse failure");
    if let MemcacheRequest::FlushAll = request.message {
    } else {
        panic!("invalid parse result");
    }
}

#[test]
fn incomplete() {
    let parser = MemcacheRequestParser::default();

    // incomplete
    if let Err(e) = parser.parse(b"get partial") {
        if e != ParseError::Incomplete {
            panic!("invalid parse result");
        }
    } else {
        panic!("invalid parse result");
    }

    // incomplete
    if let Err(e) = parser.parse(b"delete ") {
        if e != ParseError::Incomplete {
            panic!("invalid parse result");
        }
    } else {
        panic!("invalid parse result");
    }

    // incomplete
    if let Err(e) = parser.parse(b"set 0 0 0 1\r\n") {
        if e != ParseError::Incomplete {
            panic!("invalid parse result");
        }
    } else {
        panic!("invalid parse result");
    }
}

#[test]
fn middle_whitespace() {
    let parser = MemcacheRequestParser::default();

    // get
    let request = parser.parse(b"get  a  b    c    d           e\r\n").expect("parse failure");
    if let MemcacheRequest::Get { keys } = request.message {
        println!("keys: {:?}", keys);
        assert_eq!(keys.len(), 5);
        assert_eq!(keys[0].as_ref(), b"a");
        assert_eq!(keys[1].as_ref(), b"b");
        assert_eq!(keys[2].as_ref(), b"c");
        assert_eq!(keys[3].as_ref(), b"d");
        assert_eq!(keys[4].as_ref(), b"e");
    } else {
        panic!("invalid parse result");
    }
}

#[test]
fn trailing_whitespace() {
    let parser = MemcacheRequestParser::default();

    // get
    let request = parser.parse(b"get key \r\n").expect("parse failure");
    if let MemcacheRequest::Get { keys } = request.message {
        println!("keys: {:?}", keys);
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].as_ref(), b"key");
    } else {
        panic!("invalid parse result");
    }

    // set
    let request = parser
        .parse(b"set 0 0 0 1 \r\n1\r\n")
        .expect("parse failure");
    if let MemcacheRequest::Set { entry, noreply } = request.message {
        assert_eq!(entry.key(), b"0");
        assert_eq!(entry.value(), Some("1".as_bytes()));
        assert!(!noreply);
    } else {
        panic!("invalid parse result");
    }

    // set + noreply
    let request = parser
        .parse(b"set 0 0 0 1 noreply \r\n1\r\n")
        .expect("parse failure");
    if let MemcacheRequest::Set { entry, noreply } = request.message {
        assert_eq!(entry.key(), b"0");
        assert_eq!(entry.value(), Some("1".as_bytes()));
        assert!(noreply);
    } else {
        panic!("invalid parse result");
    }

    // cas
    let request = parser
        .parse(b"cas 0 0 0 1 0 \r\n1\r\n")
        .expect("parse failure");
    if let MemcacheRequest::Cas { entry, noreply } = request.message {
        assert_eq!(entry.key(), b"0");
        assert_eq!(entry.value(), Some("1".as_bytes()));
        assert_eq!(entry.cas(), Some(0));
        assert!(!noreply);
    } else {
        panic!("invalid parse result");
    }

    // cas + noreply
    let request = parser
        .parse(b"cas 0 0 0 1 0 noreply \r\n1\r\n")
        .expect("parse failure");
    if let MemcacheRequest::Cas { entry, noreply } = request.message {
        assert_eq!(entry.key(), b"0");
        assert_eq!(entry.value(), Some("1".as_bytes()));
        assert_eq!(entry.cas(), Some(0));
        assert!(noreply);
    } else {
        panic!("invalid parse result");
    }
}

#[test]
fn invalid() {
    let parser = MemcacheRequestParser::default();

    // invalid
    for request in &[
        "get   \r\n",
        "get \r\n",
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
        commodo_consequat_Duis_aute_irure_dolor_in_reprehenderit",
        "cas \r 1 2 abc\r\n",
        "set 0 0 0 1\r\nhello\r\n",
    ] {
        if let Err(e) = parser.parse(request.as_bytes()) {
            if e != ParseError::Invalid {
                println!("request: {}", request);
                panic!("invalid parse result: {:?}", e);
            }
        } else {
            println!("request: {}", request);
            panic!("invalid parse result");
        }
    }
}

#[test]
fn pipelined() {
    let parser = MemcacheRequestParser::default();

    let request = parser.parse(b"get 0\r\nget 1\r\n").expect("parse failure");
    if let MemcacheRequest::Get { keys } = request.message {
        assert!(keys.len() == 1);
        assert_eq!(keys[0].as_ref(), b"0");
    } else {
        panic!("invalid parse result");
    }

    let request = parser.parse(b"get t\x0d\x0a ").expect("parse failure");
    if let MemcacheRequest::Get { keys } = request.message {
        assert!(keys.len() == 1);
        assert_eq!(keys[0].as_ref(), b"t");
    } else {
        panic!("invalid parse result");
    }
}
