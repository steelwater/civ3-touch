use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use glyphon::{
    Attrs, Buffer as TextBuffer, Cache as GlyphonCache, Color as TextColor, Family, FontSystem,
    Metrics, Resolution, Shaping, SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer,
    Viewport,
};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use winit::application::ApplicationHandler;
use winit::event::{MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{WindowAttributes, WindowId};

use fc3_core::ai::{Agent, SimpleAgent};
use fc3_core::engine::{Engine, GameConfig};
use fc3_core::id::UnitId;
use fc3_core::protocol::{Command, Event, PlayerView, TileSnapshot};
use fc3_core::tile::{Terrain, Vegetation, Visibility};
use fc3_core::types::{PlayerId, TileCoord};
use fc3_core::world::WorldConfig;

mod advisor;
mod animation;
mod camera;
mod city_atlas;
mod city_view;
mod colors;
mod ini;
mod input;
mod instances;
mod menu;
mod renderer;
mod screen;
mod tech_dialog;
mod terrain_atlas;
mod ui_atlas;
mod ui_box;
mod ui_component;
mod ui_layer;
mod unit_atlas;

use animation::AnimationState;
use camera::{adjust_y_for_wrap, logical_to_iso, Camera, CameraUniform};
use city_view::CityViewData;
use colors::{
    improvement_tint, ownership_tint, player_color, road_tint, terrain_color, vegetation_tint,
};
use input::{InputState, DOUBLE_CLICK_MS};
use instances::{
    CityInstance, PathLineInstance, SelectionCircleInstance, TileInstance, UIRectInstance,
    UITexturedRectInstance, UnitInstance,
};
use renderer::{Renderer, UnitDrawBatch};
use screen::UIScreen;
use tech_dialog::{TechDialog, TechDialogAction, TechDialogProps, TechItem};
use ui_box::{BoxImage, BoxState, UIBox};
use ui_component::{flatten, BuildCtx, FlattenedUI, StateStore};
use ui_layer::{build_layer_visuals, LayerEvent, PopupMenu, PopupMenuItem, RectElement, UILayer};

const MAP_WIDTH: u32 = 60;
const MAP_HEIGHT: u32 = 60;
const NUM_PLAYERS: u8 = 3;
const MAX_TURNS: u32 = 100;
const SEED: u64 = 42;
const AI_STEP_INTERVAL: f32 = 0.1; // seconds between AI actions

struct TextState {
    font_system: FontSystem,
    swash_cache: SwashCache,
    atlas: TextAtlas,
    viewport: Viewport,
    text_renderer: TextRenderer,
    ui_font_family: Option<String>,
}

struct RenderState {
    text_state: TextState, // dropped before renderer so GPU resources release while device is alive
    renderer: Renderer,
}

/// A component-based dialog currently active on screen.
enum ActiveDialog {
    TechSelect { options: Vec<TechItem> },
}

struct GameState {
    engine: Engine,
    agents: Vec<Box<dyn Agent>>,
    camera: Camera,
    animations: AnimationState,
    input: InputState,
    viewing_player: PlayerId,
    cached_view: Option<PlayerView>,
    ai_timer: f32,
    last_frame: Instant,
    game_over: bool,
    frame_count: u64,
    fps_timer: f32,
    current_fps: f32,
    paused: bool,
    /// When true, all players are AI-controlled (spectator mode).
    /// When false, player 0 is human-controlled.
    auto_play: bool,
    /// Which UI screen/mode is currently active.
    active_screen: UIScreen,
    /// Set to true to close the application from a menu action.
    exit_requested: bool,
    /// Data for the city view screen, populated on entry.
    city_view_data: Option<CityViewData>,
    /// Stack of UI layers (popups, modals) rendered on top of the current screen.
    ui_layers: Vec<UILayer>,
    /// Debug: when true, all tiles shown as fully visible (no fog of war).
    debug_reveal: bool,
    /// When true, select next moveable unit once all queued animations finish.
    pending_auto_cycle: bool,
    /// When set, submit build_city for this unit once its animation finishes, then open city view.
    pending_found_city: Option<fc3_core::id::UnitId>,
    /// Action boxes displayed at the bottom of the screen for the selected unit.
    action_boxes: Vec<(UIBox, String)>,
    /// Which action box the mouse is currently hovering over.
    hovered_box_index: Option<usize>,
    /// Which action box is currently being pressed (mouse down).
    active_box_index: Option<usize>,
    /// Menu buttons displayed at the top-left of the screen.
    menu_boxes: Vec<UIBox>,
    /// Which menu button the mouse is currently hovering over.
    hovered_menu_index: Option<usize>,
    /// Which menu button is currently being pressed (mouse down).
    active_menu_index: Option<usize>,
    /// Component system state store (persists across frames).
    component_state: StateStore,
    /// Active component-based dialog (if any).
    active_dialog: Option<ActiveDialog>,
    /// Consumed key for component dialog input (set during frame, cleared after build).
    dialog_key_pressed: Option<KeyCode>,
    /// Consumed scroll delta for component dialog input.
    dialog_scroll_delta: f32,
    /// Whether mouse was clicked this frame for component dialog input.
    dialog_mouse_clicked: bool,
}

impl GameState {
    /// Get a player view, using debug_view when reveal mode is on.
    fn get_view(&self) -> PlayerView {
        if self.debug_reveal {
            self.engine.debug_view(self.viewing_player)
        } else {
            self.engine.player_view(self.viewing_player)
        }
    }
}

struct DesktopApp {
    render_state: Option<RenderState>,
    game_state: Option<GameState>,
    resource_dir: Option<PathBuf>,
}

impl DesktopApp {
    fn new(resource_dir: Option<PathBuf>) -> Self {
        DesktopApp {
            render_state: None,
            game_state: None,
            resource_dir,
        }
    }

    fn init_game(&mut self) {
        let config = GameConfig {
            world: WorldConfig {
                width: MAP_WIDTH,
                height: MAP_HEIGHT,
                wrap_x: true,
                wrap_y: true,
                num_players: NUM_PLAYERS,
                seed: SEED,
            },
            mod_paths: vec!["base".to_string()],
            units_per_player: vec!["warrior".to_string(), "settler".to_string()],
            max_turns: Some(MAX_TURNS),
        };

        let engine = Engine::new_game(&config).unwrap();
        let agents: Vec<Box<dyn Agent>> = (0..NUM_PLAYERS)
            .map(|i| {
                let agent_seed = SEED.wrapping_add(i as u64);
                Box::new(SimpleAgent::new(ChaCha8Rng::seed_from_u64(agent_seed))) as Box<dyn Agent>
            })
            .collect();

        let camera = Camera::new(
            MAP_WIDTH as f32,
            MAP_HEIGHT as f32,
            1280.0,
            720.0,
            config.world.wrap_x,
            config.world.wrap_y,
        );

        // Check environment for play mode: AUTOPLAY=1 for spectator, default is human play.
        let auto_play = std::env::var("AUTOPLAY").is_ok_and(|v| v == "1");

        // Auto-produce warriors for human player's cities (production gap stopgap)
        // This handles the case where cities have no production set
        let viewing_player = PlayerId(0);

        self.game_state = Some(GameState {
            engine,
            agents,
            camera,
            animations: AnimationState::new(),
            input: InputState::new(),
            viewing_player,
            cached_view: None,
            ai_timer: 0.0,
            last_frame: Instant::now(),
            game_over: false,
            frame_count: 0,
            fps_timer: 0.0,
            current_fps: 0.0,
            paused: false,
            auto_play,
            active_screen: UIScreen::Overworld,
            exit_requested: false,
            city_view_data: None,
            ui_layers: Vec::new(),
            debug_reveal: false,
            pending_auto_cycle: false,
            pending_found_city: None,
            action_boxes: Vec::new(),
            hovered_box_index: None,
            active_box_index: None,
            menu_boxes: Vec::new(),
            hovered_menu_index: None,
            active_menu_index: None,
            component_state: StateStore::new(),
            active_dialog: None,
            dialog_key_pressed: None,
            dialog_scroll_delta: 0.0,
            dialog_mouse_clicked: false,
        });

        // On game start in human mode, select first moveable unit and center camera
        if !auto_play {
            if let Some(game) = &mut self.game_state {
                Self::select_next_unit(game);
            }
        }
    }

    fn step_ai(game: &mut GameState) {
        if game.game_over || game.paused {
            return;
        }

        if game.engine.is_game_over().is_some() {
            game.game_over = true;
            game.cached_view = None;
            return;
        }

        let current = game.engine.current_player();

        if !game.engine.is_player_alive(current) {
            game.engine.submit_command(current, Command::EndTurn);
            game.cached_view = None;
            return;
        }

        let agent_idx = current.0 as usize;
        let view = game.engine.player_view(current);
        let available = game.engine.available_commands(current);
        let cmd = game.agents[agent_idx].decide(&view, &available);

        let is_end_turn = matches!(cmd, Command::EndTurn);

        let result = game.engine.submit_command(current, cmd);

        for event in &result.events {
            match event {
                Event::UnitMoved {
                    unit_id, from, to, ..
                } => {
                    let ut = lookup_unit_type_name(&game.engine, *unit_id);
                    game.animations.start_move(*unit_id, *from, *to, &ut);
                }
                Event::CombatStarted { tile, .. } => {
                    game.animations.start_combat_flash(*tile);
                }
                Event::UnitDestroyed { at, .. } => {
                    game.animations.start_combat_flash(*at);
                }
                _ => {}
            }
        }

        game.cached_view = None;

        if game.engine.is_game_over().is_some() {
            game.game_over = true;
        }

        if is_end_turn {
            game.viewing_player = game.engine.current_player();
        }
    }
}

/// Overlay info passed into tile instance building for hover/selection highlights.
struct TileOverlays {
    hovered_tile: Option<TileCoord>,
    selected_tile: Option<TileCoord>,
    /// Pulsing alpha for selection highlight (0.0 to 1.0 sine wave).
    selection_pulse: f32,
}

/// Pre-computed city banner data for rendering the Civ3-style city label box.
struct CityBannerInfo {
    screen_x: f32,
    screen_y: f32,
    pop: i32,
    color: [f32; 3],
    line1: String,
    line2: String,
    text_w: f32,
}

const BANNER_BORDER: f32 = 2.0;
const BANNER_POP_W: f32 = 22.0;
const BANNER_RIGHT_W: f32 = 22.0;
const BANNER_INNER_H: f32 = 30.0;
const BANNER_Y_OFFSET: f32 = 10.0;
const BANNER_CHAR_W: f32 = 6.5;
const BANNER_TEXT_PAD: f32 = 8.0;
const BANNER_FONT_SIZE: f32 = 11.0;
const BANNER_LINE_H: f32 = 13.0;
const BANNER_POP_FONT: f32 = 14.0;

fn build_tile_instances(
    view: &PlayerView,
    flash_tiles: &[(fc3_core::types::TileCoord, f32)],
    overlays: &TileOverlays,
    terrain_atlas: &terrain_atlas::TerrainAtlas,
) -> Vec<TileInstance> {
    let mut instances = Vec::with_capacity(view.visible_tiles.len());

    // Build terrain and visibility lookups for neighbor queries
    let terrain_map: HashMap<(u32, u32), Terrain> = view
        .visible_tiles
        .iter()
        .map(|t| ((t.coord.x, t.coord.y), t.terrain))
        .collect();

    let vis_map: HashMap<(u32, u32), Visibility> = view
        .visible_tiles
        .iter()
        .map(|t| ((t.coord.x, t.coord.y), t.visibility))
        .collect();

    let neighbor_terrain = |coord: TileCoord, dx: i32, dy: i32, fallback: Terrain| -> Terrain {
        let nx = (coord.x as i32 + dx).rem_euclid(view.map_width as i32) as u32;
        let ny = coord.y as i32 + dy;
        let ny = if view.wrap_y {
            ny.rem_euclid(view.map_height as i32) as u32
        } else if ny < 0 || ny >= view.map_height as i32 {
            return fallback;
        } else {
            ny as u32
        };
        terrain_map.get(&(nx, ny)).copied().unwrap_or(fallback)
    };

    let neighbor_vis = |x: i32, y: i32| -> u32 {
        let nx = x.rem_euclid(view.map_width as i32) as u32;
        let ny = if view.wrap_y {
            y.rem_euclid(view.map_height as i32) as u32
        } else if y < 0 || y >= view.map_height as i32 {
            return 0; // Unseen
        } else {
            y as u32
        };
        match vis_map.get(&(nx, ny)) {
            Some(Visibility::Visible) => 2,
            Some(Visibility::Revealed) => 1,
            _ => 0, // Unseen or missing
        }
    };

    for tile in &view.visible_tiles {
        let mut color = terrain_color(tile.terrain);
        color = vegetation_tint(color, tile.vegetation);

        if tile.road_level > 0 {
            color = road_tint(color);
        }

        if let Some(imp) = tile.improvement {
            color = improvement_tint(color, imp);
        }

        if let Some(owner) = tile.owner {
            color = ownership_tint(color, owner);
        }

        for (flash_tile, intensity) in flash_tiles {
            if tile.coord == *flash_tile {
                let flash_color = [1.0, 0.3, 0.2];
                for i in 0..3 {
                    color[i] = color[i] * (1.0 - intensity) + flash_color[i] * intensity;
                }
            }
        }

        // Hover highlight: brighten the tile slightly
        if overlays.hovered_tile == Some(tile.coord) && tile.visibility != Visibility::Unseen {
            for c in &mut color {
                *c = (*c + 0.15).min(1.0);
            }
        }

        // Selection highlight: pulsing yellow-white tint
        if overlays.selected_tile == Some(tile.coord) && tile.visibility != Visibility::Unseen {
            let pulse = overlays.selection_pulse;
            let sel_color = [1.0, 0.95, 0.4];
            for i in 0..3 {
                color[i] = color[i] * (1.0 - pulse * 0.4) + sel_color[i] * pulse * 0.4;
            }
        }

        let border_mask = compute_border_mask(
            tile,
            &view.visible_tiles,
            view.map_width,
            view.map_height,
            view.wrap_y,
        );
        let border_color = tile.owner.map(player_color).unwrap_or([0.0; 3]);

        // Junction-based terrain rendering: each diamond sits at the junction
        // of 4 grid cells: N=tile itself, W=south, E=east, S=SE diagonal.
        let hill_to_grass = |t: Terrain| -> Terrain {
            if t == Terrain::Hill {
                Terrain::Grassland
            } else {
                t
            }
        };

        let jn = hill_to_grass(tile.terrain);
        let jw = hill_to_grass(neighbor_terrain(tile.coord, 0, 1, tile.terrain));
        let je = hill_to_grass(neighbor_terrain(tile.coord, 1, 0, tile.terrain));
        let js = hill_to_grass(neighbor_terrain(tile.coord, 1, 1, tile.terrain));

        let (atlas_rect, overlay_rect) = if tile.terrain == Terrain::Hill {
            let base = terrain_atlas.get_junction_rect([jn, jw, je, js]);
            let is_elevated = |dx: i32, dy: i32| -> bool {
                let t = neighbor_terrain(tile.coord, dx, dy, tile.terrain);
                matches!(t, Terrain::Hill | Terrain::Mountain)
            };
            // Diagonal neighbors: [NW, NE, SE, SW]
            let diag = [
                is_elevated(-1, -1),
                is_elevated(1, -1),
                is_elevated(1, 1),
                is_elevated(-1, 1),
            ];
            let overlay = terrain_atlas.get_hill_rect(diag);
            (base, overlay)
        } else {
            let base = terrain_atlas.get_junction_rect([jn, jw, je, js]);
            (base, [0.0; 4])
        };

        // Vegetation overlay sprite (forest/jungle)
        let veg_variant = (tile.coord.x + tile.coord.y) % 4;
        let veg_rect =
            terrain_atlas.get_vegetation_rect(tile.terrain, tile.vegetation, veg_variant);

        // Visible tiles don't get any fog overlay, they're visible!
        let fog_rect = [0.0; 4];

        instances.push(TileInstance {
            position: [tile.coord.x as f32, tile.coord.y as f32],
            color,
            border_color,
            border_mask: border_mask as f32,
            atlas_rect,
            overlay_rect,
            veg_rect,
            fog_rect,
        });
    }

    // Fringe tiles: undiscovered neighbors of discovered tiles, rendered with real
    // terrain so the fog transition shows slivers of content underneath.
    for fringe in &view.fringe_tiles {
        let fringe_color = vegetation_tint(terrain_color(fringe.terrain), fringe.vegetation);

        // Junction-based terrain: use fringe terrain for self, look up neighbors from
        // terrain_map (discovered tiles) or fall back to fringe terrain.
        let fringe_neighbor = |dx: i32, dy: i32| -> Terrain {
            let nx = (fringe.coord.x as i32 + dx).rem_euclid(view.map_width as i32) as u32;
            let ny_raw = fringe.coord.y as i32 + dy;
            let ny = if view.wrap_y {
                ny_raw.rem_euclid(view.map_height as i32) as u32
            } else if ny_raw < 0 || ny_raw >= view.map_height as i32 {
                return fringe.terrain;
            } else {
                ny_raw as u32
            };
            terrain_map
                .get(&(nx, ny))
                .copied()
                .unwrap_or(fringe.terrain)
        };

        let hill_to_grass = |t: Terrain| -> Terrain {
            if t == Terrain::Hill {
                Terrain::Grassland
            } else {
                t
            }
        };
        let jn = hill_to_grass(fringe.terrain);
        let jw = hill_to_grass(fringe_neighbor(0, 1));
        let je = hill_to_grass(fringe_neighbor(1, 0));
        let js = hill_to_grass(fringe_neighbor(1, 1));
        let atlas_rect = terrain_atlas.get_junction_rect([jn, jw, je, js]);
        let fringe_veg_variant = (fringe.coord.x + fringe.coord.y) % 4;
        let veg_rect = terrain_atlas.get_vegetation_rect(
            fringe.terrain,
            fringe.vegetation,
            fringe_veg_variant,
        );

        // Cell-centered fog for fringe tiles (same cardinal neighbor lookup).
        let x = fringe.coord.x as i32;
        let y = fringe.coord.y as i32;
        let fog_rect = terrain_atlas.get_fog_rect([
            neighbor_vis(x, y - 1), // N neighbor
            neighbor_vis(x - 1, y), // W neighbor
            neighbor_vis(x + 1, y), // E neighbor
            neighbor_vis(x, y + 1), // S neighbor
        ]);

        instances.push(TileInstance {
            position: [fringe.coord.x as f32, fringe.coord.y as f32],
            color: fringe_color,
            border_color: [0.0; 3],
            border_mask: 0.0,
            atlas_rect,
            overlay_rect: [0.0; 4],
            veg_rect,
            fog_rect,
        });
    }

    instances
}

/// Look up a unit's type name from the engine (for animation duration lookup).
fn lookup_unit_type_name(
    engine: &fc3_core::engine::Engine,
    unit_id: fc3_core::id::UnitId,
) -> String {
    let world = engine.world().borrow();
    world
        .units
        .get(unit_id)
        .and_then(|idx| {
            let tid = world.units.unit_type[idx];
            world.unit_types.get(tid).map(|ut| ut.name.clone())
        })
        .unwrap_or_default()
}

fn build_unit_instances(
    view: &PlayerView,
    animations: &AnimationState,
    atlas: &unit_atlas::UnitAtlasSet,
    selected_unit: Option<UnitId>,
    tile_stack_index: &HashMap<TileCoord, usize>,
    viewing_player: PlayerId,
) -> (Vec<UnitInstance>, Vec<UnitDrawBatch>) {
    // Group units by tile position
    let mut units_by_tile: HashMap<TileCoord, Vec<usize>> = HashMap::new();
    for (i, unit) in view.known_units.iter().enumerate() {
        units_by_tile.entry(unit.position).or_default().push(i);
    }

    // For each tile, pick one unit to display
    let mut chosen_units: Vec<(usize, usize)> = Vec::new(); // (unit_index, stack_count)
    for (tile, indices) in &units_by_tile {
        let stack_count = indices.len().min(5);

        // If the selected unit is on this tile, always show it
        if let Some(sel_id) = selected_unit {
            if let Some(&idx) = indices.iter().find(|&&i| view.known_units[i].id == sel_id) {
                chosen_units.push((idx, stack_count));
                continue;
            }
        }

        // Check tile_stack_index for cycling (only for friendly tiles)
        let has_friendly = indices
            .iter()
            .any(|&i| view.known_units[i].owner == viewing_player);
        if has_friendly {
            if let Some(&cycle_idx) = tile_stack_index.get(tile) {
                // Filter to friendly units for cycling
                let friendly: Vec<usize> = indices
                    .iter()
                    .copied()
                    .filter(|&i| view.known_units[i].owner == viewing_player)
                    .collect();
                if !friendly.is_empty() {
                    let picked = friendly[cycle_idx % friendly.len()];
                    chosen_units.push((picked, stack_count));
                    continue;
                }
            }
        }

        // Default: show the first unit
        chosen_units.push((indices[0], stack_count));
    }

    // Group units by (type_name, anim_name), preserving order within each group
    let mut groups: Vec<((String, String), Vec<UnitInstance>)> = Vec::new();
    let mut group_index: HashMap<(String, String), usize> = HashMap::new();

    for &(unit_idx, stack_count) in &chosen_units {
        let unit = &view.known_units[unit_idx];
        let visual_pos = animations.visual_position(unit.id, unit.position);
        let color = player_color(unit.owner);
        let shape = if unit.category == "civilian" {
            1.0
        } else {
            0.0
        };
        let hp_frac = if unit.max_hp > 0 {
            unit.hp as f32 / unit.max_hp as f32
        } else {
            1.0
        };

        let offset = [0.0, 0.0];

        // Choose animation priority: RUN > named transition > looping action > FORTIFY idle > DEFAULT
        let is_moving = animations.is_moving(unit.id);
        let named = animations.named_anim_info(unit.id);
        let has_fortify_anim = atlas.has_anim(&unit.unit_type_name, "FORTIFY");
        let action_anim = unit
            .current_action_animation
            .as_deref()
            .filter(|a| atlas.has_anim(&unit.unit_type_name, a));
        let anim_name = if is_moving && atlas.has_anim(&unit.unit_type_name, "RUN") {
            "RUN"
        } else if let Some((name, _)) = named {
            if atlas.has_anim(&unit.unit_type_name, name) {
                name
            } else {
                "DEFAULT"
            }
        } else if let Some(a) = action_anim {
            a
        } else if unit.fortified && has_fortify_anim {
            "FORTIFY"
        } else {
            "DEFAULT"
        };

        let frame = if is_moving {
            // Sync frame to movement progress
            animations
                .run_frame(
                    unit.id,
                    atlas.frames_per_dir(&unit.unit_type_name, anim_name),
                )
                .unwrap_or(0)
        } else if let Some((_, progress)) = named {
            // Sync frame to named animation progress (play once through)
            let frames = atlas.frames_per_dir(&unit.unit_type_name, anim_name);
            if frames <= 1 {
                0
            } else {
                let f = (progress * frames as f32) as u32;
                f.min(frames - 1)
            }
        } else if action_anim.is_some() {
            // Loop action animation using idle clock
            animations.idle_frame(
                unit.id,
                atlas.frames_per_dir(&unit.unit_type_name, anim_name),
            )
        } else if unit.fortified && has_fortify_anim {
            // Show last frame of FORTIFY as static pose
            let frames = atlas.frames_per_dir(&unit.unit_type_name, "FORTIFY");
            frames.saturating_sub(1)
        } else {
            // Idle animation from global clock
            animations.idle_frame(
                unit.id,
                atlas.frames_per_dir(&unit.unit_type_name, "DEFAULT"),
            )
        };
        let direction = animations
            .direction_override(unit.id)
            .unwrap_or(unit.direction);
        let sprite = atlas.sprite_info(
            &unit.unit_type_name,
            anim_name,
            direction.flic_index(),
            frame,
        );

        let instance = UnitInstance {
            position: [visual_pos.x, visual_pos.y],
            color,
            shape,
            hp_frac,
            stack_count: stack_count as f32,
            offset,
            atlas_rect: sprite.atlas_rect,
            sprite_size: sprite.sprite_size,
        };

        let batch_key = (unit.unit_type_name.clone(), anim_name.to_string());
        match group_index.get(&batch_key) {
            Some(&gi) => groups[gi].1.push(instance),
            None => {
                let gi = groups.len();
                group_index.insert(batch_key.clone(), gi);
                groups.push((batch_key, vec![instance]));
            }
        }
    }

    // Flatten groups into a contiguous instance array with batch descriptors
    let mut all_instances = Vec::with_capacity(view.known_units.len());
    let mut batches = Vec::with_capacity(groups.len());

    for ((type_name, anim_name), instances) in groups {
        let start = all_instances.len() as u32;
        let count = instances.len() as u32;
        all_instances.extend(instances);
        batches.push(UnitDrawBatch {
            unit_type: type_name,
            anim_name,
            start,
            count,
        });
    }

    (all_instances, batches)
}

fn build_city_instances(view: &PlayerView, city_sprite_rect: [f32; 4]) -> Vec<CityInstance> {
    view.own_cities
        .iter()
        .chain(view.known_cities.iter())
        .map(|city| CityInstance {
            position: [city.position.x as f32, city.position.y as f32],
            color: player_color(city.owner),
            population: city.population as f32,
            atlas_rect: city_sprite_rect,
        })
        .collect()
}

fn compute_border_mask(
    tile: &TileSnapshot,
    all_tiles: &[TileSnapshot],
    map_width: u32,
    map_height: u32,
    wrap_y: bool,
) -> u32 {
    let owner = match tile.owner {
        Some(o) => o,
        None => return 0,
    };

    let find_owner = |dx: i32, dy: i32| -> Option<PlayerId> {
        let nx = (tile.coord.x as i32 + dx).rem_euclid(map_width as i32) as u32;
        let ny = tile.coord.y as i32 + dy;
        let ny = if wrap_y {
            ny.rem_euclid(map_height as i32) as u32
        } else {
            if ny < 0 || ny >= map_height as i32 {
                return None;
            }
            ny as u32
        };
        all_tiles
            .iter()
            .find(|t| t.coord.x == nx && t.coord.y == ny)
            .and_then(|t| t.owner)
    };

    let mut mask = 0u32;
    if find_owner(0, -1) != Some(owner) {
        mask |= 1;
    }
    if find_owner(1, 0) != Some(owner) {
        mask |= 2;
    }
    if find_owner(0, 1) != Some(owner) {
        mask |= 4;
    }
    if find_owner(-1, 0) != Some(owner) {
        mask |= 8;
    }
    mask
}

/// Duplicate tile instances at each wrap offset for seamless x-axis wrapping.
fn apply_wrap_tile(instances: &[TileInstance], offsets: &[f32]) -> Vec<TileInstance> {
    if offsets.len() == 1 {
        return instances.to_vec();
    }
    let mut out = Vec::with_capacity(instances.len() * offsets.len());
    for offset in offsets {
        for inst in instances {
            let mut copy = *inst;
            copy.position[0] += offset;
            out.push(copy);
        }
    }
    out
}

/// Duplicate unit instances at each wrap offset, preserving batch grouping.
fn apply_wrap_unit_batched(
    instances: &[UnitInstance],
    batches: &[UnitDrawBatch],
    offsets: &[f32],
) -> (Vec<UnitInstance>, Vec<UnitDrawBatch>) {
    if offsets.len() == 1 {
        return (
            instances.to_vec(),
            batches
                .iter()
                .map(|b| UnitDrawBatch {
                    unit_type: b.unit_type.clone(),
                    anim_name: b.anim_name.clone(),
                    start: b.start,
                    count: b.count,
                })
                .collect(),
        );
    }

    let mut all_instances = Vec::with_capacity(instances.len() * offsets.len());
    let mut new_batches = Vec::with_capacity(batches.len());

    for batch in batches {
        let start = all_instances.len() as u32;
        let batch_slice = &instances[batch.start as usize..(batch.start + batch.count) as usize];
        for offset in offsets {
            for inst in batch_slice {
                let mut copy = *inst;
                copy.position[0] += offset;
                all_instances.push(copy);
            }
        }
        new_batches.push(UnitDrawBatch {
            unit_type: batch.unit_type.clone(),
            anim_name: batch.anim_name.clone(),
            start,
            count: all_instances.len() as u32 - start,
        });
    }

    (all_instances, new_batches)
}

/// Duplicate path line instances at each wrap offset.
fn apply_wrap_path_line(instances: &[PathLineInstance], offsets: &[f32]) -> Vec<PathLineInstance> {
    if offsets.len() == 1 {
        return instances.to_vec();
    }
    let mut out = Vec::with_capacity(instances.len() * offsets.len());
    for offset in offsets {
        for inst in instances {
            let mut copy = *inst;
            copy.start_pos[0] += offset;
            copy.end_pos[0] += offset;
            out.push(copy);
        }
    }
    out
}

/// Duplicate city instances at each wrap offset.
fn apply_wrap_city(instances: &[CityInstance], offsets: &[f32]) -> Vec<CityInstance> {
    if offsets.len() == 1 {
        return instances.to_vec();
    }
    let mut out = Vec::with_capacity(instances.len() * offsets.len());
    for offset in offsets {
        for inst in instances {
            let mut copy = *inst;
            copy.position[0] += offset;
            out.push(copy);
        }
    }
    out
}

impl ApplicationHandler for DesktopApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.render_state.is_some() {
            return;
        }

        let window_attrs = WindowAttributes::default()
            .with_title("FreeC3 — Desktop Viewer")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0));

        let window = Arc::new(event_loop.create_window(window_attrs).unwrap());

        let gpu_renderer = Renderer::new(window.clone(), self.resource_dir.as_deref());

        let mut font_system = FontSystem::new();
        let mut ui_font_family = None;
        if let Some(ref dir) = self.resource_dir {
            let font_path = dir.join("LSANS.TTF");
            if font_path.exists() {
                match font_system.db_mut().load_font_file(&font_path) {
                    Ok(()) => {
                        ui_font_family = Some("Lucida Sans".to_string());
                        log::info!("Loaded UI font from {}", font_path.display());
                    }
                    Err(e) => {
                        log::warn!("Failed to load {}: {}", font_path.display(), e);
                    }
                }
            }
        }
        let swash_cache = SwashCache::new();
        let glyphon_cache = GlyphonCache::new(&gpu_renderer.device);
        let mut atlas = TextAtlas::new(
            &gpu_renderer.device,
            &gpu_renderer.queue,
            &glyphon_cache,
            gpu_renderer.config.format,
        );
        let viewport = Viewport::new(&gpu_renderer.device, &glyphon_cache);
        let text_renderer = TextRenderer::new(
            &mut atlas,
            &gpu_renderer.device,
            wgpu::MultisampleState::default(),
            None,
        );

        self.render_state = Some(RenderState {
            renderer: gpu_renderer,
            text_state: TextState {
                font_system,
                swash_cache,
                atlas,
                viewport,
                text_renderer,
                ui_font_family,
            },
        });

        self.init_game();

        // Load per-unit-type sprite atlases now that the engine knows which unit types exist
        if let (Some(ref dir), Some(ref mut rs), Some(ref mut game)) = (
            &self.resource_dir,
            &mut self.render_state,
            &mut self.game_state,
        ) {
            let unit_art = game.engine.unit_art_paths();
            if !unit_art.is_empty() {
                rs.renderer.load_unit_art(dir, &unit_art);
                game.animations
                    .set_run_durations(rs.renderer.unit_atlas.run_durations());
                for anim_name in rs.renderer.unit_atlas.all_anim_names() {
                    game.animations.set_anim_durations(
                        &anim_name,
                        rs.renderer.unit_atlas.anim_durations(&anim_name),
                    );
                }
            }
        }

        // Build static menu buttons from menu button atlas dimensions
        if let (Some(ref rs), Some(ref mut game)) = (&self.render_state, &mut self.game_state) {
            if let Some(ref atlas) = rs.renderer.menu_button_atlas {
                game.menu_boxes = build_menu_boxes(atlas.width, atlas.height);
            }
        }

        if let Some(game) = &mut self.game_state {
            let size = window.inner_size();
            game.camera
                .set_viewport(size.width as f32, size.height as f32);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(new_size) => {
                if let Some(rs) = &mut self.render_state {
                    rs.renderer.resize(new_size);
                    rs.text_state.viewport.update(
                        &rs.renderer.queue,
                        Resolution {
                            width: new_size.width,
                            height: new_size.height,
                        },
                    );
                }
                if let Some(game) = &mut self.game_state {
                    game.camera
                        .set_viewport(new_size.width as f32, new_size.height as f32);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(key) = event.physical_key {
                    if let Some(game) = &mut self.game_state {
                        // Only track held keys for camera pan in Overworld
                        if matches!(game.active_screen, UIScreen::Overworld) {
                            if event.state.is_pressed() {
                                game.input.keys_held.insert(key);
                            } else {
                                game.input.keys_held.remove(&key);
                            }
                        } else {
                            // Release tracking still works so keys don't stick
                            if !event.state.is_pressed() {
                                game.input.keys_held.remove(&key);
                            }
                        }
                    }
                    if event.state.is_pressed() && !event.repeat {
                        self.handle_key(key);
                    }
                    // Forward typed text to debug terminal
                    if event.state.is_pressed() {
                        if let Some(ref text) = event.text {
                            if let Some(game) = &mut self.game_state {
                                if let Some(top) = game.ui_layers.last_mut() {
                                    top.handle_text(text);
                                }
                            }
                        }
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if state.is_pressed() {
                    if button == winit::event::MouseButton::Right {
                        // Right-click down: just record the hold, don't fire move yet
                        if let Some(game) = &mut self.game_state {
                            game.input.mouse_buttons_held.insert(button);
                        }
                    } else {
                        // Track active box/menu on left press
                        if button == winit::event::MouseButton::Left {
                            if let Some(game) = &mut self.game_state {
                                game.active_box_index = game.hovered_box_index;
                                game.active_menu_index = game.hovered_menu_index;
                            }
                        }
                        self.handle_mouse_click(button);
                    }
                } else {
                    // Button released
                    if button == winit::event::MouseButton::Left {
                        if let Some(game) = &mut self.game_state {
                            game.active_box_index = None;
                            game.active_menu_index = None;
                        }
                    }
                    if button == winit::event::MouseButton::Right {
                        if let Some(game) = &mut self.game_state {
                            game.input.mouse_buttons_held.remove(&button);
                        }
                        // Fire the move on release (if in overworld with selected unit)
                        let should_move = self.game_state.as_ref().is_some_and(|g| {
                            matches!(g.active_screen, UIScreen::Overworld)
                                && g.input.selected_unit.is_some()
                                && g.input.hovered_tile.is_some()
                                && !g.auto_play
                        });
                        if should_move {
                            let hovered = self
                                .game_state
                                .as_ref()
                                .unwrap()
                                .input
                                .hovered_tile
                                .unwrap();
                            self.handle_right_click(hovered);
                            if let Some(game) = &mut self.game_state {
                                game.input.goto_mode = false;
                            }
                        }
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(game) = &mut self.game_state {
                    let sx = position.x as f32;
                    let sy = position.y as f32;
                    game.input.mouse_screen_pos = (sx, sy);

                    // Pass cursor to top layer if any
                    if let Some(top) = game.ui_layers.last_mut() {
                        top.handle_cursor_move(sx, sy);
                    }

                    // Only update hovered tile in overworld
                    if matches!(game.active_screen, UIScreen::Overworld) {
                        let (mw, mh) = game
                            .cached_view
                            .as_ref()
                            .map(|v| (v.map_width, v.map_height))
                            .unwrap_or((MAP_WIDTH, MAP_HEIGHT));
                        game.input.hovered_tile = game.camera.screen_to_tile(sx, sy, mw, mh);

                        // Update hovered action box and menu button (only when no layers open)
                        if game.ui_layers.is_empty() {
                            game.hovered_box_index = game
                                .action_boxes
                                .iter()
                                .position(|(b, _)| b.contains(sx, sy));
                            game.hovered_menu_index =
                                game.menu_boxes.iter().position(|b| b.contains(sx, sy));
                        } else {
                            game.hovered_box_index = None;
                            game.hovered_menu_index = None;
                        }
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if let Some(game) = &mut self.game_state {
                    let scroll_delta = match delta {
                        winit::event::MouseScrollDelta::LineDelta(_, y) => y,
                        winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
                    };
                    // Component dialog consumes scroll when active
                    if game.active_dialog.is_some() {
                        game.dialog_scroll_delta += scroll_delta;
                    } else if !game.ui_layers.is_empty() {
                        // Pass scroll to top layer if any
                        if let Some(top) = game.ui_layers.last_mut() {
                            top.handle_scroll(scroll_delta);
                        }
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                self.update_and_render();
            }
            _ => {}
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        // Drop render state while the event loop is still alive.
        // On Linux the Vulkan surface drop needs a valid display connection;
        // deferring this to after run_app() returns causes SIGSEGV.
        self.render_state = None;
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(game) = &self.game_state {
            if game.exit_requested {
                event_loop.exit();
                return;
            }
        }
        if let Some(rs) = &self.render_state {
            rs.renderer.window.request_redraw();
        }
    }
}

impl DesktopApp {
    /// Computes path preview as line segments between consecutive path tiles,
    /// plus the number of turns to reach the destination.
    fn compute_path_preview(
        game: &mut GameState,
    ) -> (Vec<PathLineInstance>, Option<(TileCoord, i32)>) {
        // Only show path preview in goto mode or while right mouse button is held
        if !game.input.goto_mode && !game.input.mouse_buttons_held.contains(&MouseButton::Right) {
            return (Vec::new(), None);
        }

        let unit_id = match game.input.selected_unit {
            Some(uid) => uid,
            None => return (Vec::new(), None),
        };
        let hovered = match game.input.hovered_tile {
            Some(t) => t,
            None => return (Vec::new(), None),
        };

        // Get unit movement info for turns calculation
        if game.cached_view.is_none() {
            game.cached_view = Some(game.get_view());
        }
        let view = game.cached_view.as_ref().unwrap();
        let (movement, max_movement) = view
            .known_units
            .iter()
            .find(|u| u.id == unit_id)
            .map(|u| (u.movement, u.max_movement))
            .unwrap_or((0, 1));

        let path_result = game
            .engine
            .query_path(game.viewing_player, unit_id, hovered);

        match path_result {
            Some(path) if path.tiles.len() >= 2 => {
                let mut lines = Vec::with_capacity(path.tiles.len() - 1);
                for i in 0..path.tiles.len() - 1 {
                    let from = path.tiles[i];
                    let to = path.tiles[i + 1];
                    lines.push(PathLineInstance {
                        start_pos: [from.x as f32, from.y as f32],
                        end_pos: [to.x as f32, to.y as f32],
                        color: [0.9, 0.05, 0.05, 1.0],
                    });
                }

                // Simulate movement turn-by-turn to compute actual turns needed.
                // Rule: a unit with any movement > 0 can always move one tile,
                // even if the tile's cost exceeds remaining movement.
                let turns = if max_movement <= 0 {
                    0
                } else {
                    let mut t = 1;
                    let mut remaining = movement;
                    for i in 1..path.tiles.len() {
                        let step_cost = path.costs[i] - path.costs[i - 1];
                        if remaining <= 0 {
                            t += 1;
                            remaining = max_movement;
                        }
                        remaining = (remaining - step_cost).max(0);
                    }
                    t
                };

                let dest = *path.tiles.last().unwrap();
                (lines, Some((dest, turns)))
            }
            _ => (Vec::new(), None),
        }
    }

    fn handle_mouse_click(&mut self, button: winit::event::MouseButton) {
        // Component dialog consumes all clicks when active
        if let Some(game) = &mut self.game_state {
            if game.active_dialog.is_some() && button == winit::event::MouseButton::Left {
                game.dialog_mouse_clicked = true;
                return;
            }
        }

        // Layer-first input dispatch
        if let Some(game) = &mut self.game_state {
            if !game.ui_layers.is_empty() && button == winit::event::MouseButton::Left {
                let (sx, sy) = game.input.mouse_screen_pos;
                let top = game.ui_layers.last_mut().unwrap();
                if top.bounds().contains(sx, sy) {
                    if let Some(event) = top.handle_click(sx, sy) {
                        // Need to process the event — can't borrow self here,
                        // so store and handle below
                        let event = Some(event);
                        // Process outside the mutable borrow
                        if let Some(evt) = event {
                            Self::process_layer_event_static(game, evt);
                        }
                    }
                } else {
                    // Click outside top layer — close it
                    game.ui_layers.pop();
                }
                return;
            }
        }

        let screen = match &self.game_state {
            Some(g) => g.active_screen.clone(),
            None => return,
        };
        match screen {
            UIScreen::Overworld => self.handle_mouse_click_overworld(button),
            UIScreen::CityView { .. } => self.handle_mouse_click_city_view(button),
            UIScreen::MainMenu { .. } => self.handle_mouse_click_main_menu(button),
        }
    }

    fn handle_mouse_click_overworld(&mut self, button: winit::event::MouseButton) {
        // Freeze game input during animations
        if let Some(game) = &self.game_state {
            if game.animations.is_busy() {
                return;
            }
        }

        // Check action box clicks first (left button only)
        if button == winit::event::MouseButton::Left {
            let clicked_action = {
                let game = match &self.game_state {
                    Some(g) => g,
                    None => return,
                };
                let (sx, sy) = game.input.mouse_screen_pos;
                game.action_boxes
                    .iter()
                    .find(|(b, _)| b.contains(sx, sy))
                    .map(|(_, action_id)| action_id.clone())
            };
            if let Some(action_id) = clicked_action {
                self.dispatch_action_box(&action_id);
                return;
            }

            // Check menu button clicks
            let clicked_menu = {
                let game = match &self.game_state {
                    Some(g) => g,
                    None => return,
                };
                let (sx, sy) = game.input.mouse_screen_pos;
                game.menu_boxes.iter().position(|b| b.contains(sx, sy))
            };
            if let Some(idx) = clicked_menu {
                if idx == 0 {
                    // Left button: open main menu (same as Escape with nothing selected)
                    if let Some(game) = &mut self.game_state {
                        game.input.keys_held.clear();
                        game.active_screen = UIScreen::MainMenu { selected_index: 0 };
                    }
                }
                return;
            }
        }

        // Goto mode: left-click commits the move and exits goto mode
        if button == winit::event::MouseButton::Left {
            let goto_commit = self.game_state.as_ref().is_some_and(|g| {
                g.input.goto_mode
                    && g.input.selected_unit.is_some()
                    && g.input.hovered_tile.is_some()
            });
            if goto_commit {
                let hovered = self
                    .game_state
                    .as_ref()
                    .unwrap()
                    .input
                    .hovered_tile
                    .unwrap();
                self.handle_right_click(hovered);
                if let Some(game) = &mut self.game_state {
                    game.input.goto_mode = false;
                }
                return;
            }
        }

        let game = match &mut self.game_state {
            Some(g) => g,
            None => return,
        };
        if game.auto_play {
            return;
        }

        let hovered = match game.input.hovered_tile {
            Some(t) => t,
            None => return,
        };

        // Refresh view if needed
        if game.cached_view.is_none() {
            game.cached_view = Some(game.get_view());
        }
        let view = game.cached_view.as_ref().unwrap();

        if button == winit::event::MouseButton::Left {
            // Detect double-click: same tile within threshold
            let now = Instant::now();
            let is_double_click = game.input.last_left_click.is_some_and(|(tile, time)| {
                tile == hovered && now.duration_since(time).as_millis() < DOUBLE_CLICK_MS
            });
            game.input.last_left_click = Some((hovered, now));

            // Double-click on own city: open city view (regardless of units)
            if is_double_click {
                let found_city = view.own_cities.iter().find(|c| c.position == hovered);
                if let Some(city) = found_city {
                    let city_id = city.id;
                    self.open_city_view(city_id);
                    return;
                }
            }

            // Single-click: select/cycle friendly units
            let friendly_units: Vec<UnitId> = view
                .known_units
                .iter()
                .filter(|u| u.position == hovered && u.owner == game.viewing_player)
                .map(|u| u.id)
                .collect();

            if !friendly_units.is_empty() {
                // Check if clicking a tile that already contains the selected unit
                let selected_on_tile = game
                    .input
                    .selected_unit
                    .is_some_and(|sel| friendly_units.contains(&sel));

                if selected_on_tile && friendly_units.len() > 1 {
                    // Cycle to the next unit in the stack
                    let current_idx = game
                        .input
                        .tile_stack_index
                        .get(&hovered)
                        .copied()
                        .unwrap_or(0);
                    let next_idx = (current_idx + 1) % friendly_units.len();
                    game.input.tile_stack_index.insert(hovered, next_idx);
                    game.input.selected_unit = Some(friendly_units[next_idx]);
                    game.input.goto_mode = false;
                } else {
                    // Select the first friendly unit (or the displayed one per stack index)
                    let idx = game
                        .input
                        .tile_stack_index
                        .get(&hovered)
                        .copied()
                        .unwrap_or(0)
                        % friendly_units.len();
                    game.input.selected_unit = Some(friendly_units[idx]);
                    game.input.goto_mode = false;
                }
            } else {
                // No friendly unit on this tile — deselect
                game.input.selected_unit = None;
                game.input.goto_mode = false;
            }
        }
    }

    fn handle_mouse_click_city_view(&mut self, button: winit::event::MouseButton) {
        if button != winit::event::MouseButton::Left {
            return;
        }
        let game = match &mut self.game_state {
            Some(g) => g,
            None => return,
        };
        let (sx, sy) = game.input.mouse_screen_pos;
        let rs = match &self.render_state {
            Some(rs) => rs,
            None => return,
        };
        let w = rs.renderer.config.width as f32;
        let h = rs.renderer.config.height as f32;

        // Close button in top-right: x > w-50, y < 30
        if sx > w - 50.0 && sy < 30.0 {
            let game = self.game_state.as_mut().unwrap();
            game.active_screen = UIScreen::Overworld;
            game.city_view_data = None;
            return;
        }

        // Check click on "Producing:" line (line index 6 in info panel)
        let half_h = h / 2.0;
        let panel_start_y = half_h + 20.0;
        let line_height = 24.0;
        let producing_line_y = panel_start_y + 6.0 * line_height;

        if sy >= producing_line_y
            && sy < producing_line_y + line_height
            && (30.0..400.0).contains(&sx)
        {
            // Open production popup
            let city_id = match &game.active_screen {
                UIScreen::CityView { city_id } => *city_id,
                _ => return,
            };

            let available = game.engine.available_commands(game.viewing_player);
            let prod_cmd = available.iter().find(|cmd| {
                matches!(
                    cmd,
                    fc3_core::protocol::AvailableCommand::SetProduction {
                        city_id: cid,
                        ..
                    } if *cid == city_id
                )
            });

            if let Some(fc3_core::protocol::AvailableCommand::SetProduction { options, .. }) =
                prod_cmd
            {
                if options.is_empty() {
                    return;
                }

                let game = self.game_state.as_mut().unwrap();
                let items: Vec<PopupMenuItem> = options
                    .iter()
                    .map(|opt| PopupMenuItem {
                        label: opt.name.clone(),
                        detail: Some(format!("{} shields", opt.cost)),
                        enabled: true,
                    })
                    .collect();

                let menu = PopupMenu::new(
                    Some("Set Production".to_string()),
                    items,
                    30.0,
                    producing_line_y + line_height,
                    w,
                    h,
                );

                game.ui_layers.push(UILayer::ProductionPopup {
                    city_id,
                    menu,
                    options: options.clone(),
                });
            }
        }
    }

    fn handle_mouse_click_main_menu(&mut self, button: winit::event::MouseButton) {
        if button != winit::event::MouseButton::Left {
            return;
        }
        let game = match &mut self.game_state {
            Some(g) => g,
            None => return,
        };
        let (sx, sy) = game.input.mouse_screen_pos;

        let rs = match &self.render_state {
            Some(rs) => rs,
            None => return,
        };
        let w = rs.renderer.config.width as f32;
        let h = rs.renderer.config.height as f32;

        // Close button (X) in top-right: x > w-50, y < 30
        if sx > w - 50.0 && sy < 30.0 {
            let game = self.game_state.as_mut().unwrap();
            game.active_screen = UIScreen::Overworld;
            return;
        }

        // Menu items: centered vertically, each ~30px tall, starting from center-40
        let menu_start_y = h / 2.0 - 40.0;
        let item_height = 30.0;
        let menu_x_min = w / 2.0 - 120.0;
        let menu_x_max = w / 2.0 + 120.0;

        if sx >= menu_x_min && sx <= menu_x_max {
            for (i, _item) in menu::MENU_ITEMS.iter().enumerate() {
                let item_y = menu_start_y + i as f32 * item_height;
                if sy >= item_y && sy < item_y + item_height {
                    let game = self.game_state.as_mut().unwrap();
                    match i {
                        0 => game.active_screen = UIScreen::Overworld,
                        1 => game.exit_requested = true,
                        _ => {}
                    }
                    return;
                }
            }
        }
    }

    fn open_city_view(&mut self, city_id: fc3_core::id::CityId) {
        let game = match &mut self.game_state {
            Some(g) => g,
            None => return,
        };

        game.input.keys_held.clear();
        let player = game.viewing_player;

        // Get city data from the player view
        if game.cached_view.is_none() {
            game.cached_view = Some(game.get_view());
        }
        let view = game.cached_view.as_ref().unwrap();

        let city = match view.own_cities.iter().find(|c| c.id == city_id) {
            Some(c) => c.clone(),
            None => return,
        };

        // Query tile yields and radius from the engine
        let tile_yields = game
            .engine
            .query_city_tile_yields(player, city_id)
            .unwrap_or_default();
        let radius_tiles = game.engine.query_city_radius(city_id).unwrap_or_default();

        // Collect tile snapshots for the radius tiles
        let tile_snapshots: Vec<TileSnapshot> = radius_tiles
            .iter()
            .filter_map(|coord| {
                view.visible_tiles
                    .iter()
                    .find(|t| t.coord == *coord)
                    .cloned()
            })
            .collect();

        game.city_view_data = Some(CityViewData {
            city,
            tile_yields,
            tile_snapshots,
        });
        game.active_screen = UIScreen::CityView { city_id };
    }

    fn open_tech_popup(&mut self) {
        let game = match &mut self.game_state {
            Some(g) => g,
            None => return,
        };

        let available = game.engine.available_commands(game.viewing_player);
        let tech_cmd = available.iter().find(|cmd| {
            matches!(
                cmd,
                fc3_core::protocol::AvailableCommand::SetResearch { .. }
            )
        });

        if let Some(fc3_core::protocol::AvailableCommand::SetResearch { options }) = tech_cmd {
            if options.is_empty() {
                return;
            }

            let items: Vec<TechItem> = options
                .iter()
                .map(|opt| TechItem {
                    id: opt.id.clone(),
                    name: opt.name.clone(),
                    cost: opt.cost,
                })
                .collect();

            game.active_dialog = Some(ActiveDialog::TechSelect { options: items });
        }
    }

    fn handle_right_click(&mut self, target: TileCoord) {
        let game = match &mut self.game_state {
            Some(g) => g,
            None => return,
        };

        // Freeze game input during animations
        if game.animations.is_busy() {
            return;
        }

        let unit_id = match game.input.selected_unit {
            Some(uid) => uid,
            None => return,
        };

        // Check if current player's turn
        if game.engine.current_player() != game.viewing_player {
            return;
        }

        // Determine if there's an adjacent enemy to attack
        if game.cached_view.is_none() {
            game.cached_view = Some(game.get_view());
        }
        let view = game.cached_view.as_ref().unwrap();

        // Find the selected unit's position
        let unit_pos = match view
            .known_units
            .iter()
            .find(|u| u.id == unit_id)
            .map(|u| u.position)
        {
            Some(p) => p,
            None => {
                game.input.selected_unit = None;
                game.input.goto_mode = false;
                return;
            }
        };

        // Check if target tile has an adjacent enemy unit (for attack)
        let enemy_on_target = view
            .known_units
            .iter()
            .find(|u| u.position == target && u.owner != game.viewing_player);

        let adjacent = is_adjacent(
            unit_pos,
            target,
            view.map_width,
            view.wrap_x,
            view.map_height,
            view.wrap_y,
        );

        let cmd = if let Some(enemy) = enemy_on_target {
            if adjacent {
                Command::AttackUnit {
                    attacker: unit_id,
                    defender: enemy.id,
                }
            } else {
                // Not adjacent — move toward the enemy instead
                Command::MoveUnit {
                    unit_id,
                    destination: target,
                }
            }
        } else {
            Command::MoveUnit {
                unit_id,
                destination: target,
            }
        };

        let result = game.engine.submit_command(game.viewing_player, cmd);
        game.cached_view = None;

        // Process events into queued animations
        for event in &result.events {
            match event {
                Event::UnitMoved {
                    unit_id, from, to, ..
                } => {
                    let ut = lookup_unit_type_name(&game.engine, *unit_id);
                    game.animations.queue_move(*unit_id, *from, *to, &ut);
                }
                Event::CombatStarted { tile, .. } => {
                    game.animations.queue_combat_flash(*tile);
                }
                Event::UnitDestroyed {
                    unit_id: uid, at, ..
                } => {
                    game.animations.queue_combat_flash(*at);
                    // If our selected unit was destroyed, deselect
                    if *uid == unit_id {
                        game.input.selected_unit = None;
                        game.input.goto_mode = false;
                    }
                }
                Event::MoveInterrupted { unit_id: _, at, .. } => {
                    game.animations.queue_combat_flash(*at);
                }
                _ => {}
            }
        }

        // If there were errors, flash the target tile red
        if !result.errors.is_empty() {
            game.animations.queue_combat_flash(target);
        }

        // Defer auto-cycle until animations finish
        if let Some(uid) = game.input.selected_unit {
            let view = game.get_view();
            let unit_still_moveable = view
                .known_units
                .iter()
                .find(|u| u.id == uid)
                .map(|u| u.movement > 0)
                .unwrap_or(false);

            if !unit_still_moveable {
                game.pending_auto_cycle = true;
            }
            game.cached_view = Some(view);
        }
    }

    fn handle_key(&mut self, key: KeyCode) {
        // Component dialog consumes all keys when active
        if let Some(game) = &mut self.game_state {
            if game.active_dialog.is_some() {
                game.dialog_key_pressed = Some(key);
                return;
            }
        }

        // Layer-first input dispatch
        if let Some(game) = &mut self.game_state {
            if !game.ui_layers.is_empty() {
                if key == KeyCode::Escape {
                    game.ui_layers.pop();
                    return;
                }
                let top = game.ui_layers.last_mut().unwrap();
                if let Some(evt) = top.handle_key(key) {
                    Self::process_layer_event_static(game, evt);
                }
                return;
            }
        }

        // Toggle debug terminal with backtick/tilde
        if key == KeyCode::Backquote {
            if let (Some(game), Some(rs)) = (&mut self.game_state, &self.render_state) {
                let w = rs.renderer.config.width as f32;
                let h = rs.renderer.config.height as f32;
                game.ui_layers.push(UILayer::DebugTerminal {
                    input_buffer: String::new(),
                    output_lines: Vec::new(),
                    screen_w: w,
                    screen_h: h,
                });
            }
            return;
        }

        let screen = match &self.game_state {
            Some(g) => g.active_screen.clone(),
            None => return,
        };
        match screen {
            UIScreen::Overworld => self.handle_key_overworld(key),
            UIScreen::CityView { .. } => self.handle_key_city_view(key),
            UIScreen::MainMenu { .. } => self.handle_key_main_menu(key),
        }
    }

    fn handle_key_overworld(&mut self, key: KeyCode) {
        // Freeze game input during animations (allow Escape to cancel)
        if key != KeyCode::Escape {
            if let Some(game) = &self.game_state {
                if game.animations.is_busy() {
                    return;
                }
            }
        }
        match key {
            KeyCode::Escape => {
                if let Some(game) = &mut self.game_state {
                    if game.input.goto_mode {
                        // Cancel goto mode without deselecting the unit
                        game.input.goto_mode = false;
                    } else if game.input.selected_unit.is_some() {
                        game.input.selected_unit = None;
                    } else {
                        // Nothing selected — open main menu
                        game.input.keys_held.clear();
                        game.active_screen = UIScreen::MainMenu { selected_index: 0 };
                    }
                }
            }
            KeyCode::Space => {
                let is_auto_play = self
                    .game_state
                    .as_ref()
                    .map(|g| g.auto_play)
                    .unwrap_or(false);
                if is_auto_play {
                    if let Some(game) = &mut self.game_state {
                        game.paused = !game.paused;
                    }
                } else {
                    self.handle_skip();
                }
            }
            KeyCode::Enter | KeyCode::NumpadEnter => {
                self.handle_enter();
            }
            KeyCode::KeyF => {
                self.handle_fortify();
            }
            KeyCode::KeyG => {
                if let Some(game) = &mut self.game_state {
                    if game.input.selected_unit.is_some() {
                        game.input.goto_mode = true;
                    }
                }
            }
            key @ (KeyCode::KeyA
            | KeyCode::KeyB
            | KeyCode::KeyC
            | KeyCode::KeyD
            | KeyCode::KeyE
            | KeyCode::KeyH
            | KeyCode::KeyI
            | KeyCode::KeyJ
            | KeyCode::KeyK
            | KeyCode::KeyL
            | KeyCode::KeyM
            | KeyCode::KeyN
            | KeyCode::KeyO
            | KeyCode::KeyP
            | KeyCode::KeyQ
            | KeyCode::KeyR
            | KeyCode::KeyS
            | KeyCode::KeyT
            | KeyCode::KeyU
            | KeyCode::KeyV
            | KeyCode::KeyY
            | KeyCode::KeyZ) => {
                self.handle_action_hotkey(key);
            }
            _ => {}
        }
    }

    fn handle_key_city_view(&mut self, key: KeyCode) {
        if key == KeyCode::Escape {
            if let Some(game) = &mut self.game_state {
                game.active_screen = UIScreen::Overworld;
                game.city_view_data = None;
            }
        }
    }

    fn handle_key_main_menu(&mut self, key: KeyCode) {
        let game = match &mut self.game_state {
            Some(g) => g,
            None => return,
        };
        match key {
            KeyCode::Escape => {
                game.active_screen = UIScreen::Overworld;
            }
            KeyCode::ArrowUp => {
                if let UIScreen::MainMenu { selected_index } = &mut game.active_screen {
                    if *selected_index > 0 {
                        *selected_index -= 1;
                    }
                }
            }
            KeyCode::ArrowDown => {
                if let UIScreen::MainMenu { selected_index } = &mut game.active_screen {
                    if *selected_index < menu::MENU_ITEMS.len() - 1 {
                        *selected_index += 1;
                    }
                }
            }
            KeyCode::Enter | KeyCode::NumpadEnter => {
                if let UIScreen::MainMenu { selected_index } = &game.active_screen {
                    match *selected_index {
                        0 => {
                            // Resume Game
                            game.active_screen = UIScreen::Overworld;
                        }
                        1 => {
                            // Exit Game
                            game.exit_requested = true;
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_build_city(&mut self) {
        let game = match &mut self.game_state {
            Some(g) => g,
            None => return,
        };
        if game.auto_play || game.game_over {
            return;
        }
        let unit_id = match game.input.selected_unit {
            Some(uid) => uid,
            None => return,
        };
        if game.engine.current_player() != game.viewing_player {
            return;
        }

        // Check if build_city action is available for this unit
        let available = game.engine.available_commands(game.viewing_player);
        let can_found = available
            .iter()
            .any(|cmd| matches!(cmd, fc3_core::protocol::AvailableCommand::UnitAction { unit_id: uid, action_id, .. } if *uid == unit_id && action_id == "build_city"));
        if !can_found {
            return;
        }

        // Queue the BUILD animation before submitting the command — the settler
        // should remain visible on screen until the animation finishes.
        let unit_type = lookup_unit_type_name(&game.engine, unit_id);
        game.animations
            .queue_named_anim(unit_id, &unit_type, "BUILD", 0.4);
        game.pending_found_city = Some(unit_id);
        game.input.selected_unit = None;
        game.input.goto_mode = false;
    }

    fn dispatch_action_box(&mut self, action_id: &str) {
        match action_id {
            "__fortify" => self.handle_fortify(),
            "__skip" => self.handle_skip(),
            "__goto" => {
                if let Some(game) = &mut self.game_state {
                    if game.input.selected_unit.is_some() {
                        game.input.goto_mode = true;
                    }
                }
            }
            "build_city" => self.handle_build_city(),
            other => {
                let action_id = other.to_string();
                let game = match &mut self.game_state {
                    Some(g) => g,
                    None => return,
                };
                if game.auto_play || game.game_over {
                    return;
                }
                let unit_id = match game.input.selected_unit {
                    Some(uid) => uid,
                    None => return,
                };
                if game.engine.current_player() != game.viewing_player {
                    return;
                }
                let result = game.engine.submit_command(
                    game.viewing_player,
                    Command::PerformAction { unit_id, action_id },
                );
                game.cached_view = None;
                if result.errors.is_empty() {
                    game.animations.queue_action_pause(unit_id, 0.3);
                    game.pending_auto_cycle = true;
                }
            }
        }
    }

    fn handle_fortify(&mut self) {
        let game = match &mut self.game_state {
            Some(g) => g,
            None => return,
        };
        if game.auto_play || game.game_over {
            return;
        }
        let unit_id = match game.input.selected_unit {
            Some(uid) => uid,
            None => return,
        };
        if game.engine.current_player() != game.viewing_player {
            return;
        }

        let unit_type = lookup_unit_type_name(&game.engine, unit_id);

        game.engine
            .submit_command(game.viewing_player, Command::FortifyUnit { unit_id });
        game.cached_view = None;

        // Queue fortify animation (or fallback pause) and defer auto-cycle
        game.animations.queue_fortify(unit_id, &unit_type);
        game.pending_auto_cycle = true;
    }

    fn handle_skip(&mut self) {
        let game = match &mut self.game_state {
            Some(g) => g,
            None => return,
        };
        if game.auto_play || game.game_over {
            return;
        }
        let unit_id = match game.input.selected_unit {
            Some(uid) => uid,
            None => return,
        };
        if game.engine.current_player() != game.viewing_player {
            return;
        }

        game.engine
            .submit_command(game.viewing_player, Command::SkipUnit { unit_id });
        game.cached_view = None;

        // Queue a brief pause animation and defer auto-cycle
        game.animations.queue_action_pause(unit_id, 0.3);
        game.pending_auto_cycle = true;
    }

    fn keycode_to_letter(key: KeyCode) -> Option<char> {
        match key {
            KeyCode::KeyA => Some('a'),
            KeyCode::KeyB => Some('b'),
            KeyCode::KeyC => Some('c'),
            KeyCode::KeyD => Some('d'),
            KeyCode::KeyE => Some('e'),
            KeyCode::KeyF => Some('f'),
            KeyCode::KeyG => Some('g'),
            KeyCode::KeyH => Some('h'),
            KeyCode::KeyI => Some('i'),
            KeyCode::KeyJ => Some('j'),
            KeyCode::KeyK => Some('k'),
            KeyCode::KeyL => Some('l'),
            KeyCode::KeyM => Some('m'),
            KeyCode::KeyN => Some('n'),
            KeyCode::KeyO => Some('o'),
            KeyCode::KeyP => Some('p'),
            KeyCode::KeyQ => Some('q'),
            KeyCode::KeyR => Some('r'),
            KeyCode::KeyS => Some('s'),
            KeyCode::KeyT => Some('t'),
            KeyCode::KeyU => Some('u'),
            KeyCode::KeyV => Some('v'),
            KeyCode::KeyW => Some('w'),
            KeyCode::KeyY => Some('y'),
            KeyCode::KeyZ => Some('z'),
            _ => None,
        }
    }

    fn handle_action_hotkey(&mut self, key: KeyCode) {
        let letter = match Self::keycode_to_letter(key) {
            Some(c) => c,
            None => return,
        };

        // Check for a build_city action first (needs special handling for city view)
        let is_build_city = {
            let game = match &mut self.game_state {
                Some(g) => g,
                None => return,
            };
            if game.auto_play || game.game_over {
                return;
            }
            let unit_id = match game.input.selected_unit {
                Some(uid) => uid,
                None => return,
            };
            if game.engine.current_player() != game.viewing_player {
                return;
            }

            // Find an action for this unit with a matching hotkey
            let available = game.engine.available_commands(game.viewing_player);
            let matching_action = available.iter().find_map(|cmd| {
                if let fc3_core::protocol::AvailableCommand::UnitAction {
                    unit_id: uid,
                    action_id,
                    hotkey,
                    ..
                } = cmd
                {
                    if *uid == unit_id
                        && hotkey
                            .as_ref()
                            .is_some_and(|h| h.to_lowercase() == letter.to_string())
                    {
                        return Some(action_id.clone());
                    }
                }
                None
            });

            match matching_action {
                Some(ref id) if id == "build_city" => true,
                Some(action_id) => {
                    let result = game.engine.submit_command(
                        game.viewing_player,
                        Command::PerformAction { unit_id, action_id },
                    );
                    game.cached_view = None;
                    if result.errors.is_empty() {
                        game.animations.queue_action_pause(unit_id, 0.3);
                        game.pending_auto_cycle = true;
                    }
                    return;
                }
                None => return,
            }
        };

        if is_build_city {
            self.handle_build_city();
        }
    }

    /// Enter key handler: cycles through unhandled units, then ends the turn.
    fn handle_enter(&mut self) {
        // Check preconditions and try unit cycling
        let has_units = {
            let game = match &mut self.game_state {
                Some(g) => g,
                None => return,
            };
            if game.auto_play || game.game_over {
                return;
            }
            if game.engine.current_player() != game.viewing_player {
                return;
            }
            Self::select_next_unit(game)
        };

        if has_units {
            return;
        }

        // No unhandled units — end turn
        self.handle_end_turn();
    }

    fn handle_end_turn(&mut self) {
        let needs_tech = {
            let game = match &mut self.game_state {
                Some(g) => g,
                None => return,
            };
            if game.auto_play || game.game_over {
                return;
            }
            if game.engine.current_player() != game.viewing_player {
                return;
            }

            // Submit EndTurn for the human player
            game.input.selected_unit = None;
            game.input.goto_mode = false;
            let result = game
                .engine
                .submit_command(game.viewing_player, Command::EndTurn);
            game.cached_view = None;

            if !result.errors.is_empty() {
                return;
            }

            // Process AI turns until it's the human's turn again (or game over)
            Self::process_ai_turns(game);

            // Reset cycle index for new turn and select first unit
            game.input.unit_cycle_index = 0;
            Self::select_next_unit(game);

            // Check if tech popup needed at turn start:
            // player has science income but no research selected
            let view = game.get_view();
            let needs = view.science_per_turn > 0 && view.researching.is_none();
            game.cached_view = Some(view);
            needs
        };

        if needs_tech {
            self.open_tech_popup();
        }
    }

    /// Run all AI player turns until it's the human player's turn again.
    fn process_ai_turns(game: &mut GameState) {
        let human = game.viewing_player;
        let mut safety = 0;
        let max_iterations = 5000; // prevent infinite loop

        while game.engine.current_player() != human && !game.game_over && safety < max_iterations {
            safety += 1;

            if game.engine.is_game_over().is_some() {
                game.game_over = true;
                break;
            }

            let current = game.engine.current_player();
            if !game.engine.is_player_alive(current) {
                game.engine.submit_command(current, Command::EndTurn);
                continue;
            }

            let view = game.engine.player_view(current);
            let available = game.engine.available_commands(current);
            let agent_idx = current.0 as usize;
            let cmd = game.agents[agent_idx].decide(&view, &available);
            let is_end_turn = matches!(cmd, Command::EndTurn);

            let result = game.engine.submit_command(current, cmd);

            // Queue animations for events visible to the human player
            let human_view = if game.debug_reveal {
                game.engine.debug_view(human)
            } else {
                game.engine.player_view(human)
            };
            for event in &result.events {
                match event {
                    Event::UnitMoved {
                        unit_id, from, to, ..
                    } => {
                        // Only animate if human can see either the from or to tile
                        let visible = human_view.visible_tiles.iter().any(|t| {
                            (t.coord == *from || t.coord == *to)
                                && t.visibility == Visibility::Visible
                        });
                        if visible {
                            let ut = lookup_unit_type_name(&game.engine, *unit_id);
                            game.animations.start_move(*unit_id, *from, *to, &ut);
                        }
                    }
                    Event::CombatStarted { tile, .. } => {
                        let visible = human_view
                            .visible_tiles
                            .iter()
                            .any(|t| t.coord == *tile && t.visibility == Visibility::Visible);
                        if visible {
                            game.animations.start_combat_flash(*tile);
                        }
                    }
                    Event::UnitDestroyed { at, .. } => {
                        let visible = human_view
                            .visible_tiles
                            .iter()
                            .any(|t| t.coord == *at && t.visibility == Visibility::Visible);
                        if visible {
                            game.animations.start_combat_flash(*at);
                        }
                    }
                    _ => {}
                }
            }

            if game.engine.is_game_over().is_some() {
                game.game_over = true;
                break;
            }

            if !is_end_turn {
                continue;
            }
        }

        game.cached_view = None;
    }

    /// Process an event returned from a UI layer.
    fn process_layer_event_static(game: &mut GameState, event: LayerEvent) {
        match event {
            LayerEvent::Consumed => {}
            LayerEvent::Close => {
                game.ui_layers.pop();
            }
            LayerEvent::ProductionSelected { city_id, item } => {
                game.engine.submit_command(
                    game.viewing_player,
                    Command::SetProduction { city_id, item },
                );
                game.cached_view = None;
                game.ui_layers.pop();

                // Refresh city view data if we're in city view for this city
                if let UIScreen::CityView {
                    city_id: view_city_id,
                } = &game.active_screen
                {
                    if *view_city_id == city_id {
                        // Refresh city view data
                        let player = game.viewing_player;
                        let view = game.get_view();
                        if let Some(city) = view.own_cities.iter().find(|c| c.id == city_id) {
                            let tile_yields = game
                                .engine
                                .query_city_tile_yields(player, city_id)
                                .unwrap_or_default();
                            let radius_tiles =
                                game.engine.query_city_radius(city_id).unwrap_or_default();
                            let tile_snapshots: Vec<TileSnapshot> = radius_tiles
                                .iter()
                                .filter_map(|coord| {
                                    view.visible_tiles
                                        .iter()
                                        .find(|t| t.coord == *coord)
                                        .cloned()
                                })
                                .collect();
                            game.city_view_data = Some(CityViewData {
                                city: city.clone(),
                                tile_yields,
                                tile_snapshots,
                            });
                        }
                        game.cached_view = Some(view);
                    }
                }
            }
            LayerEvent::DebugCommand { command } => {
                Self::process_debug_command(game, &command);
            }
        }
    }

    fn process_debug_command(game: &mut GameState, command: &str) {
        let response = match command {
            "reveal" => {
                game.debug_reveal = !game.debug_reveal;
                game.cached_view = None;
                if game.debug_reveal {
                    "Map revealed.".to_string()
                } else {
                    "Fog of war restored.".to_string()
                }
            }
            "help" => "Commands: reveal, help".to_string(),
            _ => format!("Unknown command: {command}"),
        };
        if let Some(UILayer::DebugTerminal { output_lines, .. }) = game.ui_layers.last_mut() {
            output_lines.push(response);
        }
    }

    /// Select the next unhandled unit belonging to the human player, cycling
    /// through them by `unit_cycle_index`. Returns true if a unit was selected.
    fn select_next_unit(game: &mut GameState) -> bool {
        let view = game.get_view();
        let mut eligible: Vec<_> = view
            .known_units
            .iter()
            .filter(|u| {
                u.owner == game.viewing_player
                    && u.movement > 0
                    && !u.fortified
                    && !u.skipped
                    && u.current_action.is_none()
                    && u.destination.is_none()
            })
            .map(|u| (u.id, u.position))
            .collect();
        // Stable ordering by unit ID
        eligible.sort_by_key(|(id, _)| (id.index, id.generation));

        game.input.goto_mode = false;
        game.cached_view = Some(view);

        if eligible.is_empty() {
            game.input.selected_unit = None;
            return false;
        }

        let idx = game.input.unit_cycle_index % eligible.len();
        let (uid, pos) = eligible[idx];
        game.input.selected_unit = Some(uid);
        game.camera.center_on(pos);
        game.input.unit_cycle_index = idx + 1;
        true
    }

    fn update_and_render(&mut self) {
        // --- Frame-common update ---
        let game = match &mut self.game_state {
            Some(g) => g,
            None => return,
        };

        let now = Instant::now();
        let dt = now.duration_since(game.last_frame).as_secs_f32();
        game.last_frame = now;

        game.frame_count += 1;
        game.fps_timer += dt;
        if game.fps_timer >= 1.0 {
            game.current_fps = game.frame_count as f32 / game.fps_timer;
            game.frame_count = 0;
            game.fps_timer = 0.0;
        }

        // Camera pan from held keys (only in overworld)
        if matches!(game.active_screen, UIScreen::Overworld) {
            let pan_speed = 15.0;
            let mut dx = 0.0f32;
            let mut dy = 0.0f32;
            if game.input.keys_held.contains(&KeyCode::ArrowLeft)
                || game.input.keys_held.contains(&KeyCode::KeyA)
            {
                dx -= pan_speed * dt;
            }
            if game.input.keys_held.contains(&KeyCode::ArrowRight)
                || game.input.keys_held.contains(&KeyCode::KeyD)
            {
                dx += pan_speed * dt;
            }
            if game.input.keys_held.contains(&KeyCode::ArrowUp)
                || game.input.keys_held.contains(&KeyCode::KeyW)
            {
                dy -= pan_speed * dt;
            }
            if game.input.keys_held.contains(&KeyCode::ArrowDown)
                || game.input.keys_held.contains(&KeyCode::KeyS)
            {
                dy += pan_speed * dt;
            }
            if dx != 0.0 || dy != 0.0 {
                game.camera
                    .pan(-dx * game.camera.zoom, -dy * game.camera.zoom);
            }
        }

        // AI step timer
        if game.auto_play {
            game.ai_timer += dt;
            if game.ai_timer >= AI_STEP_INTERVAL {
                game.ai_timer -= AI_STEP_INTERVAL;
                Self::step_ai(game);
            }
        }

        game.animations.update(dt);

        // Auto-cycle when queued animations finish
        if !game.animations.is_busy() && game.pending_auto_cycle {
            game.pending_auto_cycle = false;
            Self::select_next_unit(game);
        }

        // When the BUILD animation finishes, submit the deferred build_city command
        // and open the city view.
        let deferred_found = if !game.animations.is_busy() {
            game.pending_found_city.take()
        } else {
            None
        };

        // --- Screen-specific rendering ---
        let screen = game.active_screen.clone();
        match screen {
            UIScreen::Overworld => self.render_overworld(),
            UIScreen::CityView { city_id } => self.render_city_view(city_id),
            UIScreen::MainMenu { selected_index } => {
                // Render overworld as background, then overlay the menu
                self.render_overworld_and_menu(selected_index);
            }
        }

        // Submit the deferred build_city command now that the animation is done
        if let Some(unit_id) = deferred_found {
            let game = match &mut self.game_state {
                Some(g) => g,
                None => return,
            };
            let result = game.engine.submit_command(
                game.viewing_player,
                Command::PerformAction {
                    unit_id,
                    action_id: "build_city".to_string(),
                },
            );
            game.cached_view = None;
            if let Some(cid) = result.events.iter().find_map(|e| match e {
                Event::CityFounded { city_id, .. } => Some(*city_id),
                _ => None,
            }) {
                self.open_city_view(cid);
            }
        }
    }

    fn render_overworld(&mut self) {
        let (game, rs) = match (&mut self.game_state, &mut self.render_state) {
            (Some(g), Some(r)) => (g, r),
            _ => return,
        };

        if game.cached_view.is_none() {
            game.cached_view = Some(game.get_view());
        }

        let (path_lines, path_turns_info) = if !game.auto_play {
            Self::compute_path_preview(game)
        } else {
            (Vec::new(), None)
        };

        let flash_tiles = game.animations.combat_flash_tiles();
        let view = game.cached_view.as_ref().unwrap();
        let wrap_offsets = game.camera.wrap_offsets();

        let pulse_time = game.frame_count as f32 * 0.016;
        let selection_pulse = 0.35 + 0.15 * (pulse_time * 4.0).sin();

        let selected_tile = game.input.selected_unit.and_then(|uid| {
            view.known_units
                .iter()
                .find(|u| u.id == uid)
                .map(|u| u.position)
        });

        let overlays = TileOverlays {
            hovered_tile: game.input.hovered_tile,
            selected_tile,
            selection_pulse,
        };

        let tile_instances = apply_wrap_tile(
            &build_tile_instances(view, &flash_tiles, &overlays, &rs.renderer.terrain_atlas),
            &wrap_offsets,
        );
        let path_line_instances = apply_wrap_path_line(&path_lines, &wrap_offsets);

        // Build selection circle instance for the selected unit's tile
        let selection_circle_instances: Vec<SelectionCircleInstance> =
            if let Some(tile) = selected_tile {
                wrap_offsets
                    .iter()
                    .map(|off| SelectionCircleInstance {
                        position: [tile.x as f32 + off, tile.y as f32],
                        alpha: selection_pulse,
                        _pad: 0.0,
                    })
                    .collect()
            } else {
                Vec::new()
            };

        let (raw_units, raw_batches) = build_unit_instances(
            view,
            &game.animations,
            &rs.renderer.unit_atlas,
            game.input.selected_unit,
            &game.input.tile_stack_index,
            game.viewing_player,
        );
        let (unit_instances, unit_batches) =
            apply_wrap_unit_batched(&raw_units, &raw_batches, &wrap_offsets);
        let city_rect = rs.renderer.city_atlas.get_rect(0, 0); // ancient era town
        let city_instances = apply_wrap_city(&build_city_instances(view, city_rect), &wrap_offsets);

        let camera_uniform = game.camera.to_uniform();
        let turn = game.engine.current_turn();
        let viewing_player = game.viewing_player;
        let game_over = game.game_over;
        let paused = game.paused;
        let fps = game.current_fps;
        let winner = game.engine.is_game_over();
        let camera_center_x = game.camera.center_x;
        let camera_center_y = game.camera.center_y;
        let camera_zoom = game.camera.zoom;
        let map_width_f = view.map_width as f32;
        let map_height_f = view.map_height as f32;

        // Build action hints and action boxes for selected unit from available commands
        let ui_atlas_w = rs.renderer.ui_atlas.width;
        let ui_atlas_h = rs.renderer.ui_atlas.height;
        let ui_layer_h = rs.renderer.ui_atlas_layer_height;
        game.action_boxes.clear();
        if let Some(uid) = game.input.selected_unit {
            if !game.auto_play && !game.game_over {
                let available = game.engine.available_commands(viewing_player);
                let mut has_fortify = false;
                let mut has_skip = false;
                let mut has_move = false;
                for cmd in &available {
                    match cmd {
                        fc3_core::protocol::AvailableCommand::Move { unit_id, .. }
                            if *unit_id == uid =>
                        {
                            has_move = true;
                        }
                        fc3_core::protocol::AvailableCommand::Fortify { unit_id }
                            if *unit_id == uid =>
                        {
                            has_fortify = true;
                        }
                        fc3_core::protocol::AvailableCommand::Skip { unit_id }
                            if *unit_id == uid =>
                        {
                            has_skip = true;
                        }
                        fc3_core::protocol::AvailableCommand::UnitAction {
                            unit_id,
                            name,
                            hotkey,
                            action_id,
                            icon_atlas_pos,
                            ..
                        } if *unit_id == uid => {
                            let (col, row) = icon_atlas_pos.unwrap_or((7, 4));
                            game.action_boxes.push((
                                build_action_uibox(
                                    0.0,
                                    0.0,
                                    icon_uvs_from_pos(col, row, ui_atlas_w, ui_atlas_h, ui_layer_h),
                                ),
                                action_id.clone(),
                            ));
                        }
                        _ => {}
                    }
                }
                // Prepend fortify and skip buttons
                if has_skip {
                    game.action_boxes.insert(
                        0,
                        (
                            build_action_uibox(
                                0.0,
                                0.0,
                                icon_uvs_from_pos(0, 0, ui_atlas_w, ui_atlas_h, ui_layer_h),
                            ),
                            "__skip".to_string(),
                        ),
                    );
                }
                if has_fortify {
                    game.action_boxes.insert(
                        0,
                        (
                            build_action_uibox(
                                0.0,
                                0.0,
                                icon_uvs_from_pos(2, 0, ui_atlas_w, ui_atlas_h, ui_layer_h),
                            ),
                            "__fortify".to_string(),
                        ),
                    );
                }
                if has_move {
                    game.action_boxes.insert(
                        0,
                        (
                            build_action_uibox(
                                0.0,
                                0.0,
                                icon_uvs_from_pos(4, 0, ui_atlas_w, ui_atlas_h, ui_layer_h),
                            ),
                            "__goto".to_string(),
                        ),
                    );
                }
            }
        }
        // Position action boxes centered at the bottom of the screen
        {
            let vw = rs.renderer.config.width as f32;
            let vh = rs.renderer.config.height as f32;
            let btn_size = 32.0;
            let gap = 4.0;
            let total_w = game.action_boxes.len() as f32 * (btn_size + gap) - gap;
            let start_x = (vw - total_w) / 2.0;
            let btn_y = vh - btn_size - 34.0; // above the bottom HUD text
            for (i, (b, _)) in game.action_boxes.iter_mut().enumerate() {
                b.x = start_x + i as f32 * (btn_size + gap);
                b.y = btn_y;
            }
        }

        // Right box: top text (unit info) — 3 lines
        let mut right_box_unit_lines: Vec<String> = game
            .input
            .selected_unit
            .and_then(|uid| {
                let u = view.known_units.iter().find(|u| u.id == uid)?;
                let terrain_name = view
                    .visible_tiles
                    .iter()
                    .find(|t| t.coord == u.position)
                    .map(|t| {
                        let veg = match t.vegetation {
                            Vegetation::Forest => "Forest ",
                            Vegetation::Jungle => "Jungle ",
                            Vegetation::None => "",
                        };
                        format!("{}{:?}", veg, t.terrain)
                    })
                    .unwrap_or_default();
                Some(vec![
                    u.unit_type_name.clone(),
                    "Regular".to_string(),
                    format!(
                        "{}.{}.{}/{}",
                        u.attack,
                        u.defense,
                        u.movement / 3,
                        u.max_movement / 3
                    ),
                    terrain_name,
                ])
            })
            .unwrap_or_default();

        // Right box: bottom text (civ, gold, research) — 3 lines
        let civ_name = view.civ_name.as_deref().unwrap_or("Unknown");
        let gold_delta = view.gold_per_turn;
        let gold_delta_str = if gold_delta >= 0 {
            format!("+{}", gold_delta)
        } else {
            format!("{}", gold_delta)
        };
        let research_line = match (&view.researching_name, view.research_turns_left) {
            (Some(name), Some(turns)) if turns >= 0 => {
                format!("{} ({} turns)", name, turns)
            }
            (Some(name), _) => name.clone(),
            (None, _) => String::new(),
        };
        let right_box_bottom_lines = vec![
            format!("{} - Despotism", civ_name),
            format!("{} Gold ({} per turn)", view.gold, gold_delta_str),
            research_line,
        ];
        let auto_play = game.auto_play;

        let can_end_turn = if auto_play {
            false
        } else {
            let has_unhandled = view.known_units.iter().any(|u| {
                u.owner == viewing_player
                    && u.movement > 0
                    && !u.fortified
                    && !u.skipped
                    && u.current_action.is_none()
            });
            let has_idle_city = view.own_cities.iter().any(|c| c.producing.is_none());
            let needs_research = view.researching.is_none() && !view.own_cities.is_empty();
            !has_unhandled && !has_idle_city && !needs_research
        };
        if can_end_turn && right_box_unit_lines.is_empty() {
            right_box_unit_lines.push("ENTER for next turn".to_string());
        }

        // Compute screen position for path turns label
        let path_turns_screen: Option<(f32, f32, String)> =
            path_turns_info.map(|(coord, turns)| {
                let lx = coord.x as f32 + 0.5;
                let ly = coord.y as f32 + 0.5;
                let display_y = camera::adjust_y_for_wrap(lx, ly, map_height_f, view.wrap_y);
                let (ix, iy) = logical_to_iso(lx, display_y);
                (ix, iy, turns.to_string())
            });

        let (mut layer_rects, mut layer_texts) = build_layer_visuals(&game.ui_layers);

        // Build component dialog (if active)
        let width = rs.renderer.config.width;
        let height = rs.renderer.config.height;
        let vw = width as f32;
        let vh = height as f32;

        // Build city banner data (screen positions + text lines)
        let mut city_banners: Vec<CityBannerInfo> = Vec::new();
        for c in view.own_cities.iter().chain(view.known_cities.iter()) {
            let cx = c.position.x as f32 + 0.5;
            let cy = c.position.y as f32 + 0.5;
            let color = player_color(c.owner);

            let turns_to_growth = match (c.food_per_turn, c.food_stockpile, c.food_growth_threshold)
            {
                (Some(fpt), Some(fs), Some(fgt)) if fpt > 0 => Some(((fgt - fs) + fpt - 1) / fpt),
                _ => None,
            };
            let turns_to_production =
                match (c.shields_per_turn, c.shield_stockpile, c.production_cost) {
                    (Some(spt), Some(ss), Some(pc)) if spt > 0 => Some(((pc - ss) + spt - 1) / spt),
                    _ => None,
                };

            let line1 = match turns_to_growth {
                Some(t) => format!("{}: {}", c.name, t),
                None => c.name.clone(),
            };
            let line2 = match (&c.producing, turns_to_production) {
                (Some(prod), Some(t)) => format!("{}: {}", prod, t),
                (Some(prod), None) => prod.clone(),
                _ => String::new(),
            };
            let text_w =
                line1.len().max(line2.len().max(1)) as f32 * BANNER_CHAR_W + BANNER_TEXT_PAD;

            for off in &wrap_offsets {
                let lx = cx + off;
                let display_y = camera::adjust_y_for_wrap(lx, cy, map_height_f, view.wrap_y);
                let (ix, iy) = logical_to_iso(lx, display_y);
                let screen_x = (ix - camera_center_x) * camera_zoom + vw / 2.0;
                let screen_y = (iy - camera_center_y) * camera_zoom + vh / 2.0;

                if screen_x < -200.0
                    || screen_x > vw + 200.0
                    || screen_y < -50.0
                    || screen_y > vh + 50.0
                {
                    continue;
                }

                city_banners.push(CityBannerInfo {
                    screen_x,
                    screen_y,
                    pop: c.population,
                    color,
                    line1: line1.clone(),
                    line2: line2.clone(),
                    text_w,
                });
            }
        }

        let mut dialog_nine_slices: Vec<UITexturedRectInstance> = Vec::new();
        let mut advisor_rects: Vec<UITexturedRectInstance> = Vec::new();
        // Deferred action from component dialog (processed after view borrow is dropped)
        let mut deferred_tech_select: Option<String> = None;
        let mut deferred_dialog_close = false;
        if game.active_dialog.is_some() {
            let mouse_pos = game.input.mouse_screen_pos;
            let mouse_clicked = game.dialog_mouse_clicked;
            let key_pressed = game.dialog_key_pressed.take();
            let scroll_delta = game.dialog_scroll_delta;
            game.dialog_mouse_clicked = false;
            game.dialog_scroll_delta = 0.0;

            let nine_slice_id = rs.renderer.dialog_nine_slice_id;
            let nine_slice_tile = nine_slice_id.and_then(|id| {
                rs.renderer
                    .nine_slice_registry
                    .get(id)
                    .map(|img| (img.sizes[1], img.sizes[4]))
            });

            // Compute advisor UV rect if the atlas is loaded
            let advisor_uv = rs.renderer.advisor_atlas.as_ref().map(|atlas| {
                advisor::advisor_uv_rect(
                    advisor::Age::Ancient,
                    advisor::Emotion::Happy,
                    atlas.width,
                    atlas.height,
                )
            });

            let dialog_action = match &game.active_dialog {
                Some(ActiveDialog::TechSelect { options }) => {
                    let props = TechDialogProps {
                        options: options
                            .iter()
                            .map(|o| TechItem {
                                id: o.id.clone(),
                                name: o.name.clone(),
                                cost: o.cost,
                            })
                            .collect(),
                        nine_slice_id,
                        nine_slice_tile,
                        advisor_uv,
                    };
                    let mut ctx = BuildCtx::new(
                        &mut game.component_state,
                        mouse_pos,
                        mouse_clicked,
                        key_pressed,
                        scroll_delta,
                        vw,
                        vh,
                    );
                    let (element, action) = ctx.component::<TechDialog>(&props);
                    let mut flat = FlattenedUI::new();
                    flatten(
                        &element,
                        0.0,
                        0.0,
                        [vw, vh],
                        &rs.renderer.nine_slice_registry,
                        &mut flat,
                    );
                    layer_rects.extend(flat.rects);
                    layer_texts.extend(flat.texts);
                    dialog_nine_slices = flat.textured_rects;
                    advisor_rects = flat.image_rects;
                    action
                }
                None => None,
            };

            // Defer dialog action processing (view borrow still active)
            if let Some(action) = dialog_action {
                match action {
                    TechDialogAction::SelectTech { tech_id } => {
                        deferred_tech_select = Some(tech_id);
                    }
                    TechDialogAction::Close => {
                        deferred_dialog_close = true;
                    }
                }
            }
        }

        rs.renderer.update_camera(&camera_uniform);

        rs.text_state
            .viewport
            .update(&rs.renderer.queue, Resolution { width, height });

        rs.renderer.upload_tiles(&tile_instances);
        rs.renderer.upload_path_lines(&path_line_instances);
        rs.renderer
            .upload_selection_circles(&selection_circle_instances);
        rs.renderer.upload_units(&unit_instances, unit_batches);
        rs.renderer.upload_cities(&city_instances);

        // Upload dialog 9-slice quads and advisor portrait
        rs.renderer.upload_dialog_nine_slices(&dialog_nine_slices);
        rs.renderer.upload_advisor_rects(&advisor_rects);

        // Build city banner background rects
        let mut city_banner_rects: Vec<UIRectInstance> = Vec::new();
        for b in &city_banners {
            let total_w = BANNER_POP_W + b.text_w + BANNER_RIGHT_W + BANNER_BORDER * 2.0;
            let total_h = BANNER_INNER_H + BANNER_BORDER * 2.0;
            let bx = b.screen_x - total_w / 2.0;
            let by = b.screen_y + BANNER_Y_OFFSET;

            // Outer border (player color)
            city_banner_rects.push(UIRectInstance {
                rect: [bx, by, total_w, total_h],
                color: [b.color[0], b.color[1], b.color[2], 1.0],
                viewport: [vw, vh],
            });
            // Left pop square (darker shade)
            city_banner_rects.push(UIRectInstance {
                rect: [
                    bx + BANNER_BORDER,
                    by + BANNER_BORDER,
                    BANNER_POP_W,
                    BANNER_INNER_H,
                ],
                color: [b.color[0] * 0.6, b.color[1] * 0.6, b.color[2] * 0.6, 1.0],
                viewport: [vw, vh],
            });
            // Middle text bg (semi-transparent black)
            city_banner_rects.push(UIRectInstance {
                rect: [
                    bx + BANNER_BORDER + BANNER_POP_W,
                    by + BANNER_BORDER,
                    b.text_w,
                    BANNER_INNER_H,
                ],
                color: [0.0, 0.0, 0.0, 0.7],
                viewport: [vw, vh],
            });
            // Right square (darker shade)
            city_banner_rects.push(UIRectInstance {
                rect: [
                    bx + BANNER_BORDER + BANNER_POP_W + b.text_w,
                    by + BANNER_BORDER,
                    BANNER_RIGHT_W,
                    BANNER_INNER_H,
                ],
                color: [b.color[0] * 0.6, b.color[1] * 0.6, b.color[2] * 0.6, 1.0],
                viewport: [vw, vh],
            });
        }

        // Upload UI layer rects + city banner rects
        let mut ui_rect_instances = rects_to_instances(&layer_rects, vw, vh);
        ui_rect_instances.extend(city_banner_rects);
        rs.renderer.upload_ui_rects(&ui_rect_instances);

        // Upload UI chrome (box left/right)
        rs.renderer.upload_ui_chrome(vw, vh);

        // Upload action box textured rects
        let textured_rect_instances: Vec<UITexturedRectInstance> = game
            .action_boxes
            .iter()
            .enumerate()
            .map(|(i, (b, _))| {
                let state = if game.active_box_index == Some(i) {
                    BoxState::Active
                } else if game.hovered_box_index == Some(i) {
                    BoxState::Hovered
                } else {
                    BoxState::Default
                };
                b.to_instance(&state, vw, vh)
            })
            .collect();
        rs.renderer
            .upload_ui_textured_rects(&textured_rect_instances);

        // Upload menu button textured rects
        let menu_instances: Vec<UITexturedRectInstance> = game
            .menu_boxes
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let state = if game.active_menu_index == Some(i) {
                    BoxState::Active
                } else if game.hovered_menu_index == Some(i) {
                    BoxState::Hovered
                } else {
                    BoxState::Default
                };
                b.to_instance(&state, vw, vh)
            })
            .collect();
        rs.renderer.upload_menu_buttons(&menu_instances);

        // Minimap
        let minimap_w = 200.0f32;
        let minimap_h = 125.0f32;
        let minimap_margin = 10.0;
        let minimap_x = minimap_margin;
        let minimap_y = rs.renderer.config.height as f32 - minimap_h - minimap_margin;
        let minimap_rect = [minimap_x, minimap_y, minimap_w, minimap_h];

        let minimap_tile_instances = build_minimap_tiles(view);
        rs.renderer.upload_minimap_tiles(&minimap_tile_instances);

        // Use adaptive extent: zoom into revealed area when it's smaller than the full map
        let (iso_map_cx, iso_map_cy, iso_map_w, iso_map_h) =
            minimap_visible_extent(view, map_width_f, map_height_f, view.wrap_y)
                .unwrap_or_else(|| minimap_iso_extent(map_width_f, map_height_f, view.wrap_y));
        let minimap_zoom = (minimap_w / iso_map_w).min(minimap_h / iso_map_h);
        let minimap_camera = CameraUniform {
            center: [iso_map_cx, iso_map_cy],
            zoom: minimap_zoom,
            aspect: minimap_w / minimap_h,
            viewport_w: minimap_w,
            viewport_h: minimap_h,
            map_height: map_height_f,
            wrap_y: if view.wrap_y { 1.0 } else { 0.0 },
        };
        rs.renderer.update_minimap_camera(&minimap_camera);

        // Viewport indicator on minimap
        let viewport_overlay = build_minimap_viewport_indicator(
            &game.camera,
            [iso_map_cx, iso_map_cy],
            minimap_zoom,
            minimap_w,
            minimap_h,
        );
        rs.renderer.upload_minimap_overlay(&viewport_overlay);

        let width_f = width as f32;
        let height_f = height as f32;
        let hud_text = if game_over {
            let w = winner.map(|p| p.0).unwrap_or(0);
            format!("GAME OVER — Player {} wins!", w)
        } else if auto_play {
            format!("Turn {} — Player {}", turn, viewing_player.0)
        } else {
            format!("Turn {}", turn)
        };

        {
            let ts = &mut rs.text_state;
            let ui_family = match &ts.ui_font_family {
                Some(name) => Family::Name(name),
                None => Family::Monospace,
            };

            let mut hud_buffer = TextBuffer::new(&mut ts.font_system, Metrics::new(18.0, 22.0));
            hud_buffer.set_size(&mut ts.font_system, Some(width_f), Some(30.0));
            hud_buffer.set_text(
                &mut ts.font_system,
                &hud_text,
                &Attrs::new().family(ui_family),
                Shaping::Basic,
                None,
            );
            hud_buffer.shape_until_scroll(&mut ts.font_system, false);

            // Right box text buffers
            let box_right_w = rs.renderer.box_right_size[0];
            let box_right_h = rs.renderer.box_right_size[1];
            let box_right_x = width_f - box_right_w;
            let box_right_y = height_f - box_right_h;
            let box_text_w = box_right_w - 60.0; // padding
            let box_font_size = 12.0;
            let box_line_height = 14.0;

            // Top-right: unit info (right-aligned)
            let mut rb_top_bufs: Vec<TextBuffer> = Vec::new();
            for line in &right_box_unit_lines {
                let mut buf = TextBuffer::new(
                    &mut ts.font_system,
                    Metrics::new(box_font_size, box_line_height),
                );
                buf.set_size(
                    &mut ts.font_system,
                    Some(box_text_w),
                    Some(box_line_height + 2.0),
                );
                buf.set_text(
                    &mut ts.font_system,
                    line,
                    &Attrs::new().family(ui_family),
                    Shaping::Basic,
                    Some(glyphon::cosmic_text::Align::Right),
                );
                buf.shape_until_scroll(&mut ts.font_system, false);
                rb_top_bufs.push(buf);
            }

            // Bottom-center: civ/gold/research (center-aligned)
            let mut rb_bot_bufs: Vec<TextBuffer> = Vec::new();
            for line in &right_box_bottom_lines {
                if line.is_empty() {
                    continue;
                }
                let mut buf = TextBuffer::new(
                    &mut ts.font_system,
                    Metrics::new(box_font_size, box_line_height),
                );
                buf.set_size(
                    &mut ts.font_system,
                    Some(box_text_w),
                    Some(box_line_height + 2.0),
                );
                buf.set_text(
                    &mut ts.font_system,
                    line,
                    &Attrs::new().family(ui_family),
                    Shaping::Basic,
                    Some(glyphon::cosmic_text::Align::Center),
                );
                buf.shape_until_scroll(&mut ts.font_system, false);
                rb_bot_bufs.push(buf);
            }

            let mut all_text_areas = vec![TextArea {
                buffer: &hud_buffer,
                left: 10.0,
                top: 5.0,
                scale: 1.0,
                bounds: TextBounds {
                    left: 0,
                    top: 0,
                    right: width_f as i32,
                    bottom: height_f as i32,
                },
                default_color: TextColor::rgb(240, 240, 240),
                custom_glyphs: &[],
            }];

            // Right box: top text (unit info, right-aligned, top-right of box)
            for (i, buf) in rb_top_bufs.iter().enumerate() {
                all_text_areas.push(TextArea {
                    buffer: buf,
                    left: box_right_x + 30.0,
                    top: box_right_y + 20.0 + i as f32 * box_line_height,
                    scale: 1.0,
                    bounds: TextBounds {
                        left: 0,
                        top: 0,
                        right: width_f as i32,
                        bottom: height_f as i32,
                    },
                    default_color: TextColor::rgb(0, 0, 0),
                    custom_glyphs: &[],
                });
            }

            // Right box: bottom text (civ/gold/research, center-aligned, bottom of box)
            let bot_count = rb_bot_bufs.len();
            let bot_block_h = bot_count as f32 * box_line_height;
            let bot_start_y = box_right_y + box_right_h - bot_block_h - 14.0;
            for (i, buf) in rb_bot_bufs.iter().enumerate() {
                all_text_areas.push(TextArea {
                    buffer: buf,
                    left: box_right_x + 30.0,
                    top: bot_start_y + i as f32 * box_line_height,
                    scale: 1.0,
                    bounds: TextBounds {
                        left: 0,
                        top: 0,
                        right: width_f as i32,
                        bottom: height_f as i32,
                    },
                    default_color: TextColor::rgb(0, 0, 0),
                    custom_glyphs: &[],
                });
            }

            // City banner text: pop number, line1 (name:growth), line2 (production:turns)
            let mut city_pop_bufs: Vec<(TextBuffer, f32, f32)> = Vec::new();
            let mut city_line1_bufs: Vec<(TextBuffer, f32, f32)> = Vec::new();
            let mut city_line2_bufs: Vec<(TextBuffer, f32, f32)> = Vec::new();

            for b in &city_banners {
                let total_w = BANNER_POP_W + b.text_w + BANNER_RIGHT_W + BANNER_BORDER * 2.0;
                let bx = b.screen_x - total_w / 2.0;
                let by = b.screen_y + BANNER_Y_OFFSET;

                // Population number (centered in left square)
                let pop_str = b.pop.to_string();
                let mut pop_buf = TextBuffer::new(
                    &mut ts.font_system,
                    Metrics::new(BANNER_POP_FONT, BANNER_INNER_H),
                );
                pop_buf.set_size(
                    &mut ts.font_system,
                    Some(BANNER_POP_W),
                    Some(BANNER_INNER_H),
                );
                pop_buf.set_text(
                    &mut ts.font_system,
                    &pop_str,
                    &Attrs::new().family(ui_family),
                    Shaping::Basic,
                    Some(glyphon::cosmic_text::Align::Center),
                );
                pop_buf.shape_until_scroll(&mut ts.font_system, false);
                city_pop_bufs.push((pop_buf, bx + BANNER_BORDER, by + BANNER_BORDER));

                // Line 1: city name : growth turns
                let text_x = bx + BANNER_BORDER + BANNER_POP_W + 4.0;
                let text_area_w = b.text_w - 8.0;
                let mut buf1 = TextBuffer::new(
                    &mut ts.font_system,
                    Metrics::new(BANNER_FONT_SIZE, BANNER_LINE_H),
                );
                buf1.set_size(
                    &mut ts.font_system,
                    Some(text_area_w),
                    Some(BANNER_LINE_H + 2.0),
                );
                buf1.set_text(
                    &mut ts.font_system,
                    &b.line1,
                    &Attrs::new().family(ui_family),
                    Shaping::Basic,
                    None,
                );
                buf1.shape_until_scroll(&mut ts.font_system, false);
                city_line1_bufs.push((buf1, text_x, by + BANNER_BORDER + 1.0));

                // Line 2: production : turns
                if !b.line2.is_empty() {
                    let mut buf2 = TextBuffer::new(
                        &mut ts.font_system,
                        Metrics::new(BANNER_FONT_SIZE, BANNER_LINE_H),
                    );
                    buf2.set_size(
                        &mut ts.font_system,
                        Some(text_area_w),
                        Some(BANNER_LINE_H + 2.0),
                    );
                    buf2.set_text(
                        &mut ts.font_system,
                        &b.line2,
                        &Attrs::new().family(ui_family),
                        Shaping::Basic,
                        None,
                    );
                    buf2.shape_until_scroll(&mut ts.font_system, false);
                    city_line2_bufs.push((buf2, text_x, by + BANNER_BORDER + 1.0 + BANNER_LINE_H));
                }
            }

            let text_bounds = TextBounds {
                left: 0,
                top: 0,
                right: width_f as i32,
                bottom: height_f as i32,
            };
            for (buf, x, y) in &city_pop_bufs {
                all_text_areas.push(TextArea {
                    buffer: buf,
                    left: *x,
                    top: *y,
                    scale: 1.0,
                    bounds: text_bounds,
                    default_color: TextColor::rgb(255, 255, 255),
                    custom_glyphs: &[],
                });
            }
            for (buf, x, y) in &city_line1_bufs {
                all_text_areas.push(TextArea {
                    buffer: buf,
                    left: *x,
                    top: *y,
                    scale: 1.0,
                    bounds: text_bounds,
                    default_color: TextColor::rgb(255, 255, 255),
                    custom_glyphs: &[],
                });
            }
            for (buf, x, y) in &city_line2_bufs {
                all_text_areas.push(TextArea {
                    buffer: buf,
                    left: *x,
                    top: *y,
                    scale: 1.0,
                    bounds: text_bounds,
                    default_color: TextColor::rgb(255, 255, 255),
                    custom_glyphs: &[],
                });
            }

            // Path turns label on hovered destination tile
            let mut turns_buffer: Option<TextBuffer> = None;
            let mut turns_screen = (0.0f32, 0.0f32, 0.0f32);
            if let Some((ix, iy, ref label)) = path_turns_screen {
                let sx = (ix - camera_center_x) * camera_zoom + width_f / 2.0;
                let sy = (iy - camera_center_y) * camera_zoom + height_f / 2.0;
                let label_width = label.len() as f32 * 10.0 + 8.0;
                let mut buf = TextBuffer::new(&mut ts.font_system, Metrics::new(16.0, 18.0));
                buf.set_size(&mut ts.font_system, Some(label_width), Some(20.0));
                buf.set_text(
                    &mut ts.font_system,
                    label,
                    &Attrs::new().family(ui_family),
                    Shaping::Basic,
                    None,
                );
                buf.shape_until_scroll(&mut ts.font_system, false);
                turns_screen = (sx, sy, label_width);
                turns_buffer = Some(buf);
            }
            if let Some(ref buf) = turns_buffer {
                let (sx, sy, lw) = turns_screen;
                all_text_areas.push(TextArea {
                    buffer: buf,
                    left: sx - lw / 2.0,
                    top: sy - 9.0,
                    scale: 1.0,
                    bounds: TextBounds {
                        left: 0,
                        top: 0,
                        right: width_f as i32,
                        bottom: height_f as i32,
                    },
                    default_color: TextColor::rgb(255, 255, 255),
                    custom_glyphs: &[],
                });
            }

            // Layer text elements
            let mut layer_buffers: Vec<TextBuffer> = Vec::new();
            for te in &layer_texts {
                let mut buf = TextBuffer::new(
                    &mut ts.font_system,
                    Metrics::new(te.font_size, te.line_height),
                );
                buf.set_size(
                    &mut ts.font_system,
                    Some(te.max_width),
                    Some(te.line_height + 4.0),
                );
                buf.set_text(
                    &mut ts.font_system,
                    &te.text,
                    &Attrs::new().family(ui_family),
                    Shaping::Basic,
                    None,
                );
                buf.shape_until_scroll(&mut ts.font_system, false);
                layer_buffers.push(buf);
            }

            for (i, buf) in layer_buffers.iter().enumerate() {
                let te = &layer_texts[i];
                all_text_areas.push(TextArea {
                    buffer: buf,
                    left: te.x,
                    top: te.y,
                    scale: 1.0,
                    bounds: TextBounds {
                        left: 0,
                        top: 0,
                        right: width_f as i32,
                        bottom: height_f as i32,
                    },
                    default_color: TextColor::rgb(te.color[0], te.color[1], te.color[2]),
                    custom_glyphs: &[],
                });
            }

            let _ = ts.text_renderer.prepare(
                &rs.renderer.device,
                &rs.renderer.queue,
                &mut ts.font_system,
                &mut ts.atlas,
                &ts.viewport,
                all_text_areas,
                &mut ts.swash_cache,
            );
        }

        let text_info = Some((
            &rs.text_state.text_renderer,
            &rs.text_state.atlas,
            &rs.text_state.viewport,
        ));
        match rs
            .renderer
            .render_with_minimap(text_info, Some(minimap_rect))
        {
            Ok(()) => {}
            Err(wgpu::SurfaceError::Lost) => {
                let size = rs.renderer.window.inner_size();
                rs.renderer.resize(size);
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                log::error!("Out of GPU memory");
            }
            Err(e) => {
                log::warn!("Surface error: {:?}", e);
            }
        }

        // Process deferred component dialog actions (after view borrow is released)
        if let Some(tech_id) = deferred_tech_select {
            game.engine
                .submit_command(game.viewing_player, Command::SetResearch { tech_id });
            game.cached_view = None;
            game.active_dialog = None;
        } else if deferred_dialog_close {
            game.active_dialog = None;
        }

        let status = if game_over {
            let w = winner.map(|p| p.0).unwrap_or(0);
            format!("GAME OVER — Player {} wins!", w)
        } else if paused {
            "PAUSED".to_string()
        } else {
            format!("Turn {} — Player {}", turn, viewing_player.0)
        };
        rs.renderer
            .window
            .set_title(&format!("FreeC3 — {} — {:.0} FPS", status, fps));
    }

    fn render_overworld_and_menu(&mut self, selected_index: usize) {
        // First render the overworld as background
        let (game, rs) = match (&mut self.game_state, &mut self.render_state) {
            (Some(g), Some(r)) => (g, r),
            _ => return,
        };

        if game.cached_view.is_none() {
            game.cached_view = Some(game.get_view());
        }

        let view = game.cached_view.as_ref().unwrap();
        let wrap_offsets = game.camera.wrap_offsets();

        let overlays = TileOverlays {
            hovered_tile: None,
            selected_tile: None,
            selection_pulse: 0.0,
        };

        let tile_instances = apply_wrap_tile(
            &build_tile_instances(view, &[], &overlays, &rs.renderer.terrain_atlas),
            &wrap_offsets,
        );
        let (raw_units, raw_batches) = build_unit_instances(
            view,
            &game.animations,
            &rs.renderer.unit_atlas,
            game.input.selected_unit,
            &game.input.tile_stack_index,
            game.viewing_player,
        );
        let (unit_instances, unit_batches) =
            apply_wrap_unit_batched(&raw_units, &raw_batches, &wrap_offsets);
        let city_rect = rs.renderer.city_atlas.get_rect(0, 0); // ancient era town
        let city_instances = apply_wrap_city(&build_city_instances(view, city_rect), &wrap_offsets);

        let camera_uniform = game.camera.to_uniform();
        let turn = game.engine.current_turn();
        let viewing_player = game.viewing_player;
        let fps = game.current_fps;
        let map_width_f = view.map_width as f32;
        let map_height_f = view.map_height as f32;

        rs.renderer.update_camera(&camera_uniform);

        let width = rs.renderer.config.width;
        let height = rs.renderer.config.height;
        rs.text_state
            .viewport
            .update(&rs.renderer.queue, Resolution { width, height });

        rs.renderer.upload_tiles(&tile_instances);
        rs.renderer.upload_path_lines(&[]);
        rs.renderer.upload_selection_circles(&[]);
        rs.renderer.upload_units(&unit_instances, unit_batches);
        rs.renderer.upload_cities(&city_instances);
        rs.renderer.upload_ui_rects(&[]);
        rs.renderer.upload_ui_textured_rects(&[]);
        rs.renderer.upload_dialog_nine_slices(&[]);
        rs.renderer.upload_advisor_rects(&[]);
        rs.renderer.ui_chrome_buffer = None;
        rs.renderer.ui_chrome_count = 0;
        rs.renderer.ui_chrome_batches.clear();

        // Minimap
        let minimap_w = 200.0f32;
        let minimap_h = 125.0f32;
        let minimap_margin = 10.0;
        let minimap_x = width as f32 - minimap_w - minimap_margin;
        let minimap_y = height as f32 - minimap_h - minimap_margin;
        let minimap_rect = [minimap_x, minimap_y, minimap_w, minimap_h];

        let minimap_tile_instances = build_minimap_tiles(view);
        rs.renderer.upload_minimap_tiles(&minimap_tile_instances);

        let (iso_map_cx, iso_map_cy, iso_map_w, iso_map_h) =
            minimap_visible_extent(view, map_width_f, map_height_f, view.wrap_y)
                .unwrap_or_else(|| minimap_iso_extent(map_width_f, map_height_f, view.wrap_y));
        let minimap_zoom = (minimap_w / iso_map_w).min(minimap_h / iso_map_h);
        let minimap_camera = CameraUniform {
            center: [iso_map_cx, iso_map_cy],
            zoom: minimap_zoom,
            aspect: minimap_w / minimap_h,
            viewport_w: minimap_w,
            viewport_h: minimap_h,
            map_height: map_height_f,
            wrap_y: if view.wrap_y { 1.0 } else { 0.0 },
        };
        rs.renderer.update_minimap_camera(&minimap_camera);

        // Viewport indicator on minimap
        let viewport_overlay = build_minimap_viewport_indicator(
            &game.camera,
            [iso_map_cx, iso_map_cy],
            minimap_zoom,
            minimap_w,
            minimap_h,
        );
        rs.renderer.upload_minimap_overlay(&viewport_overlay);

        let width_f = width as f32;
        let height_f = height as f32;

        // Build menu text overlay
        {
            let ts = &mut rs.text_state;
            let ui_family = match &ts.ui_font_family {
                Some(name) => Family::Name(name),
                None => Family::Monospace,
            };
            let mut all_text_areas = Vec::new();

            // Title
            let title = "MENU";
            let mut title_buf = TextBuffer::new(&mut ts.font_system, Metrics::new(28.0, 34.0));
            title_buf.set_size(&mut ts.font_system, Some(300.0), Some(40.0));
            title_buf.set_text(
                &mut ts.font_system,
                title,
                &Attrs::new().family(ui_family),
                Shaping::Basic,
                None,
            );
            title_buf.shape_until_scroll(&mut ts.font_system, false);

            all_text_areas.push(TextArea {
                buffer: &title_buf,
                left: width_f / 2.0 - 40.0,
                top: height_f / 2.0 - 80.0,
                scale: 1.0,
                bounds: TextBounds {
                    left: 0,
                    top: 0,
                    right: width_f as i32,
                    bottom: height_f as i32,
                },
                default_color: TextColor::rgb(255, 255, 255),
                custom_glyphs: &[],
            });

            // Menu items
            let mut item_buffers: Vec<TextBuffer> = Vec::new();
            for (i, item) in menu::MENU_ITEMS.iter().enumerate() {
                let prefix = if i == selected_index { "> " } else { "  " };
                let text = format!("{}{}", prefix, item);
                let mut buf = TextBuffer::new(&mut ts.font_system, Metrics::new(22.0, 28.0));
                buf.set_size(&mut ts.font_system, Some(300.0), Some(30.0));
                buf.set_text(
                    &mut ts.font_system,
                    &text,
                    &Attrs::new().family(ui_family),
                    Shaping::Basic,
                    None,
                );
                buf.shape_until_scroll(&mut ts.font_system, false);
                item_buffers.push(buf);
            }

            for (i, buf) in item_buffers.iter().enumerate() {
                let color = if i == selected_index {
                    TextColor::rgb(255, 255, 100)
                } else {
                    TextColor::rgb(200, 200, 200)
                };
                all_text_areas.push(TextArea {
                    buffer: buf,
                    left: width_f / 2.0 - 80.0,
                    top: height_f / 2.0 - 40.0 + i as f32 * 30.0,
                    scale: 1.0,
                    bounds: TextBounds {
                        left: 0,
                        top: 0,
                        right: width_f as i32,
                        bottom: height_f as i32,
                    },
                    default_color: color,
                    custom_glyphs: &[],
                });
            }

            // Close button "X" in top-right
            let mut close_buf = TextBuffer::new(&mut ts.font_system, Metrics::new(20.0, 24.0));
            close_buf.set_size(&mut ts.font_system, Some(40.0), Some(28.0));
            close_buf.set_text(
                &mut ts.font_system,
                "[X]",
                &Attrs::new().family(ui_family),
                Shaping::Basic,
                None,
            );
            close_buf.shape_until_scroll(&mut ts.font_system, false);

            all_text_areas.push(TextArea {
                buffer: &close_buf,
                left: width_f - 50.0,
                top: 5.0,
                scale: 1.0,
                bounds: TextBounds {
                    left: 0,
                    top: 0,
                    right: width_f as i32,
                    bottom: height_f as i32,
                },
                default_color: TextColor::rgb(255, 100, 100),
                custom_glyphs: &[],
            });

            // Hint at bottom
            let mut hint_buf = TextBuffer::new(&mut ts.font_system, Metrics::new(14.0, 18.0));
            hint_buf.set_size(&mut ts.font_system, Some(400.0), Some(24.0));
            hint_buf.set_text(
                &mut ts.font_system,
                "Arrow keys to navigate, Enter to select, Escape to close",
                &Attrs::new().family(ui_family),
                Shaping::Basic,
                None,
            );
            hint_buf.shape_until_scroll(&mut ts.font_system, false);

            all_text_areas.push(TextArea {
                buffer: &hint_buf,
                left: width_f / 2.0 - 200.0,
                top: height_f / 2.0 + 40.0,
                scale: 1.0,
                bounds: TextBounds {
                    left: 0,
                    top: 0,
                    right: width_f as i32,
                    bottom: height_f as i32,
                },
                default_color: TextColor::rgb(150, 150, 150),
                custom_glyphs: &[],
            });

            let _ = ts.text_renderer.prepare(
                &rs.renderer.device,
                &rs.renderer.queue,
                &mut ts.font_system,
                &mut ts.atlas,
                &ts.viewport,
                all_text_areas,
                &mut ts.swash_cache,
            );
        }

        let text_info = Some((
            &rs.text_state.text_renderer,
            &rs.text_state.atlas,
            &rs.text_state.viewport,
        ));
        match rs
            .renderer
            .render_with_minimap(text_info, Some(minimap_rect))
        {
            Ok(()) => {}
            Err(wgpu::SurfaceError::Lost) => {
                let size = rs.renderer.window.inner_size();
                rs.renderer.resize(size);
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                log::error!("Out of GPU memory");
            }
            Err(e) => {
                log::warn!("Surface error: {:?}", e);
            }
        }

        rs.renderer.window.set_title(&format!(
            "FreeC3 — Menu — Turn {} — Player {} — {:.0} FPS",
            turn, viewing_player.0, fps
        ));
    }

    fn render_city_view(&mut self, city_id: fc3_core::id::CityId) {
        let game = match &mut self.game_state {
            Some(g) => g,
            None => return,
        };

        let fps = game.current_fps;
        let turn = game.engine.current_turn();

        // Get city view data
        let cvd = match &game.city_view_data {
            Some(d) => d,
            None => {
                // Data not loaded — go back to overworld
                game.active_screen = UIScreen::Overworld;
                return;
            }
        };

        let city = &cvd.city;
        let city_pos = city.position;

        // Build tile instances for the city radius (top half of screen)
        let mut tile_instances = Vec::new();
        let worked_set: std::collections::HashSet<TileCoord> = city
            .worked_tiles
            .as_ref()
            .map(|wt| wt.iter().copied().collect())
            .unwrap_or_default();

        for ts in &cvd.tile_snapshots {
            let mut color = terrain_color(ts.terrain);
            color = vegetation_tint(color, ts.vegetation);
            if ts.road_level > 0 {
                color = road_tint(color);
            }
            if let Some(imp) = ts.improvement {
                color = improvement_tint(color, imp);
            }
            // Worked tiles get brighter tint
            if worked_set.contains(&ts.coord) {
                for c in &mut color {
                    *c = (*c + 0.15).min(1.0);
                }
            }
            // City center gets special highlight
            if ts.coord == city_pos {
                let highlight = [1.0, 0.9, 0.3];
                for i in 0..3 {
                    color[i] = color[i] * 0.6 + highlight[i] * 0.4;
                }
            }
            tile_instances.push(TileInstance {
                position: [ts.coord.x as f32, ts.coord.y as f32],
                color,
                border_color: [0.0; 3],
                border_mask: 0.0,
                atlas_rect: [0.0; 4],
                overlay_rect: [0.0; 4],
                veg_rect: [0.0; 4],
                fog_rect: [0.0; 4],
            });
        }

        let (layer_rects, layer_texts) = build_layer_visuals(&game.ui_layers);

        // Extract camera params before borrowing render_state
        let cam_map_height = game.camera.map_height;
        let cam_wrap_y = game.camera.wrap_y;
        let city_lx = city_pos.x as f32 + 0.5;
        let city_ly = city_pos.y as f32 + 0.5;
        let city_display_ly =
            camera::adjust_y_for_wrap(city_lx, city_ly, cam_map_height, cam_wrap_y);
        let (city_iso_x, city_iso_y) = logical_to_iso(city_lx, city_display_ly);

        // City view camera: centered on city, zoomed to show ~7 tiles
        let rs = match &mut self.render_state {
            Some(rs) => rs,
            None => return,
        };

        let width = rs.renderer.config.width;
        let height = rs.renderer.config.height;
        let width_f = width as f32;
        let height_f = height as f32;
        let half_h = height_f / 2.0;

        // Use minimap camera buffer for city view tiles (they never coexist)
        let city_zoom = half_h / 7.0; // ~7 iso units across the top half
        let city_camera = CameraUniform {
            center: [city_iso_x, city_iso_y],
            zoom: city_zoom,
            aspect: width_f / half_h,
            viewport_w: width_f,
            viewport_h: half_h,
            map_height: cam_map_height,
            wrap_y: if cam_wrap_y { 1.0 } else { 0.0 },
        };
        rs.renderer.update_minimap_camera(&city_camera);
        rs.renderer.upload_minimap_tiles(&tile_instances);
        rs.renderer.upload_minimap_overlay(&[]);

        // Clear main tile/unit/city/path buffers
        rs.renderer.upload_tiles(&[]);
        rs.renderer.upload_path_lines(&[]);
        rs.renderer.upload_selection_circles(&[]);
        rs.renderer.upload_units(&[], Vec::new());
        rs.renderer.upload_cities(&[]);

        // Upload UI layer rects
        let ui_rect_instances = rects_to_instances(&layer_rects, width_f, height_f);
        rs.renderer.upload_ui_rects(&ui_rect_instances);
        rs.renderer.upload_ui_textured_rects(&[]);
        rs.renderer.upload_dialog_nine_slices(&[]);
        rs.renderer.upload_advisor_rects(&[]);
        rs.renderer.ui_chrome_buffer = None;
        rs.renderer.ui_chrome_count = 0;
        rs.renderer.ui_chrome_batches.clear();

        rs.text_state
            .viewport
            .update(&rs.renderer.queue, Resolution { width, height });

        // Build text overlays
        let pop = city.population;
        let food_stockpile = city.food_stockpile.unwrap_or(0);
        let food_per_turn = city.food_per_turn.unwrap_or(0);
        let shield_stockpile = city.shield_stockpile.unwrap_or(0);
        let shields_per_turn = city.shields_per_turn.unwrap_or(0);
        let commerce_per_turn = city.commerce_per_turn.unwrap_or(0);
        let growth_threshold = city.food_growth_threshold.unwrap_or(10 + 2 * pop);
        let producing_name = city.producing.as_deref().unwrap_or("None");
        let prod_cost = city.production_cost.unwrap_or(0);
        let turns_left = if shields_per_turn > 0 && prod_cost > 0 {
            let remaining = (prod_cost - shield_stockpile).max(0);
            (remaining + shields_per_turn - 1) / shields_per_turn
        } else {
            0
        };

        let producing_line = if producing_name == "None" {
            "Producing: None  [Change]".to_string()
        } else if turns_left > 0 {
            format!(
                "Producing: {} -- {} turns  [Change]",
                producing_name, turns_left
            )
        } else {
            format!("Producing: {}  [Change]", producing_name)
        };

        let info_lines = vec![
            format!("{} -- Pop: {}", city.name, pop),
            String::new(),
            format!(
                "Food:     +{}/turn   {}/{}",
                food_per_turn, food_stockpile, growth_threshold
            ),
            format!(
                "Shields:  +{}/turn   {}/{}",
                shields_per_turn, shield_stockpile, prod_cost
            ),
            format!("Commerce: +{}/turn", commerce_per_turn),
            String::new(),
            producing_line,
            String::new(),
            "[Escape] Close".to_string(),
        ];

        {
            let ts = &mut rs.text_state;
            let ui_family = match &ts.ui_font_family {
                Some(name) => Family::Name(name),
                None => Family::Monospace,
            };
            let mut all_text_areas = Vec::new();

            // Yield annotations on tiles (top half)
            let mut yield_buffers: Vec<(TextBuffer, f32, f32)> = Vec::new();
            for &(coord, f, s, c) in &cvd.tile_yields {
                let (tile_ix, tile_iy) = logical_to_iso(coord.x as f32 + 0.5, coord.y as f32 + 0.5);
                let screen_x = (tile_ix - city_iso_x) * city_zoom + width_f / 2.0;
                let screen_y = (tile_iy - city_iso_y) * city_zoom + half_h / 2.0;

                if screen_x < -50.0
                    || screen_x > width_f + 50.0
                    || screen_y < -50.0
                    || screen_y > half_h + 50.0
                {
                    continue;
                }

                let yield_text = format!("{}/{}/{}", f, s, c);
                let mut buf = TextBuffer::new(&mut ts.font_system, Metrics::new(12.0, 14.0));
                buf.set_size(&mut ts.font_system, Some(80.0), Some(16.0));
                buf.set_text(
                    &mut ts.font_system,
                    &yield_text,
                    &Attrs::new().family(ui_family),
                    Shaping::Basic,
                    None,
                );
                buf.shape_until_scroll(&mut ts.font_system, false);
                yield_buffers.push((buf, screen_x, screen_y));
            }

            for (buf, sx, sy) in &yield_buffers {
                all_text_areas.push(TextArea {
                    buffer: buf,
                    left: sx - 20.0,
                    top: sy - 7.0,
                    scale: 1.0,
                    bounds: TextBounds {
                        left: 0,
                        top: 0,
                        right: width_f as i32,
                        bottom: half_h as i32,
                    },
                    default_color: TextColor::rgb(255, 255, 200),
                    custom_glyphs: &[],
                });
            }

            // Info panel (bottom half)
            let mut info_buffers: Vec<TextBuffer> = Vec::new();
            let panel_x = 30.0;
            let panel_start_y = half_h + 20.0;
            let line_height = 24.0;

            for line in &info_lines {
                let mut buf = TextBuffer::new(&mut ts.font_system, Metrics::new(18.0, 22.0));
                buf.set_size(&mut ts.font_system, Some(width_f - 60.0), Some(26.0));
                buf.set_text(
                    &mut ts.font_system,
                    line,
                    &Attrs::new().family(ui_family),
                    Shaping::Basic,
                    None,
                );
                buf.shape_until_scroll(&mut ts.font_system, false);
                info_buffers.push(buf);
            }

            for (i, buf) in info_buffers.iter().enumerate() {
                let color = if i == 0 {
                    TextColor::rgb(255, 255, 100) // City name in yellow
                } else if i == 6 {
                    TextColor::rgb(100, 200, 255) // Producing line in cyan (clickable)
                } else {
                    TextColor::rgb(220, 220, 220)
                };
                all_text_areas.push(TextArea {
                    buffer: buf,
                    left: panel_x,
                    top: panel_start_y + i as f32 * line_height,
                    scale: 1.0,
                    bounds: TextBounds {
                        left: 0,
                        top: 0,
                        right: width_f as i32,
                        bottom: height_f as i32,
                    },
                    default_color: color,
                    custom_glyphs: &[],
                });
            }

            // Close button [X] in top-right
            let mut close_buf = TextBuffer::new(&mut ts.font_system, Metrics::new(20.0, 24.0));
            close_buf.set_size(&mut ts.font_system, Some(40.0), Some(28.0));
            close_buf.set_text(
                &mut ts.font_system,
                "[X]",
                &Attrs::new().family(ui_family),
                Shaping::Basic,
                None,
            );
            close_buf.shape_until_scroll(&mut ts.font_system, false);

            all_text_areas.push(TextArea {
                buffer: &close_buf,
                left: width_f - 50.0,
                top: 5.0,
                scale: 1.0,
                bounds: TextBounds {
                    left: 0,
                    top: 0,
                    right: width_f as i32,
                    bottom: height_f as i32,
                },
                default_color: TextColor::rgb(255, 100, 100),
                custom_glyphs: &[],
            });

            // Layer text elements
            let mut layer_buffers: Vec<TextBuffer> = Vec::new();
            for te in &layer_texts {
                let mut buf = TextBuffer::new(
                    &mut ts.font_system,
                    Metrics::new(te.font_size, te.line_height),
                );
                buf.set_size(
                    &mut ts.font_system,
                    Some(te.max_width),
                    Some(te.line_height + 4.0),
                );
                buf.set_text(
                    &mut ts.font_system,
                    &te.text,
                    &Attrs::new().family(ui_family),
                    Shaping::Basic,
                    None,
                );
                buf.shape_until_scroll(&mut ts.font_system, false);
                layer_buffers.push(buf);
            }

            for (i, buf) in layer_buffers.iter().enumerate() {
                let te = &layer_texts[i];
                all_text_areas.push(TextArea {
                    buffer: buf,
                    left: te.x,
                    top: te.y,
                    scale: 1.0,
                    bounds: TextBounds {
                        left: 0,
                        top: 0,
                        right: width_f as i32,
                        bottom: height_f as i32,
                    },
                    default_color: TextColor::rgb(te.color[0], te.color[1], te.color[2]),
                    custom_glyphs: &[],
                });
            }

            let _ = ts.text_renderer.prepare(
                &rs.renderer.device,
                &rs.renderer.queue,
                &mut ts.font_system,
                &mut ts.atlas,
                &ts.viewport,
                all_text_areas,
                &mut ts.swash_cache,
            );
        }

        // Render: city tiles in top half using minimap viewport, text everywhere
        let minimap_rect = [0.0, 0.0, width_f, half_h];
        let text_info = Some((
            &rs.text_state.text_renderer,
            &rs.text_state.atlas,
            &rs.text_state.viewport,
        ));
        match rs
            .renderer
            .render_with_minimap(text_info, Some(minimap_rect))
        {
            Ok(()) => {}
            Err(wgpu::SurfaceError::Lost) => {
                let size = rs.renderer.window.inner_size();
                rs.renderer.resize(size);
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                log::error!("Out of GPU memory");
            }
            Err(e) => {
                log::warn!("Surface error: {:?}", e);
            }
        }

        let _ = city_id; // used for type clarity
        rs.renderer.window.set_title(&format!(
            "FreeC3 — City: {} — Turn {} — {:.0} FPS",
            cvd.city.name, turn, fps
        ));
    }
}

/// Convert layer RectElements to GPU instances.
fn rects_to_instances(
    rects: &[RectElement],
    viewport_w: f32,
    viewport_h: f32,
) -> Vec<UIRectInstance> {
    rects
        .iter()
        .map(|r| UIRectInstance {
            rect: [r.rect.x, r.rect.y, r.rect.w, r.rect.h],
            color: r.color,
            viewport: [viewport_w, viewport_h],
        })
        .collect()
}

/// Build simplified tile instances for the minimap (no borders, no wrapping).
/// Compute the iso center and extent for the minimap, accounting for wrap_y.
fn minimap_iso_extent(map_width: f32, map_height: f32, wrap_y: bool) -> (f32, f32, f32, f32) {
    if wrap_y {
        // With wrap_y: iso_x ∈ [-H, 2W], iso_y ∈ [0, H/2]
        let cx = (2.0 * map_width - map_height) / 2.0;
        let cy = map_height / 4.0;
        let w = 2.0 * map_width + map_height;
        let h = map_height / 2.0;
        (cx, cy, w, h)
    } else {
        let (cx, cy) = logical_to_iso(map_width / 2.0, map_height / 2.0);
        let w = map_width + map_height;
        let h = (map_width + map_height) / 2.0;
        (cx, cy, w, h)
    }
}

fn build_minimap_tiles(view: &PlayerView) -> Vec<TileInstance> {
    let mut instances = Vec::with_capacity(view.visible_tiles.len());

    // Build a set of unit positions and city positions for dots
    let mut unit_positions: HashMap<(u32, u32), PlayerId> = HashMap::new();
    for unit in &view.known_units {
        unit_positions
            .entry((unit.position.x, unit.position.y))
            .or_insert(unit.owner);
    }

    let mut city_positions: HashMap<(u32, u32), PlayerId> = HashMap::new();
    for city in view.own_cities.iter().chain(view.known_cities.iter()) {
        city_positions
            .entry((city.position.x, city.position.y))
            .or_insert(city.owner);
    }

    for tile in &view.visible_tiles {
        let mut color = terrain_color(tile.terrain);
        color = vegetation_tint(color, tile.vegetation);

        // Ownership tint
        if let Some(owner) = tile.owner {
            color = ownership_tint(color, owner);
        }

        // City dot: override with bright player color
        if let Some(&owner) = city_positions.get(&(tile.coord.x, tile.coord.y)) {
            let pc = player_color(owner);
            color = [pc[0] * 0.9 + 0.1, pc[1] * 0.9 + 0.1, pc[2] * 0.9 + 0.1];
        }
        // Unit dot: override with player color (cities take priority above)
        else if let Some(&owner) = unit_positions.get(&(tile.coord.x, tile.coord.y)) {
            if tile.visibility == Visibility::Visible {
                color = player_color(owner);
            }
        }

        instances.push(TileInstance {
            position: [tile.coord.x as f32, tile.coord.y as f32],
            color,
            border_color: [0.0; 3],
            border_mask: 0.0,
            atlas_rect: [0.0; 4],
            overlay_rect: [0.0; 4],
            veg_rect: [0.0; 4],
            fog_rect: [0.0; 4],
        });
    }

    instances
}

/// Compute the iso bounding box of all revealed/visible tiles for adaptive minimap scaling.
/// Returns (center_x, center_y, width, height) in iso space, or None if no tiles are revealed.
/// The extent is capped to the full map extent so it never exceeds it.
fn minimap_visible_extent(
    view: &PlayerView,
    map_width: f32,
    map_height: f32,
    wrap_y: bool,
) -> Option<(f32, f32, f32, f32)> {
    let mut min_ix = f32::MAX;
    let mut max_ix = f32::MIN;
    let mut min_iy = f32::MAX;
    let mut max_iy = f32::MIN;
    let mut has_revealed = false;

    for tile in &view.visible_tiles {
        if tile.visibility == Visibility::Unseen {
            continue;
        }
        has_revealed = true;
        let lx = tile.coord.x as f32;
        let ly = tile.coord.y as f32;
        let display_y = adjust_y_for_wrap(lx, ly, map_height, wrap_y);
        let (ix, iy) = logical_to_iso(lx, display_y);
        min_ix = min_ix.min(ix);
        max_ix = max_ix.max(ix);
        min_iy = min_iy.min(iy);
        max_iy = max_iy.max(iy);
    }

    if !has_revealed {
        return None;
    }

    // Add padding (1.5 tiles in iso space)
    let padding = 1.5;
    min_ix -= padding;
    max_ix += padding;
    min_iy -= padding * 0.5;
    max_iy += padding * 0.5;

    // Cap to full map extent
    let (full_cx, full_cy, full_w, full_h) = minimap_iso_extent(map_width, map_height, wrap_y);
    let full_min_ix = full_cx - full_w / 2.0;
    let full_max_ix = full_cx + full_w / 2.0;
    let full_min_iy = full_cy - full_h / 2.0;
    let full_max_iy = full_cy + full_h / 2.0;
    min_ix = min_ix.max(full_min_ix);
    max_ix = max_ix.min(full_max_ix);
    min_iy = min_iy.max(full_min_iy);
    max_iy = max_iy.min(full_max_iy);

    // Enforce a minimum extent so the minimap doesn't zoom in ridiculously
    let min_extent = 8.0;
    let w = (max_ix - min_ix).max(min_extent);
    let h = (max_iy - min_iy).max(min_extent * 0.5);
    let cx = (min_ix + max_ix) / 2.0;
    let cy = (min_iy + max_iy) / 2.0;

    Some((cx, cy, w, h))
}

/// Build UIRectInstance line segments for the camera viewport indicator on the minimap.
/// Returns up to 4 rect instances forming a white rectangle outline.
fn build_minimap_viewport_indicator(
    camera: &Camera,
    minimap_center: [f32; 2],
    minimap_zoom: f32,
    minimap_w: f32,
    minimap_h: f32,
) -> Vec<UIRectInstance> {
    // Main camera viewport bounds in iso space
    let iso_half_w = camera.viewport_width / (2.0 * camera.zoom);
    let iso_half_h = camera.viewport_height / (2.0 * camera.zoom);
    let iso_left = camera.center_x - iso_half_w;
    let iso_right = camera.center_x + iso_half_w;
    let iso_top = camera.center_y - iso_half_h;
    let iso_bottom = camera.center_y + iso_half_h;

    // Convert iso coords to minimap pixel coords (relative to minimap top-left)
    let to_mm_x = |ix: f32| (ix - minimap_center[0]) * minimap_zoom + minimap_w / 2.0;
    let to_mm_y = |iy: f32| (iy - minimap_center[1]) * minimap_zoom + minimap_h / 2.0;

    let mm_left = to_mm_x(iso_left).clamp(0.0, minimap_w);
    let mm_right = to_mm_x(iso_right).clamp(0.0, minimap_w);
    let mm_top = to_mm_y(iso_top).clamp(0.0, minimap_h);
    let mm_bottom = to_mm_y(iso_bottom).clamp(0.0, minimap_h);

    let box_w = mm_right - mm_left;
    let box_h = mm_bottom - mm_top;

    // If the viewport covers most of the minimap, don't draw (not useful)
    if box_w >= minimap_w - 2.0 && box_h >= minimap_h - 2.0 {
        return Vec::new();
    }

    let line_w = 1.0f32;
    let color = [1.0, 1.0, 1.0, 0.8]; // White, slightly transparent
    let viewport = [minimap_w, minimap_h];

    vec![
        // Top edge
        UIRectInstance {
            rect: [mm_left, mm_top, box_w, line_w],
            color,
            viewport,
        },
        // Bottom edge
        UIRectInstance {
            rect: [mm_left, mm_bottom - line_w, box_w, line_w],
            color,
            viewport,
        },
        // Left edge
        UIRectInstance {
            rect: [mm_left, mm_top, line_w, box_h],
            color,
            viewport,
        },
        // Right edge
        UIRectInstance {
            rect: [mm_right - line_w, mm_top, line_w, box_h],
            color,
            viewport,
        },
    ]
}

/// Check if two tiles are adjacent (including diagonal), handling wrapping.
fn is_adjacent(
    a: TileCoord,
    b: TileCoord,
    map_width: u32,
    wrap_x: bool,
    map_height: u32,
    wrap_y: bool,
) -> bool {
    let dy = (a.y as i32 - b.y as i32).abs();
    let dy = if wrap_y {
        dy.min((map_height as i32) - dy)
    } else {
        dy
    };
    if dy > 1 {
        return false;
    }
    let dx = (a.x as i32 - b.x as i32).abs();
    let dx = if wrap_x {
        dx.min((map_width as i32) - dx)
    } else {
        dx
    };
    dx <= 1 && (dx + dy) > 0
}

/// Returns normalized UV rects [default, hover, active] for a grid position.
/// The atlas stacks NormButtons / RollOverButtons / HighlightedButtons vertically.
fn icon_uvs_from_pos(
    col: i32,
    row: i32,
    atlas_w: u32,
    atlas_h: u32,
    layer_h: u32,
) -> [[f32; 4]; 3] {
    let aw = atlas_w.max(1) as f32;
    let ah = atlas_h.max(1) as f32;
    let px_x = col as f32 * 32.0;
    let px_y = row as f32 * 32.0;
    let lh = layer_h as f32;
    let u = px_x / aw;
    let w = 32.0 / aw;
    let h = 32.0 / ah;
    [
        [u, px_y / ah, w, h],
        [u, (px_y + lh) / ah, w, h],
        [u, (px_y + lh * 2.0) / ah, w, h],
    ]
}

/// Build a UIBox for an action button from default/hover/active UV rects.
fn build_action_uibox(x: f32, y: f32, uvs: [[f32; 4]; 3]) -> UIBox {
    UIBox {
        x,
        y,
        width: 32.0,
        height: 32.0,
        default_image: BoxImage::Atlas {
            u: uvs[0][0],
            v: uvs[0][1],
            w: uvs[0][2],
            h: uvs[0][3],
        },
        hover_image: Some(BoxImage::Atlas {
            u: uvs[1][0],
            v: uvs[1][1],
            w: uvs[1][2],
            h: uvs[1][3],
        }),
        active_image: Some(BoxImage::Atlas {
            u: uvs[2][0],
            v: uvs[2][1],
            w: uvs[2][2],
            h: uvs[2][3],
        }),
    }
}

/// Build the three top-left menu buttons from the menuButtons.pcx atlas.
/// The atlas contains all 3 states in one image at known pixel offsets:
///   Normal at y=0, Hover at y=61, Active at y=120
/// Button positions: (0,0) 36x30, (37,0) 36x30, (74,0) 32x30
fn build_menu_boxes(atlas_w: u32, atlas_h: u32) -> Vec<UIBox> {
    let aw = atlas_w.max(1) as f32;
    let ah = atlas_h.max(1) as f32;
    // (px_x, px_y_normal, btn_w, btn_h)
    let buttons: [(f32, f32, f32); 3] = [(0.0, 36.0, 30.0), (37.0, 36.0, 30.0), (74.0, 32.0, 30.0)];
    let gap = 2.0;
    let mut boxes = Vec::with_capacity(3);
    let mut screen_x = 30.0;
    for &(px_x, btn_w, btn_h) in &buttons {
        let u = px_x / aw;
        let w = btn_w / aw;
        let h = btn_h / ah;
        let v_normal = 0.0 / ah;
        let v_hover = 60.0 / ah;
        let v_active = 120.0 / ah;
        boxes.push(UIBox {
            x: screen_x,
            y: 20.0,
            width: btn_w,
            height: btn_h,
            default_image: BoxImage::Atlas {
                u,
                v: v_normal,
                w,
                h,
            },
            hover_image: Some(BoxImage::Atlas {
                u,
                v: v_hover,
                w,
                h,
            }),
            active_image: Some(BoxImage::Atlas {
                u,
                v: v_active,
                w,
                h,
            }),
        });
        screen_x += btn_w + gap;
    }
    boxes
}

fn main() {
    env_logger::init();
    log::info!("FreeC3 Desktop Viewer starting...");

    // Parse --resource-dir from CLI args, fallback to RESOURCE_DIR env var
    let resource_dir = {
        let args: Vec<String> = std::env::args().collect();
        let mut dir: Option<PathBuf> = None;
        let mut i = 1;
        while i < args.len() {
            if args[i] == "--resource-dir" && i + 1 < args.len() {
                dir = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            } else {
                i += 1;
            }
        }
        if dir.is_none() {
            if let Ok(val) = std::env::var("RESOURCE_DIR") {
                dir = Some(PathBuf::from(val));
            }
        }
        dir
    };
    if let Some(ref dir) = resource_dir {
        log::info!("Resource directory: {}", dir.display());
    }

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);

    let mut app = DesktopApp::new(resource_dir);
    event_loop.run_app(&mut app).unwrap();
}
