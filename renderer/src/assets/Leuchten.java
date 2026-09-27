import java.io.PrintStream;
import java.nio.charset.StandardCharsets;

import net.minecraft.SharedConstants;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.state.BlockState;

/**
 * Schreibt je Block von 26.2, wie hell er selbst leuchtet, so wie
 * LightCoordsUtil.getLightCoords es beim Zeichnen nimmt: je Zustand eine
 * Ziffer 0 bis f für getLightEmission, in der Reihenfolge von
 * getPossibleStates, oder ein x für emissiveRendering, dann zeichnet ihn das
 * Spiel voll hell. Blöcke, die in keinem Zustand leuchten, fehlen; haben
 * alle Zustände dasselbe Zeichen, steht es einmal.
 */
public class Leuchten {
    public static void main(String[] args) {
        // Bootstrap leitet System.err in sein Log und damit nach System.out.
        var err = System.err;
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var out = new PrintStream(System.out, true, StandardCharsets.UTF_8);
        int voll = 0;
        for (Block block : BuiltInRegistries.BLOCK) {
            var zeichen = new StringBuilder();
            for (BlockState state : block.getStateDefinition().getPossibleStates()) {
                if (state.emissiveRendering()) {
                    zeichen.append('x');
                    voll++;
                } else {
                    zeichen.append(Character.forDigit(state.getLightEmission(), 16));
                }
            }
            String z = zeichen.toString();
            if (z.chars().allMatch(c -> c == '0')) {
                continue;
            }
            String name = BuiltInRegistries.BLOCK.getKey(block).getPath();
            if (z.chars().allMatch(c -> c == z.charAt(0))) {
                out.println(name + " " + z.charAt(0));
            } else {
                out.println(name + " " + z);
            }
        }
        err.println("emissiveRendering: " + voll + " Zustände");
    }
}
