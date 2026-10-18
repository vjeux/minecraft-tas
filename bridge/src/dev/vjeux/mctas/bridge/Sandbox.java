package dev.vjeux.mctas.bridge;

import com.mojang.authlib.GameProfile;
import com.mojang.serialization.Lifecycle;
import java.io.IOException;
import java.lang.reflect.Field;
import java.util.ArrayList;
import java.util.Collection;
import java.util.List;
import java.util.UUID;
import java.util.concurrent.locks.LockSupport;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.util.Util;
import net.minecraft.world.InteractionResult;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.item.FallingBlockEntity;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.level.GameType;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.biome.BiomeManager;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.ButtonBlock;
import net.minecraft.world.level.block.LeverBlock;
import net.minecraft.world.level.block.entity.TickingBlockEntity;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.chunk.LevelChunk;
import net.minecraft.world.level.entity.PersistentEntitySectionManager;
import net.minecraft.world.level.chunk.LevelChunkSection;
import net.minecraft.world.level.levelgen.structure.BoundingBox;
import net.minecraft.world.level.storage.PrimaryLevelData;
import net.minecraft.world.phys.BlockHitResult;
import net.minecraft.world.phys.Vec3;

/**
 * One isolated simulation: its own {@link ServerLevel} with a void generator, a fixed set of
 * force-loaded chunks, its own game time (so scheduled ticks fire on its own clock) and its own
 * seeded random. Nothing outside {@link #tick()} ever advances it.
 */
final class Sandbox {
    /** setBlock flags for "raw" placement: no neighbor updates, no shape updates, no onPlace, no drops. */
    static final int RAW_FLAGS = Block.UPDATE_CLIENTS
            | Block.UPDATE_KNOWN_SHAPE
            | Block.UPDATE_SUPPRESS_DROPS
            | Block.UPDATE_SKIP_BLOCK_ENTITY_SIDEEFFECTS
            | Block.UPDATE_SKIP_ON_PLACE;

    private static final Field BLOCK_EVENTS = field(ServerLevel.class, "blockEvents");
    private static final Field BLOCK_ENTITY_TICKERS = field(Level.class, "blockEntityTickers");
    private static final Field PENDING_BLOCK_ENTITY_TICKERS = field(Level.class, "pendingBlockEntityTickers");
    private static final Field ENTITY_MANAGER = field(ServerLevel.class, "entityManager");

    final int id;
    final ServerLevel level;
    final PrimaryLevelData data;
    final int minChunkX;
    final int minChunkZ;
    final int maxChunkX;
    final int maxChunkZ;
    final long seed;
    private Player mockPlayer;

    private Sandbox(int id, ServerLevel level, PrimaryLevelData data, int minChunkX, int minChunkZ, int maxChunkX, int maxChunkZ, long seed) {
        this.id = id;
        this.level = level;
        this.data = data;
        this.minChunkX = minChunkX;
        this.minChunkZ = minChunkZ;
        this.maxChunkX = maxChunkX;
        this.maxChunkZ = maxChunkZ;
        this.seed = seed;
    }

    static Sandbox create(SimServer server, int id, int minChunkX, int minChunkZ, int maxChunkX, int maxChunkZ, long seed) {
        ResourceKey<Level> key = ResourceKey.create(Registries.DIMENSION, Identifier.fromNamespaceAndPath("mctas", "sandbox_" + id));
        PrimaryLevelData data = new PrimaryLevelData(server.levelSettings, PrimaryLevelData.SpecialWorldProperty.FLAT, Lifecycle.stable());
        data.setInitialized(true);
        ServerLevel level = new ServerLevel(
                server,
                Util.backgroundExecutor(),
                server.storage(),
                data,
                key,
                server.voidStem,
                false,
                BiomeManager.obfuscateSeed(seed),
                List.of(),
                true);
        level.noSave = true;
        server.registerLevel(level);
        for (int cx = minChunkX; cx <= maxChunkX; cx++) {
            for (int cz = minChunkZ; cz <= maxChunkZ; cz++) {
                level.setChunkForced(cx, cz, true);
            }
        }
        // Block entities (moving pistons) and scheduled block ticks only run in chunks whose
        // entity sections are loaded. That load is asynchronous (region-file IO, even for an
        // empty world), so pump it here; otherwise the first ticks of a fresh sandbox would
        // silently skip block entities.
        PersistentEntitySectionManager<?> entityManager = (PersistentEntitySectionManager<?>) get(ENTITY_MANAGER, level);
        long deadline = System.nanoTime() + 30_000_000_000L;
        while (!entitiesLoaded(level, minChunkX, minChunkZ, maxChunkX, maxChunkZ)) {
            server.pumpTasks();
            level.getChunkSource().pollTask();
            entityManager.tick();
            if (System.nanoTime() > deadline) {
                throw new IllegalStateException("entity sections of sandbox " + id + " did not load");
            }
            LockSupport.parkNanos(20_000L);
        }
        level.getRandom().setSeed(seed);
        return new Sandbox(id, level, data, minChunkX, minChunkZ, maxChunkX, maxChunkZ, seed);
    }

    private static boolean entitiesLoaded(ServerLevel level, int minChunkX, int minChunkZ, int maxChunkX, int maxChunkZ) {
        for (int cx = minChunkX; cx <= maxChunkX; cx++) {
            for (int cz = minChunkZ; cz <= maxChunkZ; cz++) {
                if (!level.areEntitiesLoaded(ChunkPos.pack(cx, cz))) {
                    return false;
                }
            }
        }
        return true;
    }

    int minX() {
        return this.minChunkX << 4;
    }

    int minZ() {
        return this.minChunkZ << 4;
    }

    int maxX() {
        return (this.maxChunkX << 4) + 15;
    }

    int maxZ() {
        return (this.maxChunkZ << 4) + 15;
    }

    BoundingBox bounds() {
        return new BoundingBox(minX(), this.level.getMinY(), minZ(), maxX(), this.level.getMaxY(), maxZ());
    }

    /**
     * One game tick of this sandbox, then the work vanilla does between two ticks: queued server
     * tasks and this level's chunk-system tasks (a real server finishes those during the 50 ms
     * gap, so they are always done before the next tick).
     */
    void tick(SimServer server) {
        this.level.tick(() -> true);
        server.pumpTasks();
        while (this.level.getChunkSource().pollTask()) {
            // drain
        }
    }

    long gameTime() {
        return this.level.getGameTime();
    }

    int blockEventCount() {
        return ((Collection<?>) get(BLOCK_EVENTS, this.level)).size();
    }

    @SuppressWarnings("unchecked")
    int movingPistonCount() {
        int count = 0;
        for (Field f : new Field[] {BLOCK_ENTITY_TICKERS, PENDING_BLOCK_ENTITY_TICKERS}) {
            for (TickingBlockEntity ticker : (List<TickingBlockEntity>) get(f, this.level)) {
                if (!ticker.isRemoved() && ticker.getType().equals("minecraft:piston")) {
                    count++;
                }
            }
        }
        return count;
    }

    /**
     * True when nothing is pending: no scheduled block/fluid ticks, no block events, no moving
     * pistons, and no falling blocks still in the air (sand/gravel/concrete powder falling is a
     * contraption still moving; it ends by landing as a block or breaking into an item).
     */
    boolean idle() {
        return this.level.getBlockTicks().count() == 0
                && this.level.getFluidTicks().count() == 0
                && blockEventCount() == 0
                && movingPistonCount() == 0
                && fallingBlockCount() == 0;
    }

    int fallingBlockCount() {
        int count = 0;
        for (Entity e : this.level.getAllEntities()) {
            if (e instanceof FallingBlockEntity && !e.isRemoved()) {
                count++;
            }
        }
        return count;
    }

    List<Entity> entities() {
        List<Entity> out = new ArrayList<>();
        for (Entity e : this.level.getAllEntities()) {
            if (e != null && !e.isRemoved()) {
                out.add(e);
            }
        }
        return out;
    }

    /** Back to an empty void: no blocks, entities, pending ticks or block events; time 0; reseeded. */
    /**
     * Redstone torch burnout history is a static map keyed by level (RedstoneTorchBlock
     * .RECENT_TOGGLES). clear() resets this level's game time, so entries stamped later than the
     * new time would never expire and would fake burnouts in the next run: forget them.
     */
    private void forgetTorchToggles() {
        try {
            java.lang.reflect.Field f = net.minecraft.world.level.block.RedstoneTorchBlock.class.getDeclaredField("RECENT_TOGGLES");
            f.setAccessible(true);
            ((java.util.Map<?, ?>) f.get(null)).remove(this.level);
        } catch (ReflectiveOperationException e) {
            throw new RuntimeException(e);
        }
    }

    void clear() {
        forgetTorchToggles();
        for (Entity e : entities()) {
            e.discard();
        }
        BoundingBox bb = bounds();
        this.level.clearBlockEvents(bb);
        this.level.getBlockTicks().clearArea(bb);
        this.level.getFluidTicks().clearArea(bb);
        BlockState air = Blocks.AIR.defaultBlockState();
        BlockPos.MutableBlockPos pos = new BlockPos.MutableBlockPos();
        // Include the ring of neighbor chunks: a contraption can push blocks one chunk out.
        for (int cx = this.minChunkX - 1; cx <= this.maxChunkX + 1; cx++) {
            for (int cz = this.minChunkZ - 1; cz <= this.maxChunkZ + 1; cz++) {
                LevelChunk chunk = this.level.getChunkSource().getChunkNow(cx, cz);
                if (chunk == null) {
                    continue;
                }
                LevelChunkSection[] sections = chunk.getSections();
                for (int si = 0; si < sections.length; si++) {
                    LevelChunkSection section = sections[si];
                    if (section.hasOnlyAir()) {
                        continue;
                    }
                    int baseY = chunk.getSectionYFromSectionIndex(si) << 4;
                    for (int y = 0; y < 16; y++) {
                        for (int z = 0; z < 16; z++) {
                            for (int x = 0; x < 16; x++) {
                                if (!section.getBlockState(x, y, z).isAir()) {
                                    pos.set((cx << 4) + x, baseY + y, (cz << 4) + z);
                                    this.level.setBlock(pos, air, RAW_FLAGS);
                                }
                            }
                        }
                    }
                }
            }
        }
        this.data.setGameTime(0L);
        this.level.getRandom().setSeed(this.seed);
    }

    void drop(SimServer server) {
        clear();
        for (int cx = this.minChunkX; cx <= this.maxChunkX; cx++) {
            for (int cz = this.minChunkZ; cz <= this.maxChunkZ; cz++) {
                this.level.setChunkForced(cx, cz, false);
            }
        }
        server.unregisterLevel(this.level);
        try {
            this.level.close();
        } catch (IOException e) {
            throw new IllegalStateException(e);
        }
    }

    /** Interact with a block the way a player's right click would (levers and buttons directly). */
    String use(BlockPos pos, Direction face) {
        BlockState state = this.level.getBlockState(pos);
        Block block = state.getBlock();
        if (block instanceof LeverBlock lever) {
            lever.pull(state, this.level, pos, null);
            return "lever";
        }
        if (block instanceof ButtonBlock button) {
            button.press(state, this.level, pos, null);
            return "button";
        }
        BlockHitResult hit = new BlockHitResult(Vec3.atCenterOf(pos), face, pos, false);
        InteractionResult result = state.useWithoutItem(this.level, mockPlayer(), hit);
        return result.toString();
    }

    // ---- survival building (op "build") -------------------------------------------------------

    private static final java.lang.reflect.Method GET_PLACEMENT_STATE = method(
            net.minecraft.world.item.BlockItem.class, "getPlacementState", net.minecraft.world.item.context.BlockPlaceContext.class);
    private static final float[] PITCHES = {0F, 90F, -90F, 30F, -30F, 60F, -60F};
    /** Cardinal look directions first, then 20 degrees off each (a face seen at an angle). */
    private static final float[] YAWS = {0F, 90F, 180F, 270F, 20F, 340F, 70F, 110F, 160F, 200F, 250F, 290F};

    /**
     * Properties the world computes (power, connections, piston extension...): a player placing the
     * block cannot pick them, they follow from the neighbours.
     */
    static boolean dynamicProperty(BlockState state, String name) {
        Block b = state.getBlock();
        return switch (name) {
            case "power", "extended", "lit", "locked", "triggered", "enabled", "sculk_sensor_phase", "instrument", "short",
                    "attached", "disarmed", "in_wall", "has_book", "occupied", "unstable", "bottom", "distance" -> true;
            case "powered" -> !(b instanceof LeverBlock);
            case "north", "south", "east", "west", "up" -> b instanceof net.minecraft.world.level.block.RedstoneWireBlock
                    || b instanceof net.minecraft.world.level.block.CrossCollisionBlock
                    || b instanceof net.minecraft.world.level.block.WallBlock;
            default -> false;
        };
    }

    /** Properties a player sets by right-clicking the placed block (repeater delay, lever...). */
    static boolean usableProperty(BlockState state, String name) {
        Block b = state.getBlock();
        return (name.equals("delay") && b instanceof net.minecraft.world.level.block.RepeaterBlock)
                || (name.equals("mode") && b instanceof net.minecraft.world.level.block.ComparatorBlock)
                || (name.equals("powered") && b instanceof LeverBlock)
                || (name.equals("note") && b instanceof net.minecraft.world.level.block.NoteBlock)
                || (name.equals("inverted") && b instanceof net.minecraft.world.level.block.DaylightDetectorBlock)
                || name.equals("open");
    }

    /**
     * Can a player looking along (yaw, pitch) click `face` (the face of the support block toward
     * `pos`) at `hit`? The view ray must travel into the face, and it reaches it through the
     * target cell: the cell it comes from (where the ray enters `pos`, walking back from the hit)
     * must be open, or the face is out of sight.
     */
    private boolean visible(BlockPos pos, Direction face, Vec3 hit, float yaw, float pitch) {
        double yr = Math.toRadians(yaw);
        double pr = Math.toRadians(pitch);
        double[] v = {-Math.sin(yr) * Math.cos(pr), -Math.sin(pr), Math.cos(yr) * Math.cos(pr)};
        if (v[0] * face.getStepX() + v[1] * face.getStepY() + v[2] * face.getStepZ() >= -1e-6) {
            return false;
        }
        double[] h = {hit.x, hit.y, hit.z};
        int[] lo = {pos.getX(), pos.getY(), pos.getZ()};
        double best = Double.MAX_VALUE;
        int axis = -1;
        int sign = 0;
        for (int a = 0; a < 3; a++) {
            double d = -v[a];
            if (Math.abs(d) < 1e-9) {
                continue;
            }
            double t = ((d > 0 ? lo[a] + 1 : lo[a]) - h[a]) / d;
            if (t > 1e-9 && t < best) {
                best = t;
                axis = a;
                sign = d > 0 ? 1 : -1;
            }
        }
        if (axis < 0) {
            return false;
        }
        BlockPos entry = pos.offset(axis == 0 ? sign : 0, axis == 1 ? sign : 0, axis == 2 ? sign : 0);
        BlockState s = this.level.getBlockState(entry);
        return s.isAir() || s.getCollisionShape(this.level, entry).isEmpty();
    }

    /** Blocks whose right-click does something (a player sneaks to place against them). */
    static boolean interactive(BlockState s) {
        Block b = s.getBlock();
        return b instanceof LeverBlock || b instanceof ButtonBlock || b instanceof net.minecraft.world.level.block.DiodeBlock
                || b instanceof net.minecraft.world.level.block.NoteBlock || b instanceof net.minecraft.world.level.block.DoorBlock
                || b instanceof net.minecraft.world.level.block.TrapDoorBlock || b instanceof net.minecraft.world.level.block.FenceGateBlock
                || b instanceof net.minecraft.world.level.block.DaylightDetectorBlock || b instanceof net.minecraft.world.level.block.RedstoneWireBlock
                || b instanceof net.minecraft.world.level.block.BaseEntityBlock && !(b instanceof net.minecraft.world.level.block.piston.MovingPistonBlock)
                        && !(b instanceof net.minecraft.world.level.block.SculkSensorBlock);
    }

    /** How many of `target`'s player-chosen properties (facing, face, half...) `got` has; -1 if another block. */
    private static int placementScore(BlockState got, BlockState target) {
        if (got == null || got.getBlock() != target.getBlock()) {
            return -1;
        }
        int score = 0;
        for (net.minecraft.world.level.block.state.properties.Property<?> prop : target.getProperties()) {
            String name = prop.getName();
            if (!dynamicProperty(target, name) && !usableProperty(target, name) && got.getValue(prop).equals(target.getValue(prop))) {
                score++;
            }
        }
        return score;
    }

    private static int placementWanted(BlockState target) {
        int n = 0;
        for (net.minecraft.world.level.block.state.properties.Property<?> prop : target.getProperties()) {
            if (!dynamicProperty(target, prop.getName()) && !usableProperty(target, prop.getName())) {
                n++;
            }
        }
        return n;
    }

    /** What the item would place for this click (null when the click cannot place at `pos`). */
    private BlockState trialPlacement(net.minecraft.world.item.BlockItem item, net.minecraft.world.item.ItemStack stack, BlockPos pos, BlockHitResult hit) {
        net.minecraft.world.item.context.BlockPlaceContext ctx = new net.minecraft.world.item.context.BlockPlaceContext(
                mockPlayer(), net.minecraft.world.InteractionHand.MAIN_HAND, stack, hit);
        if (!ctx.canPlace() || !ctx.getClickedPos().equals(pos)) {
            return null;
        }
        net.minecraft.world.item.context.BlockPlaceContext updated = item.updatePlacementContext(ctx);
        if (updated == null || !updated.getClickedPos().equals(pos)) {
            return null;
        }
        try {
            return (BlockState) GET_PLACEMENT_STATE.invoke(item, updated);
        } catch (ReflectiveOperationException e) {
            throw new RuntimeException(e);
        }
    }

    /** One way to click: the hit, the look direction and sneaking. */
    private record Click(BlockHitResult hit, float yaw, float pitch, boolean sneak, boolean supported) {
        String describe() {
            String compass = Direction.fromYRot(yaw).getSerializedName();
            float off = net.minecraft.util.Mth.wrapDegrees(yaw - Direction.fromYRot(yaw).toYRot());
            if (Math.abs(off) > 1F) {
                compass += " (" + (int) Math.abs(off) + " degrees toward " + Direction.fromYRot(Direction.fromYRot(yaw).toYRot() + Math.signum(off) * 90F).getSerializedName() + ")";
            }
            String look = pitch >= 89F ? "looking down" : pitch <= -89F ? "looking up" : "facing " + compass
                    + (pitch > 0F ? " looking down " + (int) pitch : pitch < 0F ? " looking up " + (int) -pitch : "");
            BlockPos s = hit.getBlockPos();
            if (!supported) {
                return "put a temporary dirt block at " + s.getX() + " " + s.getY() + " " + s.getZ() + ", click its "
                        + hit.getDirection().getSerializedName() + " face, " + look + ", then break the dirt";
            }
            return "click the " + hit.getDirection().getSerializedName() + " face of " + s.getX() + " " + s.getY() + " " + s.getZ() + ", " + look
                    + (sneak ? ", sneaking" : "");
        }
    }

    private void aim(Click c) {
        Player p = mockPlayer();
        p.setShiftKeyDown(c.sneak());
        p.setYRot(c.yaw());
        p.setYHeadRot(c.yaw());
        p.setXRot(c.pitch());
    }

    /**
     * Place `target`'s block at `pos` like a survival player: try every click on a neighbouring
     * block's face (look direction, sneak; mid-air only when no click fits), keep the one whose
     * placement state has `target`'s player-chosen properties, run the real BlockItem.place
     * (getStateForPlacement, setBlock flags 11, setPlacedBy, the place game event), then right-click
     * the block until its click-set properties (repeater delay, lever...) match, settling between
     * clicks. "ok" is false when the block cannot be placed that way here.
     */
    com.google.gson.JsonObject placeLikePlayer(SimServer server, BlockPos pos, BlockState target, java.util.function.Function<BlockState, String> ser, int settle) {
        com.google.gson.JsonObject o = new com.google.gson.JsonObject();
        net.minecraft.world.item.Item it = target.getBlock().asItem();
        if (!(it instanceof net.minecraft.world.item.BlockItem item)) {
            o.addProperty("ok", false);
            o.addProperty("error", "no item places " + ser.apply(target));
            return o;
        }
        net.minecraft.world.item.ItemStack stack = new net.minecraft.world.item.ItemStack(item);
        Player player = mockPlayer();
        player.setItemInHand(net.minecraft.world.InteractionHand.MAIN_HAND, stack);
        boolean halves = target.getProperties().stream().anyMatch(p -> p.getName().equals("half") || p.getName().equals("type"));
        double[] hitYs = halves ? new double[] {0.25, 0.75} : new double[] {0.5};
        int wanted = placementWanted(target);
        int bestScore = -1;
        boolean scaffolded = false;
        Click best = null;
        search:
        for (int pass = 0; pass < 1; pass++) {
            for (Direction face : Direction.values()) {
                BlockPos support = pos.relative(face.getOpposite());
                BlockState supportState = this.level.getBlockState(support);
                if (supportState.isAir()) {
                    continue;
                }
                // Right-clicking a lever, repeater, dust... would use it instead of placing: sneak.
                boolean sneak = interactive(supportState);
                for (double hy : hitYs) {
                    Vec3 loc = Vec3.atCenterOf(support).add(face.getStepX() * 0.5,
                            face.getAxis() == Direction.Axis.Y ? face.getStepY() * 0.5 : hy - 0.5, face.getStepZ() * 0.5);
                    BlockHitResult hit = new BlockHitResult(loc, face, support, false);
                    for (float pitch : PITCHES) {
                        for (float yaw : YAWS) {
                            if (!visible(pos, face, loc, yaw, pitch)) {
                                continue;
                            }
                            Click c = new Click(hit, yaw, pitch, sneak, true);
                            aim(c);
                            int s = placementScore(trialPlacement(item, stack, pos, hit), target);
                            if (s > bestScore) {
                                bestScore = s;
                                best = c;
                                if (s == wanted) {
                                    break search;
                                }
                            }
                        }
                    }
                }
            }
        }
        // A temporary block only helps blocks that stand on their own: a repeater, dust, lever or
        // torch would pop off when the temporary block under it is broken.
        boolean standsAlone = target.canSurvive(this.level, pos);
        if (bestScore < wanted && this.level.getBlockState(pos).isAir() && standsAlone) {
            // No neighbouring block to click that gives this orientation (in the void the
            // sandbox is): put a temporary block next to it like a player would, click that,
            // break it afterwards. The temporary block's placement and removal update the
            // world like any other block.
            java.util.List<BlockPos> around = new java.util.ArrayList<>();
            for (BlockPos q : new BlockPos[] {pos.below(), pos.north(), pos.south(), pos.west(), pos.east(), pos.above()}) {
                if (this.level.getBlockState(q).isAir() && q.getY() > this.level.getMinY()) {
                    around.add(q);
                }
            }
            for (BlockPos q : around) {
                // Try the clicks against a dirt block pasted silently, so a dead end leaves no
                // trace; only the chosen one is built for real.
                BlockState dirt = Blocks.DIRT.defaultBlockState();
                this.level.setBlock(q, dirt, RAW_FLAGS);
                Direction face = Direction.getApproximateNearest((double) (pos.getX() - q.getX()), (double) (pos.getY() - q.getY()), (double) (pos.getZ() - q.getZ()));
                Click sClick = null;
                scaffoldSearch:
                for (double hy : hitYs) {
                    Vec3 loc = Vec3.atCenterOf(q).add(face.getStepX() * 0.5,
                            face.getAxis() == Direction.Axis.Y ? face.getStepY() * 0.5 : hy - 0.5, face.getStepZ() * 0.5);
                    BlockHitResult hit = new BlockHitResult(loc, face, q, false);
                    for (float pitch : PITCHES) {
                        for (float yaw : YAWS) {
                            if (!visible(pos, face, loc, yaw, pitch)) {
                                continue;
                            }
                            Click c = new Click(hit, yaw, pitch, false, false);
                            aim(c);
                            if (placementScore(trialPlacement(item, stack, pos, hit), target) == wanted) {
                                sClick = c;
                                break scaffoldSearch;
                            }
                        }
                    }
                }
                this.level.setBlock(q, Blocks.AIR.defaultBlockState(), RAW_FLAGS);
                if (sClick == null) {
                    continue;
                }
                this.level.setBlock(q, dirt, 11);
                this.level.gameEvent(net.minecraft.world.level.gameevent.GameEvent.BLOCK_PLACE, q,
                        net.minecraft.world.level.gameevent.GameEvent.Context.of(player, dirt));
                int dirtTicks = 0;
                for (int t = 0; t < settle && (t < 4 || !idle()); t++) {
                    tick(server);
                    dirtTicks++;
                }
                bestScore = wanted;
                best = sClick;
                aim(best);
                item.place(new net.minecraft.world.item.context.BlockPlaceContext(player, net.minecraft.world.InteractionHand.MAIN_HAND, stack, best.hit()));
                int placedTicks = 0;
                for (int t = 0; t < settle && (t < 4 || !idle()); t++) {
                    tick(server);
                    placedTicks++;
                }
                breakLikePlayer(q);
                o.addProperty("scaffold", q.getX() + " " + q.getY() + " " + q.getZ());
                // Ticks after the dirt went in and after the block went in (for exact replays).
                com.google.gson.JsonArray scaffoldTicks = new com.google.gson.JsonArray();
                scaffoldTicks.add(dirtTicks);
                scaffoldTicks.add(placedTicks);
                o.add("scaffold_ticks", scaffoldTicks);
                scaffolded = true;
                break;
            }
        }
        if (best == null || bestScore < wanted) {
            // A player would not place it wrong: the step fails, nothing is placed.
            player.setShiftKeyDown(false);
            player.setItemInHand(net.minecraft.world.InteractionHand.MAIN_HAND, net.minecraft.world.item.ItemStack.EMPTY);
            o.addProperty("ok", false);
            o.addProperty("error", !this.level.getBlockState(pos).isAir() ? "cannot place at " + pos.toShortString() + ": occupied by " + ser.apply(this.level.getBlockState(pos))
                    : !standsAlone ? "cannot place " + ser.apply(target) + " yet: nothing for it to stand on"
                    : "cannot place " + ser.apply(target) + " now: no visible face gives that orientation");
            return o;
        }
        if (!scaffolded) {
            aim(best);
            item.place(new net.minecraft.world.item.context.BlockPlaceContext(player, net.minecraft.world.InteractionHand.MAIN_HAND, stack, best.hit()));
        }
        player.setShiftKeyDown(false);
        player.setItemInHand(net.minecraft.world.InteractionHand.MAIN_HAND, net.minecraft.world.item.ItemStack.EMPTY);
        int clicks = 0;
        com.google.gson.JsonArray clickTicks = new com.google.gson.JsonArray();
        for (net.minecraft.world.level.block.state.properties.Property<?> prop : target.getProperties()) {
            if (!usableProperty(target, prop.getName())) {
                continue;
            }
            for (int i = 0; i < 25; i++) {
                BlockState now = this.level.getBlockState(pos);
                if (now.getBlock() != target.getBlock() || now.getValue(prop).equals(target.getValue(prop))) {
                    break;
                }
                // let the previous placement or click settle, then click again
                int waited = 0;
                for (int t = 0; t < settle && !idle(); t++) {
                    tick(server);
                    waited++;
                }
                use(pos, Direction.UP);
                clicks++;
                clickTicks.add(waited);
            }
        }
        // The exact click (look direction, hit point, sneaking) and the ticks before each
        // right-click, so another world (vanilla-build) can replay the very same inputs.
        o.addProperty("yaw", best.yaw());
        o.addProperty("pitch", best.pitch());
        o.addProperty("sneak", best.sneak());
        BlockPos hitPos = best.hit().getBlockPos();
        com.google.gson.JsonArray hitPosJson = new com.google.gson.JsonArray();
        hitPosJson.add(hitPos.getX());
        hitPosJson.add(hitPos.getY());
        hitPosJson.add(hitPos.getZ());
        o.add("hit_pos", hitPosJson);
        o.addProperty("hit_face", best.hit().getDirection().getSerializedName());
        Vec3 hitLoc = best.hit().getLocation();
        com.google.gson.JsonArray hitLocJson = new com.google.gson.JsonArray();
        hitLocJson.add(hitLoc.x);
        hitLocJson.add(hitLoc.y);
        hitLocJson.add(hitLoc.z);
        o.add("hit_loc", hitLocJson);
        o.add("click_ticks", clickTicks);
        BlockState placed = this.level.getBlockState(pos);
        boolean ok = bestScore == wanted && placed.getBlock() == target.getBlock();
        for (net.minecraft.world.level.block.state.properties.Property<?> prop : target.getProperties()) {
            if (usableProperty(target, prop.getName()) && placed.getBlock() == target.getBlock() && !placed.getValue(prop).equals(target.getValue(prop))) {
                ok = false;
            }
        }
        o.addProperty("ok", ok);
        o.addProperty("placed", ser.apply(placed));
        o.addProperty("supported", best.supported());
        o.addProperty("click", best.describe());
        o.addProperty("clicks", clicks);
        if (!ok) {
            o.addProperty("error", "placed " + ser.apply(placed) + ", wanted " + ser.apply(target));
        }
        return o;
    }

    /** Break a block like a survival player (no drops: the player picks them up). */
    void breakLikePlayer(BlockPos pos) {
        BlockState state = this.level.getBlockState(pos);
        if (state.isAir()) {
            return;
        }
        BlockState adjusted = state.getBlock().playerWillDestroy(this.level, pos, state, mockPlayer());
        if (this.level.removeBlock(pos, false)) {
            state.getBlock().destroy(this.level, pos, adjusted);
        }
    }

    private static java.lang.reflect.Method method(Class<?> owner, String name, Class<?>... params) {
        try {
            java.lang.reflect.Method m = owner.getDeclaredMethod(name, params);
            m.setAccessible(true);
            return m;
        } catch (ReflectiveOperationException e) {
            throw new IllegalStateException("Missing method " + owner.getName() + "." + name, e);
        }
    }

    private Player mockPlayer() {
        if (this.mockPlayer == null) {
            this.mockPlayer = new Player(this.level, new GameProfile(UUID.nameUUIDFromBytes(("mctas" + this.id).getBytes()), "mctas")) {
                @Override
                public GameType gameMode() {
                    return GameType.CREATIVE;
                }

                @Override
                public boolean isClientAuthoritative() {
                    return false;
                }
            };
        }
        return this.mockPlayer;
    }

    private static Field field(Class<?> owner, String name) {
        try {
            Field f = owner.getDeclaredField(name);
            f.setAccessible(true);
            return f;
        } catch (ReflectiveOperationException e) {
            throw new IllegalStateException("Missing field " + owner.getName() + "." + name, e);
        }
    }

    private static Object get(Field f, Object target) {
        try {
            return f.get(target);
        } catch (IllegalAccessException e) {
            throw new IllegalStateException(e);
        }
    }
}
