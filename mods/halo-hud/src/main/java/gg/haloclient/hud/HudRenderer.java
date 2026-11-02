package gg.haloclient.hud;

import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Collection;
import java.util.Deque;
import java.util.List;

import net.minecraft.client.DeltaTracker;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.multiplayer.PlayerInfo;
import net.minecraft.world.effect.MobEffectInstance;
import net.minecraft.world.entity.EquipmentSlot;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.item.ItemStack;

/** Draws the HUD every frame. Uses only public game state. */
final class HudRenderer {
	private static final int TEXT = 0xFFFFFFFF;
	private static final int MUTED = 0xFFB9B4C8;
	private static final int PAD = 4;
	private static final int LINE = 13;
	private static final int KEY = 22;
	private static final int GAP = 2;

	private static final ClickCounter LEFT = new ClickCounter();
	private static final ClickCounter RIGHT = new ClickCounter();

	private HudRenderer() {
	}

	static void extract(GuiGraphicsExtractor g, DeltaTracker deltaTracker) {
		HudConfig c = HaloHud.config;
		Minecraft mc = Minecraft.getInstance();
		Player player = mc.player;
		// Drawn inside the vanilla HUD pass, so F1 (hide GUI) hides it with the rest of the HUD.
		if (c == null || !c.enabled || player == null) {
			return;
		}
		Font font = mc.font;

		// Click counting runs every frame so fast clicks between ticks still count.
		LEFT.update(mc.options.keyAttack.isDown());
		RIGHT.update(mc.options.keyUse.isDown());

		g.pose().pushMatrix();
		g.pose().scale((float) c.scale);

		int x = PAD;
		int y = PAD;

		// --- counters
		List<String> lines = new ArrayList<>();
		if (c.fps) {
			lines.add("FPS " + mc.getFps());
		}
		if (c.cps) {
			lines.add("CPS " + LEFT.cps() + " | " + RIGHT.cps());
		}
		if (c.ping) {
			lines.add("Ping " + ping(mc, player));
		}
		for (String line : lines) {
			int w = font.width(line) + PAD * 2;
			g.fill(x, y, x + w, y + LINE, c.background);
			g.text(font, line, x + PAD, y + 3, TEXT);
			y += LINE + GAP;
		}

		// --- keystrokes
		if (c.keystrokes) {
			y += GAP;
			int row = KEY + GAP;
			key(g, font, c, mc.options.keyUp, "W", x + row, y, KEY, KEY);
			key(g, font, c, mc.options.keyLeft, "A", x, y + row, KEY, KEY);
			key(g, font, c, mc.options.keyDown, "S", x + row, y + row, KEY, KEY);
			key(g, font, c, mc.options.keyRight, "D", x + row * 2, y + row, KEY, KEY);
			int half = (KEY * 3 + GAP * 2 - GAP) / 2;
			key(g, font, c, mc.options.keyAttack, "LMB", x, y + row * 2, half, KEY);
			key(g, font, c, mc.options.keyUse, "RMB", x + half + GAP, y + row * 2, half, KEY);
			key(g, font, c, mc.options.keyJump, "———", x, y + row * 3, KEY * 3 + GAP * 2, 12);
			y += row * 3 + 12 + GAP;
		}

		g.pose().popMatrix();

		int right = (int) (g.guiWidth() / c.scale);
		int rightY = (int) (g.guiHeight() / c.scale / 2) - 40;
		g.pose().pushMatrix();
		g.pose().scale((float) c.scale);

		// --- armor durability (right side)
		if (c.armor) {
			EquipmentSlot[] slots = {EquipmentSlot.HEAD, EquipmentSlot.CHEST, EquipmentSlot.LEGS, EquipmentSlot.FEET, EquipmentSlot.MAINHAND};
			String[] labels = {"Helmet", "Chest", "Legs", "Boots", "Hand"};
			for (int i = 0; i < slots.length; i++) {
				ItemStack stack = player.getItemBySlot(slots[i]);
				if (stack.isEmpty() || !stack.isDamageableItem()) {
					continue;
				}
				int max = stack.getMaxDamage();
				int left = max - stack.getDamageValue();
				String text = labels[i] + " " + left + "/" + max;
				int w = font.width(text) + PAD * 2;
				g.fill(right - w - PAD, rightY, right - PAD, rightY + LINE, c.background);
				g.text(font, text, right - w, rightY + 3, durabilityColor(left, max));
				rightY += LINE + GAP;
			}
		}

		// --- potion effects (right side, under armor)
		if (c.potions) {
			Collection<MobEffectInstance> effects = player.getActiveEffects();
			if (!effects.isEmpty()) {
				rightY += GAP;
			}
			for (MobEffectInstance effect : effects) {
				String name = effect.getEffect().value().getDisplayName().getString();
				int amp = effect.getAmplifier();
				if (amp > 0) {
					name += " " + roman(amp + 1);
				}
				String time = effect.isInfiniteDuration() ? "∞" : formatTicks(effect.getDuration());
				String text = name + "  " + time;
				int w = font.width(text) + PAD * 2;
				g.fill(right - w - PAD, rightY, right - PAD, rightY + LINE, c.background);
				g.text(font, name, right - w, rightY + 3, TEXT);
				g.text(font, time, right - PAD * 2 - font.width(time), rightY + 3, MUTED);
				rightY += LINE + GAP;
			}
		}

		// --- zoom indicator
		if (HaloHud.isZooming()) {
			String z = "Zoom";
			g.text(font, z, right / 2 - font.width(z) / 2, (int) (g.guiHeight() / c.scale) - 70, MUTED);
		}

		g.pose().popMatrix();
	}

	private static void key(GuiGraphicsExtractor g, Font font, HudConfig c, KeyMapping mapping, String label, int x, int y, int w, int h) {
		boolean down = mapping.isDown();
		g.fill(x, y, x + w, y + h, down ? c.accent : c.background);
		int tx = x + (w - font.width(label)) / 2;
		int ty = y + (h - 8) / 2;
		g.text(font, label, tx, ty, TEXT);
	}

	private static String ping(Minecraft mc, Player player) {
		if (mc.getConnection() == null) {
			return "-";
		}
		PlayerInfo info = mc.getConnection().getPlayerInfo(player.getUUID());
		if (info == null) {
			return "-";
		}
		return info.getLatency() + "ms";
	}

	private static int durabilityColor(int left, int max) {
		float f = max == 0 ? 1f : (float) left / max;
		if (f > 0.5f) {
			return 0xFF7CF29A;
		}
		if (f > 0.2f) {
			return 0xFFFFD166;
		}
		return 0xFFFF6B6B;
	}

	static String formatTicks(int ticks) {
		int secs = Math.max(0, ticks / 20);
		return String.format("%d:%02d", secs / 60, secs % 60);
	}

	static String roman(int n) {
		String[] r = {"I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"};
		return n >= 1 && n <= r.length ? r[n - 1] : Integer.toString(n);
	}

	/** Counts presses in the last second from per-frame key state. */
	static final class ClickCounter {
		private final Deque<Long> clicks = new ArrayDeque<>();
		private boolean wasDown;

		void update(boolean down) {
			long now = System.currentTimeMillis();
			if (down && !wasDown) {
				clicks.addLast(now);
			}
			wasDown = down;
			while (!clicks.isEmpty() && now - clicks.peekFirst() > 1000) {
				clicks.removeFirst();
			}
		}

		int cps() {
			return clicks.size();
		}
	}
}
