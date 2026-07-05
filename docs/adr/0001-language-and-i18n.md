# ADR-0001 — English domain language, internationalised UI

Status: accepted (2026-07-05)

## Context / forces

The domain concepts, the source brief, and the UI design handoff were authored
in French. The user works in French and the design mockups use French labels.
Two forces pull against each other:

- **Code conventions & portability.** Rust types, SQL identifiers, and slice
  folder names are idiomatic in English; accented French identifiers
  (`Matériau`) are awkward and non-idiomatic, and the brief already named its
  slices in English (`spools`, `materials`, `dashboard`).
- **User-facing language.** The primary user is French-speaking and the design
  is French. A hardcoded-French UI would satisfy today's user but bakes the
  language into templates, making a second language a costly retrofit.

The user also stated a forward requirement: ship **en + fr** as standard and
allow **other languages to be added** later.

## Decision

1. **Domain language is English.** The glossary's canonical terms are English;
   Rust types, SQL tables/columns, and slice folder names use them verbatim.
2. **The UI is internationalised from the start.** No user-facing string is
   hardcoded in a template. Strings come from per-locale translation catalogs.
   **en** and **fr** ship built-in; the mechanism must let further locales be
   added without code changes to templates.
3. The glossary carries a `FR label` column as the built-in French translation
   of each term — a translation, not a second canonical name.

## Rejected alternatives

- **French domain identifiers (`Bobine`, `Rangement`, `StatutBobine`).** Total
  brief↔code↔UI consistency, but non-idiomatic Rust, accent-stripping needed in
  identifiers, and it still wouldn't give multi-language UI — it just hardcodes
  a different single language.
- **Hardcoded French UI, no i18n.** Simplest now, but retrofitting i18n later
  means touching every template and every string — exactly the hard-to-reverse
  cost this ADR exists to avoid. Rejected given the explicit multi-language
  requirement.

## Consequences

- The templating layer (Tera) needs a translation lookup available in every
  template and fragment (including htmx-swapped fragments, which must render in
  the active locale). Locale selection + persistence (cookie/header) is now an
  architectural concern, alongside the theme toggle already in the design.
- Render tests (already required for Tera) should cover at least one non-default
  locale so missing translation keys fail at `cargo test`, not in production.
- Adds scope over the original brief (which implied a French-only UI). Recorded
  in `docs/product/brief.md`.
- Translation catalogs are data, editable without recompiling templates; adding
  a language is adding a catalog.
