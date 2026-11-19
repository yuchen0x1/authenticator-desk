import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { isTrustedReleaseInvocation } from "./release-context.mjs";

const repository = "yuchen0x1/authenticator-desk";
const ref = "refs/tags/v1.2.3";
const workflowRef = repository + "/.github/workflows/release.yml@" + ref;

describe("isTrustedReleaseInvocation", () => {
  it("accepts the release workflow on a version tag push", () => {
    assert.equal(isTrustedReleaseInvocation({
      eventName: "push",
      ref,
      repository,
      workflowRef,
    }), true);
  });

  it("rejects pull request invocations", () => {
    assert.equal(isTrustedReleaseInvocation({
      eventName: "pull_request",
      ref: "refs/pull/8/merge",
      repository,
      workflowRef,
    }), false);
  });

  it("rejects callers other than the release workflow", () => {
    assert.equal(isTrustedReleaseInvocation({
      eventName: "push",
      ref,
      repository,
      workflowRef: repository + "/.github/workflows/other.yml@" + ref,
    }), false);
  });

  it("rejects branch pushes and malformed release tags", () => {
    assert.equal(isTrustedReleaseInvocation({
      eventName: "push",
      ref: "refs/heads/main",
      repository,
      workflowRef,
    }), false);
    assert.equal(isTrustedReleaseInvocation({
      eventName: "push",
      ref: "refs/tags/v01.2.3",
      repository,
      workflowRef: repository + "/.github/workflows/release.yml@refs/tags/v01.2.3",
    }), false);
  });
});
