package org.civ3touch.spike;

import android.content.Context;
import android.database.Cursor;
import android.net.Uri;
import android.provider.DocumentsContract;
import org.json.JSONArray;
import org.json.JSONObject;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.util.HashMap;
import java.util.Locale;
import java.util.Map;
import java.util.UUID;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.StandardCopyOption;

/** SAF and private storage only. Required paths and limits come from the shared profile. */
final class AssetImporter {
    private static final Object STORAGE_LOCK = new Object();
    private final Context context;
    private final File storage;
    interface Progress { void update(String message); }
    AssetImporter(Context context) {
        this.context = context;
        storage = new File(context.getNoBackupFilesDir(), "imports");
    }

    private File active() throws IOException {
        File record = new File(storage, "active");
        if (!record.exists()) return null;
        if (record.length() > 64) throw new IOException("Invalid import record. Import the GOG folder again.");
        String name = new String(Files.readAllBytes(record.toPath()), StandardCharsets.UTF_8);
        if (!name.matches("data-[a-f0-9-]{36}")) throw new IOException("Invalid import record. Import the GOG folder again.");
        return new File(storage, name);
    }

    ImportedAssets restore() throws Exception {
        synchronized (STORAGE_LOCK) {
            File root = active();
            if (root == null) return null;
            return new ImportedAssets(root, CoreBridge.loadAssets(root.getPath()));
        }
    }

    ImportedAssets importFolder(Uri tree, Progress progress) throws Exception {
        synchronized (STORAGE_LOCK) { return importLocked(tree, progress); }
    }

    private ImportedAssets importLocked(Uri tree, Progress progress) throws Exception {
        if (!storage.isDirectory() && !storage.mkdirs()) throw new IOException("Cannot create private import storage.");
        // A prior process may have died before publication. Only private staging is discarded.
        File[] leftovers = storage.listFiles();
        if (leftovers != null) for (File leftover : leftovers) {
            if (leftover.getName().matches("stage-[a-f0-9-]{36}|pointer-[a-f0-9-]{36}")) discard(leftover);
        }
        JSONArray entries = new JSONObject(CoreBridge.importProfile()).getJSONArray("files");
        Uri root = DocumentsContract.buildDocumentUriUsingTree(tree, DocumentsContract.getTreeDocumentId(tree));
        Map<String, Map<String, Uri>> directories = new HashMap<>();
        Map<String, Uri> sources = new HashMap<>();
        StringBuilder missing = new StringBuilder();
        progress.update("Checking source files…");
        for (int i = 0; i < entries.length(); i++) {
            JSONObject entry = entries.getJSONObject(i);
            String path = entry.getString("path");
            Uri source = resolve(tree, root, path, directories);
            if (source == null && entry.getBoolean("required")) missing.append("\n• ").append(path);
            if (source != null) sources.put(path, source);
        }
        if (missing.length() > 0) throw new IOException("Missing required files:" + missing
                + "\nSelect the copied installation root containing Art, Text and Conquests, including the GOG .info file.");
        long requiredSpace = 0;
        for (int i = 0; i < entries.length(); i++) requiredSpace += entries.getJSONObject(i).getLong("limit");
        if (storage.getUsableSpace() < requiredSpace + 8 * 1024 * 1024) throw new IOException("Not enough free app storage. Free at least 32 MB and retry.");
        File stage = new File(storage, "stage-" + UUID.randomUUID());
        if (!stage.mkdir()) throw new IOException("Cannot create staging folder.");
        File published = null;
        boolean committed = false;
        try {
            for (int i = 0; i < entries.length(); i++) {
                JSONObject entry = entries.getJSONObject(i);
                String path = entry.getString("path");
                if (!sources.containsKey(path)) continue;
                progress.update("Copying " + (i + 1) + "/" + entries.length() + ": " + path);
                File destination = new File(stage, path);
                if (!destination.getParentFile().isDirectory() && !destination.getParentFile().mkdirs()) throw new IOException("Cannot create import folder.");
                try (InputStream input = context.getContentResolver().openInputStream(sources.get(path));
                     FileOutputStream output = new FileOutputStream(destination)) {
                    if (input == null) throw new IOException("Cannot read " + path);
                    byte[] buffer = new byte[32768]; int count; long total = 0;
                    while ((count = input.read(buffer)) != -1) {
                        if (Thread.currentThread().isInterrupted()) throw new IOException("Import interrupted. Retry the folder selection.");
                        total += count;
                        if (total > entry.getLong("limit")) throw new IOException("Unsupported size: " + path);
                        output.write(buffer, 0, count);
                    }
                    output.getFD().sync();
                } catch (IOException failure) {
                    if (entry.getBoolean("required")) throw new IOException("Cannot copy " + path + ": " + failure.getMessage(), failure);
                    // Optional media failure must not publish an incomplete audio file.
                    if (destination.exists() && !destination.delete()) throw new IOException("Cannot discard incomplete audio: " + path);
                }
            }
            progress.update("Validating GOG identity and decoding imported graphics…");
            String metadata = CoreBridge.loadAssets(stage.getPath());
            published = new File(storage, "data-" + UUID.randomUUID());
            if (!stage.renameTo(published)) throw new IOException("Cannot finalize import. Previous data is unchanged.");
            // Build every Bitmap before switching the active record.
            ImportedAssets assets = new ImportedAssets(published, metadata);
            File previous;
            try { previous = active(); }
            catch (IOException corruptRecord) { previous = null; }
            File pointer = new File(storage, "pointer-" + UUID.randomUUID());
            try {
                try (FileOutputStream output = new FileOutputStream(pointer)) {
                    output.write(published.getName().getBytes(StandardCharsets.UTF_8));
                    output.getFD().sync();
                }
                Files.move(pointer.toPath(), new File(storage, "active").toPath(),
                        StandardCopyOption.ATOMIC_MOVE, StandardCopyOption.REPLACE_EXISTING);
            } finally { discard(pointer); }
            committed = true;
            if (previous != null) discard(previous);
            return assets;
        } finally {
            discard(stage);
            if (!committed && published != null) discard(published);
        }
    }

    private Uri resolve(Uri tree, Uri root, String path, Map<String, Map<String, Uri>> cache) throws IOException {
        Uri current = root;
        for (String part : path.split("/")) {
            String id = DocumentsContract.getDocumentId(current);
            if (!cache.containsKey(id)) {
                Map<String, Uri> children = new HashMap<>();
                Uri listing = DocumentsContract.buildChildDocumentsUriUsingTree(tree, id);
                String[] columns = {DocumentsContract.Document.COLUMN_DOCUMENT_ID, DocumentsContract.Document.COLUMN_DISPLAY_NAME};
                try (Cursor cursor = context.getContentResolver().query(listing, columns, null, null, null)) {
                    if (cursor == null) throw new IOException("Cannot read selected folder. Copy it to local device storage and retry.");
                    int count = 0;
                    while (cursor.moveToNext()) {
                        if (++count > 20000) throw new IOException("Folder contains too many entries for the supported GOG layout.");
                        String name = cursor.getString(1);
                        if (name == null) continue;
                        String key = name.toLowerCase(Locale.ROOT);
                        if (children.containsKey(key)) throw new IOException("Duplicate filename casing: " + name + ". Use an unmodified GOG folder.");
                        children.put(key, DocumentsContract.buildDocumentUriUsingTree(tree, cursor.getString(0)));
                    }
                }
                cache.put(id, children);
            }
            current = cache.get(id).get(part.toLowerCase(Locale.ROOT));
            if (current == null) return null;
        }
        return current;
    }

    // Only importer-created private staging/old-generation folders reach this method.
    private static void discard(File file) {
        File[] children = file.listFiles();
        if (children != null) for (File child : children) discard(child);
        if (file.exists()) file.delete();
    }
}
