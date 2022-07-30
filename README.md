# noa

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









