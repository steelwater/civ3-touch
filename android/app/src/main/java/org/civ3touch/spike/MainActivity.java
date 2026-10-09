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

    @Override public void onCreate(Bundle savedState) {
        super.onCreate(savedState);
        controller = (GameController) getLastNonConfigurationInstance();
        if (controller == null) controller = new GameController(this, BuildConfig.DEBUG && (getIntent().getBooleanExtra("synthetic", false) || getIntent().getBooleanExtra("smokeTest", false)));
        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
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
        newGame.setOnClickListener(view -> controller.newGame(BuildConfig.DEBUG && getIntent().getBooleanExtra("missingRules", false)));
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
        int padding = (int) (12 * getResources().getDisplayMetrics().density);
        status.setPadding(padding, 0, padding, padding / 2);
        status.setAccessibilityLiveRegion(View.ACCESSIBILITY_LIVE_REGION_POLITE);
        information = TouchUi.button(this, "", () -> {
            controller.informationExpanded = !controller.informationExpanded; render();
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
            controller.unitExpanded = !controller.selected || !controller.unitExpanded;
            controller.selected = true; render();
        });
        sheetHeader.addView(unitHeader, new LinearLayout.LayoutParams(0, -2, 2));
        sheetHeader.addView(TouchUi.button(this, "Center", () -> map.centerSelection()), new LinearLayout.LayoutParams(0, -2, 1));
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

    private void render() {
        GameState state = controller.state;
        boolean compact = getResources().getConfiguration().screenHeightDp < 480;
        information.setText(state == null ? "Game information" : getString(R.string.information_title,
                state.turn, "Information", controller.informationExpanded ? "▴" : "▾"));
        boolean showStatus = controller.informationExpanded || controller.busy || !controller.error.isEmpty()
                || state == null;
        statusScroll.setVisibility(showStatus ? View.VISIBLE : View.GONE);
        statusScroll.getLayoutParams().height = TouchUi.dp(this, compact ? 56 : 88);
        importControls.setVisibility(state == null ? View.VISIBLE : View.GONE);
        unitHeader.setText(state != null && state.index >= 0 ? getString(R.string.unit_sheet_title,
                state.name, controller.selected && controller.unitExpanded ? "Hide" : "Show") : "No selected unit");
        unitHeader.setEnabled(state != null && state.index >= 0 && !controller.busy);
        unitContent.removeAllViews();
        boolean showUnit = state != null && state.index >= 0 && controller.selected && controller.unitExpanded;
        unitScroll.setVisibility(showUnit ? View.VISIBLE : View.GONE);
        unitScroll.getLayoutParams().height = TouchUi.dp(this, compact ? 56 : 112);
        if (showUnit) new GameMenus(this, controller).unitSheet(unitContent);
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
        }
    }

    void gameSettings() {
        java.util.List<String> labels = new java.util.ArrayList<>();
        java.util.List<Runnable> choices = new java.util.ArrayList<>();
        labels.add(getString(R.string.import_data)); choices.add(() -> importData.performClick());
        if (controller.assets != null) {
            labels.add(getString(controller.audioEnabled ? R.string.audio_on : R.string.audio_off));
            choices.add(controller::toggleAudio);
        }
        TouchUi.screen(this, "Game settings", "Import replaces the app’s active artwork after validation. Your source files and saved games are retained.", labels, choices);
    }

    void showScreen(android.app.Dialog dialog) {
        if (activeScreen != null) activeScreen.dismiss();
        activeScreen = dialog;
        dialog.setOnDismissListener(ignored -> { if (activeScreen == dialog) activeScreen = null; });
        dialog.show();
    }

    @Override protected void onActivityResult(int request, int result, Intent data) {
        super.onActivityResult(request, result, data);
        if (request == 2 && result == RESULT_OK && data != null && data.getData() != null) controller.loadImport(data.getData());
    }
    @Override protected void onResume() { super.onResume(); controller.setForeground(true); }
    @Override protected void onPause() { controller.setForeground(false); super.onPause(); }

    @Override public Object onRetainNonConfigurationInstance() { return controller; }
    @Override protected void onDestroy() {
        if (activeScreen != null) activeScreen.dismiss();
        controller.listener = null;
        if (!isChangingConfigurations()) controller.close();
        super.onDestroy();
    }
}
