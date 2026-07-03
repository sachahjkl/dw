use serde::Deserialize;
use serde_json::Value;
use std::{fs, path::Path};

pub(crate) fn read_json<T>(path: impl AsRef<Path>) -> Option<T>
where
    T: for<'de> Deserialize<'de>,
{
    let path = path.as_ref();
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str::<T>(&text).ok()
}

pub(crate) fn read_json_value(path: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(path).map_err(|_| "fichier manquant".to_string())?;
    serde_json::from_str::<Value>(&text).map_err(|error| error.to_string())
}

pub(crate) fn read_jsonc_value(path: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(path).map_err(|_| "fichier manquant".to_string())?;
    serde_json::from_str::<Value>(&strip_jsonc_comments(&text)).map_err(|error| error.to_string())
}

fn strip_jsonc_comments(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;
    while let Some(ch) = chars.next() {
        if in_string {
            output.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        if ch == '"' {
            in_string = true;
            output.push(ch);
            continue;
        }
        if ch == '/' && chars.peek() == Some(&'/') {
            let _ = chars.next();
            for next in chars.by_ref() {
                if next == '\n' {
                    output.push('\n');
                    break;
                }
            }
            continue;
        }
        output.push(ch);
    }
    output
}
