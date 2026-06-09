import { backend } from "@/server/adapter/backend-api.adapter";
import AdminDbClustersTab from "@/app/(admin)/settings/server/admin-db-clusters-tab";
import { getAdminToken, ResourcePageShell, catchOrEmpty } from "../page-shell";
import { getT } from "@/i18n/server";

export default async function DatabasesPage() {
    const token = await getAdminToken();
    const dbClusters = await backend.adminDbClusters.list(token).catch(catchOrEmpty([]));
    const { t } = await getT();

    return (
        <ResourcePageShell
            title={t("page.resources.databases.title")}
            subtitle={t("page.resources.databases.subtitle")}
            current={t("page.resources.databases.title")}>
            <AdminDbClustersTab initialItems={dbClusters as any[]} />
        </ResourcePageShell>
    );
}
