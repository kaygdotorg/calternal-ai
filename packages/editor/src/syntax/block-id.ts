// Block ids: `newBlockId` makes a 16 character Crockford marker from 80 random bits.

const CROCKFORD = "0123456789abcdefghjkmnpqrstvwxyz"

/** Makes a 16 character Crockford block marker from 80 random bits. */
export function newBlockId(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(10))
  let bits = 0n
  for (const byte of bytes) bits = (bits << 8n) | BigInt(byte)
  let out = ""
  for (let n = 0; n < 16; n++) {
    out = CROCKFORD[Number(bits & 31n)]! + out
    bits >>= 5n
  }
  return out
}
