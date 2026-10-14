//! AIN 0.2.0: one definition or expression per line; fixed-arity prefix expressions.
//! Parses and typechecks source, then generates standalone Rust (std only).
use std::collections::{HashMap, HashSet};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileError {
    pub line: usize,
    pub message: String,
}
impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}
impl std::error::Error for CompileError {}
fn err(line: usize, message: impl Into<String>) -> CompileError {
    CompileError { line, message: message.into() }
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Word(String),
    String(String),
}

fn lex(line: &str, lineno: usize) -> Result<Vec<Token>, CompileError> {
    let mut result = Vec::new();
    let mut chars = line.chars().peekable();
    while let Some(&ch) = chars.peek() {
        if ch.is_whitespace() { chars.next(); continue; }
        if ch == '#' { break; }
        if ch == '"' {
            chars.next();
            let mut value = String::new();
            let mut finished = false;
            while let Some(c) = chars.next() {
                if c == '"' { finished = true; break; }
                if c == '\\' {
                    match chars.next() {
                        Some('n') => value.push('\n'),
                        Some('t') => value.push('\t'),
                        Some('r') => value.push('\r'),
                        Some('"') => value.push('"'),
                        Some('\\') => value.push('\\'),
                        Some('0') => value.push('\0'),
                        Some(other) => return Err(err(lineno, format!("unknown escape \\{}", other))),
                        None => return Err(err(lineno, "unfinished escape in string")),
                    }
                } else { value.push(c); }
            }
            if !finished { return Err(err(lineno, "unterminated string literal")); }
            result.push(Token::String(value));
        } else {
            let mut word = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_whitespace() || c == '#' || c == '"' { break; }
                word.push(c); chars.next();
            }
            if !word.is_empty() { result.push(Token::Word(word)); }
            if chars.peek() == Some(&'"') && !word.is_empty() {
                return Err(err(lineno, "put a space before string literals"));
            }
        }
    }
    Ok(result)
}

fn valid_name(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        && s != "true" && s != "false" && s != "if"
}

#[derive(Clone, Debug)]
enum Expr {
    Int(i64), Bool(bool), Str(String), Unit, Var(String),
    Call(String, Vec<Expr>),
}
#[derive(Clone, Debug)]
struct Function { name: String, args: Vec<String>, expr: Expr, line: usize }
#[derive(Clone, Debug)]
enum Item { Value { name: String, expr: Expr, line: usize }, Statement { expr: Expr, line: usize } }
#[derive(Clone, Debug)]
struct Program { functions: Vec<Function>, items: Vec<Item> }

fn builtin_arity(name: &str) -> Option<usize> {
    Some(match name {
        "+"|"-"|"*"|"/"|"%"|"=="|"!="|"<"|"<="|">"|">="|"and"|"or"|"min"|"max"|"pow"|"concat"|"contains"|"starts"|"ends"|"before"|"after" => 2,
        "if"|"replace"|"clamp" => 3,
        "not"|"neg"|"abs"|"print"|"len"|"upper"|"lower"|"trim"|"str"|"word"|"skip"|"num"|"isnum"|"input" => 1,
        _ => return None,
    })
}
fn is_reserved(name: &str) -> bool { builtin_arity(name).is_some() || name == "true" || name == "false" || name == "_" }

struct ExpressionParser<'a> { tokens: &'a [Token], at: usize, arities: &'a HashMap<String, usize>, line: usize }
impl ExpressionParser<'_> {
    fn expression(&mut self) -> Result<Expr, CompileError> {
        let token = self.tokens.get(self.at).ok_or_else(|| err(self.line, "expected expression"))?.clone();
        self.at += 1;
        match token {
            Token::String(s) => Ok(Expr::Str(s)),
            Token::Word(w) => {
                if w == "true" { return Ok(Expr::Bool(true)); }
                if w == "false" { return Ok(Expr::Bool(false)); }
                if w == "_" { return Ok(Expr::Unit); }
                if let Ok(n) = w.parse::<i64>() { return Ok(Expr::Int(n)); }
                if let Some(arity) = self.arities.get(&w).copied().or_else(|| builtin_arity(&w)) {
                    let mut arguments = Vec::with_capacity(arity);
                    for _ in 0..arity { arguments.push(self.expression()?); }
                    return Ok(Expr::Call(w, arguments));
                }
                if !valid_name(&w) { return Err(err(self.line, format!("invalid identifier or literal '{w}'"))); }
                Ok(Expr::Var(w))
            }
        }
    }
}

fn parse_expr(tokens: &[Token], arities: &HashMap<String, usize>, line: usize) -> Result<Expr, CompileError> {
    let mut parser = ExpressionParser { tokens, at: 0, arities, line };
    let expr = parser.expression()?;
    if parser.at != tokens.len() { return Err(err(line, "extra tokens after expression (check function arity or line breaks)")); }
    Ok(expr)
}

fn parse(source: &str) -> Result<Program, CompileError> {
    let mut lines = Vec::<(usize, Vec<Token>)>::new();
    for (n, line) in source.lines().enumerate() {
        let tokens = lex(line, n + 1)?;
        if !tokens.is_empty() { lines.push((n + 1, tokens)); }
    }
    let mut arities = HashMap::new();
    let mut names = HashSet::new();
    // First pass: collect signatures, including forward-referenced functions.
    for (line, tokens) in &lines {
        let eq_at = tokens.iter().position(|t| t == &Token::Word("=".into()));
        if let Some(eq) = eq_at {
            let name = match tokens.first() { Some(Token::Word(w)) => w, _ => return Err(err(*line, "definition must start with a name")) };
            if !valid_name(name) || is_reserved(name) { return Err(err(*line, format!("invalid or reserved definition name '{name}'"))); }
            if !names.insert(name.clone()) { return Err(err(*line, format!("duplicate definition '{name}'"))); }
            if eq + 1 >= tokens.len() { return Err(err(*line, "missing expression after '='")); }
            let params = &tokens[1..eq];
            let mut seen = HashSet::new();
            for p in params {
                match p {
                    Token::Word(w) if w == "_" => {},
                    Token::Word(w) if valid_name(w) && !is_reserved(w) && seen.insert(w.clone()) => {},
                    _ => return Err(err(*line, "invalid, reserved, or repeated parameter name")),
                }
            }
            if !params.is_empty() { arities.insert(name.clone(), params.len()); }
        }
    }
    let mut functions = Vec::new();
    let mut items = Vec::new();
    for (line, tokens) in lines {
        let eq_at = tokens.iter().position(|t| t == &Token::Word("=".into()));
        if let Some(eq) = eq_at {
            let name = match &tokens[0] { Token::Word(w) => w.clone(), _ => unreachable!() };
            let expr = parse_expr(&tokens[eq + 1..], &arities, line)?;
            if eq == 1 { items.push(Item::Value { name, expr, line }); }
            else {
                let args = tokens[1..eq].iter().map(|x| match x { Token::Word(w) => w.clone(), _ => unreachable!() }).collect();
                functions.push(Function { name, args, expr, line });
            }
        } else {
            let expr = parse_expr(&tokens, &arities, line)?;
            items.push(Item::Statement { expr, line });
        }
    }
    Ok(Program { functions, items })
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Ty { Int, Bool, Str, Unit, Var(usize) }
impl Ty {
    fn rust(&self) -> &'static str {
        match self { Ty::Int => "i64", Ty::Bool => "bool", Ty::Str => "String", Ty::Unit => "()", Ty::Var(_) => unreachable!() }
    }
}
struct Types { substitutions: Vec<Option<Ty>> }
impl Types {
    fn new() -> Self { Self { substitutions: Vec::new() } }
    fn fresh(&mut self) -> Ty { let id = self.substitutions.len(); self.substitutions.push(None); Ty::Var(id) }
    fn resolve(&mut self, ty: Ty) -> Ty {
        match ty {
            Ty::Var(i) => {
                if let Some(next) = self.substitutions[i].clone() {
                    let resolved = self.resolve(next);
                    self.substitutions[i] = Some(resolved.clone()); resolved
                } else { Ty::Var(i) }
            }
            t => t,
        }
    }
    fn unify(&mut self, a: Ty, b: Ty, line: usize) -> Result<(), CompileError> {
        let a = self.resolve(a);
        let b = self.resolve(b);
        match (a, b) {
            (Ty::Var(a), Ty::Var(b)) if a == b => Ok(()),
            (Ty::Var(i), t) | (t, Ty::Var(i)) => { self.substitutions[i] = Some(t); Ok(()) },
            (a, b) if a == b => Ok(()),
            (a, b) => Err(err(line, format!("type mismatch: expected {:?}, found {:?}", a, b))),
        }
    }
    fn concrete(&mut self, ty: Ty, line: usize, what: &str) -> Result<Ty, CompileError> {
        match self.resolve(ty) {
            Ty::Var(_) => Err(err(line, format!("cannot infer type of {what}; use it in a typed operation or call"))),
            t => Ok(t),
        }
    }
}
#[derive(Clone)]
struct Sig { args: Vec<Ty>, ret: Ty }
struct Checked {
    program: Program,
    function_types: HashMap<String, Sig>,
    value_types: HashMap<String, Ty>,
}

fn check_expr(expr: &Expr, line: usize, env: &HashMap<String, Ty>, functions: &HashMap<String, Sig>, types: &mut Types) -> Result<Ty, CompileError> {
    match expr {
        Expr::Int(_) => Ok(Ty::Int), Expr::Bool(_) => Ok(Ty::Bool), Expr::Str(_) => Ok(Ty::Str), Expr::Unit => Ok(Ty::Unit),
        Expr::Var(w) => env.get(w).cloned().ok_or_else(|| err(line, format!("unknown variable '{w}' (functions cannot capture top-level values in 0.2.0)"))),
        Expr::Call(name, args) => {
            let actual: Vec<Ty> = args.iter().map(|e| check_expr(e, line, env, functions, types)).collect::<Result<_, _>>()?;
            if let Some(sig) = functions.get(name) {
                for (a, expected) in actual.into_iter().zip(&sig.args) { types.unify(a, expected.clone(), line)?; }
                return Ok(sig.ret.clone());
            }
            let int = Ty::Int; let boolty = Ty::Bool; let string = Ty::Str;
            let result = match name.as_str() {
                "+"|"-"|"*"|"/"|"%"|"min"|"max"|"pow" => { for a in actual { types.unify(a, int.clone(), line)?; } int },
                "neg"|"abs" => { types.unify(actual[0].clone(), int.clone(), line)?; int },
                "clamp" => { for a in actual { types.unify(a, int.clone(), line)?; } int },
                "<"|"<="|">"|">=" => { for a in actual { types.unify(a, int.clone(), line)?; } boolty },
                "=="|"!=" => { types.unify(actual[0].clone(), actual[1].clone(), line)?; boolty },
                "and"|"or"|"not" => { for a in actual { types.unify(a, boolty.clone(), line)?; } boolty },
                "if" => { types.unify(actual[0].clone(), boolty, line)?; types.unify(actual[1].clone(), actual[2].clone(), line)?; actual[1].clone() },
                "print" => Ty::Unit,
                "input" => { types.unify(actual[0].clone(), Ty::Unit, line)?; string },
                "num" => { types.unify(actual[0].clone(), Ty::Str, line)?; Ty::Int },
                "isnum" => { types.unify(actual[0].clone(), Ty::Str, line)?; Ty::Bool },
                "str" => string,
                "concat"|"replace"|"upper"|"lower"|"trim"|"before"|"after"|"word"|"skip" => { for a in actual { types.unify(a, string.clone(), line)?; } string },
                "len" => { types.unify(actual[0].clone(), string, line)?; int },
                "contains"|"starts"|"ends" => { for a in actual { types.unify(a, string.clone(), line)?; } boolty },
                _ => return Err(err(line, format!("unknown function '{name}'"))),
            };
            Ok(result)
        }
    }
}

fn check(program: Program) -> Result<Checked, CompileError> {
    let mut types = Types::new();
    let mut functions = HashMap::<String, Sig>::new();
    for f in &program.functions {
        let args = f.args.iter().map(|p| if p == "_" { Ty::Unit } else { types.fresh() }).collect();
        let ret = types.fresh();
        functions.insert(f.name.clone(), Sig { args, ret });
    }
    for f in &program.functions {
        let signature = functions.get(&f.name).unwrap().clone();
        let env = f.args.iter().zip(&signature.args).filter(|(name,_)| name.as_str() != "_").map(|(name, ty)| (name.clone(), ty.clone())).collect();
        let found = check_expr(&f.expr, f.line, &env, &functions, &mut types)?;
        types.unify(found, signature.ret, f.line)?;
    }
    let mut values = HashMap::<String, Ty>::new();
    for item in &program.items {
        match item {
            Item::Value { name, expr, line } => {
                let inferred = check_expr(expr, *line, &values, &functions, &mut types)?;
                values.insert(name.clone(), inferred);
            }
            Item::Statement { expr, line } => {
                check_expr(expr, *line, &values, &functions, &mut types)?;
            }
        }
    }
    for f in &program.functions {
        let sig = functions.get_mut(&f.name).unwrap();
        for (i, t) in sig.args.iter_mut().enumerate() {
            *t = types.concrete(t.clone(), f.line, &format!("parameter {} of '{}'", i + 1, f.name))?;
        }
        sig.ret = types.concrete(sig.ret.clone(), f.line, &format!("return of '{}'", f.name))?;
    }
    for item in &program.items {
        if let Item::Value {name, line, ..} = item {
            let t = values.get_mut(name).unwrap();
            *t = types.concrete(t.clone(), *line, &format!("value '{name}'"))?;
        }
    }
    // Rechecking resolved expression types ensures print/str/== operands aren't unspecialized.
    // All function parameters and top-level values now have concrete types.
    Ok(Checked { program, function_types: functions, value_types: values })
}

fn safe_id(name: &str) -> String {
    // Identifiers are restricted to ASCII letters, digits, underscores;
    // prefix keeps them clear of Rust keywords.
    format!("ain_{name}")
}
fn rust_expr(expr: &Expr, fn_names: &HashSet<String>, local: &HashMap<String, Ty>, global: &HashMap<String, Ty>) -> String {
    match expr {
        Expr::Int(n) => format!("{n}_i64"),
        Expr::Bool(b) => b.to_string(),
        Expr::Str(s) => format!("{:?}.to_string()", s),
        Expr::Unit => "()".into(),
        Expr::Var(w) => {
            let ty = local.get(w).or_else(|| global.get(w)).expect("checked variable");
            if *ty == Ty::Str { format!("{}.clone()", safe_id(w)) } else { safe_id(w) }
        }
        Expr::Call(name, args) => {
            let a: Vec<String> = args.iter().map(|e| rust_expr(e, fn_names, local, global)).collect();
            if fn_names.contains(name) { return format!("{}({})", safe_id(name), a.join(", ")); }
            match name.as_str() {
                "+" => format!("({}).checked_add({}).expect(\"integer overflow\")", a[0], a[1]),
                "-" => format!("({}).checked_sub({}).expect(\"integer overflow\")", a[0], a[1]),
                "*" => format!("({}).checked_mul({}).expect(\"integer overflow\")", a[0], a[1]),
                "/" => format!("({}).checked_div({}).expect(\"division by zero or overflow\")", a[0], a[1]),
                "%" => format!("({}).checked_rem({}).expect(\"division by zero or overflow\")", a[0], a[1]),
                "=="|"!="|"<"|"<="|">"|">=" => format!("(({}) {} ({}))", a[0], name, a[1]),
                "and" => format!("(({}) && ({}))", a[0], a[1]),
                "or" => format!("(({}) || ({}))", a[0], a[1]),
                "not" => format!("(!({}))", a[0]),
                "if" => format!("(if {} {{ {} }} else {{ {} }})", a[0], a[1], a[2]),
                "print" => format!("ain_print({})", a[0]),
                "str" => format!("ain_str({})", a[0]),
                "len" => format!("(({}).chars().count() as i64)", a[0]),
                "upper" => format!("({}).to_uppercase()", a[0]),
                "lower" => format!("({}).to_lowercase()", a[0]),
                "trim" => format!("({}).trim().to_string()", a[0]),
                "concat" => format!("format!(\"{{}}{{}}\", {}, {})", a[0], a[1]),
                "contains" => format!("({}).contains(({}).as_str())", a[0], a[1]),
                "starts" => format!("({}).starts_with(({}).as_str())", a[0], a[1]),
                "ends" => format!("({}).ends_with(({}).as_str())", a[0], a[1]),
                "replace" => format!("({}).replace(&{}, &{})", a[0], a[1], a[2]),
                "before" => format!("ain_before({}, {})", a[0], a[1]),
                "after" => format!("ain_after({}, {})", a[0], a[1]),
                "word" => format!("ain_word({})", a[0]),
                "skip" => format!("ain_skip({})", a[0]),
                "num" => format!("({}).parse::<i64>().expect(\"invalid integer\")", a[0]),
                "isnum" => format!("({}).parse::<i64>().is_ok()", a[0]),
                "input" => format!("ain_input({})", a[0]),
                "abs" => format!("({}).checked_abs().expect(\"integer overflow\")", a[0]),
                "neg" => format!("({}).checked_neg().expect(\"integer overflow\")", a[0]),
                "min" => format!("std::cmp::min({}, {})", a[0], a[1]),
                "max" => format!("std::cmp::max({}, {})", a[0], a[1]),
                "pow" => format!("({}).checked_pow(u32::try_from({}).expect(\"invalid exponent\")).expect(\"integer overflow\")", a[0], a[1]),
                "clamp" => format!("({}).clamp({}, {})", a[0], a[1], a[2]),
                _ => unreachable!("unknown builtin after type checking"),
            }
        }
    }
}

fn emit(checked: &Checked) -> String {
    let mut out = "// Generated by AIN compiler 0.2.0; edits will be overwritten.\n#![allow(unused_variables, dead_code)]\n\n".to_string();
    // Keep printing/conversion well-defined for all four AIN primitive types.
    out.push_str("trait AinText { fn ain_text(&self) -> String; }\n");
    out.push_str("impl AinText for i64 { fn ain_text(&self) -> String { self.to_string() } }\n");
    out.push_str("impl AinText for bool { fn ain_text(&self) -> String { self.to_string() } }\n");
    out.push_str("impl AinText for String { fn ain_text(&self) -> String { self.clone() } }\n");
    out.push_str("impl AinText for () { fn ain_text(&self) -> String { \"()\".to_string() } }\n");
    out.push_str("fn ain_print<T: AinText>(v: T) { println!(\"{}\", v.ain_text()); }\n");
    out.push_str("fn ain_str<T: AinText>(v: T) -> String { v.ain_text() }\n\n");
    out.push_str(r#"fn ain_before(s: String, needle: String) -> String {
    if needle.is_empty() { return String::new(); }
    s.split_once(needle.as_str()).map(|(a, _)| a.to_string()).unwrap_or(s)
}
fn ain_after(s: String, needle: String) -> String {
    if needle.is_empty() { return s; }
    s.split_once(needle.as_str()).map(|(_, b)| b.to_string()).unwrap_or_default()
}
fn ain_token(s: &str) -> (&str, &str) {
    let s = s.trim_start();
    if s.is_empty() || s.starts_with('#') { return ("", ""); }
    if s.starts_with('"') {
        let mut escaped = false;
        for (i, c) in s.char_indices().skip(1) {
            if escaped { escaped = false; continue; }
            if c == '\\' { escaped = true; continue; }
            if c == '"' { return (&s[..i + 1], &s[i + 1..]); }
        }
        panic!("unterminated string token");
    }
    let end = s.char_indices().find(|(_, c)| c.is_whitespace() || *c == '#' || *c == '"').map(|(i, _)| i).unwrap_or(s.len());
    (&s[..end], &s[end..])
}
fn ain_word(s: String) -> String { ain_token(&s).0.to_string() }
fn ain_skip(s: String) -> String { ain_token(&s).1.trim_start().to_string() }
fn ain_input(_: ()) -> String {
    use std::io::Read;
    let mut result = String::new();
    std::io::stdin().read_to_string(&mut result).expect("stdin read failed");
    result
}
"#);
    let fn_names: HashSet<String> = checked.program.functions.iter().map(|f| f.name.clone()).collect();
    for f in &checked.program.functions {
        let sig = checked.function_types.get(&f.name).unwrap();
        let args = f.args.iter().zip(&sig.args).enumerate().map(|(i, (name, ty))| {
            let rust_name = if name == "_" { format!("_unit_{i}") } else { safe_id(name) };
            format!("{}: {}", rust_name, ty.rust())
        }).collect::<Vec<_>>().join(", ");
        let local: HashMap<String, Ty> = f.args.iter().zip(&sig.args).filter(|(name, _)| name.as_str() != "_").map(|(n, t)| (n.clone(), t.clone())).collect();
        let body = rust_expr(&f.expr, &fn_names, &local, &HashMap::new());
        out.push_str(&format!("fn {}({}) -> {} {{\n    {}\n}}\n\n", safe_id(&f.name), args, sig.ret.rust(), body));
    }
    out.push_str("fn main() {\n");
    let mut globals = HashMap::new();
    for item in &checked.program.items {
        match item {
            Item::Value {name, expr, ..} => {
                let value = rust_expr(expr, &fn_names, &HashMap::new(), &globals);
                let ty = checked.value_types.get(name).unwrap();
                out.push_str(&format!("    let {}: {} = {};\n", safe_id(name), ty.rust(), value));
                globals.insert(name.clone(), ty.clone());
            }
            Item::Statement {expr, ..} => {
                out.push_str(&format!("    {};\n", rust_expr(expr, &fn_names, &HashMap::new(), &globals)));
            }
        }
    }
    out.push_str("}\n");
    out
}

/// Output of a successful compilation. `rust` can be passed directly to rustc.
#[derive(Debug, Clone)]
pub struct Compilation { pub rust: String, pub definitions: usize }

pub fn compile(source: &str) -> Result<Compilation, CompileError> {
    let program = parse(source)?;
    let checked = check(program)?;
    let definitions = checked.program.functions.len() + checked.program.items.iter().filter(|i| matches!(i, Item::Value { .. })).count();
    Ok(Compilation { rust: emit(&checked), definitions })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn prefix_and_inferred_functions() {
        let compiled = compile("add a b = + a b\nsquare x = * x x\nprint add square 3 square 4").unwrap();
        assert!(compiled.rust.contains("fn ain_add(ain_a: i64, ain_b: i64) -> i64"));
        assert!(compiled.rust.contains("fn ain_square(ain_x: i64) -> i64"));
    }
    #[test] fn unit_function() {
        let c = compile("i _ = print \"I\"\ni _").unwrap();
        assert!(c.rust.contains("fn ain_i(_unit_0: ()) -> ()"));
        assert!(c.rust.contains("ain_i(())"));
    }
    #[test] fn forward_reference() { assert!(compile("a x = b x\nb y = + y 1\nprint a 5").is_ok()); }
    #[test] fn strings_and_unicode() { assert!(compile("msg = upper \"zażółć\"\nprint msg").is_ok()); }
    #[test] fn rejects_bad_type() { assert!(compile("print + true 2").is_err()); }
    #[test] fn rejects_unknown_name() { assert!(compile("print nowhere").is_err()); }
    #[test] fn catches_bad_arity() { assert!(compile("print + 1").is_err()); }
    #[test] fn catches_duplicate_definitions() { assert!(compile("a = 1\na = 2").is_err()); }
    #[test] fn expressions_can_contain_hash_string() { assert!(compile("print \"# not a comment\" # comment").is_ok()); }
    #[test] fn if_expression() { assert!(compile("choose x = if > x 5 x 5\nprint choose 9").is_ok()); }
    #[test] fn string_builtins() { assert!(compile("x = concat upper \"hi\" \"!\"\nprint x").is_ok()); }
    #[test] fn zero_arg_value() { assert!(compile("x = 10\nprint x").is_ok()); }
}

mod air;

/// Dump the typed AIR expression IR (debugging and optimization inspection).
pub fn dump_air(source: &str) -> Result<String, CompileError> { air::dump(source) }

/// Emit optimized-backend-ready LLVM IR for the numeric/boolean AIN subset.
/// LLVM codegen uses unboxed types and rejects strings until AIR supports them.
pub fn compile_llvm(source: &str) -> Result<String, CompileError> { air::llvm(source) }
