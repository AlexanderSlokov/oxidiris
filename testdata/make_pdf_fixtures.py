#!/usr/bin/env python3
"""Generate the PDF fixtures used by the oxidiris-pdf tests (OXD-060).

Run from the repository root:

    python3 testdata/make_pdf_fixtures.py

Real papers are not committed: the ones worth testing against (EuroSys, OSDI, IEEE) are under
publisher copyright, and a fixture has to be redistributable. These files are generated instead,
each one pinning down a single extraction behaviour, and each small enough to read by hand with
`less`.

No third-party library on purpose. A PDF writer that only has to emit uncompressed text is about
sixty lines, and that is cheaper than making every contributor install reportlab to touch a
fixture.
"""

import pathlib

PAGE_WIDTH, PAGE_HEIGHT = 612, 792
FONT_SIZE = 11
LINE_HEIGHT = 14

# Code 1 is remapped to the `fi` ligature glyph through an /Encoding /Differences array, which is
# exactly how a TeX-produced paper stores it. Extraction has to turn it back into "fi".
FI = b"\x01"


def escape(text: bytes) -> bytes:
    """Escape the three characters that terminate or nest a PDF literal string."""
    for special in (b"\\", b"(", b")"):
        text = text.replace(special, b"\\" + special)
    return text


def text_block(lines: list[bytes], x: int, top: int) -> bytes:
    """Draw one line per entry, top down, at a fixed left edge.

    Each line is positioned absolutely rather than with T*, so the vertical gaps are explicit and
    a reader of this file can see where the extractor's line breaks come from.
    """
    out = [b"BT", b"/F1 %d Tf" % FONT_SIZE]
    for index, line in enumerate(lines):
        out.append(b"1 0 0 1 %d %d Tm" % (x, top - index * LINE_HEIGHT))
        out.append(b"(%s) Tj" % escape(line))
    out.append(b"ET")
    return b"\n".join(out)


def build(objects: list[bytes]) -> bytes:
    """Assemble numbered objects into a PDF file with a correct cross-reference table."""
    out = bytearray(b"%PDF-1.4\n")
    offsets = []
    for number, body in enumerate(objects, start=1):
        offsets.append(len(out))
        out += b"%d 0 obj\n" % number + body + b"\nendobj\n"

    xref_at = len(out)
    out += b"xref\n0 %d\n" % (len(objects) + 1)
    out += b"0000000000 65535 f \n"
    for offset in offsets:
        out += b"%010d 00000 n \n" % offset
    out += b"trailer\n<< /Size %d /Root 1 0 R >>\n" % (len(objects) + 1)
    out += b"startxref\n%d\n%%%%EOF\n" % xref_at
    return bytes(out)


def document(streams: list[bytes], with_font: bool = True) -> bytes:
    """One page per content stream, all sharing a single Helvetica font resource."""
    page_count = len(streams)
    kids = b" ".join(b"%d 0 R" % (4 + index) for index in range(page_count))
    resources = b"<< /Font << /F1 2 0 R >> >>" if with_font else b"<< >>"

    objects = [
        b"<< /Type /Catalog /Pages 3 0 R >>",
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding "
        b"<< /Type /Encoding /BaseEncoding /WinAnsiEncoding /Differences [1 /fi] >> >>",
        b"<< /Type /Pages /Kids [%s] /Count %d >>" % (kids, page_count),
    ]
    content_first = 4 + page_count
    for index in range(page_count):
        objects.append(
            b"<< /Type /Page /Parent 3 0 R /MediaBox [0 0 %d %d] /Resources %s /Contents %d 0 R >>"
            % (PAGE_WIDTH, PAGE_HEIGHT, resources, content_first + index)
        )
    for stream in streams:
        objects.append(b"<< /Length %d >>\nstream\n" % len(stream) + stream + b"\nendstream")
    return build(objects)


def typography_pdf() -> bytes:
    """Ligature, hyphenated line break, and a running page number standing on its own.

    The page number sits at the same left edge as the body, not centred. That is what makes the
    extractor treat it as a paragraph of its own: it emits a blank line when the next glyph is
    both far below the previous one and no further right than where that line ended. A centred
    number lands to the right of the short last line and gets glued onto it instead — the
    behaviour measured on real papers, and the reason this fixture is positioned deliberately.
    """
    body = text_block(
        [
            b"Borg achieves high utilization by combining ef" + FI + b"cient",
            b"admission con-",
            b"trol and machine sharing with process-level",
            b"performance isolation.",
        ],
        x=72,
        top=700,
    )
    page_number = text_block([b"12"], x=72, top=60)
    return document([body + b"\n" + page_number])


def two_column_pdf() -> bytes:
    """Two columns whose sentences must not interleave.

    The content stream emits the left column completely before the right one, which is what a
    LaTeX-produced paper does. Reading order therefore falls out of stream order; a PDF that
    emits its columns interleaved is the OXD-061 case and is not handled yet.
    """
    left = text_block(
        [b"The left column starts here and", b"continues to the bottom of the", b"page."],
        x=72,
        top=700,
    )
    right = text_block(
        [b"The right column is read second,", b"after the left one ends, never", b"before."],
        x=330,
        top=700,
    )
    return document([left + b"\n" + right])


def no_text_layer_pdf() -> bytes:
    """A page carrying a drawn rectangle and no text at all, the way a scan does."""
    return document([b"0.5 g\n100 100 400 500 re\nf"], with_font=False)


FIXTURES = {
    "pdf_typography.pdf": typography_pdf,
    "pdf_two_column.pdf": two_column_pdf,
    "pdf_no_text_layer.pdf": no_text_layer_pdf,
}

if __name__ == "__main__":
    here = pathlib.Path(__file__).parent
    for name, make in FIXTURES.items():
        (here / name).write_bytes(make())
        print(f"wrote {here / name}")