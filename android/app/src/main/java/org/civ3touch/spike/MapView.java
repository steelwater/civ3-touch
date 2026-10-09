package org.civ3touch.spike;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Bitmap;
import android.graphics.Rect;
import android.graphics.RectF;
import android.os.SystemClock;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.Path;
import android.view.MotionEvent;
import android.view.GestureDetector;
import android.view.ScaleGestureDetector;
import android.view.View;

/** Presentation only: decoded imported art over the native player view. */
final class MapView extends View {
    interface TileTap { void tap(int x, int y); }
    private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Path diamond = new Path();
    private float halfWidth, halfHeight;
    private final float density;
    private final MapCamera camera;
    private final GestureDetector gestures;
    private final ScaleGestureDetector scaling;
    private boolean multiTouch, tapped;
    private TileTap onLongPress;
    private TileTap onTap;
    private GameState state;
    private boolean selected;
    private ImportedAssets assets;
    private long moveStarted;
    private int moveDx, moveDy;
    private final Rect source = new Rect();
    private final RectF destination = new RectF();

    public MapView(Context context) { this(context, new MapCamera()); }

    public MapView(Context context, MapCamera camera) {
        super(context);
        this.camera = camera;
        density = getResources().getDisplayMetrics().density;
        gestures = new GestureDetector(context, new GestureDetector.SimpleOnGestureListener() {
            @Override public boolean onDown(MotionEvent e) { return true; }
            @Override public boolean onScroll(MotionEvent first, MotionEvent current, float dx, float dy) {
                if (!multiTouch && state != null) {
                    camera.pan(-dx, -dy, halfWidth, halfHeight, state.width, state.height);
                    describeViewport(); invalidate();
                }
                return true;
            }
            @Override public boolean onSingleTapUp(MotionEvent e) {
                if (!multiTouch) tapped = true;
                return true;
            }
            @Override public void onLongPress(MotionEvent e) {
                if (!multiTouch) dispatchTile(e, onLongPress);
            }
        });
        scaling = new ScaleGestureDetector(context, new ScaleGestureDetector.SimpleOnScaleGestureListener() {
            @Override public boolean onScale(ScaleGestureDetector detector) {
                camera.scale(detector.getScaleFactor(), detector.getFocusX() - getWidth() / 2f,
                        detector.getFocusY() - getHeight() / 2f, halfWidth, halfHeight, state.width, state.height);
                updateScale(); describeViewport(); invalidate(); return true;
            }
        });
        scaling.setQuickScaleEnabled(false);
        updateScale();
        setClickable(true);
        setFocusable(true);
        setContentDescription("Generated map. Start a new game.");
    }

    void setOnTileLongPress(TileTap callback) { onLongPress = callback; }
    private void updateScale() { halfWidth = 48 * density * camera.zoom; halfHeight = 28 * density * camera.zoom; }
    void centerSelection() {
        if (state != null) { camera.center(state.x, state.y); describeViewport(); invalidate(); }
    }
    void setOnTileTap(TileTap onTap) { this.onTap = onTap; }

    void display(GameState next, boolean isSelected, ImportedAssets imported) {
        if (state != null && next != null && state.index == next.index && state.turn <= next.turn
                && next.unitMoved && (state.x != next.x || state.y != next.y)) {
            moveDx = next.x - state.x; moveDy = next.y - state.y;
            moveStarted = SystemClock.uptimeMillis();
        } else if (next == null || state == null || (next != state && !next.unitMoved)) {
            moveStarted = 0;
        }
        if (next != null && (!camera.initialized || (state != null && (state.index != next.index
                || state.generation != next.generation || state.x != next.x || state.y != next.y)))) {
            camera.center(next.x, next.y);
        }
        assets = imported;
        state = next;
        selected = isSelected;
        describeViewport();
        invalidate();
    }

    private void describeViewport() {
        if (state == null) setContentDescription("Generated map. Start a new game.");
        else setContentDescription("Generated map. " + state.name + " at " + state.x + ", " + state.y
                + (selected ? ". Selected." : ". Tap a unit to select; Actions lists stacked units.")
                + String.format(java.util.Locale.ROOT, " View center %.2f, %.2f; zoom %.2f. Drag to pan; pinch to scale; long press to inspect.", camera.x, camera.y, camera.zoom));
    }

    private float screenX(int x, int y) { return getWidth() / 2f + ((x - camera.x) - (y - camera.y)) * halfWidth; }
    private float screenY(int x, int y) { return getHeight() / 2f + ((x - camera.x) + (y - camera.y)) * halfHeight; }

    private void tile(Canvas canvas, int x, int y, int color, boolean stroke) {
        float cx = screenX(x, y), cy = screenY(x, y);
        diamond.reset();
        diamond.moveTo(cx, cy - halfHeight);
        diamond.lineTo(cx + halfWidth, cy);
        diamond.lineTo(cx, cy + halfHeight);
        diamond.lineTo(cx - halfWidth, cy);
        diamond.close();
        paint.setColor(color);
        paint.setStyle(stroke ? Paint.Style.STROKE : Paint.Style.FILL);
        paint.setStrokeWidth(2 * getResources().getDisplayMetrics().density);
        canvas.drawPath(diamond, paint);
        paint.setStyle(Paint.Style.FILL);
    }

    @Override protected void onDraw(Canvas canvas) {
        super.onDraw(canvas);
        canvas.drawColor(Color.rgb(22, 32, 44));
        if (state == null) return;
        for (int y = 0; y < state.height; y++) {
            for (int x = 0; x < state.width; x++) tile(canvas, x, y, Color.rgb(42, 52, 64), true);
        }
        for (GameState.Tile visible : state.tiles) {
            tile(canvas, visible.x, visible.y, terrainColor(visible.terrain), false);
            Bitmap texture = assets == null ? null : assets.terrain.get(visible.terrain);
            if (texture != null) {
                float tx = screenX(visible.x, visible.y), ty = screenY(visible.x, visible.y);
                destination.set(tx - halfWidth, ty - halfHeight, tx + halfWidth, ty + halfHeight);
                canvas.save(); canvas.clipPath(diamond);
                paint.setColor(Color.WHITE);
                canvas.drawBitmap(texture, null, destination, paint);
                canvas.restore();
            }
            if (visible.road > 0 || !visible.improvement.isEmpty()) {
                paint.setColor(0xffffdd6b); paint.setTextAlign(Paint.Align.CENTER); paint.setTextSize(halfHeight * .45f);
                canvas.drawText((visible.road > 0 ? "R " : "") + visible.improvement,
                        screenX(visible.x, visible.y), screenY(visible.x, visible.y) + halfHeight * .7f, paint);
            }
            if (!"Visible".equals(visible.visibility)) tile(canvas, visible.x, visible.y, 0x77000000, false);
            tile(canvas, visible.x, visible.y, 0xff263b35, true);
            if (!visible.resource.isEmpty()) {
                paint.setColor(0xffffe8a3); paint.setTextAlign(Paint.Align.CENTER);
                paint.setTextSize(halfHeight * .36f);
                canvas.drawText(visible.resource, screenX(visible.x, visible.y),
                        screenY(visible.x, visible.y) - halfHeight * .3f, paint);
            }
            if (!"None".equals(visible.vegetation)) {
                paint.setColor(0xff183e27);
                paint.setTextAlign(Paint.Align.CENTER);
                paint.setTextSize(halfHeight * .5f);
                canvas.drawText("Forest".equals(visible.vegetation) ? "F" : "J", screenX(visible.x, visible.y), screenY(visible.x, visible.y) + halfHeight * .18f, paint);
            }
        }
        if (selected) {
            for (int[] destination : state.destinations) {
                tile(canvas, destination[0], destination[1], 0xffffdd6b, true);
                paint.setColor(0xffffdd6b);
                canvas.drawCircle(screenX(destination[0], destination[1]), screenY(destination[0], destination[1]), halfHeight * .13f, paint);
            }
        }
        try {
            for (org.json.JSONObject city : state.displayedCities) {
                org.json.JSONObject pos = city.getJSONObject("position");
                float x = screenX(pos.getInt("x"), pos.getInt("y")), y = screenY(pos.getInt("x"), pos.getInt("y"));
                paint.setColor(city.getInt("owner") == 0 ? 0xffeddb9a : 0xffff7777);
                canvas.drawRect(x - halfHeight * .55f, y - halfHeight, x + halfHeight * .55f, y, paint);
                paint.setTextSize(halfHeight * .4f); paint.setTextAlign(Paint.Align.CENTER);
                canvas.drawText(city.getString("name") + " (" + city.getInt("population") + ")", x, y - halfHeight * 1.1f, paint);
            }
            for (int i = 0; i < state.units.length(); i++) {
                org.json.JSONObject unit = state.units.getJSONObject(i);
                if (unit.getJSONObject("id").getInt("index") == state.index) continue;
                org.json.JSONObject pos = unit.getJSONObject("position");
                marker(canvas, screenX(pos.getInt("x"), pos.getInt("y")), screenY(pos.getInt("x"), pos.getInt("y")),
                        unit.getString("unit_type_name"), unit.getInt("owner") == 0 ? Color.WHITE : 0xffff7777);
            }
        } catch (org.json.JSONException failure) { throw new IllegalStateException(failure); }
        if (state.index < 0) return;
        float cx = screenX(state.x, state.y), cy = screenY(state.x, state.y);
        if (assets != null && "settler".equals(state.name)) {
            long elapsed = SystemClock.uptimeMillis() - moveStarted;
            boolean moving = elapsed < (long) assets.runFrames * assets.runDelay;
            Bitmap sprite = moving ? assets.run : assets.idle;
            int frames = moving ? assets.runFrames : assets.idleFrames;
            int delay = moving ? assets.runDelay : assets.idleDelay;
            int frameHeight = sprite.getHeight() / frames;
            int frameWidth = sprite.getWidth() / 8;
            int frame = (int) ((moving ? elapsed : SystemClock.uptimeMillis()) / delay % frames);
            source.set(state.facingColumn * frameWidth, frame * frameHeight, (state.facingColumn + 1) * frameWidth, (frame + 1) * frameHeight);
            if (moving) {
                float remaining = 1 - (float) elapsed / (assets.runFrames * assets.runDelay);
                cx -= (moveDx - moveDy) * halfWidth * remaining;
                cy -= (moveDx + moveDy) * halfHeight * remaining;
            }
            float scale = halfWidth / 32;
            if (selected) {
                paint.setColor(0xffffdd6b); paint.setStyle(Paint.Style.STROKE);
                canvas.drawOval(cx - halfHeight * .65f, cy - halfHeight * .2f,
                        cx + halfHeight * .65f, cy + halfHeight * .3f, paint);
                paint.setStyle(Paint.Style.FILL);
            }
            destination.set(cx - frameWidth * scale / 2, cy - frameHeight * scale + halfHeight * .25f,
                    cx + frameWidth * scale / 2, cy + halfHeight * .25f);
            paint.setColor(Color.WHITE);
            canvas.drawBitmap(sprite, source, destination, paint);
            if (getWindowVisibility() == VISIBLE) postInvalidateDelayed(delay);
        } else {
            marker(canvas, cx, cy, state.name, selected ? 0xffffdd6b : Color.WHITE);
        }
    }

    private void marker(Canvas canvas, float x, float y, String name, int color) {
        paint.setColor(color); canvas.drawCircle(x, y, halfHeight * .65f, paint);
        paint.setColor(0xff24364a); paint.setTextSize(halfHeight * .5f); paint.setTextAlign(Paint.Align.CENTER);
        String label = "warrior".equals(name) ? "War" : "worker".equals(name) ? "Wkr" : name.substring(0, Math.min(3, name.length()));
        canvas.drawText(label, x, y + halfHeight * .2f, paint);
    }

    @Override public boolean onTouchEvent(MotionEvent event) {
        if (!isEnabled() || state == null) return false;
        if (event.getActionMasked() == MotionEvent.ACTION_DOWN) multiTouch = false;
        if (event.getPointerCount() > 1) multiTouch = true;
        tapped = false;
        scaling.onTouchEvent(event);
        gestures.onTouchEvent(event);
        if (tapped) { performClick(); dispatchTile(event, onTap); }
        return true;
    }

    private void dispatchTile(MotionEvent event, TileTap callback) {
        if (!isEnabled() || state == null || callback == null) return;
        float dx = (event.getX() - getWidth() / 2f) / halfWidth;
        float dy = (event.getY() - getHeight() / 2f) / halfHeight;
        int x = Math.round(camera.x + (dx + dy) / 2f);
        int y = Math.round(camera.y + (dy - dx) / 2f);
        if (x >= 0 && y >= 0 && x < state.width && y < state.height) callback.tap(x, y);
    }
    @Override public boolean performClick() { super.performClick(); return true; }

    private static int terrainColor(String terrain) {
        switch (terrain) {
            case "Grassland": return 0xff6a9850;
            case "Plains": return 0xffb3ad68;
            case "Desert": return 0xffd5bc81;
            case "Coast": return 0xff579da9;
            case "Ocean": return 0xff366889;
            case "Hill": return 0xff92885d;
            case "Mountain": return 0xff929591;
            case "Tundra": return 0xffabb7a0;
            case "Ice": return 0xffd9e7e9;
            default: throw new IllegalArgumentException("Unknown terrain: " + terrain);
        }
    }
}
