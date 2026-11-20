export function isTrustedReleaseInvocation({ eventName, ref, repository, workflowRef } = {}) {
  const versionTag = /^refs\/tags\/v(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)(?:-rc\.[1-9]\d*)?$/;
  return eventName === "push" &&
    typeof ref === "string" && versionTag.test(ref) &&
    repository === "yuchen0x1/authenticator-desk" &&
    workflowRef === repository + "/.github/workflows/release.yml@" + ref;
}
