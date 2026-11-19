import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { createReadStream } from "node:fs";
import { copyFile, mkdir, mkdtemp, readFile, readdir, rm, rmdir, stat, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";

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

async function buildSigned() {
  const key = process.env.AUTHDESK_UPDATER_PUBLIC_KEY;
  if (!key || !process.env.TAURI_SIGNING_PRIVATE_KEY || !process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD) {
    throw new Error("Missing updater signing credentials");
  }
  const config = await json("src-tauri/tauri.release.conf.json");
  config.plugins.updater.pubkey = key;
  const temporaryDir = await mkdtemp(join(tmpdir(), "authdesk-build-"));
  const path = join(temporaryDir, "tauri-release.json");
  try {
    await writeFile(path, JSON.stringify(config));
    tauri("build", "--ci", "--features", "signed-updater", "--config", path, "--", "--locked");
  } finally {
    await rm(path, { force: true });
    await rmdir(temporaryDir);
  }
}

async function stage(signed) {
  const semver = await version();
  const root = "src-tauri/target/release/bundle/";
  const sources = [await oneFile(root + "nsis", ".exe"), await oneFile(root + "msi", ".msi")];
  const out = "release-assets";
  await mkdir(out, { recursive: true });
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
    if (basename(path) !== name || bytes <= 0 || (await stat(path)).size !== bytes ||
      !/^[a-f0-9]{64}$/.test(sha256) || await checksum(path) !== sha256) {
      throw new Error("Integrity check failed for " + name);
    }
    sums.push(sha256 + "  " + name + "\n");
  }
  if (await read(resolve(dir, "SHA256SUMS.txt")) !== sums.join("")) {
    throw new Error("SHA256SUMS does not match release metadata");
  }
  if (signed) {
    const envelope = await json(resolve(dir, "update.json"));
    const update = JSON.parse(envelope.payload);
    if (!/^[A-Za-z0-9+/]+=*$/.test(envelope.signature) || update.schema !== 1 ||
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
  }
  console.log("Verified release assets for " + tag);
}

const [command, dir, tag, mode] = process.argv.slice(2);
switch (command) {
  case "version": await version(dir); break;
  case "build-signed": await buildSigned(); break;
  case "stage": await stage(dir === "signed"); break;
  case "verify": await verify(dir, tag, mode === "signed"); break;
  default: throw new Error("Usage: release.mjs version [tag] | build-signed | stage [signed] | verify <dir> <tag> [signed]");
}
