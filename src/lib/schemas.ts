import { z } from "zod";

export const accountInputSchema = z.object({
  issuer: z.string().trim().max(200),
  account_name: z.string().trim().min(1).max(320),
  secret: z.string().max(2048).nullable().optional(),
  algorithm: z.enum(["SHA1", "SHA256", "SHA512"]),
  digits: z.number().int().min(6).max(8),
  kind: z.enum(["totp", "hotp"]),
  period_secs: z.number().int().min(1).max(86400),
  t0: z.string().regex(/^-?\d{1,19}$/),
  hotp_counter: z.string().regex(/^\d{1,20}$/),
  category_id: z.string().nullable(),
  tags: z.array(z.string().min(1).max(100)).max(32),
  notes: z.string().max(16384),
  favorite: z.boolean(),
}).strict();

export const accountCreateSchema = accountInputSchema.extend({
  secret: z.string().min(1).max(2048),
});

export const importTextSchema = z.string().min(1).max(2 * 1024 * 1024);
// Base64 inflates the 2 MiB raw file bound by exactly 4/3.
export const importFileSchema = z.string().min(1).max(2800000);
