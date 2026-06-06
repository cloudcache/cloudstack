'use server'

import { getAuthUserSession, getBackendToken } from "@/server/utils/action-wrapper.utils";
import { backend, BackendApiError } from "@/server/adapter/backend-api.adapter";
import ProjectOverview from "./project-overview";
import PageTitle from "@/components/custom/page-title";
import ProjectBreadcrumbs from "./project-breadcrumbs";
import CreateProjectActions from "./create-project-actions";
import ManagedUsagePanel from "./managed-usage-panel";
import { BackendUnavailable } from "@/components/custom/backend-unavailable";
import { getT } from "@/i18n/server";

export default async function AppsPage(props: {
    searchParams?: Promise<{ [key: string]: string | undefined }>;
    params: Promise<{ projectId: string }>
}) {
    const params = await props.params;
    const session = await getAuthUserSession();
    const { t } = await getT();

    const projectId = params?.projectId;
    if (!projectId) {
        return <p>Could not find project with id {projectId}</p>
    }

    const token = await getBackendToken();
    let project: { id: string; name: string; display_name: string; my_role?: string | null };
    let apps: any[];
    try {
        project = (await backend.projects.get(token, projectId)) as { id: string; name: string; display_name: string; my_role?: string | null };
        const rawApps = (await backend.apps.list(token, projectId)) as any[];
        // Normalize snake_case → camelCase aliases for legacy components
        apps = rawApps.map((a: any) => ({
            ...a,
            appDomains: a.app_domains ?? a.appDomains ?? [],
            appPorts: a.app_ports ?? a.appPorts ?? [],
            projectId: a.project_id ?? a.projectId,
        }));
    } catch (ex) {
        if (ex instanceof BackendApiError) {
            return <BackendUnavailable message={ex.message} />;
        }
        throw ex;
    }
    const canCreateApps = session.userGroup?.name === 'admin' || project.my_role === 'ADMIN' || project.my_role === 'OPERATOR';
    const canDeleteApps = session.userGroup?.name === 'admin' || project.my_role === 'ADMIN';

    return (
        <div className="flex-1 space-y-4 pt-6">
            <PageTitle
                title={t("page.project.apps")}
                subtitle={t("page.project.appsSubtitle", { name: project.display_name ?? project.name })}>
                {canCreateApps && <CreateProjectActions projectId={projectId} />}
            </PageTitle>
            <ProjectOverview
                apps={apps}
                projectId={project.id}
                canCreateApps={canCreateApps}
                canDeleteApps={canDeleteApps} />
            <ManagedUsagePanel projectId={project.id} />
            <ProjectBreadcrumbs project={project as any} />
        </div>
    )
}
