import { getBackendToken, isAuthorizedReadForApp } from "@/server/utils/action-wrapper.utils";
import { backend, BackendApiError } from "@/server/adapter/backend-api.adapter";
import AppTabs from "./app-tabs";
import AppBreadcrumbs from "./app-breadcrumbs";
import { RolePermissionEnum } from "@/shared/model/role-extended.model.ts";
import { BackendUnavailable } from "@/components/custom/backend-unavailable";

export default async function AppPage(props: {
    searchParams?: Promise<{ [key: string]: string | undefined }>;
    params: Promise<{ appId: string }>
}) {
    const params = await props.params;
    const searchParams = await props.searchParams ?? {};
    const appId = params?.appId;
    if (!appId) {
        return <p>Could not find app with id {appId}</p>
    }
    const session = await isAuthorizedReadForApp(appId);
    const token = await getBackendToken();
    let app: Awaited<ReturnType<typeof backend.apps.getById>>;
    let project: { my_role?: string | null };
    try {
        app = await backend.apps.getById(token, appId);
        project = await backend.projects.get(token, app.project_id) as { my_role?: string | null };
    } catch (ex) {
        if (ex instanceof BackendApiError) {
            return <BackendUnavailable message={ex.message} />;
        }
        throw ex;
    }
    const role = session.userGroup?.name === 'admin' || project.my_role === 'ADMIN' || project.my_role === 'OPERATOR'
        ? RolePermissionEnum.READWRITE
        : RolePermissionEnum.READ;
    const domains = app.app_domains ?? [];
    const ports = (app as any).app_ports ?? [];
    const normalizedApp = {
        ...app,
        app_domains: domains,
        // camelCase aliases used by legacy components (DomainsList, NetworkGraph, etc.)
        appDomains: domains,
        appPorts: ports,
        appVolumes: (app as any).app_volumes ?? [],
        appFileMounts: (app as any).app_file_mounts ?? [],
        appBasicAuths: (app as any).app_basic_auths ?? [],
        projectId: app.project_id,
        sourceType: app.source_type,
    };

    let s3Targets: Awaited<ReturnType<typeof backend.s3Targets.list>>;
    let apps: { id: string; name: string }[];
    try {
        [s3Targets, apps] = await Promise.all([
            backend.s3Targets.list(token),
            backend.apps.list(token, app.project_id) as Promise<{ id: string; name: string }[]>,
        ]);
    } catch (ex) {
        if (ex instanceof BackendApiError) {
            return <BackendUnavailable message={ex.message} />;
        }
        throw ex;
    }

    return (<>
        <AppTabs
            role={role}
            volumeBackups={[]}
            s3Targets={s3Targets}
            app={normalizedApp}
            nodesInfo={[]}
            tabName={searchParams?.tabName ?? 'overview'} />
        <AppBreadcrumbs app={normalizedApp} apps={apps} tabName={searchParams?.tabName} />
    </>
    )
}
