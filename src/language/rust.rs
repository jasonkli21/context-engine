use super::*;
pub(super) struct RustAnalyzer(SyntaxAnalyzer);
impl RustAnalyzer {
    pub fn new() -> Result<Self, AnalysisError> {
        Ok(Self(SyntaxAnalyzer::new(
            tree_sitter_rust::LANGUAGE.into(),
            include_str!("queries/rust-symbols.scm"),
        )?))
    }
}
impl LanguageAnalyzer for RustAnalyzer {
    fn language(&self) -> Language {
        Language::Rust
    }
    fn analyze(&mut self, path: &Path, text: &str) -> Result<AnalyzedFile, AnalysisError> {
        self.0.analyze(self.language(), path, text, qualify)
    }
}
// Nominal impl targets omit generic arguments, which may contain arbitrary expressions.
// Unsupported targets supply no guessed container name.
fn impl_target(mut node: Node<'_>, text: &str) -> Option<String> {
    let mut parts = vec![];
    loop {
        match node.kind() {
            "generic_type" | "reference_type" | "pointer_type" => {
                node = node.child_by_field_name("type")?;
            }
            "scoped_type_identifier" | "scoped_identifier" => {
                let name = node.child_by_field_name("name")?;
                if !matches!(name.kind(), "type_identifier" | "identifier") {
                    return None;
                }
                parts.push(node_text(name, text));
                if let Some(path) = node.child_by_field_name("path") {
                    node = path;
                } else {
                    break;
                }
            }
            "type_identifier" | "identifier" | "primitive_type" | "crate" | "self" | "super" => {
                parts.push(node_text(node, text));
                break;
            }
            _ => return None,
        }
    }
    parts.reverse();
    Some(parts.join("::"))
}

fn qualify(node: Node<'_>, text: &str, mut kind: SymbolKind) -> (SymbolKind, String, SourceRange) {
    let mut parts = vec![];
    if let Some(name) = node.child_by_field_name("name") {
        parts.push(node_text(name, text).to_owned());
    }
    let mut parent = node.parent();
    // Only members of an impl/trait declaration list are methods. Functions in
    // associated-constant initializer blocks are still free nested functions.
    if kind == SymbolKind::Function
        && node.parent().is_some_and(|p| {
            p.kind() == "declaration_list"
                && p.parent()
                    .is_some_and(|owner| matches!(owner.kind(), "impl_item" | "trait_item"))
        })
    {
        kind = SymbolKind::Method;
    }
    while let Some(p) = parent {
        match p.kind() {
            "impl_item" => {
                if let Some(target) = p
                    .child_by_field_name("type")
                    .and_then(|n| impl_target(n, text))
                {
                    parts.push(target);
                }
            }
            "mod_item" | "trait_item" | "function_item" | "function_signature_item" => {
                if let Some(name) = p.child_by_field_name("name") {
                    parts.push(node_text(name, text).into());
                }
            }
            _ => {}
        }
        parent = p.parent();
    }
    parts.reverse();
    (kind, parts.join("::"), source_range(node))
}
