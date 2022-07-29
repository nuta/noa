// You can check for the existence of subcommands, and if found use their matches just as you would the top level cmd. You can check the value provided by positional arguments, or option arguments ;)





        crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen).unwrap();

        // Print captured stderr.
        drop(stderr_hold);

        // Resume panic unwind.
        std::panic::resume_unwind(e);
