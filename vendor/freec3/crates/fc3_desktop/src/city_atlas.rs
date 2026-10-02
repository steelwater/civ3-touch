use std::path::Path;

/// Holds a GPU texture for city sprites loaded from a Civ3 PCX sprite sheet.
///
/// The sprite sheet (e.g. rEURO.PCX) is a 3×4 grid:
///   columns = city size (town, city, metropolis)
///   rows    = era (ancient, medieval, industrial, modern)
///   Each sprite is 167×95 pixels.
pub struct CityAtlas {
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub bind_group: wgpu::BindGroup,
    sprite_w: u32,
    sprite_h: u32,
    atlas_w: u32,
    atlas_h: u32,
}

const SPRITE_W: u32 = 167;
const SPRITE_H: u32 = 95;
const GRID_COLS: u32 = 3;
const GRID_ROWS: u32 = 4;

impl CityAtlas {
    /// Create a 1×1 transparent fallback (no city sprites loaded).
    pub fn fallback(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let (bind_group_layout, bind_group) =
            create_city_texture(device, queue, 1, 1, &[0, 0, 0, 0]);
        CityAtlas {
            bind_group_layout,
            bind_group,
            sprite_w: 1,
            sprite_h: 1,
            atlas_w: 1,
            atlas_h: 1,
        }
    }

    /// Load city sprites from a Civ3 resource directory.
    /// Looks for Art/Cities/rEURO.PCX (European cities).
    pub fn from_resource_dir(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resource_dir: &Path,
    ) -> Self {
        let pcx_path = resource_dir.join("Art").join("Cities").join("rEURO.PCX");

        let file_data = match std::fs::read(&pcx_path) {
            Ok(data) => data,
            Err(e) => {
                log::warn!("Failed to read {}: {}", pcx_path.display(), e);
                return Self::fallback(device, queue);
            }
        };

        let image = match fc3_pcx::read_pcx(&file_data) {
            Ok(img) => img,
            Err(e) => {
                log::warn!("Failed to decode {}: {}", pcx_path.display(), e);
                return Self::fallback(device, queue);
            }
        };

        let atlas_w = image.width as u32;
        let atlas_h = image.height as u32;

        log::info!(
            "Loaded city atlas: {}x{} from {}",
            atlas_w,
            atlas_h,
            pcx_path.display()
        );

        let (bind_group_layout, bind_group) =
            create_city_texture(device, queue, atlas_w, atlas_h, &image.data);

        CityAtlas {
            bind_group_layout,
            bind_group,
            sprite_w: SPRITE_W,
            sprite_h: SPRITE_H,
            atlas_w,
            atlas_h,
        }
    }

    /// Returns atlas UV rect [u, v, w, h] for a city sprite.
    /// `col` = size (0=town, 1=city, 2=metropolis)
    /// `row` = era (0=ancient, 1=medieval, 2=industrial, 3=modern)
    pub fn get_rect(&self, col: u32, row: u32) -> [f32; 4] {
        if self.atlas_w <= 1 {
            return [0.0; 4];
        }
        let col = col.min(GRID_COLS - 1);
        let row = row.min(GRID_ROWS - 1);
        let px_x = col * self.sprite_w;
        let px_y = row * self.sprite_h;
        [
            px_x as f32 / self.atlas_w as f32,
            px_y as f32 / self.atlas_h as f32,
            self.sprite_w as f32 / self.atlas_w as f32,
            self.sprite_h as f32 / self.atlas_h as f32,
        ]
    }
}

fn create_city_texture(
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
        label: Some("city_atlas"),
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
        label: Some("city_sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("city_atlas_layout"),
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
        label: Some("city_atlas_bind_group"),
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
