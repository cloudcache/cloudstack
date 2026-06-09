'use server'

import { SuccessActionResult } from "@/shared/model/server-action-error-return.model";
import { getBackendToken, isAuthorizedWriteForApp, saveFormAction, simpleAction } from "@/server/utils/action-wrapper.utils";
import { ServiceException } from "@/shared/model/service.exception.model";
import { HostnameDnsProviderUtils } from "@/shared/utils/domain-dns-provider.utils";
import { backend } from "@/server/adapter/backend-api.adapter";
import { z } from "zod";

const saveDomainSchema = z.object({
    id: z.string().optional(),
    appId: z.string(),
    projectId: z.string().optional(),
    hostname: z.string().min(1),
    port: z.number().optional(),
    useSsl: z.boolean().optional(),
    redirect_https: z.boolean().optional(),
    redirectHttps: z.boolean().optional(),
    use_lets_encrypt: z.boolean().optional(),
    basic_auth_username: z.string().optional(),
    basic_auth_password: z.string().optional(),
});

export const saveDomain = async (prevState: any, inputData: z.infer<typeof saveDomainSchema>) =>
    saveFormAction(inputData, saveDomainSchema, async (validatedData) => {
        await isAuthorizedWriteForApp(validatedData.appId);

        let hostname = validatedData.hostname;
        if (hostname.includes('://')) {
            hostname = new URL(hostname).hostname;
        }

        if (HostnameDnsProviderUtils.containsDnsProviderHostname(hostname)) {
            if (!HostnameDnsProviderUtils.isValidDnsProviderHostname(hostname)) {
                throw new ServiceException(
                    `Invalid ${HostnameDnsProviderUtils.PROVIDER_HOSTNAME} domain. ` +
                    `Subdomain of ${HostnameDnsProviderUtils.PROVIDER_HOSTNAME} cannot contain dots.`
                );
            }
        }

        const token = await getBackendToken();
        await backend.apps.domains.create(token, validatedData.projectId ?? '', validatedData.appId, {
            hostname,
            redirect_https: validatedData.redirect_https,
            use_lets_encrypt: validatedData.use_lets_encrypt,
            basic_auth_username: validatedData.basic_auth_username,
            basic_auth_password: validatedData.basic_auth_password,
        });
    });

export const deleteDomain = async (projectId: string, appId: string, domainId: string) =>
    simpleAction(async () => {
        await isAuthorizedWriteForApp(appId);
        const token = await getBackendToken();
        await backend.apps.domains.delete(token, projectId, appId, domainId);
        return new SuccessActionResult(undefined, 'Successfully deleted domain');
    });

const savePortSchema = z.object({
    port: z.number().int().min(1).max(65535),
    protocol: z.string().optional(),
    description: z.string().optional(),
});

export const savePort = async (prevState: any, inputData: z.infer<typeof savePortSchema>, projectId: string, appId: string) =>
    saveFormAction(inputData, savePortSchema, async (validatedData) => {
        await isAuthorizedWriteForApp(appId);
        const token = await getBackendToken();
        await backend.apps.ports.add(token, projectId, appId, validatedData);
    });

export const deletePort = async (projectId: string, appId: string, portId: string) =>
    simpleAction(async () => {
        await isAuthorizedWriteForApp(appId);
        const token = await getBackendToken();
        await backend.apps.ports.delete(token, projectId, appId, portId);
        return new SuccessActionResult(undefined, 'Successfully deleted port');
    });

export const getQuickstackDomainSuffix = async () =>
    simpleAction(async () => {
        // TODO: implement via backend config endpoint
        const suffix = process.env.QUICKSTACK_DOMAIN_SUFFIX ?? '';
        return new SuccessActionResult(suffix);
    });
