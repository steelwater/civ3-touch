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
        final String terrain, vegetation, visibility;
        Tile(JSONObject tile) throws JSONException {
            JSONObject coord = tile.getJSONObject("coord");
            x = coord.getInt("x"); y = coord.getInt("y");
            terrain = tile.getString("terrain");
            vegetation = tile.getString("vegetation");
            visibility = tile.getString("visibility");
        }
    }
    final int turn, width, height, index, generation, x, y, movement;
    final String name, message;
    final List<Tile> tiles = new ArrayList<>();
    final List<int[]> destinations = new ArrayList<>();

    GameState(String json) throws JSONException {
        JSONObject root = new JSONObject(json);
        JSONObject view = root.getJSONObject("view");
        turn = view.getInt("turn");
        width = view.getInt("map_width"); height = view.getInt("map_height");
        JSONObject unit = view.getJSONArray("known_units").getJSONObject(0);
        index = unit.getJSONObject("id").getInt("index");
        generation = unit.getJSONObject("id").getInt("generation");
        x = unit.getJSONObject("position").getInt("x");
        y = unit.getJSONObject("position").getInt("y");
        movement = unit.getInt("movement");
        name = unit.getString("unit_type_name");
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
        if (errors.length() > 0) {
            Object error = errors.get(0);
            feedback = error instanceof JSONObject ? ((JSONObject) error).getString("Custom") : error.toString();
        } else {
            JSONArray events = result.getJSONArray("events");
            for (int i = 0; i < events.length(); i++) {
                JSONObject event = events.getJSONObject(i);
                if (event.has("UnitMoved")) feedback = "Moved one tile.";
                if (event.has("TurnStarted")) feedback = "Turn advanced.";
                if (event.has("MoveBlocked")) feedback = "Move blocked: " + event.getJSONObject("MoveBlocked").getString("reason");
            }
        }
        message = feedback;
    }
}
