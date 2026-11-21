import assert from "node:assert/strict";
import { createHash, generateKeyPairSync, randomBytes, sign } from "node:crypto";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, it } from "node:test";
import { verifyReleaseSignatures, verifySignature } from "./verify-signatures.mjs";

const VERSION = "1.2.3-rc.4";
const TAG = `v${VERSION}`;
const COMMENT = `timestamp:1700000000\tfile:应用.exe\tversion:${VERSION}`;
const DATA = Buffer.from([0x4d, 0x5a, 0xff, 0, 0x0d, 0x0a]);
const BASE64 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const encodeText = (text) => Buffer.from(text, "utf8").toString("base64");
const encodeLines = (lines, eol = "\n", finalEol = true) => encodeText(lines.join(eol) + (finalEol ? eol : ""));

function signingFixture(keyId = randomBytes(8)) {
  const { publicKey, privateKey } = generateKeyPairSync("ed25519");
  const rawPublicKey = Buffer.from(publicKey.export({ format: "jwk" }).x, "base64url");
  return {
    keyId,
    publicEnvelope(algorithm = "Ed", eol = "\n") {
      const packet = Buffer.concat([Buffer.from(algorithm), keyId, rawPublicKey]);
      return encodeLines(["untrusted comment: fixture public key", packet.toString("base64")], eol);
    },
    signature(data, { algorithm = "ED", comment = COMMENT, eol = "\n", finalEol = true } = {}) {
      const message = algorithm === "ED" ? createHash("blake2b512").update(data).digest() : data;
      const dataSignature = sign(null, message, privateKey);
      const packet = Buffer.concat([Buffer.from(algorithm), keyId, dataSignature]);
      const globalSignature = sign(null, Buffer.concat([dataSignature, Buffer.from(comment)]), privateKey);
      return encodeLines([
        "untrusted comment: fixture signature",
        packet.toString("base64"),
        `trusted comment: ${comment}`,
        globalSignature.toString("base64"),
      ], eol, finalEol);
    },
  };
}

function editLine(encoded, index, edit) {
  const lines = Buffer.from(encoded, "base64").toString("utf8").split("\n");
  lines[index] = edit(lines[index]);
  return encodeText(lines.join("\n"));
}

function editPacket(encoded, index, edit) {
  return editLine(encoded, index, (line) => {
    const packet = Buffer.from(line, "base64");
    edit(packet);
    return packet.toString("base64");
  });
}

const fixture = signingFixture();
const publicKey = fixture.publicEnvelope();
const signature = fixture.signature(DATA);

describe("verifySignature", () => {
  it("verifies Tauri-style ED signatures with a BLAKE2b-512 prehash and CRLF envelopes", () => {
    const key = fixture.publicEnvelope("Ed", "\r\n");
    const sig = fixture.signature(DATA, { eol: "\r\n" });
    assert.equal(verifySignature(DATA, sig, key, VERSION), undefined);
  });

  it("verifies legacy Ed signatures on raw bytes with an ED key header and no final newline", () => {
    const sig = fixture.signature(DATA, { algorithm: "Ed", comment: `version:${VERSION}`, finalEol: false });
    verifySignature(DATA, sig, fixture.publicEnvelope("ED"), VERSION);
  });

  it("rejects modified data and a changed signature algorithm", () => {
    const changed = Buffer.from(DATA);
    changed[0] ^= 1;
    assert.throws(() => verifySignature(changed, signature, publicKey, VERSION), /data signature/);
    const changedAlgorithm = editPacket(signature, 1, (packet) => { packet[1] = 0x64; });
    assert.throws(() => verifySignature(DATA, changedAlgorithm, publicKey, VERSION), /data signature/);
  });

  it("rejects altered trusted comments even when the version still matches, and altered global signatures", () => {
    const changedComment = editLine(signature, 2, (line) => line.replace("1700000000", "1700000001"));
    const changedVersion = editLine(signature, 2, (line) => line.replace(VERSION, "1.2.3"));
    const changedGlobal = editPacket(signature, 3, (packet) => { packet[0] ^= 1; });
    for (const sig of [changedComment, changedVersion, changedGlobal]) {
      assert.throws(() => verifySignature(DATA, sig, publicKey, VERSION), /global signature/);
    }
  });

  it("requires a single exact signed version field", () => {
    for (const comment of [
      "version:1.2.3",
      `version:v${VERSION}`,
      `version:${VERSION} suffix`,
      `version:${VERSION} `,
      `file:version:${VERSION}`,
      `version:${VERSION}\tversion:${VERSION}`,
      `version:1.2.3\tversion:${VERSION}`,
      "timestamp:1700000000",
    ]) {
      const sig = fixture.signature(DATA, { comment });
      assert.throws(() => verifySignature(DATA, sig, publicKey, VERSION), /version mismatch or missing/);
    }
  });

  it("rejects a different public key even with the correct key ID, and a tampered key ID", () => {
    const wrongKey = signingFixture(fixture.keyId).publicEnvelope();
    assert.throws(() => verifySignature(DATA, signature, wrongKey, VERSION), /data signature/);
    const wrongId = editPacket(signature, 1, (packet) => { packet[9] ^= 1; });
    assert.throws(() => verifySignature(DATA, wrongId, publicKey, VERSION), /key ID/);
  });

  it("rejects unsupported key and signature algorithm bytes", () => {
    const wrongSignatureAlgorithm = editPacket(signature, 1, (packet) => { packet[0] |= 0x80; });
    assert.throws(() => verifySignature(DATA, wrongSignatureAlgorithm, publicKey, VERSION), /signature algorithm/);
    assert.throws(() => verifySignature(DATA, signature, fixture.publicEnvelope("XX"), VERSION), /public key algorithm/);
  });

  it("rejects degenerate Ed25519 public keys", () => {
    for (const firstByte of [0, 1]) {
      const badKey = editPacket(publicKey, 1, (packet) => {
        packet.fill(0, 10);
        packet[10] = firstByte;
      });
      assert.throws(() => verifySignature(DATA, signature, badKey, VERSION), /Invalid minisign public key/);
    }
  });

  it("rejects noncanonical or malformed outer base64 and invalid UTF-8", () => {
    for (const value of ["", "_", "AA", "AA===", "AB==", Buffer.from([0xc0, 0xaf]).toString("base64")]) {
      assert.throws(() => verifySignature(DATA, value, publicKey, VERSION), /base64|UTF-8/);
      assert.throws(() => verifySignature(DATA, signature, value, VERSION), /base64|UTF-8/);
    }
    assert.throws(() => verifySignature(DATA, signature + "\n", publicKey, VERSION), /base64/);
    assert.throws(() => verifySignature(DATA, signature, publicKey + " ", VERSION), /base64/);
  });

  it("rejects malformed inner base64, incorrect packet lengths and malformed text envelopes", () => {
    const badKey = editLine(publicKey, 1, () => Buffer.alloc(41).toString("base64"));
    assert.throws(() => verifySignature(DATA, signature, badKey, VERSION), /packet length/);
    const noncanonicalPadBits = editLine(signature, 1, (line) => line.slice(0, -2) + BASE64[BASE64.indexOf(line.at(-2)) ^ 1] + "=");
    for (const value of [
      noncanonicalPadBits,
      editLine(signature, 1, (line) => line.slice(0, -1)),
      editLine(signature, 3, () => Buffer.alloc(65).toString("base64")),
    ]) {
      assert.throws(() => verifySignature(DATA, value, publicKey, VERSION), /base64|packet length/);
    }
    const text = Buffer.from(signature, "base64").toString("utf8");
    for (const value of ["\uFEFF" + text, text + "\n", text + "extra\n", text.replace(/\n/g, "\r")]) {
      assert.throws(() => verifySignature(DATA, encodeText(value), publicKey, VERSION), /envelope/);
    }
    const badComment = editLine(signature, 2, (line) => line.replace("trusted comment: ", "comment: "));
    assert.throws(() => verifySignature(DATA, badComment, publicKey, VERSION), /trusted comment/);
  });
});

async function stageFixture(t, { invalidInstallerSignature = false } = {}) {
  const dir = await mkdtemp(join(tmpdir(), "verify-signatures-"));
  t.after(() => rm(dir, { recursive: true, force: true }));
  const platforms = {};
  const artifacts = [];
  for (const [kind, suffix] of [["nsis", "-setup.exe"], ["msi", ".msi"]]) {
    const name = `AuthenticatorDesk-${VERSION}-windows-x64${suffix}`;
    const data = Buffer.concat([DATA, Buffer.from(kind)]);
    const bytes = data.length;
    const sha256 = createHash("sha256").update(data).digest("hex");
    const signedData = invalidInstallerSignature && kind === "msi" ? DATA : data;
    const signature = fixture.signature(signedData);
    artifacts.push({ name, bytes, sha256 });
    platforms[`windows-x86_64-${kind}`] = {
      bytes, sha256, signature,
      url: `https://github.com/yuchen0x1/authenticator-desk/releases/download/${TAG}/${name}`,
    };
    await writeFile(join(dir, name), data);
    await writeFile(join(dir, name + ".sig"), signature + "\r\n");
  }
  const manifest = { schema: 1, version: VERSION, platform: "windows", arch: "x64", artifacts };
  await writeFile(join(dir, "release-assets.json"), JSON.stringify(manifest));
  const payload = JSON.stringify({ schema: 1, version: VERSION, notes: "发行说明", platforms }, null, 2);
  const envelope = { payload, signature: fixture.signature(Buffer.from(payload)) };
  await writeFile(join(dir, "update.json"), JSON.stringify(envelope));
  return { dir, envelope, manifest };
}

describe("verifyReleaseSignatures", () => {
  it("verifies the exact update payload and both installers against the staged metadata", async (t) => {
    const { dir } = await stageFixture(t);
    await verifyReleaseSignatures(dir, TAG, publicKey);
  });

  it("rejects changes to the exact signed payload bytes even when JSON meaning is unchanged", async (t) => {
    const { dir, envelope } = await stageFixture(t);
    envelope.payload += " ";
    await writeFile(join(dir, "update.json"), JSON.stringify(envelope));
    await assert.rejects(verifyReleaseSignatures(dir, TAG, publicKey), /data signature/);
  });

  it("rejects release manifest metadata that disagrees with the signed payload", async (t) => {
    const { dir, manifest } = await stageFixture(t);
    manifest.artifacts[0].bytes++;
    await writeFile(join(dir, "release-assets.json"), JSON.stringify(manifest));
    await assert.rejects(verifyReleaseSignatures(dir, TAG, publicKey), /Update metadata does not match/);
  });

  it("rejects a sidecar that disagrees with the signed payload", async (t) => {
    const { dir, manifest } = await stageFixture(t);
    await writeFile(join(dir, manifest.artifacts[0].name + ".sig"), signature);
    await assert.rejects(verifyReleaseSignatures(dir, TAG, publicKey), /Update signature does not match/);
  });

  it("rejects changed installer bytes with the same length", async (t) => {
    const { dir, manifest } = await stageFixture(t);
    const { name, bytes } = manifest.artifacts[0];
    await writeFile(join(dir, name), Buffer.alloc(bytes));
    await assert.rejects(verifyReleaseSignatures(dir, TAG, publicKey), /Installer integrity check failed/);
  });

  it("verifies the installer data signature even when sidecars and hashes match both manifests", async (t) => {
    const { dir } = await stageFixture(t, { invalidInstallerSignature: true });
    await assert.rejects(verifyReleaseSignatures(dir, TAG, publicKey), /data signature/);
  });
});
