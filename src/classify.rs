use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Language {
    Rust,
    Python,
    TypeScript,
    JavaScript,
    Markdown,
    Json,
    Yaml,
    Toml,
    Shell,
    Sql,
    Other(String),
    Unknown,
}

impl Language {
    pub fn name(&self) -> &str {
        match self {
            Self::Rust => "Rust",
            Self::Python => "Python",
            Self::TypeScript => "TypeScript",
            Self::JavaScript => "JavaScript",
            Self::Markdown => "Markdown",
            Self::Json => "JSON",
            Self::Yaml => "YAML",
            Self::Toml => "TOML",
            Self::Shell => "Shell",
            Self::Sql => "SQL",
            Self::Other(name) => name,
            Self::Unknown => "Unknown",
        }
    }
    pub fn is_text(&self) -> bool {
        !matches!(self, Self::Unknown)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FileRole {
    Source,
    Test,
    Documentation,
    Configuration,
    Schema,
    Migration,
    Generated,
    Vendor,
    Unknown,
}
impl FileRole {
    pub fn name(self) -> &'static str {
        match self {
            Self::Source => "Source",
            Self::Test => "Test",
            Self::Documentation => "Documentation",
            Self::Configuration => "Configuration",
            Self::Schema => "Schema",
            Self::Migration => "Migration",
            Self::Generated => "Generated",
            Self::Vendor => "Vendor",
            Self::Unknown => "Unknown",
        }
    }
}

pub fn classify_language(path: &Path) -> Language {
    match path.file_name().and_then(|n| n.to_str()) {
        Some("Dockerfile") => return Language::Other("Dockerfile".into()),
        Some("Makefile" | "GNUmakefile") => return Language::Other("Makefile".into()),
        _ => {}
    }
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("rs") => Language::Rust,
        Some("py" | "pyi") => Language::Python,
        Some("ts" | "tsx" | "mts" | "cts") => Language::TypeScript,
        Some("js" | "jsx" | "mjs" | "cjs") => Language::JavaScript,
        Some("md" | "mdx") => Language::Markdown,
        Some("json") => Language::Json,
        Some("yaml" | "yml") => Language::Yaml,
        Some("toml") => Language::Toml,
        Some("sh" | "bash") => Language::Shell,
        Some("sql") => Language::Sql,
        _ => Language::Unknown,
    }
}

// Precedence: vendor > generated > test > migration > schema > docs > config > source.
pub fn classify_role(path: &Path, language: &Language) -> FileRole {
    let parts: Vec<_> = path
        .iter()
        .filter_map(|p| p.to_str())
        .map(str::to_ascii_lowercase)
        .collect();
    let name = parts.last().map(String::as_str).unwrap_or("");
    let has = |names: &[&str]| parts.iter().any(|p| names.contains(&p.as_str()));
    if has(&["vendor", "node_modules", "third_party"]) {
        FileRole::Vendor
    } else if has(&["generated", "dist", "build"]) || name.contains(".generated.") {
        FileRole::Generated
    } else if has(&["tests", "test", "__tests__"])
        || name.starts_with("test_")
        || name.contains(".test.")
        || name.contains(".spec.")
        || name.ends_with("_test.py")
    {
        FileRole::Test
    } else if has(&["migrations", "migration"]) {
        FileRole::Migration
    } else if has(&["schemas", "schema"]) {
        FileRole::Schema
    } else if has(&["docs", "doc"])
        || matches!(language, Language::Markdown)
        || name.starts_with("readme")
    {
        FileRole::Documentation
    } else if has(&["config", "configs", ".github"])
        || matches!(language, Language::Json | Language::Yaml | Language::Toml)
        || matches!(
            name,
            "dockerfile" | "makefile" | "gnumakefile" | ".gitignore" | ".ignore"
        )
    {
        FileRole::Configuration
    } else if !matches!(language, Language::Unknown) {
        FileRole::Source
    } else {
        FileRole::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn language_table() {
        for (name, expected) in [
            ("a.rs", Language::Rust),
            ("a.py", Language::Python),
            ("a.ts", Language::TypeScript),
            ("a.tsx", Language::TypeScript),
            ("a.js", Language::JavaScript),
            ("a.jsx", Language::JavaScript),
            ("a.md", Language::Markdown),
            ("a.mdx", Language::Markdown),
            ("a.json", Language::Json),
            ("a.yaml", Language::Yaml),
            ("a.yml", Language::Yaml),
            ("a.toml", Language::Toml),
            ("a.sh", Language::Shell),
            ("a.bash", Language::Shell),
            ("a.sql", Language::Sql),
            ("a.weird", Language::Unknown),
            ("a.RS", Language::Rust),
            ("Dockerfile", Language::Other("Dockerfile".into())),
        ] {
            assert_eq!(classify_language(Path::new(name)), expected, "{name}");
        }
    }
    #[test]
    fn roles_and_precedence() {
        for (name, role) in [
            ("src/main.rs", FileRole::Source),
            ("tests/cli.rs", FileRole::Test),
            ("test_auth.py", FileRole::Test),
            ("docs/architecture.md", FileRole::Documentation),
            ("README.md", FileRole::Documentation),
            ("Cargo.toml", FileRole::Configuration),
            ("migrations/001.sql", FileRole::Migration),
            ("schemas/openapi.yaml", FileRole::Schema),
            ("vendor/tests/a.rs", FileRole::Vendor),
            ("generated/tests/a.rs", FileRole::Generated),
            ("tests/schemas/a.yaml", FileRole::Test),
            ("migrations/schema/a.sql", FileRole::Migration),
            ("x.bin", FileRole::Unknown),
        ] {
            let path = Path::new(name);
            assert_eq!(
                classify_role(path, &classify_language(path)),
                role,
                "{name}"
            );
        }
    }
}
