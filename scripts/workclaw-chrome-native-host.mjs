export function encodeNativeMessage(message) {
  const json = Buffer.from(JSON.stringify(message), "utf8");
  const header = Buffer.alloc(4);
  header.writeUInt32LE(json.length, 0);
  return Buffer.concat([header, json]);
}

export function decodeNativeMessage(buffer) {
  const size = buffer.readUInt32LE(0);
  return JSON.parse(buffer.subarray(4, 4 + size).toString("utf8"));
}

export async function processNativeHostFrame(
  buffer,
  {
    baseUrl,
    fetchImpl = fetch,
  },
) {
  const envelope = decodeNativeMessage(buffer);
  const response = await fetchImpl(`${baseUrl.replace(/\/$/, "")}/api/browser-bridge/native-message`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(envelope),
  });
  const payload = await response.json();
  return encodeNativeMessage(payload);
}

if (import.meta.url === `file://${process.argv[1]?.replace(/\\/g, "/")}`) {
  const baseUrl = process.env.WORKCLAW_BROWSER_BRIDGE_BASE_URL || "http://127.0.0.1:4312";
  const chunks = [];

  process.stdin.on("data", (chunk) => {
    chunks.push(Buffer.from(chunk));
  });

  process.stdin.on("end", async () => {
    try {
      const output = await processNativeHostFrame(Buffer.concat(chunks), { baseUrl });
      process.stdout.write(output);
    } catch (error) {
      console.error(error instanceof Error ? error.message : String(error));
      process.exitCode = 1;
    }
  });
}
