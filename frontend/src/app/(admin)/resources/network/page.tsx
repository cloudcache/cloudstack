import { backend } from "@/server/adapter/backend-api.adapter";
import AdminIpPoolsTab from "@/app/(admin)/settings/server/admin-ip-pools-tab";
import { getAdminToken, ResourcePageShell, catchOrEmpty } from "../page-shell";
import { getT } from "@/i18n/server";

export default async function NetworkPage() {
    const token = await getAdminToken();
    const ipPools = await backend.adminIpPools.list(token).catch(catchOrEmpty([]));
    const { t } = await getT();

    return (
        <ResourcePageShell
            title={t("page.resources.network.title")}
            subtitle={t("page.resources.network.subtitle")}
            current={t("page.resources.network.title")}>
            <AdminIpPoolsTab initialItems={ipPools as any[]} />
        </ResourcePageShell>
    );
}
