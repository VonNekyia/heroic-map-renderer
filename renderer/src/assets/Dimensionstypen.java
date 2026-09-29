import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.util.Comparator;
import java.util.List;

import net.minecraft.SharedConstants;
import net.minecraft.core.Holder;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.attribute.EnvironmentAttribute;
import net.minecraft.world.attribute.EnvironmentAttributeMap;
import net.minecraft.world.attribute.EnvironmentAttributes;
import net.minecraft.world.level.dimension.DimensionType;

/**
 * Schreibt für 26.2 die Dimensionstypen des Spiels, wie
 * DimensionTypes.bootstrap sie über VanillaRegistries.createLookup anlegt,
 * mit dem, was der Renderer von ihnen braucht. Die Zeilen der Tabelle:
 *
 * vorgabe attribut wert: der Wert, den EnvironmentAttributes für ein Attribut
 *     vorgibt, wenn kein Dimensionstyp es setzt.
 * typ id has_skylight=… cardinal_light=… [attribut=wert]...: ein
 *     Dimensionstyp, mit den Attributen, die er setzt, auf ihre Vorgabe
 *     angewandt wie der Constant-Layer in EnvironmentAttributeSystem.
 *
 * Farben stehen als #rrggbb, Zahlen wie Float.toString: so, wie ein
 * Datenpaket sie schreiben darf.
 */
public class Dimensionstypen {
    public static void main(String[] args) {
        // Bootstrap leitet System.err in sein Log und damit nach System.out.
        var err = System.err;
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        // Die Attribute, die der Renderer liest: die der Lightmap. Erst nach
        // dem Bootstrap, sonst legt EnvironmentAttributes sie zu früh an.
        List<EnvironmentAttribute<?>> attribute = List.of(
                EnvironmentAttributes.AMBIENT_LIGHT_COLOR,
                EnvironmentAttributes.SKY_LIGHT_FACTOR,
                EnvironmentAttributes.SKY_LIGHT_COLOR,
                EnvironmentAttributes.BLOCK_LIGHT_TINT);
        var out = new PrintStream(System.out, true, StandardCharsets.UTF_8);
        // Zeilen enden mit \n wie in den anderen Tabellen, auch unter Windows.
        java.util.function.Consumer<String> zeile = text -> out.print(text + "\n");
        for (var attribut : attribute) {
            zeile.accept("vorgabe " + id(attribut) + " " + wert(attribut.defaultValue()));
        }
        var typen = VanillaRegistries.createLookup().lookupOrThrow(Registries.DIMENSION_TYPE)
                .listElements()
                .sorted(Comparator.comparing(typ -> typ.key().identifier().toString()))
                .toList();
        for (Holder.Reference<DimensionType> typ : typen) {
            var t = typ.value();
            var text = new StringBuilder("typ " + typ.key().identifier() + " has_skylight=" + t.hasSkyLight()
                    + " cardinal_light=" + t.cardinalLightType().getSerializedName());
            for (var attribut : attribute) {
                if (t.attributes().contains(attribut)) {
                    text.append(" " + id(attribut) + "=" + wert(angewandt(t.attributes(), attribut)));
                }
            }
            zeile.accept(text.toString());
        }
        err.println("Dimensionstypen: " + typen.size());
    }

    static <V> V angewandt(EnvironmentAttributeMap map, EnvironmentAttribute<V> attribut) {
        return map.applyModifier(attribut, attribut.defaultValue());
    }

    static String id(EnvironmentAttribute<?> attribut) {
        return BuiltInRegistries.ENVIRONMENT_ATTRIBUTE.getKey(attribut).toString();
    }

    static String wert(Object wert) {
        return switch (wert) {
            case Integer farbe -> String.format("#%06x", farbe & 0xffffff);
            case Float zahl -> Float.toString(zahl);
            default -> throw new AssertionError("unerwarteter Wert " + wert);
        };
    }
}
