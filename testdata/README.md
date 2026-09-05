# Test corpus

Fixtures shared by the engine tests (OXD-004). Each file exists to pin down one specific
behaviour, so please do not "tidy" the contents — the odd characters are the point.

| File | What it pins down |
|---|---|
| `simple.txt` | Baseline ASCII: paragraph splitting on blank lines |
| `vietnamese_nfc.txt` | Vietnamese in composed form (NFC) |
| `vietnamese_nfd.txt` | **Byte-different, semantically identical** to the NFC file. The two must produce identical token streams |
| `cjk.txt` | Full-width characters: 1 grapheme occupies 2 terminal columns |
| `emoji_zwj.txt` | ZWJ family sequence and a regional-indicator flag; each must count as one grapheme |
| `mixed_width.txt` | ASCII, CJK and emoji on a single line |
| `abbreviations.txt` | `Fig.`, `et al.`, `i.e.`, `e.g.`, `vs.`, `No.`, `v1.2.3`, `std::fmt`, `Dr.` — none may trigger a sentence-end pause, and the final real full stop must still be detected |
| `long_words.txt` | URLs, a git hash, and words longer than the reader frame |
| `rfc_style.txt` | Hard wrapping at column ~64. Must rejoin into 2 paragraphs, not 5 |
| `sample.md` | Markdown covering headings, lists, code fences, tables, quotes, links and autolinks |
| `utf16_bom.txt` | UTF-16LE with a byte order mark |
| `latin1.txt` | Windows-1252 bytes that are invalid UTF-8 |
| `empty.txt` | Zero bytes: must not panic anywhere |
| `pdf_typography.pdf` | A `fi` ligature, a hyphenated line break, a real compound hyphen that must survive, and a page number standing alone |
| `pdf_two_column.pdf` | Two columns that must be read one after the other, never interleaved |
| `pdf_no_text_layer.pdf` | A drawn rectangle and no text, the way a scan looks: must produce an error, not an empty reader |

## Regenerating the PDF fixtures

The three PDFs are generated, not collected:

```sh
python3 testdata/make_pdf_fixtures.py
```

No third-party library is involved — the script writes the PDF structure itself, so changing a
fixture does not mean installing a toolchain. The real papers the extractor was benchmarked
against (Borg, Omega, MapReduce, Bigtable, Spanner) are deliberately absent: they are under
publisher copyright, and a fixture has to be redistributable. See
[`docs/decisions/pdf-extraction.md`](../docs/decisions/pdf-extraction.md) for the measurements
they produced.

## Regenerating the Unicode fixtures

`vietnamese_nfd.txt` is derived from `vietnamese_nfc.txt`:

```sh
python3 -c "
import unicodedata, pathlib
nfc = pathlib.Path('testdata/vietnamese_nfc.txt').read_text()
pathlib.Path('testdata/vietnamese_nfd.txt').write_text(unicodedata.normalize('NFD', nfc))
"
```

The `tests/corpus.rs` suite asserts that the two files really are byte-different before comparing
their token streams, so a regeneration mistake fails loudly rather than making the test vacuous.