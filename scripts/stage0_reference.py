#!/usr/bin/env python3
"""Temporary independent stage-zero BABEL subset -> C compiler for offline verification.
Production bootstrap can use the Rust-based \`babelc transpile\` instead.
This reference compiler is *not* the self-hosted compiler.
"""
import argparse
import re
from pathlib import Path

ARITY={}
for names,n in [('+ - * / % == != < <= > >= and or min max pow concat contains starts ends before after',2),
                ('if replace clamp',3),
                ('not neg abs print len upper lower trim str word skip num isnum input',1)]:
    ARITY.update({w:n for w in names.split()})
MAPPING=dict(zip('+ - * / % == != < <= > >= and or'.split(),
                 'v_add v_sub v_mul v_div v_mod v_eq v_ne v_lt v_le v_gt v_ge v_and v_or'.split()))
MAPPING.update({'str':'v_str_value'})


def lex(source: str) -> list[str]:
    result=[]; i=0
    while i<len(source):
        if source[i].isspace(): i+=1; continue
        if source[i]=='#': break
        if source[i]=='"':
            j=i+1; escaped=False
            while j<len(source):
                ch=source[j]
                if escaped: escaped=False
                elif ch=='\\': escaped=True
                elif ch=='"': j+=1;break
                j+=1
            else: raise ValueError('unterminated string literal')
            result.append(source[i:j]);i=j
        else:
            m=re.match(r'[^\s#"]+',source[i:]);
            if m is None: raise ValueError(f'invalid token at {i}: {source[i:]}')
            result.append(m.group());i+=len(m.group())
    return result


def compile_ain(source:str)->str:
    lines=[lex(line) for line in source.splitlines()]
    lines=[a for a in lines if a]
    names={}; all_names=set()
    for line in lines:
        if '=' in line:
            eq=line.index('='); name=line[0]
            if name in all_names or name in ARITY:raise ValueError(f'duplicate or builtin def: {name}')
            all_names.add(name)
            if eq>1: names[name]=eq-1
    arities=ARITY|names
    def expr(tokens,pos=0):
        if pos>=len(tokens):raise ValueError('missing operand')
        t=tokens[pos];pos+=1
        if t=='_':return 'v_unit()',pos
        if t in ('true','false'):return f'v_bool({int(t=="true")})',pos
        if t.startswith('"'):return f'v_str({t})',pos
        if re.fullmatch(r'-?\d+',t):return f'v_int({t})',pos
        if t in arities:
            args=[]
            for _ in range(arities[t]):
                e,pos=expr(tokens,pos);args.append(e)
            name=MAPPING.get(t,('babel_' if t in names else 'v_')+t)
            return f'{name}({",".join(args)})',pos
        return f'babel_{t}',pos
    def full(tokens):
        e,p=expr(tokens)
        if p!=len(tokens):raise ValueError(f'extra tokens: {tokens[p:]}')
        return e
    declarations=[]; defs=[];main=[]
    for line in lines:
        if '=' not in line:main.append(f'  (void){full(line)};');continue
        eq=line.index('='); name=line[0];params=line[1:eq];body=line[eq+1:]
        if params:
            args=','.join('V '+('babel_'+p if p!='_' else f'_unit_{i}') for i,p in enumerate(params))
            declarations.append(f'V babel_{name}({args});')
            defs.append(f'V babel_{name}({args}){{return {full(body)};}}')
        else:
            main.append(f'  V babel_{name}={full(body)};')
    return '\n'.join(['#include "babel_runtime.h"',*declarations,*defs,'int main(void){',*main,'return 0;}',''])

if __name__=='__main__':
    ap=argparse.ArgumentParser();ap.add_argument('source',type=Path);ap.add_argument('-o',type=Path,default=None)
    a=ap.parse_args();r=compile_ain(a.source.read_text())
    if a.o:a.o.write_text(r)
    else:print(r,end='')

# An intentionally separate static analysis pass mirroring the primitive
# monomorphic inference rules in the v0.2 Rust stage zero. Helps offline audits.
def verify_primitive_types(source: str) -> dict[str, tuple]:
    lines=[lex(line) for line in source.splitlines()]
    lines=[l for l in lines if l]
    signatures={}; all_names=set()
    for line in lines:
        if '=' in line:
            eq=line.index('='); name=line[0]
            if name in all_names or name in ARITY:
                raise ValueError(f'duplicate/reserved: {name}')
            all_names.add(name)
            if eq>1: signatures[name]=line[1:eq]
    arities=ARITY|{n:len(p) for n,p in signatures.items()}
    def parse(ts,pos=0):
        if pos>=len(ts):raise ValueError('missing operand')
        t=ts[pos];pos+=1
        if t in arities:
            args=[]
            for _ in range(arities[t]):
                a,pos=parse(ts,pos);args.append(a)
            return (t,args),pos
        return t,pos
    def body(ts):
        v,p=parse(ts)
        if p!=len(ts):raise ValueError(f'extra tokens: {ts[p:]}')
        return v
    fn={}
    vals=[]
    for line in lines:
        if '=' in line:
            eq=line.index('=');n=line[0];params=line[1:eq];b=body(line[eq+1:])
            if params:fn[n]=(params,b)
            else:vals.append((n,b))
        else:vals.append((None,body(line)))
    sub={};counter=[0]
    def fresh():
        i=counter[0];counter[0]+=1;return ('var',i)
    def resolve(a):
        while isinstance(a,tuple) and a[0]=='var' and a in sub:
            a=sub[a]
        return a
    def unify(a,b):
        a=resolve(a);b=resolve(b)
        if a==b:return
        if isinstance(a,tuple) and a[0]=='var':sub[a]=b;return
        if isinstance(b,tuple) and b[0]=='var':sub[b]=a;return
        raise TypeError(f'{a} != {b}')
    sigs={n:([('unit' if p=='_' else fresh()) for p in params], fresh()) for n,(params,_) in fn.items()}
    def infer(e,env):
        if isinstance(e,str):
            if e=='_':return 'unit'
            if e in ('true','false'):return 'bool'
            if e.startswith('"'):return 'str'
            if re.fullmatch(r'-?\d+',e):return 'int'
            if e not in env:raise TypeError(f'unknown name: {e}')
            return env[e]
        name,inputs=e
        actual=[infer(x,env) for x in inputs]
        if name in sigs:
            pars,ret=sigs[name]
            for a,b in zip(actual,pars):unify(a,b)
            return ret
        if name in '+ - * / % min max pow neg abs clamp'.split():
            for x in actual:unify(x,'int')
            return 'int'
        if name in '< <= > >= '.split():
            for x in actual:unify(x,'int')
            return 'bool'
        if name in ('==','!='):
            unify(actual[0],actual[1]);return 'bool'
        if name in ('and','or','not'):
            for x in actual:unify(x,'bool')
            return 'bool'
        if name=='if':
            unify(actual[0],'bool');unify(actual[1],actual[2]);return actual[1]
        if name=='print':return 'unit'
        if name=='input':unify(actual[0],'unit');return 'str'
        if name=='num':unify(actual[0],'str');return 'int'
        if name=='isnum':unify(actual[0],'str');return 'bool'
        if name=='str':return 'str'
        if name in ('concat','replace','upper','lower','trim','before','after','word','skip'):
            for x in actual:unify(x,'str')
            return 'str'
        if name=='len':unify(actual[0],'str');return 'int'
        if name in ('contains','starts','ends'):
            for x in actual:unify(x,'str')
            return 'bool'
        raise TypeError(f'unknown builtin: {name}')
    for n,(params,b) in fn.items():
        env={p:t for p,t in zip(params,sigs[n][0]) if p!='_'}
        unify(infer(b,env),sigs[n][1])
    env={}
    for n,b in vals:
        v=infer(b,env)
        if n is not None:env[n]=v
    for n,(params,_) in fn.items():
        args,ret=sigs[n]
        if any(isinstance(resolve(x),tuple) for x in args+[ret]):
            raise TypeError(f'underconstrained function: {n}: {list(map(resolve,args)),resolve(ret)}')
    return {n:(tuple(map(resolve,pars)),resolve(ret)) for n,(pars,ret) in sigs.items()}
