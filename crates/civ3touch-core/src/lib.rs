//! Milestone 0 proof boundary; no persistent game session or presentation code.
use fc3_core::engine::{Engine, GameConfig, GameLog};
use fc3_core::protocol::{Command, Event, GameError};
use fc3_core::types::PlayerId;
use fc3_core::world::WorldConfig;
use std::path::Path;

/// Load the real upstream rules, reject a wrong-player command, advance a turn,
/// then round-trip the command log and replay it. Returns the observed turn.
pub fn smoke_test(rules: &Path) -> Result<u32, String> {
    let rules = rules.canonicalize().map_err(|e| e.to_string())?;
    let config = GameConfig {
        world: WorldConfig {
            width: 16,
            height: 16,
            wrap_x: true,
            wrap_y: false,
            num_players: 2,
            seed: 42,
        },
        mod_paths: vec![rules.to_str().ok_or("rules path is not UTF-8")?.to_owned()],
        units_per_player: vec!["settler".into(), "warrior".into()],
        max_turns: None,
    };
    let mut engine = Engine::new_game(&config).map_err(|e| e.to_string())?;
    let rejected = engine.submit_command(PlayerId(1), Command::EndTurn);
    if rejected.errors != vec![GameError::NotYourTurn] || !engine.event_log.is_empty() {
        return Err("wrong-player command was not rejected without mutation".into());
    }
    for player in [PlayerId(0), PlayerId(1)] {
        let units: Vec<_> = engine
            .player_view(player)
            .known_units
            .into_iter()
            .filter(|unit| unit.owner == player)
            .collect();
        if units.len() != 2 {
            return Err("expected two starting units from Lua rules".into());
        }
        for unit in units {
            if !engine
                .submit_command(player, Command::SkipUnit { unit_id: unit.id })
                .is_ok()
            {
                return Err("could not skip starting unit".into());
            }
        }
        let result = engine.submit_command(player, Command::EndTurn);
        if !result.is_ok()
            || !result
                .events
                .iter()
                .any(|e| matches!(e, Event::TurnStarted { .. }))
        {
            return Err("end-turn command failed to emit TurnStarted".into());
        }
    }
    if engine.current_turn() != 2 || engine.current_player() != PlayerId(0) {
        return Err("unexpected turn state".into());
    }
    let encoded = serde_json::to_string(&engine.to_game_log(&config)).map_err(|e| e.to_string())?;
    let log: GameLog = serde_json::from_str(&encoded).map_err(|e| e.to_string())?;
    let replay = log.replay().map_err(|e| e.to_string())?;
    for player in [PlayerId(0), PlayerId(1)] {
        let expected =
            serde_json::to_value(engine.player_view(player)).map_err(|e| e.to_string())?;
        let actual = serde_json::to_value(replay.player_view(player)).map_err(|e| e.to_string())?;
        if expected != actual {
            return Err("replay player view differs".into());
        }
    }
    Ok(engine.current_turn())
}

#[cfg(target_os = "android")]
mod android {
    use jni::{
        objects::{JClass, JString},
        sys::jint,
        JNIEnv,
    };
    use std::{
        panic::{catch_unwind, AssertUnwindSafe},
        path::Path,
    };

    /// Synchronous, scoped engine lifetime. No Rust engine crosses threads or JNI.
    #[no_mangle]
    pub extern "system" fn Java_org_civ3touch_spike_CoreBridge_smokeTest(
        mut env: JNIEnv,
        _class: JClass,
        rules: JString,
    ) -> jint {
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let path: String = env.get_string(&rules).map_err(|e| e.to_string())?.into();
            super::smoke_test(Path::new(&path))
        }));
        match outcome {
            Ok(Ok(turn)) => turn as jint,
            failure => {
                let message = match failure {
                    Ok(Err(message)) => message,
                    _ => "Rust core panicked".into(),
                };
                let _ = env.throw_new("java/lang/IllegalStateException", message);
                -1
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rules_commands_and_replay_produce_the_same_second_turn() {
        let rules = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/freec3/mods/base");
        assert_eq!(smoke_test(&rules).unwrap(), 2);
    }
    #[test]
    fn missing_rules_are_reported_as_an_error() {
        assert!(smoke_test(Path::new("/nonexistent-civ3touch-rules")).is_err());
    }
}
