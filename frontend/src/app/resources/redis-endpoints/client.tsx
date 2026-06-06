'use client'

import AdminServiceEndpointTab, { FieldDef } from "@/app/settings/server/admin-service-endpoint-tab";
import { createRedisEndpoint, updateRedisEndpoint, deleteRedisEndpoint } from "@/app/settings/server/actions";
import { Badge } from "@/components/ui/badge";

const fields: FieldDef[] = [
    { key: 'name', label: 'Name', type: 'text', placeholder: 'redis-cache' },
    { key: 'host', label: 'Host', type: 'text', placeholder: 'redis.example.com' },
    { key: 'port', label: 'Port', type: 'number', defaultValue: 6379 },
    { key: 'password', label: 'Password', type: 'password', optionalOnEdit: true },
    { key: 'db_index', label: 'DB Index', type: 'number', defaultValue: 0 },
    { key: 'tls_enabled', label: 'TLS (rediss)', type: 'switch' },
    { key: 'description', label: 'Description', type: 'text' },
];

const columns = [
    { render: (it: any) => <span className="font-mono text-xs text-muted-foreground">{it.host}:{it.port}/{it.db_index ?? 0}</span> },
    { render: (it: any) => it.tls_enabled ? <Badge variant="outline">TLS</Badge> : null },
];

export default function RedisEndpointsClient({ items }: { items: any[] }) {
    return (
        <AdminServiceEndpointTab
            title="Redis Endpoints"
            description="Cache endpoints (Redis) for tenant app bindings."
            items={items}
            fields={fields}
            columns={columns}
            createFn={(b) => createRedisEndpoint(null, b)}
            updateFn={updateRedisEndpoint}
            deleteFn={deleteRedisEndpoint}
        />
    );
}
