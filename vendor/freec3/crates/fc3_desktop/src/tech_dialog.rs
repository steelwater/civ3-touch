use winit::keyboard::KeyCode;

use crate::ui_component::{Background, BoxStyle, BuildCtx, Component, Element, NineSliceId};

// ── Props & Actions ──────────────────────────────────────────────────

pub struct TechItem {
    pub id: String,
    pub name: String,
    pub cost: i32,
}

pub struct TechDialogProps {
    pub options: Vec<TechItem>,
    pub nine_slice_id: Option<NineSliceId>,
    /// Pre-computed snap size: (tile_w, tile_h) for the 9-slice center piece.
    /// Dialog dimensions are rounded up to exact multiples of these values.
    pub nine_slice_tile: Option<(f32, f32)>,
    /// UV rect for the advisor portrait (from the advisor atlas).
    pub advisor_uv: Option<[f32; 4]>,
}

pub enum TechDialogAction {
    SelectTech { tech_id: String },
    Close,
}

// ── Component State ──────────────────────────────────────────────────

#[derive(Default)]
pub struct TechDialogState {
    pub selected_index: usize,
    pub scroll_offset: usize,
}

// ── Visual Constants ─────────────────────────────────────────────────

const DIALOG_WIDTH: f32 = 360.0;
const ITEM_HEIGHT: f32 = 24.0;
const HEADER_HEIGHT: f32 = 64.0; // title + flavor text
const PADDING: f32 = 12.0;
const MAX_VISIBLE: usize = 12;
const ADVISOR_PORTRAIT_SIZE: f32 = 150.0;

const BG_COLOR: [f32; 4] = [0.08, 0.08, 0.14, 0.95];
const TEXT_COLOR: [u8; 3] = [0, 0, 0];
const HOVER_COLOR: [u8; 3] = [0, 0, 200];

// ── TechDialog Component ────────────────────────────────────────────

pub struct TechDialog;

impl Component for TechDialog {
    type Props = TechDialogProps;
    type State = TechDialogState;
    type Action = TechDialogAction;

    fn build(
        props: &Self::Props,
        state: &mut Self::State,
        ctx: &mut BuildCtx,
    ) -> (Element, Option<Self::Action>) {
        let item_count = props.options.len();
        if item_count == 0 {
            return (
                Element::Box {
                    style: BoxStyle {
                        x: 0.0,
                        y: 0.0,
                        width: 0.0,
                        height: 0.0,
                        background: Background::None,
                    },
                    children: Vec::new(),
                },
                Some(TechDialogAction::Close),
            );
        }

        // Clamp state
        let max_visible = MAX_VISIBLE.min(item_count);
        if state.selected_index >= item_count {
            state.selected_index = item_count - 1;
        }
        let max_scroll = item_count.saturating_sub(max_visible);

        // Handle keyboard input
        let mut action: Option<TechDialogAction> = None;

        if let Some(key) = ctx.key_pressed {
            match key {
                KeyCode::Escape => {
                    action = Some(TechDialogAction::Close);
                }
                KeyCode::ArrowUp => {
                    if state.selected_index > 0 {
                        state.selected_index -= 1;
                    }
                    if state.selected_index < state.scroll_offset {
                        state.scroll_offset = state.selected_index;
                    }
                }
                KeyCode::ArrowDown => {
                    if state.selected_index < item_count - 1 {
                        state.selected_index += 1;
                    }
                    if state.selected_index >= state.scroll_offset + max_visible {
                        state.scroll_offset = state.selected_index + 1 - max_visible;
                    }
                }
                KeyCode::Enter | KeyCode::NumpadEnter => {
                    if state.selected_index < item_count {
                        action = Some(TechDialogAction::SelectTech {
                            tech_id: props.options[state.selected_index].id.clone(),
                        });
                    }
                }
                _ => {}
            }
        }

        // Handle scroll
        if ctx.scroll_delta != 0.0 {
            if ctx.scroll_delta > 0.0 && state.scroll_offset < max_scroll {
                state.scroll_offset += 1;
            } else if ctx.scroll_delta < 0.0 && state.scroll_offset > 0 {
                state.scroll_offset -= 1;
            }
        }

        // Compute dialog dimensions, snapping to 9-slice tile multiples if available
        let desired_w = DIALOG_WIDTH;
        let desired_h = HEADER_HEIGHT + max_visible as f32 * ITEM_HEIGHT + 2.0 * PADDING;
        let (dialog_w, dialog_h) = if let Some((tile_w, tile_h)) = props.nine_slice_tile {
            let snap_w = (desired_w / tile_w).ceil() * tile_w;
            let snap_h = (desired_h / tile_h).ceil() * tile_h;
            (snap_w, snap_h)
        } else {
            (desired_w, desired_h)
        };

        // Center on screen
        let dialog_x = (ctx.screen_width - dialog_w) / 2.0;
        let dialog_y = (ctx.screen_height - dialog_h) / 2.0;

        // Build children
        let mut children: Vec<Element> = Vec::new();

        // Title
        children.push(Element::Text {
            text: "Science Advisor".to_string(),
            x: PADDING,
            y: PADDING,
            font_size: 20.0,
            max_width: dialog_w - 2.0 * PADDING,
            color: TEXT_COLOR,
        });

        // Flavor text
        children.push(Element::Text {
            text: "Master, our Prophets need direction.\nShall we look into the secrets of"
                .to_string(),
            x: PADDING,
            y: PADDING + 28.0,
            font_size: 12.0,
            max_width: dialog_w - 2.0 * PADDING,
            color: TEXT_COLOR,
        });

        // Items
        for vi in 0..max_visible {
            let idx = vi + state.scroll_offset;
            if idx >= item_count {
                break;
            }
            let item = &props.options[idx];
            let item_y = PADDING + HEADER_HEIGHT + vi as f32 * ITEM_HEIGHT;

            // Absolute screen position for hit testing
            let abs_x = dialog_x;
            let abs_y = dialog_y + item_y;
            let is_hovered = ctx.is_hovered_rect(abs_x + 2.0, abs_y, dialog_w - 4.0, ITEM_HEIGHT);
            let is_selected = state.selected_index == idx;

            // Check click
            if action.is_none()
                && ctx.is_clicked_rect(abs_x + 2.0, abs_y, dialog_w - 4.0, ITEM_HEIGHT)
            {
                action = Some(TechDialogAction::SelectTech {
                    tech_id: item.id.clone(),
                });
            }

            // Update selection on hover
            if is_hovered && !ctx.mouse_clicked {
                state.selected_index = idx;
            }

            // Text color: blue on hover/selected, black otherwise
            let color = if is_hovered || is_selected {
                HOVER_COLOR
            } else {
                TEXT_COLOR
            };

            // Item label
            children.push(Element::Text {
                text: item.name.clone(),
                x: PADDING + 4.0,
                y: item_y + 4.0,
                font_size: 14.0,
                max_width: dialog_w * 0.6,
                color,
            });

            // Cost detail (right side)
            children.push(Element::Text {
                text: format!("{} sci", item.cost),
                x: dialog_w - PADDING - 80.0,
                y: item_y + 4.0,
                font_size: 12.0,
                max_width: 80.0,
                color,
            });
        }

        // Scroll indicators
        if state.scroll_offset > 0 {
            children.push(Element::Text {
                text: "...".to_string(),
                x: dialog_w / 2.0 - 10.0,
                y: PADDING + HEADER_HEIGHT - 14.0,
                font_size: 12.0,
                max_width: 40.0,
                color: TEXT_COLOR,
            });
        }
        if state.scroll_offset + max_visible < item_count {
            let bottom_y = PADDING + HEADER_HEIGHT + max_visible as f32 * ITEM_HEIGHT;
            children.push(Element::Text {
                text: "...".to_string(),
                x: dialog_w / 2.0 - 10.0,
                y: bottom_y,
                font_size: 12.0,
                max_width: 40.0,
                color: TEXT_COLOR,
            });
        }

        // Background: 9-slice if available, otherwise solid color
        let background = match props.nine_slice_id {
            Some(id) => Background::NineSlice { image_id: id },
            None => Background::Color(BG_COLOR),
        };

        let dialog_box = Element::Box {
            style: BoxStyle {
                x: dialog_x,
                y: dialog_y,
                width: dialog_w,
                height: dialog_h,
                background,
            },
            children,
        };

        // Wrap dialog + advisor portrait in an outer container.
        // The advisor portrait sits above the dialog, right-aligned.
        let mut outer_children = Vec::new();
        if let Some(uv) = props.advisor_uv {
            outer_children.push(Element::Image {
                x: dialog_x + dialog_w - ADVISOR_PORTRAIT_SIZE,
                y: dialog_y - ADVISOR_PORTRAIT_SIZE,
                width: ADVISOR_PORTRAIT_SIZE,
                height: ADVISOR_PORTRAIT_SIZE,
                uv_rect: uv,
            });
        }
        outer_children.push(dialog_box);

        let element = Element::Box {
            style: BoxStyle {
                x: 0.0,
                y: 0.0,
                width: ctx.screen_width,
                height: ctx.screen_height,
                background: Background::None,
            },
            children: outer_children,
        };

        (element, action)
    }
}
