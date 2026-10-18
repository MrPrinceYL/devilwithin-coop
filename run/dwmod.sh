#!/bin/bash
# dwmod.sh <tag> <seconds> [--press BUTTON@SECOND[+HOLD] ...]: Devil Within Mod headless.
# Frames (one PNG per game second): run/dwmod_<tag>/, log: run/dwmod_<tag>.log. The game starts
# directly in Devil Within; T5_DW_DEBUG (default: off) opens the debug title / developer menu,
# T5_DW_MENULOG=1 writes every overlay change into the log, T5_WARP skips idle time (default
# 0-40). Seconds = game time. Uses a copy of memcard/ (T5_MEMSRC to use another).
T=$1; S=$2; shift 2
cd "$(dirname "$0")"
MOD=$(cd .. && pwd)
BIN=$MOD/rust/target/debug/t5-headless
PWD_OPT=
[ -f $BIN.exe ] && BIN=$BIN.exe && PWD_OPT=-W
: "${T5_DISC:=$(cd ../../../Game && pwd $PWD_OPT)/Tekken 5 (USA).iso}"
export T5_DISC
rm -rf mc_$T dwmod_$T && cp -r "${T5_MEMSRC:-$MOD/memcard}" mc_$T
env T5_DW_MENULOG=1 T5_WARP=${T5_WARP:-0-40} T5_HOLD=0.15 \
    T5_MEMCARD=mc_$T timeout 1800 $BIN $S dwmod_$T "$@" > dwmod_$T.log 2>&1
rm -rf mc_$T
echo "done $T"
