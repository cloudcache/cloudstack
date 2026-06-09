'use server'

import { getAdminUserSession, getAuthUserSession, getBackendToken, simpleAction } from "@/server/utils/action-wrapper.utils";
import { SuccessActionResult } from "@/shared/model/server-action-error-return.model";
import { backend } from "@/server/adapter/backend-api.adapter";

export const subscribeToPlan = async (planId: string, billingCycle: string) =>
    simpleAction(async () => {
        await getAuthUserSession();
        const token = await getBackendToken();
        await backend.subscription.subscribe(token, { plan_id: planId, billing_cycle: billingCycle });
        return new SuccessActionResult(undefined, 'Subscription activated successfully.');
    });

export const cancelSubscription = async (reason?: string) =>
    simpleAction(async () => {
        await getAuthUserSession();
        const token = await getBackendToken();
        await backend.subscription.cancel(token, reason);
        return new SuccessActionResult(undefined, 'Subscription cancelled.');
    });

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
