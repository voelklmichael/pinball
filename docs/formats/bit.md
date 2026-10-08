# `.BIT` images

Headerless 24-bit pixels, **BGR** byte order, rows top to bottom, no padding.
Usually `CMP1`-compressed ([cmp1.md](cmp1.md)); `BACKBOX/BACKBOX.BIT` is
stored raw.

The dimensions aren't stored. Each file is a full screen in the resolution of
the set it belongs to, so `width × height × 3` equals the unpacked size:

| Set | Resolution |
|---|---|
| `Vx1` | 640×480 |
| `Vx2` | 800×600 |
| `Vx3` | 1024×768 |
| `Vx4` | 1280×1024 |
| `Vx5` | 1600×1200 |

`x` (0–3) is the camera view. `BACKBOX.BIT` is 640×480 only.

Implementation: `ts_formats::bit`; `ts-tools bit2png <in> <out>`.

Open: `MENU.BIT` and `Pause.bit` in `DAT/PC` not checked yet.
