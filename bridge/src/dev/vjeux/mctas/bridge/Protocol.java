package dev.vjeux.mctas.bridge;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.mojang.brigadier.exceptions.CommandSyntaxException;
import java.io.BufferedOutputStream;
import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Base64;
import java.util.HashMap;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.concurrent.TimeUnit;
import net.minecraft.SharedConstants;
import net.minecraft.commands.CommandSourceStack;
import net.minecraft.commands.arguments.blocks.BlockStateParser;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.piston.PistonMovingBlockEntity;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.phys.Vec3;

/**
 * Line-delimited JSON protocol. Requests are read on a background thread and executed in order on
 * the server thread; each request gets exactly one response line carrying the same {@code id}.
 *
 * <p>See docs/protocol.md for the full reference.
 */
public final class Protocol {
    private static final JsonObject EOF = new JsonObject();
    private final InputStream in;
    private final OutputStream out;
    private final LinkedBlockingQueue<JsonObject> queue = new LinkedBlockingQueue<>();
    private final Gson gson = new GsonBuilder().disableHtmlEscaping().create();
    private final Map<Integer, Sandbox> sandboxes = new HashMap<>();
    private final Map<String, BlockState> parsedStates = new HashMap<>();
    private final Map<BlockState, String> serializedStates = new IdentityHashMap<>();
    private SimServer server;
    private HolderLookup<Block> blockLookup;
    private int nextSandboxId = 1;

    Protocol(InputStream in, OutputStream out) {
        this.in = in;
        this.out = new BufferedOutputStream(out, 1 << 16);
    }

    /** Called on the server thread once the levels exist. */
    void start(SimServer server) {
        this.server = server;
        this.blockLookup = server.registryAccess().lookupOrThrow(Registries.BLOCK);
        Thread reader = new Thread(this::readLoop, "mctas-protocol-reader");
        reader.setDaemon(true);
        reader.start();
        JsonObject ready = new JsonObject();
        ready.addProperty("event", "ready");
        ready.addProperty("version", SharedConstants.getCurrentVersion().name());
        write(ready);
        flush();
    }

    private void readLoop() {
        try (BufferedReader reader = new BufferedReader(new InputStreamReader(this.in, StandardCharsets.UTF_8), 1 << 16)) {
            String line;
            while ((line = reader.readLine()) != null) {
                if (line.isBlank()) {
                    continue;
                }
                JsonObject request;
                try {
                    request = JsonParser.parseString(line).getAsJsonObject();
                } catch (RuntimeException e) {
                    request = new JsonObject();
                    request.addProperty("op", "__invalid");
                    request.addProperty("error", "invalid JSON: " + e.getMessage());
                }
                this.queue.put(request);
            }
        } catch (IOException | InterruptedException e) {
            // fall through: treat as EOF
        }
        this.queue.add(EOF);
    }

    /** Runs on the server thread: wait briefly for work, then drain the queue. */
    void serve() {
        JsonObject request;
        try {
            this.server.viewerTick();
            request = this.queue.poll(this.server.listening() ? 50 : 200, TimeUnit.MILLISECONDS);
        } catch (InterruptedException e) {
            return;
        }
        while (request != null) {
            if (request == EOF) {
                flush();
                Runtime.getRuntime().halt(0);
            }
            write(handle(request));
            request = this.queue.poll();
        }
        flush();
    }

    private JsonObject handle(JsonObject request) {
        JsonObject response = new JsonObject();
        if (request.has("id")) {
            response.add("id", request.get("id"));
        }
        try {
            String op = request.has("op") ? request.get("op").getAsString() : "";
            JsonObject result = dispatch(op, request);
            response.addProperty("ok", true);
            for (Map.Entry<String, JsonElement> e : result.entrySet()) {
                response.add(e.getKey(), e.getValue());
            }
        } catch (Throwable t) {
            response.addProperty("ok", false);
            String message = t.getMessage() == null ? t.toString() : t.getClass().getSimpleName() + ": " + t.getMessage();
            response.addProperty("error", message);
        }
        return response;
    }

    private JsonObject dispatch(String op, JsonObject r) throws Exception {
        return switch (op) {
            case "ping" -> new JsonObject();
            case "info" -> info();
            case "create" -> create(r);
            case "drop" -> drop(r);
            case "clear" -> clear(r);
            case "set" -> set(r);
            case "fill" -> fill(r);
            case "get" -> get(r);
            case "use" -> use(r);
            case "step" -> step(r);
            case "status" -> status(sandbox(r));
            case "entities" -> entities(r);
            case "update" -> update(r);
            case "exec" -> exec(r);
            case "viewers" -> viewers();
            case "camera" -> camera(r);
            case "play" -> play(r);
            case "hold" -> hold(r);
            case "__invalid" -> throw new IllegalArgumentException(r.get("error").getAsString());
            default -> throw new IllegalArgumentException("unknown op '" + op + "'");
        };
    }

    // ---- ops -------------------------------------------------------------------------------

    private JsonObject info() {
        JsonObject o = new JsonObject();
        o.addProperty("version", SharedConstants.getCurrentVersion().name());
        o.addProperty("sandboxes", this.sandboxes.size());
        Runtime rt = Runtime.getRuntime();
        o.addProperty("heap_used", rt.totalMemory() - rt.freeMemory());
        o.addProperty("heap_max", rt.maxMemory());
        return o;
    }

    private JsonObject create(JsonObject r) {
        int minCX = -1, minCZ = -1, maxCX = 0, maxCZ = 0;
        if (r.has("chunks")) {
            JsonArray c = r.getAsJsonArray("chunks");
            minCX = c.get(0).getAsInt();
            minCZ = c.get(1).getAsInt();
            maxCX = c.get(2).getAsInt();
            maxCZ = c.get(3).getAsInt();
        }
        long seed = r.has("seed") ? r.get("seed").getAsLong() : 0L;
        int id = this.nextSandboxId++;
        Sandbox sb = Sandbox.create(this.server, id, minCX, minCZ, maxCX, maxCZ, seed);
        this.sandboxes.put(id, sb);
        JsonObject o = new JsonObject();
        o.addProperty("sandbox", id);
        o.add("min", pos(sb.minX(), sb.level.getMinY(), sb.minZ()));
        o.add("max", pos(sb.maxX(), sb.level.getMaxY(), sb.maxZ()));
        return o;
    }

    private JsonObject drop(JsonObject r) {
        Sandbox sb = sandbox(r);
        sb.drop(this.server);
        this.sandboxes.remove(sb.id);
        return new JsonObject();
    }

    private JsonObject clear(JsonObject r) {
        sandbox(r).clear();
        return new JsonObject();
    }

    private static int flags(JsonObject r) {
        if (!r.has("mode")) {
            return Sandbox.RAW_FLAGS;
        }
        JsonElement mode = r.get("mode");
        if (mode.isJsonPrimitive() && mode.getAsJsonPrimitive().isNumber()) {
            return mode.getAsInt();
        }
        return switch (mode.getAsString()) {
            case "raw" -> Sandbox.RAW_FLAGS;
            case "update" -> Block.UPDATE_ALL;
            default -> throw new IllegalArgumentException("unknown mode " + mode);
        };
    }

    private JsonObject set(JsonObject r) throws CommandSyntaxException {
        Sandbox sb = sandbox(r);
        int flags = flags(r);
        int changed = 0;
        BlockPos.MutableBlockPos pos = new BlockPos.MutableBlockPos();
        for (JsonElement e : r.getAsJsonArray("blocks")) {
            JsonArray b = e.getAsJsonArray();
            pos.set(b.get(0).getAsInt(), b.get(1).getAsInt(), b.get(2).getAsInt());
            checkInside(sb, pos);
            if (sb.level.setBlock(pos, parse(b.get(3).getAsString()), flags)) {
                changed++;
            }
        }
        JsonObject o = new JsonObject();
        o.addProperty("changed", changed);
        return o;
    }

    private JsonObject fill(JsonObject r) throws CommandSyntaxException {
        Sandbox sb = sandbox(r);
        int flags = flags(r);
        int[] a = ints(r.getAsJsonArray("from"));
        int[] b = ints(r.getAsJsonArray("to"));
        BlockState state = parse(r.get("state").getAsString());
        int changed = 0;
        BlockPos.MutableBlockPos pos = new BlockPos.MutableBlockPos();
        for (int y = Math.min(a[1], b[1]); y <= Math.max(a[1], b[1]); y++) {
            for (int z = Math.min(a[2], b[2]); z <= Math.max(a[2], b[2]); z++) {
                for (int x = Math.min(a[0], b[0]); x <= Math.max(a[0], b[0]); x++) {
                    pos.set(x, y, z);
                    checkInside(sb, pos);
                    if (sb.level.setBlock(pos, state, flags)) {
                        changed++;
                    }
                }
            }
        }
        JsonObject o = new JsonObject();
        o.addProperty("changed", changed);
        return o;
    }

    /** Region read: palette + one u16 (little endian, base64) per block, x fastest, then z, then y. */
    private JsonObject get(JsonObject r) {
        Sandbox sb = sandbox(r);
        int[] a = ints(r.getAsJsonArray("from"));
        int[] b = ints(r.getAsJsonArray("to"));
        Region region = new Region(a, b);
        Palette palette = new Palette();
        JsonObject o = new JsonObject();
        o.add("from", pos(region.x0, region.y0, region.z0));
        o.add("size", pos(region.dx, region.dy, region.dz));
        o.addProperty("data", region.capture(sb, palette));
        o.add("palette", palette.toJson());
        o.add("flags", palette.flagsJson(sb));
        o.addProperty("time", sb.gameTime());
        if (r.has("moving") && r.get("moving").getAsBoolean()) {
            o.add("moving", moving(sb, region));
        }
        return o;
    }

    private JsonObject use(JsonObject r) {
        Sandbox sb = sandbox(r);
        int[] p = ints(r.getAsJsonArray("pos"));
        Direction face = r.has("face") ? Direction.byName(r.get("face").getAsString()) : Direction.UP;
        if (face == null) {
            throw new IllegalArgumentException("bad face " + r.get("face"));
        }
        JsonObject o = new JsonObject();
        o.addProperty("result", sb.use(new BlockPos(p[0], p[1], p[2]), face));
        return o;
    }

    /**
     * Advance sandboxes. {@code ticks} is the count (or the cap with {@code until_idle}); with
     * {@code record} the given region is captured after every tick.
     */
    private JsonObject step(JsonObject r) {
        List<Sandbox> targets = new ArrayList<>();
        if (r.has("sandboxes")) {
            for (JsonElement e : r.getAsJsonArray("sandboxes")) {
                targets.add(sandboxById(e.getAsInt()));
            }
        } else {
            targets.add(sandbox(r));
        }
        int ticks = r.has("ticks") ? r.get("ticks").getAsInt() : 1;
        boolean untilIdle = r.has("until_idle") && r.get("until_idle").getAsBoolean();
        Region region = null;
        if (r.has("record")) {
            JsonObject rec = r.getAsJsonObject("record");
            region = new Region(ints(rec.getAsJsonArray("from")), ints(rec.getAsJsonArray("to")));
        }
        JsonArray results = new JsonArray();
        for (Sandbox sb : targets) {
            Palette palette = region == null ? null : new Palette();
            JsonArray frames = region == null ? null : new JsonArray();
            int ticked = 0;
            boolean idle = false;
            while (ticked < ticks) {
                sb.tick(this.server);
                ticked++;
                if (frames != null) {
                    frames.add(region.capture(sb, palette));
                }
                if (untilIdle && sb.idle()) {
                    idle = true;
                    break;
                }
            }
            JsonObject res = new JsonObject();
            res.addProperty("sandbox", sb.id);
            res.addProperty("ticked", ticked);
            res.addProperty("time", sb.gameTime());
            if (untilIdle) {
                res.addProperty("idle", idle);
            }
            if (frames != null) {
                res.add("from", pos(region.x0, region.y0, region.z0));
                res.add("size", pos(region.dx, region.dy, region.dz));
                res.add("palette", palette.toJson());
                res.add("flags", palette.flagsJson(sb));
                res.add("frames", frames);
            }
            results.add(res);
        }
        JsonObject o = new JsonObject();
        o.add("results", results);
        return o;
    }

    private JsonObject status(Sandbox sb) {
        JsonObject o = new JsonObject();
        o.addProperty("time", sb.gameTime());
        o.addProperty("block_ticks", sb.level.getBlockTicks().count());
        o.addProperty("fluid_ticks", sb.level.getFluidTicks().count());
        o.addProperty("block_events", sb.blockEventCount());
        o.addProperty("moving_pistons", sb.movingPistonCount());
        o.addProperty("entities", sb.entities().size());
        o.addProperty("idle", sb.idle());
        return o;
    }

    private JsonObject entities(JsonObject r) {
        Sandbox sb = sandbox(r);
        JsonArray list = new JsonArray();
        for (Entity e : sb.entities()) {
            JsonObject j = new JsonObject();
            j.addProperty("type", BuiltInRegistries.ENTITY_TYPE.getKey(e.getType()).toString());
            Vec3 p = e.position();
            JsonArray a = new JsonArray();
            a.add(p.x);
            a.add(p.y);
            a.add(p.z);
            j.add("pos", a);
            j.addProperty("uuid", e.getStringUUID());
            list.add(j);
        }
        JsonObject o = new JsonObject();
        o.add("entities", list);
        return o;
    }

    /**
     * Manual updates. kind=neighbors: notify the 6 neighbors of each position (like a block
     * change there); kind=self: the block at each position re-checks its neighbors; kind=shape:
     * recompute the block's shape-dependent state (wire connections, fences...) without updates.
     */
    private JsonObject update(JsonObject r) {
        Sandbox sb = sandbox(r);
        String kind = r.has("kind") ? r.get("kind").getAsString() : "neighbors";
        int changed = 0;
        for (JsonElement e : r.getAsJsonArray("positions")) {
            int[] p = ints(e.getAsJsonArray());
            BlockPos pos = new BlockPos(p[0], p[1], p[2]);
            BlockState state = sb.level.getBlockState(pos);
            switch (kind) {
                case "neighbors" -> sb.level.updateNeighborsAt(pos, state.getBlock());
                case "self" -> sb.level.neighborChanged(pos, state.getBlock(), null);
                case "shape" -> {
                    BlockState updated = Block.updateFromNeighbourShapes(state, sb.level, pos);
                    if (updated != state) {
                        sb.level.setBlock(pos, updated, Sandbox.RAW_FLAGS);
                        changed++;
                    }
                }
                default -> throw new IllegalArgumentException("unknown update kind " + kind);
            }
        }
        JsonObject o = new JsonObject();
        o.addProperty("changed", changed);
        return o;
    }

    private JsonObject exec(JsonObject r) {
        Sandbox sb = sandbox(r);
        CommandSourceStack source = this.server.createCommandSourceStack()
                .withLevel(sb.level)
                .withPosition(Vec3.ZERO)
                .withSuppressedOutput();
        this.server.getCommands().performPrefixedCommand(source, r.get("command").getAsString());
        return new JsonObject();
    }

    private JsonObject viewers() {
        JsonObject o = new JsonObject();
        JsonArray names = new JsonArray();
        for (net.minecraft.server.level.ServerPlayer p : this.server.getPlayerList().getPlayers()) {
            names.add(p.getGameProfile().name());
        }
        o.addProperty("count", names.size());
        o.add("names", names);
        return o;
    }

    /** Put every connected viewer in spectator mode at a camera pose inside a sandbox. */
    private JsonObject camera(JsonObject r) {
        Sandbox sb = sandbox(r);
        JsonArray p = r.getAsJsonArray("pos");
        float yaw = r.has("yaw") ? r.get("yaw").getAsFloat() : 0F;
        float pitch = r.has("pitch") ? r.get("pitch").getAsFloat() : 0F;
        int n = 0;
        for (net.minecraft.server.level.ServerPlayer player : this.server.getPlayerList().getPlayers()) {
            player.setGameMode(net.minecraft.world.level.GameType.SPECTATOR);
            player.teleportTo(sb.level, p.get(0).getAsDouble(), p.get(1).getAsDouble(), p.get(2).getAsDouble(), java.util.Set.of(), yaw, pitch, true);
            n++;
        }
        JsonObject o = new JsonObject();
        o.addProperty("moved", n);
        return o;
    }

    /** Step a sandbox at a wall-clock pace (for viewers), with network work between ticks. */
    private JsonObject play(JsonObject r) throws InterruptedException {
        Sandbox sb = sandbox(r);
        int ticks = r.get("ticks").getAsInt();
        double tps = r.has("tps") ? r.get("tps").getAsDouble() : 20.0;
        long period = (long) (1_000_000_000L / tps);
        long next = System.nanoTime();
        for (int i = 0; i < ticks; i++) {
            sb.tick(this.server);
            this.server.viewerTick();
            next += period;
            long sleep = next - System.nanoTime();
            if (sleep > 0) {
                Thread.sleep(sleep / 1_000_000L, (int) (sleep % 1_000_000L));
            }
        }
        JsonObject o = new JsonObject();
        o.addProperty("time", sb.gameTime());
        return o;
    }

    /** Keep viewers served for a while without ticking any level. */
    private JsonObject hold(JsonObject r) throws InterruptedException {
        long until = System.nanoTime() + (long) (r.get("ms").getAsDouble() * 1_000_000L);
        while (System.nanoTime() < until) {
            this.server.viewerTick();
            Thread.sleep(25);
        }
        return new JsonObject();
    }

    // ---- helpers -----------------------------------------------------------------------------

    private JsonArray moving(Sandbox sb, Region region) {
        JsonArray list = new JsonArray();
        BlockPos.MutableBlockPos pos = new BlockPos.MutableBlockPos();
        for (int y = region.y0; y < region.y0 + region.dy; y++) {
            for (int z = region.z0; z < region.z0 + region.dz; z++) {
                for (int x = region.x0; x < region.x0 + region.dx; x++) {
                    pos.set(x, y, z);
                    BlockEntity be = sb.level.getBlockEntity(pos);
                    if (be instanceof PistonMovingBlockEntity m) {
                        JsonObject j = new JsonObject();
                        j.add("pos", pos(x, y, z));
                        j.addProperty("state", serialize(m.getMovedState()));
                        j.addProperty("extending", m.isExtending());
                        j.addProperty("source", m.isSourcePiston());
                        j.addProperty("direction", m.getDirection().getSerializedName());
                        j.addProperty("progress", m.getProgress(0.0F));
                        list.add(j);
                    }
                }
            }
        }
        return list;
    }

    private Sandbox sandbox(JsonObject r) {
        if (!r.has("sandbox")) {
            throw new IllegalArgumentException("missing 'sandbox'");
        }
        return sandboxById(r.get("sandbox").getAsInt());
    }

    private Sandbox sandboxById(int id) {
        Sandbox sb = this.sandboxes.get(id);
        if (sb == null) {
            throw new IllegalArgumentException("no sandbox " + id);
        }
        return sb;
    }

    private static void checkInside(Sandbox sb, BlockPos pos) {
        if (pos.getX() < sb.minX() || pos.getX() > sb.maxX() || pos.getZ() < sb.minZ() || pos.getZ() > sb.maxZ()
                || pos.getY() < sb.level.getMinY() || pos.getY() > sb.level.getMaxY()) {
            throw new IllegalArgumentException("position " + pos.toShortString() + " is outside sandbox " + sb.id);
        }
    }

    BlockState parse(String s) throws CommandSyntaxException {
        BlockState state = this.parsedStates.get(s);
        if (state == null) {
            state = BlockStateParser.parseForBlock(this.blockLookup, s, false).blockState();
            this.parsedStates.put(s, state);
        }
        return state;
    }

    String serialize(BlockState state) {
        return this.serializedStates.computeIfAbsent(state, BlockStateParser::serialize);
    }

    private static int[] ints(JsonArray a) {
        int[] out = new int[a.size()];
        for (int i = 0; i < out.length; i++) {
            out[i] = a.get(i).getAsInt();
        }
        return out;
    }

    private static JsonArray pos(int x, int y, int z) {
        JsonArray a = new JsonArray();
        a.add(x);
        a.add(y);
        a.add(z);
        return a;
    }

    private void write(JsonObject o) {
        try {
            this.out.write(this.gson.toJson(o).getBytes(StandardCharsets.UTF_8));
            this.out.write('\n');
        } catch (IOException e) {
            Runtime.getRuntime().halt(0);
        }
    }

    private void flush() {
        try {
            this.out.flush();
        } catch (IOException e) {
            Runtime.getRuntime().halt(0);
        }
    }

    /** Block states seen in one response, in first-seen order. */
    final class Palette {
        private final Map<BlockState, Integer> index = new IdentityHashMap<>();
        private final List<BlockState> states = new ArrayList<>();

        int id(BlockState state) {
            Integer i = this.index.get(state);
            if (i == null) {
                i = this.states.size();
                this.index.put(state, i);
                this.states.add(state);
            }
            return i;
        }

        JsonArray toJson() {
            JsonArray a = new JsonArray();
            for (BlockState s : this.states) {
                a.add(serialize(s));
            }
            return a;
        }

        /** Per palette entry: does it collide at all, and is its collision a full cube. */
        JsonArray flagsJson(Sandbox sb) {
            JsonArray a = new JsonArray();
            for (BlockState s : this.states) {
                JsonObject f = new JsonObject();
                f.addProperty("collides", !s.getCollisionShape(sb.level, BlockPos.ZERO).isEmpty());
                f.addProperty("full", s.isCollisionShapeFullBlock(sb.level, BlockPos.ZERO));
                a.add(f);
            }
            return a;
        }
    }

    static final class Region {
        final int x0, y0, z0, dx, dy, dz;

        Region(int[] a, int[] b) {
            this.x0 = Math.min(a[0], b[0]);
            this.y0 = Math.min(a[1], b[1]);
            this.z0 = Math.min(a[2], b[2]);
            this.dx = Math.abs(a[0] - b[0]) + 1;
            this.dy = Math.abs(a[1] - b[1]) + 1;
            this.dz = Math.abs(a[2] - b[2]) + 1;
            if ((long) this.dx * this.dy * this.dz > 16_000_000L) {
                throw new IllegalArgumentException("region too large");
            }
        }

        String capture(Sandbox sb, Palette palette) {
            byte[] bytes = new byte[this.dx * this.dy * this.dz * 2];
            BlockPos.MutableBlockPos pos = new BlockPos.MutableBlockPos();
            int i = 0;
            for (int y = this.y0; y < this.y0 + this.dy; y++) {
                for (int z = this.z0; z < this.z0 + this.dz; z++) {
                    for (int x = this.x0; x < this.x0 + this.dx; x++) {
                        int id = palette.id(sb.level.getBlockState(pos.set(x, y, z)));
                        bytes[i++] = (byte) id;
                        bytes[i++] = (byte) (id >> 8);
                    }
                }
            }
            return Base64.getEncoder().encodeToString(bytes);
        }
    }
}
