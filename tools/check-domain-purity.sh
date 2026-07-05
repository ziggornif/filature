#!/usr/bin/env bash
# Enforce: the `domain` crate has no forbidden dependencies.
# Rule: domain may only depend on what's in ALLOW. Anything else is a HEXAGONAL VIOLATION.
# See: .claude/skills/hexagonal-architecture/references/rust.md  and  docs/adr/0002-crate-structure.md
set -euo pipefail

# The ONLY deps crates/domain may have. Keep minimal — extend deliberately.
#   thiserror     : domain error types
#   rust_decimal  : money / precise weights without float drift
#   time          : date/time TYPES only (Clock is an SPI; no now() in domain)
ALLOW="thiserror|rust_decimal|time"

BAD=$(cargo tree -p domain --prefix none --no-dedupe 2>/dev/null \
      | sed 's/ v[0-9].*//' | sort -u \
      | grep -vE "^(domain|std|core|alloc| *$|${ALLOW})" || true)

if [ -n "$BAD" ]; then
  echo "HEXAGONAL VIOLATION — domain crate has forbidden dependencies:"
  echo "$BAD" | sed 's/^/  - /'
  echo ""
  echo "Rule: the domain depends on nothing framework/IO (hexagonal-architecture SKILL, 'Domain purity')."
  echo "Why:  a framework/IO type in the domain couples business rules to infrastructure and breaks the dependency rule."
  echo "Fix:  move the capability behind an SPI trait in domain's ports/spi/, implement it in crates/app,"
  echo "      then remove the forbidden dep from crates/domain/Cargo.toml."
  echo "      (Need current time or a new id? Use the Clock / IdGenerator SPI, don't add the crate here.)"
  exit 1
fi
echo "Domain purity: OK"
