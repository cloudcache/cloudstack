import { getBackendToken, isAuthorizedReadForApp } from "@/server/utils/action-wrapper.utils";
import { backend } from "@/server/adapter/backend-api.adapter";
import PageTitle from "@/components/custom/page-title";
import AppActionButtons from "./app-action-buttons";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { AlertTriangle } from "lucide-react";

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
  const app = await backend.apps.getById(token, appId);

  const showIngressWarning = app.app_domains.length > 0
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
      <AppActionButtons session={session} app={app} />
      {children}
    </div>
  );
}
