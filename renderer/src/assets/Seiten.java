import java.io.PrintStream;
import java.nio.charset.StandardCharsets;

import net.minecraft.SharedConstants;
import net.minecraft.core.Direction;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.phys.shapes.Shapes;

/**
 * Schreibt je Block von 26.2 die Seiten, an denen er voll deckt: dort ist
 * getFaceOcclusionShape genau Shapes.block(), und Block.shouldRenderFace
 * lässt die Fläche eines Nachbarn mit cullface zu dieser Seite weg. Je
 * Zustand zwei Hexziffern, in der Reihenfolge von getPossibleStates, mit
 * einem Bit je Richtung in der Reihenfolge von Direction.values(): 1 unten,
 * 2 oben, 4 Norden, 8 Süden, 10 Westen, 20 Osten. Blöcke, die in keinem
 * Zustand eine Seite voll decken, fehlen; haben alle Zustände dieselben
 * Ziffern, stehen sie einmal.
 */
public class Seiten {
    public static void main(String[] args) {
        // Bootstrap leitet System.err in sein Log und damit nach System.out.
        var err = System.err;
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var out = new PrintStream(System.out, true, StandardCharsets.UTF_8);
        int bloecke = 0;
        int zustaende = 0;
        int ganz = 0;
        for (Block block : BuiltInRegistries.BLOCK) {
            var ziffern = new StringBuilder();
            boolean etwas = false;
            for (BlockState state : block.getStateDefinition().getPossibleStates()) {
                int bits = 0;
                for (Direction dir : Direction.values()) {
                    if (state.getFaceOcclusionShape(dir) == Shapes.block()) {
                        bits |= 1 << dir.ordinal();
                    }
                }
                if (bits != 0) {
                    etwas = true;
                    zustaende++;
                }
                if (bits == 0x3f) {
                    ganz++;
                }
                ziffern.append(String.format("%02x", bits));
            }
            if (!etwas) {
                continue;
            }
            bloecke++;
            String name = BuiltInRegistries.BLOCK.getKey(block).getPath();
            String z = ziffern.toString();
            String erstes = z.substring(0, 2);
            if (z.equals(erstes.repeat(z.length() / 2))) {
                out.println(name + " " + erstes);
            } else {
                out.println(name + " " + z);
            }
        }
        err.println(bloecke + " Blöcke, " + zustaende + " Zustände mit einer vollen Seite, "
                + ganz + " mit allen sechs");
    }
}
