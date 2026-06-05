import { stringToNumber } from "@/shared/utils/zod.utils";
import { z } from "zod";


const roleAppPermissionZodModle = z.object({
  appId: z.string(),
  permission: z.string(),
});

const RoleProjectPermissionSchema = z.object({
  projectId: z.string(),
  createApps: z.boolean(),
  deleteApps: z.boolean(),
  writeApps: z.boolean(),
  readApps: z.boolean(),
  roleAppPermissions: z.array(roleAppPermissionZodModle).optional().default([]),
});

// Schema for UserRole.
export const roleEditZodModel = z.object({
  id: z.string().trim().optional(),
  name: z.string().trim().min(1),
  canAccessBackups: z.boolean().optional().default(false),
  roleProjectPermissions: z.array(RoleProjectPermissionSchema).optional().default([]),
});


export type RoleEditModel = z.infer<typeof roleEditZodModel>;
