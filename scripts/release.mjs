import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { createReadStream } from "node:fs";
import { copyFile, mkdir, mkdtemp, readFile, readdir, rm, rmdir, stat, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { requireTrustedReleaseInvocation } from "./release-context.mjs";
import { verifyReleaseSignatures } from "./verify-signatures.mjs";

const releaseRoot = "https://github.com/yuchen0x1/authenticator-desk/releases/";
const installerTargets = ["windows-x86_64-nsis", "windows-x86_64-msi"];

const read = (path) => readFile(path, "utf8");
const json = async (path) => JSON.parse(await read(path));
const checksum = async (path) => {
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(path)) hash.update(chunk);
  return hash.digest("hex");
};

async function version(tag) {
  const { version } = await json("package.json");
  const bundle = await json("src-tauri/tauri.conf.json");
  const cargo = await read("src-tauri/Cargo.toml");
  const lock = await read("src-tauri/Cargo.lock");
  const crateVersion = cargo.match(/^\[package\]\r?\n[\s\S]*?^version = "([^"]+)"/m)?.[1];
  const lockedVersion = lock.match(/^name = "authenticator-desk"\r?\nversion = "([^"]+)"/m)?.[1];
  if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-rc\.[1-9]\d*)?$/.test(version) ||
      [bundle.version, crateVersion, lockedVersion].some((value) => value !== version) ||
      (tag && tag !== "v" + version)) {
    throw new Error("Package, Cargo, Tauri, Cargo.lock and the release tag must agree");
  }
  console.log("Verified version " + version);
  return version;
}

async function oneFile(dir, extension) {
  const files = (await readdir(dir)).filter((name) => name.endsWith(extension));
  if (files.length !== 1) throw new Error("Expected one " + extension + " file in " + dir);
  return resolve(dir, files[0]);
}

function names(version) {
  return ["AuthenticatorDesk-" + version + "-windows-x64-setup.exe",
    "AuthenticatorDesk-" + version + "-windows-x64.msi"];
}

function tauri(...args) {
  execFileSync(process.execPath, ["node_modules/@tauri-apps/cli/tauri.js", ...args],
    { stdio: "inherit" });
}

function verifyPublisher(paths) {
  const thumbprint = process.env.AUTHDESK_PUBLISHER_THUMBPRINT;
  if (!thumbprint) {
    if (process.env.AUTHDESK_REQUIRE_AUTHENTICODE === "1") throw new Error("Publisher certificate is required for this release policy");
    console.warn("Authenticode not configured; updater signing is not Windows publisher signing");
    return;
  }
  if (process.platform !== "win32" || !/^[a-fA-F0-9]{40}$/.test(thumbprint)) {
    throw new Error("Publisher verification requires Windows and a pinned certificate thumbprint");
  }
  const env = { ...process.env };
  delete env.TAURI_SIGNING_PRIVATE_KEY;
  delete env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD;
  for (const artifact of paths) {
    execFileSync("powershell.exe", ["-NoProfile", "-File", resolve("scripts/verify-authenticode.ps1"), "-ExpectedThumbprint", thumbprint, "-Artifact", artifact], { stdio:"inherit", env });
  }
}

async function buildSigned() {
  requireTrustedReleaseInvocation();
  const semver = await version(process.env.GITHUB_REF_NAME);
  const key = process.env.AUTHDESK_UPDATER_PUBLIC_KEY;
  if (!key || !process.env.TAURI_SIGNING_PRIVATE_KEY || !process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD) {
    throw new Error("Missing updater signing credentials");
  }
  const config = await json("src-tauri/tauri.release.conf.json");
  config.plugins.updater.pubkey = key;
  const publisher = process.env.AUTHDESK_PUBLISHER_THUMBPRINT;
  if (process.env.AUTHDESK_REQUIRE_AUTHENTICODE === "1" && !publisher) throw new Error("Publisher signing policy requires a certificate");
  if (publisher) {
    const timestamp = process.env.AUTHDESK_TIMESTAMP_URL;
    if (!/^[a-fA-F0-9]{40}$/.test(publisher) || !timestamp || new URL(timestamp).protocol !== "https:") {
      throw new Error("Publisher signing requires a certificate pin and HTTPS RFC3161 timestamp service");
    }
    config.bundle.windows = { ...config.bundle.windows, certificateThumbprint:publisher, digestAlgorithm:"sha256", timestampUrl:timestamp, tsp:true };
  }
  // Compile and package without exposing the updater private key to build hooks.
  // Only the subsequent standalone signer sees the key; final package bytes are signed.
  config.bundle.createUpdaterArtifacts = false;
  const buildEnv = { ...process.env };
  delete buildEnv.TAURI_SIGNING_PRIVATE_KEY;
  delete buildEnv.TAURI_SIGNING_PRIVATE_KEY_PASSWORD;
  const temporaryDir = await mkdtemp(join(tmpdir(), "authdesk-build-"));
  const path = join(temporaryDir, "tauri-release.json");
  try {
    await writeFile(path, JSON.stringify(config));
    execFileSync(process.execPath, ["node_modules/@tauri-apps/cli/tauri.js", "build", "--ci", "--features", "signed-updater", "--config", path, "--", "--locked"],
      { stdio: "inherit", env: buildEnv });
    for (const [directory, extension] of [["nsis", ".exe"], ["msi", ".msi"]]) {
      const artifact = await oneFile("src-tauri/target/release/bundle/" + directory, extension);
      verifyPublisher([artifact]);
      tauri("signer", "sign", "--app-version", semver, artifact);
    }
  } finally {
    await rm(path, { force: true });
    await rmdir(temporaryDir);
  }
}

async function stage(signed) {
  if (signed) requireTrustedReleaseInvocation();
  const semver = await version();
  const root = "src-tauri/target/release/bundle/";
  const sources = [await oneFile(root + "nsis", ".exe"), await oneFile(root + "msi", ".msi")];
  const out = "release-assets";
  await mkdir(out, { recursive: true });
  if ((await readdir(out)).length !== 0) throw new Error("Release staging directory must be empty; archive previous artifacts first");
  const artifacts = [];
  for (const [index, name] of names(semver).entries()) {
    const path = resolve(out, name);
    await copyFile(sources[index], path);
    artifacts.push({ name, bytes: (await stat(path)).size, sha256: await checksum(path) });
  }
  const manifest = { schema: 1, version: semver, platform: "windows", arch: "x64", artifacts };
  await writeFile(resolve(out, "release-assets.json"), JSON.stringify(manifest, null, 2) + "\n");
  await writeFile(resolve(out, "SHA256SUMS.txt"),
    artifacts.map(({ sha256, name }) => sha256 + "  " + name + "\n").join(""));
  if (signed) await stageUpdate(out, semver, sources, artifacts);
  await verify(out, "v" + semver, signed);
}

async function stageUpdate(out, version, sources, artifacts) {
  const platforms = {};
  for (const [index, { name, bytes, sha256 }] of artifacts.entries()) {
    const path = resolve(out, name + ".sig");
    await copyFile(sources[index] + ".sig", path);
    platforms[installerTargets[index]] = {
      url: releaseRoot + "download/v" + version + "/" + name,
      signature: (await read(path)).trim(),
      bytes,
      sha256,
    };
  }
  const notes = (await read("docs/releases/v" + version + ".md")).trim();
  const releaseDate = new Intl.DateTimeFormat("en-CA", {
    timeZone: "Asia/Shanghai", year: "numeric", month: "2-digit", day: "2-digit",
  }).format(new Date());
  const payload = JSON.stringify({
    schema: 1,
    version,
    pub_date: releaseDate + "T00:00:00Z",
    release_date: releaseDate,
    notes,
    min_windows_build: 19045,
    platforms,
  });
  const temporaryDir = await mkdtemp(join(tmpdir(), "authdesk-update-"));
  const path = join(temporaryDir, "metadata.json");
  try {
    await writeFile(path, payload);
    tauri("signer", "sign", "--app-version", version, path);
    const signature = (await read(path + ".sig")).trim();
    await writeFile(resolve(out, "update.json"), JSON.stringify({ payload, signature }) + "\n");
  } finally {
    await rm(path, { force: true });
    await rm(path + ".sig", { force: true });
    await rmdir(temporaryDir);
  }
}

async function verify(dir, tag, signed) {
  if (!/^v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-rc\.[1-9]\d*)?$/.test(tag ?? "")) {
    throw new Error("Invalid release tag");
  }
  const expected = names(tag.slice(1));
  const manifest = await json(resolve(dir, "release-assets.json"));
  if (manifest.schema !== 1 || manifest.version !== tag.slice(1) || manifest.platform !== "windows" ||
      manifest.arch !== "x64" || !Array.isArray(manifest.artifacts) ||
      manifest.artifacts.map(({ name }) => name).join("\n") !== expected.join("\n")) {
    throw new Error("Unexpected release assets or metadata");
  }
  const installedFiles = (await readdir(dir)).sort();
  const expectedFiles = [...expected, "SHA256SUMS.txt", "release-assets.json"];
  if (signed) expectedFiles.push("update.json", ...expected.map((name) => name + ".sig"));
  if (installedFiles.join("\n") !== expectedFiles.sort().join("\n")) {
    throw new Error("Unexpected files in release asset directory");
  }
  const sums = [];
  for (const { name, bytes, sha256 } of manifest.artifacts) {
    const path = resolve(dir, name);
    if (basename(path) !== name || bytes > 64 * 1024 * 1024 || !Number.isSafeInteger(bytes) || bytes <= 0 || (await stat(path)).size !== bytes ||
      !/^[a-f0-9]{64}$/.test(sha256) || await checksum(path) !== sha256) {
      throw new Error("Integrity check failed for " + name);
    }
    sums.push(sha256 + "  " + name + "\n");
  }
  if (await read(resolve(dir, "SHA256SUMS.txt")) !== sums.join("")) {
    throw new Error("SHA256SUMS does not match release metadata");
  }
  if (signed) {
    const publicKey = process.env.AUTHDESK_VERIFY_PUBLIC_KEY ?? process.env.AUTHDESK_UPDATER_PUBLIC_KEY;
    if (!publicKey) throw new Error("A separately trusted updater public key is required");
    if ((await stat(resolve(dir, "update.json"))).size > 96 * 1024) throw new Error("Update manifest exceeds the client limit");
    const envelope = await json(resolve(dir, "update.json"));
    const update = JSON.parse(envelope.payload);
    if (!/^[A-Za-z0-9+/]+=*$/.test(envelope.signature) || update.schema !== 1 ||
        !Number.isSafeInteger(update.min_windows_build) || update.min_windows_build < 19045 ||
        !Number.isFinite(Date.parse(update.pub_date)) || typeof update.notes !== "string" ||
        update.version !== manifest.version || !/^\d{4}-\d{2}-\d{2}$/.test(update.release_date) ||
        Buffer.byteLength(update.notes, "utf8") > 8192 ||
        Object.keys(update.platforms).sort().join(",") !== [...installerTargets].sort().join(",")) {
      throw new Error("Invalid update metadata");
    }
    for (const [i, file] of manifest.artifacts.entries()) {
      const asset = update.platforms[installerTargets[i]];
      const signature = (await read(resolve(dir, file.name + ".sig"))).trim();
      if (!/^[A-Za-z0-9+/]+=*$/.test(signature) || asset.signature !== signature ||
          asset.url !== releaseRoot + "download/" + tag + "/" + file.name ||
          asset.bytes !== file.bytes || asset.sha256 !== file.sha256) {
        throw new Error("Update metadata does not match " + file.name);
      }
    }
    await verifyReleaseSignatures(dir, tag, publicKey);
  }
  console.log("Verified release assets for " + tag);
}

export function assertStablePromotion(tag, currentTags = "") {
  const parse = (value) => {
    const match = /^v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.exec(value);
    if (!match) throw new Error("Stable promotion requires canonical version tags");
    return match.slice(1).map(BigInt);
  };
  const candidate = parse(tag);
  for (const current of currentTags.split(/\s+/).filter(Boolean)) {
    const previous = parse(current);
    let comparison = 0;
    for (let index = 0; index < 3; index++) {
      if (candidate[index] !== previous[index]) { comparison = candidate[index] > previous[index] ? 1 : -1; break; }
    }
    if (comparison <= 0) throw new Error("Stable channel may only advance beyond existing releases");
  }
}

export { verify, version };
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [command, dir, tag, mode] = process.argv.slice(2);
  switch (command) {
    case "context": requireTrustedReleaseInvocation(); break;
    case "promotion": assertStablePromotion(requireTrustedReleaseInvocation(), dir); break;
    case "version": await version(dir); break;
    case "build-signed": await buildSigned(); break;
    case "stage": await stage(dir === "signed"); break;
    case "verify": await verify(dir, tag, mode === "signed"); break;
    default: throw new Error("Usage: release.mjs context | promotion <current-stable-tags> | version [tag] | build-signed | stage [signed] | verify <dir> <tag> [signed]");
  }
}
