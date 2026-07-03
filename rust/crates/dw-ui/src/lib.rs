pub fn banner(title: &str) -> String {
    format!("== {} ==", title)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

#[derive(Debug, Clone, Copy)]
pub struct TerminalTheme {
    enabled: bool,
}

impl TerminalTheme {
    pub fn new(mode: ColorMode, is_terminal: bool, no_color: bool) -> Self {
        let enabled = match mode {
            ColorMode::Always => true,
            ColorMode::Never => false,
            ColorMode::Auto => is_terminal && !no_color,
        };
        Self { enabled }
    }

    pub fn plain() -> Self {
        Self { enabled: false }
    }

    pub fn success(&self, text: &str) -> String {
        self.paint(anstyle::AnsiColor::Green.on_default().bold(), text)
    }

    pub fn warning(&self, text: &str) -> String {
        self.paint(anstyle::AnsiColor::Yellow.on_default().bold(), text)
    }

    pub fn error(&self, text: &str) -> String {
        self.paint(anstyle::AnsiColor::Red.on_default().bold(), text)
    }

    pub fn path(&self, text: &str) -> String {
        self.paint(anstyle::AnsiColor::Cyan.on_default(), text)
    }

    pub fn command(&self, text: &str) -> String {
        self.paint(anstyle::AnsiColor::Magenta.on_default(), text)
    }

    fn paint(&self, style: anstyle::Style, text: &str) -> String {
        if self.enabled {
            format!("{}{}{}", style.render(), text, style.render_reset())
        } else {
            text.into()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ColorMode, TerminalTheme};

    #[test]
    fn never_keeps_plain_text() {
        let theme = TerminalTheme::new(ColorMode::Never, true, false);
        assert_eq!(theme.success("OK"), "OK");
    }

    #[test]
    fn auto_disables_color_when_no_color_is_set() {
        let theme = TerminalTheme::new(ColorMode::Auto, true, true);
        assert_eq!(theme.warning("WARN"), "WARN");
    }

    #[test]
    fn always_emits_ansi() {
        let theme = TerminalTheme::new(ColorMode::Always, false, false);
        assert!(theme.error("ERR").contains("\u{1b}"));
    }
}
