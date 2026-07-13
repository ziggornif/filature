#!/usr/bin/env bash
# Enforce: the `domain` crate has no forbidden dependencies.
# Rule: domain may only depend on what's in ALLOW. Anything else is a HEXAGONAL VIOLATION.
# See: .claude/skills/hexagonal-architecture/references/rust.md  and  docs/adr/0002-crate-structure.md
set -euo pipefail

# The ONLY deps crates/domain may have. Keep minimal — extend deliberately.
#   thiserror     : domain error types
#   rust_decimal  : money / precise weights without float drift (default-features
#                   disabled in crates/domain/Cargo.toml so its `serde` feature,
#                   and therefore serde itself, never enters the domain tree)
#   time          : date/time TYPES only (Clock is an SPI; no now() in domain)
#   arrayvec, num-traits : transitive, pure-computation deps of rust_decimal itself
#   deranged, num-conv, powerfmt : transitive, pure-computation deps of time
#                   (no framework/IO surface) — not independently added by domain.
ALLOW="thiserror|rust_decimal|time|arrayvec|num-traits|deranged|num-conv|powerfmt"

# -e normal,no-proc-macro: only runtime deps, excluding proc-macro crates (e.g.
# thiserror-impl and its own proc-macro2/quote/syn/unicode-ident chain) and
# build-dependencies (e.g. autocfg) — those are compile-time only, not a
# framework/IO surface the domain links against.
BAD=$(cargo tree -p domain -e normal,no-proc-macro --prefix none --no-dedupe 2>/dev/null \
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
