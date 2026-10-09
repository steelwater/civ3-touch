package org.civ3touch.spike;

import android.app.Activity;
import android.content.Intent;
import android.os.Bundle;
import android.view.View;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;

public final class MainActivity extends Activity {
    private GameController controller;
    private TextView status;
    private Button newGame, endTurn, importData, audio, actions, information, unitHeader;
    private ScrollView statusScroll, unitScroll;
    private LinearLayout unitContent, importControls;
    private MapView map;
    private android.app.Dialog activeScreen;

    @Override protected void attachBaseContext(android.content.Context base) {
        android.content.res.Configuration configuration = new android.content.res.Configuration(base.getResources().getConfiguration());
        configuration.densityDpi = Math.round(configuration.densityDpi * QolSettings.ui(base) / 100f);
        configuration.fontScale *= QolSettings.font(base) / 100f;
        super.attachBaseContext(base.createConfigurationContext(configuration));
    }

    @Override public void onCreate(Bundle savedState) {
        setTheme(QolSettings.dark(this) ? R.style.AppThemeDark : R.style.AppTheme);
        super.onCreate(savedState);
        controller = (GameController) getLastNonConfigurationInstance();
        if (controller == null) controller = new GameController(this, BuildConfig.DEBUG && (getIntent().getBooleanExtra("synthetic", false) || getIntent().getBooleanExtra("smokeTest", false)));
        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setBackgroundColor(TouchUi.background(this));
        root.setOnApplyWindowInsetsListener((view, insets) -> {
            view.setPadding(insets.getSystemWindowInsetLeft(), insets.getSystemWindowInsetTop(),
                    insets.getSystemWindowInsetRight(), insets.getSystemWindowInsetBottom());
            return insets;
        });
        importControls = new LinearLayout(this);
        importData = TouchUi.button(this);
        importData.setText(R.string.import_data);
        importData.setOnClickListener(view -> {
            Intent picker = new Intent(Intent.ACTION_OPEN_DOCUMENT_TREE);
            picker.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
            try { startActivityForResult(picker, 2); }
            catch (android.content.ActivityNotFoundException failure) {
                controller.error = "No system folder picker is available on this device."; render();
            }
        });
        audio = TouchUi.button(this);
        audio.setOnClickListener(view -> controller.toggleAudio());
        importControls.addView(importData, new LinearLayout.LayoutParams(0, -2, 3));
        importControls.addView(audio, new LinearLayout.LayoutParams(0, -2, 1));
        root.addView(importControls);
        LinearLayout controls = new LinearLayout(this);
        newGame = TouchUi.button(this);
        newGame.setText(R.string.new_game);
        newGame.setOnClickListener(view -> startNewGame());
        endTurn = TouchUi.button(this);
        endTurn.setText(R.string.end_turn);
        endTurn.setOnClickListener(view -> controller.endTurn());
        controls.addView(newGame, new LinearLayout.LayoutParams(0, -2, 1));
        controls.addView(endTurn, new LinearLayout.LayoutParams(0, -2, 1));
        actions = TouchUi.button(this);
        actions.setText(R.string.actions);
        actions.setOnClickListener(view -> new GameMenus(this, controller).main());
        controls.addView(actions, new LinearLayout.LayoutParams(0, -2, 1));
        root.addView(controls);
        status = new TextView(this);
        status.setTextSize(16);
        TouchUi.readable(status);
        int padding = (int) (12 * getResources().getDisplayMetrics().density);
        status.setPadding(padding, 0, padding, padding / 2);
        status.setAccessibilityLiveRegion(View.ACCESSIBILITY_LIVE_REGION_POLITE);
        information = TouchUi.button(this, "", () -> {
            if (constrainedHeight()) TouchUi.screen(this, "Game information", status.getText().toString(), java.util.Collections.emptyList(), java.util.Collections.emptyList());
            else { controller.informationExpanded = !controller.informationExpanded; render(); }
        });
        information.setMaxLines(2);
        information.setEllipsize(android.text.TextUtils.TruncateAt.END);
        root.addView(information);
        statusScroll = new ScrollView(this);
        statusScroll.addView(status);
        // Bound the status area so landscape and large text retain a usable map.
        root.addView(statusScroll, new LinearLayout.LayoutParams(-1, (int) (88 * getResources().getDisplayMetrics().density)));
        map = new MapView(this, controller.camera);
        map.setOnTileTap((x, y) -> {
            GameState state = controller.state;
            if (state == null || controller.busy) return;
            if (QolSettings.inspectTap(this)) { new GameMenus(this, controller).tile(x, y); return; }
            // A selected legal destination wins over a unit/city on that tile.
            // Long press always inspects without moving.
            if (state.index >= 0 && x == state.x && y == state.y) {
                controller.selected = true;
                controller.unitExpanded = true;
                controller.notifyChanged();
            } else {
                if (controller.selected) {
                    for (int[] destination : state.destinations) {
                        if (destination[0] == x && destination[1] == y) { controller.move(x, y); return; }
                    }
                }
                try {
                    for (int i = 0; i < state.cities.length(); i++) {
                        org.json.JSONObject city = state.cities.getJSONObject(i);
                        org.json.JSONObject position = city.getJSONObject("position");
                        if (position.getInt("x") == x && position.getInt("y") == y) {
                            new GameMenus(this, controller).city(city); return;
                        }
                    }
                    for (org.json.JSONObject unit : state.ownUnits) {
                        org.json.JSONObject position = unit.getJSONObject("position");
                        if (position.getInt("x") == x && position.getInt("y") == y) { controller.selectUnit(unit); return; }
                    }
                } catch (org.json.JSONException failure) { controller.error = failure.getMessage(); }
                if (controller.selected) controller.move(x, y);
                else new GameMenus(this, controller).tile(x, y);
            }
        });
        map.setOnTileLongPress((x, y) -> {
            if (!controller.busy) new GameMenus(this, controller).tile(x, y);
        });
        root.addView(map, new LinearLayout.LayoutParams(-1, 0, 1));
        LinearLayout sheetHeader = new LinearLayout(this);
        unitHeader = TouchUi.button(this, "", () -> {
            if (constrainedHeight()) { new GameMenus(this, controller).unitScreen(); return; }
            controller.unitExpanded = !controller.selected || !controller.unitExpanded;
            controller.selected = true; render();
        });
        sheetHeader.addView(unitHeader, new LinearLayout.LayoutParams(0, -2, 2));
        Button center = TouchUi.button(this, "Center", () -> map.centerSelection());
        sheetHeader.addView(center, new LinearLayout.LayoutParams(0, -2, 1));
        root.addView(sheetHeader);
        unitContent = new LinearLayout(this);
        unitContent.setOrientation(LinearLayout.VERTICAL);
        unitScroll = new ScrollView(this);
        unitScroll.addView(unitContent);
        root.addView(unitScroll, new LinearLayout.LayoutParams(-1, TouchUi.dp(this, 112)));
        setContentView(root);
        controller.listener = this::render;
        render();
        if (BuildConfig.DEBUG && getIntent().getBooleanExtra("smokeTest", false) && savedState == null) {
            controller.smoke(getIntent().getBooleanExtra("missingRules", false));
        }
    }

    void startNewGame() {
        controller.newGame(BuildConfig.DEBUG && getIntent().getBooleanExtra("missingRules", false));
    }

    boolean largeTextPhone() {
        android.content.res.Configuration config = getResources().getConfiguration();
        return config.screenWidthDp * 100 / QolSettings.ui(this) < 500 && config.fontScale > 1.75f;
    }

    private boolean constrainedHeight() {
        return largeTextPhone() || getResources().getConfiguration().screenHeightDp * 100 / QolSettings.ui(this) < 360;
    }

    private void render() {
        GameState state = controller.state;
        boolean compact = getResources().getConfiguration().screenHeightDp < 480;
        information.setMaxLines(constrainedHeight() ? 1 : 2);
        unitHeader.setMaxLines(constrainedHeight() ? 1 : Integer.MAX_VALUE);
        unitHeader.setEllipsize(android.text.TextUtils.TruncateAt.END);
        information.setText(state == null ? "Game information" : getString(R.string.information_title,
                state.turn, "Information", controller.informationExpanded ? "▴" : "▾"));
        boolean showStatus = controller.informationExpanded || controller.busy || !controller.error.isEmpty()
                || state == null;
        statusScroll.setVisibility(showStatus && !constrainedHeight() ? View.VISIBLE : View.GONE);
        statusScroll.getLayoutParams().height = TouchUi.dp(this, compact ? 56 : 88);
        importControls.setVisibility(state == null ? View.VISIBLE : View.GONE);
        unitHeader.setText(state != null && state.index >= 0 ? getString(R.string.unit_sheet_title,
                state.name, !constrainedHeight() && controller.selected && controller.unitExpanded ? "Hide" : "Show") : "No selected unit");
        unitHeader.setContentDescription(unitHeader.getText());
        if (largeTextPhone() && state != null && state.index >= 0)
            unitHeader.setText(getString(R.string.unit_compact_title, state.name));
        ((LinearLayout.LayoutParams) unitHeader.getLayoutParams()).weight = largeTextPhone() ? 1 : 2;
        unitHeader.setEnabled(state != null && state.index >= 0 && !controller.busy);
        unitContent.removeAllViews();
        boolean showUnit = state != null && state.index >= 0 && controller.selected && controller.unitExpanded;
        unitScroll.setVisibility(showUnit && !constrainedHeight() ? View.VISIBLE : View.GONE);
        unitScroll.getLayoutParams().height = TouchUi.dp(this, compact ? 56 : 112);
        if (showUnit) new GameMenus(this, controller).unitSheet(unitContent);
        newGame.setVisibility(largeTextPhone() && state != null ? View.GONE : View.VISIBLE);
        endTurn.setVisibility(largeTextPhone() && state == null ? View.GONE : View.VISIBLE);
        newGame.setText(state == null && !controller.prototype ? R.string.play : R.string.new_game);
        newGame.setEnabled(!controller.busy && (controller.assets != null || controller.prototype));
        importData.setEnabled(!controller.busy);
        audio.setEnabled(controller.assets != null && !controller.busy);
        audio.setText(controller.audioEnabled ? R.string.audio_on : R.string.audio_off);
        endTurn.setEnabled(state != null && !controller.busy);
        actions.setEnabled(!controller.busy);
        map.setEnabled(!controller.busy);
        map.display(state, controller.selected, controller.assets);
        if (controller.busy) status.setText(controller.progress);
        else if (state == null) status.setText(!controller.error.isEmpty() ? controller.error
                : controller.assets == null ? getString(R.string.welcome) : getString(R.string.import_ready));
        else {
            String message = state.root.isNull("game_over") ? state.message : getString(R.string.game_over, state.root.optInt("game_over"));
            if (!message.isEmpty() && !controller.informationExpanded)
                information.setText(getString(R.string.information_title, state.turn, message, "▾"));
            String instruction = controller.selected
                    ? getString(R.string.selected, state.x, state.y, state.movement / 3.0)
                        + " " + getString(state.destinations.isEmpty() ? R.string.no_step : R.string.choose_tile)
                    : getString(R.string.select_unit);
            if (state.index < 0) instruction = "Use Actions for cities, research or saves.";
            String economy = getString(R.string.economy, state.gold, state.goldPerTurn,
                    state.science, state.sciencePerTurn, state.researchName);
            String details = getString(R.string.information_details, getString(R.string.game_status, state.turn, instruction, message), economy);
            status.setText(controller.error.isEmpty() ? details : getString(R.string.information_details, controller.error, details));
            if (constrainedHeight() && !controller.error.isEmpty()) information.setText(getString(R.string.information_title, state.turn, controller.error, "▾"));
        }
    }

    void gameSettings() {
        java.util.List<String> labels = new java.util.ArrayList<>();
        java.util.List<Runnable> choices = new java.util.ArrayList<>();
        labels.add("Display and readability"); choices.add(this::displaySettings);
        labels.add("Gestures and input help"); choices.add(this::inputSettings);
        labels.add("Autosave settings"); choices.add(this::autosaveSettings);
        labels.add(getString(R.string.import_data)); choices.add(() -> importData.performClick());
        if (controller.assets != null) {
            labels.add(getString(controller.audioEnabled ? R.string.audio_on : R.string.audio_off));
            choices.add(controller::toggleAudio);
        }
        TouchUi.screen(this, "Game settings", "Import replaces the app’s active artwork after validation. Your source files and saved games are retained.", labels, choices);
    }

    void displaySettings() {
        java.util.List<String> labels = new java.util.ArrayList<>();
        java.util.List<Runnable> choices = new java.util.ArrayList<>();
        for (int scale : new int[]{100, 115, 130}) {
            labels.add("UI scale " + scale + "%");
            choices.add(() -> { QolSettings.preferences(this).edit().putInt("ui", scale).apply(); recreate(); });
        }
        for (int scale : new int[]{100, 125, 150}) {
            labels.add("Font scale " + scale + "%");
            choices.add(() -> { QolSettings.preferences(this).edit().putInt("font", scale).apply(); recreate(); });
        }
        labels.add("High-contrast light"); choices.add(() -> { QolSettings.preferences(this).edit().putBoolean("dark", false).apply(); recreate(); });
        labels.add("High-contrast dark"); choices.add(() -> { QolSettings.preferences(this).edit().putBoolean("dark", true).apply(); recreate(); });
        labels.add("Reset display defaults"); choices.add(() -> {
            QolSettings.preferences(this).edit().remove("ui").remove("font").remove("dark").apply(); recreate();
        });
        TouchUi.screen(this, "Display and readability", "UI " + QolSettings.ui(this) + "%, font " + QolSettings.font(this)
                + "% • " + (QolSettings.dark(this) ? "Dark" : "Light")
                + ". Font scaling also respects your Android text setting. Changes retain the live game, selection and camera; open screens return to the map. At very large Android text sizes on phones, New Game moves into Actions and unit/information buttons open scrollable screens.", labels, choices);
    }

    void inputSettings() {
        java.util.List<String> labels = java.util.Arrays.asList("Tap: select or move (default)", "Tap: inspect only",
                QolSettings.drag(this) ? "Disable drag panning" : "Enable drag panning",
                QolSettings.pinch(this) ? "Disable pinch zoom" : "Enable pinch zoom", "Reset gesture defaults");
        java.util.List<Runnable> choices = java.util.Arrays.asList(
                () -> QolSettings.preferences(this).edit().putBoolean("inspectTap", false).apply(),
                () -> QolSettings.preferences(this).edit().putBoolean("inspectTap", true).apply(),
                () -> QolSettings.preferences(this).edit().putBoolean("drag", !QolSettings.drag(this)).apply(),
                () -> QolSettings.preferences(this).edit().putBoolean("pinch", !QolSettings.pinch(this)).apply(),
                () -> QolSettings.preferences(this).edit().remove("inspectTap").remove("drag").remove("pinch").apply());
        TouchUi.screen(this, "Gestures and input help", "Tap: " + (QolSettings.inspectTap(this) ? "inspect only" : "select/move")
                + ". Long press always inspects and lists stacked targets. Drag pans; pinch zooms. Neither issues a command. Disabled drag/pinch still consume their gestures so they cannot turn into moves. Center returns to the selected unit. Use Actions to select units and issue available unit commands in inspect-only mode. To move, restore select/move tap mode.\n\nKeyboard: Tab / arrows navigate buttons; Enter activates. With map focus, arrows pan, + / − zoom and C centers. Mouse wheel zooms the map. Touch remains available. Stylus contacts use the same hit testing as fingers; physical-device accuracy is untested. Controller support is not implemented. Keyboard/mouse bindings have emulator event coverage only.", labels, choices);
    }

    void autosaveSettings() {
        java.util.List<String> labels = new java.util.ArrayList<>();
        java.util.List<Runnable> choices = new java.util.ArrayList<>();
        for (int cadence : new int[]{0, 1, 3, 5}) {
            labels.add(cadence == 0 ? "Turn autosaves off" : "Autosave every " + cadence + " turn(s)");
            choices.add(() -> QolSettings.preferences(this).edit().putInt("cadence", cadence).apply());
        }
        for (int retention : new int[]{3, 5, 10}) {
            labels.add("Rotate " + retention + " autosave slots");
            choices.add(() -> QolSettings.preferences(this).edit().putInt("retention", retention).apply());
        }
        TouchUi.screen(this, "Autosave settings", "Current cadence: " + QolSettings.cadence(this)
                + " (0 means off); rotating slots: " + QolSettings.retention(this)
                + ". Recovery still records every completed action. Lowering retention leaves older extra slots available; total storage is bounded to 10 autosave slots. A failed write keeps the previous checkpoint.", labels, choices);
    }

    void saveDocument(boolean importing) {
        Intent picker = new Intent(importing ? Intent.ACTION_OPEN_DOCUMENT : Intent.ACTION_CREATE_DOCUMENT);
        picker.addCategory(Intent.CATEGORY_OPENABLE);
        picker.setType(importing ? "*/*" : "application/json");
        if (!importing) picker.putExtra(Intent.EXTRA_TITLE, "Civ3Touch-turn-" + controller.state.turn + ".json");
        try { startActivityForResult(picker, importing ? 3 : 4); }
        catch (android.content.ActivityNotFoundException failure) {
            controller.error = "No system document picker is available."; render();
        }
    }

    void showScreen(android.app.Dialog dialog) {
        if (activeScreen != null) activeScreen.dismiss();
        activeScreen = dialog;
        map.setAnimating(false);
        dialog.setOnDismissListener(ignored -> {
            if (activeScreen == dialog) activeScreen = null;
            map.setAnimating(controller.foreground && activeScreen == null);
        });
        dialog.show();
    }

    @Override protected void onActivityResult(int request, int result, Intent data) {
        super.onActivityResult(request, result, data);
        if (request == 3 || request == 4) {
            if (result != RESULT_OK || data == null || data.getData() == null) {
                controller.error = "Save document operation cancelled. Your game and private slots are unchanged.";
                render();
            } else if (controller.busy) {
                controller.error = "Game is still loading. Please retry the document operation when ready.";
                render();
            } else if (request == 3) controller.importSave(data.getData());
            else controller.exportSave(data.getData());
        }
        if (request == 2 && result == RESULT_OK && data != null && data.getData() != null) controller.loadImport(data.getData());
    }
    @Override protected void onResume() { super.onResume(); controller.setForeground(true); map.setAnimating(activeScreen == null); }
    @Override protected void onPause() { map.setAnimating(false); controller.setForeground(false); super.onPause(); }

    @Override public Object onRetainNonConfigurationInstance() { return controller; }
    @Override protected void onDestroy() {
        if (activeScreen != null) activeScreen.dismiss();
        controller.listener = null;
        if (!isChangingConfigurations()) controller.close();
        super.onDestroy();
    }
}
