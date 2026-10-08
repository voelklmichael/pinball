#!/bin/bash
# Run the original Windows build of Timeshock! under Wine using cnc-ddraw
# (ddraw.dll + ddraw.ini in the game folder) for scaled, borderless output,
# and GOG's ogg-winmm wrapper (winmm.dll) which plays MUSIC/TrackNN.ogg as CD audio.
#
# The game only issues CD-audio MCI commands if it finds a CD-ROM drive labelled
# PRO_PINBALL_2 (label of the original disc, see pptime.gog), so we provide an
# empty fake one as E: in the Wine prefix.
GAME_DIR="${TIMESHOCK_DIR:-$HOME/.wine/drive_c/GOG Games/Pro Pinball - Timeshock}"
PREFIX="${WINEPREFIX:-$HOME/.wine}"

if [ ! -e "$PREFIX/fake_cd/.windows-label" ]; then
    mkdir -p "$PREFIX/fake_cd"
    printf 'PRO_PINBALL_2\n' > "$PREFIX/fake_cd/.windows-label"
    ln -sfn ../fake_cd "$PREFIX/dosdevices/e:"
    WINEDEBUG=-all wine reg add 'HKLM\Software\Wine\Drives' /v 'e:' /d cdrom /f >/dev/null
fi

cd "$GAME_DIR" || exit 1
WINEDLLOVERRIDES="ddraw,winmm=n,b" WINEDEBUG=-all exec wine 'Timeshock!.exe' "$@"
