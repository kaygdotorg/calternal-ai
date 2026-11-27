/** Yield one bounded frame at a time from UTF-8 bytes. Consumers must
 * decode each frame before asking for the next one, as the buffer is reused.
 * Work is linear in bytes received. Overflow or truncation rejects the stream.
 */
export async function* newlineFrames(input: AsyncIterable<Uint8Array>, maximum: number) {
  let buffer = new Uint8Array(Math.min(4096, maximum));
  let used = 0;
  for await (const chunk of input) {
    let start = 0;
    while (start < chunk.length) {
      const newline = chunk.indexOf(10, start);
      const end = newline < 0 ? chunk.length : newline;
      const part = chunk.subarray(start, end);
      const length = used + part.length;
      if (length > maximum) throw new Error("worker frame exceeds its budget");
      if (length > buffer.length) {
        const grown = new Uint8Array(Math.min(maximum, Math.max(length, buffer.length * 2)));
        grown.set(buffer.subarray(0, used));
        buffer = grown;
      }
      buffer.set(part, used);
      used = length;
      if (newline < 0) break;
      yield buffer.subarray(0, used);
      used = 0;
      start = end + 1;
    }
  }
  if (used !== 0) throw new Error("unterminated worker frame");
}
