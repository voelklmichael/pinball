#!/bin/bash
# Import + analyze Timeshock!.exe in Ghidra (headless), apply the Watcom
# calling convention and export decompiled C and xref indexes to re/out/.
# re/ holds material derived from the original binary: never commit it.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
GHIDRA="${GHIDRA_HOME:-$HOME/opt/ghidra_12.1.4_PUBLIC}"
export JAVA_HOME="${JAVA_HOME_GHIDRA:-$HOME/opt/jdk-21.0.12.1+1}"
export PATH="$JAVA_HOME/bin:$PATH"
EXE="${TIMESHOCK_DIR:-$HOME/.wine/drive_c/GOG Games/Pro Pinball - Timeshock}/Timeshock!.exe"
PROJ="$ROOT/re/ghidra"
OUT="$ROOT/re/out"
mkdir -p "$PROJ" "$OUT"

if [ -d "$PROJ/Timeshock.rep" ]; then
  # Re-export from the existing (possibly hand-annotated) project.
  "$GHIDRA/support/analyzeHeadless" "$PROJ" Timeshock -process 'Timeshock!.exe' -noanalysis \
    -scriptPath "$ROOT/tools/ghidra" \
    -postScript WatcomSetup.java -postScript ExportAll.java "$OUT"
else
  "$GHIDRA/support/analyzeHeadless" "$PROJ" Timeshock -import "$EXE" \
    -scriptPath "$ROOT/tools/ghidra" \
    -postScript WatcomSetup.java -postScript ExportAll.java "$OUT"
fi
python3 -I "$ROOT/tools/ghidra/file_table.py" "$EXE" "$OUT/files.tsv"
