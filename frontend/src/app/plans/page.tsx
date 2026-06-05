'use server'

import { getAuthUserSession, getBackendToken } from "@/server/utils/action-wrapper.utils";
import PageTitle from "@/components/custom/page-title";
import BreadcrumbSetter from "@/components/breadcrumbs-setter";
import { backend } from "@/server/adapter/backend-api.adapter";
import PlansList from "./plans-list";
import { BackendUnavailable } from "@/components/custom/backend-unavailable";
import { getT } from "@/i18n/server";

export default async function PlansPage() {
    const session = await getAuthUserSession();
    const token = await getBackendToken();
    const { t } = await getT();
    const isAdmin = session.userGroup?.name === 'admin' || (session as any).isGlobalAdmin;

    let plans: any[] = [];
    let mySubscription: any = null;
    try {
        // Admin sees all plans (including inactive); users see only public active ones
        if (isAdmin) {
            const result = await backend.adminPlans.list(token, { include_inactive: true });
            plans = ((result as any)?.data ?? result ?? []) as any[];
        } else {
            plans = (await backend.plans.list(token)) as any[];
        }
    } catch (e) {
        return <BackendUnavailable message={e instanceof Error ? e.message : 'Could not load subscription plans.'} />;
    }
    try {
        mySubscription = await backend.subscription.get(token);
    } catch {
        // No subscription yet
    }

    return (
        <div className="flex-1 space-y-6 pt-6">
            <PageTitle
                title={t("page.business.plans.title")}
                subtitle={isAdmin ? "Manage subscription plans. Create, edit, or delete plans." : "Choose a plan that fits your needs. Upgrade or downgrade anytime."}
            />
            <BreadcrumbSetter items={[{ name: t("nav.plans") }]} />
            <PlansList plans={plans} currentSubscription={mySubscription} isAdmin={isAdmin} />
        </div>
    );
}
