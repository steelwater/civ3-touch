import android.os.SystemClock;
import android.view.InputEvent;
import android.view.InputDevice;
import android.view.MotionEvent;

/** Shell-only device test helper: inject real two-pointer touch events, never game state. */
public final class Pinch {
    public static void main(String[] args) throws Exception {
        float x = Float.parseFloat(args[0]), y = Float.parseFloat(args[1]);
        float start = Float.parseFloat(args[2]), end = Float.parseFloat(args[3]);
        Class<?> type = Class.forName("android.hardware.input.InputManagerGlobal");
        Object manager = type.getMethod("getInstance").invoke(null);
        java.lang.reflect.Method inject = type.getMethod("injectInputEvent", InputEvent.class, int.class);
        MotionEvent.PointerProperties[] properties = new MotionEvent.PointerProperties[2];
        MotionEvent.PointerCoords[] coords = new MotionEvent.PointerCoords[2];
        for (int i = 0; i < 2; i++) {
            properties[i] = new MotionEvent.PointerProperties(); properties[i].id = i;
            properties[i].toolType = MotionEvent.TOOL_TYPE_FINGER;
            coords[i] = new MotionEvent.PointerCoords(); coords[i].pressure = 1; coords[i].size = 1;
        }
        long down = SystemClock.uptimeMillis();
        for (int step = 0; step <= 24; step++) {
            float span = start + (end - start) * Math.max(0, Math.min(20, step - 2)) / 20;
            coords[0].x = x - span; coords[1].x = x + span;
            coords[0].y = y; coords[1].y = y;
            int action = step == 0 ? MotionEvent.ACTION_DOWN
                    : step == 1 ? MotionEvent.ACTION_POINTER_DOWN | (1 << MotionEvent.ACTION_POINTER_INDEX_SHIFT)
                    : step == 23 ? MotionEvent.ACTION_POINTER_UP | (1 << MotionEvent.ACTION_POINTER_INDEX_SHIFT)
                    : step == 24 ? MotionEvent.ACTION_UP : MotionEvent.ACTION_MOVE;
            int count = step == 0 || step == 24 ? 1 : 2;
            MotionEvent event = MotionEvent.obtain(down, SystemClock.uptimeMillis(), action, count,
                    properties, coords, 0, 0, 1, 1, 0, 0, InputDevice.SOURCE_TOUCHSCREEN, 0);
            if (!Boolean.TRUE.equals(inject.invoke(manager, event, 2))) throw new AssertionError("Touch injection failed");
            event.recycle(); SystemClock.sleep(20);
        }
    }
}
