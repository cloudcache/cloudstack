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

// Deploy an app from a template, after the user has filled in the setup form
// (and, for P2a, picked managed-service bindings). The payload is consumed
// by the new backend endpoint at POST /projects/:pid/apps/from-template.
export const createAppFromTemplate = async (
    _state: any,
    payload: {
        template_id: string;
        app_name: string;
        display_name?: string;
        bindings?: any[];
        input_overrides?: Record<string, any>;
    },
    projectId?: string,
) => {
    if (!projectId) {
        throw new ServiceException("projectId is required");
    }
    return simpleAction(async () => {
        await getAuthUserSession();
        const token = await getBackendToken();
        const result = await backend.apps.createFromTemplate(token, projectId, payload);
        return new SuccessActionResult(result, "App created from template.");
    });
};

// Pre-load resources the deploy dialog needs: list of database_instances
// (existing managed DBs the project can bind to), database_clusters (for
// provision-new), and s3_targets.
export const loadBindingChoices = async (projectId: string) => {
    await getAuthUserSession();
    const token = await getBackendToken();
    const [databases, dbClusters, s3Targets] = await Promise.all([
        backend.databases.list(token, projectId).catch(() => []),
        backend.databases.listClusters(token).catch(() => []),
        backend.s3Targets.list(token).catch(() => []),
    ]);
    return { databases, dbClusters, s3Targets };
};

export const deleteApp = async (projectId: string, appId: string) =>
    simpleAction(async () => {
        await isAuthorizedWriteForApp(appId);
        const token = await getBackendToken();
        await backend.apps.delete(token, projectId, appId);
        return new SuccessActionResult(undefined, "App deleted successfully.");
    });

// Templates: list everything the caller can see (PUBLIC + own PRIVATE + ORG).
// Used by the "create from template" dialog.
export const listTemplates = async () => {
    await getAuthUserSession();
    const token = await getBackendToken();
    return backend.templates.list(token);
};
