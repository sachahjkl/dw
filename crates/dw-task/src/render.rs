use dw_ui::TerminalTheme;

pub fn print_styled(line: &str) {
    println!("{}", TerminalTheme::stdout_auto().style_line(line, false));
}

pub fn print_styled_lines(lines: &[String]) {
    for line in lines {
        print_styled(line);
    }
}
