import { describe, expect, it } from "vitest";
import { futureT0Message, isFutureTotp } from "./futureT0";

const account = { kind: "totp" as const, t0: "0" };

describe("future T0 display state", () => {
  it("has an explicit message in every supported locale", () => {
    expect(futureT0Message("zh-CN")).toBe("尚未生效");
    expect(futureT0Message("zh-TW")).toBe("尚未生效");
    expect(futureT0Message("en")).toBe("Not active yet");
    expect(futureT0Message("ja")).toBe("まだ有効ではありません");
  });

  it("compares decimal T0 without Number precision loss and honors the boundary", () => {
    const farFuture = { ...account, t0: "9007199254740993" };
    expect(isFutureTotp(farFuture, 9007199254740992n)).toBe(true);
    expect(isFutureTotp(farFuture, 9007199254740993n)).toBe(false);
    expect(isFutureTotp({ ...account, t0: "100" }, 99)).toBe(true);
    expect(isFutureTotp({ ...account, t0: "100" }, 100)).toBe(false);
    expect(isFutureTotp({ kind: "hotp", t0: "9007199254740993" }, 0)).toBe(false);
  });
});
