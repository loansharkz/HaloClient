package gg.haloclient.hud;

import com.mojang.blaze3d.platform.InputConstants;
import org.lwjgl.sdl.SDLScancode;

import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;
import net.minecraft.resources.Identifier;

import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientLifecycleEvents;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keymapping.v1.KeyMappingHelper;
import net.fabricmc.fabric.api.client.rendering.v1.hud.HudElementRegistry;
import net.fabricmc.fabric.api.client.rendering.v1.hud.VanillaHudElements;

/**
 * Halo HUD entry point. Everything here is display-only: counters, key
 * display, durability and effect timers, plus a zoom that lowers the FOV
 * option while the key is held. No input automation, no hidden information.
 */
public final class HaloHud implements ClientModInitializer {
	public static final String MOD_ID = "halohud";

	static HudConfig config;
	private static KeyMapping zoomKey;
	private static KeyMapping toggleKey;

	/** FOV/smooth-camera values to restore when zoom ends; null when not zooming. */
	private static Integer savedFov;
	private static boolean savedSmoothCamera;

	@Override
	public void onInitializeClient() {
		config = HudConfig.load();

		KeyMapping.Category category = KeyMapping.Category.register(Identifier.fromNamespaceAndPath(MOD_ID, "main"));
		zoomKey = KeyMappingHelper.registerKeyMapping(new KeyMapping("key.halohud.zoom", InputConstants.Type.KEYBOARD, SDLScancode.SDL_SCANCODE_C, category));
		toggleKey = KeyMappingHelper.registerKeyMapping(new KeyMapping("key.halohud.toggle", InputConstants.Type.KEYBOARD, SDLScancode.SDL_SCANCODE_H, category));

		ClientTickEvents.END_CLIENT_TICK.register(HaloHud::tick);
		ClientLifecycleEvents.CLIENT_STOPPING.register(HaloHud::endZoom);
		HudElementRegistry.attachElementBefore(VanillaHudElements.CHAT, Identifier.fromNamespaceAndPath(MOD_ID, "hud"), HudRenderer::extract);
	}

	private static void tick(Minecraft mc) {
		while (toggleKey.consumeClick()) {
			config.enabled = !config.enabled;
			config.save();
		}

		boolean zooming = config.zoom && zoomKey.isDown() && mc.screen == null && mc.player != null;
		if (zooming && savedFov == null) {
			savedFov = mc.options.fov().get();
			savedSmoothCamera = mc.options.smoothCamera;
			mc.options.fov().set(Math.max(30, Math.min(savedFov, config.zoomFov)));
			if (config.smoothZoom) {
				mc.options.smoothCamera = true;
			}
		} else if (!zooming && savedFov != null) {
			endZoom(mc);
		}
	}

	private static void endZoom(Minecraft mc) {
		if (savedFov != null) {
			mc.options.fov().set(savedFov);
			mc.options.smoothCamera = savedSmoothCamera;
			savedFov = null;
		}
	}

	static boolean isZooming() {
		return savedFov != null;
	}
}
