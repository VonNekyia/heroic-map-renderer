"""Welche Funktion zu welcher Phase zählt, und welche Funktionen das Profil getrennt zeigen soll.

Beides folgt dem Code: Wer eine Funktion umbenennt oder verschiebt, zieht es hier nach. Die Phasen
selbst, die Läufe, die Schwelle und die Spalten sind Kriterien der Seite docs/humankeys/renderzeit.md
und ändern sich nur auf Wunsch dessen, der sie angefordert hat (AGENTS.md, Regel 28).
Siehe skills/renderzeit-erneuern/SKILL.md.
"""
import re

# Der Name der Crate in den Symbolen und der des Binärs.
CRATE = "heroic_map_renderer::"
BINAER = "heroic-map-renderer.exe"

# Die Phasen in der Reihenfolge der Seite. Die letzten vier sind fremder Code und der Rest.
PHASEN = [
    "Vorlauf, Assets und Sprites",
    "Lesen und Dekodieren",
    "Licht",
    "Sammeln und Deckung",
    "Strahl zur Sonne",
    "Zeichnen",
    "Kodieren",
    "Pyramide",
    "Fingerabdruck und Pixel-Hash",
    "Höhen und Boden",
    "Ablegen",
    "Allokator",
    "C-Laufzeit: Kopieren, Füllen, Rechnen",
    "Kern und Dateisystem",
    "Rest",
]

# Rust-Funktionen: Die erste Regel, die auf den Namen ohne den Präfix der Crate passt, gilt.
_RUST = [
    ("Allokator", r"__rust_(alloc|dealloc|realloc)|__rdl_|__rg_|alloc::raw_vec|alloc::alloc::|RawVec|mimalloc"),
    ("Strahl zur Sonne", r"metatile::strahl::|render::sonne::|ChunkCache::(sonne|frei_|saeule|horizont|sonnen_bits|wirkung)|metatile::startpunkt"),
    ("Fingerabdruck und Pixel-Hash", r"world::chunk::Fnv|Chunk::abdruck|sha256|sha512|^ring::|ring_core|cli::pixel|render::stand::"),
    ("Vorlauf, Assets und Sprites", r"tiles::(survey|block_box)|^assets::(?!blockstate::(leuchten|schatten|lichtweg))|render::sprites::(?!SpriteSet::family_index)|"
                                    r"render::rasterizer::(rastern|Canvas|ProjectedQuad|Raster|render|zuschneiden|deckung|sample)|render::banner|^image::|^png::|fdeflate|serde_json|"
                                    r"cli::(schaetzung|datenpakete|banner|client)|^ebenen::"),
    ("Sammeln und Deckung", r"world::biomzoom|world::chunk::(Chunk|Section)::(slot|block|biome|section_index|contains_column)|world::palette::(Paletted|PackedIndices)::(get|at|index)|"
                            r"metatile::von_vorn|metatile::Deckung|metatile::(wort|columns_at|u_window|v_window|origin_of|block_origin|anchor_of|ao_seiten|kino_kanaele|hellstes|rand)\b|"
                            r"ChunkCache::(candidates|sorted_candidates|expose|edge|sprite_at|tints_at|laubfarbe|biome_of|noise_biome|licht_fuer|ecken_at|lichter_um|licht_at|slot|next_tile|"
                            r"leuchtet_im_band|himmel_at|block_at|zelle|umgebung|in_die_welt|family_at)|"
                            r"metatile::impl\$\d+::(umgebung|ecken_at|tints_at|lichter_um|licht_fuer|sprite_at|candidates|sorted_candidates|expose|edge|laubfarbe|biome_of|noise_biome|slot|"
                            r"next_tile|himmel_at|block_at|zelle|bedeckt|zeichne|licht_at|family_at|leuchtet_im_band)|"
                            r"core::slice::sort|slice::sort|render::projection|metatile::Streuer|hashbrown|core::hash|BuildHasher"),
    ("Lesen und Dekodieren", r"^world::|drop_glue<world::|drop_in_place<world::|core::str::converts|simd_adler32|flate2|adler|fastnbt|serde|miniz_oxide\w*::inflate|inflate::|lz4|cesu8|"
                             r"metatile::Loaded::new|metatile::Masks::|metatile::impl\$\d+::(new|of|packe|ebene)|sprites::SpriteSet::family_index|"
                             r"assets::blockstate::(leuchten|schatten|lichtweg)|metatile::lichtweg|metatile::flags|ChunkCache::load\b|spalten_im_blick"),
    ("Licht", r"render::licht::|ChunkCache::licht_slot|Loaded::eingabe"),
    ("Zeichnen", r"metatile::(blit|blit_sichtbar|blit_hdr|mische|mische_wasser|mische_hdr|faktor|anteile|karte|relief|helligkeit|render_flach|merke_oben|render_area|mit_relief|render_hdr|"
                 r"draw_all|Wasser|EckenLicht|Hdr)|metatile::relief::|render::kino::|render::look::|rasterizer::(darken|tinted|over|ecken_faktor|smooth_blend|Lightmap|Light)|"
                 r"pyramid::to_srgb|pyramid::to_linear"),
    ("Kodieren", r"tiles::encode_webp|tiles::richte_libwebp_ein|tiles::Packen"),
    ("Pyramide", r"render::pyramid::|tiles::decode_webp"),
    ("Höhen und Boden", r"render::heights::|miniz_oxide\w*::deflate|deflate::"),
    ("Ablegen", r"std::fs::|std::io::|cli::manifest|std::path|std::sys::(fs|pal::windows::(fs|handle))|tiles::(Gebiet|Kachel)"),
]
_RUST = [(n, re.compile(m)) for n, m in _RUST]

# C-Funktionen im Binär: der Allokator, die C-Laufzeit, SHA aus ring, der Rest ist libwebp.
_C_NAME = re.compile(r"^[A-Za-z_$][A-Za-z0-9_$]*$")
_C_LAUFZEIT = re.compile(r"^(MoveSmall\d+|SetSmall\d+|(Mov|Set)\d+(Xmm|Ymm)Blocks|mem\w+|__mem\w+|_?str(len|cmp|ncmp)|expf?|logf?|log2f?|powf?|sqrtf?|roundf?|floorf?|ceilf?|floor_sse41|"
                         r"qsort|_\w+|__\w+|malloc|free|calloc|realloc)$")
_DEKODIEREN = re.compile(r"^(VP8L?(Dec|Decode|Read|Inverse|New|Delete|Check|IoPutRow|Process|BitReader|InitBitReader|DoFill|GetHeaders|GetInfo|SetError)|VP8InitBitReader|"
                         r"WebP(Decode|InitDec|GetInfo|GetFeatures|Io|FreeDec|AllocateDec|Demux|Rescaler|InitCustomIo|ResetDec|CheckCropDim)|"
                         r"Decode|ReadSymbol|ReadHuffman|ReadTransform|ReadPacked|ExtractPaletted|ProcessRows|EmitRows|AlphaApply|PredictorAdd|PredictorInverse|PredictorsAdd|AddGreen|"
                         r"ColorIndexInverse|TransformColorInverse|CopyBlock|CopySmall|BuildPacked|BuildHuffmanTable|GetCopy|GetHTree|GetMeta|ExpandColorMap|ShiftBytes|ParseHeaders|ParseVP8|"
                         r"ParseRIFF|ParseOptional|GetFeatures|DecodeInto|AllocateInternalBuffers|AllocateAndInit|ApplyInverseTransforms|Is8bOptimizable|UpdateDecoder|ClearTransform|"
                         r"ClearMetadata|ConvertBGRAToRGBA)")


def phase(modul, name):
    """Die Phase einer Funktion aus Modul und Name, `name` ohne den Präfix der Crate."""
    if modul.lower() != BINAER:
        return "Kern und Dateisystem"
    if "::" not in name and _C_NAME.match(name):
        if re.match(r"^_?mi_", name):
            return "Allokator"
        if re.match(r"^L\$\w*sha|^sha(256|512)", name):
            return "Fingerabdruck und Pixel-Hash"
        if name.startswith("_R"):
            return "Rest"
        if _C_LAUFZEIT.match(name):
            return "C-Laufzeit: Kopieren, Füllen, Rechnen"
        return "Pyramide" if _DEKODIEREN.match(name) else "Kodieren"
    for n, m in _RUST:
        if m.search(name):
            return n
    return "Rest"


def fremdes_modul(modul):
    """Eine Gruppe statt des Namens: Namen von Treibern verraten die Hardware (AGENTS.md, Regel 20)."""
    m = modul.lower()
    if m in ("ntoskrnl.exe", "hal.dll"):
        return "Kern von Windows"
    if m in ("ntdll.dll", "kernelbase.dll", "kernel32.dll", "ucrtbase.dll"):
        return "Laufzeit von Windows"
    if m in ("wdfilter.sys", "mpengine.dll"):
        return "Echtzeitschutz"
    if m in ("ntfs.sys", "fltmgr.sys", "fileinfo.sys", "wof.sys", "wcifs.sys", "storport.sys", "stornvme.sys", "classpnp.sys", "filecrypt.sys", "volmgr.sys",
             "partmgr.sys", "volsnap.sys", "disk.sys", "cldflt.sys", "bindflt.sys", "luafv.sys"):
        return "Dateisystem und Platte"
    return "übrige Treiber"


# Vor diese Funktionen setzt laeufe.py im Wegwerf-Worktree `#[inline(never)]`, damit das Profil sie
# getrennt zeigt. Je Datei unter renderer/ reguläre Ausdrücke auf den Zeilenanfang; jeder muss genau
# einmal passen. Fehlt eine Funktion, bricht laeufe.py ab und nennt sie.
MARKEN = {
    "src/world/chunk.rs": [r"    pub fn decode\(nbt", r"fn decode_section\("],
    "src/world/palette.rs": [r"    pub fn new\(name: impl", r"    pub fn mit_vorgaben\("],
    "src/world/region.rs": [r"    fn chunk_nbt\(", r"fn decompress\("],
    "src/world/mod.rs": [r"    pub fn chunk\(&self, cx"],
    "src/render/metatile.rs": [
        r"fn von_vorn<", r"fn blit\($", r"fn blit_sichtbar\($", r"fn lichtweg\(state", r"    fn of\($",
        r"    fn new\(chunk: Rc<Chunk>", r"    fn load\(&mut self", r"    fn licht_slot\(", r"    fn expose\(",
        r"    fn sorted_candidates\($", r"    fn sprite_at\(",
    ],
    "src/render/sprites.rs": [r"    pub fn family_index\("],
    "src/assets/blockstate.rs": [r"pub fn leuchten\(", r"pub fn lichtweg\(", r"pub fn schatten\("],
    "src/render/licht.rs": [r"    pub fn chunk\(&mut self"],
    "src/render/tiles.rs": [r"pub fn encode_webp\("],
}
