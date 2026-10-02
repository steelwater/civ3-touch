use std::path::Path;

/// A GPU texture loaded from a PCX file, for use as a UI atlas.
pub struct UIAtlas {
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub bind_group: wgpu::BindGroup,
    pub width: u32,
    pub height: u32,
}

impl UIAtlas {
    /// Load multiple PCX files and stack them vertically into one atlas.
    /// All PCX files must have the same width. The combined height is the sum.
    /// Returns the atlas plus the height of each individual layer (for UV offsets).
    pub fn from_pcx_stacked(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pcx_paths: &[&Path],
    ) -> (Self, u32) {
        let mut images: Vec<(u32, u32, Vec<u8>)> = Vec::new();
        for path in pcx_paths {
            match std::fs::read(path) {
                Ok(file_data) => match fc3_pcx::read_pcx_with_transparency(&file_data, |idx| {
                    idx >= 254 || idx == 70
                }) {
                    Ok(image) => {
                        images.push((image.width as u32, image.height as u32, image.data));
                    }
                    Err(e) => {
                        log::warn!("Failed to decode PCX {}: {}", path.display(), e);
                        return (Self::fallback(device, queue), 1);
                    }
                },
                Err(e) => {
                    log::warn!("Failed to read PCX {}: {}", path.display(), e);
                    return (Self::fallback(device, queue), 1);
                }
            }
        }
        if images.is_empty() {
            return (Self::fallback(device, queue), 1);
        }
        let w = images[0].0;
        let layer_h = images[0].1;
        let total_h: u32 = images.iter().map(|(_, h, _)| *h).sum();
        let mut combined = Vec::with_capacity((w * total_h * 4) as usize);
        for (_, _, data) in &images {
            combined.extend_from_slice(data);
        }
        let (layout, bg) = create_atlas_texture(device, queue, w, total_h, &combined);
        log::info!(
            "Loaded stacked UI atlas: {}x{} ({} layers)",
            w,
            total_h,
            images.len()
        );
        (
            UIAtlas {
                bind_group_layout: layout,
                bind_group: bg,
                width: w,
                height: total_h,
            },
            layer_h,
        )
    }

    /// Load a texture from a color PCX and an alpha PCX.
    /// The color PCX provides RGB; the alpha PCX provides alpha (from any color channel,
    /// mapped linearly: 255 = fully opaque, 0 = fully transparent).
    pub fn from_color_alpha_pcx(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_path: &Path,
        alpha_path: &Path,
    ) -> Option<Self> {
        let color_data = match std::fs::read(color_path) {
            Ok(d) => d,
            Err(e) => {
                log::warn!("Failed to read color PCX {}: {}", color_path.display(), e);
                return None;
            }
        };
        let alpha_data = match std::fs::read(alpha_path) {
            Ok(d) => d,
            Err(e) => {
                log::warn!("Failed to read alpha PCX {}: {}", alpha_path.display(), e);
                return None;
            }
        };
        let color_img = match fc3_pcx::read_pcx(&color_data) {
            Ok(img) => img,
            Err(e) => {
                log::warn!("Failed to decode color PCX {}: {}", color_path.display(), e);
                return None;
            }
        };
        let alpha_img = match fc3_pcx::read_pcx(&alpha_data) {
            Ok(img) => img,
            Err(e) => {
                log::warn!("Failed to decode alpha PCX {}: {}", alpha_path.display(), e);
                return None;
            }
        };
        let w = color_img.width as u32;
        let h = color_img.height as u32;
        if alpha_img.width as u32 != w || alpha_img.height as u32 != h {
            log::warn!(
                "Color/alpha PCX dimension mismatch: {}x{} vs {}x{}",
                w,
                h,
                alpha_img.width,
                alpha_img.height
            );
            return None;
        }
        // Combine: RGB from color, A from alpha's R channel (grayscale)
        let pixel_count = (w * h) as usize;
        let mut rgba = Vec::with_capacity(pixel_count * 4);
        for i in 0..pixel_count {
            let ci = i * 4;
            let r = color_img.data[ci];
            let g = color_img.data[ci + 1];
            let b = color_img.data[ci + 2];
            let a = alpha_img.data[ci]; // R channel of alpha image
            rgba.push(r);
            rgba.push(g);
            rgba.push(b);
            rgba.push(a);
        }
        let (layout, bg) = create_atlas_texture(device, queue, w, h, &rgba);
        log::info!(
            "Loaded color+alpha UI atlas: {}x{} from {} + {}",
            w,
            h,
            color_path.display(),
            alpha_path.display()
        );
        Some(UIAtlas {
            bind_group_layout: layout,
            bind_group: bg,
            width: w,
            height: h,
        })
    }

    /// Load a single PCX file with a caller-provided transparency predicate.
    pub fn from_pcx_single(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pcx_path: &Path,
        is_transparent: impl Fn(u8) -> bool,
    ) -> Option<Self> {
        let file_data = match std::fs::read(pcx_path) {
            Ok(d) => d,
            Err(e) => {
                log::warn!("Failed to read PCX {}: {}", pcx_path.display(), e);
                return None;
            }
        };
        let image = match fc3_pcx::read_pcx_with_transparency(&file_data, is_transparent) {
            Ok(img) => img,
            Err(e) => {
                log::warn!("Failed to decode PCX {}: {}", pcx_path.display(), e);
                return None;
            }
        };
        let w = image.width as u32;
        let h = image.height as u32;
        let (layout, bg) = create_atlas_texture(device, queue, w, h, &image.data);
        log::info!(
            "Loaded single PCX UI atlas: {}x{} from {}",
            w,
            h,
            pcx_path.display()
        );
        Some(UIAtlas {
            bind_group_layout: layout,
            bind_group: bg,
            width: w,
            height: h,
        })
    }

    /// Create a 1x1 white fallback texture.
    pub fn fallback(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let (layout, bg) = create_atlas_texture(device, queue, 1, 1, &[255, 255, 255, 255]);
        UIAtlas {
            bind_group_layout: layout,
            bind_group: bg,
            width: 1,
            height: 1,
        }
    }
}

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
        label: Some("ui_atlas"),
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
        label: Some("ui_atlas_sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("ui_atlas_layout"),
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
        label: Some("ui_atlas_bind_group"),
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
