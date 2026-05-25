'use server'

import { SuccessActionResult } from "@/shared/model/server-action-error-return.model";
import { ServiceException } from "@/shared/model/service.exception.model";
import { getAdminUserSession, getAuthUserSession, getBackendToken, saveFormAction, simpleAction } from "@/server/utils/action-wrapper.utils";
import { z } from "zod";
import { backend } from "@/server/adapter/backend-api.adapter";

const createUserSchema = z.object({
    username: z.string().min(1),
    email: z.string().email(),
    password: z.string().min(8),
    is_global_admin: z.boolean().optional(),
});

const updateUserSchema = z.object({
    id: z.string(),
    email: z.string().email().optional(),
    is_global_admin: z.boolean().optional(),
    new_password: z.string().min(8).optional(),
});

export const saveUser = async (prevState: any, inputData: any) =>
    saveFormAction(inputData, inputData.id ? updateUserSchema : createUserSchema, async (validatedData) => {
        const session = await getAdminUserSession();
        const token = await getBackendToken();

        if ('id' in validatedData && validatedData.id) {
            // Update existing user
            if (validatedData.email === session.email) {
                throw new ServiceException('Please edit your profile in the profile settings');
            }
            await backend.adminUsers.update(token, validatedData.id, {
                email: validatedData.email,
                is_global_admin: (validatedData as any).is_global_admin,
            });
            if ((validatedData as any).new_password) {
                await backend.adminUsers.resetPassword(token, validatedData.id, (validatedData as any).new_password);
            }
        } else {
            // Create new user
            await backend.adminUsers.create(token, {
                username: (validatedData as any).username,
                email: (validatedData as any).email,
                password: (validatedData as any).password,
                is_global_admin: (validatedData as any).is_global_admin ?? false,
            });
        }
        return new SuccessActionResult();
    });

export const deleteUser = async (userId: string) =>
    simpleAction(async () => {
        const session = await getAdminUserSession();
        const token = await getBackendToken();
        const user = await backend.adminUsers.get(token, userId);
        if (user.email === session.email) {
            throw new ServiceException('You cannot delete your own user');
        }
        await backend.adminUsers.delete(token, userId);
        return new SuccessActionResult();
    });
