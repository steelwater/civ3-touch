use fc3_core::ai::{Agent, SimpleAgent};
use fc3_core::engine::{Engine, GameConfig};
use fc3_core::protocol::Command;
use fc3_core::types::PlayerId;
use fc3_core::world::WorldConfig;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

fn run_single_game(seed: u64, max_turns: u32) -> (Option<PlayerId>, u32, u32) {
    let config = GameConfig {
        world: WorldConfig {
            width: 30,
            height: 20,
            wrap_x: true,
            wrap_y: false,
            num_players: 2,
            seed,
        },
        mod_paths: vec!["base".to_string()],
        units_per_player: vec!["warrior".to_string(), "warrior".to_string()],
        max_turns: Some(max_turns),
    };

    let mut engine = Engine::new_game(&config).unwrap();
    let mut agents: Vec<Box<dyn Agent>> = (0..2u8)
        .map(|i| {
            let agent_seed = seed.wrapping_add(i as u64);
            Box::new(SimpleAgent::new(ChaCha8Rng::seed_from_u64(agent_seed))) as Box<dyn Agent>
        })
        .collect();

    let mut combats = 0u32;

    loop {
        if let Some(winner) = engine.is_game_over() {
            return (Some(winner), engine.current_turn(), combats);
        }

        let current = engine.current_player();
        if !engine.is_player_alive(current) {
            engine.submit_command(current, Command::EndTurn);
            continue;
        }

        let agent_idx = current.0 as usize;
        let mut commands_this_turn = 0u32;

        loop {
            let view = engine.player_view(current);
            let available = engine.available_commands(current);
            let cmd = agents[agent_idx].decide(&view, &available);

            let is_end_turn = matches!(cmd, Command::EndTurn);
            let is_attack = matches!(cmd, Command::AttackUnit { .. });

            engine.submit_command(current, cmd);

            if is_attack {
                combats += 1;
            }

            if is_end_turn {
                break;
            }

            commands_this_turn += 1;
            if commands_this_turn > 500 {
                engine.submit_command(current, Command::EndTurn);
                break;
            }
        }

        if engine.is_game_over().is_some() {
            let winner = engine.is_game_over();
            return (winner, engine.current_turn(), combats);
        }
    }
}

#[test]
#[ignore]
fn stress_100_games() {
    let mut p0_wins = 0u32;
    let mut p1_wins = 0u32;
    let mut draws = 0u32;
    let mut total_turns = 0u64;
    let mut total_combats = 0u64;

    for seed in 0..100u64 {
        let (winner, turns, combats) = run_single_game(seed, 200);
        match winner {
            Some(p) if p.0 == 0 => p0_wins += 1,
            Some(_) => p1_wins += 1,
            None => draws += 1,
        }
        total_turns += turns as u64;
        total_combats += combats as u64;
    }

    let avg_turns = total_turns / 100;
    let avg_combats = total_combats / 100;
    println!(
        "100/100 games completed. Average turns: {}. Average combats: {}. P0 wins: {}, P1 wins: {}, draws: {}.",
        avg_turns, avg_combats, p0_wins, p1_wins, draws,
    );
}

#[test]
#[ignore]
fn stress_1000_games() {
    let mut p0_wins = 0u32;
    let mut p1_wins = 0u32;
    let mut draws = 0u32;
    let mut total_turns = 0u64;
    let mut total_combats = 0u64;

    for seed in 0..1000u64 {
        let (winner, turns, combats) = run_single_game(seed, 200);
        match winner {
            Some(p) if p.0 == 0 => p0_wins += 1,
            Some(_) => p1_wins += 1,
            None => draws += 1,
        }
        total_turns += turns as u64;
        total_combats += combats as u64;
    }

    let avg_turns = total_turns / 1000;
    let avg_combats = total_combats / 1000;
    println!(
        "1000/1000 games completed. Average turns: {}. Average combats: {}. P0 wins: {}, P1 wins: {}, draws: {}.",
        avg_turns, avg_combats, p0_wins, p1_wins, draws,
    );
}

#[test]
#[ignore]
fn stress_replay_determinism() {
    // For 10 games, verify that replaying produces the same final state
    for seed in 0..10u64 {
        let config = GameConfig {
            world: WorldConfig {
                width: 30,
                height: 20,
                wrap_x: true,
                wrap_y: false,
                num_players: 2,
                seed,
            },
            mod_paths: vec!["base".to_string()],
            units_per_player: vec!["warrior".to_string(), "warrior".to_string()],
            max_turns: Some(50),
        };

        let mut engine = Engine::new_game(&config).unwrap();
        let mut agents: Vec<Box<dyn Agent>> = (0..2u8)
            .map(|i| {
                let agent_seed = seed.wrapping_add(i as u64);
                Box::new(SimpleAgent::new(ChaCha8Rng::seed_from_u64(agent_seed))) as Box<dyn Agent>
            })
            .collect();

        // Play the game
        loop {
            if engine.is_game_over().is_some() {
                break;
            }

            let current = engine.current_player();
            if !engine.is_player_alive(current) {
                engine.submit_command(current, Command::EndTurn);
                continue;
            }

            let agent_idx = current.0 as usize;
            let mut commands_this_turn = 0u32;

            loop {
                let view = engine.player_view(current);
                let available = engine.available_commands(current);
                let cmd = agents[agent_idx].decide(&view, &available);

                let is_end_turn = matches!(cmd, Command::EndTurn);
                engine.submit_command(current, cmd);

                if is_end_turn {
                    break;
                }
                commands_this_turn += 1;
                if commands_this_turn > 500 {
                    engine.submit_command(current, Command::EndTurn);
                    break;
                }
            }

            if engine.is_game_over().is_some() {
                break;
            }
        }

        // Capture original state
        let original_turn = engine.current_turn();
        let original_winner = engine.is_game_over();
        let original_unit_count = engine.world().borrow().units.count();

        // Replay
        let log = engine.to_game_log(&config);
        let replayed = log.replay().unwrap();

        assert_eq!(
            replayed.current_turn(),
            original_turn,
            "Replay turn mismatch for seed {seed}"
        );
        assert_eq!(
            replayed.is_game_over(),
            original_winner,
            "Replay winner mismatch for seed {seed}"
        );
        assert_eq!(
            replayed.world().borrow().units.count(),
            original_unit_count,
            "Replay unit count mismatch for seed {seed}"
        );
    }
    println!("10/10 replay determinism checks passed.");
}
