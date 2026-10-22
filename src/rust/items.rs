//! The `rust` provider reads source files with `syn`, without building
//! anything; [Decision](nexus:markdown.section (adrs/reads-rust-with-syn.md))
//! says why and what that leaves out.

use std::path::{Path, PathBuf};

use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{FnArg, ImplItem, Item, ItemMod, Pat, Signature, TraitItem, Type};

use crate::location::{Position, Span};

/// The directory of the package whose crate is named
/// [`name`](nexus:rust.argument), relative to [`root`](nexus:rust.argument).
fn find_crate(root: &Path, name: &str) -> Option<PathBuf> {
    ignore::WalkBuilder::new(root)
        .build()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name() == "Cargo.toml")
        .find(|entry| crate_name(entry.path()).as_deref() == Some(name))
        .and_then(|entry| {
            let directory = entry.path().parent()?;
            Some(relative(root, directory))
        })
}

/// The name paths start with: the library's when the manifest names it,
/// otherwise the package's with dashes turned into underscores, as Cargo
/// does.
fn crate_name(manifest_path: &Path) -> Option<String> {
    let manifest: toml::Table = std::fs::read_to_string(manifest_path).ok()?.parse().ok()?;
    if let Some(library_name) = manifest.get("lib").and_then(|library| library.get("name")?.as_str()) {
        return Some(library_name.to_owned());
    }
    Some(manifest.get("package")?.get("name")?.as_str()?.replace('-', "_"))
}

/// Where a module's items live while walking down a path.
struct ModuleContext {
    file: PathBuf,
    items: Vec<Item>,
    /// Where the files of its child modules are looked up.
    children_directory: PathBuf,
    span: Span,
}

/// The module at a path written as in Rust, `<crate>::<module>::…`, the
/// crate alone being its root; paths are relative to
/// [`root`](nexus:rust.argument).
pub fn find_module(root: &Path, path: &str) -> Option<Span> {
    let segments: Vec<&str> = path.split("::").collect();
    Some(module_at(root, &segments)?.span)
}

fn module_at(root: &Path, segments: &[&str]) -> Option<ModuleContext> {
    let (crate_name, module_path) = segments.split_first()?;
    let source = root.join(find_crate(root, crate_name)?).join("src");
    let crate_root = ["lib.rs", "main.rs"]
        .into_iter()
        .map(|name| source.join(name))
        .find(|candidate| candidate.is_file())?;
    let mut context = file_module(root, crate_root, source)?;
    for segment in module_path {
        let declaration = context.items.iter().find_map(|item| match item {
            Item::Mod(declaration) if declaration.ident == segment => Some(declaration.clone()),
            _ => None,
        })?;
        context = child_module(root, &context, &declaration)?;
    }
    Some(context)
}

/// The spans of an argument of the functions at a path written as in Rust:
/// a module's function as `<module path>::<function>`, a method as
/// `<module path>::<type or trait>::<method>`, which every `impl` of the
/// type may define.
pub fn find_argument(root: &Path, function_path: &str, argument: &str) -> Vec<Span> {
    let segments: Vec<&str> = function_path.split("::").collect();
    let mut signatures = Vec::new();
    if let Some((name, module_path)) = segments.split_last()
        && let Some(module) = module_at(root, module_path)
    {
        for item in &module.items {
            if let Item::Fn(function) = item
                && function.sig.ident == name
            {
                signatures.push((module.file.clone(), function.sig.clone()));
            }
        }
    }
    if let [module_path @ .., owner, name] = segments.as_slice()
        && let Some(module) = module_at(root, module_path)
    {
        for item in &module.items {
            match item {
                Item::Impl(implementation) if type_name(&implementation.self_ty).is_some_and(|ty| ty == *owner) => {
                    signatures.extend(implementation.items.iter().filter_map(|item| match item {
                        ImplItem::Fn(method) if method.sig.ident == name => Some((module.file.clone(), method.sig.clone())),
                        _ => None,
                    }));
                }
                Item::Trait(declaration) if declaration.ident == owner => {
                    signatures.extend(declaration.items.iter().filter_map(|item| match item {
                        TraitItem::Fn(method) if method.sig.ident == name => Some((module.file.clone(), method.sig.clone())),
                        _ => None,
                    }));
                }
                _ => {}
            }
        }
    }
    signatures
        .iter()
        .filter_map(|(file, signature)| argument_span(&relative(root, file), signature, argument))
        .collect()
}

/// The spans of an argument of the innermost function whose definition,
/// its doc comment and body included, holds [`line`](nexus:rust.argument) of
/// [`file`](nexus:rust.argument), relative to [`root`](nexus:rust.argument).
pub fn find_enclosing_argument(root: &Path, file: &Path, line: usize, argument: &str) -> Vec<Span> {
    let Some(parsed) = std::fs::read_to_string(root.join(file))
        .ok()
        .and_then(|source| syn::parse_file(&source).ok())
    else {
        return Vec::new();
    };
    let mut functions = EnclosingFunctions { line, found: Vec::new() };
    functions.visit_file(&parsed);
    // Visited outside in, so the innermost function comes last.
    functions
        .found
        .last()
        .and_then(|signature| argument_span(file, signature, argument))
        .into_iter()
        .collect()
}

/// The signatures of the functions holding a line, outside in.
struct EnclosingFunctions {
    line: usize,
    found: Vec<Signature>,
}

impl EnclosingFunctions {
    fn holds_line(&self, function: &impl Spanned) -> bool {
        let span = function.span();
        (span.start().line..=span.end().line).contains(&self.line)
    }
}

impl<'ast> Visit<'ast> for EnclosingFunctions {
    fn visit_item_fn(&mut self, function: &'ast syn::ItemFn) {
        if self.holds_line(function) {
            self.found.push(function.sig.clone());
        }
        syn::visit::visit_item_fn(self, function);
    }

    fn visit_impl_item_fn(&mut self, method: &'ast syn::ImplItemFn) {
        if self.holds_line(method) {
            self.found.push(method.sig.clone());
        }
        syn::visit::visit_impl_item_fn(self, method);
    }

    fn visit_trait_item_fn(&mut self, method: &'ast syn::TraitItemFn) {
        if self.holds_line(method) {
            self.found.push(method.sig.clone());
        }
        syn::visit::visit_trait_item_fn(self, method);
    }
}

/// The argument named [`name`](nexus:rust.argument) in a signature, `self` for
/// the receiver; arguments destructured by a pattern have no name.
fn argument_span(file: &Path, signature: &Signature, name: &str) -> Option<Span> {
    signature
        .inputs
        .iter()
        .find(|argument| match argument {
            FnArg::Receiver(_) => name == "self",
            FnArg::Typed(typed) => matches!(&*typed.pat, Pat::Ident(pattern) if pattern.ident == name),
        })
        .map(|argument| item_span(file.to_owned(), argument))
}

/// The name of a type as `impl` blocks write it, without its module path
/// or generic arguments.
fn type_name(ty: &Type) -> Option<String> {
    match ty {
        Type::Path(path) => Some(path.path.segments.last()?.ident.to_string()),
        _ => None,
    }
}

fn relative(root: &Path, file: &Path) -> PathBuf {
    file.strip_prefix(root).unwrap_or(file).to_owned()
}

fn file_module(root: &Path, file: PathBuf, children_directory: PathBuf) -> Option<ModuleContext> {
    let parsed = syn::parse_file(&std::fs::read_to_string(&file).ok()?).ok()?;
    Some(ModuleContext {
        span: Span::whole_file(&relative(root, &file)),
        file,
        items: parsed.items,
        children_directory,
    })
}

fn child_module(root: &Path, parent: &ModuleContext, declaration: &ItemMod) -> Option<ModuleContext> {
    let name = declaration.ident.to_string();
    if let Some((_, items)) = &declaration.content {
        return Some(ModuleContext {
            file: parent.file.clone(),
            items: items.to_vec(),
            children_directory: parent.children_directory.join(&name),
            span: item_span(relative(root, &parent.file), declaration),
        });
    }
    if let Some(explicit_path) = path_attribute(declaration) {
        let file = parent.file.parent()?.join(explicit_path);
        let children_directory = file.with_extension("");
        return file_module(root, file, children_directory);
    }
    let own_file = parent.children_directory.join(format!("{name}.rs"));
    if own_file.is_file() {
        return file_module(root, own_file, parent.children_directory.join(&name));
    }
    let directory = parent.children_directory.join(&name);
    file_module(root, directory.join("mod.rs"), directory)
}

fn path_attribute(declaration: &ItemMod) -> Option<String> {
    declaration.attrs.iter().find_map(|attribute| {
        let syn::Meta::NameValue(name_value) = &attribute.meta else {
            return None;
        };
        if !name_value.path.is_ident("path") {
            return None;
        }
        let syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(path), ..
        }) = &name_value.value
        else {
            return None;
        };
        Some(path.value())
    })
}

fn item_span(path: PathBuf, item: &impl Spanned) -> Span {
    let span = item.span();
    // proc-macro2 counts columns from 0, positions here from 1.
    Span {
        path,
        start: Position {
            line: span.start().line,
            column: span.start().column + 1,
        },
        end: Position {
            line: span.end().line,
            column: span.end().column,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A package on disk, named with a dash: `lib.rs` declaring a file
    /// module, a directory module, a module behind `#[path]` and an inline
    /// module, and functions in a module, a type and a trait.
    fn write_package(root: &Path) {
        let files = [
            ("Cargo.toml", "[package]\nname = \"an-example\"\n"),
            (
                "src/lib.rs",
                "mod file;\nmod directory;\n#[path = \"elsewhere/moved.rs\"]\nmod moved;\nmod inline {\n    mod nested;\n}\n",
            ),
            ("src/file.rs", "mod child;\n"),
            ("src/file/child.rs", ""),
            (
                "src/directory/mod.rs",
                concat!(
                    "/// Runs with `count`.\n",
                    "pub fn run(count: usize, (a, b): (u8, u8)) {\n",
                    "    // Inner, with `value`.\n",
                    "    fn inner(value: u8) {}\n",
                    "}\n",
                    "pub struct Tool;\n",
                    "impl Tool {\n",
                    "    /// Uses `self`.\n",
                    "    pub fn apply(&self, input: &str) {}\n",
                    "}\n",
                    "pub trait Shape {\n",
                    "    fn area(&self, scale: f64) -> f64;\n",
                    "}\n",
                ),
            ),
            ("src/elsewhere/moved.rs", ""),
            ("src/inline/nested.rs", ""),
        ];
        for (path, text) in files {
            let path = root.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
    }

    fn temporary_package(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("nexus-rust-{name}-{}", std::process::id()));
        write_package(&root);
        root
    }

    fn module_path(root: &Path, path: &str) -> Option<String> {
        Some(format!("{}", find_module(root, path)?))
    }

    fn spans(spans: Vec<Span>) -> Vec<String> {
        spans.iter().map(|span| format!("{span}")).collect()
    }

    #[test]
    fn finds_modules_however_they_are_declared() {
        let root = temporary_package("modules");
        assert_eq!(module_path(&root, "an_example").as_deref(), Some("src/lib.rs:1:1"));
        assert_eq!(module_path(&root, "an_example::file").as_deref(), Some("src/file.rs:1:1"));
        assert_eq!(
            module_path(&root, "an_example::file::child").as_deref(),
            Some("src/file/child.rs:1:1")
        );
        assert_eq!(
            module_path(&root, "an_example::directory").as_deref(),
            Some("src/directory/mod.rs:1:1")
        );
        assert_eq!(
            module_path(&root, "an_example::moved").as_deref(),
            Some("src/elsewhere/moved.rs:1:1")
        );
        assert_eq!(module_path(&root, "an_example::inline").as_deref(), Some("src/lib.rs:5:1-7:1"));
        assert_eq!(
            module_path(&root, "an_example::inline::nested").as_deref(),
            Some("src/inline/nested.rs:1:1")
        );
        assert_eq!(module_path(&root, "an_example::missing"), None);
        assert_eq!(module_path(&root, "an-example::file"), None);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn finds_the_arguments_of_functions_and_methods_by_path() {
        let root = temporary_package("arguments");
        let argument = |path, name| spans(find_argument(&root, path, name));
        assert_eq!(argument("an_example::directory::run", "count"), vec![
            "src/directory/mod.rs:2:12-2:23"
        ]);
        assert_eq!(argument("an_example::directory::Tool::apply", "self"), vec![
            "src/directory/mod.rs:9:18-9:22"
        ]);
        assert_eq!(argument("an_example::directory::Shape::area", "scale"), vec![
            "src/directory/mod.rs:12:20-12:29"
        ]);
        assert!(argument("an_example::directory::run", "missing").is_empty());
        assert!(argument("an_example::directory::run", "a").is_empty());
        assert!(argument("an_example::run", "count").is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn finds_the_arguments_of_the_innermost_function_holding_a_line() {
        let root = temporary_package("enclosing");
        let file = Path::new("src/directory/mod.rs");
        let argument = |line, name| spans(find_enclosing_argument(&root, file, line, name));
        assert_eq!(argument(1, "count"), vec!["src/directory/mod.rs:2:12-2:23"]);
        // A plain comment is part of the function it is written in, not of
        // the one below it.
        assert_eq!(argument(3, "count"), vec!["src/directory/mod.rs:2:12-2:23"]);
        assert!(argument(3, "value").is_empty());
        assert_eq!(argument(4, "value"), vec!["src/directory/mod.rs:4:14-4:22"]);
        assert_eq!(argument(8, "input"), vec!["src/directory/mod.rs:9:25-9:35"]);
        assert!(argument(6, "count").is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
}
