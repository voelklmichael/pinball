# `CMP1` compressed files

Any data file may be stored compressed. The generic file loader
(`Timeshock!.exe` 0x44b4xx–0x44b69f) reads 4 bytes and compares the first 3
with `"CMP"`. On a match it decompresses; otherwise it reads the file raw.

| Offset | Size | Contents |
|---|---|---|
| 0 | 3 | `"CMP"` |
| 3 | 1 | method; only `'1'` (0x31) is supported (0x4571fc warns otherwise) |
| 4 | 4 | unpacked size, little-endian u32 |
| 8 | … | LHA `-lh5-` bitstream (no LHA archive header) |

## Algorithm

The decompressor is Haruhiko Okumura's `ar002` decoder (the LHA `-lh5-`
method), unchanged:

- 8 KiB sliding dictionary (`& 0x1fff`), allocated as `0x2000` at 0x45722c
- 4 KiB input buffer (`0x1000` at 0x457213)
- static Huffman, in blocks: 16-bit block size, then the code-length tree
  (`NT=19`, `TBIT=5`), the literal/length tree (`NC=510`, `CBIT=9`) and the
  position tree (`NP=14`, `PBIT=4`)
- symbol < 256 is a literal; otherwise length = `sym − 253` (`sub eax,0xfd`
  at 0x4572fa then a post-decrement loop) and the source is
  `(pos − decode_p() − 1) & 0x1fff`

| Address | ar002 name |
|---|---|
| 0x4571fc | `decode_start` (+ buffer allocation, method check) |
| 0x4572a0 | `decode(count, buffer)` |
| 0x457070 | `decode_c` |
| 0x457158 | `decode_p` |
| 0x4569c0 | `fillbuf` |
| 0x4571e4 | frees the buffers |

Implementation: `ts_formats::lh5`, `ts_formats::cmp`.

## Verified

`DAT/PC/V05/VIEW.BIT` unpacks to 5,760,000 bytes, a 1600×1200 BGR image of
the table (see [bit.md](bit.md)).
