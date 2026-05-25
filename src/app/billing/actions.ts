'use server'

import { getBackendToken } from "@/server/utils/action-wrapper.utils";
import { backend } from "@/server/adapter/backend-api.adapter";

export async function createTopup(amount: number) {
    const token = await getBackendToken();
    return backend.billing.createTopup(token, amount);
}

export async function getTopupConfig() {
    const token = await getBackendToken();
    return backend.billing.topupConfig(token);
}

export async function getTopupHistory() {
    const token = await getBackendToken();
    return backend.billing.topupHistory(token);
}

export async function getUsageHistory(from?: string, to?: string) {
    const token = await getBackendToken();
    return backend.billing.usageHistory(token, { from, to, per_page: 720 });
}
