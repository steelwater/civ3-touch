use std::collections::{HashMap, VecDeque};

use fc3_core::id::UnitId;
use fc3_core::types::{Direction, TileCoord};

/// Default move duration when no RUN timing data is available.
const DEFAULT_MOVE_DURATION: f32 = 0.3;

/// Visual position for a unit (may differ from logical position during animation).
#[derive(Debug, Clone, Copy)]
pub struct VisualPos {
    pub x: f32,
    pub y: f32,
}

/// A pending unit movement animation.
#[derive(Debug)]
struct MoveAnim {
    from: VisualPos,
    to: VisualPos,
    elapsed: f32,
    duration: f32,
}

/// A combat flash animation on a tile.
#[derive(Debug)]
struct CombatFlash {
    pub tile: TileCoord,
    pub elapsed: f32,
    pub duration: f32,
}

/// A timed pause animation (e.g. for fortify/skip visual feedback).
#[derive(Debug)]
struct ActionAnim {
    elapsed: f32,
    duration: f32,
}

/// A named animation transition (plays once through a FLIC, e.g. FORTIFY, ROAD, MINE).
#[derive(Debug)]
struct NamedAnim {
    anim_name: String,
    elapsed: f32,
    duration: f32,
}

/// A queued animation event for sequential playback.
#[derive(Debug)]
enum AnimEvent {
    Move {
        unit_id: UnitId,
        from: TileCoord,
        to: TileCoord,
        unit_type: String,
    },
    CombatFlash {
        tile: TileCoord,
    },
    ActionPause {
        unit_id: UnitId,
        duration: f32,
    },
    Named {
        unit_id: UnitId,
        unit_type: String,
        anim_name: String,
    },
}

/// Manages all visual animations.
pub struct AnimationState {
    move_anims: HashMap<UnitId, MoveAnim>,
    combat_flashes: Vec<CombatFlash>,
    action_anims: HashMap<UnitId, ActionAnim>,
    named_anims: HashMap<UnitId, NamedAnim>,
    /// Direction overrides for units with active/queued move animations.
    direction_overrides: HashMap<UnitId, Direction>,
    /// Queue of animation events for sequential playback (player commands).
    event_queue: VecDeque<AnimEvent>,
    /// Accumulated real time (seconds) used as a global animation clock.
    anim_time: f32,
    /// Per-unit-type RUN cycle durations (unit_type_name → seconds).
    run_durations: HashMap<String, f32>,
    /// Per (unit_type, anim_name) cycle durations (from INI [Timing] data).
    anim_durations: HashMap<(String, String), f32>,
}

impl AnimationState {
    pub fn new() -> Self {
        AnimationState {
            move_anims: HashMap::new(),
            combat_flashes: Vec::new(),
            action_anims: HashMap::new(),
            named_anims: HashMap::new(),
            direction_overrides: HashMap::new(),
            event_queue: VecDeque::new(),
            anim_time: 0.0,
            run_durations: HashMap::new(),
            anim_durations: HashMap::new(),
        }
    }

    /// Set per-unit-type RUN cycle durations (from INI [Timing] data).
    pub fn set_run_durations(&mut self, durations: HashMap<String, f32>) {
        self.run_durations = durations;
    }

    /// Set per-unit-type cycle durations for a named animation (e.g. "FORTIFY", "ROAD").
    /// Merges into the existing anim_durations map keyed by (unit_type, anim_name).
    pub fn set_anim_durations(&mut self, anim_name: &str, durations: HashMap<String, f32>) {
        for (unit_type, dur) in durations {
            self.anim_durations
                .insert((unit_type, anim_name.to_string()), dur);
        }
    }

    /// Start a movement animation for a unit directly (used for AI animations).
    /// Duration is looked up from run_durations for the unit type, falling back to default.
    pub fn start_move(&mut self, unit_id: UnitId, from: TileCoord, to: TileCoord, unit_type: &str) {
        self.start_move_internal(unit_id, from, to, unit_type);
    }

    /// Internal: actually insert a move animation and set direction override.
    fn start_move_internal(
        &mut self,
        unit_id: UnitId,
        from: TileCoord,
        to: TileCoord,
        unit_type: &str,
    ) {
        let duration = self
            .run_durations
            .get(unit_type)
            .copied()
            .unwrap_or(DEFAULT_MOVE_DURATION);

        // Compute facing direction for this step
        let dx = to.x as i32 - from.x as i32;
        let dy = to.y as i32 - from.y as i32;
        if let Some(dir) = Direction::from_delta(dx, dy) {
            self.direction_overrides.insert(unit_id, dir);
        }

        self.move_anims.insert(
            unit_id,
            MoveAnim {
                from: VisualPos {
                    x: from.x as f32,
                    y: from.y as f32,
                },
                to: VisualPos {
                    x: to.x as f32,
                    y: to.y as f32,
                },
                elapsed: 0.0,
                duration,
            },
        );
    }

    /// Start a combat flash on a tile directly (used for AI animations).
    pub fn start_combat_flash(&mut self, tile: TileCoord) {
        self.combat_flashes.push(CombatFlash {
            tile,
            elapsed: 0.0,
            duration: 0.3,
        });
    }

    /// Queue a movement animation for sequential playback.
    pub fn queue_move(&mut self, unit_id: UnitId, from: TileCoord, to: TileCoord, unit_type: &str) {
        self.event_queue.push_back(AnimEvent::Move {
            unit_id,
            from,
            to,
            unit_type: unit_type.to_string(),
        });
    }

    /// Queue a combat flash for sequential playback.
    pub fn queue_combat_flash(&mut self, tile: TileCoord) {
        self.event_queue.push_back(AnimEvent::CombatFlash { tile });
    }

    /// Queue an action pause for sequential playback.
    pub fn queue_action_pause(&mut self, unit_id: UnitId, duration: f32) {
        self.event_queue
            .push_back(AnimEvent::ActionPause { unit_id, duration });
    }

    /// Queue a named animation (e.g. "FORTIFY", "ROAD", "MINE").
    /// Falls back to an action pause of `fallback_duration` if no timing data exists for this
    /// unit type + anim combination.
    pub fn queue_named_anim(
        &mut self,
        unit_id: UnitId,
        unit_type: &str,
        anim_name: &str,
        fallback_duration: f32,
    ) {
        let key = (unit_type.to_string(), anim_name.to_string());
        if self.anim_durations.contains_key(&key) {
            self.event_queue.push_back(AnimEvent::Named {
                unit_id,
                unit_type: unit_type.to_string(),
                anim_name: anim_name.to_string(),
            });
        } else {
            self.queue_action_pause(unit_id, fallback_duration);
        }
    }

    /// Queue a fortify animation. Falls back to a 0.4s action pause if the unit type
    /// has no FORTIFY animation in its INI.
    pub fn queue_fortify(&mut self, unit_id: UnitId, unit_type: &str) {
        self.queue_named_anim(unit_id, unit_type, "FORTIFY", 0.4);
    }

    /// Returns true if any blocking animation is active or queued.
    pub fn is_busy(&self) -> bool {
        !self.event_queue.is_empty()
            || !self.move_anims.is_empty()
            || !self.action_anims.is_empty()
            || !self.named_anims.is_empty()
    }

    /// Advance all animations by dt seconds. Returns true if any animation is active.
    pub fn update(&mut self, dt: f32) -> bool {
        let mut any_active = false;

        // Advance the global animation clock
        self.anim_time += dt;

        // Update move animations
        self.move_anims.retain(|_, anim| {
            anim.elapsed += dt;
            if anim.elapsed < anim.duration {
                any_active = true;
                true
            } else {
                false
            }
        });

        // Update combat flashes
        self.combat_flashes.retain(|flash| {
            if flash.elapsed < flash.duration {
                any_active = true;
                true
            } else {
                false
            }
        });
        for flash in &mut self.combat_flashes {
            flash.elapsed += dt;
        }

        // Update action animations
        self.action_anims.retain(|_, anim| {
            anim.elapsed += dt;
            if anim.elapsed < anim.duration {
                any_active = true;
                true
            } else {
                false
            }
        });

        // Update named animations
        self.named_anims.retain(|_, anim| {
            anim.elapsed += dt;
            if anim.elapsed < anim.duration {
                any_active = true;
                true
            } else {
                false
            }
        });

        // Process the event queue: pop events while no blocking animation is active
        loop {
            if self.event_queue.is_empty() {
                break;
            }
            // A blocking animation is active — wait for it to finish
            if !self.move_anims.is_empty()
                || !self.action_anims.is_empty()
                || !self.named_anims.is_empty()
            {
                break;
            }
            let event = self.event_queue.pop_front().unwrap();
            match event {
                AnimEvent::Move {
                    unit_id,
                    from,
                    to,
                    ref unit_type,
                } => {
                    self.start_move_internal(unit_id, from, to, unit_type);
                    any_active = true;
                    break; // Move blocks further pops
                }
                AnimEvent::CombatFlash { tile } => {
                    self.start_combat_flash(tile);
                    any_active = true;
                    // Non-blocking — continue popping
                }
                AnimEvent::ActionPause { unit_id, duration } => {
                    self.action_anims.insert(
                        unit_id,
                        ActionAnim {
                            elapsed: 0.0,
                            duration,
                        },
                    );
                    any_active = true;
                    break; // Pause blocks further pops
                }
                AnimEvent::Named {
                    unit_id,
                    ref unit_type,
                    ref anim_name,
                } => {
                    let key = (unit_type.clone(), anim_name.clone());
                    let duration = self.anim_durations.get(&key).copied().unwrap_or(0.4);
                    self.named_anims.insert(
                        unit_id,
                        NamedAnim {
                            anim_name: anim_name.clone(),
                            elapsed: 0.0,
                            duration,
                        },
                    );
                    any_active = true;
                    break; // Named anim blocks further pops
                }
            }
        }

        if !self.event_queue.is_empty() {
            any_active = true;
        }

        // Clean up direction overrides when all animations finish
        if !any_active {
            self.direction_overrides.clear();
        }

        any_active
    }

    /// Returns the current idle animation frame for a unit, desynchronized per unit.
    /// `frames_per_dir` is the total frame count for this unit type's animation.
    pub fn idle_frame(&self, unit_id: UnitId, frames_per_dir: u32) -> u32 {
        if frames_per_dir <= 1 {
            return 0;
        }
        // ~150ms per frame (~6.67 fps) for a relaxed idle animation
        const FRAME_DURATION: f32 = 0.15;
        let global_frame = (self.anim_time / FRAME_DURATION) as u32;
        // Offset by unit index + generation to desynchronize different units
        let offset = unit_id
            .index
            .wrapping_mul(7)
            .wrapping_add(unit_id.generation);
        (global_frame.wrapping_add(offset)) % frames_per_dir
    }

    /// Returns the LERP progress (0.0 to 1.0) for a moving unit, or None if not moving.
    pub fn move_progress(&self, unit_id: UnitId) -> Option<f32> {
        self.move_anims
            .get(&unit_id)
            .map(|anim| (anim.elapsed / anim.duration).min(1.0))
    }

    /// Returns the run animation frame synced to movement progress, or None if not moving.
    pub fn run_frame(&self, unit_id: UnitId, frames_per_dir: u32) -> Option<u32> {
        if frames_per_dir <= 1 {
            return self.move_progress(unit_id).map(|_| 0);
        }
        self.move_progress(unit_id).map(|progress| {
            let frame = (progress * frames_per_dir as f32) as u32;
            frame.min(frames_per_dir - 1)
        })
    }

    /// Returns the direction override for a unit with an active/queued move animation,
    /// or None if the unit should use its logical direction from PlayerView.
    pub fn direction_override(&self, unit_id: UnitId) -> Option<Direction> {
        self.direction_overrides.get(&unit_id).copied()
    }

    /// Returns true if a unit is currently in a move animation.
    pub fn is_moving(&self, unit_id: UnitId) -> bool {
        self.move_anims.contains_key(&unit_id)
    }

    /// Returns true if a unit is currently playing a fortify transition animation.
    #[allow(dead_code)]
    pub fn is_fortifying(&self, unit_id: UnitId) -> bool {
        self.named_anims
            .get(&unit_id)
            .is_some_and(|a| a.anim_name == "FORTIFY")
    }

    /// Returns the fortify animation progress (0.0 to 1.0), or None if not fortifying.
    #[allow(dead_code)]
    pub fn fortify_progress(&self, unit_id: UnitId) -> Option<f32> {
        self.named_anims
            .get(&unit_id)
            .filter(|a| a.anim_name == "FORTIFY")
            .map(|anim| (anim.elapsed / anim.duration).min(1.0))
    }

    /// Returns (anim_name, progress) for a unit's active named animation, or None.
    pub fn named_anim_info(&self, unit_id: UnitId) -> Option<(&str, f32)> {
        self.named_anims.get(&unit_id).map(|anim| {
            let progress = (anim.elapsed / anim.duration).min(1.0);
            (anim.anim_name.as_str(), progress)
        })
    }

    /// Get the visual position for a unit (animated or logical).
    pub fn visual_position(&self, unit_id: UnitId, logical: TileCoord) -> VisualPos {
        if let Some(anim) = self.move_anims.get(&unit_id) {
            let t = (anim.elapsed / anim.duration).min(1.0);
            // Smooth step
            let t = t * t * (3.0 - 2.0 * t);
            VisualPos {
                x: anim.from.x + (anim.to.x - anim.from.x) * t,
                y: anim.from.y + (anim.to.y - anim.from.y) * t,
            }
        } else {
            VisualPos {
                x: logical.x as f32,
                y: logical.y as f32,
            }
        }
    }

    /// Get combat flash tiles with their intensity (0-1).
    pub fn combat_flash_tiles(&self) -> Vec<(TileCoord, f32)> {
        self.combat_flashes
            .iter()
            .map(|flash| {
                let intensity = 1.0 - (flash.elapsed / flash.duration).min(1.0);
                (flash.tile, intensity)
            })
            .collect()
    }
}
