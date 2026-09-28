import java.io.PrintStream;
import java.lang.reflect.Field;
import java.lang.reflect.InvocationHandler;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.IdentityHashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;

import com.mojang.blaze3d.vertex.PoseStack;
import com.mojang.blaze3d.vertex.VertexConsumer;
import net.minecraft.SharedConstants;
import net.minecraft.client.model.Model;
import net.minecraft.client.model.geom.EntityModelSet;
import net.minecraft.client.renderer.Sheets;
import net.minecraft.client.renderer.SpriteMapper;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.blockentity.BlockEntityRenderer;
import net.minecraft.client.renderer.blockentity.BlockEntityRendererProvider;
import net.minecraft.client.renderer.blockentity.BlockEntityRenderers;
import net.minecraft.client.renderer.blockentity.ChestRenderer;
import net.minecraft.client.renderer.blockentity.DecoratedPotRenderer;
import net.minecraft.client.renderer.blockentity.state.BannerRenderState;
import net.minecraft.client.renderer.blockentity.state.BlockEntityRenderState;
import net.minecraft.client.renderer.blockentity.state.ChestRenderState;
import net.minecraft.client.renderer.blockentity.state.DecoratedPotRenderState;
import net.minecraft.client.renderer.rendertype.RenderType;
import net.minecraft.client.renderer.state.level.CameraRenderState;
import net.minecraft.client.renderer.texture.TextureAtlasSprite;
import net.minecraft.client.resources.model.sprite.SpriteGetter;
import net.minecraft.client.resources.model.sprite.SpriteId;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Holder;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.item.DyeColor;
import net.minecraft.world.item.Item;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.ChestBlock;
import net.minecraft.world.level.block.EntityBlock;
import net.minecraft.world.level.block.entity.BannerPattern;
import net.minecraft.world.level.block.entity.BannerPatternLayers;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.entity.BlockEntityType;
import net.minecraft.world.level.block.entity.PotDecorations;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.state.properties.ChestType;
import net.minecraft.world.phys.Vec3;
import org.joml.Matrix4f;
import org.joml.Vector3f;

/**
 * Schreibt für 26.2, was die Blockentity-Renderer des Spiels je Zustand mit
 * einem Modell zeichnen. Jeder Renderer läuft mit seinem eigenen submit
 * gegen einen Collector, der mitschreibt, und der zeichnet wie
 * ModelFeatureRenderer.prepareModel: setupAnim, dann renderToBuffer, ohne
 * Atlas, die UV also im Raum der Textur. Die Zeilen der Tabelle:
 *
 * schicht i name [alpha=a] [beidseitig] [je_seite] [gemischt]: eine
 *     RenderType mit dem, was ihre Pipeline festlegt: ALPHA_CUTOUT, kein
 *     Culling, PER_FACE_LIGHTING, Mischen.
 * textur i id
 * form i n: danach n Zeilen mit je vier Ecken x y z u v, die Flächen eines
 *     Modells nach setupAnim, bevor die Lage des submit dazukommt.
 * lage i: die Matrix eines submit, drei Zeilen zu vier Zahlen.
 * bild i zeichnung...: was ein Zustand zeichnet, in der Reihenfolge des
 *     Spiels. Eine Zeichnung ist form/lage/textur/schicht/farbe/rolle, die
 *     Farbe ARGB. Die Rolle ist - oder muster, wenn die Bannermuster aus den
 *     Blockentity-Daten danach kommen, oder scherbeN mit N als Platz in
 *     sherds.
 * block name bild...: je Zustand in der Reihenfolge von getPossibleStates
 *     ein Bild oder -, eines, wenn alle gleich sind.
 * farbstoff name rrggbb: getTextureDiffuseColor.
 * scherbe item textur: DecoratedPotRenderer.DECORATED_POT_SPRITES.
 * muster präfix n: Sheets.BANNER_MAPPER, davor steht der Namensraum der
 *     asset_id, dahinter ihr Pfad; höchstens n Lagen zeichnet der Renderer.
 *
 * Zahlen stehen, wie Float.toString sie schreibt, und kommen beim Lesen
 * genau so zurück. Auf stderr steht, welcher Renderer nichts aus einem
 * Modell zeichnet oder ohne Spiel nicht läuft.
 */
public class Blockentities {
    /** Ein submitModel: im Raum des Modells, mit der Matrix des Aufrufs. */
    record Zeichnung(RenderType schicht, String textur, int farbe, List<float[]> ecken, Matrix4f lage, Object teil) {}

    public static void main(String[] args) throws Exception {
        // Bootstrap leitet System.err in sein Log und damit nach System.out.
        var err = System.err;
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var out = new PrintStream(System.out, true, StandardCharsets.UTF_8);

        // Ein Sprite ohne Bild, das nur für seine SpriteId steht: Die Renderer
        // reichen es nur weiter, gezeichnet wird ohne Atlas.
        Field theUnsafe = sun.misc.Unsafe.class.getDeclaredField("theUnsafe");
        theUnsafe.setAccessible(true);
        var unsafe = (sun.misc.Unsafe) theUnsafe.get(null);
        var spriteIds = new IdentityHashMap<TextureAtlasSprite, SpriteId>();
        SpriteGetter sprites = id -> {
            try {
                var sprite = (TextureAtlasSprite) unsafe.allocateInstance(TextureAtlasSprite.class);
                spriteIds.put(sprite, id);
                return sprite;
            } catch (InstantiationException e) {
                throw new IllegalStateException(e);
            }
        };
        var context = new BlockEntityRendererProvider.Context(
                null, null, null, null, EntityModelSet.vanilla(), null, sprites, null);
        var renderers = new LinkedHashMap<BlockEntityType<?>, BlockEntityRenderer<?, ?>>();
        for (var eintrag : privat(BlockEntityRenderers.class, "PROVIDERS",
                Map.<BlockEntityType<?>, BlockEntityRendererProvider<?, ?>>of()).entrySet()) {
            renderers.put(eintrag.getKey(), eintrag.getValue().create(context));
        }

        var zeichnungen = new ArrayList<Zeichnung>();
        var anderes = new TreeMap<String, String>();
        var renderer = new Object[1];
        InvocationHandler handler = (proxy, method, a) -> {
            if (method.getName().equals("order")) {
                return proxy;
            }
            // Hier kommen alle Wege an, die ein Modell zeichnen.
            if (method.getName().equals("submitModel") && a.length == 10 && a[3] instanceof RenderType schicht) {
                @SuppressWarnings("unchecked")
                var model = (Model<Object>) a[0];
                var pose = ((PoseStack) a[2]).last().copy();
                int light = (int) a[4], overlay = (int) a[5], farbe = (int) a[6];
                var sprite = (TextureAtlasSprite) a[7];
                String textur = sprite != null ? spriteIds.get(sprite).texture().toString() : sampler0(schicht);
                model.setupAnim(a[1]);
                var ecken = aufnehmen(model, new PoseStack(), light, overlay, farbe);
                var stack = new PoseStack();
                stack.last().set(pose);
                pruefen(ecken, aufnehmen(model, stack, light, overlay, farbe), pose.pose());
                zeichnungen.add(new Zeichnung(schicht, textur, farbe, ecken, new Matrix4f(pose.pose()), model.root()));
                return null;
            }
            if (method.isDefault()) {
                return InvocationHandler.invokeDefault(proxy, method, a);
            }
            anderes.merge(renderer[0].getClass().getSimpleName(), method.getName(),
                    (x, y) -> x.contains(y) ? x : x + ", " + y);
            return null;
        };
        var collector = (SubmitNodeCollector) Proxy.newProxyInstance(
                SubmitNodeCollector.class.getClassLoader(), new Class<?>[] {SubmitNodeCollector.class}, handler);
        var kamera = new CameraRenderState();
        Method material = ChestRenderer.class.getDeclaredMethod("getChestMaterial", BlockEntity.class, boolean.class);
        material.setAccessible(true);
        Map<ResourceKey<Item>, SpriteId> scherben = privat(DecoratedPotRenderer.class, "DECORATED_POT_SPRITES", Map.of());
        SpriteMapper muster = privat(Sheets.class, "BANNER_MAPPER", (SpriteMapper) null);

        var tabelle = new Tabelle();
        var fehler = new TreeMap<String, String>();
        var bloecke = new ArrayList<String>();
        int lagen = 0;
        for (Block block : BuiltInRegistries.BLOCK) {
            if (!(block instanceof EntityBlock entityBlock)) {
                continue;
            }
            String name = BuiltInRegistries.BLOCK.getKey(block).getPath();
            var bilder = new ArrayList<String>();
            for (BlockState state : block.getStateDefinition().getPossibleStates()) {
                String bild = "-";
                renderer[0] = null;
                try {
                    var be = entityBlock.newBlockEntity(BlockPos.ZERO, state);
                    @SuppressWarnings("unchecked")
                    var r = be == null ? null
                            : (BlockEntityRenderer<BlockEntity, BlockEntityRenderState>) renderers.get(be.getType());
                    if (r != null) {
                        renderer[0] = r;
                        var rs = r.createRenderState();
                        r.extractRenderState(be, rs, 0f, Vec3.ZERO, null);
                        if (rs instanceof ChestRenderState truhe) {
                            // Ohne Welt nimmt extractRenderState die Truhe als
                            // Gegenstand: einfach, nach Süden, im Material nach
                            // dem Datum. Art und Lage kommen wie im Zweig mit
                            // Welt aus dem Zustand, das Material ohne Weihnachten.
                            truhe.type = state.hasProperty(ChestBlock.TYPE) ? state.getValue(ChestBlock.TYPE) : ChestType.SINGLE;
                            truhe.facing = state.getValue(ChestBlock.FACING);
                            truhe.material = (ChestRenderState.ChestMaterialType) material.invoke(null, be, false);
                        }
                        zeichnungen.clear();
                        r.submit(rs, new PoseStack(), collector, kamera);
                        var eigene = new ArrayList<>(zeichnungen);
                        var rollen = new String[eigene.size()];
                        java.util.Arrays.fill(rollen, "-");
                        if (rs instanceof DecoratedPotRenderState krug) {
                            scherbenPlaetze(r, krug, eigene, rollen, scherben, collector, kamera, zeichnungen);
                        }
                        if (rs instanceof BannerRenderState banner) {
                            int n = musterRegel(r, banner, eigene, rollen, muster, collector, kamera, zeichnungen);
                            if (lagen != 0 && n != lagen) {
                                throw new IllegalStateException("Höchstzahl der Muster schwankt");
                            }
                            lagen = n;
                        }
                        if (!eigene.isEmpty()) {
                            bild = String.valueOf(tabelle.bild(eigene, rollen));
                        }
                    }
                } catch (Throwable e) {
                    fehler.putIfAbsent(renderer[0] == null ? name : renderer[0].getClass().getSimpleName(),
                            name + ": " + e);
                }
                bilder.add(bild);
            }
            if (bilder.stream().allMatch("-"::equals)) {
                continue;
            }
            bloecke.add("block " + name + " " + (bilder.stream().distinct().count() == 1 ? bilder.get(0) : String.join(" ", bilder)));
        }

        // Zeilen enden mit \n wie in den anderen Tabellen, auch unter Windows.
        java.util.function.Consumer<String> zeile = text -> out.print(text + "\n");
        tabelle.schreiben(zeile);
        bloecke.forEach(zeile);
        for (DyeColor farbstoff : DyeColor.values()) {
            zeile.accept(String.format("farbstoff %s %06x", farbstoff.getName(), farbstoff.getTextureDiffuseColor() & 0xffffff));
        }
        scherben.entrySet().stream().sorted(Comparator.comparing(e -> e.getKey().identifier().toString()))
                .forEach(e -> zeile.accept("scherbe " + e.getKey().identifier() + " " + e.getValue().texture()));
        zeile.accept("muster " + muster.prefix() + " " + lagen);

        err.println("Blöcke: " + bloecke.size() + ", Bilder: " + tabelle.bilder.size() + ", Formen: " + tabelle.formen.size()
                + " mit " + tabelle.flaechen + " Flächen, Lagen: " + tabelle.lagen.size() + ", Texturen: " + tabelle.texturen.size());
        err.println("Kein Modell, sondern: " + anderes);
        err.println("Ohne Spiel nicht gelaufen: " + fehler);
    }

    /** Zeichnet wie ModelFeatureRenderer.prepareModel, ohne Sprite: je Ecke x y z u v. */
    static List<float[]> aufnehmen(Model<Object> model, PoseStack stack, int light, int overlay, int farbe) {
        var ecken = new ArrayList<float[]>();
        var aufnahme = (VertexConsumer) Proxy.newProxyInstance(
                VertexConsumer.class.getClassLoader(), new Class<?>[] {VertexConsumer.class},
                (p, m, v) -> {
                    if (m.getName().equals("addVertex") && v.length == 11) {
                        ecken.add(new float[] {(float) v[0], (float) v[1], (float) v[2], (float) v[4], (float) v[5]});
                        return null;
                    }
                    throw new UnsupportedOperationException(m.toString());
                });
        model.renderToBuffer(stack, aufnahme, light, overlay, farbe);
        if (ecken.size() % 4 != 0) {
            throw new IllegalStateException("keine Vierecke");
        }
        return ecken;
    }

    /** Die Lage mal dem Raum des Modells muss geben, was das Spiel zeichnet, und kein Spiegel sein. */
    static void pruefen(List<float[]> modell, List<float[]> gezeichnet, Matrix4f lage) {
        if (lage.determinant() <= 0) {
            throw new IllegalStateException("Lage spiegelt");
        }
        for (int i = 0; i < modell.size(); i++) {
            float[] m = modell.get(i), g = gezeichnet.get(i);
            var p = lage.transformPosition(new Vector3f(m[0], m[1], m[2]));
            if (Math.abs(p.x - g[0]) > 1e-4 || Math.abs(p.y - g[1]) > 1e-4 || Math.abs(p.z - g[2]) > 1e-4
                    || m[3] != g[3] || m[4] != g[4]) {
                throw new IllegalStateException("Lage passt nicht");
            }
        }
    }

    /**
     * Welche Zeichnung des Krugs welche Scherbe trägt: noch einmal mit vier
     * verschiedenen Scherben, gelesen wie sherds im Chunk.
     */
    static void scherbenPlaetze(BlockEntityRenderer<BlockEntity, BlockEntityRenderState> r, DecoratedPotRenderState krug,
            List<Zeichnung> eigene, String[] rollen, Map<ResourceKey<Item>, SpriteId> scherben,
            SubmitNodeCollector collector, CameraRenderState kamera, List<Zeichnung> zeichnungen) {
        var items = scherben.keySet().stream().sorted(Comparator.comparing(k -> k.identifier().toString())).limit(4).toList();
        var liste = items.stream().map(k -> k.identifier().toString()).toList();
        krug.decorations = PotDecorations.CODEC.parse(com.mojang.serialization.JavaOps.INSTANCE, liste).getOrThrow();
        zeichnungen.clear();
        r.submit(krug, new PoseStack(), collector, kamera);
        krug.decorations = PotDecorations.EMPTY;
        int gefunden = 0;
        for (int i = 0; i < eigene.size(); i++) {
            for (int platz = 0; platz < 4; platz++) {
                var sprite = scherben.get(items.get(platz));
                if (zeichnungen.get(i).textur().equals(sprite.texture().toString())) {
                    rollen[i] = "scherbe" + platz;
                    gefunden++;
                }
            }
        }
        if (zeichnungen.size() != eigene.size() || gefunden != 4) {
            throw new IllegalStateException("Scherben nicht zugeordnet");
        }
    }

    /**
     * Die Regel für Bannermuster aus den Blockentity-Daten, am Spiel geprüft:
     * Jede Lage kommt als weitere Zeichnung wie die letzte des Banners dazu,
     * mit der Textur ihres Musters und der Farbe ihres Farbstoffs, aber nur
     * bis zu einer Höchstzahl. Geprüft mit 64 Lagen, liefert sie diese Zahl.
     */
    static int musterRegel(BlockEntityRenderer<BlockEntity, BlockEntityRenderState> r, BannerRenderState banner,
            List<Zeichnung> eigene, String[] rollen, SpriteMapper muster,
            SubmitNodeCollector collector, CameraRenderState kamera, List<Zeichnung> zeichnungen) {
        var ids = List.of(Identifier.withDefaultNamespace("stripe_top"), Identifier.withDefaultNamespace("cross"));
        var lagen = new ArrayList<BannerPatternLayers.Layer>();
        for (int i = 0; i < 64; i++) {
            lagen.add(new BannerPatternLayers.Layer(Holder.direct(new BannerPattern(ids.get(i % ids.size()), "")),
                    DyeColor.values()[i % DyeColor.values().length]));
        }
        banner.patterns = new BannerPatternLayers(lagen);
        zeichnungen.clear();
        r.submit(banner, new PoseStack(), collector, kamera);
        banner.patterns = BannerPatternLayers.EMPTY;
        var letzte = eigene.get(eigene.size() - 1);
        int hoechstens = zeichnungen.size() - eigene.size();
        boolean regel = hoechstens > 0 && hoechstens < lagen.size();
        for (int i = 0; regel && i < hoechstens; i++) {
            var z = zeichnungen.get(eigene.size() + i);
            var lage = lagen.get(i);
            regel = z.schicht() == letzte.schicht() && z.lage().equals(letzte.lage()) && z.teil() == letzte.teil()
                    && z.textur().equals(muster.apply(lage.pattern().value().assetId()).texture().toString())
                    && z.farbe() == (lage.color().getTextureDiffuseColor() | 0xff000000)
                    && java.util.Arrays.deepEquals(z.ecken().toArray(), letzte.ecken().toArray());
        }
        if (!regel) {
            throw new IllegalStateException("Muster folgen nicht der letzten Zeichnung");
        }
        rollen[eigene.size() - 1] = "muster";
        return hoechstens;
    }

    /** Der Ort der Textur einer RenderType ohne Sprite, als ID wie die Sprites. */
    static String sampler0(RenderType schicht) throws ReflectiveOperationException {
        Object setup = feld(RenderType.class, "state").get(schicht);
        var bindung = ((Map<?, ?>) feld(setup.getClass(), "textures").get(setup)).get("Sampler0");
        Method ort = bindung.getClass().getDeclaredMethod("location");
        ort.setAccessible(true);
        var id = (Identifier) ort.invoke(bindung);
        return id.getNamespace() + ":" + id.getPath().replaceFirst("^textures/", "").replaceFirst("\\.png$", "");
    }

    static Field feld(Class<?> klasse, String name) throws NoSuchFieldException {
        Field f = klasse.getDeclaredField(name);
        f.setAccessible(true);
        return f;
    }

    @SuppressWarnings("unchecked")
    static <T> T privat(Class<?> klasse, String name, T muster) throws ReflectiveOperationException {
        return (T) feld(klasse, name).get(null);
    }

    /** Eine Zahl, wie Float.toString sie schreibt, ohne ein ".0" am Ende. */
    static String zahl(float f) {
        return Float.toString(f).replaceFirst("\\.0$", "");
    }

    /** Name und Pipeline einer RenderType, wie sie in der Zeile schicht stehen. */
    static String schicht(RenderType schicht) throws ReflectiveOperationException {
        var pipeline = schicht.pipeline();
        var zeile = new StringBuilder((String) feld(RenderType.class, "name").get(schicht));
        var werte = pipeline.getShaderDefines().values();
        if (werte.containsKey("ALPHA_CUTOUT")) {
            zeile.append(" alpha=").append(werte.get("ALPHA_CUTOUT"));
        }
        if (!pipeline.isCull()) {
            zeile.append(" beidseitig");
        }
        if (pipeline.getShaderDefines().flags().contains("PER_FACE_LIGHTING")) {
            zeile.append(" je_seite");
        }
        if (pipeline.getColorTargetState().blendFunction().isPresent()) {
            zeile.append(" gemischt");
        }
        return zeile.toString();
    }

    /** Formen, Lagen, Texturen, Schichten und Bilder, jedes einmal. */
    static class Tabelle {
        final Map<String, Integer> schichten = new LinkedHashMap<>();
        final Map<String, Integer> texturen = new LinkedHashMap<>();
        final Map<String, Integer> formen = new LinkedHashMap<>();
        final Map<String, Integer> lagen = new LinkedHashMap<>();
        final Map<String, Integer> bilder = new LinkedHashMap<>();
        int flaechen;

        int bild(List<Zeichnung> zeichnungen, String[] rollen) throws ReflectiveOperationException {
            var bild = new StringBuilder();
            for (int i = 0; i < zeichnungen.size(); i++) {
                var z = zeichnungen.get(i);
                if (z.farbe() >>> 24 != 0xff) {
                    throw new IllegalStateException("Farbe mit Alpha");
                }
                var form = new StringBuilder();
                for (int e = 0; e < z.ecken().size(); e++) {
                    form.append(e % 4 == 0 ? "\n" : " ");
                    float[] ecke = z.ecken().get(e);
                    for (int k = 0; k < 5; k++) {
                        form.append(k == 0 ? "" : " ").append(zahl(ecke[k]));
                    }
                }
                if (!formen.containsKey(form.toString())) {
                    flaechen += z.ecken().size() / 4;
                }
                int f = formen.computeIfAbsent(form.toString(), k -> formen.size());
                var m = z.lage();
                String lage = zahl(m.m00()) + " " + zahl(m.m10()) + " " + zahl(m.m20()) + " " + zahl(m.m30()) + "\n"
                        + zahl(m.m01()) + " " + zahl(m.m11()) + " " + zahl(m.m21()) + " " + zahl(m.m31()) + "\n"
                        + zahl(m.m02()) + " " + zahl(m.m12()) + " " + zahl(m.m22()) + " " + zahl(m.m32());
                int l = lagen.computeIfAbsent(lage, k -> lagen.size());
                int t = texturen.computeIfAbsent(z.textur(), k -> texturen.size());
                int s = schichten.computeIfAbsent(schicht(z.schicht()), k -> schichten.size());
                bild.append(i == 0 ? "" : " ").append(f).append('/').append(l).append('/').append(t).append('/').append(s)
                        .append('/').append(String.format("%08x", z.farbe())).append('/').append(rollen[i]);
            }
            return bilder.computeIfAbsent(bild.toString(), k -> bilder.size());
        }

        void schreiben(java.util.function.Consumer<String> zeile) {
            schichten.forEach((schicht, i) -> zeile.accept("schicht " + i + " " + schicht));
            texturen.forEach((textur, i) -> zeile.accept("textur " + i + " " + textur));
            formen.forEach((form, i) -> zeile.accept("form " + i + " " + form.chars().filter(c -> c == '\n').count() + form));
            lagen.forEach((lage, i) -> zeile.accept("lage " + i + "\n" + lage));
            bilder.forEach((bild, i) -> zeile.accept("bild " + i + " " + bild));
        }
    }
}
