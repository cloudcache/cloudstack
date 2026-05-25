import { getBackendToken, isAuthorizedReadForApp } from "@/server/utils/action-wrapper.utils";
import { backend } from "@/server/adapter/backend-api.adapter";
import AppTabs from "./app-tabs";
import AppBreadcrumbs from "./app-breadcrumbs";
import { UserGroupUtils } from "@/shared/utils/role.utils";

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
    const role = UserGroupUtils.getRolePermissionForApp(session, appId);
    const token = await getBackendToken();
    const app = await backend.apps.getById(token, appId);

    const [s3Targets, apps] = await Promise.all([
        backend.s3Targets.list(token),
        backend.apps.list(token, app.project_id) as Promise<{ id: string; name: string }[]>,
    ]);

    return (<>
        <AppTabs
            role={role!}
            volumeBackups={[]}
            s3Targets={s3Targets}
            app={app}
            nodesInfo={[]}
            tabName={searchParams?.tabName ?? 'overview'} />
        <AppBreadcrumbs app={app} apps={apps} tabName={searchParams?.tabName} />
    </>
    )
}
