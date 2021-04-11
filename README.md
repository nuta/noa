noa
====

A simplistic, intuitive, temrinal-based text editor.

## Installation

```
$ cargo install noa
```

## Features
- [x] Multi cursors support
- [ ] Soft wrapping
- [ ] Syntax highlighting
- [ ] Finder
- [ ] Command Bar
- [ ] Find a string
- [ ] Multi process syncing
- [ ] LSP support
- [ ] Jump List
- [ ] Git Line statuses
- [ ] Mouse cursor
- [ ] Clipboard
- [ ] Tmux integration
- [ ] Click a path in the terminal to open the file (iTerm2)

## Key Bindings

| Key                                             |                                            |
|-------------------------------------------------|--------------------------------------------|
| <kbd>Ctrl</kbd> + <kbd>Q</kbd>                  | Quit.                                      |
| <kbd>Ctrl</kbd> + <kbd>S</kbd>                  | Save the current file.                     |
| <kbd>Ctrl</kbd> + <kbd></kbd>                   | Cut.                                       |
| <kbd>Ctrl</kbd> + <kbd></kbd>                   | Copy.                                      |
| <kbd>Ctrl</kbd> + <kbd></kbd>                   | Undo.                                      |
| <kbd>Alt</kbd> + <kbd></kbd>                    | Redo.                                      |
| <kbd>Ctrl</kbd> + <kbd></kbd>                   | Finder.                                    |
| <kbd>Ctrl</kbd> + <kbd></kbd>                   | Find a string.                             |
| <kbd>Ctrl</kbd> + <kbd></kbd>                   | Select all occurences of the current word. |
| <kbd>Ctrl</kbd> + <kbd>P</kbd>                  | Command Bar.                               |
| <kbd>Esc</kbd> / <kbd>Ctrl</kbd> + <kbd>G</kbd> | Cancel.                                    |
| <kbd>Ctrl</kbd> + <kbd>V</kbd>                  | Paste.                                     |
| <kbd>Shift</kbd> + Arrow                        | Select the text.                           |
| <kbd>Ctrl</kbd> + <kbd>A</kbd>                  | Move to the beginning of the line.         |
| <kbd>Ctrl</kbd> + <kbd>E</kbd>                  | Move to the end of the line.               |
| <kbd>Alt</kbd> + <kbd>Left / Right</kbd>        | Move by a word.                            |
| <kbd>Ctrl</kbd> + <kbd>Up / Down</kbd>          | Add a cursor in the previous/next line.    |
| <kbd>Alt</kbd> + <kbd></kbd>                    | Swap lines.                                |
| <kbd>Alt</kbd> + <kbd></kbd>                    | Duplicate the current line.                |
| <kbd>Ctrl</kbd> + <kbd></kbd>                   | Rename a symbol (LSP).                     |

## Building

```
$ cargo build --release
$ ./target/release/noa
```

## License
CC0 or MIT. Choose whichever you prefer.

