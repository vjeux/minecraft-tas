package dev.vjeux.mctas.bridge;

import java.io.FileDescriptor;
import java.io.FileOutputStream;
import java.io.OutputStream;
import java.nio.file.Files;
import java.nio.file.Path;

/**
 * Entry point of the in-JVM bridge.
 *
 * <p>The JVM runs the real Minecraft server classes, but instead of the dedicated server shell it
 * runs {@link SimServer}: no network, no players, no autosave, and no free-running tick loop. A
 * controller (the Rust `mctas` crate) talks to it over stdin/stdout with one JSON object per line.
 * Everything Minecraft logs goes to stderr so stdout stays a clean protocol channel.
 */
public final class Main {
    private Main() {}

    public static void main(String[] args) throws Exception {
        // Grab the real stdout before anything (log4j included) can write to it.
        OutputStream protocolOut = new FileOutputStream(FileDescriptor.out);
        System.setOut(System.err);

        Path universe = null;
        int listen = -1;
        for (int i = 0; i < args.length; i++) {
            if (args[i].equals("--universe") && i + 1 < args.length) {
                universe = Path.of(args[++i]);
            } else if (args[i].equals("--listen") && i + 1 < args.length) {
                listen = Integer.parseInt(args[++i]);
            }
        }
        if (universe == null) {
            universe = Files.createTempDirectory("mctas-universe-");
        }
        Files.createDirectories(universe);

        Protocol protocol = new Protocol(System.in, protocolOut);
        SimServer.launch(universe, protocol, listen);
    }
}
