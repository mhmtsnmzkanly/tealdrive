# TealDrive V1 Binary WebSocket Protocol Specification

This document specifies the wire format, message types, serialization rules, and connection lifecycle of the **TealDrive V1 Binary Protocol** used over WebSocket connections.

---

## 1. Protocol Architecture & Design Goals

The TealDrive V1 Protocol is a framed, bidirectional, multiplexed binary protocol designed for:
- **Low Overhead:** Direct binary streaming of file chunks without Base64 encoding overhead.
- **Multiplexing:** Correlation of asynchronous requests and responses using 16-byte UUID `RequestId`s.
- **Strict Typing:** MessagePack structured payloads for control messages and raw binary streams for file transfers.
- **Fail-Fast Validation:** Bounded payload length enforcement before deserialization to prevent memory exhaustion.

---

## 2. Frame Structure (36-Byte Header)

Every frame transmitted over the WebSocket connection consists of a fixed **32-byte header**, followed by a **4-byte payload length**, followed by the payload bytes:

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                       Magic: "TDRV"                           |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
| Version (0x01)|       Message Type (u16 Big-Endian)           |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
+                       Request ID (Bytes 0..7)                 +
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
+                       Request ID (Bytes 8..15)                +
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|     Flags     |   Encoding    |  Compression  |   Reserved    |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                       Reserved (5 bytes)                      |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                  Payload Length (u32 Big-Endian)              |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                       Payload Body ...                        |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

### Field Definitions

| Field | Offset | Size | Type | Description |
| :--- | :--- | :--- | :--- | :--- |
| **Magic** | 0 | 4 bytes | `[u8; 4]` | Protocol identifier: ASCII `b"TDRV"` (`0x54, 0x44, 0x52, 0x56`). |
| **Version** | 4 | 1 byte | `u8` | Protocol version (`0x01` for V1). |
| **MessageType**| 5 | 2 bytes | `u16` (BE) | Identifies the operation or response type. |
| **RequestId** | 7 | 16 bytes| `[u8; 16]` | UUIDv4 uniquely identifying request/response stream. |
| **Flags** | 23 | 1 byte | `u8` | Bit flags: `0x01` = Compressed, `0x02` = Error. |
| **Encoding** | 24 | 1 byte | `u8` | `0x00` = MessagePack, `0x01` = RawBinary. |
| **Compression**| 25 | 1 byte | `u8` | `0x00` = None, `0x01` = Gzip. |
| **Reserved** | 26 | 6 bytes | `[u8; 6]` | Reserved for future expansion (must be `0x00`). |
| **PayloadLength**| 32 | 4 bytes | `u32` (BE) | Number of bytes in payload body. |
| **Payload** | 36 | N bytes | bytes | Body encoded per the `Encoding` field. |

---

## 3. Connection Lifecycle & Handshake

WebSocket connections require a strict 3-way handshake prior to accepting filesystem operations:

```
Client                                                  Server
  |                                                       |
  | ------------ 1. HTTP Upgrade GET /ws (Cookie) ------> |
  | <----------- 2. 101 Switching Protocols ------------- |
  |                                                       |
  | <----------- 3. ServerHello (0x0100) ---------------- |
  |                                                       |
  | ------------ 4. ClientHello (0x0101) ---------------> |
  |                                                       |
  | <----------- 5. ProtocolReady (0x0102) -------------- |
  |                                                       |
  | === CONNECTION READY: Filesystem Operations Begin === |
```

1. **HTTP Upgrade:** The client connects to `GET /ws` with a valid `tdrv_session` authentication cookie.
2. **ServerHello (`0x0100`):** The server sends its supported encodings, compressions, feature flags, and limits.
3. **ClientHello (`0x0101`):** The client acknowledges and requests protocol parameters within server limits.
4. **ProtocolReady (`0x0102`):** The server confirms negotiated options. The connection transitions to `Ready`.
5. **Enforcement:** Any filesystem message sent by a client prior to receiving `ProtocolReady` is rejected immediately with an error frame and the connection is closed.

---

## 4. Message Types Reference

### 4.1 Handshake Messages (`0x01xx`)
| Code | Name | Direction | Payload Encoding | Description |
| :--- | :--- | :--- | :--- | :--- |
| `0x0100` | `ServerHello` | S -> C | MessagePack | Initial server parameters and limits |
| `0x0101` | `ClientHello` | C -> S | MessagePack | Client parameter acknowledgment |
| `0x0102` | `ProtocolReady` | S -> C | MessagePack | Handshake completion confirmation |
| `0x0103` | `ServerShutdown` | S -> C | MessagePack | Graceful shutdown notification |

### 4.2 Directory & Metadata Messages (`0x02xx`)
| Code | Name | Direction | Payload Encoding | Description |
| :--- | :--- | :--- | :--- | :--- |
| `0x0201` | `ListDirectory` | C -> S | MessagePack | Paginated directory entry query |
| `0x0202` | `DirectoryList` | S -> C | MessagePack | Directory entries response |
| `0x0203` | `FileMetadata` | C -> S | MessagePack | Single file stat/metadata query |
| `0x0204` | `FileMetadataResponse` | S -> C | MessagePack | Single file stat response |
| `0x0205` | `ReadTextFile` | C -> S | MessagePack | Request UTF-8 text file content |
| `0x0206` | `TextFileContent` | S -> C | MessagePack | UTF-8 text file content response |
| `0x0207` | `SaveTextFile` | C -> S | MessagePack | Save UTF-8 text file content |
| `0x0208` | `CreateFolder` | C -> S | MessagePack | Create directory |
| `0x0209` | `RenameFile` | C -> S | MessagePack | Rename file or folder |
| `0x020A` | `MoveFile` | C -> S | MessagePack | Move file or folder |
| `0x020B` | `CopyFile` | C -> S | MessagePack | Copy file or folder |

### 4.3 Trash & Maintenance Operations (`0x03xx`)
| Code | Name | Direction | Payload Encoding | Description |
| :--- | :--- | :--- | :--- | :--- |
| `0x0301` | `MoveToTrash` | C -> S | MessagePack | Move item to user-isolated trash |
| `0x0302` | `RestoreFromTrash` | C -> S | MessagePack | Restore item from trash |
| `0x0304` | `ListTrash` | C -> S | MessagePack | List caller's trash entries |
| `0x0305` | `TrashList` | S -> C | MessagePack | Caller's trash entries response |
| `0x0306` | `DeletePermanently` | C -> S | MessagePack | Permanently remove item from trash |
| `0x0307` | `ChmodFile` | C -> S | MessagePack | Change POSIX mode (masked to `0o0777`) |
| `0x0308` | `CompressZip` | C -> S | MessagePack | Compress items into Zip archive |
| `0x0309` | `CompressTarGz` | C -> S | MessagePack | Compress items into Tar.Gz archive |
| `0x030A` | `ExtractArchive` | C -> S | MessagePack | Extract Zip/Tar archive (500MB limit) |
| `0x030B` | `SearchFiles` | C -> S | MessagePack | Recursive file search within root |
| `0x030C` | `SearchResults` | S -> C | MessagePack | Recursive search matches response |

### 4.4 Chunked File Transfers (`0x04xx`)
| Code | Name | Direction | Payload Encoding | Description |
| :--- | :--- | :--- | :--- | :--- |
| `0x0401` | `UploadBegin` | C -> S | MessagePack | Initiate staged chunked upload |
| `0x0402` | `UploadChunk` | C -> S | RawBinary | Transfer file binary chunk |
| `0x0403` | `UploadEnd` | C -> S | MessagePack | Finalize upload and commit file |
| `0x0404` | `UploadAbort` | C -> S | MessagePack | Abort staged upload and clean temp |
| `0x0405` | `DownloadBegin` | C -> S | MessagePack | Initiate WebSocket chunked download |
| `0x0406` | `DownloadChunk` | S -> C | RawBinary | Streaming file binary chunk |
| `0x0407` | `DownloadEnd` | S -> C | MessagePack | WebSocket download completion |
| `0x0408` | `DownloadAbort` | C -> S | MessagePack | Abort WebSocket download |

### 4.5 General Responses & Errors (`0x05xx`)
| Code | Name | Direction | Payload Encoding | Description |
| :--- | :--- | :--- | :--- | :--- |
| `0x0501` | `OperationDone` | S -> C | MessagePack | Generic success acknowledgment |
| `0x0502` | `Error` | S -> C | MessagePack | Structured error details |

---

## 5. Payloads Specification

All structured payloads are serialized using MessagePack (`rmp-serde` named format).

### 5.1 ServerHello Payload (`0x0100`)
```json
{
  "protocol_version": 1,
  "server_name": "TealDrive",
  "selected_encoding": "MessagePack",
  "supported_encodings": ["MessagePack", "RawBinary"],
  "supported_compressions": ["None"],
  "max_frame_size": 1048576,
  "max_control_payload_size": 262144,
  "max_chunk_size": 262144,
  "feature_flags": {
    "save_text_file": true,
    "create_folder": true,
    "rename": true,
    "move_file": true,
    "copy_file": true,
    "delete_permanently": true,
    "chown": false
  }
}
```

### 5.2 ListDirectory Payload (`0x0201` / `0x0202`)
**Request (`ListDirectoryRequest`):**
```json
{
  "root_id": "storage",
  "path": "documents",
  "page": 1,
  "page_size": 100
}
```
**Response (`DirectoryListPayload`):**
```json
{
  "root_id": "storage",
  "path": "documents",
  "total_items": 2,
  "page": 1,
  "page_size": 100,
  "items": [
    {
      "name": "report.pdf",
      "path": "documents/report.pdf",
      "is_dir": false,
      "size": 145020,
      "modified_epoch": 1728640000,
      "mode": 420,
      "is_symlink": false
    },
    {
      "name": "work",
      "path": "documents/work",
      "is_dir": true,
      "size": 4096,
      "modified_epoch": 1728641200,
      "mode": 493,
      "is_symlink": false
    }
  ]
}
```

### 5.3 Upload Lifecycle Payloads (`0x0401` .. `0x0403`)
**1. UploadBegin (`UploadBeginRequest`):**
```json
{
  "upload_id": "c138d672-88f5-46aa-b2b9-69634e062ee5",
  "root_id": "storage",
  "relative_path": "uploads/dataset.csv",
  "total_bytes": 10485760,
  "chunk_size": 262144
}
```
**2. UploadChunk (`0x0402`):**
- Header `Encoding` = `RawBinary`.
- Payload body contains raw byte chunk (up to `max_chunk_size`).

**3. UploadEnd (`UploadEndRequest`):**
```json
{
  "upload_id": "c138d672-88f5-46aa-b2b9-69634e062ee5",
  "sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
}
```
**Response:** `0x0501 OperationDone`.

---

## 6. Error Handling (`0x0502 Error`)

When any operation encounters a policy denial, path validation failure, or system error, the server responds with an `Error` frame (`0x0502`):

```json
{
  "error_kind": "PolicyDenied",
  "message": "Modification of SUID/SGID bits is denied.",
  "reason": "policy_denied",
  "safe_message": "Operation denied by server policy."
}
```

### Standard Error Kinds
- `NotFound`: Target file or directory does not exist.
- `AlreadyExists`: Target destination already exists.
- `PermissionDenied`: Insufficient POSIX permissions or trash ownership mismatch.
- `ReadOnlyRoot`: Root is configured with `read_only = true`.
- `PathEscapesRoot`: Requested path canonicalizes outside root boundary.
- `SymlinkEscape`: Symlink resolution points outside allowed root.
- `PolicyDenied`: Operation prohibited by security policy (e.g. SUID bits, sensitive file list).
- `QuotaExceeded`: Operation would exceed storage quota limit.
- `FileTooLarge`: File or archive decompression exceeds size bounds.
- `InvalidTarget`: Target name or directory structure is invalid.
- `FeatureDisabled`: Requested operation is disabled in `features` configuration.
