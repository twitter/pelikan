// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

// All metrics for the Seg crate

use rustcommon_metrics::*;

// segment related
counter!(SEGMENT_REQUEST, "number of segment allocation attempts");
counter!(
    SEGMENT_REQUEST_FAILURE,
    "number of segment allocation attempts which failed"
);
counter!(
    SEGMENT_REQUEST_SUCCESS,
    "number of segment allocation attempts which were successful"
);
counter!(SEGMENT_EVICT, "number of segments evicted");
counter!(
    SEGMENT_EVICT_EX,
    "number of exceptions while evicting segments"
);
counter!(
    SEGMENT_RETURN,
    "total number of segments returned to the free pool"
);
counter!(SEGMENT_MERGE, "total number of segments merged");
gauge!(EVICT_TIME, "time, in nanoseconds, spent evicting segments");
gauge!(SEGMENT_FREE, "current number of free segments");
gauge!(SEGMENT_CURRENT, "current number of segments");

// hash table related
counter!(HASH_TAG_COLLISION, "number of partial hash collisions");
counter!(HASH_INSERT, "number of inserts into the hash table");
counter!(
    HASH_INSERT_EX,
    "number of hash table inserts which failed, likely due to capacity"
);
counter!(
    HASH_REMOVE,
    "number of hash table entries which have been removed"
);
counter!(
    HASH_LOOKUP,
    "total number of lookups against the hash table"
);
counter!(
    ITEM_RELINK,
    "number of times items have been relinked to different locations"
);

// item related
counter!(ITEM_ALLOCATE, "number of times items have been allocated");
counter!(ITEM_REPLACE, "number of times items have been replaced");
counter!(ITEM_DELETE, "number of items removed from the hash table");
counter!(ITEM_EXPIRE, "number of items removed due to expiration");
counter!(ITEM_EVICT, "number of items removed due to eviction");
counter!(ITEM_COMPACTED, "number of items which have been compacted");
gauge!(ITEM_CURRENT, "current number of live items");
gauge!(
    ITEM_CURRENT_BYTES,
    "current number of live bytes for storing items"
);
gauge!(ITEM_DEAD, "current number of dead items");
gauge!(
    ITEM_DEAD_BYTES,
    "current number of dead bytes for storing items"
);
