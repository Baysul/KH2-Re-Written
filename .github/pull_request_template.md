## Before

<!-- What a reader saw or could do before this change. -->

## After

<!-- What a reader sees or can do after this change. -->

## How

<!-- One short paragraph on the approach, when it is not obvious. -->

## Checks

- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets --locked -- -D warnings`
- [ ] `cargo test --workspace --locked`
- [ ] `cargo build --workspace --locked`
- [ ] `cargo run -p main --locked -- --frames 180`

Mark any check not run as NOT RUN and give the reason.

## Evidence

<!-- Task record ID(s) in docs/tasks/ and the verdict. Paths to private evidence, without the evidence itself. -->
