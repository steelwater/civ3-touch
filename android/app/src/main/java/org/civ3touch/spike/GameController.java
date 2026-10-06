package org.civ3touch.spike;

import android.content.Context;
import android.net.Uri;
import android.os.Handler;
import android.os.Looper;
import android.util.Log;
import org.json.JSONObject;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/** Retained across configuration changes; native state stays on exactly one worker. */
final class GameController {
    interface Listener { void changed(); }
    interface NativeAction { String run() throws IOException; }
    private final Context context;
    private final ExecutorService worker = Executors.newSingleThreadExecutor();
    private final Handler main = new Handler(Looper.getMainLooper());
    Listener listener;
    GameState state;
    boolean busy, selected;
    String error = "";
    private boolean closed;
    ImportedAssets assets;
    String progress = "";
    boolean audioEnabled, foreground;
    final boolean prototype;
    private final GameAudio audio = new GameAudio(message -> { error = message; notifyChanged(); });

    GameController(Context context, boolean prototype) {
        this.context = context.getApplicationContext();
        this.prototype = prototype;
        if (!prototype) loadImport(null);
    }

    void loadImport(Uri source) {
        if (busy || closed) return;
        busy = true; error = ""; progress = source == null ? "Checking imported data…" : "Checking source folder…";
        notifyChanged();
        worker.execute(() -> {
            try {
                AssetImporter importer = new AssetImporter(context);
                ImportedAssets loaded = source == null ? importer.restore() : importer.importFolder(source, message -> main.post(() -> {
                    if (!closed) { progress = message; notifyChanged(); }
                }));
                main.post(() -> {
                    if (closed) return;
                    assets = loaded; state = null; selected = false; busy = false;
                    if (loaded != null) { error = loaded.warnings; audio.load(loaded); }
                    notifyChanged();
                });
            } catch (Exception | LinkageError failure) {
                main.post(() -> {
                    if (closed) return;
                    busy = false;
                    error = "Import failed: " + failure.getMessage() + (assets == null ? "" : "\nPrevious import is still available.");
                    notifyChanged();
                });
            }
        });
    }
    void setForeground(boolean value) { foreground = value; updateAudio(); }
    void toggleAudio() { audioEnabled = !audioEnabled; updateAudio(); notifyChanged(); }
    private void updateAudio() { audio.setPlaying(foreground && audioEnabled && state != null); }

    void newGame(boolean missingRules) {
        if (assets == null && !prototype) return;
        execute(() -> {
            File rules = new File(context.getFilesDir(), "base");
            copyRules(rules);
            return CoreBridge.newGame(missingRules ? new File(context.getFilesDir(), "missing-rules").getPath() : rules.getPath());
        }, true);
    }
    void move(int x, int y) {
        GameState current = state;
        if (current != null) execute(() -> CoreBridge.moveUnit(current.index, current.generation, x, y), false);
    }
    void endTurn() { execute(CoreBridge::endTurn, false); }

    private void execute(NativeAction action, boolean resetSelection) {
        if (busy || closed) return;
        busy = true;
        progress = context.getString(R.string.working);
        error = "";
        notifyChanged();
        worker.execute(() -> {
            try {
                String json = action.run();
                GameState next = new GameState(json);
                if (BuildConfig.DEBUG) {
                    // Bounded acceptance evidence: full terrain can exceed logcat's line limit.
                    JSONObject evidence = new JSONObject(json);
                    JSONObject view = evidence.getJSONObject("view");
                    evidence.put("view", new JSONObject()
                            .put("turn", view.getInt("turn"))
                            .put("known_units", view.getJSONArray("known_units"))
                            .put("visible_tile_count", view.getJSONArray("visible_tiles").length()));
                    Log.i("Civ3Touch", "GAME_STATE " + evidence);
                }
                main.post(() -> {
                    if (closed) return;
                    boolean moved = next.unitMoved;
                    state = next;
                    updateAudio();
                    if (moved) audio.step();
                    if (resetSelection) selected = false;
                    busy = false;
                    notifyChanged();
                });
            } catch (Exception | LinkageError failure) {
                Log.e("Civ3Touch", "GAME_FAIL " + failure.getClass().getSimpleName());
                main.post(() -> {
                    if (closed) return;
                    error = "Could not complete action. Try New Game. " + failure.getMessage();
                    busy = false;
                    notifyChanged();
                });
            }
        });
    }

    void smoke(boolean missingRules) {
        busy = true;
        notifyChanged();
        worker.execute(() -> {
            String result;
            try {
                File rules = new File(context.getFilesDir(), "base");
                copyRules(rules);
                int turn = CoreBridge.smokeTest(missingRules ? new File(context.getFilesDir(), "missing-rules").getPath() : rules.getPath());
                if (turn != 2) throw new IllegalStateException("Unexpected smoke turn");
                result = "CORE_SMOKE_PASS turn=" + turn;
            } catch (Exception | LinkageError failure) {
                result = "CORE_SMOKE_FAIL " + failure.getClass().getSimpleName();
            }
            Log.i("Civ3Touch", result);
            String feedback = result;
            main.post(() -> {
                if (closed) return;
                error = feedback;
                busy = false;
                notifyChanged();
            });
        });
    }

    void notifyChanged() { if (listener != null) listener.changed(); }
    void close() {
        closed = true;
        audio.close();
        listener = null;
        worker.execute(() -> {
            try { CoreBridge.close(); } catch (LinkageError ignored) { /* Library failed to load. */ }
        });
        worker.shutdown();
    }

    private void copyRules(File destination) throws IOException {
        if (!destination.isDirectory() && !destination.mkdirs()) throw new IOException("Cannot create rules directory");
        String[] files = context.getAssets().list("base");
        if (files == null || files.length == 0) throw new IOException("Packaged rules missing");
        for (String name : files) {
            try (InputStream input = context.getAssets().open("base/" + name);
                 FileOutputStream output = new FileOutputStream(new File(destination, name))) {
                byte[] buffer = new byte[8192];
                int count;
                while ((count = input.read(buffer)) != -1) output.write(buffer, 0, count);
            }
        }
    }
}
