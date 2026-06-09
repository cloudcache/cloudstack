import { backend } from "@/server/adapter/backend-api.adapter";
import AdminProxyManagersTab from "@/app/(admin)/settings/server/admin-proxy-managers-tab";
import { getAdminToken, ResourcePageShell, catchOrEmpty } from "../page-shell";
import { getT } from "@/i18n/server";

export default async function LoadBalancingPage() {
    const token = await getAdminToken();
    const proxyManagers = await backend.adminProxyManagers.list(token).catch(catchOrEmpty([]));
    const { t } = await getT();

    return (
        <ResourcePageShell
            title={t("page.resources.loadBalancing.title")}
            subtitle={t("page.resources.loadBalancing.subtitle")}
            current={t("page.resources.loadBalancing.title")}>
            <AdminProxyManagersTab initialItems={proxyManagers as any[]} />
        </ResourcePageShell>
    );
}
