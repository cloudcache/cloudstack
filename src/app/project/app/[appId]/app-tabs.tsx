'use client'

import { useRouter } from "next/navigation";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import GeneralAppRateLimits from "./general/app-rate-limits";
import GeneralAppSource from "./general/app-source";
import GeneralAppContainerConfig from "./general/app-container-config";
import EnvEdit from "./environment/env-edit";
import DomainsList from "./domains/domains";
import StorageList from "./volumes/storages";
import { BackendApp } from "@/server/adapter/backend-api.adapter";
import BuildsTab from "./overview/deployments";
import Logs from "./overview/logs";
import MonitoringTab from "./overview/monitoring-app";
import InternalHostnames from "./domains/ports-and-internal-hostnames";
import FileMount from "./volumes/file-mount";
import WebhookDeploymentInfo from "./overview/webhook-deployment";
import DbCredentials from "./credentials/db-crendentials";
import VolumeBackupList from "./volumes/volume-backup";
import BasicAuth from "./advanced/basic-auth";
import NetworkPolicy from "./advanced/network-policy";
import HealthCheckSettings from "./advanced/health-check-settings";
import { ScrollArea, ScrollBar } from "@/components/ui/scroll-area";
import DbToolsCard from "./credentials/db-tools";
import { RolePermissionEnum } from "@/shared/model/role-extended.model.ts";
import { NodeInfoModel } from "@/shared/model/node-info.model";
import { Eye, Key, Settings, Zap, Globe, HardDrive, Cog } from "lucide-react";

export default function AppTabs({
    app,
    role,
    tabName,
    s3Targets,
    volumeBackups,
    nodesInfo
}: {
    app: BackendApp;
    role: RolePermissionEnum;
    tabName: string;
    s3Targets: { id: string; name: string }[];
    volumeBackups: unknown[];
    nodesInfo: NodeInfoModel[];
}) {
    const router = useRouter();
    const readonly = role !== RolePermissionEnum.READWRITE;
    const openTab = (tabName: string) => {
        router.push(`/project/app/${app.id}?tabName=${tabName}`);
    }

    // Child components still use AppExtendedModel types (to be migrated in Phase C).
    // Cast here so the layout compiles while individual tabs are progressively updated.
    const appAny = app as any;

    return (
        <Tabs defaultValue="general" value={tabName} onValueChange={(newTab) => openTab(newTab)} className="space-y-4">
            <ScrollArea>
                <TabsList>
                    <TabsTrigger value="overview"><Eye className="mr-2 h-4 w-4" />Overview</TabsTrigger>
                    {app.app_type !== 'APP' && <TabsTrigger value="credentials"><Key className="mr-2 h-4 w-4" />Credentials</TabsTrigger>}
                    <TabsTrigger value="general"><Settings className="mr-2 h-4 w-4" />General</TabsTrigger>
                    <TabsTrigger value="environment"><Zap className="mr-2 h-4 w-4" />Environment</TabsTrigger>
                    <TabsTrigger value="domains"><Globe className="mr-2 h-4 w-4" />Domains</TabsTrigger>
                    <TabsTrigger value="storage"><HardDrive className="mr-2 h-4 w-4" />Storage</TabsTrigger>
                    <TabsTrigger value="advanced"><Cog className="mr-2 h-4 w-4" />Advanced</TabsTrigger>
                </TabsList>
                <ScrollBar orientation="horizontal" />
            </ScrollArea>
            <TabsContent value="overview" className="grid grid-cols-1 3xl:grid-cols-2 gap-4">
                <MonitoringTab app={appAny} />
                <Logs role={role} app={appAny} />
                <BuildsTab role={role} app={appAny} />
                <WebhookDeploymentInfo role={role} app={appAny} />
            </TabsContent>
            {app.app_type !== 'APP' && <TabsContent value="credentials" className="space-y-4">
                {role === RolePermissionEnum.READWRITE && <DbToolsCard app={appAny} />}
                <DbCredentials app={appAny} />
            </TabsContent>}
            <TabsContent value="general" className="space-y-4">
                <GeneralAppSource readonly={readonly} app={appAny} />
                <GeneralAppRateLimits readonly={readonly} app={appAny} />
                <GeneralAppContainerConfig readonly={readonly} app={appAny} />
            </TabsContent>
            <TabsContent value="environment" className="space-y-4">
                <EnvEdit readonly={readonly} app={appAny} />
            </TabsContent>
            <TabsContent value="domains" className="space-y-4">
                <DomainsList readonly={readonly} app={appAny} />
                <InternalHostnames readonly={readonly} app={appAny} />
            </TabsContent>
            <TabsContent value="storage" className="space-y-4">
                <StorageList readonly={readonly} app={appAny} nodesInfo={nodesInfo} />
                <FileMount readonly={readonly} app={appAny} />
                <VolumeBackupList
                    readonly={readonly}
                    app={appAny}
                    s3Targets={s3Targets as any}
                    volumeBackups={volumeBackups as any} />
            </TabsContent>
            <TabsContent value="advanced" className="space-y-4">
                <BasicAuth readonly={readonly} app={appAny} />
                <NetworkPolicy readonly={readonly} app={appAny} />
                <HealthCheckSettings readonly={readonly} app={appAny} />
            </TabsContent>
        </Tabs>
    )
}
