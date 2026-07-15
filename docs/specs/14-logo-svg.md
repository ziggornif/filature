## Agent Brief — 14 Logo SVG (new brand mark)

**Category:** chore / UI
**Summary:** Replace the inline sidebar/brand logo SVG with the new concentric-ring
mark from the updated maquette. Pure presentational swap — no domain, ports, DB, or
i18n change. The smallest of the printer-round tickets; ship it first as a quick win.

**Slice / context:**
The brand mark is an inline `<svg class="sidebar-brand-logo">` living in
`crates/app/assets/templates/base.html`. It appears **twice**: once in the collapsed
mobile header, once inside the `.wordmark` next to `{{ t(key="app.name") }}`. The
current mark is two concentric circles plus a cross (`M12 3v3.9…`). The updated
maquette (`init_assets/design_handoff_filature/Filature.dc.html`, sidebar header ~L81)
replaces it with a three-ring "spool" mark. This ticket brings the running app in
line with that maquette.

**Desired behavior:**
- Both `sidebar-brand-logo` occurrences render the **new mark**: a dashed outer ring,
  a solid muted middle ring, and a filled accent centre dot — matching the maquette:
  ```
  <svg ... viewBox="0 0 24 24" fill="none">
    <circle cx="12" cy="12" r="9"   stroke="var(--border-strong)" stroke-width="1.1" stroke-dasharray="2.2 2.2"/>
    <circle cx="12" cy="12" r="6.2" stroke="var(--muted)"         stroke-width="1.3"/>
    <circle cx="12" cy="12" r="2"   fill="var(--accent)"/>
  </svg>
  ```
- Keep the existing `class="sidebar-brand-logo"`, `width/height` (as currently sized —
  22×22 in base.html; the maquette's 24 is its own scale), `viewBox="0 0 24 24"`, and
  `aria-hidden="true"`. Only the inner shapes change.
- Both light and dark themes look correct: the mark uses the theme CSS variables
  (`--border-strong`, `--muted`, `--accent`) exactly as the maquette does — **no
  hardcoded colours**, so it inherits the accent and both palettes for free.
- Keep the two occurrences **identical** so mobile and desktop match.

**Key interfaces:** none (template-only change). No Rust, no ports, no migration.

**Acceptance criteria (the done contract):**
- `base.html` renders the new three-ring mark in both the mobile header and the
  wordmark; the old cross-path mark is gone.
- The mark is visible and centred in the sidebar in light **and** dark theme, and
  picks up the selected accent colour (centre dot = `var(--accent)`).
- Existing shell/e2e tests stay green (the logo has no test asserting its path; if
  `e2e_shell` asserts anything about the brand element, it still passes).
- clippy + offline build clean; no i18n keys added or removed.

**Non-goals / out of scope (YAGNI):**
- Favicon / apple-touch-icon (none exists today — not introducing one here).
- Extracting the logo to a shared partial or static `.svg` file (it is inlined twice
  today; keep that shape unless it's trivially cleaner — not required).
- Any restyle of `.sidebar-brand-logo` CSS (`app.css:206`) beyond what the swap needs.
- The `app.name` wordmark text, nav icons, or any other iconography.

**Design notes / constraints:**
- Source of truth for the mark is the maquette in `init_assets/design_handoff_filature/`.
- The maquette sizes the header logo at 24×24; base.html uses 22×22. Keep base.html's
  22×22 (or match the maquette's 24 if it reads better) — either is fine, but keep both
  occurrences the same.
