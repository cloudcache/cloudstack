'use server'

import { getBackendToken, isAuthorizedReadForApp, simpleAction } from "@/server/utils/action-wrapper.utils";
import { ServerActionResult, SuccessActionResult } from "@/shared/model/server-action-error-return.model";
import { backend } from "@/server/adapter/backend-api.adapter";

export const getDatabaseCredentials = async (appId: string) =>
    simpleAction(async () => {
        await isAuthorizedReadForApp(appId);
        const token = await getBackendToken();
        const app = await backend.apps.getById(token, appId);
        const credentials = await backend.apps.dbCredentials(token, app.project_id, appId);
        return new SuccessActionResult(credentials);
    }) as Promise<ServerActionResult<unknown, any>>;
