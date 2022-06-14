// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use seg::*;

use std::time::Duration;

#[test]
fn integration_basic() {
    let ttl = Duration::ZERO;
    let heap_size = 2 * 256;
    let segment_size = 256;
    let mut cache = Seg::builder()
        .segment_size(segment_size)
        .heap_size(heap_size)
        .hash_power(16)
        .build()
        .expect("failed to create cache");

    println!("filling seg 0");
    let _ = cache.insert(
        b"a",
        b"What's in a name? A rose by any other name would smell as sweet.",
        None,
        ttl,
    );
    assert_eq!(cache.get(b"a").map(|v| v.value().len()), Some(64));

    let _ = cache.insert(b"b", b"All that glitters is not gold.", None, ttl);
    assert_eq!(cache.get(b"a").map(|v| v.value().len()), Some(64));
    assert_eq!(cache.get(b"b").map(|v| v.value().len()), Some(30));

    let _ = cache.insert(
        b"c",
        b"Cry 'havoc' and let slip the dogs of war.",
        None,
        ttl,
    );
    assert_eq!(cache.get(b"a").map(|v| v.value().len()), Some(64));
    assert_eq!(cache.get(b"b").map(|v| v.value().len()), Some(30));
    assert_eq!(cache.get(b"c").map(|v| v.value().len()), Some(41));
    // first segment is full

    println!("filling seg 1");
    let _ = cache.insert(b"d", b"There are more things in heaven and earth, Horatio, than are dreamt of in your philosophy.", None, ttl);
    assert_eq!(cache.get(b"a").map(|v| v.value().len()), Some(64));
    assert_eq!(cache.get(b"b").map(|v| v.value().len()), Some(30));
    assert_eq!(cache.get(b"c").map(|v| v.value().len()), Some(41));
    assert_eq!(cache.get(b"d").map(|v| v.value().len()), Some(90));

    let _ = cache.insert(
        b"e",
        b"Uneasy lies the head that wears the crown.",
        None,
        ttl,
    );
    assert_eq!(cache.get(b"a").map(|v| v.value().len()), Some(64));
    assert_eq!(cache.get(b"b").map(|v| v.value().len()), Some(30));
    assert_eq!(cache.get(b"c").map(|v| v.value().len()), Some(41));
    assert_eq!(cache.get(b"d").map(|v| v.value().len()), Some(90));
    assert_eq!(cache.get(b"e").map(|v| v.value().len()), Some(42));

    let _ = cache.insert(b"f", b"Brevity is the soul of wit.", None, ttl);
    assert_eq!(cache.get(b"a").map(|v| v.value().len()), Some(64));
    assert_eq!(cache.get(b"b").map(|v| v.value().len()), Some(30));
    assert_eq!(cache.get(b"c").map(|v| v.value().len()), Some(41));
    assert_eq!(cache.get(b"d").map(|v| v.value().len()), Some(90));
    assert_eq!(cache.get(b"e").map(|v| v.value().len()), Some(42));
    assert_eq!(cache.get(b"f").map(|v| v.value().len()), Some(27));

    #[cfg(feature = "magic")]
    {
        let _ = cache.insert(b"g", b"Et tu, Brute?", None, ttl);
        assert_eq!(cache.get(b"g").map(|v| v.value().len()), Some(13));
    }

    #[cfg(not(feature = "magic"))]
    {
        let _ = cache.insert(
            b"g",
            b"But, for my own part, it was Greek to me.",
            None,
            ttl,
        );
        assert_eq!(cache.get(b"g").map(|v| v.value().len()), Some(41));
    }

    assert_eq!(cache.get(b"a").map(|v| v.value().len()), Some(64));
    assert_eq!(cache.get(b"b").map(|v| v.value().len()), Some(30));
    assert_eq!(cache.get(b"c").map(|v| v.value().len()), Some(41));
    assert_eq!(cache.get(b"d").map(|v| v.value().len()), Some(90));
    assert_eq!(cache.get(b"e").map(|v| v.value().len()), Some(42));
    assert_eq!(cache.get(b"f").map(|v| v.value().len()), Some(27));

    // second segment is now full

    // expiration and refill of first segment
    println!("trigger expiration and refill of seg 0");
    let _ = cache.insert(
        b"h",
        b"There is nothing either good or bad, but thinking makes it so.",
        None,
        ttl,
    );
    assert_eq!(cache.get(b"a").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"b").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"c").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"d").map(|v| v.value().len()), Some(90));
    assert_eq!(cache.get(b"e").map(|v| v.value().len()), Some(42));
    assert_eq!(cache.get(b"f").map(|v| v.value().len()), Some(27));
    assert!(cache.get(b"g").is_some());
    assert_eq!(cache.get(b"h").map(|v| v.value().len()), Some(62));

    let _ = cache.insert(
        b"i",
        b"We know what we are, but know not what we may be.",
        None,
        ttl,
    );
    assert_eq!(cache.get(b"a").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"b").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"c").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"d").map(|v| v.value().len()), Some(90));
    assert_eq!(cache.get(b"e").map(|v| v.value().len()), Some(42));
    assert_eq!(cache.get(b"f").map(|v| v.value().len()), Some(27));
    assert!(cache.get(b"g").is_some());
    assert_eq!(cache.get(b"h").map(|v| v.value().len()), Some(62));
    assert_eq!(cache.get(b"i").map(|v| v.value().len()), Some(49));

    let _ = cache.insert(
        b"j",
        b"The evil that men do lives after them; The good is oft interred with their bones.",
        None,
        ttl,
    );
    assert_eq!(cache.get(b"a").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"b").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"c").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"d").map(|v| v.value().len()), Some(90));
    assert_eq!(cache.get(b"e").map(|v| v.value().len()), Some(42));
    assert_eq!(cache.get(b"f").map(|v| v.value().len()), Some(27));
    assert!(cache.get(b"g").is_some());
    assert_eq!(cache.get(b"h").map(|v| v.value().len()), Some(62));
    assert_eq!(cache.get(b"i").map(|v| v.value().len()), Some(49));
    assert_eq!(cache.get(b"j").map(|v| v.value().len()), Some(81));
    // first segment is refilled

    // expiration and refill of second segment
    println!("trigger expiration and refill of seg 1");
    let _ = cache.insert(
        b"k",
        b"Give every man thy ear, but few thy voice.",
        None,
        ttl,
    );
    assert_eq!(cache.get(b"a").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"b").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"c").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"d").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"e").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"f").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"g").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"h").map(|v| v.value().len()), Some(62));
    assert_eq!(cache.get(b"i").map(|v| v.value().len()), Some(49));
    assert_eq!(cache.get(b"j").map(|v| v.value().len()), Some(81));
    assert_eq!(cache.get(b"k").map(|v| v.value().len()), Some(42));

    let _ = cache.insert(
        b"l",
        b"There is nothing either good or bad, but thinking makes it so.",
        None,
        ttl,
    );
    assert_eq!(cache.get(b"a").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"b").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"c").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"d").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"e").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"f").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"g").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"h").map(|v| v.value().len()), Some(62));
    assert_eq!(cache.get(b"i").map(|v| v.value().len()), Some(49));
    assert_eq!(cache.get(b"j").map(|v| v.value().len()), Some(81));
    assert_eq!(cache.get(b"k").map(|v| v.value().len()), Some(42));
    assert_eq!(cache.get(b"l").map(|v| v.value().len()), Some(62));

    // check that overwrite of most recent key works properly, checking that
    // write offset and occupied size reflect the change
    println!("overwrite recent key");
    let _ = cache.insert(b"l", b"Et tu, Brute?", None, ttl);
    assert_eq!(cache.get(b"a").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"b").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"c").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"d").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"e").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"f").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"g").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"h").map(|v| v.value().len()), Some(62));
    assert_eq!(cache.get(b"i").map(|v| v.value().len()), Some(49));
    assert_eq!(cache.get(b"j").map(|v| v.value().len()), Some(81));
    assert_eq!(cache.get(b"k").map(|v| v.value().len()), Some(42));
    assert_eq!(cache.get(b"l").map(|v| v.value().len()), Some(13));

    // check that overwrite of an older key in the active segment works,
    // this also evicts the first segment
    println!("overwrite older key in active segment");
    let _ = cache.insert(b"k", b"All the world's a stage, and all the men and women merely players. They have their exits and their entrances; And one man in his time plays many parts.", None, ttl);
    assert_eq!(cache.get(b"a").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"b").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"c").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"d").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"e").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"f").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"g").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"h").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"i").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"j").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"k").map(|v| v.value().len()), Some(151));
    assert_eq!(cache.get(b"l").map(|v| v.value().len()), Some(13));

    // now, let's replace key 12, we should see that the second segment no
    // longer has any items
    println!("overwrite key, triggering eviction of seg 1");
    let _ = cache.insert(b"l", b"Action is eloquence.", None, ttl);

    assert_eq!(cache.get(b"a").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"b").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"c").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"d").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"e").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"f").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"g").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"h").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"i").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"j").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"k").map(|v| v.value().len()), Some(151));
    assert_eq!(cache.get(b"l").map(|v| v.value().len()), Some(20));

    // the next small insert should go into seg 0
    let _ = cache.insert(
        b"m",
        b"Some rise by sin, and some by virtue fall.",
        None,
        ttl,
    );

    assert_eq!(cache.get(b"a").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"b").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"c").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"d").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"e").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"f").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"g").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"h").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"i").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"j").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"k").map(|v| v.value().len()), Some(151));
    assert_eq!(cache.get(b"l").map(|v| v.value().len()), Some(20));
    assert_eq!(cache.get(b"m").map(|v| v.value().len()), Some(42));

    // This write won't fit in seg0, so seg1 is evicted. The write offset
    // and occupied size will reflect this

    let _ = cache.insert(
        b"n",
        b"Have more than thou showest, Speak less than thou knowest",
        None,
        ttl,
    );
    assert_eq!(cache.get(b"a").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"b").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"c").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"d").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"e").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"f").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"g").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"h").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"i").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"j").map(|v| v.value().len()), None);
    assert_eq!(cache.get(b"k").map(|v| v.value().len()), Some(151));
    assert_eq!(cache.get(b"l").map(|v| v.value().len()), Some(20));
    assert_eq!(cache.get(b"m").map(|v| v.value().len()), Some(42));
    assert_eq!(cache.get(b"n").map(|v| v.value().len()), Some(57));
}
