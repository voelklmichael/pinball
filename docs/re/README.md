# Reverse-engineering workflow

## Ghidra project and exports

```
tools/ghidra/run_headless.sh
```

- First run: imports `Timeshock!.exe` into `re/ghidra/` (a Ghidra project
  you can also open in the GUI with `~/opt/ghidra_12.1.4_PUBLIC/ghidraRun`)
  and analyses it.
- Later runs: reuse the project (keeping hand-made names and types), apply
  `__watcall` again and re-export.
- Needs Ghidra 12.1.4 in `~/opt/ghidra_12.1.4_PUBLIC` and JDK 21 in
  `~/opt/jdk-21.0.12.1+1`. Override with `GHIDRA_HOME` / `JAVA_HOME_GHIDRA`.

**`re/` is gitignored and must never be committed.** It contains material
derived from the copyrighted binary. Only our own Rust code and documentation
go in the repo.

## `re/out/` files

| File | Contents |
|---|---|
| `functions.tsv` | addr, name, size, calling convention, module, #callers, #callees |
| `calls.tsv` | caller, callee, call site |
| `strings.tsv` | string addr, referencing functions, data words that point to it (pointer tables), text |
| `globals.tsv` | data addr, R/W, function, instruction |
| `decomp/<addr>_<name>.c` | decompiled C, one file per function |
| `all.c` | everything concatenated |

Typical lookups:

```sh
grep -i 'view.bit\|\.spr' re/out/strings.tsv     # which functions use a file name
awk -F'\t' '$2=="00457070"' re/out/calls.tsv     # who calls decode_c
awk -F'\t' '$1=="0047a940"' re/out/globals.tsv   # who touches a global
```

## Conventions in the binary

- Watcom C/C++32 (1995), mostly the `__watcall` register convention: args in
  EAX, EDX, EBX, ECX, then stack (callee pops). The return value is in EAX;
  everything else is preserved. 57 functions whose callers pop the stack
  (`printf`-style, variadic) are `__cdecl`.
- The original sources were `\pinball2\src\...`. Only a few path strings
  survive (`pc/display/jpeg.c`, `pc/win/display/ddraw.c`).
- Known functions are listed in the `docs/formats/*.md` pages (for example
  `cmp1.md` for the lh5 decoder).

## Working on a format

1. Find the loader: search `strings.tsv` for the file name or extension, then
   follow `calls.tsv` from the referencing function.
2. Read the decompiled C, then write the format up in `docs/formats/<name>.md`
   with addresses, field tables and open questions.
3. Write the parser in `crates/ts-formats` and a dumper in `ts-tools`.
4. Check it against every matching file in the install, and visually where
   that's possible.
