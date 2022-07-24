## Profiling

```
perf record --call-graph=dwarf target/release/noa
perf report --hierarchy
```
