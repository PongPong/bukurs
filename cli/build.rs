//! Build script for bukurs-cli
//!
//! Automatically discovers plugins in cli/src/plugins/ and generates registration code.
//! Each plugin file must export a `create_plugin()` function that returns `Box<dyn Plugin>`.

use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let plugins_dir = Path::new("src/plugins");

    // Collect plugin module names (all .rs files except mod.rs)
    let mut plugin_modules = Vec::new();

    if plugins_dir.exists() {
        for entry in fs::read_dir(plugins_dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();

            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "rs" {
                        if let Some(stem) = path.file_stem() {
                            let name = stem.to_string_lossy().to_string();
                            if name != "mod" {
                                plugin_modules.push(name);
                            }
                        }
                    }
                }
            }
        }
    }

    plugin_modules.sort();

    // Generate the module declarations with #[path] attribute
    let mut mod_code = String::new();
    mod_code.push_str("// Auto-generated module declarations - do not edit below this line\n\n");

    for module in &plugin_modules {
        // Use #[path] with absolute path to the actual source file
        let abs_path = format!("{}/src/plugins/{}.rs", manifest_dir.replace('\\', "/"), module);
        mod_code.push_str(&format!("#[path = \"{}\"]\n", abs_path));
        mod_code.push_str(&format!("mod {};\n\n", module));
    }

    let dest_mods = Path::new(&out_dir).join("plugin_mods.rs");
    fs::write(&dest_mods, mod_code).unwrap();

    // Generate the registration function
    let mut reg_code = String::new();
    reg_code.push_str("// Auto-generated registration code - do not edit\n\n");
    reg_code.push_str("use bukurs::plugin::PluginManager;\n");
    reg_code.push_str("use bukurs::error::Result;\n\n");

    reg_code.push_str("/// Auto-generated function to register all discovered plugins\n");
    reg_code.push_str("pub fn register_all_plugins(manager: &mut PluginManager) -> Result<()> {\n");

    for module in &plugin_modules {
        reg_code.push_str(&format!(
            "    manager.register({}::create_plugin())?;\n",
            module
        ));
    }

    reg_code.push_str("    Ok(())\n");
    reg_code.push_str("}\n\n");

    reg_code.push_str("/// Get list of all discovered plugin modules\n");
    reg_code.push_str("pub fn list_plugin_modules() -> &'static [&'static str] {\n");
    reg_code.push_str("    &[\n");
    for module in &plugin_modules {
        reg_code.push_str(&format!("        \"{}\",\n", module));
    }
    reg_code.push_str("    ]\n");
    reg_code.push_str("}\n");

    let dest_reg = Path::new(&out_dir).join("plugin_register.rs");
    fs::write(&dest_reg, reg_code).unwrap();

    // Tell Cargo to rerun if plugins directory changes
    println!("cargo:rerun-if-changed=src/plugins");
}
