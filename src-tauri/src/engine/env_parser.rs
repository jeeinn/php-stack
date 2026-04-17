use std::collections::HashMap;
use std::fmt;

/// .env 文件中的一行
#[derive(Debug, Clone, PartialEq)]
pub enum EnvLine {
    /// 空行
    Empty,
    /// 注释行（包含 # 前缀）
    Comment(String),
    /// 键值对，可带行内注释
    KeyValue {
        key: String,
        value: String,
        inline_comment: Option<String>,
    },
}

/// 解析错误
#[derive(Debug, Clone)]
pub struct EnvParseError {
    pub line_number: usize,
    pub content: String,
    pub message: String,
}

impl fmt::Display for EnvParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Line {}: {} (content: '{}')",
            self.line_number, self.message, self.content
        )
    }
}

impl std::error::Error for EnvParseError {}

/// 解析后的 .env 文件
#[derive(Debug, Clone)]
pub struct EnvFile {
    pub lines: Vec<EnvLine>,
}

impl EnvFile {
    /// Parse .env content string into EnvFile.
    ///
    /// Handles: empty lines, comment lines (starting with #), key=value pairs,
    /// quoted values (single and double quotes), and inline comments.
    pub fn parse(content: &str) -> Result<Self, EnvParseError> {
        let mut lines = Vec::new();

        for (idx, raw_line) in content.lines().enumerate() {
            let line_number = idx + 1;
            let trimmed = raw_line.trim();

            // Empty line
            if trimmed.is_empty() {
                lines.push(EnvLine::Empty);
                continue;
            }

            // Comment line
            if trimmed.starts_with('#') {
                lines.push(EnvLine::Comment(raw_line.to_string()));
                continue;
            }

            // Must contain '=' for a key-value pair
            let eq_pos = match raw_line.find('=') {
                Some(pos) => pos,
                None => {
                    return Err(EnvParseError {
                        line_number,
                        content: raw_line.to_string(),
                        message: "Expected key=value pair, comment, or empty line".to_string(),
                    });
                }
            };

            let key = raw_line[..eq_pos].trim().to_string();
            if key.is_empty() {
                return Err(EnvParseError {
                    line_number,
                    content: raw_line.to_string(),
                    message: "Key cannot be empty".to_string(),
                });
            }

            let after_eq = &raw_line[eq_pos + 1..];
            let (value, inline_comment) = parse_value(after_eq);

            lines.push(EnvLine::KeyValue {
                key,
                value,
                inline_comment,
            });
        }

        Ok(EnvFile { lines })
    }

    /// Format back to .env string, preserving comments and empty lines.
    pub fn format(&self) -> String {
        let mut output = Vec::new();

        for line in &self.lines {
            match line {
                EnvLine::Empty => output.push(String::new()),
                EnvLine::Comment(text) => output.push(text.clone()),
                EnvLine::KeyValue {
                    key,
                    value,
                    inline_comment,
                } => {
                    let formatted = match inline_comment {
                        Some(comment) => format!("{}={} {}", key, value, comment),
                        None => format!("{}={}", key, value),
                    };
                    output.push(formatted);
                }
            }
        }

        output.join("\n")
    }

    /// Extract only key-value pairs, ignoring comments and empty lines.
    pub fn to_map(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        for line in &self.lines {
            if let EnvLine::KeyValue { key, value, .. } = line {
                map.insert(key.clone(), value.clone());
            }
        }
        map
    }

    /// Set a key-value pair. If key exists, update its value (preserve inline comment).
    /// If key doesn't exist, append as new KeyValue line.
    pub fn set(&mut self, key: &str, value: &str) {
        for line in &mut self.lines {
            if let EnvLine::KeyValue {
                key: k, value: v, ..
            } = line
            {
                if k == key {
                    *v = value.to_string();
                    return;
                }
            }
        }
        // Key not found, append new line
        self.lines.push(EnvLine::KeyValue {
            key: key.to_string(),
            value: value.to_string(),
            inline_comment: None,
        });
    }

    /// Return the value for a given key, or None.
    pub fn get(&self, key: &str) -> Option<&str> {
        for line in &self.lines {
            if let EnvLine::KeyValue { key: k, value, .. } = line {
                if k == key {
                    return Some(value.as_str());
                }
            }
        }
        None
    }

    /// Remove the line with the given key, return true if found.
    pub fn remove(&mut self, key: &str) -> bool {
        let len_before = self.lines.len();
        self.lines.retain(|line| {
            if let EnvLine::KeyValue { key: k, .. } = line {
                k != key
            } else {
                true
            }
        });
        self.lines.len() != len_before
    }
}

/// Parse the value portion after '=' in a .env line.
///
/// Rules:
/// - If value starts and ends with matching quotes (' or "), strip them.
///   Inside quotes, # is NOT treated as comment start.
/// - Outside quotes, # preceded by whitespace starts an inline comment.
/// - Leading/trailing whitespace on the unquoted value is trimmed.
fn parse_value(raw: &str) -> (String, Option<String>) {
    let trimmed = raw.trim_start();

    if trimmed.is_empty() {
        return (String::new(), None);
    }

    // Check for quoted value
    let first_char = trimmed.chars().next().unwrap();
    if first_char == '"' || first_char == '\'' {
        let quote = first_char;
        // Find the closing quote
        if let Some(close_pos) = trimmed[1..].find(quote) {
            let inner = &trimmed[1..1 + close_pos];
            let after_quote = &trimmed[1 + close_pos + 1..];

            // Check for inline comment after the closing quote
            let inline_comment = extract_inline_comment(after_quote);

            return (inner.to_string(), inline_comment);
        }
        // No closing quote found — treat the whole thing as unquoted
    }

    // Unquoted value: scan for inline comment (# preceded by whitespace)
    extract_unquoted_value(trimmed)
}

/// Extract an inline comment from text that follows a quoted value.
/// Looks for # preceded by whitespace.
fn extract_inline_comment(after_quote: &str) -> Option<String> {
    // Look for # preceded by whitespace in the remaining text
    let bytes = after_quote.as_bytes();
    for i in 0..bytes.len() {
        if bytes[i] == b'#' && i > 0 && bytes[i - 1] == b' ' {
            let comment = after_quote[i..].trim_end().to_string();
            if !comment.is_empty() {
                return Some(comment);
            }
        }
    }
    None
}

/// Parse an unquoted value, splitting off any inline comment.
/// A # is treated as comment start only if preceded by whitespace.
fn extract_unquoted_value(s: &str) -> (String, Option<String>) {
    let bytes = s.as_bytes();

    for i in 1..bytes.len() {
        if bytes[i] == b'#' && bytes[i - 1] == b' ' {
            let value = s[..i].trim_end().to_string();
            let comment = s[i..].trim_end().to_string();
            let inline_comment = if comment.is_empty() {
                None
            } else {
                Some(comment)
            };
            return (value, inline_comment);
        }
    }

    // No inline comment found
    (s.trim_end().to_string(), None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_key_value() {
        let content = "KEY=value";
        let env = EnvFile::parse(content).unwrap();
        assert_eq!(env.lines.len(), 1);
        assert_eq!(
            env.lines[0],
            EnvLine::KeyValue {
                key: "KEY".to_string(),
                value: "value".to_string(),
                inline_comment: None,
            }
        );
    }

    #[test]
    fn test_parse_empty_value() {
        let content = "EMPTY_VALUE=";
        let env = EnvFile::parse(content).unwrap();
        assert_eq!(env.get("EMPTY_VALUE"), Some(""));
    }

    #[test]
    fn test_parse_comment_line() {
        let content = "# This is a comment";
        let env = EnvFile::parse(content).unwrap();
        assert_eq!(env.lines.len(), 1);
        assert_eq!(
            env.lines[0],
            EnvLine::Comment("# This is a comment".to_string())
        );
    }

    #[test]
    fn test_parse_empty_line() {
        let content = "";
        let env = EnvFile::parse(content).unwrap();
        // An empty string has one empty line when split by lines()
        // Actually, "".lines() yields 0 items
        assert_eq!(env.lines.len(), 0);
    }

    #[test]
    fn test_parse_single_empty_line() {
        let content = "\n";
        let env = EnvFile::parse(content).unwrap();
        // "\n".lines() yields [""]
        assert_eq!(env.lines.len(), 1);
        assert_eq!(env.lines[0], EnvLine::Empty);
    }

    #[test]
    fn test_parse_double_quoted_value() {
        let content = r#"KEY="hello world""#;
        let env = EnvFile::parse(content).unwrap();
        assert_eq!(env.get("KEY"), Some("hello world"));
    }

    #[test]
    fn test_parse_single_quoted_value() {
        let content = "KEY='hello world'";
        let env = EnvFile::parse(content).unwrap();
        assert_eq!(env.get("KEY"), Some("hello world"));
    }

    #[test]
    fn test_parse_inline_comment() {
        let content = "KEY=value # this is a comment";
        let env = EnvFile::parse(content).unwrap();
        assert_eq!(env.lines.len(), 1);
        match &env.lines[0] {
            EnvLine::KeyValue {
                key,
                value,
                inline_comment,
            } => {
                assert_eq!(key, "KEY");
                assert_eq!(value, "value");
                assert_eq!(
                    inline_comment,
                    &Some("# this is a comment".to_string())
                );
            }
            _ => panic!("Expected KeyValue"),
        }
    }

    #[test]
    fn test_parse_hash_inside_quotes_not_comment() {
        let content = r#"KEY="value # not a comment""#;
        let env = EnvFile::parse(content).unwrap();
        assert_eq!(env.get("KEY"), Some("value # not a comment"));
    }

    #[test]
    fn test_parse_hash_without_preceding_space() {
        let content = "KEY=value#not_a_comment";
        let env = EnvFile::parse(content).unwrap();
        assert_eq!(env.get("KEY"), Some("value#not_a_comment"));
    }

    #[test]
    fn test_parse_windows_path() {
        let content = r"SOURCE_DIR=E:\projects";
        let env = EnvFile::parse(content).unwrap();
        assert_eq!(env.get("SOURCE_DIR"), Some(r"E:\projects"));
    }

    #[test]
    fn test_parse_error_no_equals() {
        let content = "INVALID_LINE";
        let result = EnvFile::parse(content);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.line_number, 1);
        assert_eq!(err.content, "INVALID_LINE");
    }

    #[test]
    fn test_format_roundtrip() {
        let content = "# Comment\n\nKEY=value\nQUOTED=hello\nWITH_COMMENT=val # comment";
        let env = EnvFile::parse(content).unwrap();
        let formatted = env.format();
        let env2 = EnvFile::parse(&formatted).unwrap();
        assert_eq!(env.to_map(), env2.to_map());
    }

    #[test]
    fn test_to_map() {
        let content = "# Comment\nA=1\nB=2\n\nC=3";
        let env = EnvFile::parse(content).unwrap();
        let map = env.to_map();
        assert_eq!(map.len(), 3);
        assert_eq!(map.get("A"), Some(&"1".to_string()));
        assert_eq!(map.get("B"), Some(&"2".to_string()));
        assert_eq!(map.get("C"), Some(&"3".to_string()));
    }

    #[test]
    fn test_set_existing_key() {
        let content = "KEY=old # comment";
        let mut env = EnvFile::parse(content).unwrap();
        env.set("KEY", "new");
        assert_eq!(env.get("KEY"), Some("new"));
        // Inline comment should be preserved
        match &env.lines[0] {
            EnvLine::KeyValue {
                inline_comment, ..
            } => {
                assert_eq!(inline_comment, &Some("# comment".to_string()));
            }
            _ => panic!("Expected KeyValue"),
        }
    }

    #[test]
    fn test_set_new_key() {
        let content = "A=1";
        let mut env = EnvFile::parse(content).unwrap();
        env.set("B", "2");
        assert_eq!(env.get("B"), Some("2"));
        assert_eq!(env.lines.len(), 2);
    }

    #[test]
    fn test_get_nonexistent() {
        let content = "A=1";
        let env = EnvFile::parse(content).unwrap();
        assert_eq!(env.get("B"), None);
    }

    #[test]
    fn test_remove_existing() {
        let content = "A=1\nB=2\nC=3";
        let mut env = EnvFile::parse(content).unwrap();
        assert!(env.remove("B"));
        assert_eq!(env.get("B"), None);
        assert_eq!(env.lines.len(), 2);
    }

    #[test]
    fn test_remove_nonexistent() {
        let content = "A=1";
        let mut env = EnvFile::parse(content).unwrap();
        assert!(!env.remove("B"));
    }

    #[test]
    fn test_parse_full_example() {
        let content = r#"# This is a comment
SOURCE_DIR=E:\projects

PHP82_VERSION=8.2.27
MYSQL_HOST_PORT=3306
PHP82_EXTENSIONS=pdo_mysql,mysqli,gd,curl,opcache

# Mirror settings
APT_MIRROR=aliyun  # Aliyun mirror
COMPOSER_MIRROR="aliyun"
EMPTY_VALUE=
QUOTED_VALUE='hello world'"#;

        let env = EnvFile::parse(content).unwrap();
        let map = env.to_map();

        assert_eq!(map.get("SOURCE_DIR"), Some(&r"E:\projects".to_string()));
        assert_eq!(map.get("PHP82_VERSION"), Some(&"8.2.27".to_string()));
        assert_eq!(map.get("MYSQL_HOST_PORT"), Some(&"3306".to_string()));
        assert_eq!(
            map.get("PHP82_EXTENSIONS"),
            Some(&"pdo_mysql,mysqli,gd,curl,opcache".to_string())
        );
        assert_eq!(map.get("APT_MIRROR"), Some(&"aliyun".to_string()));
        assert_eq!(map.get("COMPOSER_MIRROR"), Some(&"aliyun".to_string()));
        assert_eq!(map.get("EMPTY_VALUE"), Some(&"".to_string()));
        assert_eq!(map.get("QUOTED_VALUE"), Some(&"hello world".to_string()));

        // Verify inline comment on APT_MIRROR
        let apt_line = env.lines.iter().find(|l| {
            matches!(l, EnvLine::KeyValue { key, .. } if key == "APT_MIRROR")
        });
        match apt_line {
            Some(EnvLine::KeyValue {
                inline_comment, ..
            }) => {
                assert_eq!(inline_comment, &Some("# Aliyun mirror".to_string()));
            }
            _ => panic!("Expected APT_MIRROR KeyValue with inline comment"),
        }
    }
}
