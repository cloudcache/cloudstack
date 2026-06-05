import { z } from "zod";

export const appEnvVariablesZodModel = z.object({
  envVars: z.string(),
})

export type AppEnvVariablesModel = z.infer<typeof appEnvVariablesZodModel>;