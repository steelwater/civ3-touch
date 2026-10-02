use crate::instances::UITexturedRectInstance;

/// How a box is filled — currently only atlas-sourced textures.
pub enum BoxImage {
    Atlas { u: f32, v: f32, w: f32, h: f32 },
}

/// Visual state of a UIBox for determining which image to display.
pub enum BoxState {
    Default,
    Hovered,
    Active,
}

/// A screen-space interactive box with hover/active image states.
pub struct UIBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub default_image: BoxImage,
    pub hover_image: Option<BoxImage>,
    pub active_image: Option<BoxImage>,
}

impl UIBox {
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }

    pub fn resolve_image(&self, state: &BoxState) -> &BoxImage {
        match state {
            BoxState::Active => self
                .active_image
                .as_ref()
                .or(self.hover_image.as_ref())
                .unwrap_or(&self.default_image),
            BoxState::Hovered => self.hover_image.as_ref().unwrap_or(&self.default_image),
            BoxState::Default => &self.default_image,
        }
    }

    pub fn to_instance(&self, state: &BoxState, vw: f32, vh: f32) -> UITexturedRectInstance {
        let image = self.resolve_image(state);
        let BoxImage::Atlas { u, v, w, h } = image;
        UITexturedRectInstance {
            rect: [self.x, self.y, self.width, self.height],
            atlas_rect: [*u, *v, *w, *h],
            viewport: [vw, vh],
        }
    }
}
