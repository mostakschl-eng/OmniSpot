mod ffi;
pub mod desktop;
mod desktop_hook;
mod desktop_icons;
#[allow(dead_code)]
mod input;
#[allow(dead_code)]
mod stats;
#[allow(dead_code)]
mod switcher;
#[allow(dead_code)]
mod window_discovery;

// These file-by-file ports remain disconnected from C# until integration is requested.
#[allow(dead_code)]
mod configuration;
#[allow(dead_code)]
mod cli;
#[allow(dead_code)]
mod startup;
#[allow(dead_code)]
mod runtime;
mod spotlight;
mod storage;
