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

NED Language
------------
Noa provides *ned*, an original editing language heavily inspired by Plan 9's
[sam(1)](https://9fans.github.io/plan9port/man/man1/sam.html) editor. Those
interested can grasp the concept of its underlying concept called *structural regular expressions* by
[the ariticle authored by Rob Pike](http://doc.cat-v.org/bell_labs/structural_regexps/se.pdf).

### Examples
```
/open_file/g        -- Search for the next occurrence of `open_file`
/open_file/         -- ditto (from *Some Helpful Exceptions* below)
/open_file          -- ditto (from *Some Helpful Exceptions* below)
```

### Syntax
The NED language consists of an address and arbitrary number of pairs of opecode
and its operand.

```
[addr?][opcode1][operand1*][whitespace?][opcode2][operand2*] ...
```

### Opcodes
`[addr]` is an optional *Address* specifier. Whitespaces around opcodes (`m`, `a`, ...)
are added just for redability. `<regex>` is a regular expression enclosed by the first character.

```
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
(regex)             -- A regular expression enclosed by `/` or `#`.
[addr1]+[addr2]     -- The range until `addr2` from the end of `addr1`.
[addr1]-[addr2]     -- The range until `addr2` from the beginning of `addr1` (backwards).
[addr1],[addr2]     -- The range of beginning of `addr1` and the end of `addr2`.
```

### Some Helpful Exceptions
- If no opcode is given (i.e. only `addr`), it will be interpreted as `[addr]g`. That is,
  it moves to the first match.
- If no opcode is given (i.e. only `addr`), it is allowed to omit the closing character
  in the regular expression in the `addr`.
