# ADR-0006 — Machine Link credentials encrypted at rest (AES-GCM, env key)

Status: accepted (2026-07-22)

## Context / forces

The Machine Link feature (slices `22a`/`22b`) stores per-printer network
credentials in the database: a PrusaLink API key, and — for Bambu — a LAN
access code and serial number. These are real credentials: the Bambu access
code grants control of the physical machine on the LAN, not just read access.

Forces:
- **Self-hosted, single operator** — the DB is on the operator's own
  infrastructure, but backups (`pg_dump`) and DB dumps travel; a dump should
  not leak machine credentials in clear text.
- **No secrets manager available** — the deployment is a single Docker compose
  stack; introducing Vault/KMS is out of proportion.
- **The app already boots from env** (`AUTH_PASSWORD_HASH` precedent) — one
  more mandatory-when-used env var is an acceptable operational cost.
- **Credentials must be usable, not just verifiable** — unlike the auth
  password (argon2 hash), the app must send these values to the machines, so
  hashing is not an option; it's encryption or clear text.

## Decision

Encrypt Machine Link credentials at rest with **AES-256-GCM** using a key
derived from a new env var **`FILATURE_CREDENTIALS_KEY`**:

1. Encrypted columns hold `nonce || ciphertext` (base64); a fresh random nonce
   per write. Vetted crate (`aes-gcm`, RustCrypto), no hand-rolled crypto.
2. The key is required only when at least one Machine Link exists / is being
   created; boot fails with a clear message if a link exists and the key is
   absent or wrong (auth-gate precedent: fail loud, never degrade silently).
3. Credentials are **never sent to the browser**: edit forms show a
   « configuré » placeholder; the status fragment carries no credential
   material. Decryption happens only in the SPI adapters at request time.
4. Non-secret connection fields (host/IP, Moonraker URL, serial number is
   borderline but paired with the access code) — host and URL stay clear text
   for debuggability; access code and API key are encrypted.

## Rejected alternatives

- **Clear text in DB.** Simplest, defensible for single-user self-hosted, but a
  DB dump or backup then carries live machine credentials; encryption cost is
  one small crate + one env var.
- **argon2/hashing.** Impossible — the app must replay the credentials to the
  machines.
- **OS keyring / secrets manager.** Not available in the Docker single-binary
  deployment model; disproportionate.
- **Key in `filature.toml`.** The prod image deliberately ships without a TOML
  file (ADR-0005 precedent); env is the existing secret channel, and a key next
  to the DB it protects would defeat the purpose in same-host dumps anyway —
  env keeps key and data at least process-separated.

## Consequences

- A lost `FILATURE_CREDENTIALS_KEY` orphans stored credentials — the operator
  re-enters them (acceptable: a handful of machines, low re-entry cost). Key
  rotation = decrypt-reencrypt migration, deferred until needed.
- Backups remain safe to store casually; the threat model entry for machine
  credentials points here.
- Demo instances never set the key — Machine Link is disabled there, so the
  constraint is inert.
