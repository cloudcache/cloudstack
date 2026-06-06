import { backend } from "@/server/adapter/backend-api.adapter";
import { getAdminToken, ResourcePageShell, catchOrEmpty } from "../page-shell";
import AdminAppTemplatesTab from "@/app/settings/server/admin-app-templates-tab";

export default async function AppTemplatesPage() {
    const token = await getAdminToken();
    const items = await backend.templates.list(token).catch(catchOrEmpty([]));
    return (
        <ResourcePageShell
            title="App Templates"
            subtitle="Catalog of deployable apps with service-dependency declarations."
            current="App Templates">
            <AdminAppTemplatesTab initialItems={items as any[]} />
        </ResourcePageShell>
    );
}
