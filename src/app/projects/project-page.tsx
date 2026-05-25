'use server'

import { Button } from "@/components/ui/button";
import Link from "next/link";
import { getAuthUserSession, getBackendToken } from "@/server/utils/action-wrapper.utils";
import { backend } from "@/server/adapter/backend-api.adapter";
import ProjectsTable from "./projects-table";
import { EditProjectDialog } from "./edit-project-dialog";
import ProjectsBreadcrumbs from "./projects-breadcrumbs";
import { Plus } from "lucide-react";
import { UserGroupUtils } from "@/shared/utils/role.utils";

export default async function ProjectPage() {

    const session = await getAuthUserSession();
    const token = await getBackendToken();
    const data = (await backend.projects.list(token)) as any[];

    return (
        <div className="flex-1 space-y-4 pt-6">
            <div className="flex gap-4">
                <h2 className="text-3xl font-bold tracking-tight flex-1">Projects</h2>
                {UserGroupUtils.isAdmin(session) && <EditProjectDialog>
                    <Button><Plus /> Create Project</Button>
                </EditProjectDialog>}
            </div>
            <ProjectsTable session={session} data={data} />
            <ProjectsBreadcrumbs />
        </div>
    )
}
