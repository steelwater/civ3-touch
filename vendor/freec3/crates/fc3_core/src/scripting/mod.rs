pub mod api_action;
pub mod api_building;
pub mod api_city;
pub mod api_civilization;
pub mod api_engine;
pub mod api_tech;
pub mod api_tile;
pub mod api_unit;
pub mod api_unit_type;

use std::path::{Path, PathBuf};

use mlua::{Lua, Result as LuaResult};

pub use mlua::Error as LuaError;

/// The script engine wraps a sandboxed Lua 5.4 state.
///
/// Dangerous globals (`os`, `io`, `loadfile`, `dofile`) are removed during
/// construction.  The `load` function is replaced with a version that rejects
/// binary (bytecode) chunks.
pub struct ScriptEngine {
    pub lua: Lua,
}

impl ScriptEngine {
    /// Create a new sandboxed Lua state.
    pub fn new() -> LuaResult<Self> {
        let lua = Lua::new();

        // Remove dangerous standard library modules / globals.
        lua.load(
            r#"
            os       = nil
            io       = nil
            loadfile = nil
            dofile   = nil

            -- Replace `load` with a wrapper that rejects bytecode.
            local _raw_load = load
            load = function(chunk, chunkname, mode, env)
                if type(chunk) == "string" and chunk:sub(1,1) == "\x1b" then
                    error("bytecode loading is disabled")
                end
                -- Only allow text mode.
                return _raw_load(chunk, chunkname, "t", env)
            end
            "#,
        )
        .exec()?;

        // Load the hook system.
        lua.load(include_str!("hooks.lua"))
            .set_name("hooks.lua")
            .exec()?;

        Ok(Self { lua })
    }

    /// Load and execute a mod directory. Reads `mod.lua` from the given path,
    /// sets up `__mod_dir` and `require_mod_file` for the mod to use.
    pub fn load_mod(&self, mod_dir: &Path) -> LuaResult<()> {
        let canonical_dir = mod_dir.canonicalize().map_err(|e| {
            mlua::Error::external(format!(
                "failed to canonicalize mod dir '{}': {e}",
                mod_dir.display()
            ))
        })?;

        // Set __mod_dir global
        let dir_str = canonical_dir.to_string_lossy().to_string();
        self.lua.globals().set("__mod_dir", dir_str.clone())?;

        // Register require_mod_file(filename)
        let mod_dir_for_closure = canonical_dir.clone();
        self.lua.globals().set(
            "require_mod_file",
            self.lua.create_function(move |lua, filename: String| {
                let requested = mod_dir_for_closure.join(&filename);
                let canonical = requested.canonicalize().map_err(|e| {
                    mlua::Error::external(format!(
                        "require_mod_file: file not found '{filename}': {e}"
                    ))
                })?;

                // Path traversal protection: verify the resolved path is
                // still within the mod directory.
                if !canonical.starts_with(&mod_dir_for_closure) {
                    return Err(mlua::Error::external(format!(
                        "require_mod_file: path traversal detected for '{filename}'"
                    )));
                }

                let source = std::fs::read_to_string(&canonical).map_err(|e| {
                    mlua::Error::external(format!(
                        "require_mod_file: failed to read '{}': {e}",
                        canonical.display()
                    ))
                })?;

                lua.load(&source).set_name(filename).exec()?;
                Ok(())
            })?,
        )?;

        // Load mod.lua
        let mod_lua_path = canonical_dir.join("mod.lua");
        let source = std::fs::read_to_string(&mod_lua_path).map_err(|e| {
            mlua::Error::external(format!("failed to read '{}': {e}", mod_lua_path.display()))
        })?;
        self.lua.load(&source).set_name("mod.lua").exec()?;

        Ok(())
    }

    /// Helper to resolve a mod directory relative to the workspace root.
    /// Useful in tests; in production the caller provides an absolute path.
    pub fn find_mod_dir(mod_name: &str) -> PathBuf {
        // Walk up from the executable or current dir to find mods/
        let mut dir = std::env::current_dir().unwrap_or_default();
        loop {
            let candidate = dir.join("mods").join(mod_name);
            if candidate.exists() {
                return candidate;
            }
            if !dir.pop() {
                break;
            }
        }
        // Fallback: relative path
        PathBuf::from("mods").join(mod_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_script_engine_new_succeeds() {
        let engine = ScriptEngine::new().expect("ScriptEngine::new() should succeed");
        // Verify `os` is nil.
        let result: mlua::Value = engine
            .lua
            .load("return os")
            .eval()
            .expect("eval should work");
        assert!(result.is_nil(), "os should be nil");
    }

    #[test]
    fn test_sandbox_os_execute_blocked() {
        let engine = ScriptEngine::new().unwrap();
        let result = engine.lua.load(r#"os.execute("echo hi")"#).exec();
        assert!(result.is_err(), "os.execute should be blocked");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("nil"),
            "error should mention nil: {err_msg}"
        );
    }

    #[test]
    fn test_sandbox_io_open_blocked() {
        let engine = ScriptEngine::new().unwrap();
        let result = engine.lua.load(r#"io.open("/etc/passwd")"#).exec();
        assert!(result.is_err(), "io.open should be blocked");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("nil"),
            "error should mention nil: {err_msg}"
        );
    }

    #[test]
    fn test_sandbox_loadfile_blocked() {
        let engine = ScriptEngine::new().unwrap();
        let result = engine.lua.load(r#"loadfile("foo.lua")"#).exec();
        assert!(result.is_err(), "loadfile should be blocked");
    }

    #[test]
    fn test_sandbox_dofile_blocked() {
        let engine = ScriptEngine::new().unwrap();
        let result = engine.lua.load(r#"dofile("foo.lua")"#).exec();
        assert!(result.is_err(), "dofile should be blocked");
    }

    #[test]
    fn test_sandbox_load_bytecode_blocked() {
        let engine = ScriptEngine::new().unwrap();
        // \x1b is the Lua bytecode header byte
        let result = engine.lua.load(r#"load("\27Lua\0\0")"#).exec();
        assert!(result.is_err(), "bytecode load should be blocked");
    }

    #[test]
    fn test_sandbox_load_text_allowed() {
        let engine = ScriptEngine::new().unwrap();
        let result: i32 = engine
            .lua
            .load(r#"local f = load("return 42"); return f()"#)
            .eval()
            .expect("text load should work");
        assert_eq!(result, 42);
    }

    // ── Hook system tests ────────────────────────────────────────────

    #[test]
    fn test_hooks_globals_available() {
        let engine = ScriptEngine::new().unwrap();
        let has_register: bool = engine
            .lua
            .load("return type(register_hook) == 'function'")
            .eval()
            .unwrap();
        let has_fire: bool = engine
            .lua
            .load("return type(fire_hook) == 'function'")
            .eval()
            .unwrap();
        assert!(has_register, "register_hook should be a global function");
        assert!(has_fire, "fire_hook should be a global function");
    }

    #[test]
    fn test_hook_handler_receives_and_modifies_context() {
        let engine = ScriptEngine::new().unwrap();
        let result: i32 = engine
            .lua
            .load(
                r#"
                register_hook("test_event", function(ctx)
                    ctx.value = ctx.value + 1
                end)
                local result = fire_hook("test_event", { value = 10 })
                return result.value
                "#,
            )
            .eval()
            .unwrap();
        assert_eq!(result, 11);
    }

    #[test]
    fn test_hook_priority_ordering() {
        let engine = ScriptEngine::new().unwrap();
        let result: String = engine
            .lua
            .load(
                r#"
                register_hook("test_order", function(ctx)
                    ctx.order = ctx.order .. "B"
                end, 200)
                register_hook("test_order", function(ctx)
                    ctx.order = ctx.order .. "A"
                end, 50)
                register_hook("test_order", function(ctx)
                    ctx.order = ctx.order .. "C"
                end, 300)
                local result = fire_hook("test_order", { order = "" })
                return result.order
                "#,
            )
            .eval()
            .unwrap();
        assert_eq!(result, "ABC");
    }

    #[test]
    fn test_hook_cancellation_stops_subsequent_handlers() {
        let engine = ScriptEngine::new().unwrap();
        let result: String = engine
            .lua
            .load(
                r#"
                register_hook("cancel_test", function(ctx)
                    ctx.log = ctx.log .. "first;"
                    ctx.__cancelled = true
                end, 10)
                register_hook("cancel_test", function(ctx)
                    ctx.log = ctx.log .. "second;"
                end, 20)
                local result = fire_hook("cancel_test", { log = "" })
                return result.log
                "#,
            )
            .eval()
            .unwrap();
        assert_eq!(result, "first;");
    }

    #[test]
    fn test_hook_fire_with_no_handlers_returns_context_unchanged() {
        let engine = ScriptEngine::new().unwrap();
        let result: i32 = engine
            .lua
            .load(
                r#"
                local result = fire_hook("no_handlers", { value = 99 })
                return result.value
                "#,
            )
            .eval()
            .unwrap();
        assert_eq!(result, 99);
    }

    #[test]
    fn test_hook_handler_returning_nil_preserves_context() {
        let engine = ScriptEngine::new().unwrap();
        let result: i32 = engine
            .lua
            .load(
                r#"
                register_hook("nil_return", function(ctx)
                    -- modify in place but return nil
                    ctx.value = ctx.value + 5
                end)
                local result = fire_hook("nil_return", { value = 10 })
                return result.value
                "#,
            )
            .eval()
            .unwrap();
        assert_eq!(result, 15);
    }

    #[test]
    fn test_hook_handler_returning_new_table_replaces_context() {
        let engine = ScriptEngine::new().unwrap();
        let result: i32 = engine
            .lua
            .load(
                r#"
                register_hook("replace_ctx", function(ctx)
                    return { value = 999 }
                end)
                local result = fire_hook("replace_ctx", { value = 1 })
                return result.value
                "#,
            )
            .eval()
            .unwrap();
        assert_eq!(result, 999);
    }

    // ── Mod loader tests ────────────────────────────────────────────

    use crate::world::{World, WorldConfig};
    use std::cell::RefCell;
    use std::rc::Rc;

    /// Helper that sets up a ScriptEngine with World + UnitType API so the
    /// base mod can load (it calls UnitType.define).
    fn setup_for_mod_loading() -> ScriptEngine {
        let engine = ScriptEngine::new().unwrap();
        let world = World::new(&WorldConfig {
            width: 10,
            height: 10,
            wrap_x: true,
            wrap_y: false,
            num_players: 2,
            seed: 42,
        });
        let world = Rc::new(RefCell::new(world));
        engine.lua.set_app_data(world);
        crate::scripting::api_unit_type::register(&engine.lua).unwrap();
        crate::scripting::api_action::register(&engine.lua).unwrap();
        crate::scripting::api_tech::register(&engine.lua).unwrap();
        crate::scripting::api_building::register(&engine.lua).unwrap();
        crate::scripting::api_civilization::register(&engine.lua).unwrap();
        engine
    }

    #[test]
    fn test_load_base_mod() {
        let engine = setup_for_mod_loading();
        let mod_dir = ScriptEngine::find_mod_dir("base");
        engine.load_mod(&mod_dir).expect("base mod should load");

        // Verify Mod global was set by mod.lua
        let name: String = engine.lua.load("return Mod.name").eval().unwrap();
        assert_eq!(name, "Civ3 Base Rules");
    }

    #[test]
    fn test_require_mod_file_path_traversal_blocked() {
        let engine = setup_for_mod_loading();
        let mod_dir = ScriptEngine::find_mod_dir("base");
        engine.load_mod(&mod_dir).unwrap();

        let result = engine
            .lua
            .load(r#"require_mod_file("../../etc/passwd")"#)
            .exec();
        assert!(result.is_err(), "path traversal should be blocked");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("path traversal") || err.contains("not found"),
            "error should mention path traversal or not found: {err}"
        );
    }

    #[test]
    fn test_require_mod_file_nonexistent_errors() {
        let engine = setup_for_mod_loading();
        let mod_dir = ScriptEngine::find_mod_dir("base");
        engine.load_mod(&mod_dir).unwrap();

        let result = engine
            .lua
            .load(r#"require_mod_file("nonexistent.lua")"#)
            .exec();
        assert!(result.is_err(), "nonexistent file should error");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("not found") || err.contains("No such file"),
            "error should mention not found: {err}"
        );
    }
}
