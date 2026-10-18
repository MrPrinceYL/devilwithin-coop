"""Decode the texts of a Devil Within "CAP" message file (BDPK entry type 5, `cap_teg_msg_*`).

    python tools/dw_cap_text.py <pack.pak> <entry name>

Layout (cap_teg_msg_font_menutext): "CAP\\0", +4 glyph count, +0x10 offsets; glyph records of
16 bytes (+0xE character code); the texts are 16-bit codes: >= 0x100 a glyph (all
characters code + 0xE1), 2 space, 3 line break, 0 end of a text,
others control codes (shown as <n>).
"""
import struct
import sys

sys.path.insert(0, __file__.rsplit('tools', 1)[0] + 'tools')
import tim2png  # noqa: E402


def text(b, o):
    """The text at byte `o` up to its end code."""
    cur = ''
    while o + 1 < len(b):
        v, = struct.unpack_from('<H', b, o)
        o += 2
        if v == 0:
            break
        if v == 2:
            cur += ' '
        elif v == 3:
            cur += chr(10)
        elif v >= 0x100:
            # Every character is stored as code + 0xE1 (menu texts in capitals).
            c = v - 0xE1
            cur += chr(c) if 0x20 <= c < 0x7F else f'<g{v:#x}>'
        else:
            cur += f'<{v}>'
    return cur


def main(pak, name):
    b = tim2png.entry(pak, name)
    index, texts = struct.unpack_from('<II', b, 0x18)
    # Index from +0x18: 8 bytes per message {u16 offset in 16-bit units, u16 100, u16 number, ..}.
    o = index
    while o + 8 <= texts:
        off, _, mid = struct.unpack_from("<HHH", b, o)
        print(f'--- message {mid} (+{off})')
        print(text(b, texts + off * 2))
        o += 8


if __name__ == '__main__':
    main(sys.argv[1], sys.argv[2])
