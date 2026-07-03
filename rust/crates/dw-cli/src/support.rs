use anyhow::Result;

pub(crate) fn flag_value(value: bool) -> Option<&'static str> {
    value.then_some("true")
}

pub(crate) fn unsupported_command(name: &str, _selected: bool) -> Result<()> {
    Err(anyhow::anyhow!(
        "Commande `{name}` scaffolded dans le CLI Rust mais pas encore portee."
    ))
}

pub(crate) fn unsupported_command_with_args(
    name: &str,
    _args: &[(&str, Option<&str>)],
) -> Result<()> {
    unsupported_command(name, false)
}
