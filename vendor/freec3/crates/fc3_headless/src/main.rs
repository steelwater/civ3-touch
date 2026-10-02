use std::io::{self, BufRead};
use std::panic;

use clap::Parser;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

mod display;

use fc3_core::ai::{Agent, SimpleAgent};
use fc3_core::engine::{Engine, GameConfig, GameLog};

use display::{format_event, render_ascii};
use fc3_core::protocol::{Command, Event};
use fc3_core::types::PlayerId;
use fc3_core::world::WorldConfig;

#[derive(Parser, Debug)]
#[command(name = "fc3_headless", about = "FreeC3 headless AI runner")]
struct Cli {
    /// Random seed for game generation
    #[arg(long, default_value_t = 42)]
    seed: u64,

    /// Map width
    #[arg(long, default_value_t = 40)]
    width: u32,

    /// Map height
    #[arg(long, default_value_t = 25)]
    height: u32,

    /// Number of players
    #[arg(long, default_value_t = 2)]
    players: u8,

    /// Maximum number of turns before game ends
    #[arg(long, default_value_t = 50)]
    max_turns: u32,

    /// Print ASCII map and all events every turn
    #[arg(short, long)]
    verbose: bool,

    /// Print only the final result line
    #[arg(short, long)]
    quiet: bool,

    /// Step through turn by turn, pressing Enter to advance
    #[arg(long)]
    step: bool,

    /// Run a batch of games sequentially
    #[arg(long)]
    batch: Option<u32>,

    /// Save game log to a JSON file
    #[arg(long)]
    save_log: Option<String>,

    /// Replay a game log from a JSON file
    #[arg(long)]
    replay: Option<String>,
}

fn make_config(cli: &Cli) -> GameConfig {
    GameConfig {
        world: WorldConfig {
            width: cli.width,
            height: cli.height,
            wrap_x: true,
            wrap_y: true,
            num_players: cli.players,
            seed: cli.seed,
        },
        mod_paths: vec!["base".to_string()],
        units_per_player: vec!["warrior".to_string(), "settler".to_string()],
        max_turns: Some(cli.max_turns),
    }
}

struct GameResult {
    winner: Option<PlayerId>,
    turns: u32,
    combats: u32,
    unit_counts: Vec<usize>,
    seed: u64,
}

fn wait_for_enter() {
    eprint!("[Press Enter to continue]");
    let stdin = io::stdin();
    let _ = stdin.lock().lines().next();
}

fn run_game(
    config: &GameConfig,
    agents: &mut [Box<dyn Agent>],
    verbose: bool,
    quiet: bool,
    step: bool,
) -> GameResult {
    let mut engine = Engine::new_game(config).unwrap();
    let num_players = config.world.num_players as usize;
    let mut combats = 0u32;
    let show = verbose || step;

    if show {
        let view = engine.player_view(engine.current_player());
        println!(
            "{}",
            render_ascii(&view, config.world.width, config.world.height)
        );
        if step {
            wait_for_enter();
        }
    }

    loop {
        if let Some(winner) = engine.is_game_over() {
            let unit_counts = (0..num_players)
                .map(|p| count_units(&engine, PlayerId(p as u8)))
                .collect();
            return GameResult {
                winner: Some(winner),
                turns: engine.current_turn(),
                combats,
                unit_counts,
                seed: config.world.seed,
            };
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

            let result = engine.submit_command(current, cmd);

            if is_attack {
                combats += 1;
            }

            if show {
                for event in &result.events {
                    println!("{}", format_event(event));
                }
            }

            // Check if a turn started event came (for display)
            if show {
                for event in &result.events {
                    if matches!(event, Event::TurnStarted { .. }) {
                        let view = engine.player_view(engine.current_player());
                        println!(
                            "{}",
                            render_ascii(&view, config.world.width, config.world.height)
                        );
                        if step {
                            wait_for_enter();
                        }
                    }
                }
            }

            if is_end_turn {
                break;
            }

            commands_this_turn += 1;
            if commands_this_turn > 500 {
                // Safety cap to prevent infinite loops
                let result = engine.submit_command(current, Command::EndTurn);
                if show {
                    for event in &result.events {
                        println!("{}", format_event(event));
                    }
                    for event in &result.events {
                        if matches!(event, Event::TurnStarted { .. }) {
                            let view = engine.player_view(engine.current_player());
                            println!(
                                "{}",
                                render_ascii(&view, config.world.width, config.world.height)
                            );
                            if step {
                                wait_for_enter();
                            }
                        }
                    }
                }
                break;
            }
        }

        if !quiet && !show {
            // Default mode: brief per-turn summary
            if engine.current_player().0 == 0 || engine.is_game_over().is_some() {
                // Print summary at the start of a new round
            }
        }

        // Check for game over after turn processing
        if let Some(winner) = engine.is_game_over() {
            let unit_counts = (0..num_players)
                .map(|p| count_units(&engine, PlayerId(p as u8)))
                .collect();

            if show || !quiet {
                println!("*** GAME OVER — Player {} wins! ***", winner.0);
            }

            return GameResult {
                winner: Some(winner),
                turns: engine.current_turn(),
                combats,
                unit_counts,
                seed: config.world.seed,
            };
        }
    }
}

fn run_game_with_log(
    config: &GameConfig,
    agents: &mut [Box<dyn Agent>],
    verbose: bool,
    quiet: bool,
    step: bool,
) -> (GameResult, GameLog) {
    let mut engine = Engine::new_game(config).unwrap();
    let num_players = config.world.num_players as usize;
    let mut combats = 0u32;
    let show = verbose || step;

    if show {
        let view = engine.player_view(engine.current_player());
        println!(
            "{}",
            render_ascii(&view, config.world.width, config.world.height)
        );
        if step {
            wait_for_enter();
        }
    }

    loop {
        if let Some(winner) = engine.is_game_over() {
            let unit_counts = (0..num_players)
                .map(|p| count_units(&engine, PlayerId(p as u8)))
                .collect();
            let log = engine.to_game_log(config);
            return (
                GameResult {
                    winner: Some(winner),
                    turns: engine.current_turn(),
                    combats,
                    unit_counts,
                    seed: config.world.seed,
                },
                log,
            );
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

            let result = engine.submit_command(current, cmd);

            if is_attack {
                combats += 1;
            }

            if show {
                for event in &result.events {
                    println!("{}", format_event(event));
                }
                for event in &result.events {
                    if matches!(event, Event::TurnStarted { .. }) {
                        let view = engine.player_view(engine.current_player());
                        println!(
                            "{}",
                            render_ascii(&view, config.world.width, config.world.height)
                        );
                        if step {
                            wait_for_enter();
                        }
                    }
                }
            }

            if is_end_turn {
                break;
            }

            commands_this_turn += 1;
            if commands_this_turn > 500 {
                let result = engine.submit_command(current, Command::EndTurn);
                if show {
                    for event in &result.events {
                        println!("{}", format_event(event));
                    }
                    for event in &result.events {
                        if matches!(event, Event::TurnStarted { .. }) {
                            let view = engine.player_view(engine.current_player());
                            println!(
                                "{}",
                                render_ascii(&view, config.world.width, config.world.height)
                            );
                            if step {
                                wait_for_enter();
                            }
                        }
                    }
                }
                break;
            }
        }

        if let Some(winner) = engine.is_game_over() {
            let unit_counts = (0..num_players)
                .map(|p| count_units(&engine, PlayerId(p as u8)))
                .collect();

            if show || !quiet {
                println!("*** GAME OVER — Player {} wins! ***", winner.0);
            }

            let log = engine.to_game_log(config);
            return (
                GameResult {
                    winner: Some(winner),
                    turns: engine.current_turn(),
                    combats,
                    unit_counts,
                    seed: config.world.seed,
                },
                log,
            );
        }
    }
}

fn count_units(engine: &Engine, player: PlayerId) -> usize {
    let world = engine.world().borrow();
    world
        .units
        .iter_alive()
        .filter(|(_, idx)| world.units.owner[*idx] == player)
        .count()
}

fn print_summary(result: &GameResult) {
    let winner_str = match result.winner {
        Some(p) => format!("Player {} wins", p.0),
        None => "Draw".to_string(),
    };
    let units_str: Vec<String> = result
        .unit_counts
        .iter()
        .enumerate()
        .map(|(i, c)| format!("P{}: {} units", i, c))
        .collect();
    println!(
        "Game over: {} after {} turns (seed: {}). {}. {} combats.",
        winner_str,
        result.turns,
        result.seed,
        units_str.join(". "),
        result.combats,
    );
}

fn make_agents(num_players: u8, base_seed: u64) -> Vec<Box<dyn Agent>> {
    (0..num_players)
        .map(|i| {
            let agent_seed = base_seed.wrapping_add(i as u64);
            Box::new(SimpleAgent::new(ChaCha8Rng::seed_from_u64(agent_seed))) as Box<dyn Agent>
        })
        .collect()
}

fn run_batch(cli: &Cli, count: u32) {
    let mut total_wins: Vec<u32> = vec![0; cli.players as usize];
    let mut total_draws = 0u32;
    let mut total_turns = 0u64;
    let mut total_combats = 0u64;
    let mut panicked_seeds: Vec<u64> = Vec::new();

    for i in 0..count {
        let seed = cli.seed.wrapping_add(i as u64);
        let batch_cli = Cli {
            seed,
            width: cli.width,
            height: cli.height,
            players: cli.players,
            max_turns: cli.max_turns,
            verbose: false,
            quiet: true,
            step: false,
            batch: None,
            save_log: None,
            replay: None,
        };
        let config = make_config(&batch_cli);

        let game_result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
            let mut agents = make_agents(batch_cli.players, seed);
            run_game(&config, &mut agents, false, true, false)
        }));

        match game_result {
            Ok(result) => {
                if let Some(winner) = result.winner {
                    total_wins[winner.0 as usize] += 1;
                } else {
                    total_draws += 1;
                }
                total_turns += result.turns as u64;
                total_combats += result.combats as u64;

                if !cli.quiet {
                    let winner_str = match result.winner {
                        Some(p) => format!("P{} wins", p.0),
                        None => "Draw".to_string(),
                    };
                    println!(
                        "Game {}/{}: {} in {} turns ({} combats)",
                        i + 1,
                        count,
                        winner_str,
                        result.turns,
                        result.combats,
                    );
                }
            }
            Err(_) => {
                panicked_seeds.push(seed);
                println!("Game {}/{}: PANIC (seed: {})", i + 1, count, seed);
            }
        }
    }

    // Summary
    println!();
    let completed = count - panicked_seeds.len() as u32;
    let avg_turns = if completed > 0 {
        total_turns / completed as u64
    } else {
        0
    };
    let avg_combats = if completed > 0 {
        total_combats / completed as u64
    } else {
        0
    };
    let wins_str: Vec<String> = total_wins
        .iter()
        .enumerate()
        .map(|(i, w)| format!("P{}: {}", i, w))
        .collect();
    println!(
        "{}/{} games completed. Avg turns: {}. Avg combats: {}. Wins: {}. Draws: {}.",
        completed,
        count,
        avg_turns,
        avg_combats,
        wins_str.join(", "),
        total_draws,
    );
    if !panicked_seeds.is_empty() {
        let seeds_str: Vec<String> = panicked_seeds.iter().map(|s| s.to_string()).collect();
        println!("FAILED SEEDS: {}", seeds_str.join(", "));
    }
}

fn replay_log(path: &str, verbose: bool) {
    let json = std::fs::read_to_string(path).unwrap();
    let log: GameLog = serde_json::from_str(&json).unwrap();
    let engine = log.replay().unwrap();

    println!("Replay complete.");
    println!("Turn: {}", engine.current_turn());

    // Print final state summary
    let num_players = log.game_config.world.num_players;
    for p in 0..num_players {
        let player = PlayerId(p);
        let alive = engine.is_player_alive(player);
        let units = count_units(&engine, player);
        println!(
            "  Player {}: {} ({} units)",
            p,
            if alive { "alive" } else { "eliminated" },
            units,
        );
    }

    if let Some(winner) = engine.is_game_over() {
        println!("Winner: Player {}", winner.0);
    }

    if verbose {
        let view = engine.player_view(PlayerId(0));
        println!(
            "{}",
            render_ascii(
                &view,
                log.game_config.world.width,
                log.game_config.world.height
            )
        );
    }
}

fn main() {
    let cli = Cli::parse();

    println!("FreeC3 headless client v{}", fc3_core::version());

    // Handle replay mode
    if let Some(ref path) = cli.replay {
        replay_log(path, cli.verbose);
        return;
    }

    // Handle batch mode
    if let Some(count) = cli.batch {
        run_batch(&cli, count);
        return;
    }

    // Single game mode
    let config = make_config(&cli);
    let mut agents = make_agents(cli.players, cli.seed);

    if cli.save_log.is_some() {
        let (result, log) =
            run_game_with_log(&config, &mut agents, cli.verbose, cli.quiet, cli.step);
        print_summary(&result);

        if let Some(ref path) = cli.save_log {
            let json = serde_json::to_string_pretty(&log).unwrap();
            std::fs::write(path, json).unwrap();
            println!("Game log saved to: {}", path);
        }
    } else {
        let result = run_game(&config, &mut agents, cli.verbose, cli.quiet, cli.step);
        print_summary(&result);
    }
}
