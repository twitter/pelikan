// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::memcache::MemcacheRequest;
use crate::memcache::MemcacheRequestParser;
use crate::*;
use storage_types::Value;

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
        vec!["ca0\0\0"],
    ];

    // covers get on single and multiple keys
    for keyset in keysets {
        println!("keyset: {:?}", keyset);
        let buffer = format!("get {}\r\n", keyset.join(" "));
        let request = parser.parse(buffer.as_bytes()).expect("parse failure");
        if let MemcacheRequest::Get { keys } = request.message {
            println!("keys: {:?}", keys);
            assert_eq!(keys.len(), keyset.len());
            for (id, key) in keyset.iter().enumerate() {
                assert_eq!(keys[id].as_ref(), key.as_bytes());
            }
        } else {
            panic!("invalid parse result");
        }
        assert_eq!(request.consumed, buffer.len());
    }

    let request = parser.parse(b"get 0\r\n1 ").expect("parse failure");
    if let MemcacheRequest::Get { keys } = request.message {
        println!("keys: {:?}", keys);
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].as_ref(), b"0");
    } else {
        panic!("invalid parse result");
    }
    assert_eq!(request.consumed, 7);
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
        let buffer = format!("gets {}\r\n", keyset.join(" "));
        let request = parser.parse(buffer.as_bytes()).expect("parse failure");
        if let MemcacheRequest::Gets { keys } = request.message {
            println!("keys: {:?}", keys);
            assert_eq!(keys.len(), keyset.len());
            for (id, key) in keyset.iter().enumerate() {
                assert_eq!(keys[id].as_ref(), key.as_bytes());
            }
        } else {
            panic!("invalid parse result");
        }
        assert_eq!(request.consumed, buffer.len());
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
            let buffer = format!("set {} 0 0 {}\r\n{}\r\n", key, value.len(), value);
            let value: Value = if let Ok(value) = value.parse::<u64>() {
                value.into()
            } else {
                value.as_bytes().into()
            };
            println!("request: {}", buffer);
            let request = parser.parse(buffer.as_bytes()).expect("parse failure");
            if let MemcacheRequest::Set { entry, noreply } = request.message {
                assert_eq!(entry.key(), key.as_bytes());
                assert_eq!(entry.value().unwrap(), value);
                assert_eq!(entry.cas(), None);
                assert!(!noreply);
            } else {
                panic!("invalid parse result");
            }
            assert_eq!(request.consumed, buffer.len());
        }
    }

    let buffer = b"set 0 0 0 1 noreply\r\n0\r\n";
    let request = parser.parse(buffer).expect("parse failure");
    if let MemcacheRequest::Set { entry, noreply } = request.message {
        assert_eq!(entry.key(), b"0");
        assert_eq!(entry.value().unwrap(), Value::U64(0));
        assert!(noreply);
    } else {
        panic!("invalid parse result");
    }
    assert_eq!(request.consumed, buffer.len());
}

#[test]
fn cas() {
    let parser = MemcacheRequestParser::default();

    let buffer = b"cas 0 0 0 1 0\r\n0\r\n";
    let request = parser.parse(buffer).expect("parse failure");
    if let MemcacheRequest::Cas { entry, noreply } = request.message {
        assert_eq!(entry.key(), b"0");
        assert_eq!(entry.value().unwrap(), Value::U64(0));
        assert_eq!(entry.cas(), Some(0));
        assert!(!noreply);
    } else {
        panic!("invalid parse result");
    }
    assert_eq!(request.consumed, buffer.len());

    let buffer = b"cas 0 0 0 1 0 noreply\r\n0\r\n";
    let request = parser.parse(buffer).expect("parse failure");
    if let MemcacheRequest::Cas { entry, noreply } = request.message {
        assert_eq!(entry.key(), b"0");
        assert_eq!(entry.value().unwrap(), Value::U64(0));
        assert_eq!(entry.cas(), Some(0));
        assert!(noreply);
    } else {
        panic!("invalid parse result");
    }
    assert_eq!(request.consumed, buffer.len());
}

#[test]
fn delete() {
    let parser = MemcacheRequestParser::default();

    // keysets which are used for requests
    let keys = vec!["0", "1", "espresso"];

    // covers delete with reply
    for k in &keys {
        println!("key: {:?}", k);
        let buffer = format!("delete {}\r\n", k);
        let request = parser.parse(buffer.as_bytes()).expect("parse failure");
        if let MemcacheRequest::Delete { key, noreply } = request.message {
            assert_eq!(key.as_ref(), k.as_bytes());
            assert!(!noreply);
        } else {
            panic!("invalid parse result");
        }
        assert_eq!(request.consumed, buffer.len());
    }

    // covers delete with noreply
    for k in keys {
        println!("key: {:?}", k);
        let buffer = format!("delete {} noreply\r\n", k);
        let request = parser.parse(buffer.as_bytes()).expect("parse failure");
        if let MemcacheRequest::Delete { key, noreply } = request.message {
            assert_eq!(key.as_ref(), k.as_bytes());
            assert!(noreply);
        } else {
            panic!("invalid parse result");
        }
        assert_eq!(request.consumed, buffer.len());
    }

    // tricky edge-case where the key is 'noreply'
    let buffer = b"delete  noreply\r\n";
    let request = parser.parse(buffer).expect("parse failure");
    if let MemcacheRequest::Delete { key, noreply } = request.message {
        assert_eq!(key.as_ref(), b"noreply");
        assert!(!noreply);
    } else {
        panic!("invalid parse result");
    }
    assert_eq!(request.consumed, buffer.len());
}

#[test]
fn flush_all() {
    let parser = MemcacheRequestParser::default();

    let buffer = b"flush_all\r\n";
    let request = parser.parse(buffer).expect("parse failure");
    if let MemcacheRequest::FlushAll = request.message {
    } else {
        panic!("invalid parse result");
    }
    assert_eq!(request.consumed, buffer.len());
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
    let request = parser
        .parse(b"get  a  b    c    d           e\r\n")
        .expect("parse failure");
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
    let buffer = b"get key \r\n";
    let request = parser.parse(buffer).expect("parse failure");
    if let MemcacheRequest::Get { keys } = request.message {
        println!("keys: {:?}", keys);
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].as_ref(), b"key");
    } else {
        panic!("invalid parse result");
    }
    assert_eq!(request.consumed, buffer.len());

    // set
    let buffer = b"set 0 0 0 1 \r\n1\r\n";
    let request = parser.parse(buffer).expect("parse failure");
    if let MemcacheRequest::Set { entry, noreply } = request.message {
        assert_eq!(entry.key(), b"0");
        assert_eq!(entry.value().unwrap(), Value::U64(1));
        assert!(!noreply);
    } else {
        panic!("invalid parse result");
    }
    assert_eq!(request.consumed, buffer.len());

    // set + noreply
    let buffer = b"set 0 0 0 1 noreply \r\n1\r\n";
    let request = parser.parse(buffer).expect("parse failure");
    if let MemcacheRequest::Set { entry, noreply } = request.message {
        assert_eq!(entry.key(), b"0");
        assert_eq!(entry.value().unwrap(), Value::U64(1));
        assert!(noreply);
    } else {
        panic!("invalid parse result");
    }
    assert_eq!(request.consumed, buffer.len());

    // cas
    let buffer = b"cas 0 0 0 1 0 \r\n1\r\n";
    let request = parser.parse(buffer).expect("parse failure");
    if let MemcacheRequest::Cas { entry, noreply } = request.message {
        assert_eq!(entry.key(), b"0");
        assert_eq!(entry.value().unwrap(), Value::U64(1));
        assert_eq!(entry.cas(), Some(0));
        assert!(!noreply);
    } else {
        panic!("invalid parse result");
    }
    assert_eq!(request.consumed, buffer.len());

    // cas + noreply
    let buffer = b"cas 0 0 0 1 0 noreply \r\n1\r\n";
    let request = parser.parse(buffer).expect("parse failure");
    if let MemcacheRequest::Cas { entry, noreply } = request.message {
        assert_eq!(entry.key(), b"0");
        assert_eq!(entry.value().unwrap(), Value::U64(1));
        assert_eq!(entry.cas(), Some(0));
        assert!(noreply);
    } else {
        panic!("invalid parse result");
    }
    assert_eq!(request.consumed, buffer.len());
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

    // 2 get requests in the buffer, parsing once returns only the first
    let buffer = b"get 0\r\nget 1\r\n";
    let request = parser.parse(buffer).expect("parse failure");
    if let MemcacheRequest::Get { keys } = request.message {
        assert!(keys.len() == 1);
        assert_eq!(keys[0].as_ref(), b"0");
    } else {
        panic!("invalid parse result");
    }
    assert_eq!(request.consumed, 7);

    let request = parser.parse(b"get t\x0d\x0a ").expect("parse failure");
    if let MemcacheRequest::Get { keys } = request.message {
        assert!(keys.len() == 1);
        assert_eq!(keys[0].as_ref(), b"t");
    } else {
        panic!("invalid parse result");
    }
    assert_eq!(request.consumed, 7);
}
