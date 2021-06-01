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
    // trailing whitespace
    let request = MemcacheRequest::parse(b"get key \r\n").expect("parse failure");
    if let MemcacheRequest::Get { keys } = request.message {
        println!("keys: {:?}", keys);
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].as_ref(), b"key");
    } else {
        panic!("invalid parse result");
    }
}

#[test]
fn invalid() {
    // invalid
    for request in &["get \r\n", "get this     is    malformed\r\n"] {
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
