import assert from "node:assert/strict";
import { readFile, access } from "node:fs/promises";
import { describe, it } from "node:test";

const root = new URL("../", import.meta.url);
const versions = ["README.md", "README.zh-TW.md", "README.en.md", "README.ja.md"];

function localLinks(text) {
  return [
    ...Array.from(text.matchAll(/\]\(([^)\s]+)\)/g), (match) => match[1]),
    ...Array.from(text.matchAll(/(?:src|href)="([^"]+)"/g), (match) => match[1]),
  ].filter((link) => !/^(?:[a-z]+:|#|\/\/)/i.test(link));
}

describe("localized README navigation and assets", () => {
  for (const filename of versions) {
    it(`${filename} links to every other language`, async () => {
      const text = await readFile(new URL(filename, root), "utf8");
      const links = localLinks(text);
      for (const other of versions.filter((name) => name !== filename)) {
        assert.ok(links.includes(other), `${filename} does not link to ${other}`);
      }
    });

    it(`${filename} has no missing local targets`, async () => {
      const text = await readFile(new URL(filename, root), "utf8");
      for (const link of new Set(localLinks(text))) {
        const target = new URL(link.split("#")[0], root);
        assert.ok(target.href.startsWith(root.href), `${filename}: target escapes repository: ${link}`);
        await assert.doesNotReject(access(target), `${filename}: missing ${link}`);
      }
    });

    it(`${filename} includes the ten checked-in UI captures`, async () => {
      const text = await readFile(new URL(filename, root), "utf8");
      const captures = new Set(localLinks(text).filter((link) => /^screenshots\/\d{2}-.+\.png$/.test(link)));
      assert.equal(captures.size, 10, `${filename}: incomplete screenshot gallery`);
      assert.match(text, /NotSigned/);
      assert.match(text, /unsupported_encrypted_format/);
    });
  }
});
