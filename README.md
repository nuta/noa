# noa

A minimalistic terminal text editor. Aims to be a good alternative to GNU nano.

## Features

- Grapheme-aware text editing with multiple cursors.


## Profiling

```
cargo flamegraph --bin noa -- src/buffer/buffer.rs
```

## Using tokio-console

```
NOA_TOKIO_TRACE=1 RUSTFLAGS="--cfg tokio_unstable" cargo run --bin noa --release

cargo install --locked tokio-console
tokio-console
```
