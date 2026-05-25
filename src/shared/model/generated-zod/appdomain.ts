import * as z from "zod"

import { CompleteApp, RelatedAppModel } from "./index"

export const AppDomainModel = z.object({
  id: z.string(),
  hostname: z.string(),
  port: z.number().int(),
  useSsl: z.boolean(),
  redirectHttps: z.boolean(),
  appId: z.string(),
  createdAt: z.date(),
  updatedAt: z.date(),
})

export interface CompleteAppDomain extends z.infer<typeof AppDomainModel> {
  app: CompleteApp
}

/**
 * RelatedAppDomainModel contains all relations on your model in addition to the scalars
 *
 * NOTE: Lazy required in case of potential circular dependencies within schema
 */
export const RelatedAppDomainModel: z.ZodSchema<CompleteAppDomain> = z.lazy(() => AppDomainModel.extend({
  app: RelatedAppModel,
}))
