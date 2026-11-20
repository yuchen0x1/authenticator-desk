import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { verifyAuditReport } from "./verify-pnpm-audit.mjs";

const clean = () => ({advisories:{},metadata:{vulnerabilities:{info:0,low:0,moderate:0,high:0,critical:0}}});
describe("audit policy fails closed", () => {
  it("accepts the observed clean pnpm report shape", () => assert.equal(verifyAuditReport(clean()),true));
  for (const value of [null,[],{}, {advisories:{}}, {advisories:[],metadata:{vulnerabilities:{}}}, {error:{message:"registry unavailable"}}, {vulnerabilities:{}}]) {
    it("rejects missing/error/unknown report shape " + JSON.stringify(value), () => assert.throws(()=>verifyAuditReport(value)));
  }
  it("rejects any advisory, including the historical unscoped exception",()=>{
    const report=clean();report.advisories["one"]={github_advisory_id:"GHSA-fc5q-39xc-vjpf"};
    assert.throws(()=>verifyAuditReport(report));
  });
  it("rejects findings reported only by severity counts",()=>{
    const report=clean();report.metadata.vulnerabilities.high=1;
    assert.throws(()=>verifyAuditReport(report));
  });
  it("rejects corrupt or omitted counts",()=>{
    for(const value of [undefined,-1,"0",null,0.5]){
      const report=clean();report.metadata.vulnerabilities.low=value;
      assert.throws(()=>verifyAuditReport(report));
    }
  });
});
