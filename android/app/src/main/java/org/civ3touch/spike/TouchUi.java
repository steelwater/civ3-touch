package org.civ3touch.spike;

import android.app.Dialog;
import android.content.Context;
import android.graphics.Color;
import android.view.Window;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;
import java.util.List;

/** Shared scrollable, large-target presentation for commands and information. */
final class TouchUi {
    static int background(Context context) { return QolSettings.dark(context) ? 0xff121212 : Color.WHITE; }
    static int foreground(Context context) { return QolSettings.dark(context) ? Color.WHITE : 0xff111111; }
    static void readable(TextView text) {
        boolean dark = QolSettings.dark(text.getContext());
        text.setTextColor(new android.content.res.ColorStateList(new int[][]{new int[]{-android.R.attr.state_enabled}, new int[]{}},
                new int[]{dark ? 0xffbbbbbb : 0xff555555, foreground(text.getContext())}));
    }
    static int dp(Context context, int value) { return Math.round(value * context.getResources().getDisplayMetrics().density); }
    static Button button(Context context) {
        Button button = new Button(context);
        button.setTextSize(16);
        readable(button);
        button.setAllCaps(false);
        button.setMinHeight(dp(context, 48));
        return button;
    }
    static Button button(Context context, String label, Runnable action) {
        Button button = button(context);
        button.setText(label);
        button.setOnClickListener(view -> action.run());
        return button;
    }
    static TextView text(Context context, String value, int size) {
        TextView text = new TextView(context);
        text.setText(value); text.setTextSize(size); readable(text);
        int padding = dp(context, 12);
        text.setPadding(padding, padding, padding, padding);
        return text;
    }
    static void screen(MainActivity activity, String title, String details, List<String> labels, List<Runnable> actions) {
        Dialog dialog = new Dialog(activity);
        dialog.requestWindowFeature(Window.FEATURE_NO_TITLE);
        LinearLayout root = new LinearLayout(activity);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setBackgroundColor(background(activity));
        root.setOnApplyWindowInsetsListener((view, insets) -> {
            view.setPadding(insets.getSystemWindowInsetLeft(), insets.getSystemWindowInsetTop(),
                    insets.getSystemWindowInsetRight(), insets.getSystemWindowInsetBottom());
            return insets;
        });
        root.addView(button(activity, "Return to map", dialog::dismiss));
        ScrollView scroll = new ScrollView(activity);
        LinearLayout content = new LinearLayout(activity);
        content.setOrientation(LinearLayout.VERTICAL);
        content.addView(text(activity, title, 24));
        if (!details.isEmpty()) content.addView(text(activity, details, 18));
        for (int i = 0; i < labels.size(); i++) {
            Runnable action = actions.get(i);
            content.addView(button(activity, labels.get(i), () -> { dialog.dismiss(); action.run(); }));
        }
        scroll.addView(content);
        root.addView(scroll, new LinearLayout.LayoutParams(-1, 0, 1));
        dialog.setContentView(root);
        activity.showScreen(dialog);
        dialog.getWindow().setLayout(-1, -1);
    }
}
