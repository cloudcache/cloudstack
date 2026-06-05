import { backend } from "@/server/adapter/backend-api.adapter";
import AdminClustersTab from "@/app/settings/server/admin-clusters-tab";
import { getAdminToken, ResourcePageShell, catchOrEmpty } from "../page-shell";
import { getT } from "@/i18n/server";

export default async function ClustersPage() {
    const token = await getAdminToken();
    const [clusters, clusterStorage, pools, ipPools] = await Promise.all([
        backend.adminClusters.list(token).catch(catchOrEmpty([])),
        backend.adminClusters.getStorage(token).catch(catchOrEmpty(null)),
        backend.adminPools.list(token).catch(catchOrEmpty([])),
        backend.adminIpPools.list(token).catch(catchOrEmpty([])),
    ]);
    const { t } = await getT();

    return (
        <ResourcePageShell
            title={t("page.resources.clusters.title")}
            subtitle={t("page.resources.clusters.subtitle")}
            current={t("page.resources.clusters.title")}>
            <AdminClustersTab
                initialItems={clusters as any[]}
                clusterStorage={clusterStorage}
                pools={(pools as any[]).map(p => ({ id: p.id, name: p.name, display_name: p.display_name }))}
                ipPools={(ipPools as any[]).map(p => ({ id: p.id, name: p.name, cidr: p.cidr, gateway: p.gateway }))}
            />
        </ResourcePageShell>
    );
}
