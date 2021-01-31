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
/Emacs/r/Vim/       -- Replace the next occurence of "Emacs" with "Vim".
,/Emacs/r/Vim/      -- Replace the all occurences of "Emacs" with "Vim" in the text.
./Emacs/r/Vim/      -- Replace the all occurences of "Emacs" with "Vim" in 
                       the selection.

S{r#/*\1*/#         -- Select the current code block enclosed by `{` and `}`
                       and then comment out the part by `/* */`.
,x/foo/Cr/"\1"/     -- Search all "foo", transform them to uppercase, and then
                       enclose them with double quotes: foo -> FOO -> "FOO"
,p(rustfmt)         -- Execute rustfmt(1) and input the whole text.

11g                 -- Goto the 11th line.

/open_file/g        -- Move to the next occurrence of "open_file"
/open_file/         -- ditto (from *Some Helpful Exceptions* below)
/open_file          -- ditto (from *Some Helpful Exceptions* below)
```

### Syntax
The NED language consists of an address and arbitrary number of pairs of opecode
and its operand.

Each opcode takes as input a list of matches, edits matches if necessary, and
outputs a list of matches. For the first opcode, the range specified by
*address* is given as the single match.

```
[addr?][opcode1][operand1*][whitespace?][opcode2][operand2*] ...
```

### Opcodes
`[addr]` is an optional *Address* specifier. Whitespaces around opcodes (`m`, `a`, ...)
are added just for redability. `<regex>` is a regular expression enclosed by the first character.

```
[addr] x <char><regex><char>  -- Extract the matches (like `egrep -o`).
[addr] X <char><regex><char>  -- Select the whole matching addr/matches.
[addr] a <string>             -- Append a string.
[addr] i <string>             -- Prepend a string.
[addr] r <string>             -- Replace matches with string.
[addr] d                      -- Delete matches.
[addr] p<char><cmd><char>     -- Run a shell command. Specifically, for each
                                 match `m`, runs "echo `m` | cmd", and then
                                 replaces the range `m` with its output.
[addr] f <char>               -- Select the next occurrence of the character.
[addr] b <char>               -- Select the previous occurrence of the character.
[addr] s <char>               -- Select the string surrounded by the character.
                                 (excluding <char>).
[addr] S <char>               -- Select the string surrounded by the character
                                 (including <char>).
[addr] g [^|$%]               -- Go to:
                                     ^  -- The beginning of a line.
                                     |  -- The first non-whitespace character in
                                           a line.
                                     $  -- The end of a line.
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

### Regular Expressions
- `\1`, `\2`, ... are replaced by the groups in a match, for example,
  if `/a(.c)/r/__\1__` is applied to `"abc"`, it will be `"__bc__"`.

### Some Helpful Exceptions
- If no opcode is given (i.e. only `addr`), it will be interpreted as `[addr]g`. That is,
  it moves to the beginning of the address.
- The closing character in a regular expression can be omitted by EOF,
  i.e. `/foo` instead of `/foo/`.

### Notes
- `/foo` and `x/foo` outputs the same (single match of the next "foo"), however,
  they come from different concepts: *address* and *opcode*. Unlike the address,
  *x* opcode can output multiple matches in the text. For instance, `3,10x/foo/`
  outputs all occurences of "foo" in the line 3-10 (inclusive).
