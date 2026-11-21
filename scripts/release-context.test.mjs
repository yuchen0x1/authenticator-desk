import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { isTrustedReleaseInvocation, requireTrustedReleaseInvocation } from "./release-context.mjs";

const repository = "yuchen0x1/authenticator-desk";
const ref = "refs/tags/v1.2.3";
const workflowRef = repository + "/.github/workflows/release.yml@" + ref;

describe("isTrustedReleaseInvocation", () => {
  it("enforces the actual signing environment and matching ref name", () => {
    const env={GITHUB_EVENT_NAME:"push",GITHUB_REF:ref,GITHUB_REF_NAME:"v1.2.3",GITHUB_REPOSITORY:repository,GITHUB_WORKFLOW_REF:workflowRef};
    assert.equal(requireTrustedReleaseInvocation(env),"v1.2.3");
    for(const changes of [{GITHUB_REF_NAME:"v9.9.9"},{GITHUB_REPOSITORY:"someone/fork"},{GITHUB_EVENT_NAME:"pull_request"},{GITHUB_WORKFLOW_REF:repository+"/.github/workflows/ci.yml@"+ref},{GITHUB_REF:"refs/heads/main"}]) {
      assert.throws(()=>requireTrustedReleaseInvocation({...env,...changes}));
    }
    assert.throws(()=>requireTrustedReleaseInvocation({}));
  });
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
