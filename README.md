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
- [x] Finder
- [x] Mouse
- [x] Auto indentation
  - [x] Deindent by `}`
- [x] Backup
- [ ] Undo & redo
- [ ] Copy & paste
- [ ] Commander
- [ ] Backup
- [ ] Open link
- [ ] Completion
- [ ] Format on save
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

----

Language
--------

### Opcodes
```
[addr] <regex>                -- Search for the regex.
[addr] m <regex>              -- Select the whole matches that match the regex.
[addr] a <string>             -- Append a string.
[addr] i <string>             -- Prepend a string.
[addr] r <string>             -- Replace matches with string.
[addr] d                      -- Delete matches.
[addr] f <char>               -- Select the next occurrence of the character.
[addr] b <char>               -- Select the previous occurrence of the character.
[addr] s <char>               -- Select the string surrounded by the character.
[addr] g [^|$%]               -- Go to:
                                     ^  -- The beginning of a line.
                                     |  -- The first non-whitespace character in a line.
                                     $  -- The end of a line.
                                     %  -- Corresponding block symbols (e.g. "{}").
                                (empty) -- The first match.
[addr] c                      -- Transform to lowercase.
[addr] C                      -- Transform to uppercase.
```

### Address
*Address* is a range of text where the command will be applied.

```
(empty)             -- The whole text.
.                   -- The current selection or cursor.
#                   -- The current word.
(number)            -- The whole line. The number represents the relative line number.
[addr1]+[addr2]     -- The range until `addr2` from the end of `addr1`.
[addr1]-[addr2]     -- The range until `addr2` from the beginning of `addr1` (backwards).
[addr1],[addr2]     -- The range of beginning of `addr1` and the end of `addr2`.
```
