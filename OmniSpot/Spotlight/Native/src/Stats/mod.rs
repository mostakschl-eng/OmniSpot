pub(crate) mod migrations;
pub(crate) mod storage;
#[path = "EfficiencyCalculator.rs"]
pub(crate) mod efficiency_calculator;
#[path = "AppRegistryCache.rs"]
pub(crate) mod app_registry_cache;
#[path = "IAppRegistryCache.rs"]
pub(crate) mod i_app_registry_cache;
#[path = "ISessionStats.rs"]
pub(crate) mod i_session_stats;
#[path = "SessionStats.rs"]
pub(crate) mod session_stats;
#[path = "StatsCalculator.rs"]
pub(crate) mod stats_calculator;
#[path = "StatsConsumer.rs"]
pub(crate) mod stats_consumer;
#[path = "StatsEvent.rs"]
pub(crate) mod stats_event;
#[path = "StatsRepository.rs"]
pub(crate) mod stats_repository;
#[path = "StatsService.rs"]
pub(crate) mod stats_service;

#[allow(unused_imports)]
pub(crate) use efficiency_calculator::*;
#[allow(unused_imports)]
pub(crate) use i_app_registry_cache::IAppRegistryCache;
#[allow(unused_imports)]
pub(crate) use i_session_stats::ISessionStats;
#[allow(unused_imports)]
pub(crate) use session_stats::SessionStats;
#[allow(unused_imports)]
pub(crate) use stats_calculator::*;
#[allow(unused_imports)]
pub(crate) use stats_consumer::StatsConsumer;
