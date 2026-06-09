import { backend } from "@/server/adapter/backend-api.adapter";
import AdminRegistriesTab from "@/app/(admin)/settings/server/admin-registries-tab";
import { getAdminToken, ResourcePageShell, catchOrEmpty } from "../page-shell";
import { getT } from "@/i18n/server";

export default async function RegistriesPage() {
    const token = await getAdminToken();
    const registries = await backend.adminRegistries.list(token).catch(catchOrEmpty([]));
    const { t } = await getT();

    return (
        <ResourcePageShell
            title={t("page.resources.registries.title")}
            subtitle={t("page.resources.registries.subtitle")}
            current={t("page.resources.registries.title")}>
            <AdminRegistriesTab initialItems={registries as any[]} />
        </ResourcePageShell>
    );
}
