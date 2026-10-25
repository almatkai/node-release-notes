# Node Release Notes — native v2

Independent external Rust executable for the single `index` / **Node Versions**
command. Schema 3, protocol 1, SDK **0.1.1** pinned over HTTPS to
`e955911f0bf5b558f9d3eb5183df0adc95c22af2`; Rust **1.85** minimum. No Tezbar,
Tauri, sibling path, Bun or JavaScript runtime dependency.

This is an **unpublished development implementation**. All V1 files remain
byte-identical to `2410143f530d3d6ea7143e25717322acc4af7512` except the root README
link. The source branch is `feat/native-v2-rust`.

- [Behavior, bounds and adaptations](BEHAVIOR.md)
- [Reproducible validation and evidence](VALIDATION.md)
- [Host prerequisites and publication gates](integration/README.md)
- [Measured development ZIP](packages/package-report.json)
- [Complete deliverable byte/SHA inventory](INDEX.json)
- [License notices](NOTICE), [locked dependency licenses](licenses/cargo-licenses.json)

Build/test from this directory:

```sh
TZ=UTC cargo test --locked --all-targets
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
TZ=UTC cargo +1.85.0 test --locked --all-targets
cargo +1.85.0 clippy --locked --all-targets -- -D warnings
cargo build --locked --release --target aarch64-apple-darwin
```

Normal launch fetches `https://nodejs.org/dist/index.json`. Selecting a row fetches
its GitHub release. QA supplies captured synthetic transports/local HTTP servers;
it never requests those public product endpoints or invokes real clipboard/browser
services. Source oracle and renderer tools are development dependencies only.

The source manifest's `artifacts` is deliberately empty; both actual schema and
runtime validator accept it for development. The ZIP contains an ARM64 binary,
complete file inventory, exact original icons and notices. It is not a catalog
release or an installed extension. Parent approval, real artifact URLs/pins,
reviewed host prerequisites and platform GUI acceptance remain separate work.
