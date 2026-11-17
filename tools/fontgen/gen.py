"""Generate the LARUS display fonts (u8g2 format) from the OFL TrueType files.

    python3 gen.py <ttf dir> <out.rs>

Values: B612 Bold (Airbus cockpit font), texts: Barlow Semi Condensed
SemiBold. Rasterised without anti-aliasing (the display draws 1 bit glyphs) from an
8 times larger drawing with the baseline on a pixel edge (SS), sized so that the digit "0" (values) resp. the capital "H" (texts) has the
same ink height as the u8g2 font it replaces. Character set: ASCII plus
degree, squared, cubed - all the firmware strings use.
"""
import sys
from PIL import Image, ImageDraw, ImageFont
from u8g2fmt import encode, decode

ASCII = [c for c in range(32, 127)] + [0xB0, 0xB2, 0xB3]
NUMERIC = [ord(c) for c in ' *+,-./0123456789:'] + [0xB0]

VALUES = [16, 19, 21, 24, 27, 32, 38, 42, 53]
TEXTS = [11, 12, 14, 18, 22, 26, 30]


# supersampling of the rasteriser: the glyph is drawn SS times larger with
# its baseline exactly on a pixel edge and reduced by majority vote. Drawn
# directly at the small size, the baseline of B612 fell between two pixel
# rows and the flat letters (n, m, r, 1, 4 ...) sat one pixel higher than the
# round ones (o, e, 0 ...): the text ran in waves (Stefan 09.11.2026).
SS = 8
# share of a pixel that must be ink (majority vote; a little below one half
# keeps the thin joins of the bold letters)
COVER = 0.45


def glyph(font, ch):
    """font: the font at SS times the size (see `big`)"""
    x0, y0, x1, y1 = font.getbbox(ch, anchor='ls', mode='1')
    adv = int(round(font.getlength(ch, mode='1') / SS))
    if x1 - x0 <= 0 or y1 - y0 <= 0:
        return (0, 0, 0, 0, adv, [])
    # origin of the pen on a pixel corner of the small grid
    m = 2 * SS
    ox = m - (x0 // SS) * SS
    oy = m + ((-y0 + SS - 1) // SS) * SS
    W = ((ox + x1 + m) // SS + 1) * SS
    H = ((oy + y1 + m) // SS + 1) * SS
    im = Image.new('L', (W, H), 0)
    d = ImageDraw.Draw(im)
    d.fontmode = 'L'
    d.text((ox, oy), ch, font=font, fill=255, anchor='ls')
    small = im.resize((W // SS, H // SS), Image.BOX)
    px = small.load()
    thr = int(255 * COVER)
    on = lambda x, y: px[x, y] >= thr
    rows = [y for y in range(small.height) if any(on(x, y) for x in range(small.width))]
    cols = [x for x in range(small.width) if any(on(x, y) for y in range(small.height))]
    if not rows:
        return (0, 0, 0, 0, adv, [])
    ry0, ry1, cx0, cx1 = rows[0], rows[-1], cols[0], cols[-1]
    gw, gh = cx1 - cx0 + 1, ry1 - ry0 + 1
    bits = [1 if on(x, y) else 0 for y in range(ry0, ry1 + 1) for x in range(cx0, cx1 + 1)]
    bx, by = ox // SS, oy // SS        # pen position in the small grid
    xoff = cx0 - bx
    top = ry0 - by                     # relative to the baseline, y down
    yoff = -(top + gh)                 # bottom edge above the baseline (u8g2)
    return (gw, gh, xoff, yoff, adv, bits)


def ink(font, ch):
    g = glyph(font, ch)
    return g[1]


def big(path, size):
    return ImageFont.truetype(path, size * SS)


def sized(path, probe, target):
    """the font (at SS times the size) whose capital E is exactly `target` px
    high: the cap height lies on a pixel edge, so all flat tops end in the
    same row. E, not H: the stems of B612 (H, 1, 4) reach 1.7 % above its
    bars (E, T, Z, 3, 5, 7) - folded down to the bars (`fold`)."""
    ref = ImageFont.truetype(path, 1000)
    x0, y0, x1, y1 = ref.getbbox('E', anchor='ls')
    cap = (y1 - y0) / 1000.0
    f = big(path, target / cap)
    assert ink(f, 'E') == target, (path, target, ink(f, 'E'))
    return f


# glyphs that reach below the baseline by design
DESC = set('gjpqyQ,;()[]{}|@$_')
LOWER_X = set('acemnorsuvwxz')
CAPS = set('ABCDEFGHIJKLMNOPQRSTUVWXYZ')
DIGITS = set('0123456789')
ASC = set('bdhkl')


def fold(g, top_ref):
    """Overshoot suppression for a 1 bit display: round letters and stems
    reach one pixel below the baseline resp. above the flat bars (o vs n,
    0 vs 7, H vs E),
    which reads as a wavy line at these sizes. That row is folded into its
    neighbour (OR), the letter keeps its shape and sits on the line."""
    w, h, x, y, a, bits = g
    if w == 0 or h == 0:
        return g
    rows = [bits[i * w:(i + 1) * w] for i in range(h)]
    if y < 0 and len(rows) > 2:
        for _ in range(-y):
            last = rows.pop()
            rows[-1] = [p | q for p, q in zip(rows[-1], last)]
            y += 1
    top = y + len(rows)
    if top_ref is not None and len(rows) > 2:
        while top > top_ref:
            first = rows.pop(0)
            rows[0] = [p | q for p, q in zip(rows[0], first)]
            top -= 1
    return (w, len(rows), x, y, a, [b for r in rows for b in r])


def glyph_set(f, charset):
    raw = {c: glyph(f, chr(c)) for c in charset}
    top = lambda ch: glyph(f, ch)[3] + glyph(f, ch)[1]
    # the flat bars: z for the small letters, E for capitals and digits
    x_ref, cap_ref, asc_ref = top('z'), top('E'), top('l')
    out = {}
    for c, g in raw.items():
        ch = chr(c)
        if ch in DESC or not (ch in LOWER_X or ch in CAPS or ch in DIGITS or ch in ASC):
            out[c] = g
            continue
        ref = x_ref if ch in LOWER_X else asc_ref if ch in ASC else cap_ref
        out[c] = fold(g, ref)
    return out


# Barlow: drawn directly at the small size like up to 0.4.1.46 - its
# baseline, x height and cap height already land on whole pixels (checked:
# all flat and round letters end in the same rows), and this keeps the
# widths the layouts were made for
def glyph_direct(font, ch):
    x0, y0, x1, y1 = font.getbbox(ch, anchor='ls', mode='1')
    adv = int(round(font.getlength(ch, mode='1')))
    w, h = x1 - x0, y1 - y0
    if w <= 0 or h <= 0:
        return (0, 0, 0, 0, adv, [])
    im = Image.new('1', (w + 4, h + 4), 0)
    d = ImageDraw.Draw(im)
    d.fontmode = '1'
    d.text((2 - x0, 2 - y0), ch, font=font, fill=1, anchor='ls')
    px = im.load()
    rows = [y for y in range(im.height) if any(px[x, y] for x in range(im.width))]
    cols = [x for x in range(im.width) if any(px[x, y] for y in range(im.height))]
    if not rows:
        return (0, 0, 0, 0, adv, [])
    ry0, ry1, cx0, cx1 = rows[0], rows[-1], cols[0], cols[-1]
    gw, gh = cx1 - cx0 + 1, ry1 - ry0 + 1
    bits = [1 if px[x, y] else 0 for y in range(ry0, ry1 + 1) for x in range(cx0, cx1 + 1)]
    xoff = cx0 - 2 + x0
    top = ry0 - 2 + y0
    yoff = -(top + gh)
    return (gw, gh, xoff, yoff, adv, bits)


def sized_direct(path, probe, target):
    best = None
    for s10 in range(60, 1400):
        f = ImageFont.truetype(path, s10 / 10)
        h = glyph_direct(f, probe)[1]
        if h == target:
            return f
        if h > target and best is None:
            best = f
    return best


def build(path, probe, target, charset, tabular, direct=False):
    if direct:
        f = sized_direct(path, probe, target)
        gl = {c: glyph_direct(f, chr(c)) for c in charset}
    else:
        f = sized(path, probe, target)
        gl = glyph_set(f, charset)
    if tabular:
        # all digits one cell: values do not jitter when they change
        adv = max(gl[c][4] for c in range(0x30, 0x3A))
        for c in range(0x30, 0x3A):
            w, h, x, y, a, b = gl[c]
            gl[c] = (w, h, x + (adv - a) // 2, y, adv, b)
    data = encode(gl)
    h, g = decode(data)
    for c in gl:   # round trip through the decoder of the crate
        w, hh, x, y, a, rows = g[c]
        bits = [1 if ch == '#' else 0 for r in rows for ch in r]
        assert (w, hh, x, y, a, bits) == gl[c], (path, target, chr(c))
    return f.size / (1 if direct else SS), data


def rs_bytes(data):
    out = []
    for b in data:
        if b == 0x22:
            out.append('\\"')
        elif b == 0x5C:
            out.append('\\\\')
        elif 0x20 <= b < 0x7F:
            out.append(chr(b))
        else:
            out.append('\\x%02x' % b)
    return 'b"' + ''.join(out) + '"'


def main():
    d, out = sys.argv[1], sys.argv[2]
    value_ttf = d + '/B612-Bold.ttf'
    text_ttf = d + '/BarlowSemiCondensed-SemiBold.ttf'
    items = []
    for t in VALUES:
        items.append(('b612_value_%d' % t, value_ttf, '0', t, ASCII, True))
    items.append(('b612_value_57_num', value_ttf, '0', 57, NUMERIC, True))
    for t in TEXTS:
        items.append(('barlow_text_%d' % t, text_ttf, 'H', t, ASCII, False))
    lines = [
        '//! GENERATED by tools/fontgen/gen.py - do not edit.',
        '//!',
        '//! LARUS display fonts in the u8g2 format: B612 Bold (values) and',
        '//! Barlow Semi Condensed SemiBold (texts), both SIL Open Font License',
        '//! 1.1 (tools/fonts_ofl). Rasterised without anti-aliasing; the number',
        '//! is the cap height ("E", values) resp. the ink height of "H" (texts)',
        '//! in pixels. B612 from an 8x drawing, baseline on a pixel edge, the',
        '//! overshoot of round letters folded: all letters on one line.',
        '//! Characters: ASCII 32..126 and degree, squared, cubed',
        '//! (`_num`: blank * + , - . / 0..9 : degree).',
        '#![allow(non_camel_case_types)]',
        '',
        'use u8g2_fonts::Font;',
        '',
    ]
    total = 0
    for name, ttf, probe, t, cs, tab in items:
        size, data = build(ttf, probe, t, cs, tab, direct=ttf == text_ttf)
        total += len(data)
        print('%-22s size %5.1f  %6d bytes' % (name, size, len(data)))
        lines.append('/// %d bytes (rendered at %.1f px)' % (len(data), size))
        lines.append('pub struct %s;' % name)
        lines.append('impl Font for %s {' % name)
        lines.append("    const DATA: &'static [u8] = %s;" % rs_bytes(data))
        lines.append('}')
        lines.append('')
    print('total', total)
    open(out, 'w').write('\n'.join(lines))


if __name__ == '__main__':
    main()
