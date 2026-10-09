package org.civ3touch.spike;

import android.content.Context;
import android.content.SharedPreferences;

/** Presentation and Android backup preferences, never simulation rules. */
final class QolSettings {
    static SharedPreferences preferences(Context context) { return context.getSharedPreferences("qol", Context.MODE_PRIVATE); }
    static int cadence(Context context) { return choice(context, "cadence", 1, new int[]{0, 1, 3, 5}); }
    static int retention(Context context) { return choice(context, "retention", 3, new int[]{3, 5, 10}); }
    static int ui(Context context) { return choice(context, "ui", 100, new int[]{100, 115, 130}); }
    static int font(Context context) { return choice(context, "font", 100, new int[]{100, 125, 150}); }
    static boolean dark(Context context) { return preferences(context).getBoolean("dark", false); }
    static boolean inspectTap(Context context) { return preferences(context).getBoolean("inspectTap", false); }
    static boolean drag(Context context) { return preferences(context).getBoolean("drag", true); }
    static boolean pinch(Context context) { return preferences(context).getBoolean("pinch", true); }
    private static int choice(Context context, String key, int fallback, int[] allowed) {
        int value = preferences(context).getInt(key, fallback);
        for (int candidate : allowed) if (candidate == value) return value;
        return fallback;
    }
}
