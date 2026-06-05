'use server'

import { SuccessActionResult } from "@/shared/model/server-action-error-return.model";
import { ServiceException } from "@/shared/model/service.exception.model";
import { getAdminUserSession, getBackendToken, saveFormAction, simpleAction } from "@/server/utils/action-wrapper.utils";
import { s3TargetEditZodModel } from "@/shared/model/s3-target-edit.model";
import { backend } from "@/server/adapter/backend-api.adapter";

export const saveS3Target = async (prevState: any, inputData: any) =>
    saveFormAction(inputData, s3TargetEditZodModel, async (validatedData) => {
        await getAdminUserSession();
        const token = await getBackendToken();

        // Normalize endpoint: strip protocol prefix
        let endpoint = validatedData.endpoint;
        if (endpoint.includes('://')) {
            endpoint = new URL(endpoint).hostname;
        }
        const payload = { ...validatedData, endpoint };
        const apiPayload = {
            id: payload.id,
            name: payload.name,
            endpoint: payload.endpoint,
            region: payload.region,
            bucket_name: payload.bucketName,
            access_key_id: payload.accessKeyId,
            secret_key: payload.secretKey,
        };

        // Test connection first
        try {
            await backend.adminS3Targets.test(token, {
                endpoint,
                region: validatedData.region,
                access_key_id: validatedData.accessKeyId,
                secret_key: validatedData.secretKey,
                bucket_name: validatedData.bucketName,
            });
        } catch {
            throw new ServiceException('Could not connect to S3 Target, please check your credentials and try again');
        }

        if (validatedData.id) {
            await backend.adminS3Targets.update(token, validatedData.id, apiPayload);
        } else {
            await backend.adminS3Targets.create(token, apiPayload);
        }
    });

export const deleteS3Target = async (s3TargetId: string) =>
    simpleAction(async () => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminS3Targets.delete(token, s3TargetId);
        return new SuccessActionResult(undefined, 'Successfully deleted S3 Target');
    });
