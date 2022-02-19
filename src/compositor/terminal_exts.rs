use std::fmt;

/// An terminal extension which allows applying multiple changes in the terminal
/// at once not to show intermediate results.
///
/// There're two specifications for this purpose and we support both of them:
///
/// - iTerm2 / Alacritty: <https://gitlab.com/gnachman/iterm2/-/wikis/synchronized-updates-spec>
/// - Contour: <https://gist.github.com/christianparpart/d8a62cc1ab659194337d73e399004036>
pub enum SynchronizedOutput {
    Begin,
    End,
}

impl crossterm::Command for SynchronizedOutput {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        let (param_2026, iterm2_op) = match self {
            SynchronizedOutput::Begin => ('h', '1'),
            SynchronizedOutput::End => ('l', '2'),
        };

        write!(
            f,
            concat!(
                "\x1b[?2026{}",    // CSI ? 2026 param
                "\x1bP={}s\x1b\\"  // ESC P = OP s ESC \
            ),
            param_2026, iterm2_op
        )
    }
}

/// Changes the cursor shape.
///
/// <https://www.vt100.net/docs/vt510-rm/DECSCUSR.html>
pub enum SetCursorShape {
    BlinkingBeam,
}

impl crossterm::Command for SetCursorShape {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        let shape = match self {
            SetCursorShape::BlinkingBeam => '5',
        };

        // CSI shape SP q
        write!(f, "\x1b[{} q", shape)
    }
}
