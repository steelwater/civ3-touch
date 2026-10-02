use std::path::Path;
use std::sync::Arc;

use wgpu::util::DeviceExt;
use winit::window::Window;

use crate::camera::CameraUniform;
use crate::city_atlas::CityAtlas;
use crate::instances::{
    CityInstance, PathLineInstance, SelectionCircleInstance, TileInstance, UIRectInstance,
    UITexturedRectInstance, UnitInstance,
};
use crate::terrain_atlas::TerrainAtlas;
use crate::ui_atlas::UIAtlas;
use crate::ui_component::{NineSliceId, NineSliceImage, NineSliceRegistry};
use crate::unit_atlas::UnitAtlasSet;

/// A batch of unit instances sharing the same texture atlas (same unit type + animation).
pub struct UnitDrawBatch {
    pub unit_type: String,
    pub anim_name: String,
    pub start: u32,
    pub count: u32,
}

pub struct Renderer {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub window: Arc<Window>,

    // Camera uniform
    pub camera_buffer: wgpu::Buffer,
    pub camera_bind_group: wgpu::BindGroup,

    // Minimap camera (separate uniform for minimap viewport)
    pub minimap_camera_buffer: wgpu::Buffer,
    pub minimap_camera_bind_group: wgpu::BindGroup,

    // Terrain texture atlas
    pub terrain_atlas: TerrainAtlas,

    // City sprite atlas
    pub city_atlas: CityAtlas,

    // Unit sprite atlases (per unit type)
    pub unit_atlas: UnitAtlasSet,

    // Pipelines
    pub tile_pipeline: wgpu::RenderPipeline,
    pub unit_pipeline: wgpu::RenderPipeline,
    pub city_pipeline: wgpu::RenderPipeline,

    // Path line pipeline (world-space, alpha blended)
    pub path_line_pipeline: wgpu::RenderPipeline,

    // Selection circle pipeline (world-space, alpha blended)
    pub selection_circle_pipeline: wgpu::RenderPipeline,

    // UI rect pipeline (no bind group — screen-space)
    pub ui_rect_pipeline: wgpu::RenderPipeline,

    // UI textured rect pipeline (bind group 0 = ui_atlas texture + sampler)
    // The atlas stacks NormButtons / RollOverButtons / HighlightedButtons vertically.
    pub ui_atlas: UIAtlas,
    pub ui_atlas_layer_height: u32,
    pub ui_textured_rect_pipeline: wgpu::RenderPipeline,

    // UI chrome atlases (box left / box right)
    pub box_left_atlas: Option<UIAtlas>,
    pub box_right_atlas: Option<UIAtlas>,
    pub box_left_size: [f32; 2],  // pixel dimensions
    pub box_right_size: [f32; 2], // pixel dimensions
    pub ui_chrome_buffer: Option<wgpu::Buffer>,
    pub ui_chrome_count: u32,
    // Which atlas each chrome instance uses: (atlas_index, start, count)
    // 0 = box_left, 1 = box_right
    pub ui_chrome_batches: Vec<(u32, u32, u32)>,

    // Instance buffers (rebuilt each frame)
    pub tile_buffer: Option<wgpu::Buffer>,
    pub tile_count: u32,
    pub unit_buffer: Option<wgpu::Buffer>,
    pub unit_count: u32,
    pub unit_batches: Vec<UnitDrawBatch>,
    pub city_buffer: Option<wgpu::Buffer>,
    pub city_count: u32,
    pub minimap_tile_buffer: Option<wgpu::Buffer>,
    pub minimap_tile_count: u32,
    pub minimap_overlay_buffer: Option<wgpu::Buffer>,
    pub minimap_overlay_count: u32,
    pub path_line_buffer: Option<wgpu::Buffer>,
    pub path_line_count: u32,
    pub selection_circle_buffer: Option<wgpu::Buffer>,
    pub selection_circle_count: u32,
    pub ui_rect_buffer: Option<wgpu::Buffer>,
    pub ui_rect_count: u32,
    pub ui_textured_rect_buffer: Option<wgpu::Buffer>,
    pub ui_textured_rect_count: u32,

    // Dialog atlas (9-slice panel background)
    pub dialog_atlas: Option<UIAtlas>,
    pub nine_slice_registry: NineSliceRegistry,
    pub dialog_nine_slice_buffer: Option<wgpu::Buffer>,
    pub dialog_nine_slice_count: u32,
    pub dialog_nine_slice_id: Option<NineSliceId>,

    // Advisor portrait atlas
    pub advisor_atlas: Option<UIAtlas>,
    pub advisor_buffer: Option<wgpu::Buffer>,
    pub advisor_count: u32,

    // Menu button atlas (single PCX with all states baked in)
    pub menu_button_atlas: Option<UIAtlas>,
    pub menu_button_buffer: Option<wgpu::Buffer>,
    pub menu_button_count: u32,
}

impl Renderer {
    pub fn new(window: Arc<Window>, resource_dir: Option<&Path>) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .unwrap();

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("fc3_device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        }))
        .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // Camera uniform buffer and bind group
        let camera_uniform = CameraUniform {
            center: [0.0, 0.0],
            zoom: 20.0,
            aspect: 1.0,
            viewport_w: size.width as f32,
            viewport_h: size.height as f32,
            map_height: 0.0,
            wrap_y: 0.0,
        };
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("camera_buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("camera_bind_group_layout"),
            });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
            label: Some("camera_bind_group"),
        });

        // Minimap camera
        let minimap_camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("minimap_camera_buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let minimap_camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: minimap_camera_buffer.as_entire_binding(),
            }],
            label: Some("minimap_camera_bind_group"),
        });

        // Terrain texture atlas
        let terrain_atlas = match resource_dir {
            Some(dir) => TerrainAtlas::from_resource_dir(&device, &queue, dir),
            None => TerrainAtlas::fallback(&device, &queue),
        };

        // City sprite atlas
        let city_atlas = match resource_dir {
            Some(dir) => CityAtlas::from_resource_dir(&device, &queue, dir),
            None => CityAtlas::fallback(&device, &queue),
        };

        // Unit sprite atlases (per unit type, loaded from INI → FLIC)
        let unit_atlas = match resource_dir {
            Some(dir) => UnitAtlasSet::from_resource_dir(&device, &queue, dir, &[]),
            None => UnitAtlasSet::fallback(&device, &queue),
        };

        // Create pipelines — tile pipeline uses two bind group layouts (camera + atlas)
        let tile_pipeline = Self::create_tile_pipeline(
            &device,
            &camera_bind_group_layout,
            &terrain_atlas.bind_group_layout,
            surface_format,
        );
        let unit_pipeline = Self::create_textured_pipeline(
            &device,
            &camera_bind_group_layout,
            &unit_atlas.bind_group_layout,
            surface_format,
            "unit",
            include_str!("shaders/unit.wgsl"),
            UnitInstance::desc(),
        );
        let city_pipeline = Self::create_textured_pipeline(
            &device,
            &camera_bind_group_layout,
            &city_atlas.bind_group_layout,
            surface_format,
            "city",
            include_str!("shaders/city.wgsl"),
            CityInstance::desc(),
        );

        let path_line_pipeline = Self::create_pipeline(
            &device,
            &camera_bind_group_layout,
            surface_format,
            "path_line",
            include_str!("shaders/path_line.wgsl"),
            PathLineInstance::desc(),
            true,
        );

        let selection_circle_pipeline = Self::create_pipeline(
            &device,
            &camera_bind_group_layout,
            surface_format,
            "selection_circle",
            include_str!("shaders/selection_circle.wgsl"),
            SelectionCircleInstance::desc(),
            true,
        );

        let ui_rect_pipeline = Self::create_ui_rect_pipeline(&device, surface_format);

        // UI texture atlas: stack NormButtons / RollOverButtons / HighlightedButtons
        let (ui_atlas, ui_atlas_layer_height) = match resource_dir {
            Some(dir) => {
                let iface = dir.join("Art").join("interface");
                let norm = iface.join("NormButtons.pcx");
                let hover = iface.join("RollOverButtons.pcx");
                let active = iface.join("HighlightedButtons.pcx");
                UIAtlas::from_pcx_stacked(
                    &device,
                    &queue,
                    &[norm.as_path(), hover.as_path(), active.as_path()],
                )
            }
            None => (UIAtlas::fallback(&device, &queue), 1),
        };
        let ui_textured_rect_pipeline = Self::create_ui_textured_rect_pipeline(
            &device,
            &ui_atlas.bind_group_layout,
            surface_format,
        );

        // Load box left/right chrome atlases (color + alpha PCX pairs)
        let (box_left_atlas, box_left_size) = match resource_dir {
            Some(dir) => {
                let iface = dir.join("Art").join("interface");
                let color = iface.join("box left color.pcx");
                let alpha = iface.join("box left alpha.pcx");
                match UIAtlas::from_color_alpha_pcx(&device, &queue, &color, &alpha) {
                    Some(atlas) => {
                        let size = [atlas.width as f32, atlas.height as f32];
                        (Some(atlas), size)
                    }
                    None => (None, [0.0, 0.0]),
                }
            }
            None => (None, [0.0, 0.0]),
        };
        let (box_right_atlas, box_right_size) = match resource_dir {
            Some(dir) => {
                let iface = dir.join("Art").join("interface");
                let color = iface.join("box right color.pcx");
                let alpha = iface.join("box right alpha.pcx");
                match UIAtlas::from_color_alpha_pcx(&device, &queue, &color, &alpha) {
                    Some(atlas) => {
                        let size = [atlas.width as f32, atlas.height as f32];
                        (Some(atlas), size)
                    }
                    None => (None, [0.0, 0.0]),
                }
            }
            None => (None, [0.0, 0.0]),
        };

        // Load dialog atlas for 9-slice panel backgrounds
        let mut nine_slice_registry = NineSliceRegistry::new();
        let (dialog_atlas, dialog_nine_slice_id) = match resource_dir {
            Some(dir) => {
                let pcx_path = dir.join("Art").join("popupborders.pcx");
                match UIAtlas::from_pcx_single(&device, &queue, &pcx_path, |idx| idx >= 254) {
                    Some(atlas) => {
                        // The sprite sheet is 500x300 with 9 pieces separated by
                        // 1px gutters. UV rects use a half-texel inset to prevent
                        // edge fragments from bleeding into adjacent separator pixels.
                        //   Col 0: x=251  Col 1: x=313  Col 2: x=375
                        //   Row 0: y=1    Row 1: y=46   Row 2: y=91
                        let aw = atlas.width as f32;
                        let ah = atlas.height as f32;
                        let pieces: [(f32, f32, f32, f32); 9] = [
                            (251.0, 1.0, 61.0, 44.0),  // NW
                            (313.0, 1.0, 61.0, 44.0),  // N
                            (375.0, 1.0, 61.0, 44.0),  // NE
                            (251.0, 46.0, 61.0, 44.0), // W
                            (313.0, 46.0, 61.0, 44.0), // C
                            (375.0, 46.0, 61.0, 44.0), // E
                            (251.0, 91.0, 61.0, 44.0), // SW
                            (313.0, 91.0, 61.0, 44.0), // S
                            (375.0, 91.0, 61.0, 44.0), // SE
                        ];
                        // Half-texel inset: shrink UV rect by 0.5 texels on each
                        // side so edge samples land on texel centers, not boundaries.
                        let uv_rects: [[f32; 4]; 9] = pieces.map(|(px, py, pw, ph)| {
                            [
                                (px + 0.5) / aw,
                                (py + 0.5) / ah,
                                (pw - 1.0) / aw,
                                (ph - 1.0) / ah,
                            ]
                        });
                        let id = nine_slice_registry.register(NineSliceImage {
                            uv_rects,
                            sizes: [61.0, 61.0, 61.0, 44.0, 44.0, 44.0],
                        });
                        (Some(atlas), Some(id))
                    }
                    None => (None, None),
                }
            }
            None => (None, None),
        };

        // Load advisor portrait atlas (science advisor for now)
        let advisor_atlas = match resource_dir {
            Some(dir) => {
                let pcx_path = dir
                    .join("Art")
                    .join("SmallHeads")
                    .join("popupSCIENCE.pcx");
                UIAtlas::from_pcx_single(&device, &queue, &pcx_path, |idx| idx >= 254)
            }
            None => None,
        };

        // Menu button atlas (single PCX with normal/hover/active states at known offsets)
        let menu_button_atlas = match resource_dir {
            Some(dir) => {
                let pcx_path = dir
                    .join("Art")
                    .join("interface")
                    .join("menuButtons.pcx");
                UIAtlas::from_pcx_single(&device, &queue, &pcx_path, |idx| idx >= 254)
            }
            None => None,
        };

        Renderer {
            surface,
            device,
            queue,
            config,
            window,
            camera_buffer,
            camera_bind_group,
            minimap_camera_buffer,
            minimap_camera_bind_group,
            terrain_atlas,
            city_atlas,
            unit_atlas,
            tile_pipeline,
            unit_pipeline,
            city_pipeline,
            path_line_pipeline,
            selection_circle_pipeline,
            ui_rect_pipeline,
            ui_atlas,
            ui_atlas_layer_height,
            ui_textured_rect_pipeline,
            box_left_atlas,
            box_right_atlas,
            box_left_size,
            box_right_size,
            ui_chrome_buffer: None,
            ui_chrome_count: 0,
            ui_chrome_batches: Vec::new(),
            tile_buffer: None,
            tile_count: 0,
            unit_buffer: None,
            unit_count: 0,
            unit_batches: Vec::new(),
            city_buffer: None,
            city_count: 0,
            minimap_tile_buffer: None,
            minimap_tile_count: 0,
            minimap_overlay_buffer: None,
            minimap_overlay_count: 0,
            path_line_buffer: None,
            path_line_count: 0,
            selection_circle_buffer: None,
            selection_circle_count: 0,
            ui_rect_buffer: None,
            ui_rect_count: 0,
            ui_textured_rect_buffer: None,
            ui_textured_rect_count: 0,
            dialog_atlas,
            nine_slice_registry,
            dialog_nine_slice_buffer: None,
            dialog_nine_slice_count: 0,
            dialog_nine_slice_id,
            advisor_atlas,
            advisor_buffer: None,
            advisor_count: 0,
            menu_button_atlas,
            menu_button_buffer: None,
            menu_button_count: 0,
        }
    }

    /// Create the tile pipeline with camera (group 0) and terrain atlas (group 1) layouts.
    fn create_tile_pipeline(
        device: &wgpu::Device,
        camera_layout: &wgpu::BindGroupLayout,
        atlas_layout: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tile"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/tile.wgsl").into()),
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tile"),
            bind_group_layouts: &[camera_layout, atlas_layout],
            ..Default::default()
        });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tile"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[TileInstance::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        })
    }

    /// Create a pipeline with camera (group 0) and texture atlas (group 1), with alpha blending.
    fn create_textured_pipeline(
        device: &wgpu::Device,
        camera_layout: &wgpu::BindGroupLayout,
        atlas_layout: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
        name: &str,
        wgsl_source: &str,
        instance_layout: wgpu::VertexBufferLayout<'static>,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(name),
            source: wgpu::ShaderSource::Wgsl(wgsl_source.into()),
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(name),
            bind_group_layouts: &[camera_layout, atlas_layout],
            ..Default::default()
        });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(name),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[instance_layout],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        })
    }

    fn create_pipeline(
        device: &wgpu::Device,
        camera_layout: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
        name: &str,
        wgsl_source: &str,
        instance_layout: wgpu::VertexBufferLayout<'static>,
        alpha_blend: bool,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(name),
            source: wgpu::ShaderSource::Wgsl(wgsl_source.into()),
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(name),
            bind_group_layouts: &[camera_layout],
            ..Default::default()
        });

        let blend = if alpha_blend {
            Some(wgpu::BlendState::ALPHA_BLENDING)
        } else {
            None
        };

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(name),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[instance_layout],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        })
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    /// Load per-unit-type sprite atlases from a resource directory.
    /// Called after the game engine has loaded mods and we know which unit types exist.
    pub fn load_unit_art(&mut self, resource_dir: &Path, unit_art: &[(String, String)]) {
        self.unit_atlas =
            UnitAtlasSet::from_resource_dir(&self.device, &self.queue, resource_dir, unit_art);
        // Recreate the unit pipeline with the new atlas layout
        let camera_layout =
            self.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                    label: Some("camera_bind_group_layout"),
                });
        self.unit_pipeline = Self::create_textured_pipeline(
            &self.device,
            &camera_layout,
            &self.unit_atlas.bind_group_layout,
            self.config.format,
            "unit",
            include_str!("shaders/unit.wgsl"),
            UnitInstance::desc(),
        );
    }

    pub fn update_camera(&self, uniform: &CameraUniform) {
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[*uniform]));
    }

    pub fn upload_tiles(&mut self, instances: &[TileInstance]) {
        self.tile_count = instances.len() as u32;
        if instances.is_empty() {
            self.tile_buffer = None;
            return;
        }
        self.tile_buffer = Some(self.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("tile_instances"),
                contents: bytemuck::cast_slice(instances),
                usage: wgpu::BufferUsages::VERTEX,
            },
        ));
    }

    pub fn upload_units(&mut self, instances: &[UnitInstance], batches: Vec<UnitDrawBatch>) {
        self.unit_count = instances.len() as u32;
        self.unit_batches = batches;
        if instances.is_empty() {
            self.unit_buffer = None;
            return;
        }
        self.unit_buffer = Some(self.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("unit_instances"),
                contents: bytemuck::cast_slice(instances),
                usage: wgpu::BufferUsages::VERTEX,
            },
        ));
    }

    pub fn update_minimap_camera(&self, uniform: &CameraUniform) {
        self.queue.write_buffer(
            &self.minimap_camera_buffer,
            0,
            bytemuck::cast_slice(&[*uniform]),
        );
    }

    fn create_ui_rect_pipeline(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ui_rect"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/ui_rect.wgsl").into()),
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ui_rect_layout"),
            bind_group_layouts: &[],
            ..Default::default()
        });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ui_rect"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[UIRectInstance::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        })
    }

    pub fn upload_ui_rects(&mut self, instances: &[UIRectInstance]) {
        self.ui_rect_count = instances.len() as u32;
        if instances.is_empty() {
            self.ui_rect_buffer = None;
            return;
        }
        self.ui_rect_buffer = Some(self.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("ui_rect_instances"),
                contents: bytemuck::cast_slice(instances),
                usage: wgpu::BufferUsages::VERTEX,
            },
        ));
    }

    /// Upload UI chrome instances (box left, box right) for the current frame.
    /// Each box is a full-texture quad positioned at the bottom corners.
    pub fn upload_ui_chrome(&mut self, vw: f32, vh: f32) {
        let mut instances: Vec<UITexturedRectInstance> = Vec::new();
        let mut batches: Vec<(u32, u32, u32)> = Vec::new(); // (atlas_idx, start, count)

        let scale = 1.0;
        if self.box_left_atlas.is_some() {
            let w = self.box_left_size[0] * scale;
            let h = self.box_left_size[1] * scale;
            let start = instances.len() as u32;
            instances.push(UITexturedRectInstance {
                rect: [0.0, vh - h, w, h],
                atlas_rect: [0.0, 0.0, 1.0, 1.0], // full texture
                viewport: [vw, vh],
            });
            batches.push((0, start, 1));
        }
        if self.box_right_atlas.is_some() {
            let w = self.box_right_size[0] * scale;
            let h = self.box_right_size[1] * scale;
            let start = instances.len() as u32;
            instances.push(UITexturedRectInstance {
                rect: [vw - w, vh - h, w, h],
                atlas_rect: [0.0, 0.0, 1.0, 1.0], // full texture
                viewport: [vw, vh],
            });
            batches.push((1, start, 1));
        }

        self.ui_chrome_count = instances.len() as u32;
        self.ui_chrome_batches = batches;
        if instances.is_empty() {
            self.ui_chrome_buffer = None;
            return;
        }
        self.ui_chrome_buffer = Some(self.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("ui_chrome_instances"),
                contents: bytemuck::cast_slice(&instances),
                usage: wgpu::BufferUsages::VERTEX,
            },
        ));
    }

    pub fn upload_ui_textured_rects(&mut self, instances: &[UITexturedRectInstance]) {
        self.ui_textured_rect_count = instances.len() as u32;
        if instances.is_empty() {
            self.ui_textured_rect_buffer = None;
            return;
        }
        self.ui_textured_rect_buffer = Some(self.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("ui_textured_rect_instances"),
                contents: bytemuck::cast_slice(instances),
                usage: wgpu::BufferUsages::VERTEX,
            },
        ));
    }

    pub fn upload_menu_buttons(&mut self, instances: &[UITexturedRectInstance]) {
        self.menu_button_count = instances.len() as u32;
        if instances.is_empty() {
            self.menu_button_buffer = None;
            return;
        }
        self.menu_button_buffer = Some(self.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("menu_button_instances"),
                contents: bytemuck::cast_slice(instances),
                usage: wgpu::BufferUsages::VERTEX,
            },
        ));
    }

    pub fn upload_advisor_rects(&mut self, instances: &[UITexturedRectInstance]) {
        self.advisor_count = instances.len() as u32;
        if instances.is_empty() {
            self.advisor_buffer = None;
            return;
        }
        self.advisor_buffer = Some(self.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("advisor_instances"),
                contents: bytemuck::cast_slice(instances),
                usage: wgpu::BufferUsages::VERTEX,
            },
        ));
    }

    pub fn upload_dialog_nine_slices(&mut self, instances: &[UITexturedRectInstance]) {
        self.dialog_nine_slice_count = instances.len() as u32;
        if instances.is_empty() {
            self.dialog_nine_slice_buffer = None;
            return;
        }
        self.dialog_nine_slice_buffer = Some(self.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("dialog_nine_slice_instances"),
                contents: bytemuck::cast_slice(instances),
                usage: wgpu::BufferUsages::VERTEX,
            },
        ));
    }

    fn create_ui_textured_rect_pipeline(
        device: &wgpu::Device,
        atlas_layout: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ui_textured_rect"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/ui_textured_rect.wgsl").into()),
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ui_textured_rect_layout"),
            bind_group_layouts: &[atlas_layout],
            ..Default::default()
        });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ui_textured_rect"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[UITexturedRectInstance::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        })
    }

    pub fn upload_path_lines(&mut self, instances: &[PathLineInstance]) {
        self.path_line_count = instances.len() as u32;
        if instances.is_empty() {
            self.path_line_buffer = None;
            return;
        }
        self.path_line_buffer = Some(self.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("path_line_instances"),
                contents: bytemuck::cast_slice(instances),
                usage: wgpu::BufferUsages::VERTEX,
            },
        ));
    }

    pub fn upload_selection_circles(&mut self, instances: &[SelectionCircleInstance]) {
        self.selection_circle_count = instances.len() as u32;
        if instances.is_empty() {
            self.selection_circle_buffer = None;
            return;
        }
        self.selection_circle_buffer = Some(self.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("selection_circle_instances"),
                contents: bytemuck::cast_slice(instances),
                usage: wgpu::BufferUsages::VERTEX,
            },
        ));
    }

    pub fn upload_minimap_tiles(&mut self, instances: &[TileInstance]) {
        self.minimap_tile_count = instances.len() as u32;
        if instances.is_empty() {
            self.minimap_tile_buffer = None;
            return;
        }
        self.minimap_tile_buffer = Some(self.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("minimap_tile_instances"),
                contents: bytemuck::cast_slice(instances),
                usage: wgpu::BufferUsages::VERTEX,
            },
        ));
    }

    pub fn upload_minimap_overlay(&mut self, instances: &[UIRectInstance]) {
        self.minimap_overlay_count = instances.len() as u32;
        if instances.is_empty() {
            self.minimap_overlay_buffer = None;
            return;
        }
        self.minimap_overlay_buffer = Some(self.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("minimap_overlay_instances"),
                contents: bytemuck::cast_slice(instances),
                usage: wgpu::BufferUsages::VERTEX,
            },
        ));
    }

    pub fn upload_cities(&mut self, instances: &[CityInstance]) {
        self.city_count = instances.len() as u32;
        if instances.is_empty() {
            self.city_buffer = None;
            return;
        }
        self.city_buffer = Some(self.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("city_instances"),
                contents: bytemuck::cast_slice(instances),
                usage: wgpu::BufferUsages::VERTEX,
            },
        ));
    }

    pub fn render_with_minimap(
        &self,
        text_renderer: Option<(
            &glyphon::TextRenderer,
            &glyphon::TextAtlas,
            &glyphon::Viewport,
        )>,
        minimap_rect: Option<[f32; 4]>, // [x, y, width, height] in pixels
    ) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render_encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            // Pass 1: Tiles
            if let Some(ref buf) = self.tile_buffer {
                render_pass.set_pipeline(&self.tile_pipeline);
                render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                render_pass.set_bind_group(1, &self.terrain_atlas.bind_group, &[]);
                render_pass.set_vertex_buffer(0, buf.slice(..));
                render_pass.draw(0..6, 0..self.tile_count);
            }

            // Pass 2: Cities
            if let Some(ref buf) = self.city_buffer {
                render_pass.set_pipeline(&self.city_pipeline);
                render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                render_pass.set_bind_group(1, &self.city_atlas.bind_group, &[]);
                render_pass.set_vertex_buffer(0, buf.slice(..));
                render_pass.draw(0..6, 0..self.city_count);
            }

            // Pass 2.5: Path lines (on top of tiles/cities, below units)
            if let Some(ref buf) = self.path_line_buffer {
                render_pass.set_pipeline(&self.path_line_pipeline);
                render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                render_pass.set_vertex_buffer(0, buf.slice(..));
                render_pass.draw(0..6, 0..self.path_line_count);
            }

            // Pass 2.6: Selection circle (on top of tiles/path lines, below units)
            if let Some(ref buf) = self.selection_circle_buffer {
                render_pass.set_pipeline(&self.selection_circle_pipeline);
                render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                render_pass.set_vertex_buffer(0, buf.slice(..));
                render_pass.draw(0..6, 0..self.selection_circle_count);
            }

            // Pass 3: Units (batched by unit type for per-type textures)
            if let Some(ref buf) = self.unit_buffer {
                render_pass.set_pipeline(&self.unit_pipeline);
                render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                render_pass.set_vertex_buffer(0, buf.slice(..));
                for batch in &self.unit_batches {
                    let bg = self
                        .unit_atlas
                        .bind_group(&batch.unit_type, &batch.anim_name);
                    render_pass.set_bind_group(1, bg, &[]);
                    render_pass.draw(0..6, batch.start..batch.start + batch.count);
                }
            }

            // Pass 3.5: UI rectangles (screen-space, alpha-blended)
            if let Some(ref buf) = self.ui_rect_buffer {
                render_pass.set_pipeline(&self.ui_rect_pipeline);
                render_pass.set_vertex_buffer(0, buf.slice(..));
                render_pass.draw(0..6, 0..self.ui_rect_count);
            }

            // Pass 3.55: Dialog 9-slice panels (screen-space, dialog atlas)
            if let Some(ref buf) = self.dialog_nine_slice_buffer {
                if let Some(ref atlas) = self.dialog_atlas {
                    render_pass.set_pipeline(&self.ui_textured_rect_pipeline);
                    render_pass.set_bind_group(0, &atlas.bind_group, &[]);
                    render_pass.set_vertex_buffer(0, buf.slice(..));
                    render_pass.draw(0..6, 0..self.dialog_nine_slice_count);
                }
            }

            // Pass 3.56: Advisor portraits (on top of dialog background, below text)
            if let Some(ref buf) = self.advisor_buffer {
                if let Some(ref atlas) = self.advisor_atlas {
                    render_pass.set_pipeline(&self.ui_textured_rect_pipeline);
                    render_pass.set_bind_group(0, &atlas.bind_group, &[]);
                    render_pass.set_vertex_buffer(0, buf.slice(..));
                    render_pass.draw(0..6, 0..self.advisor_count);
                }
            }

            // Pass 3.6: UI chrome boxes (box left, box right — each with own atlas)
            if let Some(ref buf) = self.ui_chrome_buffer {
                render_pass.set_pipeline(&self.ui_textured_rect_pipeline);
                render_pass.set_vertex_buffer(0, buf.slice(..));
                for &(atlas_idx, start, count) in &self.ui_chrome_batches {
                    let bg = match atlas_idx {
                        0 => self.box_left_atlas.as_ref().map(|a| &a.bind_group),
                        1 => self.box_right_atlas.as_ref().map(|a| &a.bind_group),
                        _ => None,
                    };
                    if let Some(bg) = bg {
                        render_pass.set_bind_group(0, bg, &[]);
                        render_pass.draw(0..6, start..start + count);
                    }
                }
            }

            // Pass 3.7: Minimap (renders on top of left chrome box)
            if let (Some(ref buf), Some(rect)) = (&self.minimap_tile_buffer, minimap_rect) {
                render_pass.set_viewport(rect[0], rect[1], rect[2], rect[3], 0.0, 1.0);
                render_pass.set_pipeline(&self.tile_pipeline);
                render_pass.set_bind_group(0, &self.minimap_camera_bind_group, &[]);
                render_pass.set_bind_group(1, &self.terrain_atlas.bind_group, &[]);
                render_pass.set_vertex_buffer(0, buf.slice(..));
                render_pass.draw(0..6, 0..self.minimap_tile_count);

                // Pass 3.7b: Minimap viewport overlay (white box showing camera bounds)
                if let Some(ref overlay_buf) = self.minimap_overlay_buffer {
                    render_pass.set_pipeline(&self.ui_rect_pipeline);
                    render_pass.set_vertex_buffer(0, overlay_buf.slice(..));
                    render_pass.draw(0..6, 0..self.minimap_overlay_count);
                }

                // Reset viewport
                render_pass.set_viewport(
                    0.0,
                    0.0,
                    self.config.width as f32,
                    self.config.height as f32,
                    0.0,
                    1.0,
                );
                // Reset camera bind group for text
                render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            }

            // Pass 3.8: UI textured rectangles (screen-space, atlas-sampled)
            if let Some(ref buf) = self.ui_textured_rect_buffer {
                render_pass.set_pipeline(&self.ui_textured_rect_pipeline);
                render_pass.set_bind_group(0, &self.ui_atlas.bind_group, &[]);
                render_pass.set_vertex_buffer(0, buf.slice(..));
                render_pass.draw(0..6, 0..self.ui_textured_rect_count);
            }

            // Pass 3.9: Menu buttons (top-left, own atlas)
            if let (Some(ref buf), Some(ref atlas)) =
                (&self.menu_button_buffer, &self.menu_button_atlas)
            {
                render_pass.set_pipeline(&self.ui_textured_rect_pipeline);
                render_pass.set_bind_group(0, &atlas.bind_group, &[]);
                render_pass.set_vertex_buffer(0, buf.slice(..));
                render_pass.draw(0..6, 0..self.menu_button_count);
            }

            // Pass 4: Text overlays
            if let Some((tr, atlas, viewport)) = text_renderer {
                let _ = tr.render(atlas, viewport, &mut render_pass);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}
