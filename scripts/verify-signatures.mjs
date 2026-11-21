import { createHash, createPublicKey, verify } from "node:crypto";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { TextDecoder } from "node:util";

const ED = 0x4564; // Ed: legacy, signs the original bytes
const ED_PREHASHED = 0x4544; // ED: signs a 64-byte BLAKE2b digest
const ED25519_SPKI_PREFIX = Buffer.from("302a300506032b6570032100", "hex");
const UNTRUSTED_PREFIX = "untrusted comment: ";
const TRUSTED_PREFIX = "trusted comment: ";
const VERSION_PREFIX = "version:";
const utf8 = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true });

function decodeBase64(value, label, length) {
  if (typeof value !== "string" || value.length === 0) {
    throw new Error(`Invalid base64 for ${label}`);
  }
  const decoded = Buffer.from(value, "base64");
  // Buffer's decoder accepts whitespace, base64url, missing padding and nonzero pad bits.
  if (decoded.toString("base64") !== value || (length !== undefined && decoded.length !== length)) {
    throw new Error(`Invalid base64 or packet length for ${label}`);
  }
  return decoded;
}

function decodeUtf8(bytes, label) {
  try {
    return utf8.decode(bytes);
  } catch {
    throw new Error(`Invalid UTF-8 for ${label}`);
  }
}

function decodeEnvelope(encoded, lineCount, label) {
  const lines = decodeUtf8(decodeBase64(encoded, label), label).split(/\r?\n/);
  if (lines.at(-1) === "") lines.pop(); // Optional single final line ending.
  if (lines.length !== lineCount || !lines[0].startsWith(UNTRUSTED_PREFIX) ||
      lines.some((line) => /[\r\0]/.test(line))) {
    throw new Error(`Invalid minisign ${label} envelope`);
  }
  return lines;
}

function checkAlgorithm(packet, label) {
  const algorithm = packet.readUInt16BE(0);
  if (algorithm !== ED && algorithm !== ED_PREHASHED) {
    throw new Error(`Unsupported minisign ${label} algorithm`);
  }
  return algorithm;
}

/** Verify Tauri's outer base64-encoded minisign signature and public key, or throw.
 * @param {Buffer} data Exact artifact or update payload bytes.
 * @param {string} encodedSignature
 * @param {string} encodedPublicKey
 * @param {string} expectedVersion Version text without the release tag's leading `v`.
 * @returns {void}
 */
export function verifySignature(data, encodedSignature, encodedPublicKey, expectedVersion) {
  if (!Buffer.isBuffer(data)) throw new TypeError("Signature data must be a Buffer");
  if (typeof expectedVersion !== "string" || !expectedVersion || /[\r\n\t\0]/.test(expectedVersion)) {
    throw new TypeError("Invalid expected version");
  }

  // Packet layout and verification follow minisign-verify 0.3.0 (Tauri enables legacy mode).
  const [, keyLine] = decodeEnvelope(encodedPublicKey, 2, "public key");
  const [, signatureLine, commentLine, globalLine] = decodeEnvelope(encodedSignature, 4, "signature");
  const keyPacket = decodeBase64(keyLine, "public key packet", 42);
  const signaturePacket = decodeBase64(signatureLine, "signature packet", 74);
  const globalSignature = decodeBase64(globalLine, "global signature", 64);

  checkAlgorithm(keyPacket, "public key");
  const algorithm = checkAlgorithm(signaturePacket, "signature");
  if (!keyPacket.subarray(2, 10).equals(signaturePacket.subarray(2, 10))) {
    throw new Error("Minisign key ID mismatch");
  }
  if (!commentLine.startsWith(TRUSTED_PREFIX)) throw new Error("Invalid minisign trusted comment");

  const rawKey = keyPacket.subarray(10);
  // Match minisign's explicit rejection of the zero and identity public keys.
  if (rawKey.every((byte) => byte === 0) ||
      (rawKey[0] === 1 && rawKey.subarray(1, 31).every((byte) => byte === 0) && (rawKey[31] & 0x7f) === 0)) {
    throw new Error("Invalid minisign public key");
  }
  const publicKey = createPublicKey({
    key: Buffer.concat([ED25519_SPKI_PREFIX, rawKey]), format: "der", type: "spki",
  });
  const dataSignature = signaturePacket.subarray(10);
  const message = algorithm === ED_PREHASHED ? createHash("blake2b512").update(data).digest() : data;
  if (!verify(null, message, publicKey, dataSignature)) throw new Error("Invalid minisign data signature");

  const comment = commentLine.slice(TRUSTED_PREFIX.length);
  // Minisign signs the raw 64-byte data signature and the UTF-8 comment body, without the prefix.
  const globalMessage = Buffer.concat([dataSignature, Buffer.from(comment, "utf8")]);
  if (!verify(null, globalMessage, publicKey, globalSignature)) throw new Error("Invalid minisign global signature");

  // Tauri writes tab-delimited timestamp/file/version fields. Require one exact version field.
  const versions = comment.split("\t").filter((field) => field.startsWith(VERSION_PREFIX));
  if (versions.length !== 1 || versions[0] !== VERSION_PREFIX + expectedVersion) {
    throw new Error("Minisign trusted comment version mismatch or missing");
  }
}

const RELEASE_TAG = /^v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-rc\.[1-9]\d*)?$/;
const RELEASE_DOWNLOAD = "https://github.com/yuchen0x1/authenticator-desk/releases/download/";
const INSTALLERS = [
  ["windows-x86_64-nsis", "-setup.exe"],
  ["windows-x86_64-msi", ".msi"],
];
const isRecord = (value) => value !== null && typeof value === "object" && !Array.isArray(value);

function parseJson(text, label) {
  try {
    return JSON.parse(text);
  } catch {
    // JSON.parse errors can contain excerpts of the input; keep diagnostics input-free.
    throw new Error(`Invalid JSON for ${label}`);
  }
}

async function readText(dir, name) {
  return decodeUtf8(await readFile(join(dir, name)), name);
}

/** Verify the signed update payload and both staged installer signatures against one updater key. */
export async function verifyReleaseSignatures(dir, expectedTag, publicKey) {
  if (typeof expectedTag !== "string" || !RELEASE_TAG.test(expectedTag)) {
    throw new Error("Invalid release tag");
  }
  const version = expectedTag.slice(1);
  const envelope = parseJson(await readText(dir, "update.json"), "update envelope");
  if (!isRecord(envelope) || typeof envelope.payload !== "string") throw new Error("Invalid update envelope");
  const payloadBytes = Buffer.from(envelope.payload, "utf8");
  if (payloadBytes.toString("utf8") !== envelope.payload) throw new Error("Invalid update payload UTF-8");
  verifySignature(payloadBytes, envelope.signature, publicKey, version);
  const update = parseJson(envelope.payload, "update payload");
  if (!isRecord(update) || update.schema !== 1 || update.version !== version ||
      !isRecord(update.platforms) || Object.keys(update.platforms).length !== INSTALLERS.length) {
    throw new Error("Invalid signed update metadata");
  }

  const manifest = parseJson(await readText(dir, "release-assets.json"), "release asset manifest");
  if (!isRecord(manifest) || manifest.schema !== 1 || manifest.version !== version ||
      manifest.platform !== "windows" || manifest.arch !== "x64" ||
      !Array.isArray(manifest.artifacts) || manifest.artifacts.length !== INSTALLERS.length) {
    throw new Error("Invalid release asset manifest");
  }

  for (const [index, [target, extension]] of INSTALLERS.entries()) {
    // Use the expected filename, never a filesystem path supplied by metadata.
    const name = `AuthenticatorDesk-${version}-windows-x64${extension}`;
    const artifact = manifest.artifacts[index];
    const asset = update.platforms[target];
    if (!isRecord(artifact) || artifact.name !== name || !Number.isSafeInteger(artifact.bytes) ||
        artifact.bytes <= 0 || typeof artifact.sha256 !== "string" || !/^[a-f0-9]{64}$/.test(artifact.sha256) ||
        !isRecord(asset) || asset.url !== `${RELEASE_DOWNLOAD}${expectedTag}/${name}` ||
        asset.bytes !== artifact.bytes || asset.sha256 !== artifact.sha256 || typeof asset.signature !== "string") {
      throw new Error(`Update metadata does not match ${name}`);
    }
    const signature = (await readText(dir, name + ".sig")).replace(/\r?\n$/, "");
    if (signature !== asset.signature) throw new Error(`Update signature does not match ${name}`);
    const bytes = await readFile(join(dir, name));
    if (bytes.length !== artifact.bytes || createHash("sha256").update(bytes).digest("hex") !== artifact.sha256) {
      throw new Error(`Installer integrity check failed for ${name}`);
    }
    verifySignature(bytes, signature, publicKey, version);
  }
}
