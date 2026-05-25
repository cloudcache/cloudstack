import { z } from "zod";

export const totpZodModel = z.object({
  totp: z.string().trim(),
})

export type TotpModel = z.infer<typeof totpZodModel>;