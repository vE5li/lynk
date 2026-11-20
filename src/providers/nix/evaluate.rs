//! Evaluates the parts of a Nix file that name commands, reading nothing
//! but the file: names are followed to the bindings and arguments of the
//! file, functions written in it are applied, and the few list functions
//! of `builtins` and nixpkgs' `lib` that build lists of commands are
//! applied as Nix does. Anything else, such as what comes from other files
//! or from the function arguments of a flake, is unknown.

use std::rc::Rc;

use rnix::TextRange;
use rnix::ast::{self, BinOpKind, HasEntry, InterpolPart};
use rowan::ast::AstNode;

/// How deep evaluation goes before giving up, which ends bindings referring
/// to themselves.
const MAX_DEPTH: usize = 64;

#[derive(Clone)]
pub enum Value {
    Str(String),
    List(Vec<Value>),
    AttrSet(ast::AttrSet, Env),
    Lambda(ast::Lambda, Env),
    /// A list function, with the arguments it has been applied to so far.
    Builtin(Builtin, Vec<Value>),
    /// A function the file does not define applied to strings, or to
    /// attribute sets with a `name`, each a name of the command it may
    /// build.
    Built {
        names: Vec<String>,
        range: TextRange,
    },
    Unknown,
}

#[derive(Clone, Copy)]
pub enum Builtin {
    Map,
    ConcatMap,
    Flatten,
    ConcatLists,
    Pipe,
}

impl Builtin {
    /// The list function selected as the attribute
    /// [`name`](lynk:rust.argument), from `builtins` or `lib`.
    fn selected(name: &str) -> Option<Self> {
        match name {
            "map" => Some(Self::Map),
            "concatMap" => Some(Self::ConcatMap),
            "flatten" => Some(Self::Flatten),
            "concatLists" => Some(Self::ConcatLists),
            "pipe" => Some(Self::Pipe),
            _ => None,
        }
    }

    /// The list function in scope everywhere as
    /// [`name`](lynk:rust.argument).
    fn global(name: &str) -> Option<Self> {
        match name {
            "map" => Some(Self::Map),
            _ => None,
        }
    }

    fn arity(self) -> usize {
        match self {
            Self::Map | Self::ConcatMap | Self::Pipe => 2,
            Self::Flatten | Self::ConcatLists => 1,
        }
    }
}

/// The values the arguments of the functions being applied are bound to,
/// innermost first.
pub type Env = Option<Rc<Frame>>;

pub struct Frame {
    /// The function whose argument this is.
    lambda: TextRange,
    value: Value,
    parent: Env,
}

pub fn evaluate(expression: &ast::Expr, env: &Env, depth: usize) -> Value {
    if depth > MAX_DEPTH {
        return Value::Unknown;
    }
    let depth = depth + 1;
    match expression {
        ast::Expr::Str(text) => string(text, env, depth),
        ast::Expr::List(list) => Value::List(list.items().map(|item| evaluate(&item, env, depth)).collect()),
        ast::Expr::AttrSet(set) => Value::AttrSet(set.clone(), env.clone()),
        ast::Expr::Lambda(lambda) => Value::Lambda(lambda.clone(), env.clone()),
        ast::Expr::Paren(paren) => evaluate_some(paren.expr(), env, depth),
        ast::Expr::LetIn(let_in) => evaluate_some(let_in.body(), env, depth),
        ast::Expr::Ident(identifier) => lookup(identifier, env, depth),
        ast::Expr::Select(select) => selected_builtin(select),
        ast::Expr::BinOp(operation) if operation.operator() == Some(BinOpKind::Concat) => {
            let mut items = as_items(evaluate_some(operation.lhs(), env, depth));
            items.extend(as_items(evaluate_some(operation.rhs(), env, depth)));
            Value::List(items)
        }
        ast::Expr::Apply(apply) => application(apply, env, depth),
        _ => Value::Unknown,
    }
}

fn evaluate_some(expression: Option<ast::Expr>, env: &Env, depth: usize) -> Value {
    expression.map_or(Value::Unknown, |expression| evaluate(&expression, env, depth))
}

/// Every command a value may build, as the names it may have and where it
/// is built.
pub fn built(value: &Value) -> Vec<(Vec<String>, TextRange)> {
    match value {
        Value::Built { names, range } => vec![(names.clone(), *range)],
        Value::List(items) => items.iter().flat_map(built).collect(),
        _ => Vec::new(),
    }
}

/// The range a command is built in: the binding holding it, from its name
/// to the end of its value, or the application alone when it is not bound.
fn built_range(apply: &ast::Apply) -> TextRange {
    let mut node = apply.syntax().clone();
    while let Some(parent) = node.parent().filter(|parent| ast::Paren::can_cast(parent.kind())) {
        node = parent;
    }
    if let Some(binding) = node.parent().and_then(ast::AttrpathValue::cast)
        && let Some(attrpath) = binding.attrpath()
        && binding.value().is_some_and(|value| value.syntax() == &node)
    {
        return TextRange::new(attrpath.syntax().text_range().start(), node.text_range().end());
    }
    apply.syntax().text_range()
}

/// A list's items, any other value being a single item, so lists joined
/// with `++` keep what is known of them.
fn as_items(value: Value) -> Vec<Value> {
    match value {
        Value::List(items) => items,
        other => vec![other],
    }
}

fn string(text: &ast::Str, env: &Env, depth: usize) -> Value {
    let mut content = String::new();
    for part in text.normalized_parts() {
        match part {
            InterpolPart::Literal(literal) => content.push_str(&literal),
            InterpolPart::Interpolation(interpolation) => {
                let Value::Str(value) = evaluate_some(interpolation.expr(), env, depth) else {
                    return Value::Unknown;
                };
                content.push_str(&value);
            }
        }
    }
    Value::Str(content)
}

/// The value of a name where [`identifier`](lynk:rust.argument) is written:
/// the innermost `let`, recursive attribute set or function argument
/// binding it.
fn lookup(identifier: &ast::Ident, env: &Env, depth: usize) -> Value {
    let Some(name) = identifier.ident_token().map(|token| token.text().to_owned()) else {
        return Value::Unknown;
    };
    for ancestor in identifier.syntax().ancestors().skip(1) {
        if let Some(let_in) = ast::LetIn::cast(ancestor.clone())
            && let Some(value) = bound(&let_in, &name)
        {
            return evaluate(&value, env, depth);
        }
        if let Some(set) = ast::AttrSet::cast(ancestor.clone())
            && set.rec_token().is_some()
            && let Some(value) = bound(&set, &name)
        {
            return evaluate(&value, env, depth);
        }
        let Some(lambda) = ast::Lambda::cast(ancestor) else {
            continue;
        };
        match lambda.param() {
            Some(ast::Param::IdentParam(parameter)) if ident_text(parameter.ident()).as_deref() == Some(&name) => {
                return argument(env, lambda.syntax().text_range());
            }
            Some(ast::Param::Pattern(pattern)) if binds(&pattern, &name) => return Value::Unknown,
            _ => {}
        }
    }
    Builtin::global(&name).map_or(Value::Unknown, |builtin| Value::Builtin(builtin, Vec::new()))
}

fn ident_text(identifier: Option<ast::Ident>) -> Option<String> {
    Some(identifier?.ident_token()?.text().to_owned())
}

/// The value bound to [`name`](lynk:rust.argument) by a binding of a single
/// name.
fn bound(entries: &impl HasEntry, name: &str) -> Option<ast::Expr> {
    entries.attrpath_values().find_map(|binding| {
        let mut attributes = binding.attrpath()?.attrs();
        let (Some(ast::Attr::Ident(identifier)), None) = (attributes.next(), attributes.next()) else {
            return None;
        };
        (identifier.ident_token()?.text() == name).then(|| binding.value())?
    })
}

fn binds(pattern: &ast::Pattern, name: &str) -> bool {
    let bound_whole = pattern.pat_bind().and_then(|bind| ident_text(bind.ident()));
    bound_whole.as_deref() == Some(name)
        || pattern
            .pat_entries()
            .any(|entry| ident_text(entry.ident()).as_deref() == Some(name))
}

/// The argument the function at [`lambda`](lynk:rust.argument) is applied
/// to.
fn argument(env: &Env, lambda: TextRange) -> Value {
    let mut frame = env.as_deref();
    while let Some(current) = frame {
        if current.lambda == lambda {
            return current.value.clone();
        }
        frame = current.parent.as_deref();
    }
    Value::Unknown
}

fn selected_builtin(select: &ast::Select) -> Value {
    let name = select.attrpath().and_then(|attrpath| match attrpath.attrs().last()? {
        ast::Attr::Ident(identifier) => Some(identifier.ident_token()?.text().to_owned()),
        _ => None,
    });
    name.and_then(|name| Builtin::selected(&name))
        .map_or(Value::Unknown, |builtin| Value::Builtin(builtin, Vec::new()))
}

/// Applies a function to its arguments. A function the file does not
/// define builds a command from the arguments naming one.
fn application(apply: &ast::Apply, env: &Env, depth: usize) -> Value {
    let mut arguments = Vec::new();
    let mut function = ast::Expr::Apply(apply.clone());
    while let ast::Expr::Apply(inner) = &function {
        let (Some(argument), Some(lambda)) = (inner.argument(), inner.lambda()) else {
            return Value::Unknown;
        };
        arguments.push(argument);
        function = lambda;
    }
    arguments.reverse();

    let head = evaluate(&function, env, depth);
    if matches!(head, Value::Lambda(..) | Value::Builtin(..)) {
        return arguments
            .iter()
            .fold(head, |function, argument| call(function, evaluate(argument, env, depth), depth));
    }

    let names: Vec<String> = arguments
        .iter()
        .filter_map(|argument| name_of(&evaluate(argument, env, depth), depth))
        .collect();
    if names.is_empty() {
        return Value::Unknown;
    }
    Value::Built {
        names,
        range: built_range(apply),
    }
}

/// A string, or the `name` of an attribute set.
fn name_of(value: &Value, depth: usize) -> Option<String> {
    match value {
        Value::Str(text) => Some(text.clone()),
        Value::AttrSet(set, env) => match evaluate(&bound(set, "name")?, env, depth) {
            Value::Str(text) => Some(text),
            _ => None,
        },
        _ => None,
    }
}

fn call(function: Value, argument: Value, depth: usize) -> Value {
    match function {
        Value::Lambda(lambda, env) => {
            let env = match lambda.param() {
                Some(ast::Param::IdentParam(_)) => Some(Rc::new(Frame {
                    lambda: lambda.syntax().text_range(),
                    value: argument,
                    parent: env,
                })),
                _ => env,
            };
            evaluate_some(lambda.body(), &env, depth)
        }
        Value::Builtin(builtin, mut arguments) => {
            arguments.push(argument);
            if arguments.len() < builtin.arity() {
                return Value::Builtin(builtin, arguments);
            }
            run(builtin, arguments, depth)
        }
        _ => Value::Unknown,
    }
}

fn run(builtin: Builtin, mut arguments: Vec<Value>, depth: usize) -> Value {
    match (builtin, arguments.as_mut_slice()) {
        (Builtin::Map, [function, Value::List(items)]) => {
            Value::List(items.drain(..).map(|item| call(function.clone(), item, depth)).collect())
        }
        (Builtin::ConcatMap, [function, Value::List(items)]) => Value::List(
            items
                .drain(..)
                .flat_map(|item| as_items(call(function.clone(), item, depth)))
                .collect(),
        ),
        (Builtin::Flatten, [value]) => Value::List(flatten(std::mem::replace(value, Value::Unknown))),
        (Builtin::ConcatLists, [Value::List(lists)]) => Value::List(lists.drain(..).flat_map(as_items).collect()),
        (Builtin::Pipe, [value, Value::List(functions)]) => functions
            .drain(..)
            .fold(std::mem::replace(value, Value::Unknown), |value, function| {
                call(function, value, depth)
            }),
        _ => Value::Unknown,
    }
}

fn flatten(value: Value) -> Vec<Value> {
    match value {
        Value::List(items) => items.into_iter().flat_map(flatten).collect(),
        other => vec![other],
    }
}
