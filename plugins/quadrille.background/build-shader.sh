#!/bin/bash
# Rebuild graticule.frag.qsb after editing graticule.frag (qt6-shadertools: qsb).
# The compiled file is committed, so installing needs no Qt tools.
set -euo pipefail
cd "$(dirname "$0")"
qsb=$(command -v qsb || echo /usr/lib/qt6/bin/qsb)
"$qsb" --qt6 -o graticule.frag.qsb graticule.frag
