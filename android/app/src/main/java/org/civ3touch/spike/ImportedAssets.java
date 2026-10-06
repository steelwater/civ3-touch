package org.civ3touch.spike;

import android.graphics.Bitmap;
import org.json.JSONArray;
import org.json.JSONObject;
import java.io.File;
import java.util.HashMap;
import java.util.Map;

/** Decoded presentation data. Paths and Civ III formats are owned by Rust. */
final class ImportedAssets {
    final Map<String, Bitmap> terrain = new HashMap<>();
    final Bitmap idle, run;
    final int idleFrames, runFrames, idleDelay, runDelay;
    final File step, music;
    final String warnings;

    ImportedAssets(File root, String metadata) throws Exception {
        JSONObject info = new JSONObject(metadata);
        JSONArray images = info.getJSONArray("images");
        Map<String, Bitmap> decoded = new HashMap<>();
        Map<String, JSONObject> descriptions = new HashMap<>();
        for (int i = 0; i < images.length(); i++) {
            JSONObject image = images.getJSONObject(i);
            String key = image.getString("key");
            decoded.put(key, Bitmap.createBitmap(CoreBridge.imagePixels(key), image.getInt("width"),
                    image.getInt("height"), Bitmap.Config.ARGB_8888));
            descriptions.put(key, image);
        }
        idle = decoded.remove("idle"); run = decoded.remove("run"); terrain.putAll(decoded);
        idleFrames = descriptions.get("idle").getInt("frames");
        runFrames = descriptions.get("run").getInt("frames");
        idleDelay = descriptions.get("idle").getInt("delay");
        runDelay = descriptions.get("run").getInt("delay");
        JSONObject audio = info.getJSONObject("audio");
        step = audio.has("step") ? new File(root, audio.getString("step")) : null;
        music = audio.has("music") ? new File(root, audio.getString("music")) : null;
        warnings = info.getJSONArray("warnings").join("\n").replace("\"", "");
    }
}
