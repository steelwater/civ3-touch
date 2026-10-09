package org.civ3touch.spike;

import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.ByteBuffer;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.StandardCopyOption;

/** Bounded UTF-8 transport. All private writes run on the controller's single worker. */
final class SaveFiles {
    static final int LIMIT = 16 * 1024 * 1024;

    static String read(InputStream input) throws IOException {
        if (input == null) throw new IOException("Document provider did not open the file");
        ByteArrayOutputStream output = new ByteArrayOutputStream();
        byte[] buffer = new byte[8192];
        int count;
        while ((count = input.read(buffer)) != -1) {
            if (output.size() + count > LIMIT) throw new IOException("Save exceeds 16 MiB");
            output.write(buffer, 0, count);
        }
        try {
            return StandardCharsets.UTF_8.newDecoder().onMalformedInput(CodingErrorAction.REPORT)
                    .onUnmappableCharacter(CodingErrorAction.REPORT).decode(ByteBuffer.wrap(output.toByteArray())).toString();
        } catch (CharacterCodingException failure) {
            throw new IOException("Save is not UTF-8 Civ3Touch JSON", failure);
        }
    }

    static byte[] bytes(String value) throws IOException {
        byte[] bytes = value.getBytes(StandardCharsets.UTF_8);
        if (bytes.length > LIMIT) throw new IOException("Save exceeds 16 MiB");
        return bytes;
    }

    static synchronized void write(File destination, String value) throws IOException {
        byte[] bytes = bytes(value);
        File directory = destination.getParentFile();
        if (!directory.isDirectory() && !directory.mkdirs()) throw new IOException("Cannot create save directory");
        // Serialize app-process writers, including overlapping Activity instances.
        // Fixed staging name bounds leftovers from killed processes. Never treat staging as a save.
        File staged = new File(directory, destination.getName() + ".pending");
        try {
            try (FileOutputStream output = new FileOutputStream(staged)) {
                output.write(bytes);
                output.getFD().sync();
            }
            try (FileInputStream input = new FileInputStream(staged)) {
                if (!read(input).equals(value)) throw new IOException("Save verification failed before commit");
            }
            // Same-directory atomic replacement or failure; never fall back to delete/copy.
            Files.move(staged.toPath(), destination.toPath(), StandardCopyOption.ATOMIC_MOVE,
                    StandardCopyOption.REPLACE_EXISTING);
        } finally {
            Files.deleteIfExists(staged.toPath());
        }
    }
}
