package org.civ3touch.spike;

import java.io.ByteArrayInputStream;
import java.io.File;
import java.io.FileInputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.util.Arrays;

/** Behavior tests use the real filesystem and bounded stream path; no Android mocks. */
public final class SaveFilesTest {
    interface Checked { void run() throws Exception; }
    static void fails(Checked action) throws Exception {
        try { action.run(); } catch (IOException expected) { return; }
        throw new AssertionError("Expected an I/O failure");
    }
    static String read(File file) throws Exception {
        try (FileInputStream input = new FileInputStream(file)) { return SaveFiles.read(input); }
    }
    public static void main(String[] args) throws Exception {
        File root = Files.createTempDirectory("civ3touch-save-test").toFile();
        File file = new File(root, "quick.json");
        String original = "{\"turn\":7,\"name\":\"日本語\"}";
        SaveFiles.write(file, original);
        if (!read(file).equals(original)) throw new AssertionError("UTF-8 did not round trip");
        Files.write(new File(root, "quick.json.pending").toPath(), new byte[]{1, 2});
        if (!read(file).equals(original)) throw new AssertionError("Interrupted staging replaced committed save");
        SaveFiles.write(file, "replacement");
        if (!read(file).equals("replacement")) throw new AssertionError("Replacement failed");
        String tooLarge = "x".repeat(SaveFiles.LIMIT + 1);
        fails(() -> SaveFiles.write(file, tooLarge));
        if (!read(file).equals("replacement")) throw new AssertionError("Rejected write destroyed prior save");
        fails(() -> SaveFiles.read(new ByteArrayInputStream(tooLarge.getBytes(java.nio.charset.StandardCharsets.UTF_8))));
        fails(() -> SaveFiles.read(new ByteArrayInputStream(new byte[]{(byte) 0xc3, 0x28})));
        fails(() -> SaveFiles.read(null));
        fails(() -> SaveFiles.read(new InputStream() {
            @Override public int read() throws IOException { throw new IOException("Provider lost access"); }
        }));
        File staged = new File(root, "quick.json.pending");
        if (!staged.mkdir()) throw new AssertionError("Test setup");
        // A staging write cannot open a directory. The destination must remain byte-identical.
        fails(() -> SaveFiles.write(file, "failed write"));
        if (!read(file).equals("replacement")) throw new AssertionError("Failed write destroyed prior save");
        byte[] boundary = new byte[SaveFiles.LIMIT]; Arrays.fill(boundary, (byte) 'a');
        if (SaveFiles.read(new ByteArrayInputStream(boundary)).length() != boundary.length) throw new AssertionError("Limit boundary");
        java.util.concurrent.atomic.AtomicReference<Throwable> failure = new java.util.concurrent.atomic.AtomicReference<>();
        Thread first = new Thread(() -> {
            try { for (int i = 0; i < 20; i++) SaveFiles.write(file, "first writer"); }
            catch (Throwable error) { failure.set(error); }
        });
        Thread second = new Thread(() -> {
            try { for (int i = 0; i < 20; i++) SaveFiles.write(file, "second writer"); }
            catch (Throwable error) { failure.set(error); }
        });
        first.start(); second.start(); first.join(); second.join();
        if (failure.get() != null) throw new AssertionError("Overlapping writers", failure.get());
        if (!java.util.List.of("first writer", "second writer").contains(read(file))) throw new AssertionError("Partial concurrent save");
        for (File child : root.listFiles()) Files.delete(child.toPath());
        Files.delete(root.toPath());
        System.out.println("PASS: native save UTF-8, bounds, interrupted staging, atomic replacement failed-write preservation and overlapping writers");
    }
}
