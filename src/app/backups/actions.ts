'use server'

import { isAuthorizedForBackups, getBackendToken, simpleAction } from "@/server/utils/action-wrapper.utils";
import { backend } from "@/server/adapter/backend-api.adapter";
import { ServerActionResult, SuccessActionResult } from "@/shared/model/server-action-error-return.model";

export const downloadBackup = async (s3TargetId: string, s3Key: string) =>
    simpleAction(async () => {
        await isAuthorizedForBackups();
        const token = await getBackendToken();
        // Not yet implemented in Rust backend — S3 client integration required
        try {
            const result = await backend.backups.download(token, s3TargetId, s3Key);
            return new SuccessActionResult(result as string, 'Starting download...');
        } catch {
            throw new Error('Backup file download is not yet implemented. Please use your S3 client directly.');
        }
    }) as Promise<ServerActionResult<any, string>>;

export const deleteBackup = async (s3TargetId: string, s3Key: string) =>
    simpleAction(async () => {
        await isAuthorizedForBackups();
        const token = await getBackendToken();
        // Not yet implemented in Rust backend — S3 client integration required
        try {
            await backend.backups.deleteFile(token, s3TargetId, s3Key);
            return new SuccessActionResult(undefined, 'Backup deleted. Refresh to see changes.');
        } catch {
            throw new Error('Backup file deletion is not yet implemented. Please use your S3 client directly.');
        }
    }) as Promise<ServerActionResult<any, string>>;
