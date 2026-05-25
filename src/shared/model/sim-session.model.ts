import { RoleAppPermission } from "@/shared/model/prisma-compat";
import { Session } from "next-auth";
import { RolePermissionEnum } from "./role-extended.model.ts";

export interface UserSession {
    email: string;
    userGroup?: UserGroupExtended;
}

export type UserGroupExtended = {
    name: string;
    id: string;
    canAccessBackups: boolean;
    roleProjectPermissions: {
        projectId: string;
        project: {
            apps: {
                id: string;
                name: string;
            }[];
        };
        createApps: boolean;
        deleteApps: boolean;
        writeApps: boolean;
        readApps: boolean;
        roleAppPermissions: RoleAppPermission[];
    }[];
};