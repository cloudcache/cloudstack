'use client'

import AdminServiceEndpointTab, { FieldDef } from "@/app/settings/server/admin-service-endpoint-tab";
import { createSmtpEndpoint, updateSmtpEndpoint, deleteSmtpEndpoint } from "@/app/settings/server/actions";
import { Badge } from "@/components/ui/badge";

const fields: FieldDef[] = [
    { key: 'name', label: 'Name', type: 'text', placeholder: 'sendgrid-prod' },
    { key: 'host', label: 'Host', type: 'text', placeholder: 'smtp.sendgrid.net' },
    { key: 'port', label: 'Port', type: 'number', defaultValue: 587 },
    { key: 'username', label: 'Username', type: 'text' },
    { key: 'password', label: 'Password / API key', type: 'password', optionalOnEdit: true },
    { key: 'from_address', label: 'Default From', type: 'text', placeholder: 'noreply@example.com' },
    { key: 'tls_enabled', label: 'TLS / STARTTLS', type: 'switch', defaultValue: true },
    { key: 'description', label: 'Description', type: 'text' },
];

const columns = [
    { render: (it: any) => <span className="font-mono text-xs text-muted-foreground">{it.host}:{it.port}</span> },
    { render: (it: any) => it.from_address ? <Badge variant="outline">{it.from_address}</Badge> : null },
    { render: (it: any) => it.tls_enabled ? <Badge variant="outline">TLS</Badge> : null },
];

export default function SmtpEndpointsClient({ items }: { items: any[] }) {
    return (
        <AdminServiceEndpointTab
            title="SMTP Endpoints"
            description="SMTP relays for outbound mail from tenant apps."
            items={items}
            fields={fields}
            columns={columns}
            createFn={(b) => createSmtpEndpoint(null, b)}
            updateFn={updateSmtpEndpoint}
            deleteFn={deleteSmtpEndpoint}
        />
    );
}
