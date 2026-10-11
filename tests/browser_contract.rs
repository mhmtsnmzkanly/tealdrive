//! Exercise the shipped browser codec against actual Rust frames and payload types.
use tealdrive::protocol::{
    chunk::RawChunk,
    codec::{decode_msgpack, encode_msgpack},
    frame::{RequestId, TdrvFrame, TransferId},
    header::TdrvHeader,
    message_type::MessageType,
    payload::*,
};
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn bytes(hex: &str) -> Vec<u8> {
    hex.as_bytes()
        .chunks_exact(2)
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}
#[test]
fn rust_and_browser_share_handshake_requests_and_raw_chunks() {
    let config = tealdrive::config::AppConfig::default();
    let hello = tealdrive::ws::handshake::create_server_hello_frame(&config)
        .unwrap()
        .encode()
        .unwrap();
    let hello_json = serde_json::to_string(&ClientHelloPayload {
        protocol_version: 1,
        supported_encodings: vec![
            tealdrive::protocol::header::Encoding::MessagePack,
            tealdrive::protocol::header::Encoding::RawBinary,
        ],
        supported_compressions: vec![tealdrive::protocol::compression::Compression::None],
        max_frame_size: config.limits.max_frame_payload,
        max_control_payload_size: config.limits.max_control_payload,
        max_chunk_size: config.limits.max_chunk_size,
        feature_flags: config.features.clone(),
    })
    .unwrap();
    let upload = UploadBeginResponse {
        transfer_id: TransferId::new(),
        chunk_size: 1024,
    };
    let upload_frame = TdrvFrame::new(
        TdrvHeader::new(MessageType::OperationStarted, RequestId::new(), 0),
        encode_msgpack(&upload).unwrap(),
    )
    .unwrap()
    .encode()
    .unwrap();
    let script = format!(
        r#"{}
{}
new (require('node:vm').Script)({});
const assert=require('node:assert/strict');const w=TealDriveWire;const mt=TdrvContract.messages;
const id='00112233-4455-4677-8899-aabbccddeeff';
const hello=w.decodeFrame(Uint8Array.from(Buffer.from('{}','hex')));assert.equal(hello.messageType,mt.ServerHello);assert.equal(hello.payload.protocol_version,1);
const upload=w.decodeFrame(Uint8Array.from(Buffer.from('{}','hex')));assert.equal(w.uuidKey(upload.payload.transfer_id),w.uuidKey('{}'));assert.equal(upload.payload.chunk_size,1024);
const binary=new Uint8Array(70000);const encoded=w.MsgPack.encode({{binary,large:4294967296,text:'x'.repeat(70000)}});const decoded=w.MsgPack.decode(encoded);assert.equal(decoded.binary.length,70000);assert.equal(decoded.large,4294967296);assert.equal(decoded.text.length,70000);
assert.throws(()=>w.decodeFrame(new Uint8Array(16)));
const frames=[
 w.encodeFrame(mt.ClientHello,{},id),
 w.encodeFrame(mt.ListDirectory,{{root_id:'home',relative_path:'docs',cursor:null,limit:200,include_hidden:false,sort:'NameAsc',filter:{{query:null,file_kind:null,filter:'All'}}}},id),
 w.encodeFrame(mt.UploadChunk,w.encodeChunk(id,7,4294967296,new Uint8Array([1,2,3])),id,true),
 w.encodeFrame(mt.CreateFolder,{{root_id:'home',parent_relative_path:'docs',folder_name:'new'}},id)
];process.stdout.write(frames.map(f=>Buffer.from(f).toString('hex')).join('\n'));
"#,
        tealdrive::protocol::JAVASCRIPT_CONTRACT,
        include_str!("../src/ui/protocol.js"),
        serde_json::to_string(tealdrive::ui::assets::APP_JS).unwrap(),
        hex(&hello),
        hex(&upload_frame),
        upload.transfer_id.0,
        hello_json
    );
    let mut child = std::process::Command::new("node")
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("Node.js is required for browser contract tests");
    use std::io::Write;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(script.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let frames: Vec<_> = stdout
        .lines()
        .map(|line| TdrvFrame::decode(&bytes(line)).unwrap())
        .collect();
    let hello: ClientHelloPayload = decode_msgpack(&frames[0].payload).unwrap();
    tealdrive::ws::handshake::validate_client_hello(&hello, &config.limits).unwrap();
    let list: ListDirectoryRequest = decode_msgpack(&frames[1].payload).unwrap();
    assert_eq!(list.relative_path.0, "docs");
    list.validate(&config.limits).unwrap();
    let chunk = RawChunk::decode(&frames[2].payload).unwrap();
    assert_eq!(chunk.offset, 4294967296);
    assert_eq!(chunk.chunk_bytes, [1, 2, 3]);
    let folder: CreateFolderRequest = decode_msgpack(&frames[3].payload).unwrap();
    assert_eq!(folder.folder_name, "new");
}
