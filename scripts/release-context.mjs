export function isTrustedReleaseInvocation({ eventName, ref, repository, workflowRef } = {}) {
  const versionTag = /^refs\/tags\/v(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)(?:-rc\.[1-9]\d*)?$/;
  return eventName === "push" &&
    typeof ref === "string" && versionTag.test(ref) &&
    repository === "yuchen0x1/authenticator-desk" &&
    workflowRef === repository + "/.github/workflows/release.yml@" + ref;
}

export function requireTrustedReleaseInvocation(env = process.env) {
  const { GITHUB_EVENT_NAME, GITHUB_REF, GITHUB_REF_NAME, GITHUB_REPOSITORY, GITHUB_WORKFLOW_REF } = env;
  if (!isTrustedReleaseInvocation({
    eventName: GITHUB_EVENT_NAME,
    ref: GITHUB_REF,
    repository: GITHUB_REPOSITORY,
    workflowRef: GITHUB_WORKFLOW_REF,
  }) || GITHUB_REF_NAME !== GITHUB_REF.slice("refs/tags/".length)) {
    throw new Error("Signing and publishing require this repository's Release workflow on a version tag push");
  }
  return GITHUB_REF_NAME;
}
