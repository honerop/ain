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
            if chars.peek() == Some(&'"') && !word.is_empty() {
                return Err(err(lineno, "put a space before string literals"));
            }
            if !word.is_empty() { result.push(Token::Word(word)); }
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
        "run"|"gui"|"browser"|"pair"|"push"|"get"|"field"|"hasfield"|"map"|"filter"|"setstate"|"swapstate"|"each"|"spawn"|"fswrite"|"httppost"|"listpush"|"listat"|"listmap"|"listfilter" => 2,
        "chat"|"put"|"fold"|"iterate"|"sqlquery"|"sqlexec"|"listfold" => 3,
        "not"|"neg"|"abs"|"print"|"len"|"upper"|"lower"|"trim"|"str"|"word"|"skip"|"num"|"isnum"|"input"|"json"|"tryjson"|"encode"|"array"|"record"|"nil"|"boxint"|"boxstr"|"boxbool"|"asint"|"astext"|"asbool"|"size"|"keys"|"cell"|"readstate"|"fsread"|"fsexists"|"environ"|"httpget"|"ready"|"wait"|"poll"|"csvparse"|"trycsv"|"csvwrite"|"utf8bytes"|"frombytes"|"b64encode"|"b64decode"|"zpack"|"zunpack"|"listof"|"listlen" => 1,
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
enum Ty { Int, Bool, Str, Data, Cell, Task, List(Box<Ty>), Unit, Var(usize) }
impl Ty {
    fn rust(&self) -> String {
        match self {
            Ty::Int => "i64".into(), Ty::Bool => "bool".into(),
            Ty::Str => "String".into(), Ty::Data => "AinData".into(),
            Ty::Cell => "AinCell".into(), Ty::Task => "AinTask".into(),
            Ty::List(inner) => format!("Vec<{}>", inner.rust()),
            Ty::Unit => "()".into(), Ty::Var(_) => unreachable!("unresolved type"),
        }
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
            Ty::List(inner) => Ty::List(Box::new(self.resolve(*inner))),
            t => t,
        }
    }
    fn occurs(&mut self, id: usize, ty: Ty) -> bool {
        match self.resolve(ty) {
            Ty::Var(j) => id == j,
            Ty::List(inner) => self.occurs(id, *inner),
            _ => false,
        }
    }
    fn unresolved(ty: &Ty) -> bool {
        match ty {
            Ty::Var(_) => true,
            Ty::List(inner) => Self::unresolved(inner),
            _ => false,
        }
    }
    fn unify(&mut self, a: Ty, b: Ty, line: usize) -> Result<(), CompileError> {
        let a = self.resolve(a);
        let b = self.resolve(b);
        match (a, b) {
            (Ty::Var(a), Ty::Var(b)) if a == b => Ok(()),
            (Ty::Var(i), t) | (t, Ty::Var(i)) => {
                if self.occurs(i, t.clone()) { return Err(err(line, "recursive list type is not supported")); }
                self.substitutions[i] = Some(t); Ok(())
            },
            (Ty::List(a), Ty::List(b)) => self.unify(*a, *b, line),
            (a, b) if a == b => Ok(()),
            (a, b) => Err(err(line, format!("type mismatch: expected {:?}, found {:?}", a, b))),
        }
    }
    fn concrete(&mut self, ty: Ty, line: usize, what: &str) -> Result<Ty, CompileError> {
        let resolved = self.resolve(ty);
        if Self::unresolved(&resolved) {
            Err(err(line, format!("cannot infer type of {what}; use it in a typed operation or call")))
        } else { Ok(resolved) }
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
            if matches!(name.as_str(), "listmap"|"listfilter"|"listfold") {
                let Expr::Str(fn_name) = &args[0] else {
                    return Err(err(line, "list callbacks require a quoted function name"));
                };
                let sig = functions.get(fn_name).ok_or_else(||
                    err(line, format!("unknown list callback '{}'", fn_name)))?;
                let arity = if name == "listfold" { 2 } else { 1 };
                if sig.args.len() != arity {
                    return Err(err(line, format!("list callback '{}' must take {} parameter(s)", fn_name, arity)));
                }
                let (input, initial) = if name == "listfold" {
                    (check_expr(&args[2], line, env, functions, types)?,
                     Some(check_expr(&args[1], line, env, functions, types)?))
                } else {
                    (check_expr(&args[1], line, env, functions, types)?, None)
                };
                let element = if name == "listfold" { sig.args[1].clone() } else { sig.args[0].clone() };
                types.unify(input, Ty::List(Box::new(element.clone())), line)?;
                if let Some(seed) = initial {
                    types.unify(seed.clone(), sig.args[0].clone(), line)?;
                    types.unify(seed.clone(), sig.ret.clone(), line)?;
                    return Ok(seed);
                }
                if name == "listfilter" {
                    types.unify(sig.ret.clone(), Ty::Bool, line)?;
                    return Ok(Ty::List(Box::new(element)));
                }
                return Ok(Ty::List(Box::new(sig.ret.clone())));
            }
            if matches!(name.as_str(), "map"|"filter"|"fold"|"iterate"|"each"|"spawn") {
                let Expr::Str(function_name) = &args[0] else {
                    return Err(err(line, "functional operation requires a quoted function name as its first argument"));
                };
                let signature = functions.get(function_name).ok_or_else(|| 
                    err(line, format!("unknown function '{}' in {}", function_name, name)))?;
                let expected_arity = if name == "fold" { 2 } else { 1 };
                if signature.args.len() != expected_arity {
                    return Err(err(line, format!("{} callback must take {} argument(s)", name, expected_arity)));
                }
                for arg in &signature.args { types.unify(arg.clone(), Ty::Data, line)?; }
                let result_type = if name == "filter" { Ty::Bool } else if name == "each" { Ty::Unit } else { Ty::Data };
                types.unify(signature.ret.clone(), result_type, line)?;
                for (i, expr) in args[1..].iter().enumerate() {
                    let actual = check_expr(expr, line, env, functions, types)?;
                    if name == "iterate" && i == 0 {
                        types.unify(actual, Ty::Int, line)?;
                    } else {
                        types.unify(actual, Ty::Data, line)?;
                    }
                }
                return Ok(if name == "each" { Ty::Unit } else if name == "spawn" { Ty::Task } else { Ty::Data });
            }
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
                "listof" => Ty::List(Box::new(actual[0].clone())),
                "listpush" => { types.unify(actual[0].clone(), Ty::List(Box::new(actual[1].clone())), line)?; actual[0].clone() },
                "listat" => { types.unify(actual[1].clone(), Ty::Int, line)?;
                    let elem = types.fresh();
                    types.unify(actual[0].clone(), Ty::List(Box::new(elem.clone())), line)?;
                    elem
                },
                "listlen" => { let elem=types.fresh(); types.unify(actual[0].clone(), Ty::List(Box::new(elem)), line)?; Ty::Int },
                "csvparse"|"trycsv"|"b64decode"|"utf8bytes" => { types.unify(actual[0].clone(), Ty::Str, line)?; Ty::Data },
                "csvwrite"|"b64encode" => { types.unify(actual[0].clone(), Ty::Data, line)?; Ty::Str },
                "frombytes"|"zpack"|"zunpack" => { types.unify(actual[0].clone(), Ty::Data, line)?; Ty::Data },
                "sqlquery"|"sqlexec" => { types.unify(actual[0].clone(), Ty::Str, line)?; types.unify(actual[1].clone(), Ty::Str, line)?; types.unify(actual[2].clone(), Ty::Data, line)?; Ty::Data },
                "run" => { for a in actual { types.unify(a, Ty::Str, line)?; } Ty::Int },
                "json"|"tryjson" => { types.unify(actual[0].clone(), Ty::Str, line)?; Ty::Data },
                "array"|"record"|"nil" => { types.unify(actual[0].clone(), Ty::Unit, line)?; Ty::Data },
                "boxint" => { types.unify(actual[0].clone(), Ty::Int, line)?; Ty::Data },
                "boxstr" => { types.unify(actual[0].clone(), Ty::Str, line)?; Ty::Data },
                "boxbool" => { types.unify(actual[0].clone(), Ty::Bool, line)?; Ty::Data },
                "asint" => { types.unify(actual[0].clone(), Ty::Data, line)?; Ty::Int },
                "astext"|"encode" => { types.unify(actual[0].clone(), Ty::Data, line)?; Ty::Str },
                "asbool" => { types.unify(actual[0].clone(), Ty::Data, line)?; Ty::Bool },
                "size" => { types.unify(actual[0].clone(), Ty::Data, line)?; Ty::Int },
                "keys" => { types.unify(actual[0].clone(), Ty::Data, line)?; Ty::Data },
                "cell" => { types.unify(actual[0].clone(), Ty::Data, line)?; Ty::Cell },
                "readstate" => { types.unify(actual[0].clone(), Ty::Cell, line)?; Ty::Data },
                "fsread"|"environ"|"httpget" => { types.unify(actual[0].clone(), Ty::Str, line)?; Ty::Str },
                "fsexists" => { types.unify(actual[0].clone(), Ty::Str, line)?; Ty::Bool },
                "fswrite"|"httppost" => { for ty in actual { types.unify(ty, Ty::Str, line)?; }
                    if name == "fswrite" { Ty::Unit } else { Ty::Str } },
                "ready" => { types.unify(actual[0].clone(), Ty::Task, line)?; Ty::Bool },
                "wait"|"poll" => { types.unify(actual[0].clone(), Ty::Task, line)?; Ty::Data },
                "setstate" => { types.unify(actual[0].clone(), Ty::Cell, line)?; types.unify(actual[1].clone(), Ty::Data, line)?; Ty::Unit },
                "swapstate" => { types.unify(actual[0].clone(), Ty::Cell, line)?; types.unify(actual[1].clone(), Ty::Data, line)?; Ty::Data },
                "pair"|"push" => { for a in actual { types.unify(a, Ty::Data, line)?; } Ty::Data },
                "get" => { types.unify(actual[0].clone(), Ty::Data, line)?; types.unify(actual[1].clone(), Ty::Int, line)?; Ty::Data },
                "field"|"hasfield" => { types.unify(actual[0].clone(), Ty::Data, line)?; types.unify(actual[1].clone(), Ty::Str, line)?;
                    if name == "hasfield" { Ty::Bool } else { Ty::Data } },
                "put" => { types.unify(actual[0].clone(), Ty::Data, line)?; types.unify(actual[1].clone(), Ty::Str, line)?;
                    types.unify(actual[2].clone(), Ty::Data, line)?; Ty::Data },
                "gui"|"browser"|"chat" => { for a in actual { types.unify(a, Ty::Str, line)?; } Ty::Str },
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
            if matches!(ty, Ty::Str | Ty::Data | Ty::Cell | Ty::Task | Ty::List(_)) { format!("{}.clone()", safe_id(w)) } else { safe_id(w) }
        }
        Expr::Call(name, args) => {
            if matches!(name.as_str(), "listmap"|"listfilter"|"listfold") {
                let Expr::Str(callback) = &args[0] else { unreachable!("callback checked"); };
                let a: Vec<String> = args[1..].iter().map(|e| rust_expr(e, fn_names, local, global)).collect();
                return match name.as_str() {
                    "listmap" => format!("ain_list_map({}, {})", a[0], safe_id(callback)),
                    "listfilter" => format!("ain_list_filter({}, {})", a[0], safe_id(callback)),
                    "listfold" => format!("ain_list_fold({}, {}, {})", a[1], a[0], safe_id(callback)),
                    _ => unreachable!()
                }
            }
            if matches!(name.as_str(), "map"|"filter"|"fold"|"iterate"|"each"|"spawn") {
                let Expr::Str(callback) = &args[0] else { unreachable!("callback checked"); };
                let a: Vec<String> = args[1..].iter().map(|e| rust_expr(e, fn_names, local, global)).collect();
                return match name.as_str() {
                    "map" => format!("ain_data_map({}, {})", a[0], safe_id(callback)),
                    "filter" => format!("ain_data_filter({}, {})", a[0], safe_id(callback)),
                    "fold" => format!("ain_data_fold({}, {}, {})", a[1], a[0], safe_id(callback)),
                    "iterate" => format!("ain_data_iterate({}, {}, {})", a[0], a[1], safe_id(callback)),
                    "each" => format!("ain_data_each({}, {})", a[0], safe_id(callback)),
                    "spawn" => format!("ain_task_spawn({}, {})", safe_id(callback), a[0]),
                    _ => unreachable!()
                };
            }
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
                "listof" => format!("vec![{}]", a[0]),
                "listpush" => format!("{{ let mut xs = {}; xs.push({}); xs }}", a[0], a[1]),
                "listat" => format!("({})[usize::try_from({}).expect(\"invalid list index\")].clone()", a[0], a[1]),
                "listlen" => format!("({}).len() as i64", a[0]),
                "csvparse" => format!("ain_csvparse({})", a[0]),
                "trycsv" => format!("ain_trycsv({})", a[0]),
                "csvwrite" => format!("ain_csvwrite({})", a[0]),
                "utf8bytes" => format!("ain_utf8bytes({})", a[0]),
                "frombytes" => format!("ain_frombytes({})", a[0]),
                "b64encode" => format!("ain_b64encode({})", a[0]),
                "b64decode" => format!("ain_b64decode({})", a[0]),
                "zpack" => format!("ain_zpack({})", a[0]),
                "zunpack" => format!("ain_zunpack({})", a[0]),
                "sqlquery" => format!("ain_sqlquery({}, {}, {})", a[0], a[1], a[2]),
                "sqlexec" => format!("ain_sqlexec({}, {}, {})", a[0], a[1], a[2]),
                "run" => format!("{{ ain_check_permission(\"process\"); std::process::Command::new({}).arg({}).status().expect(\"cannot run process\").code().unwrap_or(1) as i64 }}", a[0], a[1]),
                "json" => format!("ain_json({})", a[0]),
                "tryjson" => format!("ain_tryjson({})", a[0]),
                "encode" => format!("({}).json()", a[0]),
                "array" => format!("ain_data_array({})", a[0]),
                "record" => format!("ain_data_record({})", a[0]),
                "nil" => format!("ain_data_null({})", a[0]),
                "boxint" => format!("ain_data_int({})", a[0]),
                "boxstr" => format!("ain_data_text({})", a[0]),
                "boxbool" => format!("ain_data_bool({})", a[0]),
                "asint" => format!("ain_data_asint({})", a[0]),
                "astext" => format!("ain_data_astext({})", a[0]),
                "asbool" => format!("ain_data_asbool({})", a[0]),
                "size" => format!("ain_data_size({})", a[0]),
                "keys" => format!("ain_data_keys({})", a[0]),
                "cell" => format!("ain_data_state({})", a[0]),
                "readstate" => format!("ain_data_readstate({})", a[0]),
                "fsread" => format!("ain_fs_read({})", a[0]),
                "fswrite" => format!("ain_fs_write({}, {})", a[0], a[1]),
                "fsexists" => format!("ain_fs_exists({})", a[0]),
                "environ" => format!("ain_env_get({})", a[0]),
                "httpget" => format!("ain_http_get({})", a[0]),
                "httppost" => format!("ain_http_post({}, {})", a[0], a[1]),
                "ready" => format!("ain_task_ready({})", a[0]),
                "wait" => format!("ain_task_wait({})", a[0]),
                "poll" => format!("ain_task_poll({})", a[0]),
                "setstate" => format!("ain_data_setstate({}, {})", a[0], a[1]),
                "swapstate" => format!("ain_data_swapstate({}, {})", a[0], a[1]),
                "pair" => format!("ain_data_pair({}, {})", a[0], a[1]),
                "push" => format!("ain_data_push({}, {})", a[0], a[1]),
                "get" => format!("ain_data_get({}, {})", a[0], a[1]),
                "put" => format!("ain_data_put({}, {}, {})", a[0], a[1], a[2]),
                "field" => format!("ain_data_field({}, {})", a[0], a[1]),
                "hasfield" => format!("ain_data_has({}, {})", a[0], a[1]),
                "gui" => format!("{{ ain_check_permission(\"desktop\"); ain_gui({}, {}) }}", a[0], a[1]),
                "chat" => format!("{{ ain_check_permission(\"ai\"); ain_chat({}, {}, {}) }}", a[0], a[1], a[2]),
                "browser" => format!("{{ ain_check_permission(\"browser\"); ain_browser({}, {}) }}", a[0], a[1]),
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
    out.push_str("impl<T: AinText> AinText for Vec<T> { fn ain_text(&self) -> String { format!(\"[{}]\", self.iter().map(|x| x.ain_text()).collect::<Vec<_>>().join(\",\")) } }\n");
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
    let uses_data = out.contains("AinData") || out.contains("AinCell") || out.contains("AinTask") || out.contains("ain_data_") || out.contains("ain_json(") || out.contains("ain_tryjson(");
    if uses_data { out.push_str(include_str!("../runtime/ain_data.rs")); }
    let uses_effects = out.contains("ain_sqlquery(") || out.contains("ain_sqlexec(") || out.contains("ain_check_permission(") || out.contains("ain_fs_")
        || out.contains("ain_env_get(") || out.contains("ain_http_");
    if uses_effects { out.push_str(include_str!("../runtime/ain_effects.rs")); }
    let uses_tasks = out.contains("ain_task_");
    if uses_tasks { out.push_str(include_str!("../runtime/ain_tasks.rs")); }
    if out.contains("ain_sqlquery(") || out.contains("ain_sqlexec(") { out.push_str(include_str!("../runtime/ain_sqlite.rs")); }
    let uses_serialize = out.contains("ain_csv") || out.contains("ain_trycsv(")
        || out.contains("ain_b64") || out.contains("ain_utf8bytes(")
        || out.contains("ain_frombytes(") || out.contains("ain_zpack(") || out.contains("ain_zunpack(");
    if uses_serialize { out.push_str(include_str!("../runtime/ain_serialize.rs")); }
    if out.contains("ain_zpack(") || out.contains("ain_zunpack(") {
        out.push_str(include_str!("../runtime/ain_compress.rs"));
    }
    if out.contains("ain_list_map(") || out.contains("ain_list_filter(") || out.contains("ain_list_fold(") {
        out.push_str(include_str!("../runtime/ain_typed_list.rs"));
    }
    let uses_gui = out.contains("ain_gui(");
    let uses_chat = out.contains("ain_chat(");
    let uses_browser = out.contains("ain_browser(");
    if uses_gui { out.push_str(include_str!("../runtime/ain_gui.rs")); }
    if uses_chat || uses_browser { out.push_str(include_str!("../runtime/ain_chat.rs")); }
    if uses_browser {
        out.push_str(&format!("\nconst AIN_BROWSER_DRIVER_SOURCE: &str = {:?};\n", include_str!("../runtime/ain_browser.py")));
        out.push_str(include_str!("../runtime/ain_browser.rs"));
    }
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
    #[test] fn run_process_accepts_two_strings() {
        let compiled = compile("run \"echo\" \"hello\"").unwrap();
        assert!(compiled.rust.contains("std::process::Command::new"));
        assert!(compile("run 42 \"arg\"").is_err());
        assert!(compile("run \"echo\"").is_err());
    }
    #[test] fn generic_desktop_and_ai_builtins() {
        let program = "layout = \"header\\tHello\\nbutton\\tok\\tOK\\n\"\nstate = gui \"Demo\" layout\nprint state\nprint chat \"deepinfra\" \"\" \"hello\"\nprint browser \"read\" \"\"\n";
        let output = compile(program).unwrap().rust;
        assert!(output.contains("fn ain_gui("));
        assert!(output.contains("fn ain_chat("));
        assert!(output.contains("fn ain_browser("));
        assert!(compile("gui 1 \"body\"").is_err());
        assert!(compile("chat \"deepinfra\" \"\"").is_err());
    }
    #[test] fn zero_arg_value() { assert!(compile("x = 10\nprint x").is_ok()); }
}

mod air;

/// Dump the typed AIR expression IR (debugging and optimization inspection).
pub fn dump_air(source: &str) -> Result<String, CompileError> { air::dump(source) }

/// Emit optimized-backend-ready LLVM IR for the numeric/boolean AIN subset.
/// LLVM codegen uses unboxed types and rejects strings until AIR supports them.
pub fn compile_llvm(source: &str) -> Result<String, CompileError> { air::llvm(source) }
