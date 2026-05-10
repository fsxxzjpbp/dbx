use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OperationClass {
    Read,
    Write,
    Ddl,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RiskMetadata {
    pub operation_class: OperationClass,
    pub risk_level: RiskLevel,
    pub is_production: bool,
    pub production_reason: Option<String>,
    pub first_token: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct RiskContext<'a> {
    pub connection_name: &'a str,
    pub color: Option<&'a str>,
    pub environment_label: Option<&'a str>,
}

impl<'a> RiskContext<'a> {
    pub fn new(connection_name: &'a str) -> Self {
        Self { connection_name, color: None, environment_label: None }
    }

    pub fn with_color(mut self, color: Option<&'a str>) -> Self {
        self.color = color;
        self
    }

    pub fn with_environment_label(mut self, environment_label: Option<&'a str>) -> Self {
        self.environment_label = environment_label;
        self
    }
}

pub fn classify_sql(sql: &str) -> OperationClass {
    let tokens = executable_tokens(sql);
    classify_tokens(&tokens)
}

fn classify_tokens(tokens: &[String]) -> OperationClass {
    if tokens.iter().any(|token| is_ddl_token(token)) {
        return OperationClass::Ddl;
    }
    if tokens.iter().any(|token| is_write_token(token)) {
        return OperationClass::Write;
    }

    match tokens.first().map(String::as_str) {
        Some("SELECT" | "SHOW" | "DESCRIBE" | "EXPLAIN" | "WITH") => OperationClass::Read,
        _ => OperationClass::Unknown,
    }
}

pub fn risk_for(sql: &str, context: RiskContext<'_>) -> RiskMetadata {
    let operation_class = classify_sql(sql);
    let (is_production, production_reason) = production_signal(context);
    let risk_level = match (operation_class, is_production) {
        (OperationClass::Read, _) => RiskLevel::Low,
        (OperationClass::Write, _) if has_unfiltered_destructive_write(sql) => RiskLevel::Critical,
        (OperationClass::Write, false) => RiskLevel::Medium,
        (OperationClass::Write, true) => RiskLevel::High,
        (OperationClass::Ddl, _) => RiskLevel::Critical,
        (OperationClass::Unknown, _) => RiskLevel::High,
    };

    RiskMetadata {
        operation_class,
        risk_level,
        is_production,
        production_reason,
        first_token: first_executable_token(sql),
    }
}

pub fn risk_for_connection(sql: &str, connection_name: &str, color: Option<&str>) -> RiskMetadata {
    risk_for(sql, RiskContext::new(connection_name).with_color(color))
}

fn production_signal(context: RiskContext<'_>) -> (bool, Option<String>) {
    if let Some(environment_label) = context.environment_label {
        if contains_non_production_signal(environment_label) {
            return (false, None);
        }
        if contains_production_signal(environment_label) {
            return (true, Some("environment label".to_string()));
        }
    }

    if matches!(context.color, Some("#ef4444")) {
        return (true, Some("red connection color".to_string()));
    }

    if contains_production_signal(context.connection_name) {
        return (true, Some("connection name fallback".to_string()));
    }

    (false, None)
}

fn contains_production_signal(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    ["prod", "production", "live"].iter().any(|needle| value.contains(needle))
}

fn contains_non_production_signal(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    [
        "dev",
        "development",
        "test",
        "testing",
        "qa",
        "stage",
        "staging",
        "local",
        "sandbox",
        "non-prod",
        "non-production",
        "non production",
        "nonprod",
    ]
    .iter()
    .any(|needle| value.contains(needle))
}

fn is_write_token(token: &str) -> bool {
    matches!(token, "INSERT" | "UPDATE" | "DELETE" | "MERGE" | "REPLACE")
}

fn is_ddl_token(token: &str) -> bool {
    matches!(token, "CREATE" | "ALTER" | "DROP" | "TRUNCATE" | "RENAME")
}

fn has_unfiltered_destructive_write(sql: &str) -> bool {
    executable_statements(sql).into_iter().any(|statement| {
        statement.iter().enumerate().any(|(index, token)| {
            if !matches!(token.as_str(), "DELETE" | "UPDATE") {
                return false;
            }

            !statement[index + 1..].iter().any(|token| matches!(token.as_str(), "WHERE" | "LIMIT"))
        })
    })
}

fn executable_tokens(sql: &str) -> Vec<String> {
    executable_statements(sql).into_iter().flatten().collect()
}

fn executable_statements(sql: &str) -> Vec<Vec<String>> {
    let mut statements = Vec::new();
    let mut current = Vec::new();
    scan_executable_tokens(sql, &mut current, &mut statements);
    push_statement(&mut current, &mut statements);
    statements
}

fn scan_executable_tokens(sql: &str, current: &mut Vec<String>, statements: &mut Vec<Vec<String>>) {
    let bytes = sql.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }

        if bytes[i] == b';' {
            push_statement(current, statements);
            i += 1;
            continue;
        }

        if i + 1 < bytes.len() && bytes[i] == b'-' && bytes[i + 1] == b'-' {
            i += 2;
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }

        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            if i + 2 < bytes.len() && bytes[i + 2] == b'!' {
                let content_start = i + 3;
                let content_end = block_comment_end(bytes, content_start);
                scan_executable_tokens(&sql[content_start..content_end], current, statements);
                i = (content_end + 2).min(bytes.len());
            } else {
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                i = (i + 2).min(bytes.len());
            }
            continue;
        }

        if let Some(delimiter_len) = dollar_quote_delimiter_len(bytes, i) {
            let delimiter = &sql[i..i + delimiter_len];
            i += delimiter_len;
            if let Some(end) = sql[i..].find(delimiter) {
                i += end + delimiter_len;
            } else {
                i = bytes.len();
            }
            continue;
        }

        if matches!(bytes[i], b'\'' | b'"' | b'`') {
            let quote = bytes[i];
            i += 1;
            while i < bytes.len() {
                if bytes[i] == quote {
                    if i + 1 < bytes.len() && bytes[i + 1] == quote {
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }

        if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
            let start = i;
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            current.push(sql[start..i].to_ascii_uppercase());
            continue;
        }

        i += 1;
    }
}

fn push_statement(current: &mut Vec<String>, statements: &mut Vec<Vec<String>>) {
    if !current.is_empty() {
        statements.push(std::mem::take(current));
    }
}

fn block_comment_end(bytes: &[u8], mut i: usize) -> usize {
    while i + 1 < bytes.len() {
        if bytes[i] == b'*' && bytes[i + 1] == b'/' {
            return i;
        }
        i += 1;
    }
    bytes.len()
}

fn dollar_quote_delimiter_len(bytes: &[u8], start: usize) -> Option<usize> {
    if bytes.get(start) != Some(&b'$') {
        return None;
    }

    let mut i = start + 1;
    if bytes.get(i) == Some(&b'$') {
        return Some(2);
    }

    if !bytes.get(i).is_some_and(|byte| byte.is_ascii_alphabetic() || *byte == b'_') {
        return None;
    }

    i += 1;
    while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
        i += 1;
    }

    (bytes.get(i) == Some(&b'$')).then_some(i - start + 1)
}

fn first_executable_token(sql: &str) -> Option<String> {
    executable_tokens(sql).into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comments_do_not_hide_read_token() {
        assert_eq!(classify_sql("-- comment\nSELECT 1"), OperationClass::Read);
        assert_eq!(classify_sql("/* DROP TABLE x */ SELECT 1"), OperationClass::Read);
    }

    #[test]
    fn classifies_write_and_ddl() {
        assert_eq!(classify_sql("update users set name = 'a'"), OperationClass::Write);
        assert_eq!(classify_sql("DROP TABLE users"), OperationClass::Ddl);
    }

    #[test]
    fn with_does_not_hide_write_or_ddl() {
        assert_eq!(
            classify_sql("WITH moved AS (DELETE FROM orders RETURNING *) SELECT * FROM moved"),
            OperationClass::Write
        );
        assert_eq!(classify_sql("WITH dropped AS (DROP TABLE old_orders) SELECT 1"), OperationClass::Ddl);
    }

    #[test]
    fn explain_analyze_write_is_write() {
        assert_eq!(classify_sql("EXPLAIN ANALYZE UPDATE users SET name = 'a'"), OperationClass::Write);
    }

    #[test]
    fn dangerous_statement_in_multi_statement_sql_is_not_read() {
        assert_eq!(classify_sql("SELECT * FROM users; DELETE FROM users WHERE id = 1"), OperationClass::Write);
        assert_eq!(classify_sql("SHOW TABLES; DROP TABLE users"), OperationClass::Ddl);
    }

    #[test]
    fn red_color_marks_production() {
        let risk = risk_for_connection("SELECT * FROM orders", "prod-main", Some("#ef4444"));
        assert!(risk.is_production);
        assert_eq!(risk.risk_level, RiskLevel::Low);
    }

    #[test]
    fn environment_label_marks_production() {
        let risk = risk_for(
            "UPDATE orders SET status = 'done' WHERE id = 1",
            RiskContext { connection_name: "analytics", color: None, environment_label: Some("Production") },
        );
        assert!(risk.is_production);
        assert_eq!(risk.production_reason.as_deref(), Some("environment label"));
        assert_eq!(risk.risk_level, RiskLevel::High);
    }

    #[test]
    fn environment_label_overrides_color_and_name_fallback() {
        let non_prod_label = risk_for(
            "SELECT * FROM orders",
            RiskContext { connection_name: "prod-main", color: Some("#ef4444"), environment_label: Some("Staging") },
        );
        assert!(!non_prod_label.is_production);
        assert_eq!(non_prod_label.production_reason, None);

        let prod_label = risk_for(
            "SELECT * FROM orders",
            RiskContext { connection_name: "analytics", color: Some("#22c55e"), environment_label: Some("Production") },
        );
        assert!(prod_label.is_production);
        assert_eq!(prod_label.production_reason.as_deref(), Some("environment label"));
    }

    #[test]
    fn destructive_writes_without_where_or_limit_are_critical() {
        assert_eq!(risk_for("DELETE FROM users", RiskContext::new("dev")).risk_level, RiskLevel::Critical);
        assert_eq!(
            risk_for("UPDATE users SET active = false", RiskContext::new("dev")).risk_level,
            RiskLevel::Critical
        );
        assert_eq!(risk_for("DELETE FROM users WHERE id = 1", RiskContext::new("dev")).risk_level, RiskLevel::Medium);
        assert_eq!(risk_for("TRUNCATE TABLE users", RiskContext::new("dev")).risk_level, RiskLevel::Critical);
    }

    #[test]
    fn postgresql_dollar_quotes_do_not_contribute_tokens() {
        assert_eq!(classify_sql("SELECT $$ DELETE FROM users $$"), OperationClass::Read);
        assert_eq!(classify_sql("SELECT $tag$ DROP TABLE users $tag$"), OperationClass::Read);
    }

    #[test]
    fn mysql_executable_comment_contributes_tokens() {
        assert_eq!(classify_sql("/*!50000 DELETE FROM users */ SELECT 1"), OperationClass::Write);
        let risk = risk_for("/*! UPDATE users SET active = false */", RiskContext::new("dev"));
        assert_eq!(risk.operation_class, OperationClass::Write);
        assert_eq!(risk.first_token.as_deref(), Some("UPDATE"));
    }
}
