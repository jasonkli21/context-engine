use super::*;
pub(super) struct PythonAnalyzer(SyntaxAnalyzer);
impl PythonAnalyzer {
    pub fn new() -> Result<Self, AnalysisError> {
        Ok(Self(SyntaxAnalyzer::new(
            tree_sitter_python::LANGUAGE.into(),
            include_str!("queries/python-symbols.scm"),
        )?))
    }
}
impl LanguageAnalyzer for PythonAnalyzer {
    fn language(&self) -> Language {
        Language::Python
    }
    fn analyze(&mut self, path: &Path, text: &str) -> Result<AnalyzedFile, AnalysisError> {
        self.0.analyze(self.language(), path, text, qualify)
    }
}
fn qualify(node: Node<'_>, text: &str, mut kind: SymbolKind) -> (SymbolKind, String, SourceRange) {
    let mut parts = vec![];
    if let Some(name) = node.child_by_field_name("name") {
        parts.push(node_text(name, text).to_owned());
    }
    let mut parent = node.parent();
    let mut nearest_definition = true;
    while let Some(p) = parent {
        if matches!(p.kind(), "class_definition" | "function_definition") {
            if nearest_definition && p.kind() == "class_definition" && kind == SymbolKind::Function
            {
                kind = SymbolKind::Method;
            }
            nearest_definition = false;
            if let Some(name) = p.child_by_field_name("name") {
                parts.push(node_text(name, text).into());
            }
        }
        parent = p.parent();
    }
    parts.reverse();
    (
        kind,
        parts.join("."),
        source_range(
            node.parent()
                .filter(|p| p.kind() == "decorated_definition")
                .unwrap_or(node),
        ),
    )
}
