function concatBytes(chunks: Uint8Array[]): Uint8Array {
  const totalLength = chunks.reduce((sum, chunk) => sum + chunk.byteLength, 0);
  const merged = new Uint8Array(totalLength);
  let offset = 0;
  for (const chunk of chunks) {
    merged.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return merged;
}

export function encodeNativeMessage(message: unknown): Uint8Array {
  const json = new TextEncoder().encode(JSON.stringify(message));
  const header = new Uint8Array(4);
  new DataView(header.buffer).setUint32(0, json.byteLength, true);
  return concatBytes([header, json]);
}

export function decodeNativeMessage(buffer: Uint8Array): unknown {
  const view = new DataView(buffer.buffer, buffer.byteOffset, buffer.byteLength);
  const size = view.getUint32(0, true);
  const payload = buffer.subarray(4, 4 + size);
  return JSON.parse(new TextDecoder().decode(payload));
}

export async function processNativeHostFrame(
  buffer: Uint8Array,
  handler: (message: unknown) => Promise<unknown> | unknown,
): Promise<Uint8Array> {
  const message = decodeNativeMessage(buffer);
  const response = await handler(message);
  return encodeNativeMessage(response);
}
