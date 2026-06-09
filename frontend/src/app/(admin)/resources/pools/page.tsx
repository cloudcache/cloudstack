import { backend } from "@/server/adapter/backend-api.adapter";
import AdminResourcePoolsTab from "@/app/(admin)/settings/server/admin-resource-pools-tab";
import { getAdminToken, ResourcePageShell, catchOrEmpty } from "../page-shell";
import { getT } from "@/i18n/server";

export default async function ResourcePoolsPage() {
    const token = await getAdminToken();
    const resourcePools = await backend.adminPools.list(token).catch(catchOrEmpty([]));
    const { t } = await getT();

    return (
        <ResourcePageShell
            title={t("page.resources.pools.title")}
            subtitle={t("page.resources.pools.subtitle")}
            current={t("page.resources.pools.title")}>
            <AdminResourcePoolsTab initialItems={resourcePools as any[]} />
        </ResourcePageShell>
    );
}
