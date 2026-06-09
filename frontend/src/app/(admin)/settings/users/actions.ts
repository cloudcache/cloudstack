'use server'

import { SuccessActionResult } from "@/shared/model/server-action-error-return.model";
import { ServiceException } from "@/shared/model/service.exception.model";
import { getAdminUserSession, getAuthUserSession, getBackendToken, saveFormAction, simpleAction } from "@/server/utils/action-wrapper.utils";
import { z } from "zod";
import { backend } from "@/server/adapter/backend-api.adapter";
import { roleEditZodModel } from "@/shared/model/role-edit.model";

const createUserSchema = z.object({
    username: z.string().min(1),
    email: z.string().email(),
    password: z.string().min(8),
    is_global_admin: z.boolean().optional(),
});

const updateUserSchema = z.object({
    id: z.string(),
    username: z.string().min(1).optional(),
    email: z.string().email().optional(),
    is_global_admin: z.boolean().optional(),
    new_password: z.string().min(8).optional(),
});

export const saveUser = async (prevState: any, inputData: any) => {
    const normalizedInput = {
        ...inputData,
        password: inputData.newPassword,
        new_password: inputData.newPassword,
    };

    if (inputData.id) {
        return saveFormAction(normalizedInput, updateUserSchema, async (validatedData) => {
            const session = await getAdminUserSession();
            const token = await getBackendToken();
            if (validatedData.email === session.email) {
                throw new ServiceException('Please edit your profile in the profile settings');
            }
            await backend.adminUsers.update(token, validatedData.id, {
                username: validatedData.username,
                email: validatedData.email,
                is_global_admin: validatedData.is_global_admin,
            });
            if (validatedData.new_password) {
                await backend.adminUsers.resetPassword(token, validatedData.id, validatedData.new_password);
            }
            return new SuccessActionResult();
        });
    }

    return saveFormAction(normalizedInput, createUserSchema, async (validatedData) => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminUsers.create(token, {
            username: validatedData.username,
            email: validatedData.email,
            password: validatedData.password,
            is_global_admin: validatedData.is_global_admin ?? false,
        });
        return new SuccessActionResult();
    });
};

export const deleteUser = async (userId: string) =>
    simpleAction(async () => {
        const session = await getAdminUserSession();
        const token = await getBackendToken();
        const user = await backend.adminUsers.get(token, userId) as { email?: string };
        if (user.email === session.email) {
            throw new ServiceException('You cannot delete your own user');
        }
        await backend.adminUsers.delete(token, userId);
        return new SuccessActionResult();
    });

export const assignRoleToUsers = async (userIds: string[], role: string) => {
    // TODO: implement bulk role assignment
    throw new Error("Bulk role assignment is not yet implemented.");
};

export const saveRole = async (prevState: any, inputData: any) =>
    saveFormAction(inputData, roleEditZodModel, async () => {
        await getAdminUserSession();
        throw new ServiceException('User groups are not implemented by the Rust backend yet.');
    });

export const deleteRole = async (roleId: string) =>
    simpleAction(async () => {
        await getAdminUserSession();
        throw new ServiceException('User groups are not implemented by the Rust backend yet.');
    });
