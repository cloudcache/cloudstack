'use server'

import { ServerActionResult, SuccessActionResult } from "@/shared/model/server-action-error-return.model";
import { ServiceException } from "@/shared/model/service.exception.model";
import { getBackendToken, isAuthorizedWriteForApp, saveFormAction, simpleAction } from "@/server/utils/action-wrapper.utils";
import { backend } from "@/server/adapter/backend-api.adapter";
import { z } from "zod";

const sourceContainerSchema = z.object({
    container_image: z.string().min(1),
    container_registry_user: z.string().optional(),
    container_registry_pass: z.string().optional(),
});

const sourceGitSchema = z.object({
    git_url: z.string().min(1),
    git_branch: z.string().optional(),
    git_token: z.string().optional(),
    dockerfile_path: z.string().optional(),
});

export const saveGeneralAppSourceInfo = async (
    prevState: any,
    inputData: any,
    projectId: string,
    appId: string
) => {
    await isAuthorizedWriteForApp(appId);
    const token = await getBackendToken();

    if (inputData.source_type === 'GIT') {
        return saveFormAction(inputData, sourceGitSchema.extend({ source_type: z.literal('GIT') }), async (v) => {
            await backend.apps.update(token, projectId, appId, v);
        });
    } else if (inputData.source_type === 'CONTAINER') {
        return saveFormAction(inputData, sourceContainerSchema.extend({ source_type: z.literal('CONTAINER') }), async (v) => {
            await backend.apps.update(token, projectId, appId, v);
        });
    }
    return { status: 'error', message: 'Invalid source type' } as ServerActionResult<any, any>;
};

const scalingSchema = z.object({
    replicas: z.number().int().min(1),
    cpu_reservation_mcores: z.number().int().optional(),
    cpu_limit_mcores: z.number().int().optional(),
    mem_reservation_mb: z.number().int().optional(),
    mem_limit_mb: z.number().int().optional(),
});

export const saveGeneralAppRateLimits = async (prevState: any, inputData: z.infer<typeof scalingSchema>, projectId: string, appId: string) =>
    saveFormAction(inputData, scalingSchema, async (validatedData) => {
        if (validatedData.replicas < 1) {
            throw new ServiceException('Replica Count must be at least 1');
        }
        await isAuthorizedWriteForApp(appId);
        const token = await getBackendToken();
        await backend.apps.update(token, projectId, appId, validatedData);
    });

const containerConfigSchema = z.object({
    container_command: z.string().optional(),
    container_args: z.array(z.string()).optional(),
    working_dir: z.string().optional(),
    run_as_user: z.number().int().optional(),
    run_as_group: z.number().int().optional(),
    fs_group: z.number().int().optional(),
    privileged: z.boolean().optional(),
    read_only_root_fs: z.boolean().optional(),
});

export const saveGeneralAppContainerConfig = async (prevState: any, inputData: z.infer<typeof containerConfigSchema>, projectId: string, appId: string) =>
    saveFormAction(inputData, containerConfigSchema, async (validatedData) => {
        await isAuthorizedWriteForApp(appId);
        const token = await getBackendToken();
        await backend.apps.update(token, projectId, appId, validatedData);
    });
