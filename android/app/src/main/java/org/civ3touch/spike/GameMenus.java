package org.civ3touch.spike;

import android.app.Activity;
import android.app.AlertDialog;
import org.json.JSONArray;
import org.json.JSONObject;
import org.json.JSONException;
import java.util.ArrayList;
import java.util.List;

/** Small native menus over engine-advertised commands, without duplicating game rules. */
final class GameMenus {
    private final Activity activity;
    private final GameController controller;
    private final List<String> labels = new ArrayList<>();
    private final List<Runnable> actions = new ArrayList<>();
    GameMenus(Activity activity, GameController controller) { this.activity = activity; this.controller = controller; }
    private void add(String label, Runnable action) { labels.add(label); actions.add(action); }
    private void show(String title) {
        new AlertDialog.Builder(activity).setTitle(title).setItems(labels.toArray(new String[0]),
                (dialog, which) -> actions.get(which).run()).setNegativeButton("Close", null).show();
    }
    private void send(String type, JSONObject value) {
        try { controller.command(new JSONObject().put(type, value).toString()); }
        catch (JSONException failure) { fail(failure); }
    }
    private void fail(Exception failure) { controller.error = failure.getMessage(); controller.notifyChanged(); }
    void main() {
        GameState state = controller.state;
        if (state != null) {
            add("Choose unit", () -> new GameMenus(activity, controller).units());
            add("Cities and production", () -> new GameMenus(activity, controller).cities());
            add("Research", () -> new GameMenus(activity, controller).research());
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
                    for (String kind : new String[]{"Skip", "Fortify"}) {
                        if (entry.has(kind)) {
                            JSONObject command = entry.getJSONObject(kind);
                            if (selected(command.getJSONObject("unit_id"))) add(kind + " unit", () -> send(kind + "Unit", command));
                        }
                    }
                }
            } catch (JSONException failure) { fail(failure); }
            add("Save game", controller::saveGame);
        }
        if (controller.assets != null || controller.prototype) {
            add("Load saved game", () -> controller.loadGame(false));
            add("Resume recovery save", () -> controller.loadGame(true));
        }
        show(state == null ? "Game" : state.name + " — actions");
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
    private void city(JSONObject city) {
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
            String details = city.getString("name") + " • population " + city.getInt("population")
                    + "\nFood " + city.getInt("food_stockpile") + "/" + city.getInt("food_growth_threshold")
                    + " (gross " + city.getInt("food_per_turn") + "/turn)"
                    + "\nShields " + city.getInt("shield_stockpile") + "/" + city.getInt("production_cost")
                    + " (+" + city.getInt("shields_per_turn") + ") • commerce " + city.getInt("commerce_per_turn")
                    + "\nProducing: " + city.optString("producing", "none")
                    + "\nBuildings: " + city.getJSONArray("buildings")
                    + "\nQueue: " + (pending.isEmpty() ? "empty" : String.join(", ", pending));
            add("City details", () -> new AlertDialog.Builder(activity).setTitle("City details")
                    .setMessage(details).setPositiveButton("Close", null).show());
            show(city.getString("name") + " • population " + city.getInt("population"));
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
            add("Research status", () -> new AlertDialog.Builder(activity).setTitle("Research status")
                    .setMessage(details).setPositiveButton("Close", null).show());
            show(labels.size() == 1 ? "Research in progress or unavailable" : "Choose research");
        } catch (JSONException failure) { fail(failure); }
    }
}
