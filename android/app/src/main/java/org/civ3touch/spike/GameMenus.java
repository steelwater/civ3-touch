package org.civ3touch.spike;

import android.widget.LinearLayout;
import org.json.JSONArray;
import org.json.JSONObject;
import org.json.JSONException;
import java.util.ArrayList;
import java.util.List;

/** Small native menus over engine-advertised commands, without duplicating game rules. */
final class GameMenus {
    private final MainActivity activity;
    private final GameController controller;
    private final List<String> labels = new ArrayList<>();
    private final List<Runnable> actions = new ArrayList<>();
    GameMenus(MainActivity activity, GameController controller) { this.activity = activity; this.controller = controller; }
    private void add(String label, Runnable action) { labels.add(label); actions.add(action); }
    private void show(String title) { show(title, ""); }
    private void show(String title, String details) {
        TouchUi.screen(activity, title, details, labels, actions);
    }
    private void send(String type, JSONObject value) {
        try { controller.command(new JSONObject().put(type, value).toString()); }
        catch (JSONException failure) { fail(failure); }
    }
    private void fail(Exception failure) { controller.error = failure.getMessage(); controller.notifyChanged(); }
    void main() {
        GameState state = controller.state;
        if (state != null) {
            if (activity.largeTextPhone()) add("New Game", activity::startNewGame);
            add("Choose unit", () -> new GameMenus(activity, controller).units());
            add("Cities and production", () -> new GameMenus(activity, controller).cities());
            add("Research", () -> new GameMenus(activity, controller).research());
            add("Diplomacy", () -> new GameMenus(activity, controller).diplomacy());
            add("Selected unit actions", () -> new GameMenus(activity, controller).unitScreen());
            add("Quick Save", this::quickSave);
            add("Quick Load", () -> load("quick", "Quick Load"));
            add("Save game", controller::saveGame);
            add("Saves and backups", () -> new GameMenus(activity, controller).saves());
            add("Game settings", activity::gameSettings);
        }
        if (controller.assets != null || controller.prototype) {
            add("Load saved game", () -> controller.loadGame(false));
            add("Resume recovery save", () -> controller.loadGame(true));
        }
        if (state == null) {
            if (controller.assets != null || controller.prototype) add("Quick Load", () -> load("quick", "Quick Load"));
            add("Saves and backups", () -> new GameMenus(activity, controller).saves());
            add("Game settings", activity::gameSettings);
        }
        show(state == null ? "Game" : state.name + " — actions");
    }
    private void confirm(String title, String details, Runnable action) {
        TouchUi.screen(activity, title, details, java.util.Arrays.asList("Confirm", "Cancel"),
                java.util.Arrays.asList(action, () -> {}));
    }
    private void load(String name, String title) {
        controller.inspectSaves(() -> {
            if (activity.isDestroyed()) return;
            confirm(title, controller.slotDescriptions.get(name)
                    + "\nLoad this slot? It replaces the live game after validation. Save current progress first.",
                    () -> controller.loadSlot(name));
        });
    }
    private void quickSave() {
        controller.inspectSaves(() -> {
            if (!activity.isDestroyed()) confirm("Quick Save", "Replace the quick slot? "
                    + controller.slotDescriptions.get("quick"), controller::quickSave);
        });
    }
    void saves() {
        controller.inspectSaves(() -> {
            if (!activity.isDestroyed()) new GameMenus(activity, controller).showSaves();
        });
    }
    private void showSaves() {
        if (controller.state != null) {
            add("Quick Save", this::quickSave);
            add("Export native save…", () -> activity.saveDocument(false));
        }
        if (controller.assets != null || controller.prototype) {
            add("Import native save…", () -> confirm("Import native save", "Choose a Civ3Touch JSON save. A valid file replaces your live game and recovery slot. Manual and quick slots stay unchanged. Save current progress first. Original Civilization III .sav files are unsupported.",
                    () -> activity.saveDocument(true)));
            for (String name : new String[]{"quick", "manual", "recovery"})
                add("Load " + name + " — " + controller.slotDescriptions.get(name), () -> load(name, "Load " + name));
            for (String name : controller.autoSlots)
                add("Load " + name + " — " + controller.slotDescriptions.get(name), () -> load(name, "Load " + name));
        }
        add("Autosave settings", activity::autosaveSettings);
        show("Saves and backups", "Recovery records every completed action. Turn autosaves keep older checkpoints. Slots are private to this app and are lost on uninstall or clear-data. Export a native save for an external backup. No slot is loaded automatically.");
    }
    private void unitActions() {
        GameState state = controller.state;
        if (state == null || state.index < 0) return;
        try {
            for (int i = 0; i < state.available.length(); i++) {
                JSONObject entry = state.available.optJSONObject(i);
                if (entry == null) continue;
                if (entry.has("UnitAction")) {
                    JSONObject a = entry.getJSONObject("UnitAction");
                    if (selected(a.getJSONObject("unit_id"))) {
                        JSONObject command = new JSONObject().put("unit_id", a.getJSONObject("unit_id")).put("action_id", a.getString("action_id"));
                        add(a.getString("name"), () -> send("PerformAction", command));
                    }
                }
                if (entry.has("Attack") && state.attack > 0) {
                    JSONObject a = entry.getJSONObject("Attack");
                    if (selected(a.getJSONObject("unit_id"))) {
                        JSONArray targets = a.getJSONArray("targets");
                        for (int j = 0; j < targets.length(); j++) {
                            JSONObject target = targets.getJSONObject(j);
                            JSONObject command = new JSONObject().put("attacker", a.getJSONObject("unit_id")).put("defender", target);
                            add("Attack enemy " + target.getInt("index"), () -> send("AttackUnit", command));
                        }
                    }
                }
            }
            // Put unit-specific work first; common wait/defence commands follow.
            for (int i = 0; i < state.available.length(); i++) {
                JSONObject entry = state.available.optJSONObject(i);
                if (entry == null) continue;
                for (String kind : new String[]{"Skip", "Fortify"}) {
                    if (entry.has(kind)) {
                        JSONObject command = entry.getJSONObject(kind);
                        if (selected(command.getJSONObject("unit_id"))) add(kind + " unit", () -> send(kind + "Unit", command));
                    }
                }
            }
        } catch (JSONException failure) { fail(failure); }
    }
    void unitSheet(LinearLayout content) {
        unitActions();
        if (labels.isEmpty()) content.addView(TouchUi.text(activity, "No available commands. Select another unit or end the turn.", 16));
        for (int i = 0; i < labels.size(); i++) {
            android.widget.Button button = TouchUi.button(activity, labels.get(i), actions.get(i));
            button.setEnabled(!controller.busy);
            content.addView(button);
        }
    }
    void unitScreen() { unitActions(); show("Selected unit actions"); }
    void diplomacy() {
        java.util.Set<Integer> owners = new java.util.TreeSet<>();
        GameState state = controller.state;
        try {
            for (int i = 0; i < state.units.length(); i++) {
                int owner = state.units.getJSONObject(i).getInt("owner");
                if (owner != state.view.getInt("player")) owners.add(owner);
            }
            for (JSONObject city : state.displayedCities) {
                int owner = city.getInt("owner");
                if (owner != state.view.getInt("player")) owners.add(owner);
            }
        } catch (JSONException failure) { fail(failure); return; }
        String details = owners.isEmpty() ? "No other civilization appears in your current known map information. Explore to find other units and cities."
                : "Other civilizations in your known map information: " + owners + ".";
        show("Diplomacy", details + "\n\nDiplomatic negotiations are not available in this prototype. Peace, treaties and alliances are not supported yet. Return to the map to continue exploring.");
    }
    void tile(int x, int y) {
        GameState state = controller.state;
        try {
            for (JSONObject city : state.displayedCities) {
                JSONObject pos = city.getJSONObject("position");
                if (pos.getInt("x") == x && pos.getInt("y") == y && city.getInt("owner") == state.view.getInt("player"))
                    add("Open " + city.getString("name"), () -> new GameMenus(activity, controller).city(city));
            }
            for (JSONObject unit : state.ownUnits) {
                JSONObject pos = unit.getJSONObject("position");
                if (pos.getInt("x") == x && pos.getInt("y") == y)
                    add("Select " + unit.getString("unit_type_name") + " #" + unit.getJSONObject("id").getInt("index"), () -> {
                        controller.unitExpanded = true; controller.selectUnit(unit);
                    });
            }
            String details = "Unexplored tile";
            for (GameState.Tile tile : state.tiles) if (tile.x == x && tile.y == y)
                details = tile.terrain + " • " + tile.vegetation + "\n" + tile.visibility
                        + (tile.road > 0 ? " • Road" : "") + " " + tile.improvement
                        + (tile.resource.isEmpty() ? "" : "\nResource: " + tile.resource);
            show("Tile " + x + ", " + y, details);
        } catch (JSONException failure) { fail(failure); }
    }
    private boolean selected(JSONObject id) throws JSONException {
        return id.getInt("index") == controller.state.index && id.getInt("generation") == controller.state.generation;
    }
    private void units() {
        try {
            for (JSONObject unit : controller.state.ownUnits) {
                JSONObject p = unit.getJSONObject("position");
                String label = unit.getString("unit_type_name") + " #" + unit.getJSONObject("id").getInt("index")
                        + " at " + p.getInt("x") + "," + p.getInt("y") + " • HP " + unit.getInt("hp")
                        + " • move " + unit.getInt("movement") / 3.0;
                if (!unit.isNull("current_action")) label += " • " + unit.getString("current_action");
                add(label, () -> controller.selectUnit(unit));
            }
        } catch (JSONException failure) { fail(failure); }
        show(labels.isEmpty() ? "No units remain" : "Choose unit");
    }
    private void cities() {
        try {
            JSONArray cities = controller.state.cities;
            for (int i = 0; i < cities.length(); i++) {
                JSONObject city = cities.getJSONObject(i);
                add(city.getString("name") + " • population " + city.getInt("population"),
                        () -> new GameMenus(activity, controller).city(city));
            }
        } catch (JSONException failure) { fail(failure); }
        show(labels.isEmpty() ? "Found a city with a Settler first" : "Cities");
    }
    void city(JSONObject city) {
        try {
            JSONObject id = city.getJSONObject("id");
            add("Choose production (changing item resets shields)", () -> new GameMenus(activity, controller).production(id, false));
            add("Add to production queue", () -> new GameMenus(activity, controller).production(id, true));
            add("Clear pending queue", () -> send("ClearQueue", id));
            JSONArray queues = controller.state.root.getJSONArray("queues");
            List<String> pending = new ArrayList<>();
            for (int i = 0; i < queues.length(); i++) {
                JSONArray queue = queues.getJSONArray(i);
                JSONObject owner = queue.getJSONObject(0);
                if (sameId(id, owner)) {
                    JSONArray items = queue.getJSONArray(1);
                    for (int j = 0; j < items.length(); j++) pending.add(productionName(id, items.get(j)));
                }
            }
            String details = "Food " + city.getInt("food_stockpile") + "/" + city.getInt("food_growth_threshold")
                    + " (gross " + city.getInt("food_per_turn") + "/turn)"
                    + "\nShields " + city.getInt("shield_stockpile") + "/" + city.getInt("production_cost")
                    + " (+" + city.getInt("shields_per_turn") + ") • commerce " + city.getInt("commerce_per_turn")
                    + "\nProducing: " + city.optString("producing", "none")
                    + "\nBuildings: " + city.getJSONArray("buildings")
                    + "\nQueue: " + (pending.isEmpty() ? "empty" : String.join(", ", pending));
            show(city.getString("name") + " • population " + city.getInt("population"), details);
        } catch (JSONException failure) { fail(failure); }
    }
    private static boolean sameId(JSONObject a, JSONObject b) throws JSONException {
        return a.getInt("index") == b.getInt("index") && a.getInt("generation") == b.getInt("generation");
    }
    private JSONArray options(JSONObject cityId) throws JSONException {
        JSONArray available = controller.state.available;
        for (int i = 0; i < available.length(); i++) {
            JSONObject entry = available.optJSONObject(i);
            if (entry == null) continue;
            if (entry.has("SetProduction")) {
                JSONObject a = entry.getJSONObject("SetProduction");
                if (sameId(a.getJSONObject("city_id"), cityId)) return a.getJSONArray("options");
            }
        }
        return new JSONArray();
    }
    private String productionName(JSONObject city, Object item) throws JSONException {
        JSONArray options = options(city);
        for (int i = 0; i < options.length(); i++) {
            JSONObject option = options.getJSONObject(i);
            if (option.get("item").toString().equals(item.toString())) return option.getString("name");
        }
        return "Unavailable item";
    }
    private void production(JSONObject city, boolean queue) {
        try {
            JSONArray options = options(city);
            for (int i = 0; i < options.length(); i++) {
                JSONObject option = options.getJSONObject(i);
                JSONObject command = new JSONObject().put("city_id", city).put("item", option.get("item"));
                add(option.getString("name") + " • " + option.getInt("cost") + " shields",
                        () -> send(queue ? "QueueProduction" : "SetProduction", command));
            }
        } catch (JSONException failure) { fail(failure); }
        show(queue ? "Queue next item" : "Choose production");
    }
    private void research() {
        try {
            GameState state = controller.state;
            for (int i = 0; i < state.available.length(); i++) {
                JSONObject entry = state.available.optJSONObject(i);
                if (entry == null) continue;
                if (!entry.has("SetResearch")) continue;
                JSONArray options = entry.getJSONObject("SetResearch").getJSONArray("options");
                for (int j = 0; j < options.length(); j++) {
                    JSONObject option = options.getJSONObject(j);
                    JSONObject command = new JSONObject().put("tech_id", option.getString("id"));
                    add(option.getString("name") + " • " + option.getInt("cost") + " science", () -> send("SetResearch", command));
                }
            }
            String details = "Research: " + (state.view.isNull("researching_name") ? "none" : state.view.getString("researching_name"))
                    + "\nScience " + state.view.getInt("science") + " (+" + state.view.getInt("science_per_turn") + "/turn)"
                    + "\nTurns left: " + state.view.optString("research_turns_left", "—")
                    + "\nCompleted: " + state.view.getJSONArray("researched_techs");
            add("Research status", () -> TouchUi.screen(activity, "Research status", details,
                    java.util.Collections.emptyList(), java.util.Collections.emptyList()));
            show(labels.size() == 1 ? "Research in progress or unavailable" : "Choose research");
        } catch (JSONException failure) { fail(failure); }
    }
}
