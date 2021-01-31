noa
====
[![Build Status](https://travis-ci.com/nuta/noa.svg?branch=master)](https://travis-ci.com/nuta/noa)
[![Latest version](https://img.shields.io/crates/v/noa.svg)](https://crates.io/crates/noa)

A simplistic terminal-based text editor which is surely not your cup of tea. By sacrificing your favorite Vim/Emacs/VS Code features, it allows you to focus on editing without any annoying distractions.

Features
--------
- Structural-regular-expression-based editing mode (see below)
- Fuzzy file search
- Mouse support
- EditorConfig support
- Auto indentation
- Soft wrap
- [ ] Undo & redo
- [ ] Copy & paste

Planned Features
----------------
- Word completion
- Language Server Protocol support
- Format on save
- Support opening a link on iTerm (something like `src/main.c:11`)

Unsupported Features (intentionally)
------------------------------------
- Tabs / Windows
- Syntax highlighting
- Multiple cursors

Installation
------------

```
$ cargo install noa
```

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

.+-j$a/;            -- Append a semicolon at the end of lines in the current selection.
#d                  -- Delete the current word.
S{r#/*\1*/#         -- Select the current code block enclosed by `{` and `}`
                       and then comment out the part by `/* */`.
,x/foo/Cr/"\1"/     -- Search all "foo", transform them to uppercase, and then
                       enclose them with double quotes: foo -> FOO -> "FOO"
.y/,/s"r/\0 /       -- "foo", "bar" => foo bar
,p(rustfmt)         -- Apply rustfmt(1) to the whole text.

11j                 -- Goto the 11th line.

/open_file/j        -- Move to the next occurrence of "open_file"
/open_file/         -- ditto (from *Some Helpful Exceptions* below)
/open_file          -- ditto (from *Some Helpful Exceptions* below)
```

### Syntax
The NED language consists of an address and arbitrary number of pairs of opecode
and its operand.

```
[addr?][opcode1][operand1*][whitespace?][opcode2][operand2*] ...
```

`[addr]` is an optional *address* specifier.

Each opcode takes as input a list of matches, edits matches if necessary, and
outputs a list of matches. For the first opcode, the range specified by
*address* is given as the single match.

### Opcodes
Whitespaces around opcodes (`m`, `a`, ...) are added just for redability.
`<regex>` is a regular expression enclosed by the 
first character. `</>` is an any character (except backslash) which delimits
a regular expression `<regex>`, string `<string>`, etc. `<pattern>` is a character
or regular expression enclosed by `/` if it starts with `/`.

```
g </><regex></>    -- Filter matches by the regex (like `egrep`).
v </><regex></>    -- Filter out matches by the regex (like `egrep -v`).
x </><regex></>    -- Extract the matches (like `egrep -o`).
y </><regex></>    -- Extract substring before/between/after the matches.
a </><string></>   -- Append a string.
i </><string></>   -- Prepend a string.
r </><string></>   -- Replace matches with string.
d                  -- Delete matches.
p </><string></>   -- Run a shell command. Specifically, for each match `m`,
                      runs "echo `m` | cmd", and then replaces the range `m`
                      with its output.
f <pattern>        -- Select the next occurrence of the character.
b <pattern>        -- Select the previous occurrence of the character.
s <pattern>        -- Select the string surrounded by the character
                      (excluding <pattern>).
S <pattern>        -- Select the string surrounded by the character
                      (including <pattern>).
j [^|$]            -- Jump To:
                            ^  -- The beginning of the match.
                            |  -- The first non-whitespace character in
                                  the match.
                            $  -- The end of a match.
                      (empty)  -- The beginning of first match.
c                  -- Transform to lowercase.
C                  -- Transform to uppercase.
```

### Address
*Address* is an inclusive range of text where the command will be applied.

```
.                   -- The current selection or cursor.
#                   -- The current word.
(number)            -- The whole line excluding the trailing newline. The number
                       represents the relative line number.
/regex/             -- A regular expression enclosed by `/` or `#`.
[addr1]+[addr2]     -- The range until [addr2] from the end of [addr1].
                       If [addr1] and/or [addr2] are omitted, they're interpreted
                       as `.` and `$` respectively. If [addr2] is (number), it's
                       intrepreted as a *relative* number.
[addr1]-[addr2]     -- Searches backwards for [addr2] from the beginning of [addr1].
                       If [addr1] and/or [addr2] are omitted, they're interpreted
                       as `.` and `0` respectively. If [addr2] is (number), it's
                       intrepreted as a *relative* number.
[addr1],[addr2]     -- The range of beginning of [addr1] and the end of [addr2].
                       If [addr1] and/or [addr2] are omitted, they're interpreted
                       as `0` and `$` respectively.
(empty)             -- Interpreted as `.+`.
```

### Regular Expressions
- `\1`, `\2`, ... are replaced by the groups in a match, for example,
  if `/a(.c)/r/__\1__` is applied to `"abc"`, it will be `"__bc__"`.

### Some Helpful Exceptions
- If no opcode is given (i.e. only `addr`), it will be interpreted as `[addr]j`. That is,
  it moves to the beginning of the address.
- The closing character in a regular expression can be omitted by EOF,
  i.e. `/foo` instead of `/foo/`.

### Notes
- `/foo` and `x/foo` outputs the same (single match of the next "foo"), however,
  they come from different concepts: *address* and *opcode*. Unlike the address,
  *x* opcode can output multiple matches in the text. For instance, `3,10x/foo/`
  outputs all occurences of "foo" in the line 3-10 (inclusive).

License
-------
CC0 or MIT. Choose whichever you prefer.
