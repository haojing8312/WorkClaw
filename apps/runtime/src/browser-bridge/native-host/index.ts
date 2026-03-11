export function encodeNativeMessage(message: unknown): Buffer {
  const json = Buffer.from(JSON.stringify(message), "utf8");
  const header = Buffer.alloc(4);
  header.writeUInt32LE(json.length, 0);
  return Buffer.concat([header, json]);
}

export function decodeNativeMessage(buffer: Buffer): unknown {
  const size = buffer.readUInt32LE(0);
  return JSON.parse(buffer.subarray(4, 4 + size).toString("utf8"));
}
