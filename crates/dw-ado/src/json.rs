use serde_json::Value;

pub(crate) fn field_text(fields: &Value, name: &str) -> Option<String> {
    fields.get(name).and_then(|value| element_text(Some(value)))
}

pub(crate) fn identity_text(value: Option<&Value>) -> Option<String> {
    let value = value?;
    value
        .get("displayName")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| element_text(Some(value)))
}

pub(crate) fn element_text(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Object(object) => object
            .get("displayName")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        Value::Null => None,
        other => Some(other.to_string()),
    }
}

pub(crate) fn work_item_id_from_relation_url(url: &str) -> Option<String> {
    let marker = "/workItems/";
    let index = url.find(marker)?;
    let id = &url[index + marker.len()..];
    Some(id.split(['/', '?']).next()?.to_string())
}

pub(crate) fn clean_text(value: Option<String>) -> Option<String> {
    let value = value?;
    let mut in_tag = false;
    let mut out = String::new();
    for c in value.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    let trimmed = out.replace("&nbsp;", " ").trim().to_string();
    (!trimmed.is_empty()).then_some(trimmed)
}
