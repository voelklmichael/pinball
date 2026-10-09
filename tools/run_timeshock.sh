#!/bin/bash
# Run the original Windows build of Timeshock! under Wine using cnc-ddraw
# (ddraw.dll + ddraw.ini in the game folder) for scaled, borderless output,
# and GOG's ogg-winmm wrapper (winmm.dll) which plays MUSIC/TrackNN.ogg as CD audio.
#
# Getting CD music to work needs four things:
#  1. GOG's exe has the CD-audio init routine (0x45a0dc, called from 0x445913)
#     stubbed out with `xor eax,eax; ret`. We run a copy, Timeshock!_music.exe,
#     with the original prologue `push ebx; push ecx; push edx` restored.
#  2. That routine scans for a CD-ROM drive (GetDriveTypeA == 5) labelled
#     TIMESHOCK!, so we provide an empty fake one as E: in the Wine prefix.
#  3. winmm=n,b so the game loads GOG's wrapper instead of Wine's builtin.
#  4. cnc-ddraw's API hooks reroute mciSendCommandA to Wine's builtin winmm,
#     so ddraw.ini has a [Timeshock!_music] section with hook=0.
GAME_DIR="${TIMESHOCK_DIR:-$HOME/.wine/drive_c/GOG Games/Pro Pinball - Timeshock}"
PREFIX="${WINEPREFIX:-$HOME/.wine}"
EXE='Timeshock!_music.exe'

if [ "$(cat "$PREFIX/fake_cd/.windows-label" 2>/dev/null)" != 'TIMESHOCK!' ]; then
    mkdir -p "$PREFIX/fake_cd"
    printf 'TIMESHOCK!\n' > "$PREFIX/fake_cd/.windows-label"
    ln -sfn ../fake_cd "$PREFIX/dosdevices/e:"
    # /reg:64: mountmgr reads the 64-bit view; the default lands in Wow6432Node.
    WINEDEBUG=-all wine reg add 'HKLM\Software\Wine\Drives' /v 'e:' /d cdrom /f /reg:64 >/dev/null
    wineserver -k
fi

cd "$GAME_DIR" || exit 1

if [ ! -f "$EXE" ]; then
    off=$((0x45a0dc - 0x401000 + 0x400))
    if [ "$(xxd -s $off -l 3 -p 'Timeshock!.exe')" != "31c0c3" ]; then
        echo "Timeshock!.exe is not the expected GOG build, not patching" >&2
        exit 1
    fi
    cp 'Timeshock!.exe' "$EXE"
    printf '\x53\x51\x52' | dd of="$EXE" bs=1 seek=$off conv=notrunc status=none
fi

if ! grep -q '^\[Timeshock!_music\]' ddraw.ini; then
    printf '\n; Timeshock (music-enabled copy): no hooking, so mciSendCommandA reaches the ogg-winmm wrapper\n[Timeshock!_music]\nhook=0\n' >> ddraw.ini
fi

WINEDLLOVERRIDES="ddraw,winmm=n,b" WINEDEBUG=-all exec wine "$EXE" "$@"
