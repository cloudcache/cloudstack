'use server'

import { getBackendToken, isAuthorizedReadForApp, isAuthorizedWriteForApp, simpleAction } from "@/server/utils/action-wrapper.utils";
import { ServerActionResult, SuccessActionResult } from "@/shared/model/server-action-error-return.model";
import { backend } from "@/server/adapter/backend-api.adapter";

export type DbToolIds = 'dbgate' | 'phpmyadmin' | 'pgadmin';

export const getDatabaseCredentials = async (appId: string) =>
    simpleAction(async () => {
        await isAuthorizedReadForApp(appId);
        const token = await getBackendToken();
        const app = await backend.apps.getById(token, appId);
        const credentials = await backend.apps.dbCredentials(token, app.project_id, appId);
        return new SuccessActionResult(credentials);
    }) as Promise<ServerActionResult<unknown, any>>;

export const getIsDbToolActive = async (appId: string, dbTool: DbToolIds) =>
    simpleAction(async () => {
        await isAuthorizedReadForApp(appId);
        const token = await getBackendToken();
        const app = await backend.apps.getById(token, appId);
        try {
            const tool = await backend.apps.dbTools.get(token, app.project_id, appId, dbTool);
            return new SuccessActionResult(tool.status === 'RUNNING');
        } catch {
            return new SuccessActionResult(false);
        }
    }) as Promise<ServerActionResult<unknown, boolean>>;

export const deployDbTool = async (appId: string, dbTool: DbToolIds) =>
    simpleAction(async () => {
        await isAuthorizedWriteForApp(appId);
        const token = await getBackendToken();
        const app = await backend.apps.getById(token, appId);
        await backend.apps.dbTools.deploy(token, app.project_id, appId, dbTool);
        return new SuccessActionResult();
    }) as Promise<ServerActionResult<unknown, void>>;

export const getLoginCredentialsForRunningDbTool = async (appId: string, dbTool: DbToolIds) =>
    simpleAction(async () => {
        await isAuthorizedWriteForApp(appId);
        const token = await getBackendToken();
        const app = await backend.apps.getById(token, appId);
        const tool = await backend.apps.dbTools.get(token, app.project_id, appId, dbTool);
        return new SuccessActionResult({
            url: tool.access_url ?? '',
            username: tool.username ?? '',
            password: tool.password ?? '',
        });
    }) as Promise<ServerActionResult<unknown, { url: string; username: string; password: string }>>;

export const deleteDbToolDeploymentForAppIfExists = async (appId: string, dbTool: DbToolIds) =>
    simpleAction(async () => {
        await isAuthorizedWriteForApp(appId);
        const token = await getBackendToken();
        const app = await backend.apps.getById(token, appId);
        try {
            await backend.apps.dbTools.delete(token, app.project_id, appId, dbTool);
        } catch {
            // Ignore if not found
        }
        return new SuccessActionResult();
    }) as Promise<ServerActionResult<unknown, void>>;

export const downloadDbGateFilesForApp = async (appId: string) =>
    simpleAction(async () => {
        // Deferred — DB tool config file download not yet implemented
        throw new Error('DB tool config download is not yet implemented');
    }) as Promise<ServerActionResult<unknown, string>>;
