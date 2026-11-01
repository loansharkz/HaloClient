package gg.haloclient.hud;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;

import net.fabricmc.loader.api.FabricLoader;

/** Settings stored in config/halohud.json. */
public final class HudConfig {
	private static final Gson GSON = new GsonBuilder().setPrettyPrinting().create();

	public boolean enabled = true;
	public boolean fps = true;
	public boolean cps = true;
	public boolean ping = true;
	public boolean keystrokes = true;
	public boolean armor = true;
	public boolean potions = true;
	public boolean zoom = true;
	public boolean smoothZoom = true;
	/** FOV used while zooming (the game allows 30–110). */
	public int zoomFov = 30;
	/** Accent color for pressed keys, ARGB. Matches the launcher's violet. */
	public int accent = 0xC08B5CF6;
	/** Background of HUD boxes, ARGB. */
	public int background = 0x90000000;
	public double scale = 1.0;

	private static Path path() {
		return FabricLoader.getInstance().getConfigDir().resolve("halohud.json");
	}

	public static HudConfig load() {
		try {
			Path p = path();
			if (Files.exists(p)) {
				HudConfig c = GSON.fromJson(Files.readString(p), HudConfig.class);
				if (c != null) {
					return c;
				}
			}
		} catch (Exception e) {
			// Bad file: fall back to defaults and rewrite it.
		}
		HudConfig c = new HudConfig();
		c.save();
		return c;
	}

	public void save() {
		try {
			Files.createDirectories(path().getParent());
			Files.writeString(path(), GSON.toJson(this));
		} catch (IOException ignored) {
			// Not fatal; settings just won't persist.
		}
	}
}
