import { backend } from "@/server/adapter/backend-api.adapter";
import AdminPlansTab from "@/app/(admin)/settings/server/admin-plans-tab";
import { BusinessPageShell, getBusinessAdminToken, catchOrEmpty } from "../page-shell";
import { getT } from "@/i18n/server";

export default async function BusinessPlansPage() {
    const token = await getBusinessAdminToken();
    const allPlans = await backend.adminPlans.list(token, { include_inactive: true }).catch(catchOrEmpty({ data: [] }));
    const plans = ((allPlans as any)?.data ?? allPlans ?? []) as any[];
    const { t } = await getT();

    return (
        <BusinessPageShell
            title={t("page.business.plans.title")}
            subtitle={t("page.business.plans.subtitle")}
            current={t("page.business.plans.title")}>
            <AdminPlansTab initialItems={plans} />
        </BusinessPageShell>
    );
}
