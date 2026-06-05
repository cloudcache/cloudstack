import { RoleAppPermission, User, UserGroup } from "@/shared/model/prisma-compat";

export type RoleExtended = UserGroup & {
    roleAppPermissions: (RoleAppPermission & {
        app: {
            name: string;
        };
    })[];
}

export enum RolePermissionEnum {
    READ = 'READ',
    READWRITE = 'READWRITE'
}


export const adminRoleName = "admin";