# TDRV v1 binary protocol

One WebSocket binary message contains one TDRV frame. Text WebSocket messages
are rejected. The header is **32 bytes**, with big-endian numeric fields:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 4 | ASCII `TDRV` |
| 4 | 1 | Version = 1 |
| 5 | 2 | Flags |
| 7 | 2 | Message type |
| 9 | 1 | Compression: None=0, Gzip=1 |
| 10 | 1 | Encoding: None=0, MessagePack=1, RawBinary=2 |
| 11 | 1 | Reserved = 0 |
| 12 | 16 | Request UUID |
| 28 | 4 | Payload length |
| 32 | length | Payload |

Flags: error `0x0001`, stream `0x0002`, final `0x0004`, ack required `0x0008`,
compressed `0x0010`. Unknown bits and inconsistent compression/encoding are rejected.
Deflate/Zstd values are reserved, not supported. The shipped browser negotiates
MessagePack + RawBinary with no compression. Rust control decoding supports bounded
Gzip; the browser codec does not decode compressed server frames.

## Handshake and correlation

Server sends ServerHello after authenticated upgrade. Browser replies ClientHello
with protocol_version, supported_encodings, supported_compressions, max_frame_size,
max_control_payload_size, max_chunk_size and feature_flags. Server returns ProtocolReady
with negotiated limits. Send operations only after ProtocolReady. Enum values inside
MessagePack payloads use their Serde names, e.g. `NameAsc`, `Directory`, `MessagePack`.

Response header request UUID correlates with the client request. Do not treat
OperationStarted as completion for long-running operations. UploadBegin returns
an OperationStarted payload carrying the server transfer UUID and selected chunk
size; each chunk is acknowledged before the browser sends the next one. UploadEnd
or Abort closes the transfer. UI success requires the server response.

## Message types

| Name | Value |
|---|---:|
| ServerHello | `0x0001` |
| ClientHello | `0x0002` |
| ProtocolReady | `0x0003` |
| ConnectionStatus | `0x0004` |
| Error | `0x0005` |
| ListDirectory | `0x0100` |
| DirectoryList | `0x0101` |
| FileMetadata | `0x0102` |
| ReadTextFile | `0x0103` |
| TextFileContent | `0x0104` |
| SaveTextFile | `0x0105` |
| FileMetadataRequest | `0x0106` |
| SearchFiles | `0x0107` |
| SearchResults | `0x0108` |
| ListTrash | `0x0109` |
| TrashList | `0x010A` |
| UploadBegin | `0x0200` |
| UploadChunk | `0x0201` |
| UploadEnd | `0x0202` |
| UploadAbort | `0x0203` |
| DownloadBegin | `0x0210` |
| DownloadChunk | `0x0211` |
| DownloadEnd | `0x0212` |
| CreateFolder | `0x0300` |
| RenameFile | `0x0301` |
| MoveFile | `0x0302` |
| CopyFile | `0x0303` |
| MoveToTrash | `0x0304` |
| RestoreFromTrash | `0x0305` |
| DeletePermanently | `0x0306` |
| ChmodFile | `0x0307` |
| CompressZip | `0x0310` |
| CompressTarGz | `0x0311` |
| ExtractArchive | `0x0312` |
| OperationStarted | `0x0400` |
| OperationProgress | `0x0401` |
| OperationDone | `0x0402` |
| OperationFailed | `0x0403` |
| PermissionDenied | `0x0404` |
| PolicyDenied | `0x0405` |
| ValidationError | `0x0406` |

## Raw chunks and payloads

UploadChunk/DownloadChunk use a 32-byte subheader, followed by file bytes:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 16 | Transfer UUID |
| 16 | 4 | Chunk index, u32 BE |
| 20 | 8 | File offset, u64 BE |
| 28 | 4 | Chunk byte count, u32 BE |
| 32 | count | Raw data |

JavaScript uses BigInt while writing 64-bit offsets and rejects values outside the
safe integer range when decoding general numbers. UUID payloads may deserialize
as binary byte arrays; use the shipped uuid helpers rather than assuming strings.

Examples of control requests:

```json
{"root_id":"home","relative_path":"docs","cursor":null,"limit":200,
 "include_hidden":false,"sort":"NameAsc",
 "filter":{"query":null,"file_kind":null,"filter":"All"}}
```

```json
{"root_id":"home","parent_relative_path":"docs","folder_name":"new"}
```

DirectoryList returns entries, next_cursor, has_more, total_estimate, current_path
and capabilities. Entry type is `file_type`, not `is_dir`. Rename uses `new_name`.
UploadBegin requires root_id, relative_path, filename, total_size, chunk_size and
optional sha256. UploadEnd carries transfer_id, total_size and optional sha256;
a hash supplied at Begin remains binding even when End omits it.

The authoritative payload structures are in `src/protocol/payload.rs`; limits and
validators are in `src/limits.rs` and the protocol modules. `build.rs` generates the
browser's header constants, codes and required request fields from those Rust
sources. `tests/browser_contract.rs` runs the actual browser codec with Node against
Rust handshake, directory, create-folder, UUID and >4 GiB raw offset fixtures.
Changes to the Rust contracts must pass this test before deployment.
