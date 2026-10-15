//! Parser-specific syntax objects remain confined to this module tree.
mod python;
mod relations;
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
pub enum AnalysisError {
    #[error("{0}")]
    Failure(String),
    #[error("per-file symbol analysis limit exceeded")]
    LimitExceeded,
}

#[derive(Clone, Copy)]
struct AnalysisLimits {
    max_symbols: usize,
    max_name_bytes: usize,
    max_qualification_nodes: usize,
}

const ANALYSIS_LIMITS: AnalysisLimits = AnalysisLimits {
    max_symbols: 4_096,
    max_name_bytes: 4 * 1024 * 1024,
    max_qualification_nodes: 25_000,
};

struct AnalysisBudget {
    limits: AnalysisLimits,
    symbols: usize,
    name_bytes: usize,
    qualification_nodes: usize,
}

type QualifyFn = fn(
    Node<'_>,
    &str,
    SymbolKind,
    &mut AnalysisBudget,
) -> Result<(SymbolKind, String, SourceRange), AnalysisError>;

impl AnalysisBudget {
    fn new(limits: AnalysisLimits) -> Self {
        Self {
            limits,
            symbols: 0,
            name_bytes: 0,
            qualification_nodes: 0,
        }
    }

    fn reserve_symbol(&mut self) -> Result<(), AnalysisError> {
        if self.symbols >= self.limits.max_symbols {
            return Err(AnalysisError::LimitExceeded);
        }
        self.symbols += 1;
        Ok(())
    }

    fn charge_name_bytes(&mut self, bytes: usize) -> Result<(), AnalysisError> {
        if bytes > self.limits.max_name_bytes.saturating_sub(self.name_bytes) {
            return Err(AnalysisError::LimitExceeded);
        }
        self.name_bytes += bytes;
        Ok(())
    }

    fn charge_joined_name(&mut self, parts: &[&str], separator: &str) -> Result<(), AnalysisError> {
        let mut bytes = parts
            .len()
            .saturating_sub(1)
            .checked_mul(separator.len())
            .ok_or(AnalysisError::LimitExceeded)?;
        for part in parts {
            bytes = bytes
                .checked_add(part.len())
                .ok_or(AnalysisError::LimitExceeded)?;
        }
        self.charge_name_bytes(bytes)
    }

    fn visit_qualification_node(&mut self) -> Result<(), AnalysisError> {
        if self.qualification_nodes >= self.limits.max_qualification_nodes {
            return Err(AnalysisError::LimitExceeded);
        }
        self.qualification_nodes += 1;
        Ok(())
    }
}

pub trait LanguageAnalyzer {
    fn language(&self) -> Language;
    fn analyze(&mut self, path: &Path, text: &str) -> Result<AnalyzedFile, AnalysisError>;
    fn analyze_graph(&mut self, path: &Path, text: &str) -> Result<AnalyzedFile, AnalysisError>;
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
            .map_err(|e| AnalysisError::Failure(format!("parser initialization: {e}")))?;
        let query = Query::new(&language, query)
            .map_err(|e| AnalysisError::Failure(format!("embedded symbol query: {e}")))?;
        Ok(Self { parser, query })
    }
    fn analyze(
        &mut self,
        language: Language,
        path: &Path,
        text: &str,
        qualify: QualifyFn,
    ) -> Result<AnalyzedFile, AnalysisError> {
        self.analyze_mode(language, path, text, qualify, ANALYSIS_LIMITS, false)
    }

    #[cfg(test)]
    fn analyze_with_limits(
        &mut self,
        language: Language,
        path: &Path,
        text: &str,
        qualify: QualifyFn,
        limits: AnalysisLimits,
    ) -> Result<AnalyzedFile, AnalysisError> {
        self.analyze_mode(language, path, text, qualify, limits, false)
    }

    fn analyze_graph(
        &mut self,
        language: Language,
        path: &Path,
        text: &str,
        qualify: QualifyFn,
    ) -> Result<AnalyzedFile, AnalysisError> {
        self.analyze_mode(language, path, text, qualify, ANALYSIS_LIMITS, true)
    }

    fn analyze_mode(
        &mut self,
        language: Language,
        path: &Path,
        text: &str,
        qualify: QualifyFn,
        limits: AnalysisLimits,
        graph: bool,
    ) -> Result<AnalyzedFile, AnalysisError> {
        let tree = self
            .parser
            .parse(text, None)
            .ok_or_else(|| AnalysisError::Failure("parser returned no tree".into()))?;
        let mut result = AnalyzedFile {
            relations: None,
            symbols: vec![],
            diagnostics: vec![],
            syntax_error_count: 0,
        };
        let mut cursor = QueryCursor::new();
        cursor.set_match_limit(65_536);
        let mut matches = cursor.matches(&self.query, tree.root_node(), text.as_bytes());
        let mut budget = AnalysisBudget::new(limits);
        while let Some(m) = matches.next() {
            let mut declaration = None;
            let mut name = None;
            for capture in m.captures() {
                let label = self.query.capture_names()[capture.index as usize];
                if label == "name" {
                    if name.replace(capture.node).is_some() {
                        return Err(AnalysisError::Failure("duplicate name capture".into()));
                    }
                } else if let Some(kind) = capture_kind(label) {
                    if declaration.replace((capture.node, kind)).is_some() {
                        return Err(AnalysisError::Failure(
                            "duplicate declaration capture".into(),
                        ));
                    }
                } else {
                    return Err(AnalysisError::Failure(
                        "unknown embedded query capture".into(),
                    ));
                }
            }
            let (node, kind) = declaration
                .ok_or_else(|| AnalysisError::Failure("missing declaration capture".into()))?;
            let name = name.ok_or_else(|| AnalysisError::Failure("missing name capture".into()))?;
            budget.reserve_symbol()?;
            let local_name = node_text(name, text);
            budget.charge_name_bytes(local_name.len())?;
            let local = local_name.to_owned();
            let (kind, qualified_name, range) = qualify(node, text, kind, &mut budget)?;
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
            return Err(AnalysisError::Failure(
                "symbol query match limit exceeded; results discarded".into(),
            ));
        }
        if graph {
            result.relations = Some(relations::extract(tree.root_node(), text, &language));
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
