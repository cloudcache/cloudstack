'use server'

import { getAuthUserSession, getBackendToken } from "@/server/utils/action-wrapper.utils";
import { backend } from "@/server/adapter/backend-api.adapter";
import ProjectOverview from "./project-overview";
import PageTitle from "@/components/custom/page-title";
import ProjectBreadcrumbs from "./project-breadcrumbs";
import CreateProjectActions from "./create-project-actions";
import { UserGroupUtils } from "@/shared/utils/role.utils";

export default async function AppsPage(props: {
    searchParams?: Promise<{ [key: string]: string | undefined }>;
    params: Promise<{ projectId: string }>
}) {
    const params = await props.params;
    const session = await getAuthUserSession();

    const projectId = params?.projectId;
    if (!projectId) {
        return <p>Could not find project with id {projectId}</p>
    }

    const token = await getBackendToken();
    const project = (await backend.projects.get(token, projectId)) as { id: string; name: string; display_name: string };
    const apps = (await backend.apps.list(token, projectId)) as any[];

    return (
        <div className="flex-1 space-y-4 pt-6">
            <PageTitle
                title="Apps"
                subtitle={`All Apps for Project "${project.display_name ?? project.name}"`}>
                {UserGroupUtils.sessionCanCreateNewAppsForProject(session, params.projectId) &&
                    <CreateProjectActions projectId={projectId} />}
            </PageTitle>
            <ProjectOverview session={session} apps={apps} projectId={project.id} />
            <ProjectBreadcrumbs project={project as any} />
        </div>
    )
}
