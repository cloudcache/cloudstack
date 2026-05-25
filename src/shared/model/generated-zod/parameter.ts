import * as z from "zod"


export const ParameterModel = z.object({
  name: z.string(),
  value: z.string(),
  createdAt: z.date(),
  updatedAt: z.date(),
})
