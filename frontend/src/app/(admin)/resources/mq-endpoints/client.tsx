'use client'

import AdminServiceEndpointTab, { FieldDef } from "@/app/(admin)/settings/server/admin-service-endpoint-tab";
import { createMqEndpoint, updateMqEndpoint, deleteMqEndpoint } from "@/app/(admin)/settings/server/actions";
import { Badge } from "@/components/ui/badge";

const fields: FieldDef[] = [
    { key: 'name', label: 'Name', type: 'text', placeholder: 'rabbit-prod' },
    { key: 'host', label: 'Host', type: 'text', placeholder: 'rabbitmq.example.com' },
    { key: 'port', label: 'Port', type: 'number', defaultValue: 5672 },
    { key: 'vhost', label: 'vhost', type: 'text', defaultValue: '/' },
    { key: 'username', label: 'Username', type: 'text' },
    { key: 'password', label: 'Password', type: 'password', optionalOnEdit: true },
    { key: 'tls_enabled', label: 'TLS (AMQPS)', type: 'switch' },
    { key: 'description', label: 'Description', type: 'text' },
];

const columns = [
    { render: (it: any) => <span className="font-mono text-xs text-muted-foreground">{it.host}:{it.port}</span> },
    { render: (it: any) => it.tls_enabled ? <Badge variant="outline">TLS</Badge> : null },
];

export default function MqEndpointsClient({ items }: { items: any[] }) {
    return (
        <AdminServiceEndpointTab
            title="MQ Endpoints"
            description="Message brokers (RabbitMQ etc.) available for app bindings."
            items={items}
            fields={fields}
            columns={columns}
            createFn={(b) => createMqEndpoint(null, b)}
            updateFn={updateMqEndpoint}
            deleteFn={deleteMqEndpoint}
        />
    );
}
