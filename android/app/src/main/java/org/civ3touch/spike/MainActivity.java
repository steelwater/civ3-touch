package org.civ3touch.spike;

import android.app.Activity;
import android.os.Bundle;
import android.util.Log;
import android.widget.ScrollView;
import android.widget.TextView;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;

public final class MainActivity extends Activity {
    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        TextView result = new TextView(this);
        result.setTextSize(20);
        int padding = (int) (24 * getResources().getDisplayMetrics().density);
        result.setPadding(padding, padding, padding, padding);
        result.setText(R.string.running);
        ScrollView scroll = new ScrollView(this);
        scroll.addView(result);
        scroll.setOnApplyWindowInsetsListener((view, insets) -> {
            view.setPadding(insets.getSystemWindowInsetLeft(), insets.getSystemWindowInsetTop(),
                    insets.getSystemWindowInsetRight(), insets.getSystemWindowInsetBottom());
            return insets;
        });
        setContentView(scroll);
        new Thread(() -> {
            try {
                File rules = new File(getFilesDir(), "base");
                copyRules(rules);
                // Explicit failure path for the smoke harness, debug builds only.
                String path = BuildConfig.DEBUG && getIntent().getBooleanExtra("missingRules", false)
                        ? new File(getFilesDir(), "missing-rules").getPath() : rules.getPath();
                int turn = CoreBridge.smokeTest(path);
                if (turn != 2) throw new IllegalStateException("Unexpected native result: " + turn);
                Log.i("Civ3Touch", "CORE_SMOKE_PASS turn=" + turn);
                runOnUiThread(() -> result.setText(getString(R.string.success, turn)));
            } catch (IOException | RuntimeException | LinkageError error) {
                Log.e("Civ3Touch", "CORE_SMOKE_FAIL " + error.getClass().getSimpleName());
                runOnUiThread(() -> result.setText(getString(R.string.failure, error.getMessage())));
            }
        }, "core-smoke").start();
    }

    private void copyRules(File destination) throws IOException {
        if (!destination.isDirectory() && !destination.mkdirs()) throw new IOException("Cannot create rules directory");
        String[] files = getAssets().list("base");
        if (files == null || files.length == 0) throw new IOException("Packaged rules missing");
        for (String name : files) {
            try (InputStream input = getAssets().open("base/" + name);
                 FileOutputStream output = new FileOutputStream(new File(destination, name))) {
                byte[] buffer = new byte[8192];
                int count;
                while ((count = input.read(buffer)) != -1) output.write(buffer, 0, count);
            }
        }
    }
}
