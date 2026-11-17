# fontgen - the display fonts

Generates `core/src/utils/larus_fonts_data.rs` (u8g2 font format) from the
TrueType files in `tools/fonts_ofl`:

    cd tools/fontgen
    python3 gen.py ../fonts_ofl ../../core/src/utils/larus_fonts_data.rs

Needs Python 3 with Pillow. Values: B612 Bold, texts: Barlow Semi Condensed
SemiBold (both SIL Open Font License 1.1). The sizes (`VALUES`, `TEXTS` in
gen.py) are ink heights in pixels; `core/src/utils/larus_fonts.rs` names the
FontRenderer constants.

`u8g2fmt.py` holds the u8g2 encoder and a decoder that works like the reader
of the u8g2-fonts crate; gen.py checks every glyph by a round trip.

B612 (values) is drawn 8 times larger with its baseline and cap height on
pixel edges and reduced by majority vote (`SS`, `COVER`); the overshoot of
round letters and the stem tops of B612 (H, 1, 4 stand 1.7 % above E, T, 7)
are folded into the neighbouring row (`fold`). Drawn directly, the flat
letters (n, m, r, 1) sat one pixel above the round ones and the text ran in
waves. Since 0.4.1.47 every B612 size has one baseline, one x height and one
cap height. Barlow is drawn directly as before (already on whole pixels).

