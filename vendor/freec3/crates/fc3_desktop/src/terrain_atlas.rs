use std::collections::HashSet;
use std::path::Path;

use fc3_core::tile::{Terrain, Vegetation};

/// Configuration for a Civ3 terrain PCX file.
/// Each PCX is a 9×9 grid of tiles encoding 3^4 = 81 neighbor-transition variants
/// for 3 terrain types.
struct TerrainFileConfig {
    filename: &'static str,
    terrains: [Terrain; 3],
}

/// Metadata for a loaded PCX file within the atlas.
struct LoadedFile {
    terrains: [Terrain; 3],
    tile_w: u32,
    tile_h: u32,
    y_offset: u32, // pixel offset in the vertical atlas
}

/// The 6 Civ3 terrain PCX files covering Grassland, Plains, Desert, Tundra, and Coast.
fn terrain_file_configs() -> Vec<TerrainFileConfig> {
    use Terrain::*;
    vec![
        TerrainFileConfig {
            filename: "xggc.pcx",
            terrains: [Grassland, Grassland, Coast],
        },
        TerrainFileConfig {
            filename: "xtgc.pcx",
            terrains: [Tundra, Grassland, Coast],
        },
        TerrainFileConfig {
            filename: "xdgc.pcx",
            terrains: [Desert, Grassland, Coast],
        },
        TerrainFileConfig {
            filename: "xdgp.pcx",
            terrains: [Desert, Grassland, Plains],
        },
        TerrainFileConfig {
            filename: "xdpc.pcx",
            terrains: [Desert, Plains, Coast],
        },
        TerrainFileConfig {
            filename: "xpgc.pcx",
            terrains: [Plains, Grassland, Coast],
        },
    ]
}

/// Configuration for a vegetation (forest/jungle) PCX file.
/// Each file has 4 rows × N columns of 126×81 sprites.
/// Rows: 0=jungle large, 1=jungle small, 2=forest large, 3=forest small.
/// We use row 2 (forest large) and row 0 (jungle large), 4 variants each.
struct VegetationFileConfig {
    filename: &'static str,
    terrain: Terrain,
}

/// The vegetation PCX files for each base terrain type.
fn vegetation_file_configs() -> Vec<VegetationFileConfig> {
    vec![
        VegetationFileConfig {
            filename: "grassland forests.pcx",
            terrain: Terrain::Grassland,
        },
        VegetationFileConfig {
            filename: "plains forests.pcx",
            terrain: Terrain::Plains,
        },
        VegetationFileConfig {
            filename: "tundra forests.pcx",
            terrain: Terrain::Tundra,
        },
        VegetationFileConfig {
            filename: "hill forests.pcx",
            terrain: Terrain::Hill,
        },
    ]
}

/// Metadata for a loaded vegetation overlay file within the atlas.
struct VegetationFile {
    terrain: Terrain,
    tile_w: u32,
    tile_h: u32,
    cols: u32,     // number of variants per row
    y_offset: u32, // pixel offset in the vertical atlas
}

/// Metadata for a loaded overlay file (e.g. hills) within the atlas.
struct OverlayFile {
    tile_w: u32,
    tile_h: u32,
    grid_cols: u32,
    y_offset: u32, // pixel offset in the vertical atlas
}

/// Holds a GPU texture atlas of terrain tiles and per-file metadata for variant lookup.
pub struct TerrainAtlas {
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub bind_group: wgpu::BindGroup,
    files: Vec<LoadedFile>,
    hill: Option<OverlayFile>,
    fog: Option<OverlayFile>,
    veg_files: Vec<VegetationFile>,
    atlas_w: u32,
    atlas_h: u32,
}

impl TerrainAtlas {
    /// Create a 1×1 white fallback texture (no terrain textures loaded).
    pub fn fallback(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let (bind_group_layout, bind_group) =
            create_atlas_texture(device, queue, 1, 1, &[255, 255, 255, 255]);
        TerrainAtlas {
            bind_group_layout,
            bind_group,
            files: Vec::new(),
            hill: None,
            fog: None,
            veg_files: Vec::new(),
            atlas_w: 1,
            atlas_h: 1,
        }
    }

    /// Load terrain textures from a Civ3 resource directory and build the atlas.
    /// Each PCX file is loaded as a full image and stacked vertically.
    pub fn from_resource_dir(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resource_dir: &Path,
    ) -> Self {
        let configs = terrain_file_configs();
        let terrain_art_dir = resource_dir.join("Art").join("Terrain");

        // Load each PCX file as a full RGBA image
        struct RawFile {
            terrains: [Terrain; 3],
            width: u32,
            height: u32,
            data: Vec<u8>,
        }

        let mut raw_files: Vec<RawFile> = Vec::new();
        for config in &configs {
            let pcx_path = terrain_art_dir.join(config.filename);
            match std::fs::read(&pcx_path) {
                Ok(file_data) => match fc3_pcx::read_pcx(&file_data) {
                    Ok(image) => {
                        raw_files.push(RawFile {
                            terrains: config.terrains,
                            width: image.width as u32,
                            height: image.height as u32,
                            data: image.data,
                        });
                    }
                    Err(e) => {
                        log::warn!("Failed to decode PCX {}: {}", pcx_path.display(), e);
                    }
                },
                Err(e) => {
                    log::warn!("Failed to read {}: {}", pcx_path.display(), e);
                }
            }
        }

        if raw_files.is_empty() {
            log::warn!("No terrain PCX files loaded, using fallback");
            return Self::fallback(device, queue);
        }

        // Load hill overlay (xhills.pcx) — 4×4 grid
        struct RawOverlay {
            width: u32,
            height: u32,
            data: Vec<u8>,
        }
        let mut hill_raw: Option<RawOverlay> = None;
        let hills_path = terrain_art_dir.join("xhills.pcx");
        match std::fs::read(&hills_path) {
            Ok(file_data) => match fc3_pcx::read_pcx(&file_data) {
                Ok(image) => {
                    hill_raw = Some(RawOverlay {
                        width: image.width as u32,
                        height: image.height as u32,
                        data: image.data,
                    });
                }
                Err(e) => {
                    log::warn!("Failed to decode xhills.pcx: {}", e);
                }
            },
            Err(e) => {
                log::warn!("Failed to read xhills.pcx: {}", e);
            }
        }

        // Load fog of war overlay (FogOfWar.pcx) — 9×9 grid
        let mut fog_raw: Option<RawOverlay> = None;
        let fog_path = terrain_art_dir.join("FogOfWar.pcx");
        match std::fs::read(&fog_path) {
            Ok(file_data) => match fc3_pcx::read_pcx(&file_data) {
                Ok(image) => {
                    fog_raw = Some(RawOverlay {
                        width: image.width as u32,
                        height: image.height as u32,
                        data: image.data,
                    });
                }
                Err(e) => {
                    log::warn!("Failed to decode FogOfWar.pcx: {}", e);
                }
            },
            Err(e) => {
                log::warn!("Failed to read FogOfWar.pcx: {}", e);
            }
        }

        // Load vegetation overlay PCX files (forest/jungle per terrain)
        struct RawVeg {
            terrain: Terrain,
            width: u32,
            height: u32,
            data: Vec<u8>,
        }
        let mut veg_raws: Vec<RawVeg> = Vec::new();
        for config in &vegetation_file_configs() {
            let pcx_path = terrain_art_dir.join(config.filename);
            match std::fs::read(&pcx_path) {
                Ok(file_data) => match fc3_pcx::read_pcx(&file_data) {
                    Ok(image) => {
                        veg_raws.push(RawVeg {
                            terrain: config.terrain,
                            width: image.width as u32,
                            height: image.height as u32,
                            data: image.data,
                        });
                    }
                    Err(e) => {
                        log::warn!("Failed to decode {}: {}", config.filename, e);
                    }
                },
                Err(e) => {
                    log::warn!("Failed to read {}: {}", pcx_path.display(), e);
                }
            }
        }

        // All PCX files should have the same width; use the first as canonical
        let atlas_w = raw_files[0].width;
        let mut atlas_h: u32 = 0;
        let mut files: Vec<LoadedFile> = Vec::new();

        for raw in &raw_files {
            let tile_w = raw.width / 9;
            let tile_h = raw.height / 9;
            files.push(LoadedFile {
                terrains: raw.terrains,
                tile_w,
                tile_h,
                y_offset: atlas_h,
            });
            atlas_h += raw.height;
        }

        // Add hill overlay to atlas height
        let hill = hill_raw.as_ref().map(|h| {
            let ov = OverlayFile {
                tile_w: h.width / 4,
                tile_h: h.height / 4,
                grid_cols: 4,
                y_offset: atlas_h,
            };
            atlas_h += h.height;
            ov
        });

        // Add fog overlay to atlas height
        let fog = fog_raw.as_ref().map(|f| {
            let ov = OverlayFile {
                tile_w: f.width / 9,
                tile_h: f.height / 9,
                grid_cols: 9,
                y_offset: atlas_h,
            };
            atlas_h += f.height;
            ov
        });

        // Add vegetation overlays to atlas height
        let mut veg_files: Vec<VegetationFile> = Vec::new();
        for raw in &veg_raws {
            // Vegetation files: 4 rows of sprites, N columns per row
            // Sprite size is 126×81, rows: jungle-large, jungle-small, forest-large, forest-small
            let tile_w = 126u32;
            let rows = 4u32;
            let cols = raw.width / tile_w;
            // We only use 4 variants per type
            let cols = cols.min(4);
            veg_files.push(VegetationFile {
                terrain: raw.terrain,
                tile_w,
                tile_h: raw.height / rows,
                cols,
                y_offset: atlas_h,
            });
            atlas_h += raw.height;
        }

        // Build vertical atlas: stack all PCX images top to bottom
        let mut atlas_data = vec![0u8; (atlas_w * atlas_h * 4) as usize];
        let mut y_cursor: u32 = 0;
        for raw in &raw_files {
            let copy_w = raw.width.min(atlas_w);
            for row in 0..raw.height {
                let src_start = (row * raw.width * 4) as usize;
                let src_end = src_start + (copy_w * 4) as usize;
                let dst_start = ((y_cursor + row) * atlas_w * 4) as usize;
                let dst_end = dst_start + (copy_w * 4) as usize;
                if src_end <= raw.data.len() && dst_end <= atlas_data.len() {
                    atlas_data[dst_start..dst_end].copy_from_slice(&raw.data[src_start..src_end]);
                }
            }
            y_cursor += raw.height;
        }

        // Append hill overlay image
        if let Some(h) = &hill_raw {
            let copy_w = h.width.min(atlas_w);
            for row in 0..h.height {
                let src_start = (row * h.width * 4) as usize;
                let src_end = src_start + (copy_w * 4) as usize;
                let dst_start = ((y_cursor + row) * atlas_w * 4) as usize;
                let dst_end = dst_start + (copy_w * 4) as usize;
                if src_end <= h.data.len() && dst_end <= atlas_data.len() {
                    atlas_data[dst_start..dst_end].copy_from_slice(&h.data[src_start..src_end]);
                }
            }
            y_cursor += h.height;
        }

        // Append fog overlay image
        if let Some(f) = &fog_raw {
            let copy_w = f.width.min(atlas_w);
            for row in 0..f.height {
                let src_start = (row * f.width * 4) as usize;
                let src_end = src_start + (copy_w * 4) as usize;
                let dst_start = ((y_cursor + row) * atlas_w * 4) as usize;
                let dst_end = dst_start + (copy_w * 4) as usize;
                if src_end <= f.data.len() && dst_end <= atlas_data.len() {
                    atlas_data[dst_start..dst_end].copy_from_slice(&f.data[src_start..src_end]);
                }
            }
            y_cursor += f.height;
        }

        // Append vegetation overlay images
        for raw in &veg_raws {
            let copy_w = raw.width.min(atlas_w);
            for row in 0..raw.height {
                let src_start = (row * raw.width * 4) as usize;
                let src_end = src_start + (copy_w * 4) as usize;
                let dst_start = ((y_cursor + row) * atlas_w * 4) as usize;
                let dst_end = dst_start + (copy_w * 4) as usize;
                if src_end <= raw.data.len() && dst_end <= atlas_data.len() {
                    atlas_data[dst_start..dst_end]
                        .copy_from_slice(&raw.data[src_start..src_end]);
                }
            }
            y_cursor += raw.height;
        }

        log::info!(
            "Built terrain atlas: {}x{} with {} terrain + {} overlay + {} vegetation PCX file(s)",
            atlas_w,
            atlas_h,
            files.len(),
            u32::from(hill.is_some()) + u32::from(fog.is_some()),
            veg_files.len(),
        );

        let (bind_group_layout, bind_group) =
            create_atlas_texture(device, queue, atlas_w, atlas_h, &atlas_data);

        TerrainAtlas {
            bind_group_layout,
            bind_group,
            files,
            hill,
            fog,
            veg_files,
            atlas_w,
            atlas_h,
        }
    }

    /// Returns atlas UV rect for a junction between 4 tiles.
    ///
    /// `terrains` = [N, W, E, S] where each is the terrain of one of the 4
    /// grid cells meeting at this junction point. Unlike `get_rect`, there is
    /// no "center" terrain — the PCX tile represents the junction itself.
    pub fn get_junction_rect(&self, terrains: [Terrain; 4]) -> [f32; 4] {
        if self.files.is_empty() {
            return [0.0; 4];
        }

        let file = match self.select_file_for_junction(&terrains) {
            Some(f) => f,
            None => return [0.0; 4],
        };

        // Find a fallback slot: first junction terrain present in the file.
        let fallback_slot = terrains
            .iter()
            .find_map(|&t| {
                file.terrains
                    .iter()
                    .position(|&ft| ft == t)
                    .map(|i| i as u32)
            })
            .unwrap_or(0);

        let n_slot = terrain_slot_or(&file.terrains, terrains[0], fallback_slot);
        let w_slot = terrain_slot_or(&file.terrains, terrains[1], fallback_slot);
        let e_slot = terrain_slot_or(&file.terrains, terrains[2], fallback_slot);
        let s_slot = terrain_slot_or(&file.terrains, terrains[3], fallback_slot);

        let index = n_slot + w_slot * 3 + e_slot * 9 + s_slot * 27;
        let col = index % 9;
        let row = index / 9;

        let px_x = col * file.tile_w;
        let px_y = file.y_offset + row * file.tile_h;

        [
            px_x as f32 / self.atlas_w as f32,
            px_y as f32 / self.atlas_h as f32,
            file.tile_w as f32 / self.atlas_w as f32,
            file.tile_h as f32 / self.atlas_h as f32,
        ]
    }

    /// Select the best PCX file for a junction of 4 terrains (no center requirement).
    /// Scores files by how many of the 4 junction terrains they contain.
    /// Tiebreak: prefer files containing terrains[0] (the N terrain).
    fn select_file_for_junction(&self, terrains: &[Terrain; 4]) -> Option<&LoadedFile> {
        // Check that at least one junction terrain is a PCX terrain
        if !terrains.iter().any(|t| is_pcx_terrain(*t)) {
            return None;
        }

        let mut best: Option<&LoadedFile> = None;
        let mut best_score: i32 = -1;

        for file in &self.files {
            let file_set: HashSet<Terrain> = file.terrains.iter().copied().collect();

            // Score: how many of the 4 junction terrains does this file cover?
            let mut score: i32 = 0;
            for t in terrains {
                if is_pcx_terrain(*t) && file_set.contains(t) {
                    score += 1;
                }
            }

            if score > best_score
                || (score == best_score && file_set.contains(&terrains[0]))
            {
                best_score = score;
                best = Some(file);
            }
        }

        best
    }

    /// Returns atlas UV rect for a hill overlay tile given 4 diagonal neighbors.
    ///
    /// `diag_elevated` = [NW, NE, SE, SW] — true if that diagonal neighbor is
    /// a Hill or Mountain.
    ///
    /// The xhills.pcx is a 4×4 grid indexed by a 4-bit bitmask:
    ///   NW=1, NE=2, SE=4, SW=8
    ///   col = mask % 4,  row = mask / 4
    pub fn get_hill_rect(&self, diag_elevated: [bool; 4]) -> [f32; 4] {
        let hill = match &self.hill {
            Some(h) => h,
            None => return [0.0; 4],
        };

        let mask = diag_elevated[0] as u32       // NW = 1
            | (diag_elevated[1] as u32) << 1     // NE = 2
            | (diag_elevated[2] as u32) << 2     // SE = 4
            | (diag_elevated[3] as u32) << 3; // SW = 8

        let col = mask % hill.grid_cols;
        let row = mask / hill.grid_cols;

        let px_x = col * hill.tile_w;
        let px_y = hill.y_offset + row * hill.tile_h;

        [
            px_x as f32 / self.atlas_w as f32,
            px_y as f32 / self.atlas_h as f32,
            hill.tile_w as f32 / self.atlas_w as f32,
            hill.tile_h as f32 / self.atlas_h as f32,
        ]
    }

    /// Returns atlas UV rect for a fog-of-war overlay tile.
    ///
    /// `vis` = [N, W, E, S] where 0=Unseen, 1=Revealed, 2=Visible.
    /// Index formula: `n + w*3 + e*9 + s*27` (same 3^4 encoding as terrain).
    /// The FogOfWar.pcx is a 9×9 grid of 81 transition tiles.
    pub fn get_fog_rect(&self, vis: [u32; 4]) -> [f32; 4] {
        let fog = match &self.fog {
            Some(f) => f,
            None => return [0.0; 4],
        };

        let index = vis[0] + vis[1] * 3 + vis[2] * 9 + vis[3] * 27;
        let col = index % fog.grid_cols;
        let row = index / fog.grid_cols;

        let px_x = col * fog.tile_w;
        let px_y = fog.y_offset + row * fog.tile_h;

        [
            px_x as f32 / self.atlas_w as f32,
            px_y as f32 / self.atlas_h as f32,
            fog.tile_w as f32 / self.atlas_w as f32,
            fog.tile_h as f32 / self.atlas_h as f32,
        ]
    }
    /// Returns atlas UV rect for a vegetation overlay sprite.
    ///
    /// `terrain` = the base terrain type (Grassland, Plains, Tundra, Hill).
    /// `vegetation` = Forest or Jungle.
    /// `variant` = which variant to use (0..3), typically `(x + y) % 4`.
    ///
    /// Row layout in each PCX: 0=jungle-large, 1=jungle-small, 2=forest-large, 3=forest-small.
    /// We use row 2 for forest, row 0 for jungle ("large" variants).
    pub fn get_vegetation_rect(
        &self,
        terrain: Terrain,
        vegetation: Vegetation,
        variant: u32,
    ) -> [f32; 4] {
        if vegetation == Vegetation::None {
            return [0.0; 4];
        }

        // Find the vegetation file for this terrain
        let vf = match self.veg_files.iter().find(|v| v.terrain == terrain) {
            Some(f) => f,
            None => {
                // Fall back to grassland for terrains without a specific file
                match self.veg_files.iter().find(|v| v.terrain == Terrain::Grassland) {
                    Some(f) => f,
                    None => return [0.0; 4],
                }
            }
        };

        // Row: 0=jungle-large, 2=forest-large
        let row = match vegetation {
            Vegetation::Forest => 2,
            Vegetation::Jungle => 0,
            Vegetation::None => return [0.0; 4],
        };

        let col = variant % vf.cols;
        let px_x = col * vf.tile_w;
        let px_y = vf.y_offset + row * vf.tile_h;

        [
            px_x as f32 / self.atlas_w as f32,
            px_y as f32 / self.atlas_h as f32,
            vf.tile_w as f32 / self.atlas_w as f32,
            vf.tile_h as f32 / self.atlas_h as f32,
        ]
    }
}

/// Terrains that have PCX transition files.
fn is_pcx_terrain(t: Terrain) -> bool {
    matches!(
        t,
        Terrain::Grassland | Terrain::Plains | Terrain::Desert | Terrain::Tundra | Terrain::Coast
    )
}

/// Find the slot for a terrain, or return a fallback slot if not found.
fn terrain_slot_or(terrains: &[Terrain; 3], target: Terrain, fallback: u32) -> u32 {
    for (i, &t) in terrains.iter().enumerate() {
        if t == target {
            return i as u32;
        }
    }
    fallback
}

/// Create a GPU texture + sampler + bind group for the terrain atlas.
fn create_atlas_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    width: u32,
    height: u32,
    rgba_data: &[u8],
) -> (wgpu::BindGroupLayout, wgpu::BindGroup) {
    let texture_size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("terrain_atlas"),
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
        label: Some("terrain_sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("terrain_atlas_layout"),
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
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("terrain_atlas_bind_group"),
        layout: &bind_group_layout,
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

    (bind_group_layout, bind_group)
}
