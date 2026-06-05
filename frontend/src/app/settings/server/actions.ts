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
    ssh_password: z.string().min(1),
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
            ssh_password: data.ssh_password,
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

export const updateNode = async (nodeId: string, body: { hostname?: string; ip_address?: string; node_role?: string; storage_path?: string }) =>
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
        await backend.adminNodes.reprovision(token, nodeId, { ssh_password: sshPassword });
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

export const adminRecharge = async (userId: string, amount: number, description?: string) =>
    simpleAction(async () => {
        await getAdminUserSession();
        const token = await getBackendToken();
        const result = await backend.adminBilling.recharge(token, userId, amount, description);
        return new SuccessActionResult(result, `Recharged ¥${amount.toFixed(2)} successfully.`);
    });

export const adminAdjustBalance = async (userId: string, amount: number, description: string) =>
    simpleAction(async () => {
        await getAdminUserSession();
        const token = await getBackendToken();
        const result = await backend.adminBilling.adjust(token, userId, amount, description);
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
