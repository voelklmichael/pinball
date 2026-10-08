"""Dump the game's file ID table from Timeshock!.exe as TSV.

The loaders take a numeric file ID. Entry `id` is 8 bytes at 0x478578:
u32 directory index (into the pointer table at 0x478528), u32 name pointer.

usage: python3 -I file_table.py <Timeshock!.exe> <out.tsv>
"""
import struct
import sys

DGROUP_VA, DGROUP_OFF, DGROUP_END = 0x464000, 0x63400, 0x487000
DIR_TABLE, FILE_TABLE = 0x478528, 0x478578


def main(exe_path, out_path):
    data = open(exe_path, "rb").read()

    def u32(va):
        return struct.unpack_from("<I", data, va - DGROUP_VA + DGROUP_OFF)[0]

    def cstr(va):
        off = va - DGROUP_VA + DGROUP_OFF
        return data[off:data.index(b"\0", off)].decode("latin-1")

    dirs = [cstr(u32(DIR_TABLE + 4 * i)) for i in range((FILE_TABLE - DIR_TABLE) // 4)]
    rows = []
    while True:
        entry = FILE_TABLE + 8 * len(rows)
        dir_idx, name_ptr = u32(entry), u32(entry + 4)
        if dir_idx >= len(dirs) or not DGROUP_VA <= name_ptr < DGROUP_END:
            break
        rows.append((len(rows), dir_idx, dirs[dir_idx], cstr(name_ptr)))

    with open(out_path, "w") as f:
        f.write("id\tid_hex\tdir_idx\tdir\tname\n")
        for i, di, d, n in rows:
            f.write(f"{i}\t0x{i:x}\t{di}\t{d}\t{n}\n")
    print(f"file_table: {len(rows)} files, {len(dirs)} dirs -> {out_path}")


if __name__ == "__main__":
    main(*sys.argv[1:3])
