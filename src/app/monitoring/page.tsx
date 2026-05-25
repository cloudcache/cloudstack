'use server'

import { getAuthUserSession, getBackendToken } from "@/server/utils/action-wrapper.utils";
import PageTitle from "@/components/custom/page-title";
import { backend } from "@/server/adapter/backend-api.adapter";
import ResourceNodes from "./monitoring-nodes";
import AppRessourceMonitoring from "./app-monitoring";
import AppVolumeMonitoring from "./app-volumes-monitoring";
import { AppMonitoringUsageModel } from "@/shared/model/app-monitoring-usage.model";
import { AppVolumeMonitoringUsageModel } from "@/shared/model/app-volume-monitoring-usage.model";
import { NodeResourceModel } from "@/shared/model/node-resource.model";
import BreadcrumbSetter from "@/components/breadcrumbs-setter";

export default async function MonitoringPage() {
    await getAuthUserSession();
    const token = await getBackendToken();

    let appsUsage: AppMonitoringUsageModel[] | undefined;
    let volumesUsage: AppVolumeMonitoringUsageModel[] | undefined;
    let nodeResources: NodeResourceModel[] | undefined;

    try {
        const [appsRaw, volumesRaw] = await Promise.all([
            backend.monitoring.apps(token),
            backend.monitoring.managedVolumes(token),
        ]);

        appsUsage = appsRaw.map(app => ({
            appId: app.app_id,
            appName: app.app_name,
            projectId: app.project_id,
            projectName: app.project_name,
            cpuUsage: app.cpu_mcores / 1000,
            cpuUsagePercent: app.cpu_mcores / 10,
            ramUsageBytes: app.ram_bytes,
        }));

        volumesUsage = volumesRaw.map(v => ({
            appId: v.app_id,
            appName: v.app_name,
            projectId: v.project_id,
            projectName: v.project_name,
            mountPath: v.container_mount_path,
            usedBytes: v.usage_bytes ?? 0,
            capacityBytes: v.usage_bytes ?? 1,
            isBaseVolume: true,
        }));
    } catch {
        // components show spinner/error on undefined
    }

    // Node aggregate is admin-only — gracefully skip for regular users
    try {
        const agg = await backend.monitoring.nodesAggregate(token);
        nodeResources = [{
            name: 'Cluster Aggregate',
            cpuUsage: agg.avg_cpu_used_pct / 100,
            cpuCapacity: agg.total_cpu_capacity_mcores / 1000,
            ramUsage: agg.total_mem_used_bytes,
            ramCapacity: agg.total_mem_capacity_mb * 1024 * 1024,
            diskUsageAbsolut: agg.total_disk_used_bytes,
            diskUsageCapacity: agg.total_disk_total_bytes,
            diskUsageReserved: 0,
            diskSpaceSchedulable: agg.total_disk_total_bytes - agg.total_disk_used_bytes,
        }];
    } catch {
        // Non-admin — skip node section
    }

    return (
        <div className="flex-1 space-y-4 pt-6">
            <PageTitle
                title={'Monitoring'}
                subtitle={'View resource usage across all apps and volumes in your cluster.'}>
            </PageTitle>
            <BreadcrumbSetter items={[{ name: "Monitoring" }]} />
            <div className="space-y-6">
                {nodeResources && <ResourceNodes resourcesNodes={nodeResources} />}
                <AppRessourceMonitoring appsRessourceUsage={appsUsage} />
                <AppVolumeMonitoring volumesUsage={volumesUsage} />
            </div>
        </div>
    );
}
