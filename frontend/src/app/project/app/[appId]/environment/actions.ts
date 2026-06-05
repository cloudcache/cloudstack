'use server'

import { ServerActionResult, SuccessActionResult } from "@/shared/model/server-action-error-return.model";
import { getBackendToken, isAuthorizedReadForApp, isAuthorizedWriteForApp, simpleAction } from "@/server/utils/action-wrapper.utils";
import { ServiceException } from "@/shared/model/service.exception.model";
import { backend } from "@/server/adapter/backend-api.adapter";
import { z } from "zod";

const setEnvSchema = z.object({
    key: z.string().min(1),
    value: z.string(),
    is_secret: z.boolean().optional(),
});

export const listEnvVars = async (projectId: string, appId: string) =>
    simpleAction(async () => {
        await isAuthorizedReadForApp(appId);
        const token = await getBackendToken();
        const vars = await backend.apps.env.list(token, projectId, appId);
        return new SuccessActionResult(vars);
    }) as Promise<ServerActionResult<unknown, any[]>>;

export const setEnvVar = async (projectId: string, appId: string, key: string, value: string, isSecret?: boolean) =>
    simpleAction(async () => {
        await isAuthorizedWriteForApp(appId);
        const token = await getBackendToken();
        await backend.apps.env.set(token, projectId, appId, { key, value, is_secret: isSecret ?? false });
        return new SuccessActionResult(undefined, 'Environment variable saved.');
    });

export const deleteEnvVar = async (projectId: string, appId: string, envId: string) =>
    simpleAction(async () => {
        await isAuthorizedWriteForApp(appId);
        const token = await getBackendToken();
        await backend.apps.env.delete(token, projectId, appId, envId);
        return new SuccessActionResult(undefined, 'Environment variable deleted.');
    });

export const saveEnvVariables = async (prevState: any, inputData: any, appId: string) => {
    // TODO: migrate to new env API - this is a legacy bulk-save action
    throw new Error("saveEnvVariables is not yet implemented. Use setEnvVar instead.");
};
