import java.io.PrintStream;
import java.nio.charset.StandardCharsets;

import net.minecraft.SharedConstants;
import net.minecraft.core.BlockPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.EmptyBlockGetter;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.state.BlockState;

/**
 * Schreibt je Block von 26.2, ob er in der Ecke einer Fläche die Sicht
 * nimmt, wie die weiche Beleuchtung von 26.2 es fragt
 * (BlockModelLighter.prepareQuadAmbientOcclusion): isViewBlocking und
 * getLightDampening > 0. Je Zustand eine Ziffer 1 oder 0, in der
 * Reihenfolge von getPossibleStates. Blöcke ohne eine 1 fehlen; haben alle
 * Zustände dieselbe Ziffer, steht sie einmal. Nur mit dem Spiel von 26.2:
 * In 26.3 fragt die weiche Beleuchtung isLightPermeable, das steht in
 * schatten.txt.
 */
public class Sicht262 {
    public static void main(String[] args) {
        // Bootstrap leitet System.err in sein Log und damit nach System.out.
        var err = System.err;
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var level = EmptyBlockGetter.INSTANCE;
        var pos = BlockPos.ZERO;
        var out = new PrintStream(System.out, true, StandardCharsets.UTF_8);
        int bloecke = 0;
        int gemischt = 0;
        for (Block block : BuiltInRegistries.BLOCK) {
            String name = BuiltInRegistries.BLOCK.getKey(block).getPath();
            var ziffern = new StringBuilder();
            for (BlockState state : block.getStateDefinition().getPossibleStates()) {
                boolean sicht = state.isViewBlocking(level, pos) && state.getLightDampening() > 0;
                ziffern.append(sicht ? '1' : '0');
            }
            String z = ziffern.toString();
            if (z.chars().allMatch(c -> c == '0')) {
                continue;
            }
            bloecke++;
            if (z.chars().allMatch(c -> c == z.charAt(0))) {
                out.println(name + " " + z.charAt(0));
            } else {
                gemischt++;
                out.println(name + " " + z);
            }
        }
        err.println("Sicht262: " + bloecke + " Blöcke, " + gemischt + " mit Zuständen verschieden");
    }
}
