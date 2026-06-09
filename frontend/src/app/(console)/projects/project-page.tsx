'use server'

import { Button } from "@/components/ui/button";
import Link from "next/link";
import { getAuthUserSession, getBackendToken } from "@/server/utils/action-wrapper.utils";
import { backend, BackendApiError } from "@/server/adapter/backend-api.adapter";
import ProjectsTable from "./projects-table";
import { EditProjectDialog } from "./edit-project-dialog";
import ProjectsBreadcrumbs from "./projects-breadcrumbs";
import { Plus } from "lucide-react";
import { BackendUnavailable } from "@/components/custom/backend-unavailable";
import { getT } from "@/i18n/server";

export default async function ProjectPage() {

    const session = await getAuthUserSession();
    const token = await getBackendToken();
    const { t } = await getT();
    let data: any[];
    try {
        data = (await backend.projects.list(token)) as any[];
    } catch (ex) {
        if (ex instanceof BackendApiError) {
            return <BackendUnavailable message={ex.message} />;
        }
        throw ex;
    }

    return (
        <div className="flex-1 space-y-4 pt-6">
            <div className="flex gap-4">
                <h2 className="text-3xl font-bold tracking-tight flex-1">{t("page.projects.title")}</h2>
                <EditProjectDialog>
                    <Button><Plus /> {t("page.projects.create")}</Button>
                </EditProjectDialog>
            </div>
            <ProjectsTable session={session} data={data} />
            <ProjectsBreadcrumbs />
        </div>
    )
}
