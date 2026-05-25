import * as z from "zod"

import { CompleteApp, RelatedAppModel } from "./index"

export const AppPortModel = z.object({
  id: z.string(),
  appId: z.string(),
  port: z.number().int(),
  createdAt: z.date(),
  updatedAt: z.date(),
})

export interface CompleteAppPort extends z.infer<typeof AppPortModel> {
  app: CompleteApp
}

/**
 * RelatedAppPortModel contains all relations on your model in addition to the scalars
 *
 * NOTE: Lazy required in case of potential circular dependencies within schema
 */
export const RelatedAppPortModel: z.ZodSchema<CompleteAppPort> = z.lazy(() => AppPortModel.extend({
  app: RelatedAppModel,
}))
