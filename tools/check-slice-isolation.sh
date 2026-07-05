#!/usr/bin/env bash
# Enforce: no domain slice imports a sibling slice (crate graph can't catch this — same crate).
# A slice may use `crate::shared::…` but never `crate::<otherslice>::…`.
# See: .claude/skills/hexagonal-architecture/references/rust.md ("Slice isolation check")
set -euo pipefail

SRC="crates/domain/src"
[ -d "$SRC" ] || { echo "Slice isolation: no $SRC yet — skipping."; exit 0; }

# Slice modules = top-level dirs under domain/src, minus the shared kernel.
SLICES=$(find "$SRC" -mindepth 1 -maxdepth 1 -type d -not -name shared -exec basename {} \;)
[ -n "$SLICES" ] || { echo "Slice isolation: no slices yet — skipping."; exit 0; }

ALT=$(echo "$SLICES" | paste -sd'|' -)
FAIL=0

for slice in $SLICES; do
  # any `crate::<otherslice>` reference from inside this slice, excluding self
  others=$(echo "$SLICES" | grep -vx "$slice" | paste -sd'|' -)
  [ -n "$others" ] || continue
  hits=$(grep -rnE "crate::(${others})\b" "$SRC/$slice" || true)
  if [ -n "$hits" ]; then
    echo "SLICE ISOLATION VIOLATION — slice '$slice' imports a sibling slice:"
    echo "$hits" | sed 's/^/  /'
    echo ""
    FAIL=1
  fi
done

if [ "$FAIL" -ne 0 ]; then
  echo "Rule: no slice imports another slice (hexagonal-architecture SKILL, 'Slice cohesion')."
  echo "Why:  slice→slice coupling turns vertical slices back into a tangle and breaks independent delegability."
  echo "Fix:  promote the shared concept into crates/domain/src/shared/, or go through an SPI port — never a direct slice::use."
  exit 1
fi
echo "Slice isolation: OK ($(echo "$SLICES" | paste -sd',' -))"
