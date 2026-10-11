(function(){
"use strict";

  const MsgPack = {
    encode(val) {
      const parts = [];
      function push(u8) { parts.push(u8); }
      function pushBytes(arr) { parts.push(new Uint8Array(arr)); }
      function enc(v) {
        if (v === null || v === undefined) {
          push(new Uint8Array([0xc0]));
        } else if (typeof v === 'boolean') {
          push(new Uint8Array([v ? 0xc3 : 0xc2]));
        } else if (typeof v === 'number') {
          if (Number.isInteger(v)) {
            if (v >= 0 && v <= 0x7f) {
              push(new Uint8Array([v]));
            } else if (v >= -32 && v < 0) {
              push(new Uint8Array([v & 0xff]));
            } else if (v >= 0 && v <= 0xffff) {
              push(new Uint8Array([0xcd, (v >> 8) & 0xff, v & 0xff]));
            } else if (v >= 0 && v <= 0xffffffff) {
              const b = new Uint8Array(5);
              b[0] = 0xce;
              new DataView(b.buffer).setUint32(1, v, false);
              push(b);
            } else if (v > 0xffffffff || v < -0x80000000) {
              if (!Number.isSafeInteger(v)) throw new Error('Unsafe integer');
              const b = new Uint8Array(9); b[0]=v>=0?0xcf:0xd3;
              if(v>=0)new DataView(b.buffer).setBigUint64(1,BigInt(v),false);
              else new DataView(b.buffer).setBigInt64(1,BigInt(v),false);
              push(b);
            } else {
              const b = new Uint8Array(5);
              b[0] = 0xd2;
              new DataView(b.buffer).setInt32(1, v, false);
              push(b);
            }
          } else {
            const b = new Uint8Array(9);
            b[0] = 0xcb;
            new DataView(b.buffer).setFloat64(1, v, false);
            push(b);
          }
        } else if (typeof v === 'string') {
          const strBytes = new TextEncoder().encode(v);
          const len = strBytes.length;
          if (len <= 31) {
            push(new Uint8Array([0xa0 | len]));
          } else if (len <= 0xff) {
            push(new Uint8Array([0xd9, len]));
          } else if (len <= 0xffff) {
            push(new Uint8Array([0xda, (len >> 8) & 0xff, len & 0xff]));
          } else {
            const b = new Uint8Array(5);
            b[0] = 0xdb;
            new DataView(b.buffer).setUint32(1, len, false);
            push(b);
          }
          push(strBytes);
        } else if (v instanceof Uint8Array) {
          const len = v.length;
          if (len <= 0xff) {
            push(new Uint8Array([0xc4, len]));
          } else if (len <= 0xffff) {
            push(new Uint8Array([0xc5, (len >> 8) & 0xff, len & 0xff]));
          } else {
            const b = new Uint8Array(5);
            b[0] = 0xc6;
            new DataView(b.buffer).setUint32(1, len, false);
            push(b);
          }
          push(v);
        } else if (Array.isArray(v)) {
          const len = v.length;
          if (len <= 15) {
            push(new Uint8Array([0x90 | len]));
          } else if (len <= 0xffff) {
            push(new Uint8Array([0xdc, (len >> 8) & 0xff, len & 0xff]));
          } else {
            const b = new Uint8Array(5);
            b[0] = 0xdd;
            new DataView(b.buffer).setUint32(1, len, false);
            push(b);
          }
          for (let i = 0; i < len; i++) enc(v[i]);
        } else if (typeof v === 'object') {
          const keys = Object.keys(v);
          const len = keys.length;
          if (len <= 15) {
            push(new Uint8Array([0x80 | len]));
          } else if (len <= 0xffff) {
            push(new Uint8Array([0xde, (len >> 8) & 0xff, len & 0xff]));
          } else {
            const b = new Uint8Array(5);
            b[0] = 0xdf;
            new DataView(b.buffer).setUint32(1, len, false);
            push(b);
          }
          for (const k of keys) {
            enc(k);
            enc(v[k]);
          }
        }
      }
      enc(val);
      const totalLen = parts.reduce((acc, p) => acc + p.length, 0);
      const res = new Uint8Array(totalLen);
      let offset = 0;
      for (const p of parts) {
        res.set(p, offset);
        offset += p.length;
      }
      return res;
    },

    decode(bytes) {
      const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
      let pos = 0;
      function read() {
        if (pos >= bytes.length) throw new Error("Truncated MessagePack");
        const b = bytes[pos++];
        if (b <= 0x7f) return b;
        if (b >= 0xe0) return b - 0x100;
        if ((b & 0xe0) === 0xa0) {
          const len = b & 0x1f;
          const s = new TextDecoder().decode(bytes.subarray(pos, pos + len));
          pos += len;
          return s;
        }
        if ((b & 0xf0) === 0x90) {
          const len = b & 0x0f;
          const arr = [];
          for (let i = 0; i < len; i++) arr.push(read());
          return arr;
        }
        if ((b & 0xf0) === 0x80) {
          const len = b & 0x0f;
          const obj = Object.create(null);
          for (let i = 0; i < len; i++) {
            const k = read();
            obj[k] = read();
          }
          return obj;
        }
        if (b === 0xc0) return null;
        if (b === 0xc2) return false;
        if (b === 0xc3) return true;
        if (b === 0xc4) {
          const len = bytes[pos++];
          const bin = bytes.subarray(pos, pos + len);
          pos += len;
          return bin;
        }
        if (b === 0xc5) {
          const len = view.getUint16(pos, false);
          pos += 2;
          const bin = bytes.subarray(pos, pos + len);
          pos += len;
          return bin;
        }
        if (b === 0xcf || b === 0xd3) { const n=b===0xcf?view.getBigUint64(pos,false):view.getBigInt64(pos,false);pos+=8;
          const value=Number(n);if(!Number.isSafeInteger(value))throw new Error('Unsafe integer');return value; }
        if (b === 0xcb) {const n=view.getFloat64(pos,false);pos+=8;return n;}
        if (b === 0xc6 || b === 0xdb) {const len=view.getUint32(pos,false);pos+=4;if(len>bytes.length-pos)throw new Error('Truncated value');const data=bytes.subarray(pos,pos+len);pos+=len;return b===0xdb?new TextDecoder().decode(data):data;}
        if (b === 0xcc) return bytes[pos++];
        if (b === 0xcd) {
          const v = view.getUint16(pos, false);
          pos += 2;
          return v;
        }
        if (b === 0xce) {
          const v = view.getUint32(pos, false);
          pos += 4;
          return v;
        }
        if (b === 0xd0) return view.getInt8(pos++);
        if (b === 0xd1) {
          const v = view.getInt16(pos, false);
          pos += 2;
          return v;
        }
        if (b === 0xd2) {
          const v = view.getInt32(pos, false);
          pos += 4;
          return v;
        }
        if (b === 0xd9) {
          const len = bytes[pos++];
          const s = new TextDecoder().decode(bytes.subarray(pos, pos + len));
          pos += len;
          return s;
        }
        if (b === 0xda) {
          const len = view.getUint16(pos, false);
          pos += 2;
          const s = new TextDecoder().decode(bytes.subarray(pos, pos + len));
          pos += len;
          return s;
        }
        if (b === 0xdc) {
          const len = view.getUint16(pos, false);
          pos += 2;
          const arr = [];
          for (let i = 0; i < len; i++) arr.push(read());
          return arr;
        }
        if (b === 0xde) {
          const len = view.getUint16(pos, false);
          pos += 2;
          const obj = Object.create(null);
          for (let i = 0; i < len; i++) {
            const k = read();
            obj[k] = read();
          }
          return obj;
        }
        throw new Error("Unsupported MessagePack type: "+b);
      }
      const value=read();if(pos!==bytes.length)throw new Error("Invalid MessagePack length");return value;
    }
  };



function uuidBytes(value){
  if(value instanceof Uint8Array && value.length===16)return value;
  if(typeof value==='string'){const hex=value.replaceAll('-','');if(!/^[0-9a-f]{32}$/i.test(hex))throw new Error('Invalid UUID');return Uint8Array.from(hex.match(/../g),x=>parseInt(x,16));}
  throw new Error('Invalid UUID');
}
function uuidKey(value){return Array.from(uuidBytes(value),b=>b.toString(16).padStart(2,'0')).join('');}
function encodeFrame(messageType,payload,id,raw=false){
  if(!raw){for(const field of TdrvContract.requests[messageType]||[]){if(!(field in payload))throw new Error(`Missing ${field} in request`);}}
  const c=TdrvContract.header;const bytes=raw?payload:MsgPack.encode(payload);
  const frame=new Uint8Array(c.HEADER_LEN+bytes.length);const view=new DataView(frame.buffer);
  frame.set([84,68,82,86]);view.setUint8(c.VERSION_OFFSET,TdrvContract.version);
  view.setUint16(c.MESSAGE_TYPE_OFFSET,messageType,false);view.setUint8(c.ENCODING_OFFSET,raw?TdrvContract.encoding.RawBinary:TdrvContract.encoding.MessagePack);
  frame.set(uuidBytes(id),c.REQUEST_ID_OFFSET);view.setUint32(c.PAYLOAD_LENGTH_OFFSET,bytes.length,false);frame.set(bytes,c.HEADER_LEN);return frame;
}
function decodeFrame(input){
  const bytes=input instanceof Uint8Array?input:new Uint8Array(input);const c=TdrvContract.header;
  if(bytes.length<c.HEADER_LEN)throw new Error('Truncated frame');
  const view=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength);
  if(view.getUint32(0,false)!==0x54445256 || view.getUint8(c.VERSION_OFFSET)!==TdrvContract.version)throw new Error('Unsupported protocol');
  if(view.getUint8(c.COMPRESSION_OFFSET)!==0 || view.getUint8(c.RESERVED_OFFSET)!==0)throw new Error('Unsupported frame options');
  const length=view.getUint32(c.PAYLOAD_LENGTH_OFFSET,false);if(length!==bytes.length-c.HEADER_LEN)throw new Error('Invalid frame length');
  const encoding=view.getUint8(c.ENCODING_OFFSET);if(encoding!==1&&encoding!==2)throw new Error('Unsupported encoding');
  return {messageType:view.getUint16(c.MESSAGE_TYPE_OFFSET,false),requestId:uuidKey(bytes.subarray(c.REQUEST_ID_OFFSET,c.PAYLOAD_LENGTH_OFFSET)),
    payload:encoding===2?bytes.subarray(c.HEADER_LEN):MsgPack.decode(bytes.subarray(c.HEADER_LEN))};
}
function encodeChunk(transfer,index,offset,data){
  const bytes=new Uint8Array(TdrvContract.chunk.RAW_CHUNK_HEADER_LEN+data.length);const view=new DataView(bytes.buffer);
  bytes.set(uuidBytes(transfer));view.setUint32(16,index,false);view.setBigUint64(20,BigInt(offset),false);view.setUint32(28,data.length,false);bytes.set(data,32);return bytes;
}
globalThis.TealDriveWire={MsgPack,uuidBytes,uuidKey,encodeFrame,decodeFrame,encodeChunk};
})();
