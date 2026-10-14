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
// Reference/pointer targets and other unsupported forms supply no guessed container.
fn impl_target<'src>(
    mut node: Node<'_>,
    text: &'src str,
    budget: &mut AnalysisBudget,
) -> Result<Option<Vec<&'src str>>, AnalysisError> {
    let mut parts = vec![];
    loop {
        budget.visit_qualification_node()?;
        match node.kind() {
            "generic_type" => {
                let Some(target) = node.child_by_field_name("type") else {
                    return Ok(None);
                };
                node = target;
            }
            "scoped_type_identifier" | "scoped_identifier" => {
                let Some(name) = node.child_by_field_name("name") else {
                    return Ok(None);
                };
                if !matches!(name.kind(), "type_identifier" | "identifier") {
                    return Ok(None);
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
            _ => return Ok(None),
        }
    }
    Ok(Some(parts))
}

pub(super) fn qualify(
    node: Node<'_>,
    text: &str,
    mut kind: SymbolKind,
    budget: &mut AnalysisBudget,
) -> Result<(SymbolKind, String, SourceRange), AnalysisError> {
    let mut parts = vec![];
    if let Some(name) = node.child_by_field_name("name") {
        parts.push(node_text(name, text));
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
        budget.visit_qualification_node()?;
        match p.kind() {
            "impl_item" => {
                if let Some(target_parts) = p
                    .child_by_field_name("type")
                    .map(|n| impl_target(n, text, budget))
                    .transpose()?
                    .flatten()
                {
                    parts.extend(target_parts);
                }
            }
            "mod_item" | "trait_item" | "function_item" | "function_signature_item" => {
                if let Some(name) = p.child_by_field_name("name") {
                    parts.push(node_text(name, text));
                }
            }
            _ => {}
        }
        parent = p.parent();
    }
    parts.reverse();
    budget.charge_joined_name(&parts, "::")?;
    Ok((kind, parts.join("::"), source_range(node)))
}
