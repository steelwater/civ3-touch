package org.civ3touch.spike;

import android.util.AtomicFile;
import java.io.File;
import java.io.FileOutputStream;
import java.nio.file.Files;
import org.json.JSONObject;

/** Run against Android's real AtomicFile compatibility reader and filesystem, via app_process. */
public final class AndroidSaveSlotsTest {
    public static void main(String[] args) throws Exception {
        File root = Files.createTempDirectory(new File(args[0]).toPath(), "civ3touch-slots-").toFile();
        GameSaves saves = new GameSaves(root);
        String prior = "{\"turn\":7}";
        saves.write("manual", prior);
        saves.write("quick", "{\"turn\":8}");
        saves.write("recovery", "{\"turn\":9}");
        if (!saves.read(false).equals(prior)) throw new AssertionError("Slot isolation");
        if (!saves.describe("quick").startsWith("Turn 8 • ")) throw new AssertionError("Slot metadata");
        // Model an interrupted old APK's backup-based transaction.
        File manual = new File(root, "manual.json");
        Files.move(manual.toPath(), new File(root, "manual.json.bak").toPath());
        Files.write(manual.toPath(), "partial".getBytes(java.nio.charset.StandardCharsets.UTF_8));
        if (!saves.read(false).equals(prior)) throw new AssertionError("Legacy backup recovery");
        saves.write("manual", "{\"turn\":10}");
        if (new JSONObject(saves.read(false)).getInt("turn") != 10) throw new AssertionError("Legacy backup overrode new commit");
        for (int turn = 1; turn <= 8; turn++) {
            saves.autosave("{\"turn\":" + turn + "}", 3);
            Thread.sleep(5);
        }
        if (saves.autosaves().size() != 3) throw new AssertionError("Retention bound");
        java.util.Set<Integer> turns = new java.util.TreeSet<>();
        for (String slot : saves.autosaves()) turns.add(new JSONObject(saves.read(slot)).getInt("turn"));
        if (!turns.toString().equals("[6, 7, 8]")) throw new AssertionError("Oldest-first replacement: " + turns);
        for (int turn = 9; turn <= 20; turn++) { saves.autosave("{\"turn\":" + turn + "}", 10); Thread.sleep(5); }
        if (saves.autosaves().size() != 10) throw new AssertionError("Maximum retention");
        saves.autosave("{\"turn\":21}", 3);
        if (saves.autosaves().size() != 10) throw new AssertionError("Preference reduction deleted older backups");
        // AtomicFile recovery from a genuinely unfinished transaction must retain committed bytes.
        AtomicFile legacy = new AtomicFile(new File(root, "recovery.json"));
        FileOutputStream output = legacy.startWrite(); output.write("incomplete".getBytes()); output.close();
        if (new JSONObject(saves.read(true)).getInt("turn") != 9) throw new AssertionError("Interrupted AtomicFile read");
        for (File child : root.listFiles()) Files.delete(child.toPath());
        Files.delete(root.toPath());
        System.out.println("PASS: Android slot isolation, date/turn labels, legacy backup recovery, interrupted writes and 3/10 slot retention");
    }
}
