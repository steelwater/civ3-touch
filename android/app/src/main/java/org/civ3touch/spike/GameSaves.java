package org.civ3touch.spike;

import android.content.Context;
import android.util.AtomicFile;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;

/** App-private durable slots. A failed write retains the previously committed slot. */
final class GameSaves {
    private static final int LIMIT = 16 * 1024 * 1024;
    private final File directory;
    GameSaves(Context context, boolean synthetic) {
        directory = new File(context.getNoBackupFilesDir(), synthetic ? "synthetic-saves" : "saves");
    }
    private AtomicFile slot(boolean recovery) {
        return new AtomicFile(new File(directory, recovery ? "recovery.json" : "manual.json"));
    }
    void write(boolean recovery, String value) throws IOException {
        byte[] bytes = value.getBytes(StandardCharsets.UTF_8);
        if (bytes.length > LIMIT) throw new IOException("Save exceeds 16 MiB");
        if (!directory.isDirectory() && !directory.mkdirs()) throw new IOException("Cannot create save directory");
        AtomicFile file = slot(recovery);
        FileOutputStream output = null;
        try {
            output = file.startWrite();
            output.write(bytes);
            output.getFD().sync();
            file.finishWrite(output);
            output = null;
            if (!read(recovery).equals(value)) throw new IOException("Save verification failed");
        } catch (IOException failure) {
            if (output != null) file.failWrite(output);
            throw failure;
        }
    }
    String read(boolean recovery) throws IOException {
        try (FileInputStream input = slot(recovery).openRead(); ByteArrayOutputStream output = new ByteArrayOutputStream()) {
            byte[] buffer = new byte[8192];
            int count;
            while ((count = input.read(buffer)) != -1) {
                if (output.size() + count > LIMIT) throw new IOException("Save exceeds 16 MiB");
                output.write(buffer, 0, count);
            }
            return output.toString(StandardCharsets.UTF_8.name());
        }
    }
}
