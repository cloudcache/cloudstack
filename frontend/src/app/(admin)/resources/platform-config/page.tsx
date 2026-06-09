import { backend } from "@/server/adapter/backend-api.adapter";
import AdminPlatformConfigTab from "@/app/(admin)/settings/server/admin-platform-config-tab";
import { getAdminToken, ResourcePageShell, catchOrEmpty } from "../page-shell";
import { getT } from "@/i18n/server";

export default async function PlatformConfigPage() {
    const token = await getAdminToken();
    const platformConfig = await backend.adminPlatform.list(token).catch(catchOrEmpty([]));
    const { t } = await getT();

    return (
        <ResourcePageShell
            title={t("page.resources.platformConfig.title")}
            subtitle={t("page.resources.platformConfig.subtitle")}
            current={t("page.resources.platformConfig.title")}>
            <AdminPlatformConfigTab initialConfig={platformConfig as any[]} />
        </ResourcePageShell>
    );
}
