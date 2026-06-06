import { backend } from "@/server/adapter/backend-api.adapter";
import { getAdminToken, ResourcePageShell, catchOrEmpty } from "../page-shell";
import MqEndpointsClient from "./client";

export default async function MqEndpointsPage() {
    const token = await getAdminToken();
    const items = await backend.adminMqEndpoints.list(token).catch(catchOrEmpty([]));
    return (
        <ResourcePageShell
            title="Message Queue Endpoints"
            subtitle="RabbitMQ / AMQP brokers tenants can bind to via templates."
            current="MQ Endpoints">
            <MqEndpointsClient items={items as any[]} />
        </ResourcePageShell>
    );
}
