use fc3_core::city::ProductionItem;
use fc3_core::id::CityId;
use fc3_core::protocol::ProductionOption;
use winit::keyboard::KeyCode;

/// Screen-space rectangle.
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.w && py >= self.y && py < self.y + self.h
    }
}

/// A text element in absolute screen coordinates.
#[derive(Debug, Clone)]
pub struct TextElement {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub font_size: f32,
    pub line_height: f32,
    pub max_width: f32,
    pub color: [u8; 3],
}

/// A colored rectangle in screen space.
#[derive(Debug, Clone, Copy)]
pub struct RectElement {
    pub rect: Rect,
    pub color: [f32; 4],
}

/// Everything a layer wants to draw.
pub struct LayerVisuals {
    pub rects: Vec<RectElement>,
    pub texts: Vec<TextElement>,
}

/// A generic menu item for popup menus.
pub struct PopupMenuItem {
    pub label: String,
    pub detail: Option<String>,
    pub enabled: bool,
}

/// Visual constants for popup menus.
const POPUP_BG: [f32; 4] = [0.1, 0.1, 0.15, 0.92];
const POPUP_HOVER: [f32; 4] = [0.25, 0.25, 0.35, 0.8];
const POPUP_SELECTED: [f32; 4] = [0.3, 0.3, 0.5, 0.9];
const ITEM_HEIGHT: f32 = 28.0;
const HEADER_HEIGHT: f32 = 32.0;
const PADDING: f32 = 8.0;
const POPUP_WIDTH: f32 = 320.0;
const MAX_VISIBLE_DEFAULT: usize = 12;

/// A generic reusable popup menu.
pub struct PopupMenu {
    pub title: Option<String>,
    pub items: Vec<PopupMenuItem>,
    pub bounds: Rect,
    pub selected_index: usize,
    pub hovered_index: Option<usize>,
    pub scroll_offset: usize,
    pub max_visible: usize,
}

impl PopupMenu {
    /// Create a new popup menu positioned near an anchor point, clamped to screen.
    pub fn new(
        title: Option<String>,
        items: Vec<PopupMenuItem>,
        anchor_x: f32,
        anchor_y: f32,
        screen_w: f32,
        screen_h: f32,
    ) -> Self {
        let max_visible = MAX_VISIBLE_DEFAULT.min(items.len());
        let bounds = Self::compute_bounds(
            items.len(),
            max_visible,
            title.is_some(),
            anchor_x,
            anchor_y,
            screen_w,
            screen_h,
        );
        PopupMenu {
            title,
            items,
            bounds,
            selected_index: 0,
            hovered_index: None,
            scroll_offset: 0,
            max_visible,
        }
    }

    fn compute_bounds(
        item_count: usize,
        max_visible: usize,
        has_title: bool,
        anchor_x: f32,
        anchor_y: f32,
        screen_w: f32,
        screen_h: f32,
    ) -> Rect {
        let visible = max_visible.min(item_count);
        let header = if has_title { HEADER_HEIGHT } else { 0.0 };
        let h = header + visible as f32 * ITEM_HEIGHT + 2.0 * PADDING;
        let w = POPUP_WIDTH;

        // Position below anchor, clamped to screen
        let mut x = anchor_x;
        let mut y = anchor_y;
        if x + w > screen_w {
            x = (screen_w - w - 4.0).max(0.0);
        }
        if y + h > screen_h {
            y = (screen_h - h - 4.0).max(0.0);
        }

        Rect { x, y, w, h }
    }

    /// Which item index was clicked (relative to visible items + scroll).
    pub fn hit_test(&self, screen_x: f32, screen_y: f32) -> Option<usize> {
        if !self.bounds.contains(screen_x, screen_y) {
            return None;
        }
        let header = if self.title.is_some() {
            HEADER_HEIGHT
        } else {
            0.0
        };
        let rel_y = screen_y - self.bounds.y - PADDING - header;
        if rel_y < 0.0 {
            return None;
        }
        let idx = (rel_y / ITEM_HEIGHT) as usize;
        let visible = self.max_visible.min(self.items.len());
        if idx < visible {
            let actual = idx + self.scroll_offset;
            if actual < self.items.len() {
                return Some(actual);
            }
        }
        None
    }

    /// Update hover based on cursor position.
    pub fn update_hover(&mut self, screen_x: f32, screen_y: f32) {
        self.hovered_index = self.hit_test(screen_x, screen_y);
    }

    /// Scroll the menu (positive = down).
    pub fn scroll(&mut self, delta: f32) {
        if self.items.len() <= self.max_visible {
            return;
        }
        let max_scroll = self.items.len() - self.max_visible;
        if delta > 0.0 && self.scroll_offset < max_scroll {
            self.scroll_offset += 1;
        } else if delta < 0.0 && self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    /// Move selection up.
    pub fn select_prev(&mut self) {
        if self.items.is_empty() {
            return;
        }
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
        // Ensure selected item is visible
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        }
    }

    /// Move selection down.
    pub fn select_next(&mut self) {
        if self.items.is_empty() {
            return;
        }
        if self.selected_index < self.items.len() - 1 {
            self.selected_index += 1;
        }
        // Ensure selected item is visible
        let visible = self.max_visible.min(self.items.len());
        if self.selected_index >= self.scroll_offset + visible {
            self.scroll_offset = self.selected_index + 1 - visible;
        }
    }

    /// Build visual elements for this popup menu.
    pub fn visuals(&self) -> LayerVisuals {
        let mut rects = Vec::new();
        let mut texts = Vec::new();

        // Background
        rects.push(RectElement {
            rect: self.bounds,
            color: POPUP_BG,
        });

        let header = if self.title.is_some() {
            HEADER_HEIGHT
        } else {
            0.0
        };

        // Title
        if let Some(ref title) = self.title {
            texts.push(TextElement {
                text: title.clone(),
                x: self.bounds.x + PADDING,
                y: self.bounds.y + PADDING,
                font_size: 18.0,
                line_height: 22.0,
                max_width: self.bounds.w - 2.0 * PADDING,
                color: [255, 255, 100],
            });
        }

        // Items
        let visible = self.max_visible.min(self.items.len());
        for vi in 0..visible {
            let idx = vi + self.scroll_offset;
            if idx >= self.items.len() {
                break;
            }
            let item = &self.items[idx];
            let item_y = self.bounds.y + PADDING + header + vi as f32 * ITEM_HEIGHT;

            // Highlight rect for hovered/selected
            if self.hovered_index == Some(idx) {
                rects.push(RectElement {
                    rect: Rect {
                        x: self.bounds.x + 2.0,
                        y: item_y,
                        w: self.bounds.w - 4.0,
                        h: ITEM_HEIGHT,
                    },
                    color: POPUP_HOVER,
                });
            }
            if self.selected_index == idx {
                rects.push(RectElement {
                    rect: Rect {
                        x: self.bounds.x + 2.0,
                        y: item_y,
                        w: self.bounds.w - 4.0,
                        h: ITEM_HEIGHT,
                    },
                    color: POPUP_SELECTED,
                });
            }

            // Label
            let label_color = if item.enabled {
                [220, 220, 220]
            } else {
                [100, 100, 100]
            };
            texts.push(TextElement {
                text: item.label.clone(),
                x: self.bounds.x + PADDING + 4.0,
                y: item_y + 4.0,
                font_size: 16.0,
                line_height: 20.0,
                max_width: self.bounds.w * 0.6,
                color: label_color,
            });

            // Detail (right-aligned)
            if let Some(ref detail) = item.detail {
                texts.push(TextElement {
                    text: detail.clone(),
                    x: self.bounds.x + self.bounds.w - PADDING - 80.0,
                    y: item_y + 4.0,
                    font_size: 14.0,
                    line_height: 18.0,
                    max_width: 80.0,
                    color: [160, 160, 180],
                });
            }
        }

        // Scroll indicators
        if self.scroll_offset > 0 {
            texts.push(TextElement {
                text: "...".to_string(),
                x: self.bounds.x + self.bounds.w / 2.0 - 10.0,
                y: self.bounds.y + PADDING + header - 14.0,
                font_size: 12.0,
                line_height: 14.0,
                max_width: 40.0,
                color: [180, 180, 180],
            });
        }
        if self.scroll_offset + visible < self.items.len() {
            let bottom_y = self.bounds.y + PADDING + header + visible as f32 * ITEM_HEIGHT;
            texts.push(TextElement {
                text: "...".to_string(),
                x: self.bounds.x + self.bounds.w / 2.0 - 10.0,
                y: bottom_y,
                font_size: 12.0,
                line_height: 14.0,
                max_width: 40.0,
                color: [180, 180, 180],
            });
        }

        LayerVisuals { rects, texts }
    }
}

/// Events returned by layer input handling.
pub enum LayerEvent {
    /// Input was consumed by the layer, no further action needed.
    Consumed,
    /// The layer should be closed.
    Close,
    /// A production item was selected.
    ProductionSelected {
        city_id: CityId,
        item: ProductionItem,
    },
    /// A debug command was entered.
    DebugCommand { command: String },
}

/// A UI layer that sits on top of the game screen.
pub enum UILayer {
    ProductionPopup {
        city_id: CityId,
        menu: PopupMenu,
        /// Parallel to menu.items — the actual production options.
        options: Vec<ProductionOption>,
    },
    DebugTerminal {
        input_buffer: String,
        output_lines: Vec<String>,
        screen_w: f32,
        screen_h: f32,
    },
}

impl UILayer {
    /// Get the bounding rectangle for this layer.
    pub fn bounds(&self) -> Rect {
        match self {
            UILayer::ProductionPopup { menu, .. } => menu.bounds,
            UILayer::DebugTerminal {
                screen_w, screen_h, ..
            } => Rect {
                x: 0.0,
                y: 0.0,
                w: *screen_w,
                h: *screen_h,
            },
        }
    }

    /// Handle a mouse click at the given screen coordinates.
    pub fn handle_click(&mut self, screen_x: f32, screen_y: f32) -> Option<LayerEvent> {
        match self {
            UILayer::ProductionPopup {
                city_id,
                menu,
                options,
            } => {
                if let Some(idx) = menu.hit_test(screen_x, screen_y) {
                    if idx < options.len() && menu.items[idx].enabled {
                        return Some(LayerEvent::ProductionSelected {
                            city_id: *city_id,
                            item: options[idx].item.clone(),
                        });
                    }
                    return Some(LayerEvent::Consumed);
                }
                if menu.bounds.contains(screen_x, screen_y) {
                    return Some(LayerEvent::Consumed);
                }
                None
            }
            UILayer::DebugTerminal { .. } => Some(LayerEvent::Consumed),
        }
    }

    /// Handle typed text input for the debug terminal.
    pub fn handle_text(&mut self, text: &str) -> Option<LayerEvent> {
        match self {
            UILayer::DebugTerminal { input_buffer, .. } => {
                for ch in text.chars() {
                    if !ch.is_control() && ch != '`' {
                        input_buffer.push(ch);
                    }
                }
                Some(LayerEvent::Consumed)
            }
            _ => None,
        }
    }

    /// Handle a key press.
    pub fn handle_key(&mut self, key: KeyCode) -> Option<LayerEvent> {
        match self {
            UILayer::ProductionPopup {
                city_id,
                menu,
                options,
            } => match key {
                KeyCode::Escape => Some(LayerEvent::Close),
                KeyCode::ArrowUp => {
                    menu.select_prev();
                    Some(LayerEvent::Consumed)
                }
                KeyCode::ArrowDown => {
                    menu.select_next();
                    Some(LayerEvent::Consumed)
                }
                KeyCode::Enter | KeyCode::NumpadEnter => {
                    let idx = menu.selected_index;
                    if idx < options.len() && menu.items[idx].enabled {
                        Some(LayerEvent::ProductionSelected {
                            city_id: *city_id,
                            item: options[idx].item.clone(),
                        })
                    } else {
                        Some(LayerEvent::Consumed)
                    }
                }
                _ => Some(LayerEvent::Consumed),
            },
            UILayer::DebugTerminal {
                input_buffer,
                output_lines,
                ..
            } => match key {
                KeyCode::Backquote => Some(LayerEvent::Close),
                KeyCode::Escape => Some(LayerEvent::Close),
                KeyCode::Backspace => {
                    input_buffer.pop();
                    Some(LayerEvent::Consumed)
                }
                KeyCode::Enter | KeyCode::NumpadEnter => {
                    let cmd = input_buffer.trim().to_string();
                    input_buffer.clear();
                    if cmd.is_empty() {
                        return Some(LayerEvent::Consumed);
                    }
                    output_lines.push(format!("> {cmd}"));
                    Some(LayerEvent::DebugCommand { command: cmd })
                }
                _ => Some(LayerEvent::Consumed),
            },
        }
    }

    /// Update hover state from cursor position.
    pub fn handle_cursor_move(&mut self, screen_x: f32, screen_y: f32) {
        match self {
            UILayer::ProductionPopup { menu, .. } => {
                menu.update_hover(screen_x, screen_y);
            }
            UILayer::DebugTerminal { .. } => {}
        }
    }

    /// Handle scroll wheel input.
    pub fn handle_scroll(&mut self, delta: f32) {
        match self {
            UILayer::ProductionPopup { menu, .. } => {
                menu.scroll(delta);
            }
            UILayer::DebugTerminal { .. } => {}
        }
    }

    /// Build visual elements for this layer.
    pub fn visuals(&self) -> LayerVisuals {
        match self {
            UILayer::ProductionPopup { menu, .. } => menu.visuals(),
            UILayer::DebugTerminal {
                input_buffer,
                output_lines,
                screen_w,
                screen_h,
            } => {
                let mut rects = Vec::new();
                let mut texts = Vec::new();

                // Semi-transparent black background covering the whole screen
                rects.push(RectElement {
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        w: *screen_w,
                        h: *screen_h,
                    },
                    color: [0.0, 0.0, 0.0, 0.8],
                });

                let font_size = 16.0;
                let line_height = 22.0;
                let margin = 16.0;

                // Output lines (from bottom up, above the prompt)
                let prompt_y = *screen_h - margin - line_height;
                let max_lines = ((prompt_y - margin) / line_height) as usize;
                let start = output_lines.len().saturating_sub(max_lines);
                for (i, line) in output_lines[start..].iter().enumerate() {
                    texts.push(TextElement {
                        text: line.clone(),
                        x: margin,
                        y: margin + i as f32 * line_height,
                        font_size,
                        line_height,
                        max_width: *screen_w - 2.0 * margin,
                        color: [180, 180, 180],
                    });
                }

                // Input prompt
                texts.push(TextElement {
                    text: format!("> {input_buffer}_"),
                    x: margin,
                    y: prompt_y,
                    font_size,
                    line_height,
                    max_width: *screen_w - 2.0 * margin,
                    color: [100, 255, 100],
                });

                LayerVisuals { rects, texts }
            }
        }
    }
}

/// Collect visuals from all layers into combined rect and text lists.
pub fn build_layer_visuals(layers: &[UILayer]) -> (Vec<RectElement>, Vec<TextElement>) {
    let mut all_rects = Vec::new();
    let mut all_texts = Vec::new();
    for layer in layers {
        let vis = layer.visuals();
        all_rects.extend(vis.rects);
        all_texts.extend(vis.texts);
    }
    (all_rects, all_texts)
}
