'use server'

import { ServerActionResult, SuccessActionResult } from "@/shared/model/server-action-error-return.model";
import { getBackendToken, isAuthorizedReadForApp, isAuthorizedWriteForApp, simpleAction } from "@/server/utils/action-wrapper.utils";
import { backend } from "@/server/adapter/backend-api.adapter";

export const getDeploymentsForApp = async (projectId: string, appId: string) =>
    simpleAction(async () => {
        await isAuthorizedReadForApp(appId);
        const token = await getBackendToken();
        const deployments = await backend.apps.deployments(token, projectId, appId);
        return new SuccessActionResult(deployments);
    }) as Promise<ServerActionResult<unknown, any[]>>;

export const getPodsForApp = async (projectId: string, appId: string) =>
    simpleAction(async () => {
        await isAuthorizedReadForApp(appId);
        const token = await getBackendToken();
        const pods = await backend.apps.pods(token, projectId, appId);
        return new SuccessActionResult(pods);
    }) as Promise<ServerActionResult<unknown, any[]>>;

export const getRessourceDataApp = async (projectId: string, appId: string) =>
    simpleAction(async () => {
        await isAuthorizedReadForApp(appId);
        const token = await getBackendToken();
        const metrics = await backend.apps.metrics.current(token, projectId, appId);
        return new SuccessActionResult(metrics);
    }) as Promise<ServerActionResult<unknown, any>>;

export const createNewWebhookUrl = async (projectId: string, appId: string) =>
    simpleAction(async () => {
        await isAuthorizedWriteForApp(appId);
        const token = await getBackendToken();
        const result = await backend.apps.webhook.regenerate(token, projectId, appId);
        return new SuccessActionResult(result);
    }) as Promise<ServerActionResult<unknown, { webhook_id: string }>>;

export const deployApp = async (projectId: string, appId: string) =>
    simpleAction(async () => {
        await isAuthorizedWriteForApp(appId);
        const token = await getBackendToken();
        await backend.apps.deploy(token, projectId, appId);
        return new SuccessActionResult(undefined, 'Deployment triggered.');
    });

export const pauseApp = async (projectId: string, appId: string) =>
    simpleAction(async () => {
        await isAuthorizedWriteForApp(appId);
        const token = await getBackendToken();
        await backend.apps.pause(token, projectId, appId);
        return new SuccessActionResult(undefined, 'App paused.');
    });

export const resumeApp = async (projectId: string, appId: string) =>
    simpleAction(async () => {
        await isAuthorizedWriteForApp(appId);
        const token = await getBackendToken();
        await backend.apps.resume(token, projectId, appId);
        return new SuccessActionResult(undefined, 'App resumed.');
    });

export const getBuildLogs = async (projectId: string, appId: string, buildId: string) =>
    simpleAction(async () => {
        await isAuthorizedReadForApp(appId);
        // Build logs URL — client connects directly
        return new SuccessActionResult(
            backend.apps.buildLogsUrl(projectId, appId, buildId)
        );
    }) as Promise<ServerActionResult<unknown, string>>;

export const getLogsStreamUrl = async (projectId: string, appId: string): Promise<string> =>
    backend.apps.logsUrl(projectId, appId);

/** Compat alias used by deployments.tsx — fetches deployment history for an app by appId only */
export const getDeploymentsAndBuildsForApp = async (appId: string) =>
    simpleAction(async () => {
        await isAuthorizedReadForApp(appId);
        const token = await getBackendToken();
        const app = await backend.apps.getById(token, appId);
        const deployments = await backend.apps.deployments(token, app.project_id, appId);
        return new SuccessActionResult(deployments);
    }) as Promise<ServerActionResult<unknown, any[]>>;

/** Cancel a build job — buildId is the Rust build record ID */
export const deleteBuild = async (buildId: string) =>
    simpleAction(async () => {
        // buildId context: the component passes `item.buildJobName` which maps to the
        // Rust build ID in the new system. We look up projectId+appId via the build.
        // For now, this is a no-op stub — full migration happens in Phase C.
        return new SuccessActionResult(undefined, 'Build cancellation not yet implemented.');
    });
