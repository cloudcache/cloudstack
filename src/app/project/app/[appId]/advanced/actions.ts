'use server'

import { SuccessActionResult } from "@/shared/model/server-action-error-return.model";
import { getBackendToken, isAuthorizedWriteForApp, saveFormAction, simpleAction } from "@/server/utils/action-wrapper.utils";
import { BasicAuthEditModel, basicAuthEditZodModel } from "@/shared/model/basic-auth-edit.model";
import { appNetworkPolicy } from "@/shared/model/network-policy.model";
import { HealthCheckModel, healthCheckZodModel } from "./health-check.model";
import { backend } from "@/server/adapter/backend-api.adapter";

export const saveBasicAuth = async (prevState: any, inputData: BasicAuthEditModel) =>
    saveFormAction(inputData, basicAuthEditZodModel, async (validatedData) => {
        await isAuthorizedWriteForApp(validatedData.appId);
        const token = await getBackendToken();
        const app = await backend.apps.getById(token, validatedData.appId);
        await backend.apps.basicAuth.set(token, app.project_id, validatedData.appId, {
            username: validatedData.username,
            password: validatedData.password,
        });
        return new SuccessActionResult();
    });

export const deleteBasicAuth = async (appId: string) =>
    simpleAction(async () => {
        await isAuthorizedWriteForApp(appId);
        const token = await getBackendToken();
        const app = await backend.apps.getById(token, appId);
        await backend.apps.basicAuth.delete(token, app.project_id, appId);
        return new SuccessActionResult(undefined, 'Successfully deleted item');
    });

export const saveNetworkPolicy = async (appId: string, ingressPolicy: string, egressPolicy: string, useNetworkPolicy: boolean) =>
    simpleAction(async () => {
        await isAuthorizedWriteForApp(appId);
        appNetworkPolicy.parse(ingressPolicy);
        appNetworkPolicy.parse(egressPolicy);
        const token = await getBackendToken();
        const app = await backend.apps.getById(token, appId);
        await backend.apps.update(token, app.project_id, appId, {
            ingress_network_policy: ingressPolicy,
            egress_network_policy: egressPolicy,
            use_network_policy: useNetworkPolicy,
        });
        return new SuccessActionResult(undefined, 'Network policy saved');
    });

export const saveHealthCheck = async (prevState: any, inputData: HealthCheckModel) =>
    saveFormAction(inputData, healthCheckZodModel, async (validatedData) => {
        await isAuthorizedWriteForApp(validatedData.appId);
        const token = await getBackendToken();
        const app = await backend.apps.getById(token, validatedData.appId);

        let updateBody: Record<string, unknown> = {
            health_check_period: validatedData.periodSeconds,
            health_check_timeout: validatedData.timeoutSeconds,
            health_check_failures: validatedData.failureThreshold,
        };

        if (validatedData.enabled) {
            if (validatedData.probeType === 'HTTP') {
                updateBody = {
                    ...updateBody,
                    health_check_type: 'HTTP',
                    health_check_path: validatedData.path ?? null,
                    health_check_port: validatedData.httpPort ?? null,
                    health_check_scheme: validatedData.scheme ?? null,
                };
            } else if (validatedData.probeType === 'TCP') {
                updateBody = {
                    ...updateBody,
                    health_check_type: 'TCP',
                    health_check_port: validatedData.tcpPort ?? null,
                    health_check_path: null,
                    health_check_scheme: null,
                };
            }
        } else {
            updateBody = {
                ...updateBody,
                health_check_type: null,
                health_check_path: null,
                health_check_port: null,
                health_check_scheme: null,
            };
        }

        await backend.apps.update(token, app.project_id, validatedData.appId, updateBody as any);
        return new SuccessActionResult(undefined, 'Health check settings saved');
    });
