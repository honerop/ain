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
        if ch == '[' || ch == ']' {
            result.push(Token::Word(ch.to_string()));
            chars.next();
            continue;
        }
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
                if c.is_whitespace() || c == '#' || c == '"' || c == '[' || c == ']' { break; }
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

/// Deterministic source formatter for AI-authored Ain programs.
///
/// It first parses the source to reject malformed expressions, then formats
/// lexical tokens without changing definition order or comment placement.
/// The transform is idempotent and emits UTF-8 source with LF line endings.
pub fn format_source(source: &str) -> Result<String, CompileError> {
    parse(source)?;
    let mut output = String::new();
    for (line_idx, raw) in source.lines().enumerate() {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        let mut in_string = false;
        let mut escaped = false;
        let mut comment_offset = None;
        for (offset, ch) in line.char_indices() {
            if in_string {
                if escaped { escaped = false; }
                else if ch == '\\' { escaped = true; }
                else if ch == '"' { in_string = false; }
            } else if ch == '"' { in_string = true; }
            else if ch == '#' { comment_offset = Some(offset); break; }
        }
        let prefix = &line[..comment_offset.unwrap_or(line.len())];
        let tokens = lex(prefix, line_idx + 1)?;
        let mut formatted = Vec::with_capacity(tokens.len());
        for token in tokens {
            match token {
                Token::Word(word) => formatted.push(word),
                Token::String(value) => {
                    let mut text = String::from("\"");
                    for ch in value.chars() {
                        match ch {
                            '"' => text.push_str("\\\""),
                            '\\' => text.push_str("\\\\"),
                            '\n' => text.push_str("\\n"),
                            '\r' => text.push_str("\\r"),
                            '\t' => text.push_str("\\t"),
                            '\0' => text.push_str("\\0"),
                            c => text.push(c),
                        }
                    }
                    text.push('"');
                    formatted.push(text);
                }
            }
        }
        let mut joined = String::new();
        for (index, word) in formatted.iter().enumerate() {
            if index > 0 && word != "]" && formatted[index - 1] != "[" {
                joined.push(' ');
            }
            joined.push_str(word);
        }
        output.push_str(&joined);
        if let Some(offset) = comment_offset {
            if !formatted.is_empty() { output.push(' '); }
            output.push_str(line[offset..].trim_end());
        }
        output.push('\n');
    }
    Ok(output)
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

// Aliases only change surface syntax: the checked AST always uses full builtin names.
// Keeping canonical names downstream avoids duplicating type rules and backends.
fn canonical_builtin(name: &str) -> &str {
    match name {
        "lo" => "listof",
        "lp" => "listpush",
        "ln" => "listlen",
        "lm" => "listmap",
        "lf" => "listfilter",
        "ld" => "listfold",
        "lh" => "listhead",
        "lt" => "listlast",
        "lz" => "listzip",
        _ => name,
    }
}
fn callback_builtin(name: &str) -> bool {
    matches!(name, "listmap"|"listfilter"|"listfold"|"map"|"filter"|"fold"|"iterate"|"each"|"spawn")
}
fn builtin_arity(name: &str) -> Option<usize> {
    let name = canonical_builtin(name);
    Some(match name {
        "+"|"-"|"*"|"/"|"%"|"=="|"!="|"<"|"<="|">"|">="|"and"|"or"|"min"|"max"|"pow"|"concat"|"contains"|"starts"|"ends"|"before"|"after" => 2,
        "if"|"replace"|"clamp" => 3,
        "ffi2" => 4,
        "run"|"gui"|"guiupdate"|"browser"|"pair"|"push"|"get"|"field"|"hasfield"|"map"|"filter"|"setstate"|"swapstate"|"each"|"spawn"|"fswrite"|"httppost"|"listpush"|"listat"|"listmap"|"listfilter"|"option_get"|"result_get"|"mapone"|"maphas"|"mapremove"|"wavwrite"|"svgwrite"|"gltfwrite"|"tcprequest"|"httpserveonce"|"ffi0"|"tuple2"|"pathjoin"|"processrun"|"split"|"strjoin"|"listtake"|"listdrop"|"listappend"|"listzip" => 2,
        "chat"|"put"|"fold"|"iterate"|"tuple3"|"sqlquery"|"sqlexec"|"listfold"|"httprequest"|"mapset"|"mapat"|"ffi1" => 3,
        "not"|"neg"|"abs"|"assert"|"print"|"len"|"upper"|"lower"|"trim"|"str"|"word"|"skip"|"num"|"isnum"|"input"|"argv"|"readline"|"json"|"tryjson"|"encode"|"array"|"record"|"nil"|"boxint"|"boxstr"|"boxbool"|"asint"|"astext"|"asbool"|"size"|"keys"|"cell"|"readstate"|"fsread"|"fsexists"|"environ"|"httpget"|"ready"|"wait"|"poll"|"csvparse"|"trycsv"|"csvwrite"|"utf8bytes"|"frombytes"|"b64encode"|"b64decode"|"zpack"|"zunpack"|"listof"|"listlen"|"option_some"|"option_none"|"option_has"|"result_ok"|"result_err"|"result_valid"|"result_error"|"tryread"|"guitick"|"maplen"|"mapkeys"|"mapvalues"|"tomlparse"|"tomlwrite"|"tuplefst"|"tuplesnd"|"tupleswap"|"tuple3fst"|"tuple3snd"|"tuple3third"|"fslist"|"fsmkdir"|"fsremove"|"workingdir"|"lines"|"listreverse"|"listflatten"|"sortint"|"sortstr"|"listhead"|"listlast"|"listenumerate" => 1,
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
                if w == "[" {
                    // A non-empty list literal lowers to the existing typed
                    // listof/listpush operations. No new runtime operation.
                    let mut values = Vec::new();
                    loop {
                        match self.tokens.get(self.at) {
                            Some(Token::Word(end)) if end == "]" => {
                                self.at += 1;
                                break;
                            }
                            None => return Err(err(self.line, "unclosed list literal: expected ']'")),
                            _ => values.push(self.expression()?),
                        }
                    }
                    let mut values = values.into_iter();
                    let first = values.next().ok_or_else(||
                        err(self.line, "empty list literal has no inferable element type"))?;
                    let mut list = Expr::Call("listof".into(), vec![first]);
                    for value in values {
                        list = Expr::Call("listpush".into(), vec![list, value]);
                    }
                    return Ok(list);
                }
                if w == "]" {return Err(err(self.line, "unexpected ']' outside list literal"));}
                if w == "true" { return Ok(Expr::Bool(true)); }
                if w == "false" { return Ok(Expr::Bool(false)); }
                if w == "_" { return Ok(Expr::Unit); }
                if let Ok(n) = w.parse::<i64>() { return Ok(Expr::Int(n)); }
                if let Some(arity) = self.arities.get(&w).copied().or_else(|| builtin_arity(&w)) {
                    let canonical = canonical_builtin(&w);
                    let mut arguments = Vec::with_capacity(arity);
                    // A callback identifier is a name, not a call with arguments.
                    // Bare callback names save quote tokens; quoted names still work.
                    if callback_builtin(canonical) {
                        if let Some(Token::Word(callback)) = self.tokens.get(self.at) {
                            if valid_name(callback) && !is_reserved(callback) {
                                arguments.push(Expr::Str(callback.clone()));
                                self.at += 1;
                            }
                        }
                    }
                    while arguments.len() < arity { arguments.push(self.expression()?); }
                    return Ok(Expr::Call(canonical.to_string(), arguments));
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
enum Ty { Int, Bool, Str, Data, Cell, Task, List(Box<Ty>), Map(Box<Ty>), Tuple(Box<Ty>, Box<Ty>), Tuple3(Box<Ty>, Box<Ty>, Box<Ty>), Option(Box<Ty>), Result(Box<Ty>), Unit, Var(usize) }
impl Ty {
    fn rust(&self) -> String {
        match self {
            Ty::Int => "i64".into(), Ty::Bool => "bool".into(),
            Ty::Str => "String".into(), Ty::Data => "AinData".into(),
            Ty::Cell => "AinCell".into(), Ty::Task => "AinTask".into(),
            Ty::List(inner) => format!("Vec<{}>", inner.rust()),
            Ty::Map(inner) => format!("std::collections::BTreeMap<String, {}>", inner.rust()),
            Ty::Tuple(first, second) => format!("({}, {})", first.rust(), second.rust()),
            Ty::Tuple3(a,b,c) => format!("({}, {}, {})",a.rust(),b.rust(),c.rust()),
            Ty::Option(inner) => format!("Option<{}>", inner.rust()),
            Ty::Result(inner) => format!("Result<{}, String>", inner.rust()),
            Ty::Unit => "()".into(), Ty::Var(i) => format!("G{i}"),
        }
    }
    fn variables(&self, into: &mut std::collections::BTreeSet<usize>) {
        match self {
            Ty::Var(id) => {into.insert(*id);}
            Ty::List(x)|Ty::Map(x)|Ty::Option(x)|Ty::Result(x) => x.variables(into),
            Ty::Tuple(a,b) => {a.variables(into);b.variables(into);}
            Ty::Tuple3(a,b,c) => {a.variables(into);b.variables(into);c.variables(into);}
            _ => {}
        }
    }
}
struct Types { substitutions: Vec<Option<Ty>>, generalized: HashSet<String> }
impl Types {
    fn new() -> Self { Self { substitutions: Vec::new(), generalized: HashSet::new() } }
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
            Ty::Map(inner) => Ty::Map(Box::new(self.resolve(*inner))),
            Ty::Tuple(first,second) => Ty::Tuple(Box::new(self.resolve(*first)),Box::new(self.resolve(*second))),
            Ty::Tuple3(a,b,c) => Ty::Tuple3(Box::new(self.resolve(*a)),Box::new(self.resolve(*b)),Box::new(self.resolve(*c))),
            Ty::Option(inner) => Ty::Option(Box::new(self.resolve(*inner))),
            Ty::Result(inner) => Ty::Result(Box::new(self.resolve(*inner))),
            t => t,
        }
    }
    fn instantiate(&mut self, sig: &Sig) -> Sig {
        fn freshen(t: Ty, state: &mut Types, map: &mut HashMap<usize, Ty>) -> Ty {
            match t {
                Ty::Var(id) => map.entry(id).or_insert_with(||state.fresh()).clone(),
                Ty::List(x) => Ty::List(Box::new(freshen(*x,state,map))),
                Ty::Map(x) => Ty::Map(Box::new(freshen(*x,state,map))),
                Ty::Option(x) => Ty::Option(Box::new(freshen(*x,state,map))),
                Ty::Result(x) => Ty::Result(Box::new(freshen(*x,state,map))),
                Ty::Tuple(a,b) => Ty::Tuple(Box::new(freshen(*a,state,map)),Box::new(freshen(*b,state,map))),
                Ty::Tuple3(a,b,c) => Ty::Tuple3(Box::new(freshen(*a,state,map)),Box::new(freshen(*b,state,map)),Box::new(freshen(*c,state,map))),
                other => other,
            }
        }
        let mut map=HashMap::new();
        let mut args=Vec::new();
        for t in sig.args.iter().cloned() {
            let resolved=self.resolve(t);
            args.push(freshen(resolved,self,&mut map));
        }
        let result=self.resolve(sig.ret.clone());
        Sig {args,ret:freshen(result,self,&mut map)}
    }
    fn occurs(&mut self, id: usize, ty: Ty) -> bool {
        match self.resolve(ty) {
            Ty::Var(j) => id == j,
            Ty::List(inner) | Ty::Map(inner) | Ty::Option(inner) | Ty::Result(inner) => self.occurs(id, *inner),
            Ty::Tuple(a,b) => self.occurs(id,*a) || self.occurs(id,*b),
            Ty::Tuple3(a,b,c) => self.occurs(id,*a) || self.occurs(id,*b) || self.occurs(id,*c),
            _ => false,
        }
    }
    fn unresolved(ty: &Ty) -> bool {
        match ty {
            Ty::Var(_) => true,
            Ty::List(inner) | Ty::Map(inner) | Ty::Option(inner) | Ty::Result(inner) => Self::unresolved(inner),
            Ty::Tuple(a,b) => Self::unresolved(a) || Self::unresolved(b),
            Ty::Tuple3(a,b,c) => Self::unresolved(a) || Self::unresolved(b) || Self::unresolved(c),
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
            (Ty::Tuple(a,b),Ty::Tuple(c,d)) => {
                self.unify(*a,*c,line)?; self.unify(*b,*d,line)
            },
            (Ty::Tuple3(a,b,c),Ty::Tuple3(d,e,f)) => {
                self.unify(*a,*d,line)?; self.unify(*b,*e,line)?; self.unify(*c,*f,line)
            },
            (Ty::List(a), Ty::List(b)) | (Ty::Map(a), Ty::Map(b)) | (Ty::Option(a), Ty::Option(b)) |
                (Ty::Result(a), Ty::Result(b)) => self.unify(*a, *b, line),
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
                let original = functions.get(fn_name).ok_or_else(||
                    err(line, format!("unknown list callback '{}'", fn_name)))?;
                let sig = if types.generalized.contains(fn_name) {types.instantiate(original)} else {original.clone()};
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
                let original = functions.get(function_name).ok_or_else(|| 
                    err(line, format!("unknown function '{}' in {}", function_name, name)))?;
                let signature = if types.generalized.contains(function_name) {types.instantiate(original)} else {original.clone()};
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
            if let Some(original) = functions.get(name) {
                let sig = if types.generalized.contains(name) {types.instantiate(original)} else {original.clone()};
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
                "assert" => { types.unify(actual[0].clone(), Ty::Bool, line)?; Ty::Unit },
                "split" => { for arg in actual { types.unify(arg,Ty::Str,line)?; } Ty::List(Box::new(Ty::Str)) },
                "lines" => { types.unify(actual[0].clone(),Ty::Str,line)?; Ty::List(Box::new(Ty::Str)) },
                "strjoin" => {
                    types.unify(actual[0].clone(),Ty::List(Box::new(Ty::Str)),line)?;
                    types.unify(actual[1].clone(),Ty::Str,line)?;
                    Ty::Str
                },
                "listreverse" => { let elem=types.fresh();
                    types.unify(actual[0].clone(),Ty::List(Box::new(elem)),line)?;
                    actual[0].clone()
                },
                "listtake"|"listdrop" => { let elem=types.fresh();
                    types.unify(actual[0].clone(),Ty::List(Box::new(elem)),line)?;
                    types.unify(actual[1].clone(),Ty::Int,line)?;
                    actual[0].clone()
                },
                "listappend" => { types.unify(actual[0].clone(),actual[1].clone(),line)?;
                    let elem=types.fresh();
                    types.unify(actual[0].clone(),Ty::List(Box::new(elem)),line)?;
                    actual[0].clone()
                },
                "listflatten" => { let elem=types.fresh();
                    types.unify(actual[0].clone(),Ty::List(Box::new(Ty::List(Box::new(elem.clone())))),line)?;
                    Ty::List(Box::new(elem))
                },
                "sortint"|"sortstr" => { let elem=if name=="sortint" { Ty::Int } else { Ty::Str };
                    types.unify(actual[0].clone(),Ty::List(Box::new(elem.clone())),line)?;
                    Ty::List(Box::new(elem))
                },
                "fslist"|"fsmkdir"|"fsremove" => {
                    types.unify(actual[0].clone(), Ty::Str, line)?;
                    Ty::Data
                },
                "workingdir" => { types.unify(actual[0].clone(), Ty::Unit, line)?; Ty::Data },
                "pathjoin" => {
                    for arg in actual { types.unify(arg, Ty::Str, line)?; }
                    Ty::Str
                },
                "processrun" => {
                    types.unify(actual[0].clone(), Ty::Str, line)?;
                    types.unify(actual[1].clone(), Ty::Data, line)?;
                    Ty::Data
                },
                "tuple2" => Ty::Tuple(Box::new(actual[0].clone()),Box::new(actual[1].clone())),
                "tuple3" => Ty::Tuple3(Box::new(actual[0].clone()),Box::new(actual[1].clone()),Box::new(actual[2].clone())),
                "tuplefst"|"tuplesnd"|"tupleswap" => {
                    let a=types.fresh(); let b=types.fresh();
                    types.unify(actual[0].clone(),Ty::Tuple(Box::new(a.clone()),Box::new(b.clone())),line)?;
                    match name.as_str() {
                        "tuplefst" => a,
                        "tuplesnd" => b,
                        _ => Ty::Tuple(Box::new(b),Box::new(a))
                    }
                },
                "tuple3fst"|"tuple3snd"|"tuple3third" => {
                    let a=types.fresh(); let b=types.fresh(); let c=types.fresh();
                    types.unify(actual[0].clone(),Ty::Tuple3(Box::new(a.clone()),Box::new(b.clone()),Box::new(c.clone())),line)?;
                    match name.as_str() {
                        "tuple3fst" => a,
                        "tuple3snd" => b,
                        _ => c,
                    }
                },
                "tomlparse" => { types.unify(actual[0].clone(), Ty::Str, line)?; Ty::Data },
                "tomlwrite" => { types.unify(actual[0].clone(), Ty::Data, line)?; Ty::Data },
                "ffi0"|"ffi1"|"ffi2" => {
                    types.unify(actual[0].clone(), Ty::Str, line)?;
                    types.unify(actual[1].clone(), Ty::Str, line)?;
                    for arg in &actual[2..] { types.unify(arg.clone(), Ty::Int, line)?; }
                    Ty::Data
                },
                "tcprequest" => { types.unify(actual[0].clone(), Ty::Str, line)?;
                    types.unify(actual[1].clone(), Ty::Str, line)?; Ty::Data },
                "httpserveonce" => { types.unify(actual[0].clone(), Ty::Int, line)?;
                    types.unify(actual[1].clone(), Ty::Str, line)?; Ty::Data },
                "wavwrite"|"svgwrite"|"gltfwrite" => {
                    types.unify(actual[0].clone(), Ty::Str, line)?;
                    types.unify(actual[1].clone(), Ty::Data, line)?;
                    Ty::Data
                },
                "mapone" => { types.unify(actual[0].clone(), Ty::Str, line)?; Ty::Map(Box::new(actual[1].clone())) },
                "mapset" => { types.unify(actual[0].clone(), Ty::Map(Box::new(actual[2].clone())), line)?;
                    types.unify(actual[1].clone(), Ty::Str, line)?; actual[0].clone() },
                "mapat" => { types.unify(actual[0].clone(), Ty::Map(Box::new(actual[2].clone())), line)?;
                    types.unify(actual[1].clone(), Ty::Str, line)?; actual[2].clone() },
                "maphas" => { let elem=types.fresh();
                    types.unify(actual[0].clone(), Ty::Map(Box::new(elem)), line)?;
                    types.unify(actual[1].clone(), Ty::Str, line)?; Ty::Bool },
                "mapremove" => { let elem=types.fresh();
                    types.unify(actual[0].clone(), Ty::Map(Box::new(elem)), line)?;
                    types.unify(actual[1].clone(), Ty::Str, line)?; actual[0].clone() },
                "maplen" => { let elem=types.fresh();
                    types.unify(actual[0].clone(), Ty::Map(Box::new(elem)), line)?; Ty::Int },
                "mapkeys" => { let elem=types.fresh();
                    types.unify(actual[0].clone(), Ty::Map(Box::new(elem)), line)?; Ty::List(Box::new(Ty::Str)) },
                "mapvalues" => { let elem=types.fresh();
                    types.unify(actual[0].clone(), Ty::Map(Box::new(elem.clone())), line)?; Ty::List(Box::new(elem)) },
                "httprequest" => { types.unify(actual[0].clone(), Ty::Str, line)?; types.unify(actual[1].clone(), Ty::Str, line)?; types.unify(actual[2].clone(), Ty::Data, line)?; Ty::Data },
                "guiupdate" => { for value in actual { types.unify(value, Ty::Str, line)?; } Ty::Unit },
                "guitick" => { types.unify(actual[0].clone(), Ty::Int, line)?; Ty::Str },
                "option_some" => Ty::Option(Box::new(actual[0].clone())),
                "option_none" => { types.unify(actual[0].clone(), Ty::Unit, line)?; Ty::Option(Box::new(types.fresh())) },
                "option_has" => { let elem=types.fresh(); types.unify(actual[0].clone(), Ty::Option(Box::new(elem)), line)?; Ty::Bool },
                "option_get" => { types.unify(actual[0].clone(), Ty::Option(Box::new(actual[1].clone())), line)?; actual[1].clone() },
                "result_ok" => Ty::Result(Box::new(actual[0].clone())),
                "result_err" => { types.unify(actual[0].clone(), Ty::Str, line)?; Ty::Result(Box::new(types.fresh())) },
                "result_valid"|"result_error" => { let elem=types.fresh(); types.unify(actual[0].clone(), Ty::Result(Box::new(elem)), line)?; if name == "result_valid" { Ty::Bool } else { Ty::Str } },
                "result_get" => { types.unify(actual[0].clone(), Ty::Result(Box::new(actual[1].clone())), line)?; actual[1].clone() },
                "tryread" => { types.unify(actual[0].clone(), Ty::Str, line)?; Ty::Result(Box::new(Ty::Str)) },
                "listof" => Ty::List(Box::new(actual[0].clone())),
                "listpush" => { types.unify(actual[0].clone(), Ty::List(Box::new(actual[1].clone())), line)?; actual[0].clone() },
                "listat" => { types.unify(actual[1].clone(), Ty::Int, line)?;
                    let elem = types.fresh();
                    types.unify(actual[0].clone(), Ty::List(Box::new(elem.clone())), line)?;
                    elem
                },
                "listlen" => { let elem=types.fresh(); types.unify(actual[0].clone(), Ty::List(Box::new(elem)), line)?; Ty::Int },
                "listhead"|"listlast" => {
                    let elem=types.fresh();
                    types.unify(actual[0].clone(),Ty::List(Box::new(elem.clone())),line)?;
                    Ty::Option(Box::new(elem))
                },
                "listenumerate" => {
                    let elem=types.fresh();
                    types.unify(actual[0].clone(),Ty::List(Box::new(elem.clone())),line)?;
                    Ty::List(Box::new(Ty::Tuple(Box::new(Ty::Int),Box::new(elem))))
                },
                "listzip" => {
                    let a=types.fresh(); let b=types.fresh();
                    types.unify(actual[0].clone(),Ty::List(Box::new(a.clone())),line)?;
                    types.unify(actual[1].clone(),Ty::List(Box::new(b.clone())),line)?;
                    Ty::List(Box::new(Ty::Tuple(Box::new(a),Box::new(b))))
                },
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
                "argv"|"readline" => { types.unify(actual[0].clone(), Ty::Unit, line)?; Ty::Data },
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

// Dependency groups prevent a call to a forward-declared polymorphic function
// from accidentally fixing its type for all uses.
fn function_dependencies(expr:&Expr,names:&HashSet<String>,out:&mut HashSet<String>) {
    if let Expr::Call(name,args)=expr {
        if names.contains(name) {out.insert(name.clone());}
        if matches!(name.as_str(),"listmap"|"listfilter"|"listfold"|"map"|"filter"|"fold"|"iterate"|"each"|"spawn") {
            if let Some(Expr::Str(callback))=args.first() {
                if names.contains(callback) {out.insert(callback.clone());}
            }
        }
        for arg in args {function_dependencies(arg,names,out);}
    }
}
fn reaches(from:&str,target:&str,graph:&HashMap<String,HashSet<String>>,visited:&mut HashSet<String>)->bool {
    if from==target {return true;}
    if !visited.insert(from.to_string()) {return false;}
    graph.get(from).is_some_and(|deps|deps.iter().any(|dep|reaches(dep,target,graph,visited)))
}
fn check(program: Program) -> Result<Checked, CompileError> {
    let mut types=Types::new();
    let mut functions=HashMap::<String,Sig>::new();
    let names=program.functions.iter().map(|f|f.name.clone()).collect::<Vec<_>>();
    for f in &program.functions {
        let args=f.args.iter().map(|p|if p=="_" {Ty::Unit}else{types.fresh()}).collect();
        functions.insert(f.name.clone(),Sig {args,ret:types.fresh()});
    }
    let known=names.iter().cloned().collect::<HashSet<_>>();
    let graph=program.functions.iter().map(|f|{
        let mut deps=HashSet::new();
        function_dependencies(&f.expr,&known,&mut deps);
        (f.name.clone(),deps)
    }).collect::<HashMap<_,_>>();
    let mut remaining=known.clone();
    let mut groups=Vec::<Vec<String>>::new();
    for name in &names {
        if !remaining.contains(name) {continue;}
        let group=names.iter().filter(|other|{
            remaining.contains(*other)
            && reaches(name,other,&graph,&mut HashSet::new())
            && reaches(other,name,&graph,&mut HashSet::new())
        }).cloned().collect::<Vec<_>>();
        for member in &group {remaining.remove(member);}
        groups.push(group);
    }
    let mut completed=HashSet::<String>::new();
    while completed.len()<names.len() {
        let group=groups.iter().find(|group|{
            group.iter().any(|name|!completed.contains(name))
            &&group.iter().all(|name|graph[name].iter().all(|dep|
                group.contains(dep)||completed.contains(dep)))
        }).ok_or_else(||err(1,"internal error: generic dependency ordering"))?;
        for name in group {
            let f=program.functions.iter().find(|f|&f.name==name).unwrap();
            let sig=functions.get(name).unwrap().clone();
            let env=f.args.iter().zip(&sig.args).filter(|(p,_)|p.as_str()!="_")
                .map(|(p,ty)|(p.clone(),ty.clone())).collect();
            let found=check_expr(&f.expr,f.line,&env,&functions,&mut types)?;
            types.unify(found,sig.ret,f.line)?;
        }
        // Recursive definitions share monomorphic signatures internally.
        // Outside this group, every call gets a fresh scheme instantiation.
        for name in group {
            completed.insert(name.clone());
            types.generalized.insert(name.clone());
        }
    }
    let mut values=HashMap::<String,Ty>::new();
    for item in &program.items {
        match item {
            Item::Value {name,expr,line}=>{
                let ty=check_expr(expr,*line,&values,&functions,&mut types)?;
                values.insert(name.clone(),ty);
            }
            Item::Statement {expr,line}=>{check_expr(expr,*line,&values,&functions,&mut types)?;}
        }
    }
    for f in &program.functions {
        let sig=functions.get_mut(&f.name).unwrap();
        for arg in &mut sig.args {*arg=types.resolve(arg.clone());}
        sig.ret=types.resolve(sig.ret.clone());
    }
    for item in &program.items {
        if let Item::Value{name,line,..}=item {
            let t=values.get_mut(name).unwrap();
            *t=types.concrete(t.clone(),*line,&format!("value '{name}'"))?;
        }
    }
    Ok(Checked {program,function_types:functions,value_types:values})
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
            if matches!(ty, Ty::Str | Ty::Data | Ty::Cell | Ty::Task | Ty::List(_) | Ty::Map(_) | Ty::Tuple(_,_) | Ty::Tuple3(_,_,_) | Ty::Var(_) | Ty::Option(_) | Ty::Result(_)) { format!("{}.clone()", safe_id(w)) } else { safe_id(w) }
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
                "assert" => format!("{{ assert!({}, \"Ain assertion failed\"); }}", a[0]),
                "split" => format!("{{ let source={}; let needle={}; if needle.is_empty() {{ source.chars().map(|c|c.to_string()).collect::<Vec<String>>() }} else {{ source.split(&needle).map(|s|s.to_string()).collect::<Vec<String>>() }} }}",a[0],a[1]),
                "lines" => format!("({}).lines().map(|s|s.to_string()).collect::<Vec<String>>()",a[0]),
                "strjoin" => format!("({}).join(({}).as_str())",a[0],a[1]),
                "listreverse" => format!("{{ let mut list={}; list.reverse(); list }}",a[0]),
                "listtake" => format!("({}).into_iter().take(usize::try_from({}).unwrap_or(0)).collect::<Vec<_>>()",a[0],a[1]),
                "listdrop" => format!("({}).into_iter().skip(usize::try_from({}).unwrap_or(0)).collect::<Vec<_>>()",a[0],a[1]),
                "listappend" => format!("{{ let mut list={}; list.extend({}); list }}",a[0],a[1]),
                "listflatten" => format!("({}).into_iter().flatten().collect::<Vec<_>>()",a[0]),
                "listzip" => format!("({}).into_iter().zip(({}).into_iter()).collect::<Vec<_>>()",a[0],a[1]),
                "listhead" => format!("({}).into_iter().next()",a[0]),
                "listlast" => format!("({}).into_iter().last()",a[0]),
                "listenumerate" => format!("({}).into_iter().enumerate().map(|(i,v)| (i64::try_from(i).expect(\"too many list elements\"),v)).collect::<Vec<_>>()",a[0]),
                "sortint"|"sortstr" => format!("{{ let mut list={}; list.sort(); list }}",a[0]),
                "fslist" => format!("ain_fs_list({})",a[0]),
                "fsmkdir" => format!("ain_fs_mkdir({})",a[0]),
                "fsremove" => format!("ain_fs_remove({})",a[0]),
                "workingdir" => format!("ain_working_directory({})",a[0]),
                "pathjoin" => format!("ain_path_join({}, {})",a[0],a[1]),
                "processrun" => format!("ain_process_run({}, {})",a[0],a[1]),
                "tuple2" => format!("({}, {})",a[0],a[1]),
                "tuple3" => format!("({}, {}, {})",a[0],a[1],a[2]),
                "tuplefst" => format!("({}).0",a[0]),
                "tuplesnd" => format!("({}).1",a[0]),
                "tupleswap" => format!("{{ let (first,second) = {}; (second,first) }}",a[0]),
                "tuple3fst" => format!("({}).0",a[0]),
                "tuple3snd" => format!("({}).1",a[0]),
                "tuple3third" => format!("({}).2",a[0]),
                "tomlparse" => format!("ain_tomlparse({})", a[0]),
                "tomlwrite" => format!("ain_tomlwrite({})", a[0]),
                "ffi0" => format!("ain_ffi0({}, {})", a[0], a[1]),
                "ffi1" => format!("ain_ffi1({}, {}, {})", a[0], a[1], a[2]),
                "ffi2" => format!("ain_ffi2({}, {}, {}, {})", a[0], a[1], a[2], a[3]),
                "tcprequest" => format!("ain_tcp_request({}, {})", a[0], a[1]),
                "httpserveonce" => format!("ain_http_serveonce({}, {})", a[0], a[1]),
                "wavwrite" => format!("ain_wavwrite({}, {})", a[0], a[1]),
                "svgwrite" => format!("ain_svgwrite({}, {})", a[0], a[1]),
                "gltfwrite" => format!("ain_gltfwrite({}, {})", a[0], a[1]),
                "mapone" => format!("ain_map_one({}, {})", a[0], a[1]),
                "mapset" => format!("ain_map_set({}, {}, {})", a[0], a[1], a[2]),
                "mapat" => format!("ain_map_at({}, {}, {})", a[0], a[1], a[2]),
                "maphas" => format!("ain_map_has({}, {})", a[0], a[1]),
                "mapremove" => format!("ain_map_remove({}, {})", a[0], a[1]),
                "maplen" => format!("ain_map_len({})", a[0]),
                "mapkeys" => format!("ain_map_keys({})", a[0]),
                "mapvalues" => format!("ain_map_values({})", a[0]),
                "httprequest" => format!("ain_http_request_data({}, {}, {})", a[0], a[1], a[2]),
                "guiupdate" => format!("{{ ain_check_permission(\"desktop\"); ain_guiupdate({}, {}) }}", a[0], a[1]),
                "guitick" => format!("{{ ain_check_permission(\"desktop\"); ain_guitick({}) }}", a[0]),
                "option_some" => format!("Some({})", a[0]),
                "option_none" => "None".into(),
                "option_has" => format!("({}).is_some()", a[0]),
                "option_get" => format!("({}).unwrap_or({})", a[0], a[1]),
                "result_ok" => format!("Ok({})", a[0]),
                "result_err" => format!("Err({})", a[0]),
                "result_valid" => format!("({}).is_ok()", a[0]),
                "result_error" => format!("({}).err().unwrap_or_default()", a[0]),
                "result_get" => format!("({}).unwrap_or({})", a[0], a[1]),
                "tryread" => format!("ain_fs_tryread({})", a[0]),
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
                "argv" => format!("ain_argv({})", a[0]),
                "readline" => format!("ain_readline({})", a[0]),
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

#[derive(Clone,Copy,Default,PartialEq,Eq)]
struct TraitNeed { text: bool, equal: bool }
impl TraitNeed {
    fn add(&mut self, other:Self) {
        self.text|=other.text;self.equal|=other.equal;
    }
}
fn mentions_param(expr:&Expr,param:&str)->bool {
    match expr {
        Expr::Var(name)=>name==param,
        Expr::Call(_,args)=>args.iter().any(|arg|mentions_param(arg,param)),
        _=>false,
    }
}
fn needed_by_expr(expr:&Expr,param:&str,known:&HashMap<String,Vec<TraitNeed>>)->TraitNeed {
    let mut need=TraitNeed::default();
    if let Expr::Call(name,args)=expr {
        if matches!(name.as_str(),"print"|"str") &&
            args.iter().any(|arg|mentions_param(arg,param)) {need.text=true;}
        if matches!(name.as_str(),"=="|"!=") &&
            args.iter().any(|arg|mentions_param(arg,param)) {need.equal=true;}
        if let Some(callee)=known.get(name) {
            for (arg,req) in args.iter().zip(callee) {
                if mentions_param(arg,param) {need.add(*req);}
            }
        }
        // Quoted callbacks carry generic requirements into list combinators.
        if matches!(name.as_str(),"listmap"|"listfilter"|"listfold"|"map"|"filter"|"fold"|"iterate"|"each"|"spawn") {
            if let Some(Expr::Str(callback))=args.first() {
                if let Some(reqs)=known.get(callback) {
                    if args.iter().skip(1).any(|arg|mentions_param(arg,param)) {
                        for req in reqs {need.add(*req);}
                    }
                }
            }
        }
        for arg in args {need.add(needed_by_expr(arg,param,known));}
    }
    need
}
fn required_traits(program:&Program)->HashMap<String,Vec<TraitNeed>> {
    let mut current=program.functions.iter().map(|f|
        (f.name.clone(),vec![TraitNeed::default();f.args.len()])
    ).collect::<HashMap<_,_>>();
    // Monotone finite fixed point: one bit per trait per function parameter.
    loop {
        let previous=current.clone();
        for f in &program.functions {
            for (i,param) in f.args.iter().enumerate() {
                if param!="_" {
                    current.get_mut(&f.name).unwrap()[i].add(
                        needed_by_expr(&f.expr,param,&previous));
                }
            }
        }
        if current==previous {return current;}
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
    out.push_str("impl<A:AinText,B:AinText> AinText for (A,B) { fn ain_text(&self) -> String { format!(\"({}, {})\",self.0.ain_text(),self.1.ain_text()) } }\n");
    out.push_str("impl<A:AinText,B:AinText,C:AinText> AinText for (A,B,C) { fn ain_text(&self) -> String { format!(\"({}, {}, {})\",self.0.ain_text(),self.1.ain_text(),self.2.ain_text()) } }\n");
    out.push_str("impl<T: AinText> AinText for std::collections::BTreeMap<String,T> { fn ain_text(&self) -> String { format!(\"{{{}}}\", self.iter().map(|(k,v)| format!(\"{}:{}\", k, v.ain_text())).collect::<Vec<_>>().join(\",\")) } }\n");
    out.push_str("impl<T: AinText> AinText for Option<T> { fn ain_text(&self) -> String { match self { Some(v) => format!(\"some({})\",v.ain_text()), None => \"none\".into() } } }\n");
    out.push_str("impl<T: AinText> AinText for Result<T,String> { fn ain_text(&self) -> String { match self { Ok(v) => format!(\"ok({})\",v.ain_text()), Err(e) => format!(\"err({})\",e) } } }\n");
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
    let trait_needs=required_traits(&checked.program);
    for f in &checked.program.functions {
        let sig = checked.function_types.get(&f.name).unwrap();
        let mut free=std::collections::BTreeSet::new();
        for ty in &sig.args {ty.variables(&mut free);}
        sig.ret.variables(&mut free);
        let generic_header=if free.is_empty() {String::new()}else{
            let specs=free.iter().map(|id|{
                let mut bounds=vec!["Clone"];
                let mut traits=TraitNeed::default();
                for (t,need) in sig.args.iter().zip(&trait_needs[&f.name]) {
                    let mut vars=std::collections::BTreeSet::new();
                    t.variables(&mut vars);
                    if vars.contains(id) {traits.add(*need);}
                }
                if traits.text {bounds.push("AinText");}
                if traits.equal {bounds.push("PartialEq");}
                format!("G{id}: {}",bounds.join(" + "))
            }).collect::<Vec<_>>();
            format!("<{}>",specs.join(", "))
        };
        let args = f.args.iter().zip(&sig.args).enumerate().map(|(i, (name, ty))| {
            let rust_name = if name == "_" { format!("_unit_{i}") } else { safe_id(name) };
            format!("{}: {}", rust_name, ty.rust())
        }).collect::<Vec<_>>().join(", ");
        let local: HashMap<String, Ty> = f.args.iter().zip(&sig.args).filter(|(name, _)| name.as_str() != "_").map(|(n, t)| (n.clone(), t.clone())).collect();
        let body = rust_expr(&f.expr, &fn_names, &local, &HashMap::new());
        out.push_str(&format!("fn {}{}({}) -> {} {{\n    {}\n}}\n\n", safe_id(&f.name), generic_header, args, sig.ret.rust(), body));
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
    let uses_data = out.contains("ain_chat(") || out.contains("ain_browser(") || out.contains("AinData") || out.contains("AinCell") || out.contains("AinTask") || out.contains("ain_data_") || out.contains("ain_json(") || out.contains("ain_tryjson(");
    if uses_data { out.push_str(include_str!("../runtime/ain_data.rs")); }
    if out.contains("ain_argv(") || out.contains("ain_readline(") {
        out.push_str(include_str!("../runtime/ain_terminal.rs"));
    }
    let uses_effects = out.contains("ain_fs_list(") || out.contains("ain_fs_mkdir(")
        || out.contains("ain_fs_remove(") || out.contains("ain_process_run(")
        || out.contains("ain_working_directory(") || out.contains("ain_ffi0(") || out.contains("ain_ffi1(") || out.contains("ain_ffi2(")
        || out.contains("ain_tcp_request(") || out.contains("ain_http_serveonce(") || out.contains("ain_wavwrite(") || out.contains("ain_svgwrite(")
        || out.contains("ain_gltfwrite(") || out.contains("ain_sqlquery(") || out.contains("ain_sqlexec(") || out.contains("ain_check_permission(") || out.contains("ain_fs_")
        || out.contains("ain_env_get(") || out.contains("ain_http_");
    if uses_effects { out.push_str(include_str!("../runtime/ain_effects.rs")); }
    let uses_tasks = out.contains("ain_task_");
    if uses_tasks { out.push_str(include_str!("../runtime/ain_tasks.rs")); }
    if out.contains("ain_sqlquery(") || out.contains("ain_sqlexec(") { out.push_str(include_str!("../runtime/ain_sqlite.rs")); }
    let uses_serialize = out.contains("ain_svgwrite(") || out.contains("ain_gltfwrite(") || out.contains("ain_csv") || out.contains("ain_trycsv(")
        || out.contains("ain_b64") || out.contains("ain_utf8bytes(")
        || out.contains("ain_frombytes(") || out.contains("ain_zpack(") || out.contains("ain_zunpack(");
    if uses_serialize { out.push_str(include_str!("../runtime/ain_serialize.rs")); }
    if out.contains("ain_zpack(") || out.contains("ain_zunpack(") {
        out.push_str(include_str!("../runtime/ain_compress.rs"));
    }
    if out.contains("ain_list_map(") || out.contains("ain_list_filter(") || out.contains("ain_list_fold(") {
        out.push_str(include_str!("../runtime/ain_typed_list.rs"));
    }
    if out.contains("ain_http_request_data(") { out.push_str(include_str!("../runtime/ain_http_client.rs")); }
    if out.contains("ain_map_one(") || out.contains("ain_map_set(") || out.contains("ain_map_at(")
        || out.contains("ain_map_has(") || out.contains("ain_map_remove(") || out.contains("ain_map_len(")
        || out.contains("ain_map_keys(") || out.contains("ain_map_values(") {
        out.push_str(include_str!("../runtime/ain_typed_map.rs"));
    }
    if out.contains("ain_wavwrite(") { out.push_str(include_str!("../runtime/ain_audio.rs")); }
    if out.contains("ain_svgwrite(") || out.contains("ain_gltfwrite(") {
        out.push_str(include_str!("../runtime/ain_graphics.rs"));
    }
    if out.contains("ain_tcp_request(") || out.contains("ain_http_serveonce(") {
        out.push_str(include_str!("../runtime/ain_network.rs"));
    }
    if out.contains("ain_ffi0(") || out.contains("ain_ffi1(") || out.contains("ain_ffi2(") {
        out.push_str(include_str!("../runtime/ain_ffi.rs"));
    }
    if out.contains("ain_tomlparse(") || out.contains("ain_tomlwrite(") {
        out.push_str(include_str!("../runtime/ain_toml.rs"));
    }
    if out.contains("ain_fs_list(") || out.contains("ain_fs_mkdir(")
        || out.contains("ain_fs_remove(") || out.contains("ain_path_join(")
        || out.contains("ain_working_directory(") || out.contains("ain_process_run(") {
        out.push_str(include_str!("../runtime/ain_fs_process.rs"));
    }
    let uses_gui = out.contains("ain_gui(") || out.contains("ain_guiupdate(") || out.contains("ain_guitick(");
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
    #[test] fn formatter_preserves_compact_list_brackets() {
        let source="xs = [1 2 3]\\nprint ln lm inc xs\\n";
        // The source is parsed before formatting, so declare the callback.
        let source=format!("inc x = + x 1\\n{source}");
        let formatted=format_source(&source).unwrap();
        assert_eq!(formatted, source);
        assert_eq!(format_source(&formatted).unwrap(),formatted);
    }
    #[test] fn nonempty_list_literals_are_typed_and_nested() {
        let canonical="print listlen listpush listpush listof 1 2 3";
        let literal="print listlen [1 2 3]";
        assert_eq!(compile(canonical).unwrap().rust,compile(literal).unwrap().rust);
        assert!(compile("print listat [listof 1 listof 2] 0").is_ok());
        assert!(compile("print [1 true]").is_err());
        assert!(compile("print []").is_err());
        assert!(compile("print [1 2").is_err());
        assert!(compile("print 2]").is_err());
        assert!(compile("print [1 2]]").is_err());
    }
    #[test] fn compact_list_builtins_desugar_to_canonical_calls() {
        let full="inc x = + x 1\nxs = listpush listpush listof 1 2 3\nprint listlen listmap \"inc\" xs\n";
        let compact="inc x = + x 1\nxs = lp lp lo 1 2 3\nprint ln lm inc xs\n";
        assert_eq!(compile(full).unwrap().rust,compile(compact).unwrap().rust);
    }
    #[test] fn bare_callbacks_are_checked_as_function_names() {
        assert!(compile("inc x = + x 1\nprint lm inc lo 1").is_ok());
        assert!(compile("print lm missing lo 1").is_err());
        assert!(compile("inc x = + x 1\nprint lm inc lo \"abc\"").is_err());
        assert!(compile("lo x = x\nprint lo 1").is_err());
    }
    #[test] fn parametric_identity_instantiates_separately() {
        let s="identity x = x\nprint identity 4\nprint identity \"hello\"\n";
        let rust=compile(s).unwrap().rust;
        assert!(rust.contains("fn ain_identity<"));
        assert!(rust.contains("-> G"));
    }
    #[test] fn type_restrictions_still_enforced() {
        assert!(compile("same x y = + x y\nprint same 1 \"no\"").is_err());
        assert!(compile("id x = x\nx = id option_none _").is_err());
    }
    #[test] fn typed_list_zip_and_safe_access() {
        assert!(compile("print tuplefst listat listzip listof 1 listof true 0").is_ok());
        assert!(compile("print option_get listhead listof 3 0").is_ok());
        assert!(compile("print tuplefst listat listenumerate listof \"x\" 0").is_ok());
        assert!(compile("print listzip listof 1 listof 2").is_ok());
        assert!(compile("print listzip listof 1 2").is_err());
    }
    #[test] fn assert_requires_boolean() { assert!(compile("assert > 3 2").is_ok()); assert!(compile("assert 1").is_err()); }
    #[test] fn typed_triples_reject_invalid_projection() { assert!(compile("print tuple3fst tuple3 1 2 true").is_ok()); assert!(compile("print tuple3fst tuple2 1 2").is_err()); }
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
    #[test] fn terminal_builtins_are_typed() {
        assert!(compile(r#"print encode argv _
print asbool field readline _ "ok"
"#).is_ok());
        assert!(compile("print argv 2").is_err());
        assert!(compile(r#"print readline "not unit""#).is_err());
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
