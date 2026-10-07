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
    private final GameSaves saves;
    private final ExecutorService worker = Executors.newSingleThreadExecutor();
    private final Handler main = new Handler(Looper.getMainLooper());
    Listener listener;
    GameState state;
    boolean busy, selected;
    String error = "";
    private boolean closed;
    private long debugSequence;
    ImportedAssets assets;
    String progress = "";
    boolean audioEnabled, foreground;
    final boolean prototype;
    private final GameAudio audio = new GameAudio(message -> { error = message; notifyChanged(); });

    GameController(Context context, boolean prototype) {
        this.context = context.getApplicationContext();
        this.prototype = prototype;
        this.saves = new GameSaves(context, prototype);
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
        if (current != null && current.index >= 0) execute(() -> CoreBridge.moveUnit(current.index, current.generation, x, y), false);
    }
    void endTurn() { execute(CoreBridge::endTurn, false); }

    void command(String request) { execute(() -> CoreBridge.command(request), false); }
    void selectUnit(JSONObject unit) {
        if (state == null || busy) return;
        try {
            JSONObject id = unit.getJSONObject("id");
            state = new GameState(state.json, id.getInt("index"), id.getInt("generation"));
            selected = true; notifyChanged();
        } catch (org.json.JSONException failure) { error = failure.getMessage(); notifyChanged(); }
    }
    void saveGame() {
        execute(() -> { saves.write(false, CoreBridge.save()); return CoreBridge.snapshot(); }, false, "Game saved.");
    }
    void loadGame(boolean recovery) {
        if (assets == null && !prototype) return;
        execute(() -> {
            File rules = new File(context.getFilesDir(), "base");
            copyRules(rules);
            return CoreBridge.load(rules.getPath(), saves.read(recovery));
        }, true, "Saved game loaded.");
    }
    private void execute(NativeAction action, boolean resetSelection) { execute(action, resetSelection, ""); }
    private void execute(NativeAction action, boolean resetSelection, String successMessage) {
        if (busy || closed) return;
        busy = true;
        progress = context.getString(R.string.working);
        error = "";
        notifyChanged();
        int selectedIndex = resetSelection || state == null ? -1 : state.index;
        int selectedGeneration = resetSelection || state == null ? -1 : state.generation;
        worker.execute(() -> {
            try {
                String json = action.run();
                GameState next = new GameState(json, selectedIndex, selectedGeneration);
                String saveFailure = "";
                try { saves.write(true, CoreBridge.save()); }
                catch (IOException | RuntimeException failure) { saveFailure = "Recovery save failed: " + failure.getMessage() + ". Keep the app open and retry Save game."; }
                String saveFeedback = saveFailure;
                if (BuildConfig.DEBUG) try {
                    android.util.AtomicFile debug = new android.util.AtomicFile(new File(context.getFilesDir(), "debug-state.json"));
                    FileOutputStream stream = debug.startWrite();
                    try { stream.write(new JSONObject(json).put("_sequence", ++debugSequence).toString().getBytes(java.nio.charset.StandardCharsets.UTF_8)); debug.finishWrite(stream); }
                    catch (IOException failure) { debug.failWrite(stream); throw failure; }
                    // Bounded acceptance evidence: full terrain can exceed logcat's line limit.
                    JSONObject evidence = new JSONObject(json);
                    JSONObject view = evidence.getJSONObject("view");
                    evidence.put("settler_atlas_column", next.facingColumn);
                    evidence.remove("available");
                    evidence.put("view", new JSONObject()
                            .put("turn", view.getInt("turn"))
                            .put("known_units", view.getJSONArray("known_units"))
                            .put("visible_tile_count", view.getJSONArray("visible_tiles").length()));
                    if (evidence.toString().length() < 3900) Log.i("Civ3Touch", "GAME_STATE " + evidence);
                    else Log.i("Civ3Touch", "GAME_STATE_FILE sequence=" + debugSequence);
                } catch (IOException | org.json.JSONException failure) {
                    Log.e("Civ3Touch", "Debug evidence unavailable: " + failure.getClass().getSimpleName());
                }
                main.post(() -> {
                    if (closed) return;
                    boolean moved = next.unitMoved;
                    state = next;
                    error = saveFeedback.isEmpty() ? successMessage : saveFeedback;
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
                    error = "Could not complete action. " + failure.getMessage() + ". Existing saves are retained.";
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
