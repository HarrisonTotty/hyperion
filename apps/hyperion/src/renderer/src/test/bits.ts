/**
 * A digest of a value's bits, for the tests that hold a bake spread over the optics workers to the
 * bytes of one baked whole (plan R08, R08.T14.d's bake time).
 */

/**
 * SHA-256 of every number and typed array's bytes `value` holds, in its order, with its keys and
 * strings: the same bits give the same digest.
 */
export async function bitsOf(value: unknown): Promise<string> {
  const chunks: Uint8Array[] = [];
  const text = new TextEncoder();
  const walk = (item: unknown): void => {
    if (ArrayBuffer.isView(item)) {
      chunks.push(text.encode(`${item.constructor.name}:`));
      chunks.push(new Uint8Array(item.buffer, item.byteOffset, item.byteLength));
    } else if (typeof item === "number") {
      chunks.push(new Uint8Array(Float64Array.of(item).buffer));
    } else if (Array.isArray(item)) {
      chunks.push(text.encode(`[${item.length}`));
      item.forEach(walk);
    } else if (typeof item === "object" && item !== null) {
      for (const [key, member] of Object.entries(item)) {
        chunks.push(text.encode(`{${key}`));
        walk(member);
      }
    } else {
      chunks.push(text.encode(String(item)));
    }
  };
  walk(value);
  const bytes = new Uint8Array(chunks.reduce((sum, chunk) => sum + chunk.byteLength, 0));
  let at = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, at);
    at += chunk.byteLength;
  }
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
  return Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join("");
}
