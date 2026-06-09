'use server'

import { getAdminUserSession, getBackendToken, simpleAction, saveFormAction } from "@/server/utils/action-wrapper.utils";
import { ServerActionResult, SuccessActionResult } from "@/shared/model/server-action-error-return.model";
import { backend } from "@/server/adapter/backend-api.adapter";
import { z } from "zod";

// ── Nodes ──────────────────────────────────────────────────────────────────────

export const setNodeSchedulable = async (nodeId: string, schedulable: boolean) =>
    simpleAction(async () => {
        await getAdminUserSession();
        const token = await getBackendToken();
        if (schedulable) {
            await backend.adminNodes.uncordon(token, nodeId);
        } else {
            await backend.adminNodes.cordon(token, nodeId);
        }
        return new SuccessActionResult(undefined, `Node ${schedulable ? 'activated' : 'deactivated'} successfully.`);
    });

const addNodeSchema = z.object({
    cluster_id: z.string().min(1),
    hostname: z.string().min(1),
    ip_address: z.string().min(1),
    // Optional: blank → provision over the platform SSH key (no password).
    ssh_password: z.string().optional(),
    ssh_port: z.number().int().min(1).max(65535).optional(),
    node_role: z.string().optional(),
    storage_path: z.string().optional(),
});

export const addNode = async (prevState: any, inputData: z.infer<typeof addNodeSchema>) =>
    saveFormAction(inputData, addNodeSchema, async (data) => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminNodes.add(token, {
            cluster_id: data.cluster_id,
            hostname: data.hostname,
            ip_address: data.ip_address,
            ssh_password: data.ssh_password || undefined,
            ssh_port: data.ssh_port,
            node_role: data.node_role || undefined,
            storage_path: data.storage_path || undefined,
        });
        return new SuccessActionResult(undefined, 'Node added. Provisioning started in background.');
    });

export const deleteNode = async (nodeId: string) =>
    simpleAction(async () => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminNodes.delete(token, nodeId);
        return new SuccessActionResult(undefined, 'Node deleted.');
    });

export const updateNode = async (nodeId: string, body: { hostname?: string; ip_address?: string; node_role?: string; storage_path?: string; ssh_port?: number }) =>
    simpleAction(async () => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminNodes.update(token, nodeId, body);
        return new SuccessActionResult(undefined, 'Node updated.');
    });

export const reprovisionNode = async (nodeId: string, sshPassword: string) =>
    simpleAction(async () => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminNodes.reprovision(token, nodeId, { ssh_password: sshPassword || undefined });
        return new SuccessActionResult(undefined, 'Reprovisioning started in background.');
    });

// ── Proxy Managers ─────────────────────────────────────────────────────────────

const proxyManagerSchema = z.object({
    name: z.string().min(1),
    host: z.string().min(1),
    api_base_url: z.string().min(1),
    api_password: z.string().min(1),
});

export const createProxyManager = async (prevState: any, inputData: z.infer<typeof proxyManagerSchema>) =>
    saveFormAction(inputData, proxyManagerSchema, async (data) => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminProxyManagers.create(token, data);
        return new SuccessActionResult(undefined, 'Proxy manager created.');
    });

export const updateProxyManager = async (id: string, inputData: { name?: string; host?: string; api_base_url?: string; api_password?: string; is_active?: boolean }) =>
    simpleAction(async () => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminProxyManagers.update(token, id, inputData);
        return new SuccessActionResult(undefined, 'Proxy manager updated.');
    });

export const deleteProxyManager = async (id: string) =>
    simpleAction(async () => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminProxyManagers.delete(token, id);
        return new SuccessActionResult(undefined, 'Proxy manager deleted.');
    });

// ── Registries ─────────────────────────────────────────────────────────────────

const registrySchema = z.object({
    name: z.string().min(1),
    endpoint: z.string().min(1),
    username: z.string().optional(),
    password: z.string().optional(),
    is_default: z.boolean().optional(),
    priority: z.number().int().optional(),
});

export const createRegistry = async (prevState: any, inputData: z.infer<typeof registrySchema>) =>
    saveFormAction(inputData, registrySchema, async (data) => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminRegistries.create(token, data);
        return new SuccessActionResult(undefined, 'Registry created.');
    });

export const updateRegistry = async (id: string, inputData: { name?: string; endpoint?: string; username?: string; password?: string; is_default?: boolean; priority?: number; is_active?: boolean }) =>
    simpleAction(async () => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminRegistries.update(token, id, inputData);
        return new SuccessActionResult(undefined, 'Registry updated.');
    });

export const deleteRegistry = async (id: string) =>
    simpleAction(async () => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminRegistries.delete(token, id);
        return new SuccessActionResult(undefined, 'Registry deleted.');
    });

// ── Resource Pools ─────────────────────────────────────────────────────────────

const resourcePoolSchema = z.object({
    name: z.string().min(1),
    display_name: z.string().min(1),
    region: z.string().optional(),
    description: z.string().optional(),
});

export const createResourcePool = async (prevState: any, inputData: z.infer<typeof resourcePoolSchema>) =>
    saveFormAction(inputData, resourcePoolSchema, async (data) => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminPools.create(token, data);
        return new SuccessActionResult(undefined, 'Resource pool created.');
    });

export const updateResourcePool = async (id: string, body: { display_name?: string; region?: string; description?: string; is_active?: boolean }) =>
    simpleAction(async () => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminPools.update(token, id, body);
        return new SuccessActionResult(undefined, 'Resource pool updated.');
    });

export const deleteResourcePool = async (id: string) =>
    simpleAction(async () => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminPools.delete(token, id);
        return new SuccessActionResult(undefined, 'Resource pool deleted.');
    });

// ── Clusters ───────────────────────────────────────────────────────────────────

const clusterSchema = z.object({
    name: z.string().min(1),
    display_name: z.string().optional(),
    description: z.string().optional(),
    pool_id: z.string().optional(),
    ip_pool_id: z.string().optional(),
    orchestrator: z.enum(['K3S', 'DOCKER']).optional(),
});

export const createCluster = async (prevState: any, inputData: z.infer<typeof clusterSchema>) =>
    saveFormAction(inputData, clusterSchema, async (data) => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminClusters.create(token, data);
        return new SuccessActionResult(undefined, 'Cluster created.');
    });

export const updateCluster = async (id: string, body: { display_name?: string; description?: string; is_active?: boolean; ip_pool_id?: string; node_main_iface?: string }) =>
    simpleAction(async () => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminClusters.update(token, id, body);
        return new SuccessActionResult(undefined, 'Cluster updated.');
    });

export const deleteCluster = async (id: string) =>
    simpleAction(async () => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminClusters.delete(token, id);
        return new SuccessActionResult(undefined, 'Cluster deleted.');
    });

export const updateClusterStorage = async (body: unknown) =>
    simpleAction(async () => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminClusters.updateStorage(token, body);
        return new SuccessActionResult(undefined, 'Cluster storage updated.');
    });

// ── Platform Config ────────────────────────────────────────────────────────────

export const setPlatformConfig = async (key: string, value: string) =>
    simpleAction(async () => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminPlatform.set(token, key, value);
        return new SuccessActionResult(undefined, 'Config saved.');
    });

// ── IP Pools ───────────────────────────────────────────────────────────────────

const ipPoolSchema = z.object({
    name: z.string().min(1),
    cidr: z.string().min(1),
    pool_type: z.string().optional(),
    gateway: z.string().optional(),
    description: z.string().optional(),
});

export const createIpPool = async (prevState: any, inputData: z.infer<typeof ipPoolSchema>) =>
    saveFormAction(inputData, ipPoolSchema, async (data) => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminIpPools.create(token, data);
        return new SuccessActionResult(undefined, 'IP pool created.');
    });

export const updateIpPool = async (id: string, body: { name?: string; gateway?: string; description?: string; is_active?: boolean }) =>
    simpleAction(async () => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminIpPools.update(token, id, body);
        return new SuccessActionResult(undefined, 'IP pool updated.');
    });

export const deleteIpPool = async (id: string) =>
    simpleAction(async () => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminIpPools.delete(token, id);
        return new SuccessActionResult(undefined, 'IP pool deleted.');
    });

// ── DB Clusters ────────────────────────────────────────────────────────────────

const dbClusterSchema = z.object({
    name: z.string().min(1),
    cluster_type: z.string().min(1),
    host: z.string().min(1),
    port: z.number().int().min(1).max(65535),
    admin_user: z.string().min(1),
    admin_password: z.string().min(1),
    max_databases: z.number().int().optional(),
    description: z.string().optional(),
    manager_url: z.string().optional(),
});

export const createDbCluster = async (prevState: any, inputData: z.infer<typeof dbClusterSchema>) =>
    saveFormAction(inputData, dbClusterSchema, async (data) => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminDbClusters.create(token, data);
        return new SuccessActionResult(undefined, 'DB cluster created.');
    });

export const updateDbCluster = async (id: string, body: { host?: string; port?: number; admin_user?: string; admin_password?: string; max_databases?: number; description?: string; manager_url?: string; is_active?: boolean }) =>
    simpleAction(async () => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminDbClusters.update(token, id, body);
        return new SuccessActionResult(undefined, 'DB cluster updated.');
    });

export const deleteDbCluster = async (id: string) =>
    simpleAction(async () => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminDbClusters.delete(token, id);
        return new SuccessActionResult(undefined, 'DB cluster deleted.');
    });

// ── Subscription Plans ────────────────────────────────────────────────────────

export const createPlan = async (data: Record<string, unknown>) =>
    simpleAction(async () => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminPlans.create(token, data as any);
        return new SuccessActionResult(undefined, 'Plan created.');
    });

export const updatePlan = async (id: string, body: Record<string, unknown>) =>
    simpleAction(async () => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminPlans.update(token, id, body);
        return new SuccessActionResult(undefined, 'Plan updated.');
    });

export const deletePlan = async (id: string) =>
    simpleAction(async () => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminPlans.delete(token, id);
        return new SuccessActionResult(undefined, 'Plan deleted.');
    });

// ── Admin Billing ─────────────────────────────────────────────────────────────

export const adminRecharge = async (userId: string, amount: number, description: string, idempotencyKey?: string) =>
    simpleAction(async () => {
        await getAdminUserSession();
        const token = await getBackendToken();
        const result = await backend.adminBilling.recharge(token, userId, amount, description, idempotencyKey);
        return new SuccessActionResult(result, `Recharged ¥${amount.toFixed(2)} successfully.`);
    });

export const adminAdjustBalance = async (userId: string, amount: number, description: string, idempotencyKey?: string) =>
    simpleAction(async () => {
        await getAdminUserSession();
        const token = await getBackendToken();
        const result = await backend.adminBilling.adjust(token, userId, amount, description, idempotencyKey);
        return new SuccessActionResult(result, `Balance adjusted by ¥${amount.toFixed(2)}.`);
    });

export const adminGenerateInvoice = async (userId: string, periodStart: string, periodEnd: string) =>
    simpleAction(async () => {
        await getAdminUserSession();
        const token = await getBackendToken();
        const result = await backend.adminBilling.generateInvoice(token, {
            user_id: userId,
            period_start: periodStart,
            period_end: periodEnd,
        });
        return new SuccessActionResult(result, 'Order created.');
    });

export const adminMarkInvoicePaid = async (invoiceId: string) =>
    simpleAction(async () => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminBilling.markPaid(token, invoiceId);
        return new SuccessActionResult(undefined, 'Order marked as paid.');
    });

export const adminVoidInvoice = async (invoiceId: string) =>
    simpleAction(async () => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminBilling.voidInvoice(token, invoiceId);
        return new SuccessActionResult(undefined, 'Order voided.');
    });

// ── MQ / SMTP / Redis endpoints ────────────────────────────────────────────────

const mqSchema = z.object({
    name: z.string().min(1),
    host: z.string().min(1),
    port: z.number().int().positive().optional(),
    vhost: z.string().optional(),
    username: z.string().min(1),
    password: z.string().optional(),
    tls_enabled: z.boolean().optional(),
    description: z.string().optional(),
});
export const createMqEndpoint = async (prevState: any, input: z.infer<typeof mqSchema>) =>
    saveFormAction(input, mqSchema, async (data) => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminMqEndpoints.create(token, data);
        return new SuccessActionResult(undefined, 'MQ endpoint created.');
    });
export const updateMqEndpoint = async (id: string, body: any) =>
    simpleAction(async () => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminMqEndpoints.update(token, id, body);
        return new SuccessActionResult(undefined, 'MQ endpoint updated.');
    });
export const deleteMqEndpoint = async (id: string) =>
    simpleAction(async () => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminMqEndpoints.delete(token, id);
        return new SuccessActionResult(undefined, 'MQ endpoint deleted.');
    });

const smtpSchema = z.object({
    name: z.string().min(1),
    host: z.string().min(1),
    port: z.number().int().positive().optional(),
    username: z.string().optional(),
    password: z.string().optional(),
    from_address: z.string().optional(),
    tls_enabled: z.boolean().optional(),
    description: z.string().optional(),
});
export const createSmtpEndpoint = async (prevState: any, input: z.infer<typeof smtpSchema>) =>
    saveFormAction(input, smtpSchema, async (data) => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminSmtpEndpoints.create(token, data);
        return new SuccessActionResult(undefined, 'SMTP endpoint created.');
    });
export const updateSmtpEndpoint = async (id: string, body: any) =>
    simpleAction(async () => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminSmtpEndpoints.update(token, id, body);
        return new SuccessActionResult(undefined, 'SMTP endpoint updated.');
    });
export const deleteSmtpEndpoint = async (id: string) =>
    simpleAction(async () => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminSmtpEndpoints.delete(token, id);
        return new SuccessActionResult(undefined, 'SMTP endpoint deleted.');
    });

const redisSchema = z.object({
    name: z.string().min(1),
    host: z.string().min(1),
    port: z.number().int().positive().optional(),
    password: z.string().optional(),
    db_index: z.number().int().min(0).optional(),
    tls_enabled: z.boolean().optional(),
    description: z.string().optional(),
});
export const createRedisEndpoint = async (prevState: any, input: z.infer<typeof redisSchema>) =>
    saveFormAction(input, redisSchema, async (data) => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminRedisEndpoints.create(token, data);
        return new SuccessActionResult(undefined, 'Redis endpoint created.');
    });
export const updateRedisEndpoint = async (id: string, body: any) =>
    simpleAction(async () => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminRedisEndpoints.update(token, id, body);
        return new SuccessActionResult(undefined, 'Redis endpoint updated.');
    });
export const deleteRedisEndpoint = async (id: string) =>
    simpleAction(async () => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminRedisEndpoints.delete(token, id);
        return new SuccessActionResult(undefined, 'Redis endpoint deleted.');
    });

// ── App templates (admin PUBLIC) ───────────────────────────────────────────────

export const createAppTemplate = async (body: any) =>
    simpleAction(async () => {
        await getAdminUserSession();
        const token = await getBackendToken();
        const res = await backend.adminTemplates.create(token, body);
        return new SuccessActionResult(res, 'Template created.');
    });
export const updateAppTemplate = async (id: string, body: any) =>
    simpleAction(async () => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminTemplates.update(token, id, body);
        return new SuccessActionResult(undefined, 'Template updated.');
    });
export const deleteAppTemplate = async (id: string) =>
    simpleAction(async () => {
        await getAdminUserSession();
        const token = await getBackendToken();
        await backend.adminTemplates.delete(token, id);
        return new SuccessActionResult(undefined, 'Template deleted.');
    });
