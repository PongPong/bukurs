//! Plugin auto-discovery module
//!
//! Plugins are automatically discovered at build time. To add a new plugin:
//! 1. Create a new `.rs` file in this directory (e.g., `my_plugin.rs`)
//! 2. Implement the `Plugin` trait
//! 3. Export a `pub fn create_plugin() -> Box<dyn Plugin>` function
//!
//! The plugin will be automatically registered when the CLI starts.
//!
//! # Example Plugin
//!
//! ```rust,ignore
//! use bukurs::plugin::{Plugin, PluginInfo, HookResult, PluginContext};
//! use bukurs::models::bookmark::Bookmark;
//!
//! pub struct MyPlugin;
//!
//! impl Plugin for MyPlugin {
//!     fn info(&self) -> PluginInfo {
//!         PluginInfo {
//!             name: "my-plugin".to_string(),
//!             version: "1.0.0".to_string(),
//!             description: "My custom plugin".to_string(),
//!             author: "me".to_string(),
//!         }
//!     }
//!
//!     fn on_pre_add(&self, _ctx: &PluginContext, bookmark: &mut Bookmark) -> HookResult {
//!         // Modify bookmark before adding
//!         HookResult::Continue
//!     }
//! }
//!
//! pub fn create_plugin() -> Box<dyn Plugin> {
//!     Box::new(MyPlugin)
//! }
//! ```

// Include auto-generated module declarations
include!(concat!(env!("OUT_DIR"), "/plugin_mods.rs"));

// Include auto-generated registration function
include!(concat!(env!("OUT_DIR"), "/plugin_register.rs"));
