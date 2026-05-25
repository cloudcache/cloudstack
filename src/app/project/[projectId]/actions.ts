'use server'

import { SuccessActionResult } from "@/shared/model/server-action-error-return.model";
import { getAuthUserSession, getBackendToken, isAuthorizedWriteForApp, saveFormAction, simpleAction } from "@/server/utils/action-wrapper.utils";
import { z } from "zod";
import { ServiceException } from "@/shared/model/service.exception.model";
import { backend } from "@/server/adapter/backend-api.adapter";

const createAppSchema = z.object({
    appName: z.string().min(1)
});

export const createApp = async (appName: string, projectId: string, appId?: string) =>
    saveFormAction({ appName }, createAppSchema, async (validatedData) => {
        await getAuthUserSession();
        const token = await getBackendToken();
        const result = await backend.apps.create(token, projectId, {
            name: validatedData.appName,
            display_name: validatedData.appName,
            source_type: 'CONTAINER',
        });
        return new SuccessActionResult(result, "App created successfully.");
    });

export const deleteApp = async (projectId: string, appId: string) =>
    simpleAction(async () => {
        await isAuthorizedWriteForApp(appId);
        const token = await getBackendToken();
        await backend.apps.delete(token, projectId, appId);
        return new SuccessActionResult(undefined, "App deleted successfully.");
    });
