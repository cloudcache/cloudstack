import { z } from "zod";

export const profilePasswordChangeZodModel = z.object({
  oldPassword: z.string().trim().min(1),
  newPassword: z.string().trim().min(6),
  confirmNewPassword: z.string().trim().min(6)
})

export type ProfilePasswordChangeModel = z.infer<typeof profilePasswordChangeZodModel>;