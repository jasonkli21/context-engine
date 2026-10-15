//! Iterative, syntax-only normalization. No arbitrary expression text is retained.
use super::{node_text, source_range};
use crate::{classify::Language, relations::*};
use tree_sitter::Node;

#[derive(Clone, Copy)]
struct Limits {
    candidates: usize,
    bytes: usize,
    visits: usize,
    segments: usize,
    target_bytes: usize,
}
const LIMITS: Limits = Limits {
    candidates: 16_384,
    bytes: 4 * 1024 * 1024,
    visits: 50_000,
    segments: 64,
    target_bytes: 4096,
};
#[derive(Debug)]
enum Failure {
    Limit,
    Unsupported,
}
type Result<T> = std::result::Result<T, Failure>;
struct Extractor<'a> {
    text: &'a str,
    limits: Limits,
    visits: usize,
    bytes: usize,
    candidates: Vec<Candidate>,
    unsupported: usize,
}
impl<'a> Extractor<'a> {
    fn visit(&mut self) -> Result<()> {
        if self.visits >= self.limits.visits {
            return Err(Failure::Limit);
        }
        self.visits += 1;
        Ok(())
    }
    fn segment(&mut self, parts: &mut Vec<&'a str>, part: &'a str) -> Result<()> {
        if parts.len() >= self.limits.segments || part.len() > self.limits.target_bytes {
            return Err(Failure::Limit);
        }
        parts.push(part);
        Ok(())
    }
    fn path(&mut self, mut node: Node<'_>, python: bool) -> Result<Vec<&'a str>> {
        let mut parts = vec![];
        loop {
            self.visit()?;
            if node.has_error() || node.is_missing() {
                return Err(Failure::Unsupported);
            }
            match node.kind() {
                "identifier" | "crate" | "self" | "super" => {
                    self.segment(&mut parts, node_text(node, self.text))?;
                    break;
                }
                "scoped_identifier" if !python => {
                    let name = node
                        .child_by_field_name("name")
                        .ok_or(Failure::Unsupported)?;
                    if !matches!(name.kind(), "identifier" | "super") {
                        return Err(Failure::Unsupported);
                    }
                    self.segment(&mut parts, node_text(name, self.text))?;
                    if let Some(path) = node.child_by_field_name("path") {
                        node = path;
                    } else {
                        self.segment(&mut parts, "")?;
                        break;
                    }
                }
                "attribute" if python => {
                    let name = node
                        .child_by_field_name("attribute")
                        .ok_or(Failure::Unsupported)?;
                    if name.kind() != "identifier" {
                        return Err(Failure::Unsupported);
                    }
                    self.segment(&mut parts, node_text(name, self.text))?;
                    node = node
                        .child_by_field_name("object")
                        .ok_or(Failure::Unsupported)?;
                }
                "dotted_name" if python => {
                    // Children, rather than source slicing, reject arbitrary expressions/comments.
                    for i in (0..node.named_child_count()).rev() {
                        self.visit()?;
                        let child = node.named_child(i as u32).ok_or(Failure::Unsupported)?;
                        if child.kind() != "identifier" {
                            return Err(Failure::Unsupported);
                        }
                        self.segment(&mut parts, node_text(child, self.text))?;
                    }
                    break;
                }
                _ => return Err(Failure::Unsupported),
            }
        }
        parts.reverse();
        Ok(parts)
    }
    fn emit(
        &mut self,
        parts: &[&str],
        separator: &str,
        node: Node<'_>,
        binding: Option<&str>,
        wildcard: bool,
        derivation: Derivation,
    ) -> Result<()> {
        if parts.len() > self.limits.segments {
            return Err(Failure::Limit);
        }
        if parts.is_empty() {
            return Err(Failure::Unsupported);
        }
        let bytes = parts.iter().map(|s| s.len()).sum::<usize>()
            + separator.len() * parts.len().saturating_sub(1);
        let materialized = bytes.saturating_add(binding.map_or(0, str::len));
        if self.candidates.len() >= self.limits.candidates
            || bytes > self.limits.target_bytes
            || binding.is_some_and(|b| b.len() > self.limits.target_bytes)
            || materialized > self.limits.bytes.saturating_sub(self.bytes)
        {
            return Err(Failure::Limit);
        }
        self.bytes += materialized;
        let import = matches!(
            derivation,
            Derivation::RustUseSyntax | Derivation::PythonImportSyntax
        );
        self.candidates.push(Candidate {
            kind: if import {
                RelationKind::Imports
            } else {
                RelationKind::References
            },
            target: parts.join(separator),
            range: source_range(node),
            derivation,
            metadata: if import {
                Metadata::Import {
                    binding: binding.map(str::to_owned),
                    wildcard,
                }
            } else {
                Metadata::Reference {
                    context: ReferenceContext::Call,
                }
            },
        });
        Ok(())
    }
    fn rust_import(&mut self, root: Node<'_>) -> Result<()> {
        let arg = root
            .child_by_field_name("argument")
            .ok_or(Failure::Unsupported)?;
        let mut pending = vec![(arg, Vec::new())];
        while let Some((node, prefix)) = pending.pop() {
            self.visit()?;
            if node.has_error() {
                self.unsupported += 1;
                continue;
            }
            let result = match node.kind() {
                "scoped_use_list" => {
                    let mut next = prefix;
                    if let Some(path) = node.child_by_field_name("path") {
                        next.extend(self.path(path, false)?);
                    } else {
                        self.segment(&mut next, "")?;
                    }
                    if next.len() > self.limits.segments {
                        return Err(Failure::Limit);
                    }
                    pending.push((
                        node.child_by_field_name("list")
                            .ok_or(Failure::Unsupported)?,
                        next,
                    ));
                    Ok(())
                }
                "use_list" => {
                    for i in (0..node.named_child_count()).rev() {
                        self.visit()?;
                        let child = node.named_child(i as u32).ok_or(Failure::Unsupported)?;
                        if child.kind() != "line_comment" && child.kind() != "block_comment" {
                            pending.push((child, prefix.clone()));
                        }
                    }
                    Ok(())
                }
                _ => {
                    let (target, binding, wildcard) = match node.kind() {
                        "use_as_clause" => {
                            let alias = node
                                .child_by_field_name("alias")
                                .ok_or(Failure::Unsupported)?;
                            if alias.kind() != "identifier" {
                                return Err(Failure::Unsupported);
                            }
                            (
                                node.child_by_field_name("path"),
                                Some(node_text(alias, self.text)),
                                false,
                            )
                        }
                        "use_wildcard" => (node.named_child(0), None, true),
                        _ => (Some(node), None, false),
                    };
                    let mut parts = prefix;
                    if let Some(target) = target {
                        match self.path(target, false) {
                            Ok(path) => parts.extend(path),
                            Err(Failure::Unsupported) => {
                                self.unsupported += 1;
                                continue;
                            }
                            Err(e) => return Err(e),
                        }
                    }
                    self.emit(
                        &parts,
                        "::",
                        node,
                        binding,
                        wildcard,
                        Derivation::RustUseSyntax,
                    )
                }
            };
            match result {
                Err(Failure::Unsupported) => self.unsupported += 1,
                other => other?,
            }
        }
        Ok(())
    }
    fn python_prefix(&mut self, node: Node<'_>) -> Result<Vec<&'a str>> {
        if node.kind() != "relative_import" {
            return self.path(node, true);
        }
        let mut parts = vec![];
        for i in 0..node.named_child_count() {
            self.visit()?;
            let child = node.named_child(i as u32).ok_or(Failure::Unsupported)?;
            if child.kind() == "import_prefix" {
                for dot in node_text(child, self.text).chars() {
                    self.visit()?;
                    if dot != '.' {
                        return Err(Failure::Unsupported);
                    }
                    self.segment(&mut parts, "")?;
                }
            } else {
                parts.extend(self.path(child, true)?);
            }
        }
        if parts.len() > self.limits.segments {
            return Err(Failure::Limit);
        }
        Ok(parts)
    }
    fn python_import(&mut self, node: Node<'_>) -> Result<()> {
        if node.has_error() {
            return Err(Failure::Unsupported);
        }
        let module = node.child_by_field_name("module_name");
        let prefix = module
            .map(|n| self.python_prefix(n))
            .transpose()?
            .unwrap_or_default();
        for i in 0..node.named_child_count() {
            self.visit()?;
            let child = node.named_child(i as u32).ok_or(Failure::Unsupported)?;
            if Some(child) == module || matches!(child.kind(), "comment") {
                continue;
            }
            let wildcard = child.kind() == "wildcard_import";
            let (target, binding) = if child.kind() == "aliased_import" {
                let alias = child
                    .child_by_field_name("alias")
                    .ok_or(Failure::Unsupported)?;
                if alias.kind() != "identifier" {
                    return Err(Failure::Unsupported);
                }
                (
                    child.child_by_field_name("name"),
                    Some(node_text(alias, self.text)),
                )
            } else {
                ((!wildcard).then_some(child), None)
            };
            let mut parts = prefix.clone();
            if let Some(target) = target {
                parts.extend(self.path(target, true)?);
            }
            // A prefix of N empty segments joins to N-1 dots; preserve N leading dots for wildcard.
            if wildcard && parts.last() == Some(&"") {
                self.segment(&mut parts, "")?;
            }
            self.emit(
                &parts,
                ".",
                child,
                binding,
                wildcard,
                Derivation::PythonImportSyntax,
            )?;
        }
        Ok(())
    }
    fn run(&mut self, root: Node<'_>, python: bool) -> Result<()> {
        let mut walk = root.walk();
        loop {
            let node = walk.node();
            let result = match (python, node.kind()) {
                (false, "use_declaration") => self.rust_import(node),
                (true, "import_statement" | "import_from_statement") => self.python_import(node),
                (true, "future_import_statement") => Err(Failure::Unsupported),
                (false, "call_expression") | (true, "call") => {
                    let callee = node
                        .child_by_field_name("function")
                        .ok_or(Failure::Unsupported);
                    callee.and_then(|n| self.path(n, python)).and_then(|parts| {
                        self.emit(
                            &parts,
                            if python { "." } else { "::" },
                            node,
                            None,
                            false,
                            if python {
                                Derivation::PythonCallSyntax
                            } else {
                                Derivation::RustCallSyntax
                            },
                        )
                    })
                }
                _ => Ok(()),
            };
            match result {
                Err(Failure::Unsupported) => self.unsupported += 1,
                other => other?,
            }
            if walk.goto_first_child() {
                continue;
            }
            loop {
                if walk.goto_next_sibling() {
                    break;
                }
                if !walk.goto_parent() {
                    return Ok(());
                }
            }
        }
    }
}
pub(super) fn extract(root: Node<'_>, text: &str, language: &Language) -> Extraction {
    extract_with_limits(root, text, language, LIMITS)
}
fn extract_with_limits(
    root: Node<'_>,
    text: &str,
    language: &Language,
    limits: Limits,
) -> Extraction {
    let mut e = Extractor {
        text,
        limits,
        visits: 0,
        bytes: 0,
        candidates: vec![],
        unsupported: 0,
    };
    let status = match e.run(root, *language == Language::Python) {
        Ok(()) => RelationStatus::Analyzed,
        Err(Failure::Limit) => RelationStatus::AnalysisLimitExceeded,
        Err(Failure::Unsupported) => RelationStatus::ExtractionFailed,
    };
    if status != RelationStatus::Analyzed {
        e.candidates.clear();
    }
    Extraction {
        status,
        candidates: e.candidates,
        unsupported: e.unsupported,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn analyze(text: &str, python: bool, limits: Limits) -> Extraction {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&if python {
                tree_sitter_python::LANGUAGE.into()
            } else {
                tree_sitter_rust::LANGUAGE.into()
            })
            .unwrap();
        let tree = parser.parse(text, None).unwrap();
        assert!(
            !tree.root_node().has_error(),
            "{}",
            tree.root_node().to_sexp()
        );
        extract_with_limits(
            tree.root_node(),
            text,
            &if python {
                Language::Python
            } else {
                Language::Rust
            },
            limits,
        )
    }
    fn targets(e: &Extraction) -> Vec<&str> {
        e.candidates.iter().map(|c| c.target.as_str()).collect()
    }
    #[test]
    fn rust_import_matrix() {
        let e = analyze(
            "use foo::bar; use foo::bar as baz; use foo::{bar, baz}; use foo::{bar as b, nested::{Thing, Other}}; use foo::*; use crate::x::Y; use self::x; use super::x; use ::external::Thing;",
            false,
            LIMITS,
        );
        assert_eq!(e.status, RelationStatus::Analyzed);
        assert_eq!(e.unsupported, 0);
        assert_eq!(
            targets(&e),
            [
                "foo::bar",
                "foo::bar",
                "foo::bar",
                "foo::baz",
                "foo::bar",
                "foo::nested::Thing",
                "foo::nested::Other",
                "foo",
                "crate::x::Y",
                "self::x",
                "super::x",
                "::external::Thing"
            ]
        );
        assert_eq!(
            e.candidates[1].metadata,
            Metadata::Import {
                binding: Some("baz".into()),
                wildcard: false
            }
        );
        assert_eq!(
            e.candidates[4].metadata,
            Metadata::Import {
                binding: Some("b".into()),
                wildcard: false
            }
        );
        assert_eq!(
            e.candidates[7].metadata,
            Metadata::Import {
                binding: None,
                wildcard: true
            }
        );
        assert!(
            e.candidates
                .iter()
                .all(|c| c.derivation == Derivation::RustUseSyntax)
        );
    }
    #[test]
    fn python_import_matrix() {
        let e = analyze(
            "import foo\nimport foo.bar\nimport foo as f\nimport foo.bar as fb\nfrom foo import Bar\nfrom foo.bar import Baz\nfrom foo import A, B\nfrom foo import A as Alias\nfrom . import sibling\nfrom ..pkg import Name\nfrom foo import *\nfrom .. import *\n",
            true,
            LIMITS,
        );
        assert_eq!(e.status, RelationStatus::Analyzed);
        assert_eq!(e.unsupported, 0);
        assert_eq!(
            targets(&e),
            [
                "foo",
                "foo.bar",
                "foo",
                "foo.bar",
                "foo.Bar",
                "foo.bar.Baz",
                "foo.A",
                "foo.B",
                "foo.A",
                ".sibling",
                "..pkg.Name",
                "foo",
                ".."
            ]
        );
        assert_eq!(
            e.candidates[3].metadata,
            Metadata::Import {
                binding: Some("fb".into()),
                wildcard: false
            }
        );
        assert_eq!(
            e.candidates[8].metadata,
            Metadata::Import {
                binding: Some("Alias".into()),
                wildcard: false
            }
        );
        assert_eq!(
            e.candidates[11].metadata,
            Metadata::Import {
                binding: None,
                wildcard: true
            }
        );
    }
    #[test]
    fn conservative_call_forms_and_repeated_occurrences() {
        let rust = analyze(
            "fn f() { helper(); helper(); module::helper(); crate::module::helper(); obj.method(); (factory())(); arr[i](); }",
            false,
            LIMITS,
        );
        assert_eq!(
            targets(&rust),
            [
                "helper",
                "helper",
                "module::helper",
                "crate::module::helper",
                "factory"
            ]
        );
        assert_eq!(rust.unsupported, 3);
        assert_ne!(rust.candidates[0].range, rust.candidates[1].range);
        let python = analyze(
            "helper()\nmodule.helper()\nOuter.helper()\nself.method()\nfactory()()\nitems[0]()\n",
            true,
            LIMITS,
        );
        assert_eq!(
            targets(&python),
            [
                "helper",
                "module.helper",
                "Outer.helper",
                "self.method",
                "factory"
            ]
        );
        assert_eq!(python.unsupported, 2);
        assert!(python.candidates.iter().all(|c| matches!(
            c.metadata,
            Metadata::Reference {
                context: ReferenceContext::Call
            }
        )));
    }
    #[test]
    fn relation_budgets_exact_and_over_fail_closed() {
        let base = Limits {
            candidates: 2,
            bytes: 2,
            visits: 2,
            segments: 1,
            target_bytes: 1,
        };
        for python in [false, true] {
            let text = if python {
                "x()\nx()\n"
            } else {
                "fn f() { x(); x(); }"
            };
            assert_eq!(targets(&analyze(text, python, base)), ["x", "x"]);
            for limits in [
                Limits {
                    candidates: 1,
                    ..base
                },
                Limits { bytes: 1, ..base },
                Limits { visits: 1, ..base },
                Limits {
                    segments: 0,
                    ..base
                },
                Limits {
                    target_bytes: 0,
                    ..base
                },
            ] {
                let e = analyze(text, python, limits);
                assert_eq!(e.status, RelationStatus::AnalysisLimitExceeded);
                assert!(e.candidates.is_empty());
            }
        }
        // Alias materialization also consumes the aggregate byte budget.
        let text = "import x as alias\n";
        assert_eq!(
            analyze(text, true, Limits { bytes: 6, ..LIMITS }).status,
            RelationStatus::Analyzed
        );
        assert_eq!(
            analyze(text, true, Limits { bytes: 5, ..LIMITS }).status,
            RelationStatus::AnalysisLimitExceeded
        );
    }
    #[test]
    fn nested_paths_and_groups_have_independent_bounds() {
        for (text, python) in [
            ("fn f(){ a::b::c(); }", false),
            ("a.b.c()\n", true),
            ("use a::{b::{c}};", false),
            ("from ...pkg import Name\n", true),
        ] {
            let e = analyze(
                text,
                python,
                Limits {
                    segments: 2,
                    ..LIMITS
                },
            );
            assert_eq!(e.status, RelationStatus::AnalysisLimitExceeded);
            assert!(e.candidates.is_empty());
        }
        let long = format!("{}()\n", "a".repeat(4097));
        assert_eq!(
            analyze(&long, true, LIMITS).status,
            RelationStatus::AnalysisLimitExceeded
        );
        // Repeated targets hit aggregate bytes before the candidate ceiling.
        let repeated = format!("{}()\n", "x".repeat(1024)).repeat(4097);
        assert_eq!(
            analyze(
                &repeated,
                true,
                Limits {
                    visits: 100_000,
                    ..LIMITS
                }
            )
            .status,
            RelationStatus::AnalysisLimitExceeded
        );
    }
}
