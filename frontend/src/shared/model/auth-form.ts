import { z } from "zod";

export const authFormInputSchemaZod = z.object({
    email: z.string().trim().email(),
    password: z.string().trim().min(1)
});
export type AuthFormInputSchema = z.infer<typeof authFormInputSchemaZod>;

export const registgerFormInputSchemaZod = authFormInputSchemaZod.merge(z.object({
    qsHostname: z.string().trim().optional(),
}));
export type RegisterFormInputSchema = z.infer<typeof registgerFormInputSchemaZod>;

