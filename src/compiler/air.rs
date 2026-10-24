//! AIR v0.3: a typed, optimizable expression IR lowered directly to LLVM IR.
//! The C-producing self-hosted bootstrap remains a separate experimental backend.
use super::{check, err, parse, Checked, CompileError, Expr, Item, Ty};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
struct Node { ty: Ty, kind: Kind }
#[derive(Debug, Clone)]
enum Kind { Int(i64), Bool(bool), Unit, Var(String), Call(String, Vec<Node>) }
struct Function { name: String, args: Vec<(String, Ty)>, ret: Ty, body: Node }
enum Top { Value(String, Node), Statement(Node) }
struct Module { functions: Vec<Function>, items: Vec<Top> }

fn lower_expr(e: &Expr, env: &HashMap<String, Ty>, checked: &Checked, line: usize) -> Result<Node, CompileError> {
    let (ty, kind) = match e {
        Expr::Int(v) => (Ty::Int, Kind::Int(*v)),
        Expr::Bool(v) => (Ty::Bool, Kind::Bool(*v)),
        Expr::Unit => (Ty::Unit, Kind::Unit),
        Expr::Str(_) => return Err(err(line, "LLVM backend does not support strings yet; use the Rust backend")),
        Expr::Var(name) => (env.get(name).cloned().ok_or_else(|| err(line, format!("undefined AIR value '{name}'")))?, Kind::Var(name.clone())),
        Expr::Call(name, operands) => {
            let args = operands.iter().map(|x| lower_expr(x, env, checked, line)).collect::<Result<Vec<_>,_>>()?;
            let result = if let Some(sig) = checked.function_types.get(name) {
                sig.ret.clone()
            } else {
                match name.as_str() {
                    "+"|"-"|"*"|"/"|"%"|"min"|"max"|"pow"|"neg"|"abs"|"clamp" => Ty::Int,
                    "=="|"!="|"<"|"<="|">"|">="|"and"|"or"|"not" => Ty::Bool,
                    "if" => args[1].ty.clone(),
                    "print"|"assert" => Ty::Unit,
                    _ => return Err(err(line, format!("LLVM backend does not support '{name}' yet; use the Rust backend"))),
                }
            };
            (result, Kind::Call(name.clone(), args))
        }
    };
    Ok(fold(Node { ty, kind }))
}

fn fold(node: Node) -> Node {
    let Node { ty, kind } = node;
    let (name, args) = match kind { Kind::Call(name, args) => (name, args), other => return Node { ty, kind: other } };
    let literal = match (name.as_str(), args.as_slice()) {
        ("if", [Node {kind:Kind::Bool(c),..}, a, b]) => return if *c {a.clone()} else {b.clone()},
        ("and", [Node {kind:Kind::Bool(false),..}, _]) => Some(Kind::Bool(false)),
        ("or", [Node {kind:Kind::Bool(true),..}, _]) => Some(Kind::Bool(true)),
        ("not", [Node {kind:Kind::Bool(b),..}]) => Some(Kind::Bool(!b)),
        ("neg", [Node {kind:Kind::Int(a),..}]) => a.checked_neg().map(Kind::Int),
        ("abs", [Node {kind:Kind::Int(a),..}]) => a.checked_abs().map(Kind::Int),
        ("+", [Node {kind:Kind::Int(a),..}, Node {kind:Kind::Int(b),..}]) => a.checked_add(*b).map(Kind::Int),
        ("-", [Node {kind:Kind::Int(a),..}, Node {kind:Kind::Int(b),..}]) => a.checked_sub(*b).map(Kind::Int),
        ("*", [Node {kind:Kind::Int(a),..}, Node {kind:Kind::Int(b),..}]) => a.checked_mul(*b).map(Kind::Int),
        ("/", [Node {kind:Kind::Int(a),..}, Node {kind:Kind::Int(b),..}]) => a.checked_div(*b).map(Kind::Int),
        ("%", [Node {kind:Kind::Int(a),..}, Node {kind:Kind::Int(b),..}]) => a.checked_rem(*b).map(Kind::Int),
        ("<", [Node {kind:Kind::Int(a),..}, Node {kind:Kind::Int(b),..}]) => Some(Kind::Bool(a < b)),
        ("<=", [Node {kind:Kind::Int(a),..}, Node {kind:Kind::Int(b),..}]) => Some(Kind::Bool(a <= b)),
        (">", [Node {kind:Kind::Int(a),..}, Node {kind:Kind::Int(b),..}]) => Some(Kind::Bool(a > b)),
        (">=", [Node {kind:Kind::Int(a),..}, Node {kind:Kind::Int(b),..}]) => Some(Kind::Bool(a >= b)),
        ("==", [Node {kind:Kind::Int(a),..}, Node {kind:Kind::Int(b),..}]) => Some(Kind::Bool(a == b)),
        ("!=", [Node {kind:Kind::Int(a),..}, Node {kind:Kind::Int(b),..}]) => Some(Kind::Bool(a != b)),
        ("==", [Node {kind:Kind::Bool(a),..}, Node {kind:Kind::Bool(b),..}]) => Some(Kind::Bool(a == b)),
        ("!=", [Node {kind:Kind::Bool(a),..}, Node {kind:Kind::Bool(b),..}]) => Some(Kind::Bool(a != b)),
        _ => None,
    };
    Node { ty, kind: literal.unwrap_or(Kind::Call(name, args)) }
}

fn lower(checked: &Checked) -> Result<Module, CompileError> {
    let mut functions = Vec::new();
    for f in &checked.program.functions {
        let sig = &checked.function_types[&f.name];
        let args: Vec<(String,Ty)> = f.args.iter().cloned().zip(sig.args.iter().cloned()).collect();
        let env = args.iter().filter(|(name,_)| name != "_").cloned().collect();
        functions.push(Function {name:f.name.clone(),args,ret:sig.ret.clone(),body:lower_expr(&f.expr,&env,checked,f.line)?});
    }
    let mut items = Vec::new();
    let mut globals = HashMap::new();
    for item in &checked.program.items {
        match item {
            Item::Value{name,expr,line} => {
                let value = lower_expr(expr,&globals,checked,*line)?;
                globals.insert(name.clone(),value.ty.clone());
                items.push(Top::Value(name.clone(),value));
            }
            Item::Statement{expr,line} => items.push(Top::Statement(lower_expr(expr,&globals,checked,*line)?)),
        }
    }
    Ok(Module {functions,items})
}

fn ty(t:&Ty) -> &'static str { match t {Ty::Int=>"i64",Ty::Bool=>"i1",Ty::Unit=>"i8",Ty::Str=>"ptr",Ty::Data|Ty::Cell|Ty::Task|Ty::List(_)|Ty::Map(_)|Ty::Tuple(_,_)|Ty::Tuple3(_,_,_)|Ty::Option(_)|Ty::Result(_)=>unreachable!("Structured types are not supported by AIR/LLVM"),Ty::Var(_)=>unreachable!("types must be resolved") } }
fn air_node(n:&Node) -> String {
    match &n.kind {
        Kind::Int(v) => format!("{v}:i64"), Kind::Bool(v) => format!("{v}:bool"), Kind::Unit => "_:unit".into(),
        Kind::Var(v) => format!("{v}:{}",ty(&n.ty)),
        Kind::Call(f,args) => format!("({f}:{} {})",ty(&n.ty),args.iter().map(air_node).collect::<Vec<_>>().join(" ")),
    }
}
fn air_text(m:&Module) -> String {
    let mut out = String::from("air.module 0.3\n");
    for f in &m.functions {
        let args = f.args.iter().map(|(n,t)|format!("{n}:{}",ty(t))).collect::<Vec<_>>().join(" ");
        out.push_str(&format!("func {} {} -> {} = {}\n",f.name,args,ty(&f.ret),air_node(&f.body)));
    }
    for t in &m.items {
        match t {Top::Value(n,x)=>out.push_str(&format!("let {n}:{} = {}\n",ty(&x.ty),air_node(x))),Top::Statement(x)=>out.push_str(&format!("eval {}\n",air_node(x)))}
    }
    out
}

struct Emitter { code:String, counter:usize, current:String, variables:HashMap<String,(String,Ty)>, names:HashSet<String> }
impl Emitter {
    fn new(names:HashSet<String>) -> Self {Self{code:String::new(),counter:0,current:"entry".into(),variables:HashMap::new(),names}}
    fn tmp(&mut self) -> String {let t=format!("%v{}",self.counter);self.counter+=1;t}
    fn label(&mut self,prefix:&str) -> String {let t=format!("{}{}",prefix,self.counter);self.counter+=1;t}
    fn line(&mut self,line:impl AsRef<str>) {self.code.push_str("  ");self.code.push_str(line.as_ref());self.code.push('\n');}
    fn block(&mut self,name:&str) {self.current=name.into();self.code.push_str(name);self.code.push_str(":\n");}
    fn trap_if(&mut self,condition:&str) {
        let bad=self.label("trap");let good=self.label("ok");
        self.line(format!("br i1 {condition}, label %{bad}, label %{good}"));
        self.block(&bad);self.line("call void @llvm.trap()");self.line("unreachable");self.block(&good);
    }
    fn guarded_binary(&mut self,op:&str,a:&str,b:&str) -> String {
        let pair=self.tmp();let result=self.tmp();let overflow=self.tmp();
        self.line(format!("{pair} = call {{ i64, i1 }} @llvm.{op}.with.overflow.i64(i64 {a}, i64 {b})"));
        self.line(format!("{result} = extractvalue {{ i64, i1 }} {pair}, 0"));
        self.line(format!("{overflow} = extractvalue {{ i64, i1 }} {pair}, 1"));
        self.trap_if(&overflow);result
    }
    fn binary(&mut self,op:&str,t:&str,a:&str,b:&str) -> String {
        let v=self.tmp();self.line(format!("{v} = {op} {t} {a}, {b}"));v
    }
    fn lazy(&mut self,name:&str,args:&[Node]) -> String {
        let cond=self.gen(&args[0]);
        let left=self.label("left");let right=self.label("right");let merge=self.label("merge");
        self.line(format!("br i1 {cond}, label %{left}, label %{right}"));
        self.block(&left);
        let (lv,lt) = if name=="if" { (self.gen(&args[1]),args[1].ty.clone()) }
            else if name=="and" { (self.gen(&args[1]),Ty::Bool) } else { ("1".into(),Ty::Bool) };
        let left_end=self.current.clone();self.line(format!("br label %{merge}"));
        self.block(&right);
        let (rv,rt) = if name=="if" { (self.gen(&args[2]),args[2].ty.clone()) }
            else if name=="and" { ("0".into(),Ty::Bool) } else { (self.gen(&args[1]),Ty::Bool) };
        let right_end=self.current.clone();self.line(format!("br label %{merge}"));
        self.block(&merge);
        debug_assert_eq!(lt,rt);
        let v=self.tmp();self.line(format!("{v} = phi {} [{lv}, %{left_end}], [{rv}, %{right_end}]",ty(&lt)));v
    }
    fn gen(&mut self,node:&Node) -> String {
        match &node.kind {
            Kind::Int(v)=>v.to_string(),Kind::Bool(v)=>if *v {"1".into()} else {"0".into()},Kind::Unit=>"0".into(),
            Kind::Var(n)=>self.variables[n].0.clone(),
            Kind::Call(name,args) => {
                if matches!(name.as_str(),"if"|"and"|"or") {return self.lazy(name,args);}
                let x:Vec<String>=args.iter().map(|a|self.gen(a)).collect();
                if self.names.contains(name) {
                    let call=self.tmp();let actual=args.iter().zip(&x).map(|(a,x)|format!("{} {x}",ty(&a.ty))).collect::<Vec<_>>().join(", ");
                    self.line(format!("{call} = call {} @babel_{name}({actual})",ty(&node.ty)));return call;
                }
                match name.as_str() {
                    "+"=>self.guarded_binary("sadd",&x[0],&x[1]),
                    "-"=>self.guarded_binary("ssub",&x[0],&x[1]),
                    "*"=>self.guarded_binary("smul",&x[0],&x[1]),
                    "neg"=>self.guarded_binary("ssub","0",&x[0]),
                    "abs"=> {let neg=self.guarded_binary("ssub","0",&x[0]);let cond=self.binary("icmp slt","i64",&x[0],"0");let v=self.tmp();self.line(format!("{v} = select i1 {cond}, i64 {neg}, i64 {}",x[0]));v},
                    "/"|"%" => {
                        let zero=self.binary("icmp eq","i64",&x[1],"0");
                        let min=self.binary("icmp eq","i64",&x[0],"-9223372036854775808");
                        let minus=self.binary("icmp eq","i64",&x[1],"-1");
                        let bad_overflow=self.binary("and","i1",&min,&minus);
                        let bad=self.binary("or","i1",&zero,&bad_overflow);
                        self.trap_if(&bad);
                        self.binary(if name=="/"{"sdiv"}else{"srem"},"i64",&x[0],&x[1])
                    },
                    "=="|"!="|"<"|"<="|">"|">=" => {
                        let predicate=match name.as_str(){"=="=>"eq","!="=>"ne","<"=>"slt","<="=>"sle",">"=>"sgt",_=>"sge"};
                        self.binary(&format!("icmp {predicate}"),ty(&args[0].ty),&x[0],&x[1])
                    },
                    "not"=>self.binary("xor","i1",&x[0],"1"),
                    "min"|"max"=> {let pred=if name=="min"{"slt"}else{"sgt"};let c=self.binary(&format!("icmp {pred}"),"i64",&x[0],&x[1]);let v=self.tmp();self.line(format!("{v} = select i1 {c}, i64 {}, i64 {}",x[0],x[1]));v},
                    "clamp"=>{let invalid=self.binary("icmp sgt","i64",&x[1],&x[2]);self.trap_if(&invalid);let lo=self.gen_minmax("max",&x[0],&x[1]);self.gen_minmax("min",&lo,&x[2])},
                    "pow"=>{let v=self.tmp();self.line(format!("{v} = call i64 @babel_pow(i64 {}, i64 {})",x[0],x[1]));v},
                    "assert"=>{let inverted=self.binary("xor","i1",&x[0],"1");self.trap_if(&inverted);"0".into()},
                    "print"=> {
                        let fmt=if args[0].ty==Ty::Int {"@fmt_i64"}else{"@fmt_str"};
                        let printed=match args[0].ty {
                            Ty::Int=>format!("i64 {}",x[0]),
                            Ty::Bool=>{let v=self.tmp();self.line(format!("{v} = select i1 {}, ptr @bool_true, ptr @bool_false",x[0]));format!("ptr {v}")},
                            Ty::Unit=>"ptr @unit_string".into(),
                            _=>unreachable!(),
                        };
                        self.line(format!("call i32 (ptr, ...) @printf(ptr {fmt}, {printed})"));"0".into()
                    }
                    _=>unreachable!("AIR unsupported function was rejected while lowering"),
                }
            }
        }
    }
    fn gen_minmax(&mut self,name:&str,a:&str,b:&str)->String {
        let pred=if name=="min"{"slt"}else{"sgt"};let cond=self.binary(&format!("icmp {pred}"),"i64",a,b);
        let v=self.tmp();self.line(format!("{v} = select i1 {cond}, i64 {a}, i64 {b}"));v
    }
}

fn emit_llvm(m:&Module)->String {
    let mut out=String::from("; Generated by BABEL AIR 0.3 (typed, unboxed). LLVM opaque pointer IR.\n");
    out.push_str("@fmt_i64 = private constant [6 x i8] c\"%lld\\0A\\00\"\n@fmt_str = private constant [4 x i8] c\"%s\\0A\\00\"\n@bool_true = private constant [5 x i8] c\"true\\00\"\n@bool_false = private constant [6 x i8] c\"false\\00\"\n@unit_string = private constant [3 x i8] c\"()\\00\"\n");
    out.push_str("declare i32 @printf(ptr, ...)\ndeclare void @llvm.trap()\ndeclare { i64, i1 } @llvm.sadd.with.overflow.i64(i64, i64)\ndeclare { i64, i1 } @llvm.ssub.with.overflow.i64(i64, i64)\ndeclare { i64, i1 } @llvm.smul.with.overflow.i64(i64, i64)\n\n");
    // Implement exponentiation with a checked iterative square-and-multiply loop.
    out.push_str("define internal i64 @babel_pow(i64 %base, i64 %exp) {\nentry:\n  %neg = icmp slt i64 %exp, 0\n  %big = icmp sgt i64 %exp, 4294967295\n  %invalid = or i1 %neg, %big\n  br i1 %invalid, label %trap, label %loop\nloop:\n  %n = phi i64 [ %exp, %entry ], [ %shift, %step ]\n  %b = phi i64 [ %base, %entry ], [ %squared, %step ]\n  %r = phi i64 [ 1, %entry ], [ %newr, %step ]\n  %zero = icmp eq i64 %n, 0\n  br i1 %zero, label %done, label %multiply\nmultiply:\n  %bit = and i64 %n, 1\n  %test = icmp ne i64 %bit, 0\n  %prod = call { i64, i1 } @llvm.smul.with.overflow.i64(i64 %r, i64 %b)\n  %prodval = extractvalue { i64, i1 } %prod, 0\n  %ov1 = extractvalue { i64, i1 } %prod, 1\n  %ovused = and i1 %test, %ov1\n  br i1 %ovused, label %trap, label %step\nstep:\n  %newr = select i1 %test, i64 %prodval, i64 %r\n  %shift = lshr i64 %n, 1\n  %square = call { i64, i1 } @llvm.smul.with.overflow.i64(i64 %b, i64 %b)\n  %squared = extractvalue { i64, i1 } %square, 0\n  %ov2 = extractvalue { i64, i1 } %square, 1\n  %again = icmp ne i64 %shift, 0\n  %bad = and i1 %again, %ov2\n  br i1 %bad, label %trap, label %loop\ndone:\n  ret i64 %r\ntrap:\n  call void @llvm.trap()\n  unreachable\n}\n\n");
    let names=m.functions.iter().map(|f|f.name.clone()).collect::<HashSet<_>>();
    for f in &m.functions {
        let mut e=Emitter::new(names.clone());
        let params=f.args.iter().enumerate().map(|(i,(_,t))|format!("{} %arg{i}",ty(t))).collect::<Vec<_>>().join(", ");
        out.push_str(&format!("define {} @babel_{}({params}) {{\n",ty(&f.ret),f.name));e.block("entry");
        for (i,(name,ty)) in f.args.iter().enumerate(){if name!="_" {e.variables.insert(name.clone(),(format!("%arg{i}"),ty.clone()));}}
        let v=e.gen(&f.body);e.line(format!("ret {} {v}",ty(&f.ret)));out.push_str(&e.code);out.push_str("}\n\n");
    }
    let mut e=Emitter::new(names);e.block("entry");
    for item in &m.items {
        match item {
            Top::Value(name,n)=>{let v=e.gen(n);e.variables.insert(name.clone(),(v,n.ty.clone()));},
            Top::Statement(n)=>{e.gen(n);},
        }
    }
    e.line("ret i32 0");out.push_str("define i32 @main() {\n");out.push_str(&e.code);out.push_str("}\n");out
}

fn module(source:&str)->Result<Module,CompileError>{
    let p=parse(source)?;
    let c=check(p)?;
    if c.function_types.values().any(|sig|
        sig.args.iter().chain(std::iter::once(&sig.ret)).any(|ty| {let mut vars=std::collections::BTreeSet::new();ty.variables(&mut vars);!vars.is_empty()})) {
        return Err(err(1,"LLVM does not support polymorphic functions yet; use the Rust backend"));
    }
    lower(&c)
}
pub fn dump(source:&str)->Result<String,CompileError>{module(source).map(|m|air_text(&m))}
pub fn llvm(source:&str)->Result<String,CompileError>{module(source).map(|m|emit_llvm(&m))}
