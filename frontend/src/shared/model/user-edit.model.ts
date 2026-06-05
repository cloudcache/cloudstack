import { z } from "zod";

export const userEditZodModel = z.object({
  id: z.string().trim().optional(),
  username: z.string().trim().min(1),
  email: z.string().trim().min(1),
  newPassword: z.string().optional(),
  userGroupId: z.string().trim().nullable(),
})

export type UserEditModel = z.infer<typeof userEditZodModel>;
