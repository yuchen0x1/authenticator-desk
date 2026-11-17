import { describe,expect,it } from "vitest";
import { accountCreateSchema,accountInputSchema,importTextSchema } from "./schemas";

const valid={issuer:"GitHub",account_name:"alice@example.com",secret:"GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ",algorithm:"SHA1" as const,digits:6,kind:"totp" as const,period_secs:30,t0:"0",hotp_counter:"18446744073709551615",category_id:null,tags:["work"],notes:"",favorite:false};
describe("account and import validation",()=>{
  it("accepts decimal-string counters without JavaScript precision loss",()=>{expect(accountCreateSchema.parse(valid).hotp_counter).toBe("18446744073709551615");});
  it("requires a secret on creation but allows retaining it on edit",()=>{const edit={...valid,secret:null};expect(accountInputSchema.parse(edit).secret).toBeNull();expect(()=>accountCreateSchema.parse(edit)).toThrow();});
  it("rejects unsafe OTP settings and unknown fields before IPC",()=>{expect(()=>accountInputSchema.parse({...valid,digits:9})).toThrow();expect(()=>accountInputSchema.parse({...valid,unexpected:true})).toThrow();});
  it("bounds raw import text before it crosses IPC",()=>{expect(()=>importTextSchema.parse("")).toThrow();expect(()=>importTextSchema.parse("x".repeat(2*1024*1024+1))).toThrow();});
});
