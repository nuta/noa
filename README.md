noa
====
[![Build Status](https://travis-ci.com/nuta/noa.svg?branch=master)](https://travis-ci.com/nuta/noa)
[![Latest version](https://img.shields.io/crates/v/noa.svg)](https://crates.io/crates/noa)

A opinionated terminal text editor written just for me.

Features (or TODO)
------------------
- [x] Basic editing
  - [x] Clean up
  - [x] Logical x
- [x] Terminal
  - [x] Wrapping
- [ ] Finder
- [ ] Commander
- [ ] Mouse
- [ ] Undo & redo
- [ ] Auto indentation
  - [ ] Deindent by `}`
- [ ] Copy & paste
- [ ] Format on save
- [ ] Backup
- [ ] Faster search algorithm

Installation
------------

```
$ cargo install noa
```

Building from source
--------------------
```
$ cargo build --release
$ ./target/release/noa
```

License
-------
CC0 or MIT. Choose whichever you prefer.
