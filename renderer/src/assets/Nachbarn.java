import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.BitSet;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Random;
import java.util.Set;
import java.util.TreeMap;
import java.util.stream.Collectors;

import net.minecraft.SharedConstants;
import net.minecraft.core.Direction;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.RegistryLayer;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.repository.ServerPacksSource;
import net.minecraft.server.packs.resources.MultiPackResourceManager;
import net.minecraft.tags.TagLoader;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.LiquidBlock;
import net.minecraft.world.level.block.state.BlockBehaviour;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.state.properties.BooleanProperty;

/**
 * Schreibt je Block von 26.3, der Flächen zu bestimmten Nachbarn weglässt,
 * seine Regel: das eigene skipRendering, das Block.shouldRenderFace fragt.
 * "gleich": jede Fläche zu einem Nachbarn desselben Blocks. "senkrecht":
 * nur oben und unten. "verbunden": oben und unten zu demselben Block,
 * waagrecht nur, wenn beide zueinander verbunden sind (north, east, south,
 * west); steht ein Tag dahinter, waagrecht auch zu den anderen Blöcken des
 * Tags. Die Tags bindet er wie WorldLoader. Jede Regel prüft er an allen
 * Paaren aus Zustand, Nachbarzustand und Richtung, die ganze Tabelle danach
 * an zufälligen Paaren über alle Zustände; passt etwas nicht, bricht er ab.
 * Es fehlen Flüssigkeiten (LiquidBlock), deren Flächen über die Masken der
 * Flüssigkeiten entfallen, und Blöcke, die mit den Vorgaben des Spiels
 * nichts weglassen. Beide nennt er auf stderr.
 */
public class Nachbarn {
    static final Direction[] RICHTUNGEN = Direction.values();

    record Regel(String name, Set<Block> gruppe) {}

    public static void main(String[] args) throws Exception {
        // Bootstrap leitet System.err in sein Log und damit nach System.out.
        var err = System.err;
        try {
            schreibe(err);
        } catch (IllegalStateException e) {
            err.println(e.getMessage());
            System.exit(1);
        }
    }

    /** Erst am Ende geschrieben: Bricht er ab, bleibt stdout leer. */
    static void schreibe(PrintStream err) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        try (var resources = new MultiPackResourceManager(
                PackType.SERVER_DATA, List.of(ServerPacksSource.createVanillaPackSource().fullResources()))) {
            var statisch = RegistryLayer.createRegistryAccess().getLayer(RegistryLayer.STATIC);
            TagLoader.loadTagsForExistingRegistries(resources, statisch).forEach(tags -> tags.apply());
        }
        var out = new PrintStream(System.out, true, StandardCharsets.UTF_8);

        List<BlockState> alle = new ArrayList<>();
        for (Block block : BuiltInRegistries.BLOCK) {
            alle.addAll(block.getStateDefinition().getPossibleStates());
        }
        Map<String, Integer> jeKlasse = new TreeMap<>();
        Map<Block, Regel> regeln = new LinkedHashMap<>();
        List<String> ohneWirkung = new ArrayList<>();
        Set<Block> fluessig = new LinkedHashSet<>();
        long paare = 0;
        for (Block block : BuiltInRegistries.BLOCK) {
            Class<?> klasse = erklaert(block.getClass());
            if (klasse == BlockBehaviour.class) {
                continue;
            }
            jeKlasse.merge(klasse.getSimpleName(), 1, Integer::sum);
            if (klasse == LiquidBlock.class) {
                fluessig.add(block);
                continue;
            }
            var states = block.getStateDefinition().getPossibleStates();
            var weg = new BitSet();
            Set<Block> gruppe = new LinkedHashSet<>();
            gruppe.add(block);
            for (int s = 0; s < states.size(); s++) {
                for (int t = 0; t < alle.size(); t++) {
                    for (int d = 0; d < 6; d++) {
                        if (states.get(s).skipRendering(alle.get(t), RICHTUNGEN[d])) {
                            weg.set((s * alle.size() + t) * 6 + d);
                            gruppe.add(alle.get(t).getBlock());
                        }
                    }
                }
            }
            if (weg.isEmpty()) {
                ohneWirkung.add(name(block));
                continue;
            }
            paare += weg.cardinality();
            Regel regel = null;
            for (var kandidat : List.of(
                    new Regel("gleich", Set.of(block)),
                    new Regel("senkrecht", Set.of(block)),
                    new Regel("verbunden", gruppe))) {
                if (passt(kandidat, block, states, alle, weg)) {
                    regel = kandidat;
                    break;
                }
            }
            if (regel == null) {
                throw new IllegalStateException(name(block) + ": keine Regel passt, Nachbarn " + namen(gruppe));
            }
            regeln.put(block, regel);
        }

        // Die Gruppe einer Regel "verbunden" ist ein Tag mit genau ihren
        // Blöcken, und jeder davon hat dieselbe Gruppe.
        Map<Block, String> tags = new HashMap<>();
        for (var e : regeln.entrySet()) {
            var gruppe = e.getValue().gruppe();
            if (gruppe.size() == 1) {
                continue;
            }
            var passend = BuiltInRegistries.BLOCK.getTags()
                    .filter(tag -> tag.stream().map(h -> h.value()).collect(Collectors.toSet()).equals(gruppe))
                    .map(tag -> tag.key().location())
                    .toList();
            if (passend.size() != 1) {
                throw new IllegalStateException(name(e.getKey()) + ": Tags mit genau " + namen(gruppe) + ": " + passend);
            }
            var id = passend.get(0);
            tags.put(e.getKey(), id.getNamespace().equals("minecraft") ? id.getPath() : id.toString());
            for (Block anderer : gruppe) {
                var r = regeln.get(anderer);
                if (r == null || !r.gruppe().equals(gruppe)) {
                    throw new IllegalStateException(name(anderer) + " fehlt in der Gruppe von " + name(e.getKey()));
                }
            }
        }

        // Gegenprobe der ganzen Tabelle, auch für die Blöcke ohne eigenes
        // skipRendering.
        var zufall = new Random(58);
        int proben = 10_000_000;
        for (int i = 0; i < proben; i++) {
            var s = alle.get(zufall.nextInt(alle.size()));
            var t = alle.get(zufall.nextInt(alle.size()));
            if (regeln.containsKey(s.getBlock()) && zufall.nextBoolean()) {
                // Die Hälfte der Nachbarn aus der Gruppe, sonst träfe die
                // Probe fast nie einen Fall, in dem etwas wegfällt.
                var gruppe = new ArrayList<>(regeln.get(s.getBlock()).gruppe());
                var nachbar = gruppe.get(zufall.nextInt(gruppe.size())).getStateDefinition().getPossibleStates();
                t = nachbar.get(zufall.nextInt(nachbar.size()));
            }
            var d = RICHTUNGEN[zufall.nextInt(6)];
            var regel = regeln.get(s.getBlock());
            boolean tabelle = regel != null && sagt(regel, s, t, d);
            boolean spiel = !fluessig.contains(s.getBlock()) && s.skipRendering(t, d);
            if (tabelle != spiel) {
                throw new IllegalStateException("Gegenprobe: " + s + " gegen " + t + " nach " + d + ": Spiel " + spiel);
            }
        }

        for (var e : regeln.entrySet()) {
            var tag = tags.get(e.getKey());
            out.println(name(e.getKey()) + " " + e.getValue().name() + (tag == null ? "" : " " + tag));
        }
        Map<String, Integer> jeRegel = new TreeMap<>();
        regeln.values().forEach(r -> jeRegel.merge(r.name(), 1, Integer::sum));
        err.println("Blöcke mit eigenem skipRendering: " + jeKlasse.values().stream().mapToInt(i -> i).sum() + " " + jeKlasse);
        err.println("Regeln: " + jeRegel + ", mit Tag: " + new TreeMap<>(tags.values().stream()
                .collect(Collectors.groupingBy(t -> t, Collectors.counting()))));
        err.println("Ohne Wirkung mit den Vorgaben des Spiels: " + ohneWirkung.size() + " " + ohneWirkung);
        err.println("Über die Masken der Flüssigkeiten: " + namen(fluessig));
        err.println("Zustände " + alle.size() + ", Paare mit skipRendering ohne Flüssigkeiten " + paare);
        err.println("Gegenprobe: " + proben + " zufällige Paare wie im Spiel");
    }

    /** Sagt die Regel für jedes Paar dasselbe wie das Spiel? */
    static boolean passt(Regel regel, Block block, List<BlockState> states, List<BlockState> alle, BitSet weg) {
        for (int s = 0; s < states.size(); s++) {
            for (int t = 0; t < alle.size(); t++) {
                for (int d = 0; d < 6; d++) {
                    boolean soll = weg.get((s * alle.size() + t) * 6 + d);
                    if (sagt(regel, states.get(s), alle.get(t), RICHTUNGEN[d]) != soll) {
                        return false;
                    }
                }
            }
        }
        return true;
    }

    /** Entfällt nach der Regel die Fläche von s zum Nachbarn t in Richtung d? */
    static boolean sagt(Regel regel, BlockState s, BlockState t, Direction d) {
        boolean gleich = t.getBlock() == s.getBlock();
        boolean senkrecht = d.getAxis() == Direction.Axis.Y;
        return switch (regel.name()) {
            case "gleich" -> gleich;
            case "senkrecht" -> gleich && senkrecht;
            default -> regel.gruppe().contains(t.getBlock())
                    && (senkrecht ? gleich : verbunden(s, d) && verbunden(t, d.getOpposite()));
        };
    }

    /** Ist der Zustand zur Seite d verbunden, nach der Eigenschaft ihres Namens? */
    static boolean verbunden(BlockState state, Direction d) {
        return state.getBlock().getStateDefinition().getProperty(d.getSerializedName()) instanceof BooleanProperty p
                && state.getValue(p);
    }

    /** Die Klasse, die skipRendering für diesen Block festlegt. */
    static Class<?> erklaert(Class<?> c) {
        for (; ; c = c.getSuperclass()) {
            try {
                c.getDeclaredMethod("skipRendering", BlockState.class, BlockState.class, Direction.class);
                return c;
            } catch (NoSuchMethodException e) {
                // weiter oben suchen
            }
        }
    }

    static String name(Block block) {
        return BuiltInRegistries.BLOCK.getKey(block).getPath();
    }

    static String namen(Set<Block> bloecke) {
        return bloecke.stream().map(Nachbarn::name).toList().toString();
    }
}
