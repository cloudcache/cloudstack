'use server'

import { SuccessActionResult } from "@/shared/model/server-action-error-return.model";
import { ServiceException } from "@/shared/model/service.exception.model";
import { getAdminUserSession, getBackendToken, saveFormAction, simpleAction } from "@/server/utils/action-wrapper.utils";
import { z } from "zod";
import { backend } from "@/server/adapter/backend-api.adapter";

const s3Schema = z.object({
    id: z.string().optional(),
    name: z.string().trim().min(1),
    endpoint: z.string().trim().min(1),
    bucket_name: z.string().trim().min(1),
    region: z.string().trim().min(1),
    access_key_id: z.string().trim().min(1),
    secret_key: z.string().trim().min(1),
});

export const saveS3Target = async (prevState: any, inputData: z.infer<typeof s3Schema>) =>
    saveFormAction(inputData, s3Schema, async (validatedData) => {
        await getAdminUserSession();
        const token = await getBackendToken();

        // Normalize endpoint: strip protocol prefix
        let endpoint = validatedData.endpoint;
        if (endpoint.includes('://')) {
            endpoint = new URL(endpoint).hostname;
        }
        const payload = { ...validatedData, endpoint };

        // Test connection first
        try {
            await backend.adminS3Targets.test(token, {
                endpoint,
                region: validatedData.region,
                access_key_id: validatedData.access_key_id,
                secret_key: validatedData.secret_key,
                bucket_name: validatedData.bucket_name,
            });
        } catch {
            throw new ServiceException('Could not connect to S3 Target, please check your credentials and try again');
        }

        if (validatedData.id) {
            await backend.adminS3Targets.update(token, validatedData.id, payload);
        } else {
            await backend.adminS3Targets.create(token, payload);
        }
    });

export const deleteS3Target = async (s3TargetId: string) =>
    simpleAction(async () => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminS3Targets.delete(token, s3TargetId);
        return new SuccessActionResult(undefined, 'Successfully deleted S3 Target');
    });
