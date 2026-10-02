use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::ini::parse_ini;

/// A single animation entry for a unit type (e.g. DEFAULT, RUN).
///
/// The atlas is a grid of `num_directions` columns x `frames_per_dir` rows.
/// Column = direction index (0=SW, 1=S, 2=SE, 3=E, 4=NE, 5=N, 6=NW, 7=W)
/// Row = frame index within that direction.
struct UnitAnimEntry {
    bind_group: wgpu::BindGroup,
    num_directions: u32,
    frames_per_dir: u32,
    sprite_w: u32,
    sprite_h: u32,
    atlas_w: u32,
    atlas_h: u32,
    /// Duration of one full animation cycle in seconds (from INI [Timing] section).
    cycle_duration: f32,
}

impl UnitAnimEntry {
    /// Returns the sprite iso-space dimensions [width, height].
    /// Uses 64 px per iso unit to match terrain tile density (128x64 px = 2x1 iso).
    fn sprite_iso_size(&self) -> [f32; 2] {
        const PX_PER_ISO: f32 = 64.0;
        if self.atlas_w <= 1 {
            return [0.0, 0.0];
        }
        [
            self.sprite_w as f32 / PX_PER_ISO,
            self.sprite_h as f32 / PX_PER_ISO,
        ]
    }

    /// Returns atlas UV rect [u, v, w, h] for a given direction and frame.
    fn get_rect(&self, direction: u32, frame: u32) -> [f32; 4] {
        if self.atlas_w <= 1 {
            return [0.0; 4];
        }
        let dir = direction.min(self.num_directions - 1);
        let fr = frame.min(self.frames_per_dir - 1);
        let px_x = dir * self.sprite_w;
        let px_y = fr * self.sprite_h;
        [
            px_x as f32 / self.atlas_w as f32,
            px_y as f32 / self.atlas_h as f32,
            self.sprite_w as f32 / self.atlas_w as f32,
            self.sprite_h as f32 / self.atlas_h as f32,
        ]
    }
}

/// All animations for a single unit type (e.g. DEFAULT + RUN).
struct UnitTypeAnims {
    anims: HashMap<String, UnitAnimEntry>,
}

impl UnitTypeAnims {
    fn get_anim(&self, anim: &str) -> Option<&UnitAnimEntry> {
        self.anims.get(anim).or_else(|| {
            if anim != "DEFAULT" {
                self.anims.get("DEFAULT")
            } else {
                None
            }
        })
    }
}

/// Holds per-unit-type GPU texture atlases loaded from Civ3 FLIC animation files.
///
/// Each unit type can have multiple animations (DEFAULT, RUN, etc.).
/// Unit types without art fall back to a 1x1 transparent texture (SDF rendering).
pub struct UnitAtlasSet {
    pub bind_group_layout: wgpu::BindGroupLayout,
    entries: HashMap<String, UnitTypeAnims>,
    fallback_bind_group: wgpu::BindGroup,
}

/// Info needed to look up sprite data for a given unit type.
pub struct SpriteInfo {
    pub atlas_rect: [f32; 4],
    pub sprite_size: [f32; 2],
}

impl UnitAtlasSet {
    /// Create a set with no loaded sprites (all units use SDF fallback).
    pub fn fallback(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let bind_group_layout = create_bind_group_layout(device);
        let (fallback_bind_group, _) =
            create_unit_bind_group(device, queue, &bind_group_layout, 1, 1, &[0, 0, 0, 0]);
        UnitAtlasSet {
            bind_group_layout,
            entries: HashMap::new(),
            fallback_bind_group,
        }
    }

    /// Load unit sprites from a Civ3 resource directory using INI art definitions.
    ///
    /// `unit_art` is a list of (unit_type_name, art_ini_path) pairs where art_ini_path
    /// is relative to resource_dir (e.g. "Art/Units/warrior/Warrior.INI").
    pub fn from_resource_dir(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resource_dir: &Path,
        unit_art: &[(String, String)],
    ) -> Self {
        // Create the shared bind group layout once
        let bind_group_layout = create_bind_group_layout(device);

        let (fallback_bind_group, _) =
            create_unit_bind_group(device, queue, &bind_group_layout, 1, 1, &[0, 0, 0, 0]);

        let mut entries = HashMap::new();

        for (unit_type_name, art_ini_rel) in unit_art {
            match load_unit_from_ini(device, queue, &bind_group_layout, resource_dir, art_ini_rel) {
                Some(type_anims) => {
                    let anim_names: Vec<&str> =
                        type_anims.anims.keys().map(|s| s.as_str()).collect();
                    log::info!(
                        "Loaded unit sprites for '{}': animations {:?}",
                        unit_type_name,
                        anim_names,
                    );
                    entries.insert(unit_type_name.clone(), type_anims);
                }
                None => {
                    log::warn!(
                        "Failed to load sprites for '{}' from {}",
                        unit_type_name,
                        art_ini_rel,
                    );
                }
            }
        }

        UnitAtlasSet {
            bind_group_layout,
            entries,
            fallback_bind_group,
        }
    }

    /// Get sprite info (atlas rect and size) for a unit type and animation.
    /// Falls back: requested anim → DEFAULT → SDF fallback.
    pub fn sprite_info(
        &self,
        unit_type_name: &str,
        anim: &str,
        direction: u32,
        frame: u32,
    ) -> SpriteInfo {
        if let Some(type_anims) = self.entries.get(unit_type_name) {
            if let Some(entry) = type_anims.get_anim(anim) {
                return SpriteInfo {
                    atlas_rect: entry.get_rect(direction, frame),
                    sprite_size: entry.sprite_iso_size(),
                };
            }
        }
        SpriteInfo {
            atlas_rect: [0.0; 4],
            sprite_size: [0.0, 0.0],
        }
    }

    /// Get the number of animation frames per direction for a unit type and animation.
    /// Falls back: requested anim → DEFAULT → 1 (SDF fallback).
    pub fn frames_per_dir(&self, unit_type_name: &str, anim: &str) -> u32 {
        if let Some(type_anims) = self.entries.get(unit_type_name) {
            if let Some(entry) = type_anims.get_anim(anim) {
                return entry.frames_per_dir.max(1);
            }
        }
        1
    }

    /// Get the bind group for a given unit type and animation, or the fallback.
    /// Falls back: requested anim → DEFAULT → SDF fallback.
    pub fn bind_group(&self, unit_type_name: &str, anim: &str) -> &wgpu::BindGroup {
        if let Some(type_anims) = self.entries.get(unit_type_name) {
            if let Some(entry) = type_anims.get_anim(anim) {
                return &entry.bind_group;
            }
        }
        &self.fallback_bind_group
    }

    /// Check if a unit type has a specific animation loaded.
    pub fn has_anim(&self, unit_type_name: &str, anim: &str) -> bool {
        self.entries
            .get(unit_type_name)
            .is_some_and(|ta| ta.anims.contains_key(anim))
    }

    /// Get the cycle duration for a unit type's animation (from INI [Timing]).
    /// Returns 0.0 if not found.
    #[allow(dead_code)]
    pub fn cycle_duration(&self, unit_type_name: &str, anim: &str) -> f32 {
        self.entries
            .get(unit_type_name)
            .and_then(|ta| ta.anims.get(anim))
            .map_or(0.0, |e| e.cycle_duration)
    }

    /// Returns per-unit-type RUN cycle durations for all unit types that have a RUN animation.
    pub fn run_durations(&self) -> HashMap<String, f32> {
        let mut result = HashMap::new();
        for (name, type_anims) in &self.entries {
            if let Some(entry) = type_anims.anims.get("RUN") {
                if entry.cycle_duration > 0.0 {
                    result.insert(name.clone(), entry.cycle_duration);
                }
            }
        }
        result
    }

    /// Returns per-unit-type cycle durations for a given animation name
    /// (e.g. "FORTIFY", "ROAD", "MINE").
    pub fn anim_durations(&self, anim_name: &str) -> HashMap<String, f32> {
        let mut result = HashMap::new();
        for (name, type_anims) in &self.entries {
            if let Some(entry) = type_anims.anims.get(anim_name) {
                if entry.cycle_duration > 0.0 {
                    result.insert(name.clone(), entry.cycle_duration);
                }
            }
        }
        result
    }

    /// Returns all unique animation names across all unit types, excluding DEFAULT and RUN
    /// (which are handled separately).
    pub fn all_anim_names(&self) -> Vec<String> {
        let mut names = std::collections::BTreeSet::new();
        for type_anims in self.entries.values() {
            for anim_name in type_anims.anims.keys() {
                if anim_name != "DEFAULT" && anim_name != "RUN" {
                    names.insert(anim_name.clone());
                }
            }
        }
        names.into_iter().collect()
    }
}

/// Load a unit type's animations from an INI file (DEFAULT, RUN, etc.).
fn load_unit_from_ini(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    resource_dir: &Path,
    art_ini_rel: &str,
) -> Option<UnitTypeAnims> {
    // Resolve the INI file path case-insensitively
    let ini_path = resolve_case_insensitive(resource_dir, art_ini_rel)?;

    let ini_content = match std::fs::read_to_string(&ini_path) {
        Ok(s) => s,
        Err(e) => {
            log::warn!("Failed to read INI {}: {}", ini_path.display(), e);
            return None;
        }
    };

    let sections = parse_ini(&ini_content);
    let animations = sections.get("Animations")?;
    let timing = sections.get("Timing");

    let ini_dir = ini_path.parent()?;
    let mut anims = HashMap::new();

    for (anim_name, flc_filename) in animations {
        if flc_filename.is_empty() {
            continue;
        }

        let flc_path = match find_file_case_insensitive(ini_dir, flc_filename) {
            Some(p) => p,
            None => continue,
        };

        if let Some(mut entry) = load_flic_atlas(device, queue, layout, &flc_path) {
            // Fill in cycle_duration from [Timing] section
            if let Some(timing_section) = timing {
                if let Some(duration_str) = timing_section.get(anim_name.as_str()) {
                    entry.cycle_duration = duration_str.parse::<f32>().unwrap_or(0.0);
                }
            }

            log::info!(
                "  Loaded {} animation: {}x{} ({}dirs x {}frames, sprite {}x{}, duration {:.3}s)",
                anim_name,
                entry.atlas_w,
                entry.atlas_h,
                entry.num_directions,
                entry.frames_per_dir,
                entry.sprite_w,
                entry.sprite_h,
                entry.cycle_duration,
            );
            anims.insert(anim_name.clone(), entry);
        }
    }

    if anims.is_empty() {
        return None;
    }

    Some(UnitTypeAnims { anims })
}

/// Load a FLIC file and create a GPU atlas entry from it.
fn load_flic_atlas(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    flc_path: &Path,
) -> Option<UnitAnimEntry> {
    let file_data = match std::fs::read(flc_path) {
        Ok(data) => data,
        Err(e) => {
            log::warn!("Failed to read FLIC {}: {}", flc_path.display(), e);
            return None;
        }
    };

    let flic = match fc3_flic::read_flic(&file_data) {
        Ok(f) => f,
        Err(e) => {
            log::warn!("Failed to decode FLIC {}: {:?}", flc_path.display(), e);
            return None;
        }
    };

    let num_directions = flic.num_animations as u32;
    let frames_per_dir = flic.animation_length as u32;
    let sprite_w = flic.width as u32;
    let sprite_h = flic.height as u32;

    if num_directions == 0 || frames_per_dir == 0 {
        log::warn!(
            "FLIC has 0 directions or 0 frames per direction: {}",
            flc_path.display()
        );
        return None;
    }

    let atlas_w = sprite_w * num_directions;
    let atlas_h = sprite_h * frames_per_dir;
    let bytes_per_pixel = 4u32;
    let bytes_per_frame = (sprite_w * sprite_h * bytes_per_pixel) as usize;

    let mut atlas_data = vec![0u8; (atlas_w * atlas_h * bytes_per_pixel) as usize];

    // Copy each frame into its grid cell, skipping ring frames.
    // FLIC frames are stored sequentially: direction 0 frames then direction 1 frames, etc.
    // Each direction has (animation_length + 1) frames; the last is the ring frame (duplicate).
    let total_per_dir = frames_per_dir + 1; // including ring frame
    for dir in 0..num_directions {
        for frame in 0..frames_per_dir {
            let src_frame_idx = (dir * total_per_dir + frame) as usize;
            let src_offset = src_frame_idx * bytes_per_frame;

            if src_offset + bytes_per_frame > flic.frame_data.len() {
                continue;
            }

            // Destination: column=dir, row=frame in the atlas grid
            let dst_x = dir * sprite_w;
            let dst_y = frame * sprite_h;

            for row in 0..sprite_h {
                let src_row_start = src_offset + (row * sprite_w * bytes_per_pixel) as usize;
                let dst_row_start =
                    ((dst_y + row) * atlas_w + dst_x) as usize * bytes_per_pixel as usize;
                let row_bytes = (sprite_w * bytes_per_pixel) as usize;

                if src_row_start + row_bytes <= flic.frame_data.len()
                    && dst_row_start + row_bytes <= atlas_data.len()
                {
                    atlas_data[dst_row_start..dst_row_start + row_bytes].copy_from_slice(
                        &flic.frame_data[src_row_start..src_row_start + row_bytes],
                    );
                }
            }
        }
    }

    let (bind_group, _) =
        create_unit_bind_group(device, queue, layout, atlas_w, atlas_h, &atlas_data);

    Some(UnitAnimEntry {
        bind_group,
        num_directions,
        frames_per_dir,
        sprite_w,
        sprite_h,
        atlas_w,
        atlas_h,
        cycle_duration: 0.0,
    })
}

/// Resolve a relative path against a base directory, matching each path component
/// case-insensitively. Returns None if any component can't be matched.
fn resolve_case_insensitive(base: &Path, relative: &str) -> Option<PathBuf> {
    // First try the exact path (fast path for case-sensitive filesystems with correct casing)
    let exact = base.join(relative);
    if exact.exists() {
        return Some(exact);
    }

    // Walk each component case-insensitively
    let mut current = base.to_path_buf();
    for component in Path::new(relative).components() {
        let target = component.as_os_str().to_string_lossy().to_lowercase();
        let mut found = false;
        if let Ok(entries) = std::fs::read_dir(&current) {
            for entry in entries.flatten() {
                if entry.file_name().to_string_lossy().to_lowercase() == target {
                    current = entry.path();
                    found = true;
                    break;
                }
            }
        }
        if !found {
            log::warn!(
                "Case-insensitive lookup failed for '{}' in {}",
                component.as_os_str().to_string_lossy(),
                current.display()
            );
            return None;
        }
    }
    Some(current)
}

/// Find a file by name in a directory, matching case-insensitively.
fn find_file_case_insensitive(dir: &Path, filename: &str) -> Option<PathBuf> {
    // Fast path: try exact name first
    let exact = dir.join(filename);
    if exact.exists() {
        return Some(exact);
    }

    // Case-insensitive search
    let target = filename.to_lowercase();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().to_lowercase() == target {
                return Some(entry.path());
            }
        }
    }

    log::warn!(
        "Case-insensitive lookup failed for '{}' in {}",
        filename,
        dir.display()
    );
    None
}

/// Create the shared bind group layout for unit atlas textures.
fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("unit_atlas_layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

/// Create a GPU texture and bind group for a unit atlas.
fn create_unit_bind_group(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    width: u32,
    height: u32,
    rgba_data: &[u8],
) -> (wgpu::BindGroup, wgpu::Texture) {
    let texture_size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("unit_atlas"),
        size: texture_size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        rgba_data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: Some(height),
        },
        texture_size,
    );

    let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("unit_sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("unit_atlas_bind_group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
        ],
    });

    (bind_group, texture)
}
