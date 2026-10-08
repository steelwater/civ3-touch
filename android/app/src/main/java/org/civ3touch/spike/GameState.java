package org.civ3touch.spike;

import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;
import java.util.ArrayList;
import java.util.List;

/** Read-only presentation of the native snapshot. No movement rules live here. */
final class GameState {
    static final class Tile {
        final int x, y;
        final String terrain, vegetation, visibility, improvement;
        final int road;
        Tile(JSONObject tile) throws JSONException {
            JSONObject coord = tile.getJSONObject("coord");
            x = coord.getInt("x"); y = coord.getInt("y");
            terrain = tile.getString("terrain");
            vegetation = tile.getString("vegetation");
            visibility = tile.getString("visibility");
            road = tile.getInt("road_level");
            improvement = tile.isNull("improvement") ? "" : tile.getInt("improvement") == 1 ? "Mine" : "Irrigation";
        }
    }
    final int turn, width, height, index, generation, x, y, movement, attack;
    final String name, message, researchName;
    final int gold, goldPerTurn, science, sciencePerTurn;
    final int facingColumn;
    final boolean unitMoved;
    final String json;
    final JSONObject view, root;
    final JSONArray units, cities, available;
    final List<JSONObject> displayedCities = new ArrayList<>();
    final List<JSONObject> ownUnits = new ArrayList<>();
    final List<Tile> tiles = new ArrayList<>();
    final List<int[]> destinations = new ArrayList<>();

    GameState(String json, int selectedIndex, int selectedGeneration) throws JSONException {
        this.json = json;
        root = new JSONObject(json);
        view = root.getJSONObject("view");
        gold = view.getInt("gold"); goldPerTurn = view.getInt("gold_per_turn");
        science = view.getInt("science"); sciencePerTurn = view.getInt("science_per_turn");
        researchName = view.isNull("researching_name") ? "none" : view.getString("researching_name");
        units = view.getJSONArray("known_units");
        cities = view.getJSONArray("own_cities");
        available = root.getJSONArray("available");
        for (int i = 0; i < cities.length(); i++) displayedCities.add(cities.getJSONObject(i));
        JSONArray known = view.getJSONArray("known_cities");
        for (int i = 0; i < known.length(); i++) displayedCities.add(known.getJSONObject(i));
        for (int i = 0; i < units.length(); i++) {
            JSONObject candidate = units.getJSONObject(i);
            if (candidate.getInt("owner") == 0) ownUnits.add(candidate);
        }
        turn = view.getInt("turn");
        width = view.getInt("map_width"); height = view.getInt("map_height");
        JSONObject unit = ownUnits.isEmpty() ? null : ownUnits.get(0);
        for (JSONObject candidate : ownUnits) {
            JSONObject id = candidate.getJSONObject("id");
            if (id.getInt("index") == selectedIndex && id.getInt("generation") == selectedGeneration) unit = candidate;
        }
        if (unit == null) {
            index = -1; generation = -1; movement = 0; attack = 0; facingColumn = 2; name = "city";
            JSONObject position = cities.length() == 0 ? null : cities.getJSONObject(0).getJSONObject("position");
            x = position == null ? width / 2 : position.getInt("x");
            y = position == null ? height / 2 : position.getInt("y");
        } else {
            index = unit.getJSONObject("id").getInt("index");
            generation = unit.getJSONObject("id").getInt("generation");
            x = unit.getJSONObject("position").getInt("x");
            y = unit.getJSONObject("position").getInt("y");
            movement = unit.getInt("movement");
            attack = unit.getInt("attack");
            // Presentation mapping only; the engine owns the direction itself.
            switch (unit.getString("direction")) {
                case "SW": facingColumn = 0; break;
                case "S": facingColumn = 1; break;
                case "SE": facingColumn = 2; break;
                case "E": facingColumn = 3; break;
                case "NE": facingColumn = 4; break;
                case "N": facingColumn = 5; break;
                case "NW": facingColumn = 6; break;
                case "W": facingColumn = 7; break;
                default: throw new JSONException("Unknown native unit direction");
            }
            name = unit.getString("unit_type_name");
        }
        JSONArray visible = view.getJSONArray("visible_tiles");
        for (int i = 0; i < visible.length(); i++) tiles.add(new Tile(visible.getJSONObject(i)));
        JSONArray moves = root.getJSONArray("moves");
        for (int i = 0; i < moves.length(); i++) {
            JSONObject move = moves.getJSONObject(i);
            JSONObject id = move.getJSONObject("unit_id");
            if (id.getInt("index") != index || id.getInt("generation") != generation) continue;
            JSONArray coords = move.getJSONArray("destinations");
            for (int j = 0; j < coords.length(); j++) {
                JSONObject coord = coords.getJSONObject(j);
                destinations.add(new int[]{coord.getInt("x"), coord.getInt("y")});
            }
        }
        JSONObject result = root.getJSONObject("result");
        JSONArray errors = result.getJSONArray("errors");
        String feedback = "";
        boolean moved = false;
        if (errors.length() > 0) {
            Object error = errors.get(0);
            feedback = error instanceof JSONObject ? ((JSONObject) error).getString("Custom") : error.toString();
        } else {
            List<String> completions = new ArrayList<>();
            JSONArray events = result.getJSONArray("events");
            for (int i = 0; i < events.length(); i++) {
                JSONObject event = events.getJSONObject(i);
                if (event.has("UnitMoved")) { feedback = "Moved one tile."; moved = true; }
                if (event.has("TurnStarted")) feedback = "Turn advanced.";
                if (event.has("CityFounded")) feedback = "Founded " + event.getJSONObject("CityFounded").getString("name") + ".";
                if (event.has("ProductionComplete")) completions.add("Produced " + event.getJSONObject("ProductionComplete").getString("item_name") + ".");
                if (event.has("TechResearched")) completions.add("Research complete: " + event.getJSONObject("TechResearched").getString("tech_id").replace('_', ' ') + ".");
                if (event.has("ActionCompleted")) completions.add("Completed " + event.getJSONObject("ActionCompleted").getString("action_id").replace('_', ' ') + ".");
                if (event.has("CombatResolved")) feedback = "Combat resolved. Surviving unit has " + event.getJSONObject("CombatResolved").getInt("winner_hp") + " HP.";
                if (event.has("MoveBlocked")) feedback = "Move blocked: " + event.getJSONObject("MoveBlocked").getString("reason");
            }
            if (!completions.isEmpty()) feedback = String.join("\n", completions);
        }
        message = feedback;
        unitMoved = moved;
    }
}
