package org.civ3touch.spike;

/** Retained presentation coordinates; never participates in simulation commands. */
final class MapCamera {
    float x, y, zoom = 1;
    boolean initialized;

    void center(float x, float y) { this.x = x; this.y = y; initialized = true; }
    void pan(float dx, float dy, float halfWidth, float halfHeight, int width, int height) {
        x -= (dx / halfWidth + dy / halfHeight) / 2;
        y -= (dy / halfHeight - dx / halfWidth) / 2;
        clamp(width, height);
    }
    void scale(float factor, float focusX, float focusY, float halfWidth, float halfHeight,
            int width, int height) {
        float next = Math.max(.65f, Math.min(2f, zoom * factor));
        float ratio = zoom / next;
        x += (focusX / halfWidth + focusY / halfHeight) / 2 * (1 - ratio);
        y += (focusY / halfHeight - focusX / halfWidth) / 2 * (1 - ratio);
        zoom = next;
        clamp(width, height);
    }
    private void clamp(int width, int height) {
        x = Math.max(0, Math.min(width - 1, x));
        y = Math.max(0, Math.min(height - 1, y));
    }
}
