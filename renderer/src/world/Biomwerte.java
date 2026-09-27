import java.io.PrintStream;

import net.minecraft.SharedConstants;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Holder;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.biome.BiomeManager;
import net.minecraft.world.level.biome.BiomeSpecialEffects;

/**
 * Gibt die Sollwerte der Tests für Seed, Biomzoom und Sumpfrauschen aus den
 * Klassen des Spiels selbst aus: BiomeManager.obfuscateSeed,
 * BiomeManager.getBiome mit einer Quelle, die jede Viertelposition als
 * eigenes Biom meldet, und Biome.BIOME_INFO_NOISE mit
 * GrassColorModifier.SWAMP. Die Werte stehen in den Tests von
 * biomzoom.rs, noise.rs und colors.rs.
 *
 * Aufruf mit Java 25: java -cp <Klassenpfad> Biomwerte.java, der
 * Klassenpfad ist META-INF/versions/26.2/server-26.2.jar aus dem Server-JAR
 * 26.2 samt den JARs unter META-INF/libraries.
 */
public class Biomwerte {
    @SuppressWarnings({"unchecked", "rawtypes", "removal"})
    public static void main(String[] args) {
        // Bootstrap leitet System.out in sein Log; die Werte gehen daran vorbei.
        PrintStream out = System.out;
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();

        long[] seeds = {0L, 1L, -1L, 12345L, Long.MIN_VALUE, Long.MAX_VALUE, -4172144997902289642L};
        for (long seed : seeds) {
            out.println("obfuscate " + seed + " " + BiomeManager.obfuscateSeed(seed));
        }

        // Jede Viertelposition ein eigenes Biom: der Holder trägt ihre
        // Koordinaten.
        BiomeManager.NoiseBiomeSource quelle =
            (x, y, z) -> (Holder<Biome>) (Holder) Holder.direct(new int[] {x, y, z});
        for (long seed : new long[] {12345L, -4172144997902289642L}) {
            BiomeManager zoom = new BiomeManager(quelle, BiomeManager.obfuscateSeed(seed));
            for (int y = -3; y <= 2; y++) {
                StringBuilder zeile = new StringBuilder();
                for (int z = -6; z <= 5; z++) {
                    for (int x = -6; x <= 5; x++) {
                        zeile.append(ecke(zoom, x, y, z));
                    }
                }
                out.println("zoom " + seed + " y=" + y + " " + zeile);
            }
            int[][] weit = {
                {29999999, 319, -29999999}, {-30000000, -64, 30000000}, {-1, -64, -1},
                {1000000, 70, -2000000}, {-123457, 63, 98765},
            };
            for (int[] p : weit) {
                Holder raw = zoom.getBiome(new BlockPos(p[0], p[1], p[2]));
                int[] q = (int[]) raw.value();
                out.println("weit " + seed + " " + p[0] + " " + p[1] + " " + p[2] + " -> "
                    + q[0] + " " + q[1] + " " + q[2]);
            }
        }

        // Das Rauschen an einem Gitter, x aussen, z innen, und je Seite der
        // Grenze -0,1 die Stelle im Quadrat bis ±1000, die ihr am nächsten
        // liegt.
        int[] stellen = {-1000, -517, -64, -1, 0, 1, 7, 100, 333, 1024, 29999999};
        for (int x : stellen) {
            for (int z : stellen) {
                double wert = Biome.BIOME_INFO_NOISE.getValue(x * 0.0225, z * 0.0225, false);
                int farbe = BiomeSpecialEffects.GrassColorModifier.SWAMP.modifyColor(x, z, 0);
                out.println("sumpf " + x + " " + z + " " + wert + " " + Integer.toHexString(farbe));
            }
        }
        double unter = Double.NEGATIVE_INFINITY, ueber = Double.POSITIVE_INFINITY;
        int[] u = null, o = null;
        for (int x = -1000; x < 1000; x++) {
            for (int z = -1000; z < 1000; z++) {
                double wert = Biome.BIOME_INFO_NOISE.getValue(x * 0.0225, z * 0.0225, false);
                if (wert < -0.1 && wert > unter) {
                    unter = wert;
                    u = new int[] {x, z};
                }
                if (wert >= -0.1 && wert < ueber) {
                    ueber = wert;
                    o = new int[] {x, z};
                }
            }
        }
        out.println("grenze unter " + u[0] + " " + u[1] + " " + unter);
        out.println("grenze ueber " + o[0] + " " + o[1] + " " + ueber);
    }

    /**
     * Welche der acht Ecken um (x - 2, y - 2, z - 2) gewinnt, als Ziffer wie
     * p in getBiome: 4 für x + 1, 2 für y + 1, 1 für z + 1.
     */
    @SuppressWarnings("rawtypes")
    static int ecke(BiomeManager zoom, int x, int y, int z) {
        Holder raw = zoom.getBiome(new BlockPos(x, y, z));
        int[] q = (int[]) raw.value();
        int dx = q[0] - ((x - 2) >> 2), dy = q[1] - ((y - 2) >> 2), dz = q[2] - ((z - 2) >> 2);
        if (((dx | dy | dz) >>> 1) != 0) {
            throw new IllegalStateException("keine Ecke: " + x + " " + y + " " + z);
        }
        return dx * 4 + dy * 2 + dz;
    }
}
