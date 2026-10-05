package org.civ3touch.spike;

import android.app.Activity;
import android.os.Bundle;
import android.view.View;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;

public final class MainActivity extends Activity {
    private GameController controller;
    private TextView status;
    private Button newGame, endTurn;
    private MapView map;

    @Override public void onCreate(Bundle savedState) {
        super.onCreate(savedState);
        controller = (GameController) getLastNonConfigurationInstance();
        if (controller == null) controller = new GameController(this);
        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setOnApplyWindowInsetsListener((view, insets) -> {
            view.setPadding(insets.getSystemWindowInsetLeft(), insets.getSystemWindowInsetTop(),
                    insets.getSystemWindowInsetRight(), insets.getSystemWindowInsetBottom());
            return insets;
        });
        LinearLayout controls = new LinearLayout(this);
        newGame = new Button(this);
        newGame.setText(R.string.new_game);
        newGame.setOnClickListener(view -> controller.newGame(BuildConfig.DEBUG && getIntent().getBooleanExtra("missingRules", false)));
        endTurn = new Button(this);
        endTurn.setText(R.string.end_turn);
        endTurn.setOnClickListener(view -> controller.endTurn());
        controls.addView(newGame, new LinearLayout.LayoutParams(0, -2, 1));
        controls.addView(endTurn, new LinearLayout.LayoutParams(0, -2, 1));
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
            if (x == state.x && y == state.y) {
                controller.selected = true;
                controller.notifyChanged();
            } else if (controller.selected) controller.move(x, y);
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
        newGame.setEnabled(!controller.busy);
        endTurn.setEnabled(state != null && !controller.busy);
        map.setEnabled(!controller.busy);
        map.display(state, controller.selected);
        if (controller.busy) status.setText(R.string.working);
        else if (!controller.error.isEmpty()) status.setText(controller.error);
        else if (state == null) status.setText(R.string.welcome);
        else {
            String instruction = controller.selected
                    ? getString(R.string.selected, state.x, state.y, state.movement / 3.0)
                        + " " + getString(state.destinations.isEmpty() ? R.string.no_step : R.string.choose_tile)
                    : getString(R.string.select_unit);
            status.setText(getString(R.string.game_status, state.turn, instruction, state.message));
        }
    }

    @Override public Object onRetainNonConfigurationInstance() { return controller; }
    @Override protected void onDestroy() {
        controller.listener = null;
        if (!isChangingConfigurations()) controller.close();
        super.onDestroy();
    }
}
