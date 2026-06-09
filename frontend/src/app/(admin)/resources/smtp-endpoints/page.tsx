import { backend } from "@/server/adapter/backend-api.adapter";
import { getAdminToken, ResourcePageShell, catchOrEmpty } from "../page-shell";
import SmtpEndpointsClient from "./client";

export default async function SmtpEndpointsPage() {
    const token = await getAdminToken();
    const items = await backend.adminSmtpEndpoints.list(token).catch(catchOrEmpty([]));
    return (
        <ResourcePageShell
            title="SMTP Endpoints"
            subtitle="SMTP relays tenants can bind to via templates."
            current="SMTP Endpoints">
            <SmtpEndpointsClient items={items as any[]} />
        </ResourcePageShell>
    );
}
