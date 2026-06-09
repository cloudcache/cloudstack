import { backend } from "@/server/adapter/backend-api.adapter";
import { getAdminToken, ResourcePageShell, catchOrEmpty } from "../page-shell";
import RedisEndpointsClient from "./client";

export default async function RedisEndpointsPage() {
    const token = await getAdminToken();
    const items = await backend.adminRedisEndpoints.list(token).catch(catchOrEmpty([]));
    return (
        <ResourcePageShell
            title="Redis Endpoints"
            subtitle="Redis cache instances tenants can bind to via templates."
            current="Redis Endpoints">
            <RedisEndpointsClient items={items as any[]} />
        </ResourcePageShell>
    );
}
