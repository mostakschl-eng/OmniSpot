mod ffi;
mod index;
#[allow(dead_code)]
#[path = "Input/mod.rs"]
mod input;
mod matcher;
mod model;
#[allow(dead_code)]
#[path = "Stats/mod.rs"]
mod stats;
#[allow(dead_code)]
#[path = "Switcher.rs"]
mod switcher;
#[allow(dead_code)]
#[path = "WindowDiscovery/mod.rs"]
mod window_discovery;

pub(crate) use matcher::rank_candidates;

// These file-by-file ports remain disconnected from C# until integration is requested.
#[allow(dead_code)]
#[path = "Configuration/mod.rs"]
mod configuration;
#[allow(dead_code)]
#[path = "Runtime.rs"]
mod runtime;
#[allow(dead_code)]
#[path = "UI/mod.rs"]
mod ui;
