use crate::query::QueryResult;

pub fn render_query_result_tsv(result: &QueryResult) -> String {
    let mut lines = vec![result.columns.join("\t")];
    lines.extend(result.rows.iter().map(|row| {
        row.iter()
            .map(|value| value.clone().unwrap_or_else(|| "NULL".into()))
            .collect::<Vec<_>>()
            .join("\t")
    }));
    lines.push(if result.truncated {
        format!("-- {} rows (truncated)", result.rows.len())
    } else {
        format!("-- {} rows", result.rows.len())
    });
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_tsv_with_null_and_truncation() {
        let result = QueryResult {
            columns: vec!["Id".into(), "Name".into()],
            rows: vec![vec![Some("1".into()), None]],
            truncated: true,
        };

        assert_eq!(
            render_query_result_tsv(&result),
            "Id\tName\n1\tNULL\n-- 1 rows (truncated)"
        );
    }
}
