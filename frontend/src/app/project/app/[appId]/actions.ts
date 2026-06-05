'use server'

import { SuccessActionResult } from "@/shared/model/server-action-error-return.model";
import { getBackendToken, isAuthorizedReadForApp, isAuthorizedWriteForApp, simpleAction } from "@/server/utils/action-wrapper.utils";
import { backend } from "@/server/adapter/backend-api.adapter";

export const deploy = async (projectId: string, appId: string, forceBuild = false) =>
    simpleAction(async () => {
        await isAuthorizedWriteForApp(appId);
        const token = await getBackendToken();
        await backend.apps.deploy(token, projectId, appId);
        return new SuccessActionResult(undefined, 'Successfully started deployment.');
    });

export const stopApp = async (projectId: string, appId: string) =>
    simpleAction(async () => {
        await isAuthorizedWriteForApp(appId);
        const token = await getBackendToken();
        await backend.apps.scale(token, projectId, appId, 0);
        return new SuccessActionResult(undefined, 'Successfully stopped app.');
    });

export const startApp = async (projectId: string, appId: string) =>
    simpleAction(async () => {
        await isAuthorizedWriteForApp(appId);
        const token = await getBackendToken();
        await backend.apps.resume(token, projectId, appId);
        return new SuccessActionResult(undefined, 'Successfully started app.');
    });

export const getLatestAppEvents = async (appId: string) =>
    simpleAction(async () => {
        await isAuthorizedReadForApp(appId);
        const token = await getBackendToken();
        const app = await backend.apps.getById(token, appId);
        return await backend.apps.listEvents(token, app.project_id, appId);
    });
