/// Policies define the eviction strategy to be used. All eviction strategies
/// exclude segments which are currently accepting new items.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Policy {
    /// No eviction. When all the segments are full, inserts will fail until
    /// segments are freed by TTL expiration.
    None,
    /// Segment random eviction. Selects a random segment and evicts it. Similar
    /// to slab random eviction.
    Random,
    /// FIFO segment eviction. Selects the oldest segment and evicts it. As
    /// segments are append-only, this is similar to both slab LRU and slab LRC
    /// eviction strategies.
    Fifo,
    /// Closest to expiration. Selects the segment that would expire first and
    /// evicts it. This is a unique eviction strategy in segcache and
    /// effectively causes early expiration to free a segment.
    Cte,
    /// Least utilized segment. As segments are append-only, when an item is
    /// replaced or removed the segment containing that item now has dead bytes.
    /// This eviction strategy will free the segment that has the lowest number
    /// of live bytes. This strategy should cause the smallest impact to the
    /// number of live bytes held in the cache.
    Util,
    /// Merge eviction is a unique feature in segcache. It tries to retain items
    /// which have the biggest positive effect on hitrate.
    /// At its core, the idea is to take sequential segments in a chain,
    /// and merge their items into one segment. Unlike the NSDI paper, this
    /// implementation performs two different types of merge operations. The one
    /// matching the NSDI paper is used during eviction and may cause items to
    /// be evicted based on an estimate of their hit frequency. The other
    /// possible merge operation is a simple compaction which will combine
    /// segments which have low utilization (due to item replacement/deletion)
    /// without evicting any live items. Compaction has proven to be beneficial
    /// in workloads that frequently overwrite or delete items in the cache.
    Merge {
        /// The maximum number of segments to merge in a single pass. This can
        /// be used to bound the tail latency impact of a merge operation.
        max: usize,
        /// The target number of segments to merge during eviction. Setting this
        /// higher will result in fewer eviction passes and allow the algorithm
        /// to see more item frequencies. Setting this lower will cause fewer
        /// item evictions per pass.
        merge: usize,
        /// The target number of segments to merge during compaction. Compaction
        /// will only occur if a segment falls below `1/N`th occupancy. Setting
        /// this higher will cause fewer compaction runs but can result in a
        /// larger percentage of dead bytes.
        compact: usize,
    },
}
