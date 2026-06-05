/**
 * Prisma compatibility shim.
 *
 * These interfaces replace the generated types that were previously imported
 * from "@prisma/client". Defined with an index signature so components that
 * still access arbitrary fields continue to compile without change.
 *
 * Once each component is fully migrated to use backend snake_case shapes these
 * can be tightened or removed.
 */

// ── Auth / user management ────────────────────────────────────────────────────

export interface User {
    id: string;
    email: string;
    [key: string]: any;
}

export interface UserGroup {
    id: string;
    name: string;
    canAccessBackups: boolean;
    [key: string]: any;
}

export interface RoleAppPermission {
    id: string;
    roleProjectPermissionId: string;
    appId: string;
    permission: string;
    [key: string]: any;
}

// ── Projects / Apps ───────────────────────────────────────────────────────────

export interface Project {
    id: string;
    name: string;
    createdAt?: Date | string;
    updatedAt?: Date | string;
    [key: string]: any;
}

export interface App {
    id: string;
    name: string;
    projectId: string;
    appDomains?: AppDomain[];
    appPorts?: AppPort[];
    [key: string]: any;
}

// ── S3 ────────────────────────────────────────────────────────────────────────

export interface S3Target {
    id: string;
    name: string;
    createdAt?: Date | string;
    updatedAt?: Date | string;
    [key: string]: any;
}

// ── App sub-models ────────────────────────────────────────────────────────────

export interface AppVolume {
    id: string;
    appId: string;
    containerMountPath: string;
    [key: string]: any;
}

export interface AppBasicAuth {
    id: string;
    appId: string;
    username: string;
    [key: string]: any;
}

export interface AppFileMount {
    id: string;
    appId: string;
    filename?: string;
    containerMountPath: string;
    [key: string]: any;
}

export interface AppDomain {
    id: string;
    appId: string;
    hostname: string;
    [key: string]: any;
}

export interface AppPort {
    id: string;
    appId: string;
    port: number;
    [key: string]: any;
}

export interface VolumeBackup {
    id: string;
    volumeId?: string;
    [key: string]: any;
}

// ── Prisma namespace — only AppFileMountUncheckedCreateInput is used ──────────

export namespace Prisma {
    export interface AppFileMountUncheckedCreateInput {
        containerMountPath: string;
        content: string;
        appId: string;
        [key: string]: any;
    }
}
