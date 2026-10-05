#!/bin/bash
# Rebuild the kit's compiled shaders (Q/shaders/*.frag -> *.frag.qsb) with Qt's
# shader baker. The .qsb files are committed: the shell loads them as they are.
set -euo pipefail
cd "$(dirname "$0")/../quadrille.bar/Q/shaders"
for f in *.frag; do
  /usr/lib/qt6/bin/qsb --glsl "100 es,120,150" --hlsl 50 --msl 12 -o "$f.qsb" "$f"
  echo "$f.qsb"
done
