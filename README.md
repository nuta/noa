noa
====

A simplistic, intuitive, temrinal-based text editor.

## Installation

- **For iTerm2 users:** Open `Preferences > Profiles > Keys` and enable `Report modifiers using CSI u`.

```
$ cargo install noa
```

## Key Bindings

| Key                                                      |                                                         |
|----------------------------------------------------------|---------------------------------------------------------|
| <kbd>Ctrl</kbd> + <kbd>Q</kbd>                           | Quit.                                                   |
| <kbd>Ctrl</kbd> + <kbd>S</kbd>                           | Save the current file.                                  |
| <kbd>Ctrl</kbd> + <kbd>X</kbd>                           | Cut.                                                    |
| <kbd>Ctrl</kbd> + <kbd>C</kbd>                           | Copy.                                                   |
| <kbd>Ctrl</kbd> + <kbd>V</kbd>                           | Paste.                                                  |
| <kbd>Ctrl</kbd> + <kbd>U</kbd>                           | Undo.                                                   |
| <kbd>Alt</kbd> + <kbd>Y</kbd>                            | Redo.                                                   |
| <kbd>Ctrl</kbd> + <kbd>F</kbd>                           | Finder.                                                 |
| <kbd>Ctrl</kbd> + <kbd>R</kbd>                           | Find a string.                                          |
| <kbd>Ctrl</kbd> + <kbd>H</kbd>                           | Select all occurences of the current selection or word. |
| <kbd>Ctrl</kbd> + <kbd>P</kbd>                           | Command Bar.                                            |
| <kbd>Ctrl</kbd> + <kbd>u</kbd>                           | Undo.                                                   |
| <kbd>Esc</kbd> / <kbd>Ctrl</kbd> + <kbd>G</kbd>          | Cancel.                                                 |
| <kbd>Shift</kbd> + <kbd>Arrow</kbd>                      | Select the text.                                        |
| <kbd>Ctrl</kbd> + <kbd>A</kbd>                           | Move to the beginning of the line.                      |
| <kbd>Ctrl</kbd> + <kbd>E</kbd>                           | Move to the end of the line.                            |
| <kbd>Alt</kbd> + <kbd>Left / Right</kbd>                 | Move by a word.                                         |
| <kbd>Ctrl</kbd> + <kbd>Alt</kbd> + <kbd>Up / Down</kbd>  | Add a cursor in the previous/next line.                 |
| <kbd>Alt</kbd> + <kbd>Up / Down</kbd>                    | Move the current line.                                  |
| <kbd>Alt</kbd> + <kbd>Shift</kbd> + <kbd>Up / Down</kbd> | Duplicate the current line.                             |
| <kbd>Ctrl</kbd> + <kbd>N</kbd>                           | Rename a symbol (LSP).                                  |

## Building

```
$ cargo build --release
$ ./target/release/noa
```

## License
CC0 or MIT. Choose whichever you prefer.

