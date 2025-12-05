//! Plugin system for bukurs
//!
//! This module provides a flexible plugin architecture that allows extending bukurs
//! functionality through hooks, custom output formats, and custom commands.
//!
//! # Plugin Types
//!
//! - **Hook Plugins**: Execute code before/after bookmark operations (add, update, delete, search)
//! - **Output Format Plugins**: Provide custom output formats for bookmark display
//! - **Command Plugins**: Add new commands to bukurs
//!
//! # Example Plugin
//!
//! ```rust,ignore
//! use bukurs::plugin::{Plugin, PluginInfo, HookResult, PluginContext};
//! use bukurs::models::Bookmark;
//!
//! pub struct MyPlugin;
//!
//! impl Plugin for MyPlugin {
//!     fn info(&self) -> PluginInfo {
//!         PluginInfo {
//!             name: "my-plugin".to_string(),
//!             version: "1.0.0".to_string(),
//!             description: "A sample plugin".to_string(),
//!             author: "Author Name".to_string(),
//!         }
//!     }
//!
//!     fn on_bookmark_add(&self, ctx: &PluginContext, bookmark: &mut Bookmark) -> HookResult {
//!         // Modify bookmark before it's added
//!         HookResult::Continue
//!     }
//! }
//! ```

mod hooks;
mod manager;
mod traits;

pub use hooks::*;
pub use manager::*;
pub use traits::*;
