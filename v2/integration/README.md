# Host prerequisites — review only, never applied

All inspection/extraction uses committed external-process host baseline
`ea100cee3f69711de12485ba061950c5fc65258f` via `git show`. No dirty host file is
read as an implementation/build input. This is schema 3/protocol 1, not a schema-2
compiled adapter. The independent SDK pin is
`e955911f0bf5b558f9d3eb5183df0adc95c22af2`, crate 0.1.1.

`host-prerequisites.patch` contains five narrow generic repairs:

1. Detail's Enter button currently passes a React MouseEvent as its optional
   actionId. Wrap the callback so it receives no event argument. Keyboard Enter's
   separate path already passes the correct action ID.
2. List Dropdown ignores its controlled value. Respect `props.value`, including
   empty-string All, when initializing/updating selection. This restores LTS/All
   styling on Back; `storeValue` still has no persistence implementation.
3. List accessory parsing drops tooltip and its span has no title. Preserve and
   render the source date tooltip.
4. Registry resolves/validates the active package but discards `package.dir` and
   looks for native icons in the package root/legacy tree. Keep a separate verified
   `resource_dir` for icon/command-icon lookup, leaving preference/uninstall roots
   untouched. Damaged canonical packages get no legacy asset fallback.

5. The compatibility router strips native/raycast prefixes but never recognizes
   the declared own `com.tezbar.<slug>` replacement ID. Add recognition only when
   that exact replacement is declared by the trusted catalog manifest. A test
   executes the actual before/after function with this port's manifest as the
   explicit catalog seam. No catalog entry is created here. Raycast aliases still
   require the parent's selected catalog entry; canonical installed commands use
   the ordinary direct native route.

`make-patches.py` regenerates the patch from committed snapshots. It changes only
`v2/.work/patched-host`. `git apply --check --directory=v2/.work/host ...` passes.
The host working tree is never patched or built. Both Installed struct literals
are covered. Native row icons already use SDK `packageDir` (the verified active
version) plus `resources/nodejs-icon.svg`, so they do not depend on registry
launcher-icon lookup.

Evidence: `qa/renderer-results.json` executes actual List/Dropdown/sections,
Detail, metadata, Surface/Back/actions, and Markdown before/after. Cosmetic
primitives and unrelated Form/Grid/color helpers are honest stubs; host services
capture calls. `host_contract.rs` mechanically extracts the real runtime validator,
artifact lookup, file verifier and registry icon functions (rustfmt only), then
`tests/host_contract.rs` executes before/after filesystem probes. The resolver
boundary in that probe supplies the active verified directory; the full app and
its platform UI are not built or executed. The actual JSON schema is separately
executed using Ajv 2020. Runtime and JSON schema differ in strictness; both must
pass. Empty development artifacts validate, but artifact acquisition correctly
fails with publication/pins required.

The baseline List client filter omits tag keywords. This adapter uses the existing
`__hasServerSearch` path and filters the in-memory feed in native code, so no
keyword-filter host patch is required. It never refetches per keystroke. The
baseline Markdown engine permits generic links/images; this adapter makes remote
body content inert before rendering, so no generic Markdown permission widening
or host actions from arbitrary body content are required.

Before release, the parent must review/apply generic prerequisites in the host's
own task, build and validate that host, publish real target ZIPs, pin their exact
archive/executable digests and real HTTPS acquisition URLs in the trusted catalog,
and run native platform acceptance. Source and ZIP manifests here have no fake
artifact URL/digest or host catalog edits. No installation, PR, release, push or
completion-ledger update is part of this implementation. Acquisition/import is
separate from implementation and real browser/clipboard/GUI acceptance.
