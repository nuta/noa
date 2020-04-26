noa
====
[![Build Status](https://travis-ci.com/nuta/noa.svg?branch=master)](https://travis-ci.com/nuta/noa)
[![Latest version](https://img.shields.io/crates/v/noa.svg)](https://crates.io/crates/noa)

A simple *batteries-inclued* terminal text editor written in Rust.

Features (or TODO)
------------------
- [x] File Finder
- [x] Undo & Redo
- [x] Copy & Paste
- [x] Multiple Cursors
- [x] *Semantic* Syntax Highlighting
- [ ] Mouse support
- [ ] Multiple Process Syncing
- [ ] Language Server Protocol support

Installation
------------
Install `libxcb1-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev` if you're using Ubuntu.


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
