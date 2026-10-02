use std::any::{Any, TypeId};
use std::collections::HashMap;

use winit::keyboard::KeyCode;

use crate::instances::UITexturedRectInstance;
use crate::ui_layer::{Rect, RectElement, TextElement};

// ── 9-Slice Registry ─────────────────────────────────────────────────

/// Describes a 9-slice image as 9 explicit sprite regions within an atlas.
pub struct NineSliceImage {
    /// UV rects for the 9 pieces: [NW, N, NE, W, C, E, SW, S, SE]
    /// Each is [u, v, w, h] in normalized atlas coordinates.
    pub uv_rects: [[f32; 4]; 9],
    /// Pixel sizes: [left_w, center_w, right_w, top_h, center_h, bottom_h]
    pub sizes: [f32; 6],
}

/// Identifier for a registered 9-slice image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NineSliceId(pub usize);

pub struct NineSliceRegistry {
    pub images: Vec<NineSliceImage>,
}

impl NineSliceRegistry {
    pub fn new() -> Self {
        NineSliceRegistry { images: Vec::new() }
    }

    pub fn register(&mut self, image: NineSliceImage) -> NineSliceId {
        let id = NineSliceId(self.images.len());
        self.images.push(image);
        id
    }

    pub fn get(&self, id: NineSliceId) -> Option<&NineSliceImage> {
        self.images.get(id.0)
    }
}

// ── Element Tree ─────────────────────────────────────────────────────

/// Background style for a Box element.
pub enum Background {
    None,
    Color([f32; 4]),
    NineSlice { image_id: NineSliceId },
}

/// Style for a Box element.
pub struct BoxStyle {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub background: Background,
}

/// An element in the UI tree.
pub enum Element {
    Box {
        style: BoxStyle,
        children: Vec<Element>,
    },
    Text {
        text: String,
        x: f32,
        y: f32,
        font_size: f32,
        max_width: f32,
        color: [u8; 3],
    },
    /// A textured image from a non-dialog atlas (e.g. advisor portraits).
    /// Position is relative to the parent Box, like Text.
    Image {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        /// Normalized UV rect [u, v, w, h] within the image's atlas.
        uv_rect: [f32; 4],
    },
}

// ── State Store ──────────────────────────────────────────────────────

type ComponentPath = Vec<(TypeId, usize)>;

/// Stores component state keyed by tree position.
pub struct StateStore {
    states: HashMap<ComponentPath, Box<dyn Any>>,
}

impl StateStore {
    pub fn new() -> Self {
        StateStore {
            states: HashMap::new(),
        }
    }

    fn get_or_default<S: Default + 'static>(&mut self, path: &ComponentPath) -> S {
        match self.states.remove(path) {
            Some(boxed) => match boxed.downcast::<S>() {
                Ok(s) => *s,
                Err(_) => S::default(),
            },
            None => S::default(),
        }
    }

    fn put<S: 'static>(&mut self, path: ComponentPath, state: S) {
        self.states.insert(path, Box::new(state));
    }
}

// ── BuildCtx ─────────────────────────────────────────────────────────

/// Context passed to Component::build(), tracks tree position and input.
pub struct BuildCtx<'a> {
    path: ComponentPath,
    sibling_counters: Vec<HashMap<TypeId, usize>>,
    pub state_store: &'a mut StateStore,
    pub mouse_pos: (f32, f32),
    pub mouse_clicked: bool,
    pub key_pressed: Option<KeyCode>,
    pub scroll_delta: f32,
    pub screen_width: f32,
    pub screen_height: f32,
}

impl<'a> BuildCtx<'a> {
    pub fn new(
        state_store: &'a mut StateStore,
        mouse_pos: (f32, f32),
        mouse_clicked: bool,
        key_pressed: Option<KeyCode>,
        scroll_delta: f32,
        screen_width: f32,
        screen_height: f32,
    ) -> Self {
        BuildCtx {
            path: Vec::new(),
            sibling_counters: vec![HashMap::new()],
            state_store,
            mouse_pos,
            mouse_clicked,
            key_pressed,
            scroll_delta,
            screen_width,
            screen_height,
        }
    }

    /// Hit-test: is the mouse within the given screen-space rectangle?
    pub fn is_hovered_rect(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        let (mx, my) = self.mouse_pos;
        mx >= x && mx < x + w && my >= y && my < y + h
    }

    /// Click-test: was the mouse clicked within the given screen-space rectangle?
    pub fn is_clicked_rect(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        self.mouse_clicked && self.is_hovered_rect(x, y, w, h)
    }

    /// Build a child component, managing path and state automatically.
    pub fn component<C: Component>(&mut self, props: &C::Props) -> (Element, Option<C::Action>) {
        let type_id = TypeId::of::<C>();

        // Assign sibling index
        let counter = self
            .sibling_counters
            .last_mut()
            .expect("sibling counter stack empty");
        let sibling_idx = *counter.entry(type_id).or_insert(0);
        *counter.get_mut(&type_id).unwrap() += 1;

        // Push path
        self.path.push((type_id, sibling_idx));
        self.sibling_counters.push(HashMap::new());

        // Get or create state
        let mut state: C::State = self.state_store.get_or_default(&self.path);

        // Build
        let result = C::build(props, &mut state, self);

        // Store state back
        let path_clone = self.path.clone();
        self.state_store.put(path_clone, state);

        // Pop path
        self.sibling_counters.pop();
        self.path.pop();

        result
    }
}

// ── Component Trait ──────────────────────────────────────────────────

pub trait Component: 'static {
    type Props;
    type State: Default + 'static;
    type Action;

    fn build(
        props: &Self::Props,
        state: &mut Self::State,
        ctx: &mut BuildCtx,
    ) -> (Element, Option<Self::Action>);
}

// ── Flatten ──────────────────────────────────────────────────────────

/// Flattened render output from the element tree.
pub struct FlattenedUI {
    pub rects: Vec<RectElement>,
    pub texts: Vec<TextElement>,
    /// Textured rects for the dialog atlas (9-slice backgrounds).
    pub textured_rects: Vec<UITexturedRectInstance>,
    /// Textured rects for non-dialog images (e.g. advisor portraits).
    pub image_rects: Vec<UITexturedRectInstance>,
}

impl FlattenedUI {
    pub fn new() -> Self {
        FlattenedUI {
            rects: Vec::new(),
            texts: Vec::new(),
            textured_rects: Vec::new(),
            image_rects: Vec::new(),
        }
    }
}

/// Walk the element tree and emit render primitives.
pub fn flatten(
    element: &Element,
    parent_x: f32,
    parent_y: f32,
    vp: [f32; 2],
    registry: &NineSliceRegistry,
    out: &mut FlattenedUI,
) {
    match element {
        Element::Box { style, children } => {
            let abs_x = parent_x + style.x;
            let abs_y = parent_y + style.y;

            match &style.background {
                Background::None => {}
                Background::Color(color) => {
                    out.rects.push(RectElement {
                        rect: Rect {
                            x: abs_x,
                            y: abs_y,
                            w: style.width,
                            h: style.height,
                        },
                        color: *color,
                    });
                }
                Background::NineSlice { image_id } => {
                    emit_nine_slice(registry, *image_id, abs_x, abs_y, style.width, style.height, vp, out);
                }
            }

            for child in children {
                flatten(child, abs_x, abs_y, vp, registry, out);
            }
        }
        Element::Text {
            text,
            x,
            y,
            font_size,
            max_width,
            color,
        } => {
            out.texts.push(TextElement {
                text: text.clone(),
                x: parent_x + x,
                y: parent_y + y,
                font_size: *font_size,
                line_height: *font_size + 4.0,
                max_width: *max_width,
                color: *color,
            });
        }
        Element::Image {
            x,
            y,
            width,
            height,
            uv_rect,
        } => {
            out.image_rects.push(UITexturedRectInstance {
                rect: [parent_x + x, parent_y + y, *width, *height],
                atlas_rect: *uv_rect,
                viewport: vp,
            });
        }
    }
}

/// Emit textured quads for a 9-slice panel.
///
/// Corners are drawn once at native size. Edges and center are tiled (repeated)
/// at their native piece size, with a partial piece at the end if needed.
#[allow(clippy::too_many_arguments)]
fn emit_nine_slice(
    registry: &NineSliceRegistry,
    image_id: NineSliceId,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    vp: [f32; 2],
    out: &mut FlattenedUI,
) {
    let image = match registry.get(image_id) {
        Some(img) => img,
        None => return,
    };

    // sizes: [left_w, center_w, right_w, top_h, center_h, bottom_h]
    let piece_w = [image.sizes[0], image.sizes[1], image.sizes[2]];
    let piece_h = [image.sizes[3], image.sizes[4], image.sizes[5]];

    let left_w = piece_w[0].min(w / 2.0);
    let right_w = piece_w[2].min(w / 2.0);
    let center_w = (w - left_w - right_w).max(0.0);

    let top_h = piece_h[0].min(h / 2.0);
    let bottom_h = piece_h[2].min(h / 2.0);
    let center_h = (h - top_h - bottom_h).max(0.0);

    let col_x = [x, x + left_w, x + left_w + center_w];
    let col_total_w = [left_w, center_w, right_w];
    let row_y = [y, y + top_h, y + top_h + center_h];
    let row_total_h = [top_h, center_h, bottom_h];

    // uv_rects order: [NW, N, NE, W, C, E, SW, S, SE] = row-major
    for r in 0..3usize {
        for c in 0..3usize {
            let total_w = col_total_w[c];
            let total_h = row_total_h[r];
            if total_w <= 0.0 || total_h <= 0.0 {
                continue;
            }
            let uv = image.uv_rects[r * 3 + c];
            let is_corner = (r == 0 || r == 2) && (c == 0 || c == 2);
            if is_corner {
                // Corners: single quad at native size (already clamped above)
                out.textured_rects.push(UITexturedRectInstance {
                    rect: [col_x[c], row_y[r], total_w, total_h],
                    atlas_rect: uv,
                    viewport: vp,
                });
            } else {
                // Edges and center: tile at native piece size
                let tile_w = piece_w[c];
                let tile_h = piece_h[r];
                emit_tiled_quads(
                    col_x[c], row_y[r], total_w, total_h, tile_w, tile_h, uv, vp, out,
                );
            }
        }
    }
}

/// Emit quads that tile a source piece across a destination rectangle.
/// Partial tiles at the right/bottom edge use a cropped UV rect.
#[allow(clippy::too_many_arguments)]
fn emit_tiled_quads(
    x: f32,
    y: f32,
    total_w: f32,
    total_h: f32,
    tile_w: f32,
    tile_h: f32,
    uv: [f32; 4],
    vp: [f32; 2],
    out: &mut FlattenedUI,
) {
    let mut dy = 0.0;
    while dy < total_h {
        let qh = tile_h.min(total_h - dy);
        let uv_h = uv[3] * (qh / tile_h);
        let mut dx = 0.0;
        while dx < total_w {
            let qw = tile_w.min(total_w - dx);
            let uv_w = uv[2] * (qw / tile_w);
            out.textured_rects.push(UITexturedRectInstance {
                rect: [x + dx, y + dy, qw, qh],
                atlas_rect: [uv[0], uv[1], uv_w, uv_h],
                viewport: vp,
            });
            dx += tile_w;
        }
        dy += tile_h;
    }
}
