"""Gibt aus, wie gross ein Binär gepackt ist, so wie es im Jar des Plugins liegt.

xz mit dem BCJ-Filter x86, davor LZMA2 mit Preset 9e und 8 MiB Wörterbuch,
Prüfsumme CRC64. Das Plugin packt mit denselben Werten; ändert sich einer,
ändern ihn beide. Siehe docs/entwicklung/weitergabe.md, „Grenze“.

    python .github/gepackt.py <binär>
"""

import lzma
import sys

FILTER = [
    {"id": lzma.FILTER_X86},
    {"id": lzma.FILTER_LZMA2, "preset": 9 | lzma.PRESET_EXTREME, "dict_size": 8 << 20},
]

with open(sys.argv[1], "rb") as datei:
    print(len(lzma.compress(datei.read(), format=lzma.FORMAT_XZ, check=lzma.CHECK_CRC64, filters=FILTER)))
