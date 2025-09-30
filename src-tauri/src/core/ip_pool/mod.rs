pub mod cache;
pub mod config;
pub mod global;
pub mod history;
pub mod preheat;

mod builder;
mod maintenance;
mod manager;
mod sampling;

pub use cache::{IpCacheKey, IpCacheSlot, IpCandidate, IpScoreCache, IpSource, IpStat};
pub use config::{
    EffectiveIpPoolConfig, IpPoolFileConfig, IpPoolRuntimeConfig, IpPoolSourceToggle,
    PreheatDomain, UserStaticIp,
};
pub use history::{IpHistoryRecord, IpHistoryStore};

pub use builder::{load_effective_config, load_effective_config_at};
pub use manager::{IpOutcome, IpPool, IpSelection, IpSelectionStrategy, OutcomeMetrics};
