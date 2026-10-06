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
    private Button newGame, endTurn, importData, audio, actions;
    private MapView map;

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
        LinearLayout importControls = new LinearLayout(this);
        importData = new Button(this);
        importData.setText(R.string.import_data);
        importData.setOnClickListener(view -> {
            Intent picker = new Intent(Intent.ACTION_OPEN_DOCUMENT_TREE);
            picker.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
            try { startActivityForResult(picker, 2); }
            catch (android.content.ActivityNotFoundException failure) {
                controller.error = "No system folder picker is available on this device."; render();
            }
        });
        audio = new Button(this);
        audio.setOnClickListener(view -> controller.toggleAudio());
        importControls.addView(importData, new LinearLayout.LayoutParams(0, -2, 3));
        importControls.addView(audio, new LinearLayout.LayoutParams(0, -2, 1));
        root.addView(importControls);
        LinearLayout controls = new LinearLayout(this);
        newGame = new Button(this);
        newGame.setText(R.string.new_game);
        newGame.setOnClickListener(view -> controller.newGame(BuildConfig.DEBUG && getIntent().getBooleanExtra("missingRules", false)));
        endTurn = new Button(this);
        endTurn.setText(R.string.end_turn);
        endTurn.setOnClickListener(view -> controller.endTurn());
        controls.addView(newGame, new LinearLayout.LayoutParams(0, -2, 1));
        controls.addView(endTurn, new LinearLayout.LayoutParams(0, -2, 1));
        actions = new Button(this);
        actions.setText(R.string.actions);
        actions.setOnClickListener(view -> new GameMenus(this, controller).main());
        controls.addView(actions, new LinearLayout.LayoutParams(0, -2, 1));
        root.addView(controls);
        status = new TextView(this);
        status.setTextSize(16);
        int padding = (int) (12 * getResources().getDisplayMetrics().density);
        status.setPadding(padding, 0, padding, padding / 2);
        status.setAccessibilityLiveRegion(View.ACCESSIBILITY_LIVE_REGION_POLITE);
        ScrollView statusScroll = new ScrollView(this);
        statusScroll.addView(status);
        // Bound the status area so landscape and large text retain a usable map.
        root.addView(statusScroll, new LinearLayout.LayoutParams(-1, (int) (88 * getResources().getDisplayMetrics().density)));
        map = new MapView(this);
        map.setOnTileTap((x, y) -> {
            GameState state = controller.state;
            if (state == null || controller.busy) return;
            if (state.index >= 0 && x == state.x && y == state.y) {
                controller.selected = true;
                controller.notifyChanged();
            } else {
                if (controller.selected) {
                    for (int[] destination : state.destinations) {
                        if (destination[0] == x && destination[1] == y) { controller.move(x, y); return; }
                    }
                }
                try {
                    for (org.json.JSONObject unit : state.ownUnits) {
                        org.json.JSONObject position = unit.getJSONObject("position");
                        if (position.getInt("x") == x && position.getInt("y") == y) { controller.selectUnit(unit); return; }
                    }
                } catch (org.json.JSONException failure) { controller.error = failure.getMessage(); }
                if (controller.selected) controller.move(x, y);
            }
        });
        root.addView(map, new LinearLayout.LayoutParams(-1, 0, 1));
        setContentView(root);
        controller.listener = this::render;
        render();
        if (BuildConfig.DEBUG && getIntent().getBooleanExtra("smokeTest", false) && savedState == null) {
            controller.smoke(getIntent().getBooleanExtra("missingRules", false));
        }
    }

    private void render() {
        GameState state = controller.state;
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
        else if (!controller.error.isEmpty()) status.setText(controller.error);
        else if (state == null) status.setText(controller.assets == null ? getString(R.string.welcome)
                : getString(R.string.import_ready));
        else {
            String instruction = controller.selected
                    ? getString(R.string.selected, state.x, state.y, state.movement / 3.0)
                        + " " + getString(state.destinations.isEmpty() ? R.string.no_step : R.string.choose_tile)
                    : getString(R.string.select_unit);
            if (state.index < 0) instruction = "Use Actions for cities, research or saves.";
            String message = state.root.isNull("game_over") ? state.message : getString(R.string.game_over, state.root.optInt("game_over"));
            status.setText(getString(R.string.game_status, state.turn, instruction, message));
        }
    }

    @Override protected void onActivityResult(int request, int result, Intent data) {
        super.onActivityResult(request, result, data);
        if (request == 2 && result == RESULT_OK && data != null && data.getData() != null) controller.loadImport(data.getData());
    }
    @Override protected void onResume() { super.onResume(); controller.setForeground(true); }
    @Override protected void onPause() { controller.setForeground(false); super.onPause(); }

    @Override public Object onRetainNonConfigurationInstance() { return controller; }
    @Override protected void onDestroy() {
        controller.listener = null;
        if (!isChangingConfigurations()) controller.close();
        super.onDestroy();
    }
}
