import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

import net.minecraft.SharedConstants;
import net.minecraft.core.Direction;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.phys.shapes.Shapes;
import net.minecraft.world.phys.shapes.VoxelShape;

/**
 * Schreibt je Block von 26.2, was die Lichtausbreitung des Spiels
 * (LightEngine.propagateIncrease, ChunkSkyLightSources) über ihn wissen
 * muss: je Zustand sieben Zeichen, in der Reihenfolge von
 * getPossibleStates. Das erste ist getLightDampening als Ziffer (0, 1 oder
 * f), die sechs danach die Flächen, mit denen er das Licht an seinen
 * Seiten aufhält, in der Reihenfolge von Direction.values() (unten, oben,
 * Norden, Süden, Westen, Osten), wie LightEngine.getOcclusionShape sie
 * nimmt: 0 keine, 1 die ganze Seite, ab 2 eine Teilfläche, als Nummer in
 * der Liste der Teilflächen dieser Richtung, Ziffern zur Basis 36. Blöcke,
 * die in keinem Zustand dämpfen oder eine Fläche haben, fehlen; haben alle
 * Zustände dieselben Zeichen, stehen sie einmal. Am Ende je Achse eine
 * Zeile "paar y a b" für jede Teilfläche a der positiven und b der
 * negativen Richtung, die zusammen die ganze Seite decken
 * (Shapes.faceShapeOccludes). Wie hell ein Block leuchtet, steht in
 * leuchten.txt.
 */
public class Licht {
    public static void main(String[] args) {
        // Bootstrap leitet System.err in sein Log und damit nach System.out.
        var err = System.err;
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var out = new PrintStream(System.out, true, StandardCharsets.UTF_8);
        List<List<VoxelShape>> teile = new ArrayList<>();
        List<Map<String, Integer>> nummern = new ArrayList<>();
        for (int i = 0; i < 6; i++) {
            teile.add(new ArrayList<>());
            nummern.add(new HashMap<>());
        }
        int zustaende = 0;
        for (Block block : BuiltInRegistries.BLOCK) {
            var zeichen = new StringBuilder();
            boolean etwas = false;
            for (BlockState state : block.getStateDefinition().getPossibleStates()) {
                int d = state.getLightDampening();
                if (d != 0 && d != 1 && d != 15) {
                    throw new IllegalStateException(state + ": Dämpfung " + d);
                }
                var z = new StringBuilder().append(Character.forDigit(d, 16));
                // LightEngine.isEmptyShape: ohne canOcclude oder
                // useShapeForLightOcclusion hält er an seinen Seiten nichts auf.
                boolean formen = state.canOcclude() && state.useShapeForLightOcclusion();
                for (Direction dir : Direction.values()) {
                    int k = 0;
                    if (formen) {
                        VoxelShape shape = state.getFaceOcclusionShape(dir);
                        if (shape.isEmpty()) {
                            k = 0;
                        } else if (Shapes.faceShapeOccludes(Shapes.empty(), shape)) {
                            k = 1;
                        } else {
                            int o = dir.ordinal();
                            k = 2 + nummern.get(o).computeIfAbsent(shape.toAabbs().toString(), key -> {
                                teile.get(o).add(shape);
                                return teile.get(o).size() - 1;
                            });
                        }
                    }
                    if (k >= 36) {
                        throw new IllegalStateException(dir + ": mehr als 34 Teilflächen");
                    }
                    z.append(Character.forDigit(k, 36));
                }
                String s = z.toString();
                if (!s.equals("0000000")) {
                    etwas = true;
                    zustaende++;
                }
                zeichen.append(s);
            }
            if (!etwas) {
                continue;
            }
            String name = BuiltInRegistries.BLOCK.getKey(block).getPath();
            String z = zeichen.toString();
            String erstes = z.substring(0, 7);
            if (z.equals(erstes.repeat(z.length() / 7))) {
                out.println(name + " " + erstes);
            } else {
                out.println(name + " " + z);
            }
        }
        int paare = 0;
        for (Direction dir : Direction.values()) {
            if (dir.getAxisDirection() != Direction.AxisDirection.POSITIVE) {
                continue;
            }
            var a = teile.get(dir.ordinal());
            var b = teile.get(dir.getOpposite().ordinal());
            for (int i = 0; i < a.size(); i++) {
                for (int j = 0; j < b.size(); j++) {
                    if (Shapes.faceShapeOccludes(a.get(i), b.get(j))) {
                        out.println("paar " + dir.getAxis().getName() + " "
                                + Character.forDigit(i + 2, 36) + " " + Character.forDigit(j + 2, 36));
                        paare++;
                    }
                }
            }
        }
        var anzahl = new StringBuilder();
        for (Direction dir : Direction.values()) {
            anzahl.append(' ').append(dir.getName()).append(' ').append(teile.get(dir.ordinal()).size());
        }
        err.println(zustaende + " Zustände, " + paare + " Paare, Teilflächen:" + anzahl);
    }
}
