import { adminRoleName, RolePermissionEnum } from "@/shared/model/role-extended.model.ts";
import { UserSession } from "@/shared/model/sim-session.model";

export class UserGroupUtils {

    static sessionHasReadAccessToProject(session: UserSession, projectId: string) {
        if (this.isAdmin(session)) {
            return true;
        }

        const projectPermission = UserGroupUtils.getProjectPermissionForProjectId(session, projectId);
        if (!projectPermission) {
            return false;
        }

        if (projectPermission.roleAppPermissions.length > 0) {
            return true;
        }

        return projectPermission.readApps;
    }

    private static getProjectPermissionForProjectId(session: UserSession, projectId: string) {
        return session.userGroup?.roleProjectPermissions?.find((projectPermission) => projectPermission.projectId === projectId);
    }

    private static getProjectPermissionForAppId(session: UserSession, appId: string) {
        return session.userGroup?.roleProjectPermissions?.find((projectPermission) => {
            return projectPermission.project?.apps?.some(app => app.id === appId);
        });
    }

    static getRolePermissionForApp(session: UserSession, appId: string): RolePermissionEnum | null {
        if (this.isAdmin(session)) {
            return RolePermissionEnum.READWRITE;
        }
        const projectPermission = this.getProjectPermissionForAppId(session, appId);
        if (!projectPermission) {
            return null;
        }
        if (projectPermission?.roleAppPermissions.length > 0) {
            return (projectPermission.roleAppPermissions.find(app => app.appId === appId)?.permission ?? null) as RolePermissionEnum | null;
        }
        // If no roleAppPermissions are defined, we fallback to the projectPermission
        if (projectPermission.writeApps) {
            return RolePermissionEnum.READWRITE;
        }
        if (projectPermission.readApps) {
            return RolePermissionEnum.READ;
        }
        return null;
    }

    static sessionHasAccessToBackups(session: UserSession) {
        if (this.isAdmin(session)) {
            return true;
        }
        return !!session.userGroup?.canAccessBackups;
    }

    static sessionCanCreateNewAppsForProject(session: UserSession, projectId: string) {
        if (this.isAdmin(session)) {
            return true;
        }
        const projectPermission = this.getProjectPermissionForProjectId(session, projectId);
        if (!projectPermission) {
            return false;
        }
        return !!projectPermission.createApps;
    }

    static sessionCanDeleteAppsForProject(session: UserSession, projectId: string) {
        if (this.isAdmin(session)) {
            return true;
        }
        const projectPermission = this.getProjectPermissionForProjectId(session, projectId);
        if (!projectPermission) {
            return false;
        }
        return !!projectPermission.deleteApps;
    }

    static sessionIsReadOnlyForApp(session: UserSession, appId: string) {
        if (this.isAdmin(session)) {
            return false;
        }
        const rolePermission = this.getRolePermissionForApp(session, appId);
        const roleHasReadAccessForApp = rolePermission === RolePermissionEnum.READ;
        const roleHasWriteAccessForApp = rolePermission === RolePermissionEnum.READWRITE;
        return !!roleHasReadAccessForApp && !roleHasWriteAccessForApp;
    }

    static sessionHasReadAccessForApp(session: UserSession, appId: string) {
        if (this.isAdmin(session)) {
            return true;
        }
        const rolePermission = this.getRolePermissionForApp(session, appId);
        const roleHasReadAccessForApp = rolePermission === RolePermissionEnum.READ || rolePermission === RolePermissionEnum.READWRITE;
        return !!roleHasReadAccessForApp;
    }

    static sessionHasWriteAccessForApp(session: UserSession, appId: string) {
        if (this.isAdmin(session)) {
            return true;
        }
        const rolePermission = this.getRolePermissionForApp(session, appId);
        const roleHasReadAccessForApp = rolePermission === RolePermissionEnum.READWRITE;
        return roleHasReadAccessForApp;
    }

    static isAdmin(session: UserSession) {
        return session.userGroup?.name === adminRoleName;
    }
}