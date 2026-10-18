package dev.vjeux.mctas.client;

import net.minecraft.client.Minecraft;

/**
 * Launches the real Minecraft client unchanged, plus one tweak for recording: once a world is
 * shown, hide the HUD (what F1 does) so frames contain only the scene.
 */
public final class ClientMain {
    private ClientMain() {}

    public static void main(String[] args) throws Exception {
        Thread hud = new Thread(() -> {
            while (true) {
                try {
                    Thread.sleep(250);
                } catch (InterruptedException e) {
                    return;
                }
                Minecraft mc = Minecraft.getInstance();
                if (mc != null && mc.player != null && mc.gui != null) {
                    mc.execute(() -> {
                        if (!mc.gui.hud.isHidden()) {
                            mc.gui.hud.toggle();
                        }
                    });
                }
            }
        }, "mctas-hide-hud");
        hud.setDaemon(true);
        hud.start();
        net.minecraft.client.main.Main.main(args);
    }
}
