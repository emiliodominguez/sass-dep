//! Parser integration tests.

use sass_dep::parser::{Directive, Namespace, Parser, Visibility};

#[test]
fn parse_use_directive_simple() {
    let input = r#"@use "variables";"#;
    let directives = Parser::parse(input).unwrap();

    assert_eq!(directives.len(), 1);
    match &directives[0] {
        Directive::Use(u) => {
            assert_eq!(u.path, "variables");
            assert!(u.namespace.is_none());
            assert!(!u.configured);
        }
        _ => panic!("Expected Use directive"),
    }
}

#[test]
fn parse_use_directive_with_namespace() {
    let input = r#"@use "variables" as vars;"#;
    let directives = Parser::parse(input).unwrap();

    assert_eq!(directives.len(), 1);
    match &directives[0] {
        Directive::Use(u) => {
            assert_eq!(u.path, "variables");
            assert_eq!(u.namespace, Some(Namespace::Named("vars".to_string())));
        }
        _ => panic!("Expected Use directive"),
    }
}

#[test]
fn parse_use_directive_with_star_namespace() {
    let input = r#"@use "variables" as *;"#;
    let directives = Parser::parse(input).unwrap();

    match &directives[0] {
        Directive::Use(u) => {
            assert_eq!(u.namespace, Some(Namespace::Star));
        }
        _ => panic!("Expected Use directive"),
    }
}

#[test]
fn parse_use_directive_with_configuration() {
    let input = r#"@use "variables" with ($primary: blue, $secondary: green);"#;
    let directives = Parser::parse(input).unwrap();

    match &directives[0] {
        Directive::Use(u) => {
            assert!(u.configured);
        }
        _ => panic!("Expected Use directive"),
    }
}

#[test]
fn parse_forward_directive_simple() {
    let input = r#"@forward "mixins";"#;
    let directives = Parser::parse(input).unwrap();

    assert_eq!(directives.len(), 1);
    match &directives[0] {
        Directive::Forward(f) => {
            assert_eq!(f.path, "mixins");
            assert!(f.prefix.is_none());
            assert_eq!(f.visibility, Visibility::All);
        }
        _ => panic!("Expected Forward directive"),
    }
}

#[test]
fn parse_forward_directive_with_prefix() {
    let input = r#"@forward "functions" as fn-*;"#;
    let directives = Parser::parse(input).unwrap();

    match &directives[0] {
        Directive::Forward(f) => {
            assert_eq!(f.prefix, Some("fn-".to_string()));
        }
        _ => panic!("Expected Forward directive"),
    }
}

#[test]
fn parse_forward_directive_with_show() {
    let input = r#"@forward "utils" show $var, mixin-name, function-name;"#;
    let directives = Parser::parse(input).unwrap();

    match &directives[0] {
        Directive::Forward(f) => {
            assert_eq!(
                f.visibility,
                Visibility::Show(vec![
                    "$var".to_string(),
                    "mixin-name".to_string(),
                    "function-name".to_string()
                ])
            );
        }
        _ => panic!("Expected Forward directive"),
    }
}

#[test]
fn parse_forward_directive_with_hide() {
    let input = r#"@forward "utils" hide $internal, _private-mixin;"#;
    let directives = Parser::parse(input).unwrap();

    match &directives[0] {
        Directive::Forward(f) => {
            assert_eq!(
                f.visibility,
                Visibility::Hide(vec!["$internal".to_string(), "_private-mixin".to_string()])
            );
        }
        _ => panic!("Expected Forward directive"),
    }
}

#[test]
fn parse_import_directive_single() {
    let input = r#"@import "legacy";"#;
    let directives = Parser::parse(input).unwrap();

    assert_eq!(directives.len(), 1);
    match &directives[0] {
        Directive::Import(i) => {
            assert_eq!(i.paths, vec!["legacy".to_string()]);
        }
        _ => panic!("Expected Import directive"),
    }
}

#[test]
fn parse_import_directive_multiple() {
    let input = r#"@import "a", "b", "c";"#;
    let directives = Parser::parse(input).unwrap();

    match &directives[0] {
        Directive::Import(i) => {
            assert_eq!(
                i.paths,
                vec!["a".to_string(), "b".to_string(), "c".to_string()]
            );
        }
        _ => panic!("Expected Import directive"),
    }
}

#[test]
fn parse_mixed_directives() {
    let input = r#"
@use "variables" as vars;
@use "mixins";
@forward "functions" as fn-*;
@import "legacy";

.container {
    color: vars.$primary;
}
"#;

    let directives = Parser::parse(input).unwrap();
    assert_eq!(directives.len(), 4);

    assert!(matches!(&directives[0], Directive::Use(_)));
    assert!(matches!(&directives[1], Directive::Use(_)));
    assert!(matches!(&directives[2], Directive::Forward(_)));
    assert!(matches!(&directives[3], Directive::Import(_)));
}

#[test]
fn parse_ignores_comments() {
    let input = r#"
// Single line comment
@use "a";

/* Multi-line
   comment with @use "fake" inside */
@use "b";

// @use "commented-out";
@use "c";
"#;

    let directives = Parser::parse(input).unwrap();
    assert_eq!(directives.len(), 3);

    let paths: Vec<&str> = directives
        .iter()
        .flat_map(|d| d.paths())
        .collect();
    assert_eq!(paths, vec!["a", "b", "c"]);
}

#[test]
fn parse_ignores_strings_in_selectors() {
    let input = r#"
@use "real";

.selector[data-attr="@use fake"] {
    content: "@forward also-fake";
}

@forward "also-real";
"#;

    let directives = Parser::parse(input).unwrap();
    assert_eq!(directives.len(), 2);

    let paths: Vec<&str> = directives
        .iter()
        .flat_map(|d| d.paths())
        .collect();
    assert_eq!(paths, vec!["real", "also-real"]);
}

#[test]
fn parse_ignores_other_at_rules() {
    let input = r#"
@use "variables";

@mixin foo($a) {
    color: $a;
}

@media screen and (min-width: 768px) {
    .container { width: 100%; }
}

@keyframes slide {
    from { opacity: 0; }
    to { opacity: 1; }
}

@forward "mixins";

@function double($n) {
    @return $n * 2;
}
"#;

    let directives = Parser::parse(input).unwrap();
    assert_eq!(directives.len(), 2);
}

#[test]
fn parse_tracks_location() {
    let input = "@use \"a\";\n@use \"b\";\n\n@use \"c\";";
    let directives = Parser::parse(input).unwrap();

    assert_eq!(directives[0].location().line, 1);
    assert_eq!(directives[0].location().column, 1);

    assert_eq!(directives[1].location().line, 2);
    assert_eq!(directives[1].location().column, 1);

    assert_eq!(directives[2].location().line, 4);
    assert_eq!(directives[2].location().column, 1);
}

#[test]
fn parse_single_quotes() {
    let input = r#"@use 'single-quoted';"#;
    let directives = Parser::parse(input).unwrap();

    match &directives[0] {
        Directive::Use(u) => {
            assert_eq!(u.path, "single-quoted");
        }
        _ => panic!("Expected Use directive"),
    }
}

#[test]
fn parse_case_insensitive() {
    let input = r#"
@USE "upper";
@Use "mixed";
@use "lower";
"#;

    let directives = Parser::parse(input).unwrap();
    assert_eq!(directives.len(), 3);
}

#[test]
fn parse_file_simple_fixture() {
    let path = std::path::Path::new("tests/fixtures/simple/main.scss");
    let directives = Parser::parse_file(path).unwrap();

    assert_eq!(directives.len(), 2);

    // First should be @use "variables" as vars
    match &directives[0] {
        Directive::Use(u) => {
            assert_eq!(u.path, "variables");
            assert_eq!(u.namespace, Some(Namespace::Named("vars".to_string())));
        }
        _ => panic!("Expected Use directive"),
    }

    // Second should be @use "mixins"
    match &directives[1] {
        Directive::Use(u) => {
            assert_eq!(u.path, "mixins");
            assert!(u.namespace.is_none());
        }
        _ => panic!("Expected Use directive"),
    }
}
