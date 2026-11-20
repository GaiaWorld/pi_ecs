use super::{PartitionId, SpawnConfig};

impl SpawnConfig {
    pub(super) fn default_inner() -> Self {
        Self {
            partition: PartitionId(0),
        }
    }
}
