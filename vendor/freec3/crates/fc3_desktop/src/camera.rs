use fc3_core::types::TileCoord;

/// Convert logical tile coordinates to isometric world coordinates.
/// iso_x = lx - ly, iso_y = (lx + ly) * 0.5
#[inline]
pub fn logical_to_iso(lx: f32, ly: f32) -> (f32, f32) {
    (lx - ly, (lx + ly) * 0.5)
}

/// Convert isometric world coordinates back to logical tile coordinates.
/// lx = ix/2 + iy, ly = iy - ix/2
#[inline]
pub fn iso_to_logical(ix: f32, iy: f32) -> (f32, f32) {
    (ix / 2.0 + iy, iy - ix / 2.0)
}

/// Adjust a tile's logical Y for y-axis wrapping display.
///
/// When wrap_y is active, the torus is unrolled into a rectangle by mapping
/// each tile's Y based on its diagonal `(x + y) % H`. This collapses the
/// wrapped Y-axis so that iso_y ranges over `[0, H/2)` instead of `[0, (W+H)/2)`.
///
/// Formula: `display_y = y - H * floor((x + y) / H)`
#[inline]
pub fn adjust_y_for_wrap(x: f32, y: f32, map_height: f32, wrap_y: bool) -> f32 {
    if wrap_y {
        y - map_height * ((x + y) / map_height).floor()
    } else {
        y
    }
}

/// Camera for 2D isometric map rendering.
///
/// Camera center lives in iso world space. The shader performs the
/// logical-to-iso transform per-instance, so instance data stays in
/// logical tile coordinates.
pub struct Camera {
    /// Center of the viewport in iso world coordinates.
    pub center_x: f32,
    pub center_y: f32,
    /// Pixels per iso unit.
    pub zoom: f32,
    /// Viewport dimensions in pixels.
    pub viewport_width: f32,
    pub viewport_height: f32,
    /// Map dimensions in logical tiles (for clamping).
    pub map_width: f32,
    pub map_height: f32,
    /// Whether the map wraps on the logical x-axis.
    pub wrap_x: bool,
    /// Whether the map wraps on the logical y-axis.
    pub wrap_y: bool,
}

/// GPU-uploadable camera uniform. Must match the WGSL struct layout.
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    pub center: [f32; 2],
    pub zoom: f32,
    pub aspect: f32,
    pub viewport_w: f32,
    pub viewport_h: f32,
    /// Map height in logical tiles (used by shader for y-axis wrap adjustment).
    pub map_height: f32,
    /// Whether y-axis wrapping is active (0.0 = false, 1.0 = true).
    pub wrap_y: f32,
}

impl Camera {
    /// Iso extent of the full map (width, height) in iso units.
    fn iso_extent(map_width: f32, map_height: f32, wrap_y: bool) -> (f32, f32) {
        if wrap_y {
            // With y-axis wrapping, adjust_y_for_wrap collapses iso_y to [0, H/2].
            // iso_x spans roughly [-H, 2W], so width ≈ 2W + H.
            (2.0 * map_width + map_height, map_height / 2.0)
        } else {
            // Standard iso bounding box of a W x H logical grid:
            // iso_x ranges from -(H) to +(W), so width = W + H
            // iso_y ranges from 0 to (W+H)/2, so height = (W + H) / 2
            (map_width + map_height, (map_width + map_height) / 2.0)
        }
    }

    pub fn new(
        map_width: f32,
        map_height: f32,
        viewport_width: f32,
        viewport_height: f32,
        wrap_x: bool,
        wrap_y: bool,
    ) -> Self {
        let min_zoom = Self::compute_min_zoom(
            map_width,
            map_height,
            viewport_width,
            viewport_height,
            wrap_x,
            wrap_y,
        );
        // Fixed zoom: 64 px per iso unit = 1:1 pixel mapping for 128×64 terrain tiles.
        let zoom = min_zoom.max(64.0);

        // Center on the logical map center, using display Y for wrap_y
        let center_lx = map_width / 2.0;
        let center_ly = map_height / 2.0;
        let display_ly = adjust_y_for_wrap(center_lx, center_ly, map_height, wrap_y);
        let (cx, cy) = logical_to_iso(center_lx, display_ly);

        Camera {
            center_x: cx,
            center_y: cy,
            zoom,
            viewport_width,
            viewport_height,
            map_width,
            map_height,
            wrap_x,
            wrap_y,
        }
    }

    /// Minimum zoom so the viewport never exceeds the iso map extent.
    /// For wrapping maps, width is infinite so only the height constraint applies.
    fn compute_min_zoom(
        map_width: f32,
        map_height: f32,
        viewport_width: f32,
        viewport_height: f32,
        wrap_x: bool,
        wrap_y: bool,
    ) -> f32 {
        let (iso_w, iso_h) = Self::iso_extent(map_width, map_height, wrap_y);
        let zoom_y = viewport_height / iso_h;
        if wrap_x {
            zoom_y.max(4.0)
        } else {
            let zoom_x = viewport_width / iso_w;
            zoom_x.max(zoom_y).max(4.0)
        }
    }

    /// Pan the camera by pixel deltas, clamped/wrapped to map bounds.
    pub fn pan(&mut self, dx_pixels: f32, dy_pixels: f32) {
        self.center_x -= dx_pixels / self.zoom;
        self.center_y -= dy_pixels / self.zoom;
        self.constrain_position();
    }

    /// Update viewport dimensions (e.g., on window resize).
    pub fn set_viewport(&mut self, width: f32, height: f32) {
        self.viewport_width = width;
        self.viewport_height = height;
        let min_zoom = Self::compute_min_zoom(
            self.map_width,
            self.map_height,
            self.viewport_width,
            self.viewport_height,
            self.wrap_x,
            self.wrap_y,
        );
        if self.zoom < min_zoom {
            self.zoom = min_zoom;
        }
        self.constrain_position();
    }

    /// Clamp camera center in iso space to keep the map visible.
    /// For wrapping maps, only center_y is constrained (center_x is free).
    /// For non-wrapping maps, both axes are constrained.
    fn constrain_position(&mut self) {
        let iso_half_w = self.viewport_width / (2.0 * self.zoom);
        let iso_half_h = self.viewport_height / (2.0 * self.zoom);

        // The iso_y range depends on whether y-axis wrapping is active.
        // With wrap_y, adjust_y_for_wrap collapses iso_y to [0, H/2].
        // Without wrap_y, iso_y spans [0, (W+H)/2].
        let iso_y_max = if self.wrap_y {
            self.map_height / 2.0
        } else {
            (self.map_width + self.map_height) / 2.0
        };

        // Constrain center_y so the viewport stays within the map's vertical extent
        let min_cy = iso_half_h;
        let max_cy = iso_y_max - iso_half_h;
        if min_cy >= max_cy {
            self.center_y = iso_y_max / 2.0;
        } else {
            self.center_y = self.center_y.clamp(min_cy, max_cy);
        }

        if !self.wrap_x {
            let iso_x_min = -self.map_height;
            let iso_x_max = if self.wrap_y {
                2.0 * self.map_width
            } else {
                self.map_width
            };
            let min_cx = iso_x_min + iso_half_w;
            let max_cx = iso_x_max - iso_half_w;
            if min_cx >= max_cx {
                self.center_x = (iso_x_min + iso_x_max) / 2.0;
            } else {
                self.center_x = self.center_x.clamp(min_cx, max_cx);
            }
        }
    }

    /// Returns the logical x offsets at which instances should be duplicated for wrapping.
    /// For wrapping maps, dynamically computes all multiples of map_width that overlap
    /// with the viewport's logical x range, so the camera works at any position.
    pub fn wrap_offsets(&self) -> Vec<f32> {
        if !self.wrap_x {
            return vec![0.0];
        }

        let w = self.map_width;
        let iso_half_w = self.viewport_width / (2.0 * self.zoom);

        if self.wrap_y {
            // When wrap_y is active, the shader's y-adjustment means each logical
            // offset of W shifts iso_x by 2*W (not W). We compute which offsets n
            // produce iso coverage overlapping the viewport directly in iso space.
            //
            // Canonical tiles cover iso_x in [-(H-1), 2W-H]. Use conservative
            // bounds [-H, 2W] so we never miss an offset.
            let h = self.map_height;
            let two_w = 2.0 * w;
            let n_min = ((self.center_x - iso_half_w - two_w) / two_w).floor() as i32;
            let n_max = ((self.center_x + iso_half_w + h) / two_w).ceil() as i32;
            (n_min..=n_max).map(|n| n as f32 * w).collect()
        } else {
            let iso_half_h = self.viewport_height / (2.0 * self.zoom);
            let corners = [
                iso_to_logical(self.center_x - iso_half_w, self.center_y - iso_half_h),
                iso_to_logical(self.center_x + iso_half_w, self.center_y - iso_half_h),
                iso_to_logical(self.center_x - iso_half_w, self.center_y + iso_half_h),
                iso_to_logical(self.center_x + iso_half_w, self.center_y + iso_half_h),
            ];
            let mut min_lx = f32::MAX;
            let mut max_lx = f32::MIN;
            for &(lx, _) in &corners {
                min_lx = min_lx.min(lx);
                max_lx = max_lx.max(lx);
            }
            // Add 1-tile margin to avoid popping at edges
            min_lx -= 1.0;
            max_lx += 1.0;

            let min_n = (min_lx / w).floor() as i32;
            let max_n = (max_lx / w).floor() as i32;
            (min_n..=max_n).map(|n| n as f32 * w).collect()
        }
    }

    /// Returns the range of visible tiles as (min_x, min_y, max_x, max_y).
    #[allow(dead_code)]
    pub fn visible_tile_range(&self) -> (i32, i32, i32, i32) {
        let iso_half_w = self.viewport_width / (2.0 * self.zoom);
        let iso_half_h = self.viewport_height / (2.0 * self.zoom);
        // Convert iso viewport corners to logical and take bounding box
        let corners = [
            iso_to_logical(self.center_x - iso_half_w, self.center_y - iso_half_h),
            iso_to_logical(self.center_x + iso_half_w, self.center_y - iso_half_h),
            iso_to_logical(self.center_x - iso_half_w, self.center_y + iso_half_h),
            iso_to_logical(self.center_x + iso_half_w, self.center_y + iso_half_h),
        ];
        let min_x = corners.iter().map(|c| c.0).fold(f32::MAX, f32::min).floor() as i32 - 1;
        let min_y = corners.iter().map(|c| c.1).fold(f32::MAX, f32::min).floor() as i32 - 1;
        let max_x = corners.iter().map(|c| c.0).fold(f32::MIN, f32::max).ceil() as i32 + 1;
        let max_y = corners.iter().map(|c| c.1).fold(f32::MIN, f32::max).ceil() as i32 + 1;
        (min_x, min_y, max_x, max_y)
    }

    /// Convert screen pixel coordinates to iso world coordinates.
    pub fn screen_to_world(&self, screen_x: f32, screen_y: f32) -> (f32, f32) {
        let iso_x = (screen_x - self.viewport_width / 2.0) / self.zoom + self.center_x;
        let iso_y = (screen_y - self.viewport_height / 2.0) / self.zoom + self.center_y;
        (iso_x, iso_y)
    }

    /// Convert screen pixel coordinates to a tile coordinate, handling wrapping.
    /// screen → iso world → logical tile
    pub fn screen_to_tile(
        &self,
        screen_x: f32,
        screen_y: f32,
        map_width: u32,
        map_height: u32,
    ) -> Option<TileCoord> {
        let (iso_x, iso_y) = self.screen_to_world(screen_x, screen_y);
        let (lx, ly) = iso_to_logical(iso_x, iso_y);

        let mut tx = lx.floor() as i32;
        let mut ty = ly.floor() as i32;

        if self.wrap_y {
            ty = ty.rem_euclid(map_height as i32);
        } else if ty < 0 || ty >= map_height as i32 {
            return None;
        }

        if self.wrap_x {
            tx = tx.rem_euclid(map_width as i32);
        } else if tx < 0 || tx >= map_width as i32 {
            return None;
        }

        Some(TileCoord {
            x: tx as u32,
            y: ty as u32,
        })
    }

    /// Center the camera on a specific tile coordinate.
    /// For wrapping maps, picks the closest logical x equivalent to minimize camera movement.
    /// Uses display-adjusted Y when wrap_y is active to match shader coordinates.
    pub fn center_on(&mut self, coord: TileCoord) {
        let target_lx = coord.x as f32 + 0.5;
        let target_ly = coord.y as f32 + 0.5;

        let lx = if self.wrap_x {
            let (current_lx, _) = iso_to_logical(self.center_x, self.center_y);
            let n = ((current_lx - target_lx) / self.map_width).round();
            target_lx + n * self.map_width
        } else {
            target_lx
        };

        // Use display Y so the camera targets where the tile is actually rendered
        let display_ly = adjust_y_for_wrap(lx, target_ly, self.map_height, self.wrap_y);
        let (ix, iy) = logical_to_iso(lx, display_ly);
        self.center_x = ix;
        self.center_y = iy;
        self.constrain_position();
    }

    /// Convert to GPU uniform.
    pub fn to_uniform(&self) -> CameraUniform {
        CameraUniform {
            center: [self.center_x, self.center_y],
            zoom: self.zoom,
            aspect: self.viewport_width / self.viewport_height,
            viewport_w: self.viewport_width,
            viewport_h: self.viewport_height,
            map_height: self.map_height,
            wrap_y: if self.wrap_y { 1.0 } else { 0.0 },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logical_iso_roundtrip() {
        let (ix, iy) = logical_to_iso(10.0, 5.0);
        let (lx, ly) = iso_to_logical(ix, iy);
        assert!((lx - 10.0).abs() < 0.001);
        assert!((ly - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_camera_new_centers_on_map() {
        let cam = Camera::new(40.0, 25.0, 800.0, 600.0, false, false);
        // Without wrap_y, display_y = logical_y, so center is at logical_to_iso(20, 12.5)
        let (expected_cx, expected_cy) = logical_to_iso(20.0, 12.5);
        assert!((cam.center_x - expected_cx).abs() < 0.01);
        assert!((cam.center_y - expected_cy).abs() < 0.01);
    }

    #[test]
    fn test_camera_new_centers_on_map_wrap_y() {
        let cam = Camera::new(50.0, 32.0, 1280.0, 720.0, true, true);
        // With wrap_y, center uses display Y: adjust_y_for_wrap(25, 16, 32) = 16 - 32*floor(41/32) = -16
        let display_ly = adjust_y_for_wrap(25.0, 16.0, 32.0, true);
        let (expected_cx, expected_cy) = logical_to_iso(25.0, display_ly);
        assert!((cam.center_x - expected_cx).abs() < 0.01);
        assert!((cam.center_y - expected_cy).abs() < 0.01);
        // iso_y should be in [0, H/2] range
        assert!(
            cam.center_y >= 0.0 && cam.center_y <= 16.0,
            "center_y {} should be within [0, 16]",
            cam.center_y
        );
    }

    #[test]
    fn test_camera_wrap_x_pans_freely() {
        let mut cam = Camera::new(50.0, 32.0, 1280.0, 720.0, true, false);
        cam.zoom = 40.0; // zoom in so constraints don't dominate
        let cx_before = cam.center_x;
        let cy_before = cam.center_y;
        // Pan horizontally in screen space (large dx, zero dy)
        cam.pan(5000.0, 0.0);
        // center_x should move freely (no wrapping or clamping)
        assert!(
            (cam.center_x - cx_before).abs() > 10.0,
            "center_x should have moved, was {} now {}",
            cx_before,
            cam.center_x
        );
        // center_y should be unchanged — iso-space constraint only affects Y
        assert!(
            (cam.center_y - cy_before).abs() < 0.01,
            "center_y should be unchanged, was {} now {}",
            cy_before,
            cam.center_y
        );
    }

    #[test]
    fn test_camera_wrap_offsets_wrapping() {
        let cam = Camera::new(50.0, 32.0, 1280.0, 720.0, true, false);
        let offsets = cam.wrap_offsets();
        // Offsets should be dynamic multiples of W that cover the viewport
        assert!(offsets.contains(&0.0), "offsets must include 0.0");
        for &offset in &offsets {
            let remainder = (offset / 50.0).round() * 50.0 - offset;
            assert!(
                remainder.abs() < 0.01,
                "offset {} is not a multiple of W",
                offset
            );
        }
    }

    #[test]
    fn test_camera_wrap_offsets_no_wrap() {
        let cam = Camera::new(40.0, 25.0, 800.0, 600.0, false, false);
        let offsets = cam.wrap_offsets();
        assert_eq!(offsets, vec![0.0]);
    }

    #[test]
    fn test_camera_fixed_zoom() {
        let cam = Camera::new(40.0, 25.0, 800.0, 600.0, false, false);
        // Zoom should be fixed at 64.0 (1:1 pixel mapping for 128x64 terrain tiles)
        assert!((cam.zoom - 64.0).abs() < 0.01);
    }

    #[test]
    fn test_camera_uniform_roundtrip() {
        let cam = Camera::new(40.0, 25.0, 800.0, 600.0, false, false);
        let u = cam.to_uniform();
        assert_eq!(u.center[0], cam.center_x);
        assert_eq!(u.center[1], cam.center_y);
        assert_eq!(u.zoom, cam.zoom);
    }

    #[test]
    fn test_screen_to_world_center() {
        let cam = Camera::new(40.0, 25.0, 800.0, 600.0, false, false);
        let (wx, wy) = cam.screen_to_world(400.0, 300.0);
        assert!((wx - cam.center_x).abs() < 0.01);
        assert!((wy - cam.center_y).abs() < 0.01);
    }

    #[test]
    fn test_screen_to_tile_center() {
        let mut cam = Camera::new(40.0, 25.0, 800.0, 600.0, false, false);
        cam.zoom = 40.0;
        // Center on tile (20, 12) in iso space
        let (ix, iy) = logical_to_iso(20.5, 12.5);
        cam.center_x = ix;
        cam.center_y = iy;
        // Screen center should map to tile (20, 12)
        let tile = cam.screen_to_tile(400.0, 300.0, 40, 25);
        assert_eq!(tile, Some(TileCoord { x: 20, y: 12 }));
    }

    #[test]
    fn test_screen_to_tile_wrapping() {
        let mut cam = Camera::new(50.0, 32.0, 1280.0, 720.0, true, false);
        cam.zoom = 40.0;
        let (ix, iy) = logical_to_iso(1.0, 16.0);
        cam.center_x = ix;
        cam.center_y = iy;
        // Screen left of center should be negative x, which wraps
        let tile = cam.screen_to_tile(0.0, 360.0, 50, 32);
        assert!(tile.is_some());
        assert!(tile.unwrap().x < 50);
    }

    #[test]
    fn test_screen_to_tile_out_of_bounds_y() {
        let cam = Camera::new(40.0, 25.0, 800.0, 600.0, false, false);
        // Far above the map
        let tile = cam.screen_to_tile(400.0, -10000.0, 40, 25);
        assert!(tile.is_none());
    }

    #[test]
    fn test_center_on_tile() {
        let mut cam = Camera::new(80.0, 50.0, 800.0, 600.0, false, false);
        // Zoom in enough that centering on a mid-map tile won't be constrained
        cam.zoom = 60.0;
        cam.center_on(TileCoord { x: 40, y: 25 });
        let (expected_ix, expected_iy) = logical_to_iso(40.5, 25.5);
        assert!((cam.center_x - expected_ix).abs() < 0.01);
        assert!((cam.center_y - expected_iy).abs() < 0.01);
    }

    #[test]
    fn test_center_on_wrapping_closest() {
        let mut cam = Camera::new(50.0, 32.0, 1280.0, 720.0, true, false);
        cam.zoom = 40.0;
        // Position camera at logical x ~48 (near right edge)
        let (ix, iy) = logical_to_iso(48.0, 16.0);
        cam.center_x = ix;
        cam.center_y = iy;
        // Center on tile x=2 — should pick the +50 equivalent (near 52.5), not x=2.5
        cam.center_on(TileCoord { x: 2, y: 16 });
        let (lx, _) = iso_to_logical(cam.center_x, cam.center_y);
        // Should have chosen the +50 wrap (lx near 50+), not the direct route (lx near 2.5)
        assert!(lx > 40.0, "expected lx > 40 (chose +50 wrap), got {}", lx);
    }

    #[test]
    fn test_wrap_offsets_far_from_origin() {
        let mut cam = Camera::new(50.0, 32.0, 1280.0, 720.0, true, false);
        cam.zoom = 40.0;
        // Move camera far from origin to logical x ~200
        let (ix, iy) = logical_to_iso(200.0, 16.0);
        cam.center_x = ix;
        cam.center_y = iy;
        let offsets = cam.wrap_offsets();
        // Offsets should cover the area near lx=200, not just [-W, 0, W]
        let has_nearby = offsets.iter().any(|&o| (o - 200.0).abs() < 50.0);
        assert!(
            has_nearby,
            "offsets {:?} should include values near 200",
            offsets
        );
        // Should NOT include 0.0 since that's far from the viewport
        // (at zoom 40, viewport ~16 iso units wide → ~16 logical units)
        assert!(
            !offsets.contains(&0.0),
            "offsets {:?} should not include 0.0 when camera is at lx=200",
            offsets
        );
    }

    #[test]
    fn test_horizontal_pan_does_not_shift_vertically() {
        // This was the core bug: logical-space clamping caused iso center_y
        // to shift when panning purely horizontally, because the coordinate
        // systems are rotated 45 degrees.
        let mut cam = Camera::new(50.0, 32.0, 1280.0, 720.0, true, false);
        cam.zoom = 40.0;
        let cy_before = cam.center_y;
        // Pan purely left in screen space
        cam.pan(-2000.0, 0.0);
        assert!(
            (cam.center_y - cy_before).abs() < 0.01,
            "horizontal pan should not change center_y: was {} now {}",
            cy_before,
            cam.center_y
        );
        // Pan purely right
        cam.pan(4000.0, 0.0);
        assert!(
            (cam.center_y - cy_before).abs() < 0.01,
            "horizontal pan should not change center_y: was {} now {}",
            cy_before,
            cam.center_y
        );
    }

    #[test]
    fn test_non_wrapping_constraint_in_iso_space() {
        let mut cam = Camera::new(40.0, 25.0, 800.0, 600.0, false, false);
        cam.zoom = 40.0;
        // Pan way past the map bounds
        cam.pan(-100000.0, -100000.0);
        // Camera should be constrained within the non-wrap_y iso bounding box
        let iso_y_max = (40.0 + 25.0) / 2.0;
        assert!(
            cam.center_y >= 0.0 && cam.center_y <= iso_y_max,
            "center_y {} should be within [0, {}]",
            cam.center_y,
            iso_y_max
        );
        assert!(
            cam.center_x >= -25.0 && cam.center_x <= 40.0,
            "center_x {} should be within [-25, 40]",
            cam.center_x
        );
    }

    #[test]
    fn test_wrap_y_constraint_tighter_than_non_wrap() {
        let cam_wrap = Camera::new(50.0, 32.0, 1280.0, 720.0, true, true);
        let cam_no_wrap = Camera::new(50.0, 32.0, 1280.0, 720.0, true, false);
        // With wrap_y, iso_y_max = H/2 = 16; without, iso_y_max = (W+H)/2 = 41
        // The wrap_y camera center_y should be within [0, 16]
        assert!(
            cam_wrap.center_y >= 0.0 && cam_wrap.center_y <= 16.0,
            "wrap_y center_y {} should be within [0, 16]",
            cam_wrap.center_y
        );
        assert!(
            cam_no_wrap.center_y >= 0.0 && cam_no_wrap.center_y <= 41.0,
            "no_wrap center_y {} should be within [0, 41]",
            cam_no_wrap.center_y
        );
    }

    #[test]
    fn test_min_zoom_wrapping_only_constrains_height() {
        let min_wrap = Camera::compute_min_zoom(50.0, 32.0, 800.0, 600.0, true, false);
        let min_no_wrap = Camera::compute_min_zoom(50.0, 32.0, 800.0, 600.0, false, false);
        // Wrapping min zoom should be <= non-wrapping (no width constraint)
        assert!(
            min_wrap <= min_no_wrap,
            "wrapping min_zoom {} should be <= non-wrapping {}",
            min_wrap,
            min_no_wrap
        );
    }

    #[test]
    fn test_screen_to_tile_wrap_y() {
        let mut cam = Camera::new(64.0, 32.0, 1280.0, 720.0, true, true);
        // Zoom in enough that viewport (iso half-height = 3) fits within
        // the wrap_y iso extent (0..16), allowing tiles to be centered.
        cam.zoom = 120.0;
        // Center on tile (25, 16) — iso_y ≈ 5
        cam.center_on(TileCoord { x: 25, y: 16 });
        let tile = cam.screen_to_tile(640.0, 360.0, 64, 32);
        assert_eq!(tile, Some(TileCoord { x: 25, y: 16 }));

        // Center on tile near y=0 — iso_y ≈ 13
        cam.center_on(TileCoord { x: 25, y: 0 });
        let tile2 = cam.screen_to_tile(640.0, 360.0, 64, 32);
        assert_eq!(tile2, Some(TileCoord { x: 25, y: 0 }));
    }

    #[test]
    fn test_screen_to_tile_wrap_xy_cross_boundary() {
        // With wrap_x + wrap_y and W%H==0, clicking on a wrap copy of a tile
        // must return the correct original tile coordinates.
        let mut cam = Camera::new(64.0, 32.0, 1280.0, 720.0, true, true);
        cam.zoom = 120.0;

        // Test tiles in the middle iso_y range (avoid poles where the camera
        // constraint clamps center_y away from the tile).
        for &(tx, ty) in &[(25, 16), (10, 6), (40, 10), (0, 15), (63, 16)] {
            cam.center_on(TileCoord { x: tx, y: ty });
            let tile = cam.screen_to_tile(640.0, 360.0, 64, 32);
            assert_eq!(
                tile,
                Some(TileCoord { x: tx, y: ty }),
                "screen center should map to tile ({}, {})",
                tx,
                ty
            );
        }
    }

    #[test]
    fn test_center_on_wrap_xy_uses_closest_period() {
        let mut cam = Camera::new(64.0, 32.0, 1280.0, 720.0, true, true);
        cam.zoom = 40.0;
        // Position camera near tile (62, 16)
        cam.center_on(TileCoord { x: 62, y: 16 });
        let cx_before = cam.center_x;
        // Center on tile (2, 16) — should pick the closest wrap copy
        cam.center_on(TileCoord { x: 2, y: 16 });
        // Camera should have moved to a nearby wrap copy, not jumped far
        let delta = (cam.center_x - cx_before).abs();
        assert!(
            delta < 2.0 * 64.0,
            "camera should pick closest wrap copy, delta {} too large",
            delta
        );
    }

    #[test]
    fn test_wrap_offsets_wrap_y_covers_viewport() {
        // When wrap_y is active, each logical offset of W shifts iso_x by 2*W.
        // The wrap_offsets must always produce enough copies to cover the viewport.
        let mut cam = Camera::new(64.0, 32.0, 1280.0, 720.0, true, true);
        cam.zoom = 120.0;

        // Test at various iso_x positions that previously caused black gaps.
        // The canonical tiles cover iso_x in [-31, 96]. Each offset of W=64
        // shifts iso_x by 128. Test positions near the boundary (iso_x ~ 96).
        for center_x in [96.0, 100.0, 112.0, 128.0, 200.0, 224.0, -100.0, -200.0] {
            cam.center_x = center_x;
            cam.center_y = 8.0;
            let offsets = cam.wrap_offsets();
            let iso_half_w = cam.viewport_width / (2.0 * cam.zoom);
            let vp_min = center_x - iso_half_w;
            let vp_max = center_x + iso_half_w;

            // Check that some offset's iso coverage includes the viewport center.
            // Each offset n*W has iso coverage [-(H-1) + 2*n*W, (2W-H) + 2*n*W].
            let h = cam.map_height;
            let w = cam.map_width;
            let covered = offsets.iter().any(|&off| {
                let n = (off / w).round() as i32;
                let iso_min = -(h - 1.0) + 2.0 * n as f32 * w;
                let iso_max = (2.0 * w - h) + 2.0 * n as f32 * w;
                iso_min <= vp_min && iso_max >= vp_max
            });
            // At minimum, the viewport must be fully covered by the union of offsets
            let union_covered = {
                let mut ranges: Vec<(f32, f32)> = offsets
                    .iter()
                    .map(|&off| {
                        let n = (off / w).round() as i32;
                        let iso_min = -(h - 1.0) + 2.0 * n as f32 * w;
                        let iso_max = (2.0 * w - h) + 2.0 * n as f32 * w;
                        (iso_min, iso_max)
                    })
                    .collect();
                ranges.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
                // Check that merged ranges cover [vp_min, vp_max]
                let mut cover_up_to = vp_min;
                for (lo, hi) in &ranges {
                    if *lo <= cover_up_to + 1.0 {
                        // +1.0 accounts for diamond vertex extent
                        cover_up_to = cover_up_to.max(*hi);
                    }
                }
                cover_up_to >= vp_max
            };
            assert!(
                covered || union_covered,
                "wrap_offsets {:?} don't cover viewport [{}, {}] at center_x={}",
                offsets,
                vp_min,
                vp_max,
                center_x
            );
        }
    }
}
