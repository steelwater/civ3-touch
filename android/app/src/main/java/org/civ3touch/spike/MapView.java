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
import android.view.View;

/** Presentation only: decoded imported art over the native player view. */
final class MapView extends View {
    interface TileTap { void tap(int x, int y); }
    private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Path diamond = new Path();
    private final float halfWidth, halfHeight;
    private TileTap onTap;
    private GameState state;
    private boolean selected;
    private float downX, downY;
    private ImportedAssets assets;
    private long moveStarted;
    private int moveDx, moveDy;
    private final Rect source = new Rect();
    private final RectF destination = new RectF();

    public MapView(Context context) {
        super(context);
        float density = getResources().getDisplayMetrics().density;
        halfWidth = 48 * density;
        halfHeight = 28 * density;
        setClickable(true);
        setFocusable(true);
        setContentDescription("Generated map. Start a new game.");
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
        assets = imported;
        state = next;
        selected = isSelected;
        if (state == null) setContentDescription("Generated map. Start a new game.");
        else setContentDescription("Generated map. " + state.name + " at " + state.x + ", " + state.y
                + (selected ? ". Selected." : ". Tap a unit to select; Actions lists stacked units."));
        invalidate();
    }

    private float screenX(int x, int y) { return getWidth() / 2f + ((x - state.x) - (y - state.y)) * halfWidth; }
    private float screenY(int x, int y) { return getHeight() / 2f + ((x - state.x) + (y - state.y)) * halfHeight; }

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
        if (event.getAction() == MotionEvent.ACTION_DOWN) {
            downX = event.getX(); downY = event.getY();
            return true;
        }
        if (event.getAction() == MotionEvent.ACTION_UP) {
            if (Math.hypot(event.getX() - downX, event.getY() - downY) > halfHeight / 2) return true;
            performClick();
            float dx = (event.getX() - getWidth() / 2f) / halfWidth;
            float dy = (event.getY() - getHeight() / 2f) / halfHeight;
            int x = Math.round(state.x + (dx + dy) / 2f);
            int y = Math.round(state.y + (dy - dx) / 2f);
            if (x >= 0 && y >= 0 && x < state.width && y < state.height) onTap.tap(x, y);
            return true;
        }
        return true;
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
