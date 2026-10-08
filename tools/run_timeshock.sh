#!/bin/bash
# Run the original Windows build of Timeshock! under Wine using cnc-ddraw
# (ddraw.dll + ddraw.ini in the game folder) for scaled, borderless output,
# and GOG's ogg-winmm wrapper (winmm.dll) which plays MUSIC/TrackNN.ogg as CD audio.
GAME_DIR="${TIMESHOCK_DIR:-$HOME/.wine/drive_c/GOG Games/Pro Pinball - Timeshock}"
cd "$GAME_DIR" || exit 1
WINEDLLOVERRIDES="ddraw,winmm=n,b" WINEDEBUG=-all exec wine 'Timeshock!.exe' "$@"
