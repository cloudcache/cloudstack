import * as z from "zod"

import { CompleteApp, RelatedAppModel } from "./index"

export const AppPortsModel = z.object({
  id: z.string(),
  appId: z.string(),
  port: z.number().int(),
  createdAt: z.date(),
  updatedAt: z.date(),
})

export interface CompleteAppPorts extends z.infer<typeof AppPortsModel> {
  app: CompleteApp
}

/**
 * RelatedAppPortsModel contains all relations on your model in addition to the scalars
 *
 * NOTE: Lazy required in case of potential circular dependencies within schema
 */
export const RelatedAppPortsModel: z.ZodSchema<CompleteAppPorts> = z.lazy(() => AppPortsModel.extend({
  app: RelatedAppModel,
}))
