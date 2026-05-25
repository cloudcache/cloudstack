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

export const createNewTotpToken = async () =>
    simpleAction(async () => {
        const token = await getBackendToken();
        const result = await backend.auth.totpSetup(token);
        return new SuccessActionResult(result);
    });

export const verifyTotpToken = async (prevState: any, inputData: { totp: string }) =>
    saveFormAction(inputData, z.object({ totp: z.string().min(6) }), async (validatedData) => {
        const token = await getBackendToken();
        await backend.auth.totpVerify(token, validatedData.totp);
    });

export const deactivate2fa = async () =>
    simpleAction(async () => {
        const token = await getBackendToken();
        await backend.auth.totpDisable(token);
        return new SuccessActionResult(undefined, '2FA settings deactivated successfully');
    });
