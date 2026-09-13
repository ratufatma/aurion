#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![deny(clippy::arithmetic_side_effects)]

/// Canonical Zenoh key expression for new block gossip announcements.
pub const TOPIC_BLOCKS_NEW: &str = "aurion/net/v1/blocks/new";

/// Canonical Zenoh key expression for new transaction gossip.
pub const TOPIC_TX_NEW: &str = "aurion/net/v1/tx/new";

/// Canonical Zenoh key expression for peer announcement gossip.
pub const TOPIC_PEERS_ANNOUNCE: &str = "aurion/net/v1/peers/announce";

/// Canonical Zenoh key expression for peer discovery queries.
pub const TOPIC_PEERS_LIST: &str = "aurion/net/v1/peers/list";

/// Canonical Zenoh key expression for chain header synchronization queries.
pub const TOPIC_SYNC_HEADERS: &str = "aurion/net/v1/sync/headers";

/// Canonical Zenoh key expression for block range synchronization queries.
pub const TOPIC_SYNC_BLOCKS: &str = "aurion/net/v1/sync/blocks";

/// IPC key expression: miner template request/reply channel.
pub const TOPIC_IPC_MINER_TEMPLATE: &str = "aurion/ipc/v1/miner/template";

/// IPC key expression: miner solution submission channel.
pub const TOPIC_IPC_MINER_SUBMIT: &str = "aurion/ipc/v1/miner/submit";