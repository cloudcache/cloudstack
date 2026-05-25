'use server'

import { ServerActionResult, SuccessActionResult } from "@/shared/model/server-action-error-return.model";
import { getBackendToken, isAuthorizedReadForApp, isAuthorizedWriteForApp, saveFormAction, simpleAction } from "@/server/utils/action-wrapper.utils";
import { z } from "zod";
import { backend } from "@/server/adapter/backend-api.adapter";
import { fileMountEditZodModel } from "@/shared/model/file-mount-edit.model";

// ── Managed volumes ────────────────────────────────────────────────────────────

const managedVolumeCreateSchema = z.object({
    appId: z.string().min(1),
    name: z.string().min(1),
    container_mount_path: z.string().min(1),
    share_with_others: z.boolean().optional(),
    shared_volume_id: z.string().nullish(),
});

const managedVolumeUpdateSchema = z.object({
    appId: z.string().min(1),
    volumeId: z.string().min(1),
    name: z.string().optional(),
    container_mount_path: z.string().optional(),
    share_with_others: z.boolean().optional(),
});

export const createManagedVolume = async (prevState: any, inputData: z.infer<typeof managedVolumeCreateSchema>) =>
    saveFormAction(inputData, managedVolumeCreateSchema, async (validatedData) => {
        await isAuthorizedWriteForApp(validatedData.appId);
        const token = await getBackendToken();
        const app = await backend.apps.getById(token, validatedData.appId);
        await backend.apps.managedVolumes.create(token, app.project_id, validatedData.appId, {
            name: validatedData.name,
            container_mount_path: validatedData.container_mount_path,
            share_with_others: validatedData.share_with_others,
            shared_volume_id: validatedData.shared_volume_id ?? undefined,
        });
        return new SuccessActionResult();
    });

export const updateManagedVolume = async (prevState: any, inputData: z.infer<typeof managedVolumeUpdateSchema>) =>
    saveFormAction(inputData, managedVolumeUpdateSchema, async (validatedData) => {
        await isAuthorizedWriteForApp(validatedData.appId);
        const token = await getBackendToken();
        const app = await backend.apps.getById(token, validatedData.appId);
        await backend.apps.managedVolumes.update(token, app.project_id, validatedData.appId, validatedData.volumeId, {
            name: validatedData.name,
            container_mount_path: validatedData.container_mount_path,
            share_with_others: validatedData.share_with_others,
        });
        return new SuccessActionResult();
    });

export const deleteManagedVolume = async (appId: string, volumeId: string) =>
    simpleAction(async () => {
        await isAuthorizedWriteForApp(appId);
        const token = await getBackendToken();
        const app = await backend.apps.getById(token, appId);
        await backend.apps.managedVolumes.delete(token, app.project_id, appId, volumeId);
        return new SuccessActionResult(undefined, 'Successfully deleted volume');
    });

export const getShareableVolumes = async (appId: string) =>
    simpleAction(async () => {
        await isAuthorizedReadForApp(appId);
        const token = await getBackendToken();
        const app = await backend.apps.getById(token, appId);
        const volumes = await backend.apps.managedVolumes.shareable(token, app.project_id, appId);
        return new SuccessActionResult(volumes);
    }) as Promise<ServerActionResult<unknown, unknown[]>>;

export const getManagedVolumeUsage = async (appId: string, volumeId: string) =>
    simpleAction(async () => {
        await isAuthorizedReadForApp(appId);
        const token = await getBackendToken();
        const app = await backend.apps.getById(token, appId);
        const usage = await backend.apps.managedVolumes.usage(token, app.project_id, appId, volumeId);
        return new SuccessActionResult(usage);
    }) as Promise<ServerActionResult<unknown, { host_path: string; usage_bytes: number }>>;

// ── Volume backup schedules ────────────────────────────────────────────────────

const backupCreateSchema = z.object({
    appId: z.string().min(1),
    volumeId: z.string().min(1),
    s3_target_id: z.string().min(1),
    cron_expr: z.string().min(1),
    retention_days: z.number().int().min(1).optional(),
    use_db_backup: z.boolean().optional(),
});

const backupUpdateSchema = z.object({
    appId: z.string().min(1),
    volumeId: z.string().min(1),
    backupId: z.string().min(1),
    cron_expr: z.string().optional(),
    retention_days: z.number().int().min(1).optional(),
    is_active: z.boolean().optional(),
});

export const createVolumeBackup = async (prevState: any, inputData: z.infer<typeof backupCreateSchema>) =>
    saveFormAction(inputData, backupCreateSchema, async (validatedData) => {
        await isAuthorizedWriteForApp(validatedData.appId);
        const token = await getBackendToken();
        const app = await backend.apps.getById(token, validatedData.appId);
        await backend.apps.managedVolumes.backups.create(
            token, app.project_id, validatedData.appId, validatedData.volumeId,
            {
                s3_target_id: validatedData.s3_target_id,
                cron_expr: validatedData.cron_expr,
                retention_days: validatedData.retention_days,
                use_db_backup: validatedData.use_db_backup,
            }
        );
        return new SuccessActionResult();
    });

export const updateVolumeBackup = async (prevState: any, inputData: z.infer<typeof backupUpdateSchema>) =>
    saveFormAction(inputData, backupUpdateSchema, async (validatedData) => {
        await isAuthorizedWriteForApp(validatedData.appId);
        const token = await getBackendToken();
        const app = await backend.apps.getById(token, validatedData.appId);
        await backend.apps.managedVolumes.backups.update(
            token, app.project_id, validatedData.appId, validatedData.volumeId, validatedData.backupId,
            {
                cron_expr: validatedData.cron_expr,
                retention_days: validatedData.retention_days,
                is_active: validatedData.is_active,
            }
        );
        return new SuccessActionResult();
    });

export const deleteVolumeBackup = async (appId: string, volumeId: string, backupId: string) =>
    simpleAction(async () => {
        await isAuthorizedWriteForApp(appId);
        const token = await getBackendToken();
        const app = await backend.apps.getById(token, appId);
        await backend.apps.managedVolumes.backups.delete(token, app.project_id, appId, volumeId, backupId);
        return new SuccessActionResult(undefined, 'Successfully deleted backup schedule');
    });

export const runVolumeBackup = async (appId: string, volumeId: string, backupId: string) =>
    simpleAction(async () => {
        await isAuthorizedWriteForApp(appId);
        const token = await getBackendToken();
        const app = await backend.apps.getById(token, appId);
        await backend.apps.managedVolumes.backups.run(token, app.project_id, appId, volumeId, backupId);
        return new SuccessActionResult(undefined, 'Backup triggered successfully');
    });

// ── File mounts ────────────────────────────────────────────────────────────────

const actionFileMountSchema = fileMountEditZodModel.merge(z.object({
    appId: z.string().min(1),
    id: z.string().nullish(),
    filename: z.string().min(1).optional(),
}));

export const saveFileMount = async (prevState: any, inputData: z.infer<typeof actionFileMountSchema>) =>
    saveFormAction(inputData, actionFileMountSchema, async (validatedData) => {
        await isAuthorizedWriteForApp(validatedData.appId);
        const token = await getBackendToken();
        const app = await backend.apps.getById(token, validatedData.appId);
        if (validatedData.id) {
            // Update: delete + recreate (Rust has no PUT for file mounts)
            await backend.apps.deleteFile(token, app.project_id, validatedData.appId, validatedData.id);
        }
        await backend.apps.setFile(token, app.project_id, validatedData.appId, {
            filename: validatedData.filename ?? 'config',
            mount_path: validatedData.containerMountPath,
            content: validatedData.content ?? '',
        });
    });

export const deleteFileMount = async (appIdOrFileMountId: string, fileMountId?: string) =>
    simpleAction(async () => {
        // Handle both old (fileMountId only) and new (appId, fileMountId) signatures
        const resolvedAppId = fileMountId ? appIdOrFileMountId : undefined;
        const resolvedFileMountId = fileMountId ?? appIdOrFileMountId;

        if (!resolvedAppId) {
            // Legacy: we don't have appId, can't resolve — skip
            throw new Error('deleteFileMount requires appId as first argument in the new system');
        }
        await isAuthorizedWriteForApp(resolvedAppId);
        const token = await getBackendToken();
        const app = await backend.apps.getById(token, resolvedAppId);
        await backend.apps.deleteFile(token, app.project_id, resolvedAppId, resolvedFileMountId);
        return new SuccessActionResult(undefined, 'Successfully deleted file mount');
    });

// ── Deferred / not yet implemented ────────────────────────────────────────────

export const restoreVolumeFromZip = async (prevState: any, inputData: FormData, volumeId: string) =>
    simpleAction(async () => {
        throw new Error('Volume restore from zip is not yet implemented');
    });

export const downloadPvcData = async (volumeId: string) =>
    simpleAction(async () => {
        throw new Error('Volume data download is not yet implemented');
    }) as Promise<ServerActionResult<any, string>>;

export const openFileBrowserForVolume = async (volumeId: string) =>
    simpleAction(async () => {
        throw new Error('File browser is not yet implemented');
    }) as Promise<ServerActionResult<any, { url: string; password: string }>>;

// Legacy compat aliases used by storages.tsx
export const saveVolume = createManagedVolume;
export const deleteVolume = async (volumeId: string) =>
    simpleAction(async () => {
        throw new Error('Use deleteManagedVolume(appId, volumeId) instead');
    });
export const getPvcUsage = getManagedVolumeUsage;
export const saveBackupVolume = createVolumeBackup;
export const deleteBackupVolume = async (backupVolumeId: string) =>
    simpleAction(async () => {
        throw new Error('Use deleteVolumeBackup(appId, volumeId, backupId) instead');
    });
export const runBackupVolumeSchedule = async (backupVolumeId: string) =>
    simpleAction(async () => {
        throw new Error('Use runVolumeBackup(appId, volumeId, backupId) instead');
    });
