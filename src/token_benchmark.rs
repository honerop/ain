//! Reproducible source-only token counts with o200k_base.
//! This is not a model-generation benchmark or a measure of hidden reasoning.
use super::json_string;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

struct Entry {
    path: String,
    task: String,
    language: &'static str,
    bytes: usize,
    tokens: usize,
}
fn label(p:&Path)->&'static str {
    match p.extension().and_then(|x|x.to_str()) {
        Some("babel")=>"Babel",
        Some("py")=>"Python",
        Some("ts")=>"TypeScript",
        Some("rs")=>"Rust",
        _=>"Other",
    }
}
pub fn run(args:&[String])->Result<(),String> {
    let json=args.iter().any(|arg|arg=="--json");
    let files=args.iter().filter(|arg|arg.as_str()!="--json").collect::<Vec<_>>();
    if files.is_empty() {return Err("usage: babelc tokens FILE... [--json] (with --features token-bench)".into());}
    let enc=tiktoken::get_encoding("o200k_base")
        .ok_or("o200k_base encoding not available in this build")?;
    let mut entries=Vec::new();
    for file in files {
        if file.starts_with('-') {return Err(format!("unknown token benchmark option: {file}"));}
        let contents=fs::read_to_string(file).map_err(|e|format!("{file}: {e}"))?;
        let path=Path::new(file);
        entries.push(Entry{
            path:file.clone(),
            task:path.parent().and_then(|dir|dir.file_name()).and_then(|x|x.to_str()).unwrap_or("other").to_string(),
            language:label(path),
            bytes:contents.len(),
            tokens:enc.count(&contents),
        });
    }
    entries.sort_by(|a,b|a.task.cmp(&b.task).then(a.language.cmp(b.language)));
    let mut grouped=BTreeMap::<String,Vec<&Entry>>::new();
    for entry in &entries {grouped.entry(entry.task.clone()).or_default().push(entry);}
    if json {
        print!("{{\"tokenizer\":\"o200k_base\",\"scope\":\"source_only\",\"files\":[");
        for (i,e) in entries.iter().enumerate() {
            if i>0 {print!(",");}
            print!("{{\"path\":{},\"task\":{},\"language\":{},\"bytes\":{},\"tokens\":{}}}",
                json_string(&e.path),json_string(&e.task),json_string(e.language),e.bytes,e.tokens);
        }
        print!("],\"comparisons\":[");
        let mut first=true;
        for (task,entries) in &grouped {
            if let Some(babel)=entries.iter().find(|e|e.language=="Babel") {
                for other in entries.iter().filter(|e|e.language!="Babel") {
                    if !first {print!(",");}
                    first=false;
                    print!("{{\"task\":{},\"against\":{},\"babel_tokens\":{},\"other_tokens\":{},\"savings_pct\":{:.2}}}",
                        json_string(task),json_string(other.language),babel.tokens,other.tokens,
                        100.0*(1.0-(babel.tokens as f64/other.tokens as f64)));
                }
            }
        }
        println!("]}}");
    } else {
        println!("Tokenizer: o200k_base | source text only, not model API usage");
        println!("{:<13} {:<11} {:>7} {:>7}", "Task","Language","Tokens","Bytes");
        for e in &entries {println!("{:<13} {:<11} {:>7} {:>7}",e.task,e.language,e.tokens,e.bytes);}
        println!("\nAin source token savings (negative means Babel uses more):");
        for (task,group) in grouped {
            if let Some(babel)=group.iter().find(|e|e.language=="Babel") {
                for other in group.iter().filter(|e|e.language!="Babel") {
                    println!("  {:<12} vs {:<11}: {:+.1}%",task,other.language,
                        100.0*(1.0-babel.tokens as f64/other.tokens as f64));
                }
            }
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    #[test] fn real_tokenizer_known_example(){
        let enc=tiktoken::get_encoding("o200k_base").unwrap();
        assert_eq!(enc.count("hello world"),2);
        let verbose=include_str!("../benchmarks/experiments/lists-original.babel");
        let compact=include_str!("../benchmarks/tokens/lists/babel.babel");
        assert!(enc.count(compact)<enc.count(verbose),"token-minimal list form should be smaller");
        assert_eq!(enc.count(verbose),33);
        assert_eq!(enc.count(compact),23);
    }
}
