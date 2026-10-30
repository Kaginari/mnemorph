//! Skeletons from a tree-sitter parse: Rust, TypeScript, TSX, JavaScript (read with the TSX
//! grammar), Python and Go.

use tree_sitter::{Language, Node, Parser};

use super::{Lang, Made, Symbol, squash};

/// What one node is to the skeleton.
enum Item {
    /// A symbol: its kind and name; its range is the node's, widened over the comments and
    /// attributes just above it.
    Symbol(&'static str, String),
    /// Several symbols declared by one statement (`const a = 1, b = 2`, a Go `const (…)` block):
    /// each its kind, name and node.
    Many(Vec<(&'static str, String, usize, usize)>),
    /// An import, by its text.
    Import(String),
    /// Nothing; its children are looked at.
    Through,
}

/// The imports and symbols of `text`, or `None` when the parser could not run.
pub(super) fn parse(lang: Lang, text: &str) -> Option<Made> {
    let language: Language = match lang {
        Lang::Rust => tree_sitter_rust::LANGUAGE.into(),
        Lang::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        Lang::Tsx | Lang::JavaScript => tree_sitter_typescript::LANGUAGE_TSX.into(),
        Lang::Python => tree_sitter_python::LANGUAGE.into(),
        Lang::Go => tree_sitter_go::LANGUAGE.into(),
        Lang::Markdown | Lang::Toml | Lang::Json => return None,
    };
    let mut parser = Parser::new();
    parser.set_language(&language).ok()?;
    let tree = parser.parse(text, None)?;
    let mut w = Walk {
        lang,
        src: text.as_bytes(),
        imports: Vec::new(),
        symbols: Vec::new(),
    };
    w.children(tree.root_node(), 0);
    Some((w.imports, w.symbols))
}

struct Walk<'a> {
    lang: Lang,
    src: &'a [u8],
    imports: Vec<(usize, String)>,
    symbols: Vec<Symbol>,
}

/// A node's first line, 1-based.
fn first(n: Node<'_>) -> usize {
    n.start_position().row + 1
}

/// A node's last line, 1-based: a node that ends at a line's start ends on the line before.
fn last(n: Node<'_>) -> usize {
    let end = n.end_position();
    if end.column == 0 && end.row > n.start_position().row {
        end.row
    } else {
        end.row + 1
    }
}

impl Walk<'_> {
    fn text(&self, n: Node<'_>) -> &str {
        n.utf8_text(self.src).unwrap_or("")
    }

    fn field(&self, n: Node<'_>, name: &str) -> Option<String> {
        n.child_by_field_name(name).map(|c| squash(self.text(c)))
    }

    fn children(&mut self, node: Node<'_>, depth: usize) {
        let mut cursor = node.walk();
        let kids: Vec<Node<'_>> = node.named_children(&mut cursor).collect();
        for n in kids {
            self.visit(n, depth);
        }
    }

    fn visit(&mut self, n: Node<'_>, depth: usize) {
        match self.item(n) {
            Item::Symbol(kind, name) => {
                let start = Self::lead(n);
                self.symbols.push(Symbol {
                    kind,
                    name,
                    start,
                    end: last(n),
                    depth,
                });
                // A decorated definition's body is its definition's.
                let inner = if n.kind() == "decorated_definition" {
                    n.child_by_field_name("definition").unwrap_or(n)
                } else {
                    n
                };
                self.children(inner, depth + 1);
            }
            Item::Many(list) => {
                for (kind, name, start, end) in list {
                    self.symbols.push(Symbol {
                        kind,
                        name,
                        start,
                        end,
                        depth,
                    });
                }
            }
            Item::Import(text) => self.imports.push((first(n), text)),
            Item::Through => self.children(n, depth),
        }
    }

    /// Where a symbol starts once the comments, attributes and decorators right above it are
    /// counted in: a ranged read of it then shows its documentation.
    fn lead(n: Node<'_>) -> usize {
        let mut start = first(n);
        let mut at = n;
        while let Some(prev) = at.prev_sibling() {
            let lead = matches!(
                prev.kind(),
                "attribute_item" | "line_comment" | "block_comment" | "comment" | "decorator"
            );
            if !lead || last(prev) + 1 < start {
                break;
            }
            // A comment that ends a line of code is that code's, not this symbol's.
            if prev
                .prev_sibling()
                .is_some_and(|before| last(before) == first(prev) && before.kind() != prev.kind())
            {
                break;
            }
            start = first(prev);
            at = prev;
        }
        start
    }

    fn item(&self, n: Node<'_>) -> Item {
        match self.lang {
            Lang::Rust => self.rust(n),
            Lang::TypeScript | Lang::Tsx | Lang::JavaScript => self.script(n),
            Lang::Python => self.python(n),
            Lang::Go => self.go(n),
            Lang::Markdown | Lang::Toml | Lang::Json => Item::Through,
        }
    }

    fn named(&self, n: Node<'_>, kind: &'static str) -> Item {
        Item::Symbol(
            kind,
            self.field(n, "name")
                .unwrap_or_else(|| "(anonymous)".into()),
        )
    }

    fn import(&self, n: Node<'_>) -> Item {
        Item::Import(squash(self.text(n)).trim_end_matches(';').to_owned())
    }

    fn rust(&self, n: Node<'_>) -> Item {
        match n.kind() {
            "function_item" | "function_signature_item" => self.named(n, "fn"),
            "struct_item" => self.named(n, "struct"),
            "enum_item" => self.named(n, "enum"),
            "union_item" => self.named(n, "union"),
            "trait_item" => self.named(n, "trait"),
            "mod_item" => self.named(n, "mod"),
            "const_item" => self.named(n, "const"),
            "static_item" => self.named(n, "static"),
            "type_item" => self.named(n, "type"),
            "macro_definition" => self.named(n, "macro"),
            "impl_item" => {
                let ty = self.field(n, "type").unwrap_or_default();
                let name = match self.field(n, "trait") {
                    Some(t) => format!("{t} for {ty}"),
                    None => ty,
                };
                Item::Symbol("impl", name)
            }
            "use_declaration" | "extern_crate_declaration" => self.import(n),
            _ => Item::Through,
        }
    }

    /// Whether `n` sits at a module's top level, an `export` in between allowed.
    fn top(n: Node<'_>) -> bool {
        let mut p = n.parent();
        if p.is_some_and(|p| p.kind() == "export_statement") {
            p = p.and_then(|p| p.parent());
        }
        p.is_some_and(|p| p.kind() == "program")
    }

    fn script(&self, n: Node<'_>) -> Item {
        match n.kind() {
            "function_declaration" | "generator_function_declaration" | "function_signature" => {
                self.named(n, "fn")
            }
            "class_declaration" | "abstract_class_declaration" => self.named(n, "class"),
            "method_definition" | "abstract_method_signature" => self.named(n, "method"),
            "interface_declaration" => self.named(n, "interface"),
            "type_alias_declaration" => self.named(n, "type"),
            "enum_declaration" => self.named(n, "enum"),
            "internal_module" | "module" => self.named(n, "namespace"),
            "import_statement" => self.import(n),
            "lexical_declaration" | "variable_declaration" if Self::top(n) => {
                let word = self
                    .text(n)
                    .split_whitespace()
                    .next()
                    .map_or("var", |w| if w == "const" { "const" } else { "var" });
                let mut cursor = n.walk();
                let one = n.named_child_count() == 1;
                let list = n
                    .named_children(&mut cursor)
                    .filter(|d| d.kind() == "variable_declarator")
                    .filter_map(|d| {
                        let name = d.child_by_field_name("name")?;
                        if name.kind() != "identifier" {
                            return None;
                        }
                        let func = d.child_by_field_name("value").is_some_and(|v| {
                            matches!(
                                v.kind(),
                                "arrow_function" | "function_expression" | "function"
                            )
                        });
                        let kind = if func { "fn" } else { word };
                        // One declarator spans its whole statement; several, each its own.
                        let (start, end) = if one {
                            (Self::lead(n), last(n))
                        } else {
                            (first(d), last(d))
                        };
                        Some((kind, squash(self.text(name)), start, end))
                    })
                    .collect();
                Item::Many(list)
            }
            _ => Item::Through,
        }
    }

    fn python(&self, n: Node<'_>) -> Item {
        match n.kind() {
            "decorated_definition" => {
                // The definition is named for what it decorates; its range takes the decorators.
                match n.child_by_field_name("definition") {
                    Some(d) => self.python_def(d, n),
                    None => Item::Through,
                }
            }
            "function_definition" | "class_definition" => self.python_def(n, n),
            "import_statement" | "import_from_statement" | "future_import_statement" => {
                self.import(n)
            }
            "expression_statement" if n.parent().is_some_and(|p| p.kind() == "module") => {
                // A module-level constant: `NAME = …` in capitals.
                let Some(a) = n.named_child(0).filter(|a| a.kind() == "assignment") else {
                    return Item::Many(Vec::new());
                };
                let Some(left) = a.child_by_field_name("left") else {
                    return Item::Many(Vec::new());
                };
                let name = self.text(left);
                let constant = left.kind() == "identifier"
                    && name.chars().any(|c| c.is_ascii_uppercase())
                    && !name.chars().any(|c| c.is_ascii_lowercase());
                if constant {
                    Item::Many(vec![("const", name.to_owned(), Self::lead(n), last(n))])
                } else {
                    Item::Many(Vec::new())
                }
            }
            _ => Item::Through,
        }
    }

    /// A Python definition `d`, its range `outer`'s (the decorated definition, when decorated).
    fn python_def(&self, d: Node<'_>, outer: Node<'_>) -> Item {
        let kind = if d.kind() == "class_definition" {
            "class"
        } else if Self::in_class(outer) {
            "method"
        } else {
            "fn"
        };
        if outer.id() == d.id() {
            return self.named(d, kind);
        }
        // Decorated: the decorated node is the symbol; its name is the definition's.
        Item::Symbol(
            kind,
            self.field(d, "name")
                .unwrap_or_else(|| "(anonymous)".into()),
        )
    }

    fn in_class(n: Node<'_>) -> bool {
        n.parent()
            .filter(|b| b.kind() == "block")
            .and_then(|b| b.parent())
            .is_some_and(|c| c.kind() == "class_definition")
    }

    fn go(&self, n: Node<'_>) -> Item {
        let top = n.parent().is_some_and(|p| p.kind() == "source_file");
        match n.kind() {
            "function_declaration" => self.named(n, "fn"),
            "method_declaration" => {
                let name = self.field(n, "name").unwrap_or_default();
                let recv = n
                    .child_by_field_name("receiver")
                    .map(|r| receiver(self.text(r)))
                    .unwrap_or_default();
                let name = if recv.is_empty() {
                    name
                } else {
                    format!("{recv}.{name}")
                };
                Item::Symbol("method", name)
            }
            "type_declaration" => {
                let mut cursor = n.walk();
                let specs: Vec<Node<'_>> = n
                    .named_children(&mut cursor)
                    .filter(|s| matches!(s.kind(), "type_spec" | "type_alias"))
                    .collect();
                let one = specs.len() == 1;
                let list = specs
                    .iter()
                    .map(|s| {
                        let kind = match s.child_by_field_name("type").map(|t| t.kind()) {
                            Some("struct_type") => "struct",
                            Some("interface_type") => "interface",
                            _ => "type",
                        };
                        let name = self.field(*s, "name").unwrap_or_default();
                        let (start, end) = if one {
                            (Self::lead(n), last(n))
                        } else {
                            (first(*s), last(*s))
                        };
                        (kind, name, start, end)
                    })
                    .collect();
                Item::Many(list)
            }
            "const_declaration" | "var_declaration" if top => {
                let kind = if n.kind() == "const_declaration" {
                    "const"
                } else {
                    "var"
                };
                let mut specs = Vec::new();
                collect_specs(n, &mut specs);
                let one = specs.len() == 1;
                let list = specs
                    .iter()
                    .filter_map(|s| {
                        let name = self.field(*s, "name")?;
                        let (start, end) = if one {
                            (Self::lead(n), last(n))
                        } else {
                            (first(*s), last(*s))
                        };
                        Some((kind, name, start, end))
                    })
                    .collect();
                Item::Many(list)
            }
            "import_spec" => self.import(n),
            _ => Item::Through,
        }
    }
}

/// A Go declaration's specs, through a parenthesised list.
fn collect_specs<'t>(n: Node<'t>, out: &mut Vec<Node<'t>>) {
    let mut cursor = n.walk();
    for c in n.named_children(&mut cursor) {
        match c.kind() {
            "const_spec" | "var_spec" => out.push(c),
            "var_spec_list" => collect_specs(c, out),
            _ => {}
        }
    }
}

/// A Go receiver's type: `(s *Stack[T])` reads as `Stack`.
fn receiver(text: &str) -> String {
    let inner = text.trim_matches(|c| c == '(' || c == ')');
    let ty = inner.split_whitespace().last().unwrap_or("");
    let ty = ty.trim_start_matches('*');
    ty.split('[').next().unwrap_or(ty).to_owned()
}
