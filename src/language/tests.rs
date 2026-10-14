use super::*;
use crate::model::SymbolKind::*;
fn analyze(language: Language, text: &str) -> AnalyzedFile {
    let mut registry = AnalyzerRegistry::new().unwrap();
    let analyzer = registry.analyzer_for(&language).unwrap();
    assert_eq!(analyzer.language(), language);
    let result = analyzer.analyze(Path::new("fixture"), text).unwrap();
    for symbol in &result.symbols {
        let r = symbol.name_range;
        assert_eq!(&text[r.start.byte..r.end.byte], symbol.name);
        assert_eq!(symbol.relative_path, Path::new("fixture"));
        assert_eq!(symbol.language, language);
        assert!(symbol.range.start.byte <= r.start.byte && symbol.range.end.byte >= r.end.byte);
        for point in [symbol.range.start, symbol.range.end, r.start, r.end] {
            let prefix = &text[..point.byte];
            assert_eq!(prefix.bytes().filter(|b| *b == b'\n').count(), point.row);
            assert_eq!(prefix.rsplit('\n').next().unwrap().len(), point.column);
        }
        assert!(!text[symbol.range.start.byte..symbol.range.end.byte].is_empty());
    }
    result
}
fn assert_symbols(result: &AnalyzedFile, expected: &[(&str, SymbolKind)]) {
    let actual: Vec<_> = result
        .symbols
        .iter()
        .map(|s| (s.qualified_name.as_str(), s.kind))
        .collect();
    assert_eq!(actual.len(), expected.len(), "{actual:?}");
    for entry in expected {
        assert!(actual.contains(entry), "missing {entry:?} in {actual:?}");
    }
}
#[test]
fn initialization_registry_and_bad_query() {
    let mut registry = AnalyzerRegistry::new().unwrap();
    assert!(registry.analyzer_for(&Language::TypeScript).is_none());
    assert!(SyntaxAnalyzer::new(tree_sitter_rust::LANGUAGE.into(), "(not_a_node) @name").is_err());
    for language in [Language::Rust, Language::Python] {
        assert!(analyze(language, "").symbols.is_empty());
    }
}
#[test]
fn rust_symbols_and_ranges() {
    let result = analyze(
        Language::Rust,
        include_str!("../../tests/symbol_fixtures/rust/basic.rs"),
    );
    assert_eq!(result.syntax_error_count, 0);
    assert_symbols(
        &result,
        &[
            ("DEFAULT", Constant),
            ("ENABLED", Static),
            ("Name", TypeAlias),
            ("Engine", Struct),
            ("Mode", Enum),
            ("Bits", Union),
            ("Analyzer", Trait),
            ("Analyzer::Output", TypeAlias),
            ("Analyzer::analyze", Method),
            ("Analyzer::defaulted", Method),
            ("Engine::VALUE", Constant),
            ("Engine::run", Method),
            ("Engine::run::inner", Function),
            ("Engine::Output", TypeAlias),
            ("Engine::analyze", Method),
            ("top_level", Function),
            ("parser", Module),
            ("parser::nested", Module),
            ("parser::nested::parse", Function),
            ("r#type", Function),
            ("café", Function),
            ("café::naïve", Function),
        ],
    );
    let inner = result.symbols.iter().find(|s| s.name == "naïve").unwrap();
    assert!(inner.name_range.start.column > 20); // preceding multibyte text counts bytes
    let text = "fn a() {}";
    let a = analyze(Language::Rust, text);
    assert_eq!(a.symbols[0].range.start.byte, 0);
    assert_eq!(a.symbols[0].range.end.byte, text.len());
    assert_eq!(a.symbols[0].name_range.start.column, 3);
}

#[test]
fn project_analysis_budgets_allow_the_limit_and_reject_excess() {
    let source = "mod a { fn b() {} }";
    let mut syntax = SyntaxAnalyzer::new(
        tree_sitter_rust::LANGUAGE.into(),
        include_str!("queries/rust-symbols.scm"),
    )
    .unwrap();
    let exact = AnalysisLimits {
        max_symbols: 2,
        max_name_bytes: 7,
        max_qualification_nodes: 4,
    };
    let result = syntax
        .analyze_with_limits(
            Language::Rust,
            Path::new("fixture"),
            source,
            rust::qualify,
            exact,
        )
        .unwrap();
    assert_eq!(result.symbols.len(), 2);
    assert_eq!(
        result
            .symbols
            .iter()
            .map(|symbol| symbol.qualified_name.as_str())
            .collect::<Vec<_>>(),
        ["a", "a::b"]
    );

    for limits in [
        AnalysisLimits {
            max_symbols: 1,
            ..exact
        },
        AnalysisLimits {
            max_name_bytes: 6,
            ..exact
        },
        AnalysisLimits {
            max_qualification_nodes: 3,
            ..exact
        },
    ] {
        assert!(matches!(
            syntax.analyze_with_limits(
                Language::Rust,
                Path::new("fixture"),
                source,
                rust::qualify,
                limits,
            ),
            Err(AnalysisError::LimitExceeded)
        ));
    }
}
#[test]
fn python_symbols_and_ranges() {
    let result = analyze(
        Language::Python,
        include_str!("../../tests/symbol_fixtures/python/basic.py"),
    );
    assert_eq!(result.syntax_error_count, 0);
    assert_symbols(
        &result,
        &[
            ("Router", Class),
            ("Router.route", Method),
            ("Router.route.inner", Function),
            ("Router.Inner", Class),
            ("Router.Inner.method", Method),
            ("outer", Function),
            ("outer.inner", Function),
            ("outer.Local", Class),
            ("outer.Local.run", Method),
            ("top_level", Function),
            ("café", Function),
            ("café.naïve", Function),
        ],
    );
    let text = "def a():\n    pass";
    let a = analyze(Language::Python, text);
    assert_eq!(a.symbols[0].range.start.byte, 0);
    assert_eq!(a.symbols[0].range.end.byte, text.len());
}
#[test]
fn malformed_recovery_and_diagnostic_cap() {
    for (language, text, bad) in [
        (
            Language::Rust,
            include_str!("../../tests/symbol_fixtures/rust/malformed.rs"),
            "fn broken() { let x = ; }\n",
        ),
        (
            Language::Python,
            include_str!("../../tests/symbol_fixtures/python/malformed.py"),
            "def broken(:\n    pass\n",
        ),
    ] {
        let result = analyze(language.clone(), text);
        assert!(result.syntax_error_count > 0);
        assert!(result.symbols.iter().any(|s| s.name == "recovered"));
        let result = analyze(language, &bad.repeat(100));
        assert!(result.syntax_error_count > 20);
        assert_eq!(result.diagnostics.len(), 20);
        assert!(
            result
                .diagnostics
                .iter()
                .all(|d| d.range.is_some() && !d.message.contains("broken"))
        );
    }
}
#[test]
fn missing_syntax_is_reported() {
    let result = analyze(Language::Rust, "fn a() { let x = 1 }");
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.kind == DiagnosticKind::MissingSyntax)
    );
}

#[test]
fn impl_targets_omit_generic_expressions_and_decorators_are_in_whole_range() {
    let result = analyze(
        Language::Rust,
        "impl Foo<{ { 123 } }> { fn method() {} } impl module::Foo<u8> { fn other() {} }",
    );
    assert_symbols(
        &result,
        &[("Foo::method", Method), ("module::Foo::other", Method)],
    );
    let nested = analyze(
        Language::Rust,
        "impl Foo { const VALUE: () = { fn nested() {} }; }",
    );
    assert_symbols(
        &nested,
        &[("Foo::VALUE", Constant), ("Foo::nested", Function)],
    );
    let text = "@deco\nasync def a():\n    pass";
    let result = analyze(Language::Python, text);
    assert_eq!(result.symbols[0].range.start.byte, 0);
    assert_eq!(result.symbols[0].range.end.byte, text.len());
    assert_eq!(result.symbols[0].name_range.start.row, 1);
}

#[test]
fn rust_reference_and_pointer_impl_targets_do_not_guess_nominal_names() {
    let result = analyze(
        Language::Rust,
        "trait LocalTrait {}\n\
         struct Foo;\n\
         struct Wrapper<T>(T);\n\
         impl LocalTrait for Foo { fn direct() {} }\n\
         impl LocalTrait for Wrapper<u8> { fn generic() {} }\n\
         impl LocalTrait for &Foo { fn borrowed() {} }\n\
         impl LocalTrait for &mut Foo { fn mutable() {} }\n\
         impl LocalTrait for *const Foo { fn pointer() {} }",
    );
    assert_symbols(
        &result,
        &[
            ("LocalTrait", Trait),
            ("Foo", Struct),
            ("Foo::direct", Method),
            ("Wrapper", Struct),
            ("Wrapper::generic", Method),
            ("borrowed", Method),
            ("mutable", Method),
            ("pointer", Method),
        ],
    );
    assert!(!result.symbols.iter().any(|symbol| {
        ["Foo::borrowed", "Foo::mutable", "Foo::pointer"].contains(&symbol.qualified_name.as_str())
    }));
}
