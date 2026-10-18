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
