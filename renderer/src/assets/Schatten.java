import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.util.TreeSet;

import net.minecraft.SharedConstants;
import net.minecraft.core.BlockPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.EmptyBlockGetter;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.state.BlockState;

/**
 * Schreibt je Block von 26.3, was die weiche Beleuchtung des Spiels
 * (BlockModelLighter.prepareQuadAmbientOcclusion) über ihn wissen muss: je
 * Zustand eine Ziffer, in der Reihenfolge von getPossibleStates.
 * Bit 1: getShadeBrightness ist 0,2 statt 1. Bit 2: nicht
 * isLightPermeable, also solidRender und getLightDampening > 0 (in 26.2
 * isViewBlocking statt solidRender). Bit 4: isCollisionShapeFullBlock,
 * dann liegt jede ebene Fläche des Modells im Licht der Zelle davor
 * (BlockModelLighter.prepareQuadShape, faceCubic). Ob ein Block leuchtet und
 * deshalb ohne weiche Beleuchtung gezeichnet wird, steht in leuchten.txt.
 * Blöcke ohne ein Bit fehlen; haben alle Zustände dieselbe Ziffer, steht
 * sie einmal.
 */
public class Schatten {
    public static void main(String[] args) {
        // Bootstrap leitet System.err in sein Log und damit nach System.out.
        var err = System.err;
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var level = EmptyBlockGetter.INSTANCE;
        var pos = BlockPos.ZERO;
        var out = new PrintStream(System.out, true, StandardCharsets.UTF_8);
        var helligkeiten = new TreeSet<Float>();
        for (Block block : BuiltInRegistries.BLOCK) {
            String name = BuiltInRegistries.BLOCK.getKey(block).getPath();
            var ziffern = new StringBuilder();
            for (BlockState state : block.getStateDefinition().getPossibleStates()) {
                float shade = state.getShadeBrightness(level, pos);
                helligkeiten.add(shade);
                int f = (shade < 1.0f ? 1 : 0)
                        | (!state.isLightPermeable() ? 2 : 0)
                        | (state.isCollisionShapeFullBlock(level, pos) ? 4 : 0);
                ziffern.append((char) ('0' + f));
            }
            String z = ziffern.toString();
            if (z.chars().allMatch(c -> c == '0')) {
                continue;
            }
            if (z.chars().allMatch(c -> c == z.charAt(0))) {
                out.println(name + " " + z.charAt(0));
            } else {
                out.println(name + " " + z);
            }
        }
        err.println("getShadeBrightness: " + helligkeiten);
    }
}
