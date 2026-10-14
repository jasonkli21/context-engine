//! Parser-specific syntax objects remain confined to this module tree.
mod python;
mod rust;
use crate::{
    classify::Language,
    model::{
        AnalysisDiagnostic, AnalyzedFile, DiagnosticKind, SourcePoint, SourceRange, Symbol,
        SymbolKind,
    },
};
use std::path::Path;
use tree_sitter::{Node, Parser, Query, QueryCursor, StreamingIterator};

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct AnalysisError(pub String);
pub trait LanguageAnalyzer {
    fn language(&self) -> Language;
    fn analyze(&mut self, path: &Path, text: &str) -> Result<AnalyzedFile, AnalysisError>;
}
pub struct AnalyzerRegistry {
    rust: rust::RustAnalyzer,
    python: python::PythonAnalyzer,
}
impl AnalyzerRegistry {
    pub fn new() -> Result<Self, AnalysisError> {
        Ok(Self {
            rust: rust::RustAnalyzer::new()?,
            python: python::PythonAnalyzer::new()?,
        })
    }
    pub fn analyzer_for(&mut self, language: &Language) -> Option<&mut dyn LanguageAnalyzer> {
        match language {
            Language::Rust => Some(&mut self.rust),
            Language::Python => Some(&mut self.python),
            _ => None,
        }
    }
}
struct SyntaxAnalyzer {
    parser: Parser,
    query: Query,
}
impl SyntaxAnalyzer {
    fn new(language: tree_sitter::Language, query: &str) -> Result<Self, AnalysisError> {
        let mut parser = Parser::new();
        parser
            .set_language(&language)
            .map_err(|e| AnalysisError(format!("parser initialization: {e}")))?;
        let query = Query::new(&language, query)
            .map_err(|e| AnalysisError(format!("embedded symbol query: {e}")))?;
        Ok(Self { parser, query })
    }
    fn analyze(
        &mut self,
        language: Language,
        path: &Path,
        text: &str,
        qualify: fn(Node<'_>, &str, SymbolKind) -> (SymbolKind, String, SourceRange),
    ) -> Result<AnalyzedFile, AnalysisError> {
        let tree = self
            .parser
            .parse(text, None)
            .ok_or_else(|| AnalysisError("parser returned no tree".into()))?;
        let mut result = AnalyzedFile {
            symbols: vec![],
            diagnostics: vec![],
            syntax_error_count: 0,
        };
        let mut cursor = QueryCursor::new();
        cursor.set_match_limit(65_536);
        let mut matches = cursor.matches(&self.query, tree.root_node(), text.as_bytes());
        while let Some(m) = matches.next() {
            let mut declaration = None;
            let mut name = None;
            for capture in m.captures() {
                let label = self.query.capture_names()[capture.index as usize];
                if label == "name" {
                    if name.replace(capture.node).is_some() {
                        return Err(AnalysisError("duplicate name capture".into()));
                    }
                } else if let Some(kind) = capture_kind(label) {
                    if declaration.replace((capture.node, kind)).is_some() {
                        return Err(AnalysisError("duplicate declaration capture".into()));
                    }
                } else {
                    return Err(AnalysisError("unknown embedded query capture".into()));
                }
            }
            let (node, kind) =
                declaration.ok_or_else(|| AnalysisError("missing declaration capture".into()))?;
            let name = name.ok_or_else(|| AnalysisError("missing name capture".into()))?;
            let local = node_text(name, text).to_owned();
            let (kind, qualified_name, range) = qualify(node, text, kind);
            result.symbols.push(Symbol {
                relative_path: path.into(),
                language: language.clone(),
                kind,
                name: local,
                qualified_name,
                range,
                name_range: source_range(name),
            });
        }
        if cursor.did_exceed_match_limit() {
            return Err(AnalysisError(
                "symbol query match limit exceeded; results discarded".into(),
            ));
        }
        // Cursor traversal avoids call-stack growth on deeply nested untrusted input.
        let mut walk = tree.walk();
        loop {
            let node = walk.node();
            if node.is_error() || node.is_missing() {
                result.syntax_error_count += 1;
                if result.diagnostics.len() < 20 {
                    result.diagnostics.push(AnalysisDiagnostic {
                        relative_path: path.into(),
                        kind: if node.is_missing() {
                            DiagnosticKind::MissingSyntax
                        } else {
                            DiagnosticKind::SyntaxError
                        },
                        message: if node.is_missing() {
                            "missing syntax"
                        } else {
                            "syntax error"
                        }
                        .into(),
                        range: Some(source_range(node)),
                    });
                }
            }
            if walk.goto_first_child() {
                continue;
            }
            loop {
                if walk.goto_next_sibling() {
                    break;
                }
                if !walk.goto_parent() {
                    return Ok(result);
                }
            }
        }
    }
}
fn capture_kind(label: &str) -> Option<SymbolKind> {
    Some(match label {
        "symbol.module" => SymbolKind::Module,
        "symbol.struct" => SymbolKind::Struct,
        "symbol.enum" => SymbolKind::Enum,
        "symbol.union" => SymbolKind::Union,
        "symbol.trait" => SymbolKind::Trait,
        "symbol.class" => SymbolKind::Class,
        "symbol.function" => SymbolKind::Function,
        "symbol.type_alias" => SymbolKind::TypeAlias,
        "symbol.constant" => SymbolKind::Constant,
        "symbol.static" => SymbolKind::Static,
        _ => return None,
    })
}
fn node_text<'a>(node: Node<'_>, text: &'a str) -> &'a str {
    &text[node.byte_range()]
}
fn source_range(node: Node<'_>) -> SourceRange {
    let start = node.start_position();
    let end = node.end_position();
    SourceRange {
        start: SourcePoint {
            byte: node.start_byte(),
            row: start.row,
            column: start.column,
        },
        end: SourcePoint {
            byte: node.end_byte(),
            row: end.row,
            column: end.column,
        },
    }
}
#[cfg(test)]
mod tests;
