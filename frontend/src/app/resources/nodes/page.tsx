import { backend } from "@/server/adapter/backend-api.adapter";
import AdminNodesTab from "@/app/settings/server/admin-nodes-tab";
import { getAdminToken, ResourcePageShell, catchOrEmpty } from "../page-shell";
import { getT } from "@/i18n/server";

export default async function NodesPage() {
    const token = await getAdminToken();
    const [nodes, clusters] = await Promise.all([
        backend.adminNodes.list(token).catch(catchOrEmpty([])),
        backend.adminClusters.list(token).catch(catchOrEmpty([])),
    ]);
    const { t } = await getT();

    return (
        <ResourcePageShell
            title={t("page.resources.nodes.title")}
            subtitle={t("page.resources.nodes.subtitle")}
            current={t("page.resources.nodes.title")}>
            <AdminNodesTab
                initialItems={nodes as any[]}
                clusters={(clusters as any[]).map((c: any) => ({ id: c.id, name: c.name, display_name: c.display_name, orchestrator: c.orchestrator }))}
            />
        </ResourcePageShell>
    );
}
