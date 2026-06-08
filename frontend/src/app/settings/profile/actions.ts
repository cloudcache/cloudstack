'use server'

import { ServiceException } from "@/shared/model/service.exception.model";
import { SuccessActionResult } from "@/shared/model/server-action-error-return.model";
import { getAuthUserSession, getBackendToken, saveFormAction, simpleAction } from "@/server/utils/action-wrapper.utils";
import { z } from "zod";
import { backend } from "@/server/adapter/backend-api.adapter";

const changePasswordSchema = z.object({
    oldPassword: z.string().min(1),
    newPassword: z.string().min(8),
    confirmNewPassword: z.string().min(1),
});

export const changePassword = async (prevState: any, inputData: z.infer<typeof changePasswordSchema>) =>
    saveFormAction(inputData, changePasswordSchema, async (validatedData) => {
        if (validatedData.newPassword !== validatedData.confirmNewPassword) {
            throw new ServiceException('New password and confirm password do not match.');
        }
        if (validatedData.oldPassword === validatedData.newPassword) {
            throw new ServiceException('New password cannot be the same as the old password.');
        }
        const token = await getBackendToken();
        await backend.profile.changePassword(token, validatedData.oldPassword, validatedData.newPassword);
    });

export const updateProfile = async (displayName: string) =>
    simpleAction(async () => {
        const token = await getBackendToken();
        await backend.profile.update(token, { display_name: displayName });
        return new SuccessActionResult(undefined, 'Profile updated.');
    });

