# Phantom vendor notes: tungstenite

**Audience:** maintainers auditing or refreshing Phantom's WebSocket engine.
This file records provenance, local behavior changes, and the replay check. It
is not integration documentation.

The files listed in `patches/series` are the canonical local changes, in
application order. Change those patches and replay them; do not make an
unrecorded edit to the vendored crate.

## Publish identity

`publish-identity.patch` is always the last entry in `patches/series`. It
renames the package (`tungstenite` becomes `phantom-tungstenite` at
`0.30.0-phantom.4`), keeps the upstream library name so source, tests, and
examples are unchanged, and points the repository metadata at Phantom. It
removes the upstream documentation link, keeps Cargo's reserved archive files
out of the packaged crate, and records the upstream package, version, and
source archive under `[package.metadata.phantom]`. It changes no Rust source.

Phantom depends on this package only through the renamed package with an exact
version and a path, so the stock package cannot be selected in its place and no
root `[patch]` table is required. When refreshing, regenerate this patch after
the source patches. Increase the `-phantom.N` suffix whenever the fork's
content changes without an upstream version change, and update the exact pins
in the root `Cargo.toml` and in every renamed dependent.

## Upstream baseline

Upstream: `tungstenite` 0.30.0 from crates.io.

- Crates.io archive SHA-256:
  `e48ac77174b19c110a50ab2128b24215ac9cb40e0e12e093fb602d175c569d22`
- Upstream repository: <https://github.com/snapview/tungstenite-rs>
- Upstream licenses remain in `LICENSE-APACHE` and `LICENSE-MIT`.

## Why this patch exists

The ordered patch series adds RFC 7692 `permessage-deflate` support derived
from upstream pull request 561, keeps frame and message payloads out of logs
and errors, makes client masking entropy failure an ordinary result, and
reports forbidden peer Close codes instead of rewriting them. A small
integration patch exposes ordered negotiation for Phantom's custom handshake,
configures raw DEFLATE, and keeps an 8-bit negotiated window
within its wire bound. The default-preserving `message-fragment-limit.patch`
adds an opt-in per-message data fragment count, rejects overflow before
decompression or reassembly, and makes that receive failure terminal. The
`deflate` feature enables only its codec and HTTP value types; the separate
`handshake` feature still owns tungstenite's built-in handshake implementation.

`deprecated-max-value.patch` replaces the two `usize::max_value` calls, which
Rust 1.99 deprecates, with `usize::MAX`. It changes no behavior, and upstream
`master` still has the deprecated calls as of this patch.

`deflate-http-whitespace.patch` limits padding around compression parameters
to HTTP space and horizontal tab. Unicode whitespace remains part of the
parameter and is rejected by its grammar. The separate regression patch
checks invalid padding around names and values, plus legal space/tab controls.

## Client fragmentation and decoder boundary

`rfc7692-client-fragments.patch` queues compressed client payloads over 16 KiB
as independently masked fragments before writing to the transport. If their
additional headers exceed the write-buffer limit, the admitted single frame
still fits.

The private `protocol/deflate/codec/decoder.rs` module owns the zlib-rs raw
inflater. Its C API exposes the sync-point check for the four omitted trailer
bytes and the history-preserving reset needed at BFINAL boundaries (RFC 7692
sections 7.2.1 and 7.2.3.4). Each successful initialization owns one allocation
and its destructor ends the stream once. Calls borrow input and bounded scratch
slices exclusively for the duration of inflation and clear their pointers
before returning. Moving the decoder transfers exclusive ownership. Shared
access exposes no stream operation. The outer codec enforces the cumulative
inflated message limit across continuation frames.

## Required check

`scripts/ci/check-vendor.sh tungstenite` downloads the checksummed upstream
crate, replays every entry in `patches/series`, and compares the result with
this directory.
