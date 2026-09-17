// Probe P01, browser side: does the demuxing library the website will use hand back each sample exactly
// as stored, so that the digest of its packets equals adiungere-track-fp/1?
//
//     node tools/oracles/browser-reader.mjs RECORDING
//
// Prints one line per track: the packet count, the byte count, the SHA-256 of the packets concatenated in
// decode order, and the SHA-256 of the decoder description the library exposes. The payload digest is
// compared with the manifest's track fingerprint; the description digest is compared for video only,
// because for audio the library exposes the two-byte specific configuration rather than the whole
// elementary stream descriptor the fingerprint digests.
//
// The library version is pinned in tools/oracles/package.json.

import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { ALL_FORMATS, BufferSource, EncodedPacketSink, Input } from "mediabunny";

const path = process.argv[2];
if (!path) {
  console.error("usage: node tools/oracles/browser-reader.mjs RECORDING");
  process.exit(2);
}

const bytes = await readFile(path);
const input = new Input({ formats: ALL_FORMATS, source: new BufferSource(new Uint8Array(bytes)) });
const tracks = await input.getTracks();
const lines = [];

for (const track of tracks) {
  const sink = new EncodedPacketSink(track);
  const payload = createHash("sha256");
  let packets = 0;
  let total = 0;
  for await (const packet of sink.packets()) {
    payload.update(packet.data);
    packets += 1;
    total += packet.data.byteLength;
  }
  const config = await track.getDecoderConfig();
  const description = config?.description
    ? createHash("sha256").update(new Uint8Array(config.description)).digest("hex")
    : "none";
  lines.push({
    id: track.id,
    type: track.type,
    codec: track.codec,
    packets,
    bytes: total,
    payload_sha256: payload.digest("hex"),
    description_sha256: description,
  });
}

console.log(JSON.stringify(lines, null, 2));
