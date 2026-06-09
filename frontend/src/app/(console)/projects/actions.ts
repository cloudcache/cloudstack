'use server'

import { SuccessActionResult } from "@/shared/model/server-action-error-return.model";
import { getAdminUserSession, getAuthUserSession, getBackendToken, saveFormAction, simpleAction } from "@/server/utils/action-wrapper.utils";
import { z } from "zod";
import { ServiceException } from "@/shared/model/service.exception.model";
import { backend } from "@/server/adapter/backend-api.adapter";

const createProjectSchema = z.object({
    projectName: z.string().min(1),
    projectId: z.string().optional()
});

export const createProject = async (projectName: string, projectId?: string) =>
    saveFormAction({ projectName, projectId }, createProjectSchema, async (validatedData) => {
        const token = await getBackendToken();
        await backend.projects.create(token, {
            name: validatedData.projectName,
            display_name: validatedData.projectName,
        });
        return new SuccessActionResult(undefined, "Project created successfully.");
    });

export const deleteProject = async (projectId: string) =>
    simpleAction(async () => {
        const token = await getBackendToken();
        await backend.adminProjects.delete(token, projectId);
        return new SuccessActionResult(undefined, "Project deleted successfully.");
    });
