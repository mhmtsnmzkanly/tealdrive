use std::{env,fs,path::PathBuf};
fn main(){
    let message=fs::read_to_string("src/protocol/message_type.rs").unwrap();
    let header=fs::read_to_string("src/protocol/header.rs").unwrap();
    let chunk=fs::read_to_string("src/protocol/chunk.rs").unwrap();
    for path in ["src/protocol/message_type.rs","src/protocol/header.rs","src/protocol/chunk.rs","src/protocol/flags.rs"]{println!("cargo:rerun-if-changed={path}");}
    let payload=fs::read_to_string("src/protocol/payload.rs").unwrap();
    println!("cargo:rerun-if-changed=src/protocol/payload.rs");
    let mut messages=Vec::new();
    let mut requests=Vec::new();
    let body=message.split("pub enum MessageType {").nth(1).unwrap().split('}').next().unwrap();
    for line in body.lines(){if let Some((name,value))=line.trim().trim_end_matches(',').split_once(" = "){messages.push(format!("{name}:{value}"));
        let structure=if name=="ClientHello" {"ClientHelloPayload".to_owned()}else{format!("{name}Request")};
        if let Some(body)=payload.split(&format!("pub struct {structure} {{")).nth(1) {
            let body=body.split('}').next().unwrap();
            let fields:Vec<_>=body.lines().filter_map(|line|{let (field,ty)=line.trim().strip_prefix("pub ")?.split_once(": ")?;
                if ty.starts_with("Option<"){None}else{Some(format!("\"{field}\""))}}).collect();
            requests.push(format!("{value}:[{}]",fields.join(",")));
        }}}
    let constants=|source:&str|->Vec<String>{source.lines().filter_map(|line|{
        let line=line.strip_prefix("pub const ")?;let (name,tail)=line.split_once(": usize = ")?;
        let value=tail.trim_end_matches(';');value.parse::<usize>().ok()?;Some(format!("{name}:{value}"))
    }).collect()};
    let encoding=header.split("pub enum Encoding {").nth(1).unwrap().split('}').next().unwrap().lines().filter_map(|line|line.trim().trim_end_matches(',').split_once(" = ").map(|(name,value)|format!("{name}:{value}"))).collect::<Vec<_>>().join(",");
    let code=format!("// Generated from Rust protocol definitions; do not edit.\nconst TdrvContract=Object.freeze({{version:1,messages:Object.freeze({{{}}}),header:Object.freeze({{{}}}),chunk:Object.freeze({{{}}}),encoding:Object.freeze({{{}}}),requests:Object.freeze({{{}}})}});\n",messages.join(","),constants(&header).join(","),constants(&chunk).join(","),encoding,requests.join(","));
    fs::write(PathBuf::from(env::var("OUT_DIR").unwrap()).join("protocol-contract.js"),code).unwrap();
}
