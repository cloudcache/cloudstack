'use server'

import { AuthFormInputSchema, authFormInputSchemaZod, RegisterFormInputSchema, registgerFormInputSchemaZod } from "@/shared/model/auth-form";
import { SuccessActionResult } from "@/shared/model/server-action-error-return.model";
import { ServiceException } from "@/shared/model/service.exception.model";
import { saveFormAction } from "@/server/utils/action-wrapper.utils";
import { backend } from "@/server/adapter/backend-api.adapter";

export const registerUser = async (prevState: any, inputData: RegisterFormInputSchema) =>
    saveFormAction(inputData, registgerFormInputSchemaZod, async (validatedData) => {
        // Check if registration is currently allowed
        const status = await backend.auth.registrationStatus();
        if (!status.enabled) {
            throw new ServiceException("User registration is currently not possible");
        }

        await backend.auth.register({
            username: validatedData.email.split('@')[0],
            email: validatedData.email,
            password: validatedData.password,
        });

        return new SuccessActionResult(undefined, 'Successfully registered. Please log in.');
    });

// authUser is handled by NextAuth signIn — this action is kept for compatibility
export const authUser = async (inputData: AuthFormInputSchema) =>
    saveFormAction(inputData, authFormInputSchemaZod, async (validatedData) => {
        // NextAuth handles actual authentication via backendAuth.login()
        // This action is a no-op placeholder; callers should use signIn() directly
        return new SuccessActionResult(undefined);
    });
