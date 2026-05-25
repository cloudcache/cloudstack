'use server'

import { getAdminUserSession, getBackendToken } from "@/server/utils/action-wrapper.utils";
import PageTitle from "@/components/custom/page-title";
import BreadcrumbSetter from "@/components/breadcrumbs-setter";
import { Separator } from "@/components/ui/separator";
import { TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ServerSettingsTabs } from "./server-settings-tabs";
import { Network, HardDrive, Server, Cpu, Database, Globe, Settings, Shield } from "lucide-react";
import { backend } from "@/server/adapter/backend-api.adapter";
import AdminNodesTab from "./admin-nodes-tab";
import AdminProxyManagersTab from "./admin-proxy-managers-tab";
import AdminRegistriesTab from "./admin-registries-tab";
import AdminResourcePoolsTab from "./admin-resource-pools-tab";
import AdminClustersTab from "./admin-clusters-tab";
import AdminPlatformConfigTab from "./admin-platform-config-tab";
import AdminIpPoolsTab from "./admin-ip-pools-tab";
import AdminDbClustersTab from "./admin-db-clusters-tab";
import { ScrollArea, ScrollBar } from "@/components/ui/scroll-area";

export default async function ServerSettingsPage(props: {
    searchParams: Promise<{ [key: string]: string | string[] | undefined }>
}) {
    const searchParams = await props.searchParams;
    await getAdminUserSession();
    const token = await getBackendToken();

    const [
        nodes,
        proxyManagers,
        registries,
        resourcePools,
        clusters,
        clusterStorage,
        platformConfig,
        ipPools,
        dbClusters,
    ] = await Promise.all([
        backend.adminNodes.list(token).catch(() => []),
        backend.adminProxyManagers.list(token).catch(() => []),
        backend.adminRegistries.list(token).catch(() => []),
        backend.adminPools.list(token).catch(() => []),
        backend.adminClusters.list(token).catch(() => []),
        backend.adminClusters.getStorage(token).catch(() => null),
        backend.adminPlatform.list(token).catch(() => []),
        backend.adminIpPools.list(token).catch(() => []),
        backend.adminDbClusters.list(token).catch(() => []),
    ]);

    const defaultTab = typeof searchParams?.tab === 'string' ? searchParams.tab : 'proxy-managers';

    return (
        <div className="flex-1 space-y-6 pt-6 pb-16">
            <div className="space-y-0.5">
                <PageTitle
                    title={'Server Settings'}
                    subtitle={'Manage infrastructure: proxy managers, nodes, registries, and more.'}>
                </PageTitle>
            </div>
            <BreadcrumbSetter items={[
                { name: "Settings", url: "/settings/profile" },
                { name: "Server" },
            ]} />

            <Separator className="my-6" />

            <ServerSettingsTabs defaultTab={defaultTab}>
                <ScrollArea>
                    <TabsList>
                        <TabsTrigger value="proxy-managers"><Network className="mr-2 h-4 w-4" />Proxy Managers</TabsTrigger>
                        <TabsTrigger value="nodes"><Server className="mr-2 h-4 w-4" />Nodes</TabsTrigger>
                        <TabsTrigger value="registries"><Shield className="mr-2 h-4 w-4" />Registries</TabsTrigger>
                        <TabsTrigger value="resource-pools"><Cpu className="mr-2 h-4 w-4" />Resource Pools</TabsTrigger>
                        <TabsTrigger value="clusters"><HardDrive className="mr-2 h-4 w-4" />Clusters & Storage</TabsTrigger>
                        <TabsTrigger value="platform-config"><Settings className="mr-2 h-4 w-4" />Platform Config</TabsTrigger>
                        <TabsTrigger value="ip-pools"><Globe className="mr-2 h-4 w-4" />IP Pools</TabsTrigger>
                        <TabsTrigger value="db-clusters"><Database className="mr-2 h-4 w-4" />DB Clusters</TabsTrigger>
                    </TabsList>
                    <ScrollBar orientation="horizontal" />
                </ScrollArea>

                <TabsContent value="proxy-managers" className="space-y-4">
                    <AdminProxyManagersTab initialItems={proxyManagers as any[]} />
                </TabsContent>

                <TabsContent value="nodes" className="space-y-4">
                    <AdminNodesTab initialItems={nodes as any[]} />
                </TabsContent>

                <TabsContent value="registries" className="space-y-4">
                    <AdminRegistriesTab initialItems={registries as any[]} />
                </TabsContent>

                <TabsContent value="resource-pools" className="space-y-4">
                    <AdminResourcePoolsTab initialItems={resourcePools as any[]} />
                </TabsContent>

                <TabsContent value="clusters" className="space-y-4">
                    <AdminClustersTab initialItems={clusters as any[]} clusterStorage={clusterStorage} />
                </TabsContent>

                <TabsContent value="platform-config" className="space-y-4">
                    <AdminPlatformConfigTab initialConfig={platformConfig as any[]} />
                </TabsContent>

                <TabsContent value="ip-pools" className="space-y-4">
                    <AdminIpPoolsTab initialItems={ipPools as any[]} />
                </TabsContent>

                <TabsContent value="db-clusters" className="space-y-4">
                    <AdminDbClustersTab initialItems={dbClusters as any[]} />
                </TabsContent>
            </ServerSettingsTabs>
        </div>
    );
}
