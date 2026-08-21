use tree_sitter::{Language, Node, Parser};

#[derive(Debug, Clone, Copy)]
pub enum SourceLanguage {
    JavaScript,
    TypeScript,
    Tsx,
}

impl SourceLanguage {
    fn language(self) -> Language {
        match self {
            Self::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Self::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
        }
    }
}

pub fn language_for_path(path: &str) -> Option<SourceLanguage> {
    match path.rsplit('.').next()? {
        "js" | "mjs" | "cjs" => Some(SourceLanguage::JavaScript),
        "ts" => Some(SourceLanguage::TypeScript),
        "tsx" | "jsx" => Some(SourceLanguage::Tsx),
        _ => None,
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn validate_syntax(code: &str) -> Result<(), Vec<String>> {
    validate_syntax_as(code, SourceLanguage::TypeScript)
}

pub fn validate_syntax_as(code: &str, language: SourceLanguage) -> Result<(), Vec<String>> {
    let mut parser = Parser::new();
    if let Err(error) = parser.set_language(&language.language()) {
        return Err(vec![format!("parser initialization failed: {error}")]);
    }
    let Some(tree) = parser.parse(code, None) else {
        return Err(vec!["parser returned no syntax tree".to_string()]);
    };
    let mut errors = Vec::new();
    collect_errors(tree.root_node(), &mut errors);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn collect_errors(node: Node<'_>, errors: &mut Vec<String>) {
    if node.is_error() || node.is_missing() {
        let start = node.start_position();
        let end = node.end_position();
        let kind = if node.is_missing() {
            "missing syntax"
        } else {
            "syntax error"
        };
        errors.push(format!(
            "{kind} at line {}, column {} through line {}, column {}",
            start.row + 1,
            start.column + 1,
            end.row + 1,
            end.column + 1
        ));
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_errors(child, errors);
    }
}

#[cfg(test)]
mod tests {
    use super::{validate_syntax, validate_syntax_as, SourceLanguage};

    #[test]
    fn accepts_valid_typescript() {
        assert!(validate_syntax("const answer: number = 42;").is_ok());
    }

    #[test]
    fn reports_invalid_typescript() {
        assert!(validate_syntax("const answer: number = ;").is_err());
    }

    #[test]
    fn parses_javascript() {
        assert!(
            validate_syntax_as("export const ready = true;", SourceLanguage::JavaScript).is_ok()
        );
    }
}
