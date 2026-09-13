#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]

pub struct IndexerEngine;

impl IndexerEngine {
    pub fn new() -> Self {
        Self
    }
}

impl Default for IndexerEngine {
    fn default() -> Self {
        Self::new()
    }
}
