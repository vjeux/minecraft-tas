package dev.vjeux.mctas.probe;

import fi.dy.masa.litematica.data.DataManager;
import fi.dy.masa.litematica.scheduler.TaskScheduler;
import fi.dy.masa.litematica.schematic.LitematicaSchematic;
import fi.dy.masa.litematica.schematic.placement.SchematicPlacement;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.nio.file.StandardOpenOption;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.function.Supplier;
import net.fabricmc.api.ClientModInitializer;
import net.minecraft.client.Minecraft;
import net.minecraft.core.BlockPos;

/**
 * `mctas litematic verify-ingame`: pastes .litematic files with Litematica's OWN code, in the real
 * client. Settings come from {@code mctas-litematica/config.txt} in the game directory: {@code nbt
 * <none|place_data_modify|place_clone>} (Litematica's pasteNbtRestoreBehavior), {@code world single}
 * (create a flat creative singleplayer world, freeze its ticks, so Litematica pastes directly into
 * the integrated server; otherwise the client is on the bridge's server and Litematica pastes with
 * commands) and {@code center x y z} (where the player waits, flying). Jobs come from
 * {@code mctas-litematica/jobs.txt}, one per line: {@code <id> <file.litematic> <x> <y> <z> <x1> <y1>
 * <z1>}; each is loaded ({@code LitematicaSchematic.createFromFile}), placed at the origin
 * {@code x y z} ({@code SchematicPlacement.createFor}, added and selected in the placement manager)
 * and pasted ({@code SchematicPlacementManager.pastePlacementToWorld}); when Litematica's task
 * schedulers are empty again, in a singleplayer world the box {@code x..x1, y..y1, z..z1} is dumped
 * from the integrated server to {@code dump-<id>.txt} ({@code B x y z <state>}, {@code T x y z <type>
 * <snbt>} per block entity, {@code N <type> <x> <y> <z> <snbt>} per entity, all saved data), and
 * {@code <id> ok} is appended to {@code done.txt} ({@code <id> error <why>} when it failed).
 */
public final class LitematicaProbe implements ClientModInitializer {
    @Override
    public void onInitializeClient() {
        Thread t = new Thread(LitematicaProbe::loop, "mctas-litematica-probe");
        t.setDaemon(true);
        t.start();
    }

    private static final Path DIR = Paths.get("mctas-litematica");

    private static Map<String, String[]> config() {
        Map<String, String[]> m = new HashMap<>();
        try {
            Path cfg = DIR.resolve("config.txt");
            if (Files.exists(cfg)) {
                for (String line : Files.readAllLines(cfg, StandardCharsets.UTF_8)) {
                    String[] f = line.trim().split("\\s+");
                    if (f.length >= 2) {
                        m.put(f[0], f);
                    }
                }
            }
        } catch (Exception ignored) {
        }
        return m;
    }

    private static void loop() {
        Set<String> seen = new HashSet<>();
        String running = null;
        String[] job = null;
        // 0: idle; 1: placed, waiting for Litematica's schematic world; 2: pasting
        int phase = 0;
        int waited = 0;
        int quiet = 0;
        boolean creating = false;
        boolean ready = false;
        while (true) {
            try {
                Thread.sleep(100);
            } catch (InterruptedException e) {
                return;
            }
            Minecraft mc = Minecraft.getInstance();
            if (mc == null) {
                continue;
            }
            try {
                Map<String, String[]> cfg = config();
                boolean single = cfg.containsKey("world") && cfg.get("world")[1].equals("single");
                if (mc.player == null || mc.level == null) {
                    if (single && !creating && onClient(mc, () -> mc.gui != null && mc.gui.overlay() == null && mc.gui.screen() != null
                            && mc.gui.screen().getClass().getSimpleName().equals("TitleScreen"))) {
                        creating = true;
                        mc.execute(() -> createWorld(mc));
                        append(DIR.resolve("probe.log"), "creating a singleplayer world");
                    }
                    continue;
                }
                if (!ready) {
                    if (single) {
                        if (!mc.hasSingleplayerServer()) {
                            continue;
                        }
                        String[] c = cfg.get("center");
                        double[] center = c == null ? new double[] {0, 40, 0} : new double[] {Double.parseDouble(c[1]), Double.parseDouble(c[2]), Double.parseDouble(c[3])};
                        String err = onServer(mc, () -> setupSingle(mc, center));
                        append(DIR.resolve("probe.log"), "singleplayer world ready" + (err == null ? "" : ": " + err));
                        Thread.sleep(3000);
                    }
                    ready = true;
                    append(DIR.resolve("ready.txt"), "ready");
                    continue;
                }
                if (phase == 1) {
                    // the placement's chunks fill the schematic world (blocks, then entities) over
                    // a few ticks: paste after PLACE_WAIT polls
                    if (++waited < PLACE_WAIT) {
                        continue;
                    }
                    final String id = running;
                    String info = onClient(mc, () -> diagnostics());
                    append(DIR.resolve("probe.log"), id + " before paste: " + info);
                    String err = onClient(mc, () -> pasteCurrent(mc, cfg));
                    if (err != null) {
                        append(DIR.resolve("done.txt"), id + " error " + err);
                        running = null;
                        phase = 0;
                    } else {
                        phase = 2;
                        quiet = 0;
                    }
                    continue;
                }
                if (phase == 2) {
                    boolean busy = onClient(mc, () -> !TaskScheduler.getInstanceClient().getAllTasks().isEmpty()
                            || !TaskScheduler.getInstanceServer().getAllTasks().isEmpty());
                    // a few quiet polls in a row: the task may be added on the next tick
                    quiet = busy ? 0 : quiet + 1;
                    if (quiet >= 5) {
                        String result = "ok";
                        if (single && job.length >= 8) {
                            final String[] j = job;
                            String err = onServer(mc, () -> dump(mc, j));
                            if (err != null) {
                                result = "error dump: " + err;
                            }
                        }
                        append(DIR.resolve("done.txt"), running + " " + result);
                        running = null;
                        phase = 0;
                        quiet = 0;
                    }
                    continue;
                }
                Path jobs = DIR.resolve("jobs.txt");
                if (!Files.exists(jobs)) {
                    continue;
                }
                List<String> lines = Files.readAllLines(jobs, StandardCharsets.UTF_8);
                for (String line : lines) {
                    String[] f = line.trim().split("\\s+");
                    if (f.length < 5 || seen.contains(f[0])) {
                        continue;
                    }
                    seen.add(f[0]);
                    final Path file = Paths.get(f[1]).toAbsolutePath();
                    final BlockPos origin = new BlockPos(Integer.parseInt(f[2]), Integer.parseInt(f[3]), Integer.parseInt(f[4]));
                    final String[] fields = f;
                    String err = onClient(mc, () -> {
                        applySettings(cfg);
                        return place(file, origin);
                    });
                    if (err != null) {
                        append(DIR.resolve("done.txt"), fields[0] + " error " + err);
                    } else {
                        running = f[0];
                        job = f;
                        phase = 1;
                        waited = 0;
                    }
                    break;
                }
            } catch (Throwable e) {
                try {
                    append(DIR.resolve("probe.log"), e.toString());
                } catch (Exception ignored) {
                }
            }
        }
    }

    /** A flat creative world with commands (the player's own world: Litematica pastes into it directly). */
    private static void createWorld(Minecraft mc) {
        net.minecraft.world.level.LevelSettings settings = new net.minecraft.world.level.LevelSettings(
                "mctas", net.minecraft.world.level.GameType.CREATIVE, net.minecraft.world.level.LevelSettings.DifficultySettings.DEFAULT,
                true, net.minecraft.world.level.WorldDataConfiguration.DEFAULT);
        mc.createWorldOpenFlows().createFreshLevel(
                "mctas-" + System.currentTimeMillis(),
                settings,
                new net.minecraft.world.level.levelgen.WorldOptions(0L, false, false),
                registries -> registries.lookupOrThrow(net.minecraft.core.registries.Registries.WORLD_PRESET)
                        .getOrThrow(net.minecraft.world.level.levelgen.presets.WorldPresets.FLAT).value().createWorldDimensions(),
                null);
    }

    /** Freeze the game (nothing ticks but the player) and put the player, flying, above the grid. */
    private static String setupSingle(Minecraft mc, double[] center) {
        net.minecraft.client.server.IntegratedServer s = mc.getSingleplayerServer();
        s.getCommands().performPrefixedCommand(s.createCommandSourceStack(), "tick freeze");
        for (net.minecraft.server.level.ServerPlayer p : s.getPlayerList().getPlayers()) {
            p.getAbilities().flying = true;
            p.onUpdateAbilities();
            p.teleportTo(center[0], center[1], center[2]);
        }
        return null;
    }

    /** Polls (100 ms each) between the placement and the paste. */
    private static final int PLACE_WAIT = 30;
    private static SchematicPlacement current;
    private static BlockPos currentOrigin;
    private static int currentEntities;

    private static String place(Path file, BlockPos origin) {
        LitematicaSchematic s = LitematicaSchematic.createFromFile(file.getParent(), file.getFileName().toString());
        if (s == null) {
            return "Litematica could not load " + file;
        }
        int n = 0;
        for (String region : s.getAreas().keySet()) {
            List<?> l = s.getEntityListForRegion(region);
            n += l == null ? 0 : l.size();
        }
        SchematicPlacement p = SchematicPlacement.createFor(s, origin, file.getFileName().toString(), true, true);
        DataManager.getSchematicPlacementManager().addSchematicPlacement(p, false);
        DataManager.getSchematicPlacementManager().setSelectedSchematicPlacement(p);
        current = p;
        currentOrigin = origin;
        currentEntities = n;
        return null;
    }

    private static String diagnostics() {
        fi.dy.masa.litematica.world.WorldSchematic w = fi.dy.masa.litematica.world.SchematicWorldHandler.getSchematicWorld();
        int inWorld = -1;
        if (w != null && currentOrigin != null) {
            net.minecraft.world.phys.AABB bb = new net.minecraft.world.phys.AABB(currentOrigin).inflate(48);
            inWorld = w.getEntities((net.minecraft.world.entity.Entity) null, bb, e -> true).size();
        }
        return "entities in the file " + currentEntities + ", in the schematic world near the origin " + inWorld;
    }

    private static String pasteCurrent(Minecraft mc, Map<String, String[]> cfg) {
        if (current == null) {
            return "no placement";
        }
        applySettings(cfg);
        DataManager.getSchematicPlacementManager().pastePlacementToWorld(current, false, false, mc);
        return null;
    }

    /**
     * Litematica's own settings: {@code nbt <v>} = Generic > pasteNbtRestoreBehavior, {@code dedupe
     * <true|false>} = Generic > deduplicateSchematicEntities (off by default: when Litematica loads
     * a placement's chunk into its schematic world again, its entities are added again, and a
     * server paste summons every copy).
     */
    private static void applySettings(Map<String, String[]> cfg) {
        String[] nbt = cfg.get("nbt");
        if (nbt != null) {
            for (fi.dy.masa.litematica.util.PasteNbtBehavior b : fi.dy.masa.litematica.util.PasteNbtBehavior.VALUES) {
                if (b.getStringValue().equals(nbt[1])) {
                    fi.dy.masa.litematica.config.Configs.Generic.PASTE_NBT_BEHAVIOR.setOptionListValue(b);
                }
            }
        }
        String[] dedupe = cfg.get("dedupe");
        if (dedupe != null) {
            fi.dy.masa.litematica.config.Configs.Generic.DEDUPLICATE_SCHEMATIC_ENTITIES.setBooleanValue(dedupe[1].equals("true"));
        }
    }

    /** The job's box as the integrated server has it: every block, block entity and entity. */
    private static String dump(Minecraft mc, String[] job) throws Exception {
        net.minecraft.client.server.IntegratedServer s = mc.getSingleplayerServer();
        net.minecraft.server.level.ServerLevel level = s.overworld();
        int x0 = Integer.parseInt(job[2]), y0 = Integer.parseInt(job[3]), z0 = Integer.parseInt(job[4]);
        int x1 = Integer.parseInt(job[5]), y1 = Integer.parseInt(job[6]), z1 = Integer.parseInt(job[7]);
        List<String> out = new ArrayList<>();
        for (int y = y0; y <= y1; y++) {
            for (int z = z0; z <= z1; z++) {
                for (int x = x0; x <= x1; x++) {
                    BlockPos p = new BlockPos(x, y, z);
                    net.minecraft.world.level.block.state.BlockState st = level.getBlockState(p);
                    out.add("B " + x + " " + y + " " + z + " " + net.minecraft.commands.arguments.blocks.BlockStateParser.serialize(st));
                    net.minecraft.world.level.block.entity.BlockEntity be = level.getBlockEntity(p);
                    if (be != null) {
                        net.minecraft.nbt.CompoundTag t = be.saveWithoutMetadata(level.registryAccess());
                        String type = String.valueOf(net.minecraft.core.registries.BuiltInRegistries.BLOCK_ENTITY_TYPE.getKey(be.getType()));
                        out.add("T " + x + " " + y + " " + z + " " + type + " " + t);
                    }
                }
            }
        }
        net.minecraft.world.phys.AABB box = new net.minecraft.world.phys.AABB(x0, y0, z0, x1 + 1, y1 + 1, z1 + 1);
        for (net.minecraft.world.entity.Entity e : level.getEntities((net.minecraft.world.entity.Entity) null, box, e -> !(e instanceof net.minecraft.world.entity.player.Player))) {
            net.minecraft.world.level.storage.TagValueOutput o = net.minecraft.world.level.storage.TagValueOutput.createWithContext(
                    net.minecraft.util.ProblemReporter.DISCARDING, level.registryAccess());
            e.saveWithoutId(o);
            String type = String.valueOf(net.minecraft.world.entity.EntityType.getKey(e.getType()));
            out.add("N " + type + " " + e.getX() + " " + e.getY() + " " + e.getZ() + " " + o.buildResult());
        }
        Files.write(DIR.resolve("dump-" + job[0] + ".txt"), out, StandardCharsets.UTF_8);
        return null;
    }

    private static <T> T onClient(Minecraft mc, Supplier<T> f) throws Exception {
        return mc.submit(f).get();
    }

    private interface ServerWork {
        String run() throws Exception;
    }

    private static String onServer(Minecraft mc, ServerWork f) throws Exception {
        net.minecraft.client.server.IntegratedServer s = mc.getSingleplayerServer();
        return s.submit(() -> {
            try {
                return f.run();
            } catch (Exception e) {
                return e.toString();
            }
        }).get();
    }

    private static void append(Path file, String line) throws Exception {
        Files.createDirectories(file.getParent());
        Files.writeString(file, line + "\n", StandardCharsets.UTF_8, StandardOpenOption.CREATE, StandardOpenOption.APPEND);
    }
}
