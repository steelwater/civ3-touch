package org.civ3touch.spike;

import android.content.Context;
import android.util.AtomicFile;
import org.json.JSONObject;
import java.io.File;
import java.io.FileInputStream;
import java.io.IOException;
import java.text.DateFormat;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.Date;
import java.util.List;

/** App-private slots. Native save bytes and existing manual/recovery paths stay unchanged. */
final class GameSaves {
    private final File directory;
    GameSaves(Context context, boolean synthetic) {
        this(new File(context.getNoBackupFilesDir(), synthetic ? "synthetic-saves" : "saves"));
    }
    GameSaves(File directory) { this.directory = directory; }
    private File slot(String name) {
        if (!name.matches("manual|recovery|quick|auto-[0-9]")) throw new IllegalArgumentException("Unknown save slot");
        return new File(directory, name + ".json");
    }
    void write(boolean recovery, String value) throws IOException { write(recovery ? "recovery" : "manual", value); }
    void write(String name, String value) throws IOException {
        File file = slot(name);
        // Recover a pre-M7 AtomicFile backup before replacing its base file.
        if (new File(file.getPath() + ".bak").exists()) {
            try (FileInputStream ignored = new AtomicFile(file).openRead()) { /* Restores legacy committed bytes. */ }
        }
        SaveFiles.write(file, value);
    }
    String read(boolean recovery) throws IOException { return read(recovery ? "recovery" : "manual"); }
    String read(String name) throws IOException {
        try (FileInputStream input = new AtomicFile(slot(name)).openRead()) { return SaveFiles.read(input); }
    }
    String describe(String name) {
        try {
            JSONObject saved = new JSONObject(read(name));
            return "Turn " + saved.getInt("turn") + " • "
                    + DateFormat.getDateTimeInstance(DateFormat.SHORT, DateFormat.SHORT).format(new Date(slot(name).lastModified()));
        } catch (java.io.FileNotFoundException failure) { return "Empty slot"; }
        catch (Exception failure) { return "Unreadable slot — loading will show details"; }
    }
    void autosave(String value, int retention) throws IOException {
        if (retention != 3 && retention != 5 && retention != 10) throw new IllegalArgumentException("Unsupported retention");
        String oldest = "auto-0";
        for (int i = 0; i < retention; i++) {
            String name = "auto-" + i;
            if (!slot(name).exists()) { oldest = name; break; }
            if (slot(name).lastModified() < slot(oldest).lastModified()) oldest = name;
        }
        write(oldest, value);
    }
    List<String> autosaves() {
        List<String> names = new ArrayList<>();
        // Older slots remain available if the user lowers retention; never silently delete backups.
        for (int i = 0; i < 10; i++) if (slot("auto-" + i).exists()) names.add("auto-" + i);
        names.sort(Comparator.comparingLong((String name) -> slot(name).lastModified()).reversed());
        return names;
    }
}
