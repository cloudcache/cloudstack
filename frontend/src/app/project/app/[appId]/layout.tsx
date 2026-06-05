import { getBackendToken, isAuthorizedReadForApp } from "@/server/utils/action-wrapper.utils";
import { backend, BackendApiError } from "@/server/adapter/backend-api.adapter";
import PageTitle from "@/components/custom/page-title";
import AppActionButtons from "./app-action-buttons";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { AlertTriangle } from "lucide-react";
import { RolePermissionEnum } from "@/shared/model/role-extended.model.ts";
import { BackendUnavailable } from "@/components/custom/backend-unavailable";

export default async function RootLayout(props: Readonly<{
  params: Promise<{ appId: string }>
  children: React.ReactNode;
}>) {
  const params = await props.params;
  const { children } = props;

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
  const appDomains = app.app_domains ?? [];

  const showIngressWarning = appDomains.length > 0
    && app.ingress_network_policy !== 'ALLOW_ALL'
    && app.ingress_network_policy !== 'INTERNET_ONLY';

  return (
    <div className="flex-1 space-y-6 pt-6">
      <PageTitle
        title={app.name}
        subtitle={`App ID: ${app.id}`}>
      </PageTitle>
      {showIngressWarning && (
        <Alert variant="destructive">
          <AlertTriangle className="h-4 w-4" />
          <AlertTitle>Warning</AlertTitle>
          <AlertDescription>
            You have configured domains for this app, but the Ingress Network Policy is not set to &quot;Allow All&quot; or &quot;Internet Only&quot;.
            External traffic via the domain might be blocked.
          </AlertDescription>
        </Alert>
      )}
      <AppActionButtons app={{ ...app, app_domains: appDomains }} role={role} />
      {children}
    </div>
  );
}
