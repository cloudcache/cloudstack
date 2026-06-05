import * as z from "zod"

import { CompleteApp, RelatedAppModel, CompleteRoleProjectPermission, RelatedRoleProjectPermissionModel } from "./index"

export const RoleAppPermissionModel = z.object({
  id: z.string(),
  appId: z.string(),
  permission: z.string(),
  roleProjectPermissionId: z.string().nullish(),
  createdAt: z.date(),
  updatedAt: z.date(),
})

export interface CompleteRoleAppPermission extends z.infer<typeof RoleAppPermissionModel> {
  app: CompleteApp
  roleProjectPermission?: CompleteRoleProjectPermission | null
}

/**
 * RelatedRoleAppPermissionModel contains all relations on your model in addition to the scalars
 *
 * NOTE: Lazy required in case of potential circular dependencies within schema
 */
export const RelatedRoleAppPermissionModel: z.ZodSchema<CompleteRoleAppPermission> = z.lazy(() => RoleAppPermissionModel.extend({
  app: RelatedAppModel,
  roleProjectPermission: RelatedRoleProjectPermissionModel.nullish(),
}))
