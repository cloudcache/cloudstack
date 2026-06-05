'use server'

import { getAuthUserSession, getBackendToken, simpleAction } from "@/server/utils/action-wrapper.utils";
import { backend } from "@/server/adapter/backend-api.adapter";
import { ServerActionResult, SuccessActionResult } from "@/shared/model/server-action-error-return.model";
import { AppMonitoringUsageModel } from "@/shared/model/app-monitoring-usage.model";
import { AppVolumeMonitoringUsageModel } from "@/shared/model/app-volume-monitoring-usage.model";
import { NodeResourceModel } from "@/shared/model/node-resource.model";

export const getMonitoringForAllApps = async () =>
    simpleAction(async () => {
        await getAuthUserSession();
        const token = await getBackendToken();
        const apps = await backend.monitoring.apps(token);

        // Map Rust response → AppMonitoringUsageModel
        const mapped: AppMonitoringUsageModel[] = apps.map(app => ({
            appId: app.app_id,
            appName: app.app_name,
            projectId: app.project_id,
            projectName: app.project_name,
            // cpu_mcores → cores; percentage of 1 CPU (1000 mcores)
            cpuUsage: app.cpu_mcores / 1000,
            cpuUsagePercent: app.cpu_mcores / 10,   // 1000 mcores = 100%
            ramUsageBytes: app.ram_bytes,
        }));

        return new SuccessActionResult(mapped);
    }) as Promise<ServerActionResult<unknown, AppMonitoringUsageModel[]>>;

export const getVolumeMonitoringUsage = async () =>
    simpleAction(async () => {
        await getAuthUserSession();
        const token = await getBackendToken();
        const volumes = await backend.monitoring.managedVolumes(token);

        // Map Rust response → AppVolumeMonitoringUsageModel
        const mapped: AppVolumeMonitoringUsageModel[] = volumes.map(v => ({
            appId: v.app_id,
            appName: v.app_name,
            projectId: v.project_id,
            projectName: v.project_name,
            mountPath: v.container_mount_path,
            usedBytes: v.usage_bytes ?? 0,
            capacityBytes: v.usage_bytes ?? 1,   // avoid division by zero; 1 byte if unknown
            isBaseVolume: true,                  // all managed volumes are base volumes
        }));

        return new SuccessActionResult(mapped);
    }) as Promise<ServerActionResult<unknown, AppVolumeMonitoringUsageModel[]>>;

export const getNodeResourceUsage = async () =>
    simpleAction(async () => {
        await getAuthUserSession();
        const token = await getBackendToken();
        try {
            const agg = await backend.monitoring.nodesAggregate(token);

            // Map aggregate → NodeResourceModel array (single "cluster" entry)
            const cpuCapacityMcores = agg.total_cpu_capacity_mcores;
            const ramCapacityMb = agg.total_mem_capacity_mb;

            const nodes: NodeResourceModel[] = [{
                name: 'Cluster Aggregate',
                cpuUsage: agg.avg_cpu_used_pct / 100,
                cpuCapacity: cpuCapacityMcores / 1000,   // convert to cores
                ramUsage: agg.total_mem_used_bytes,
                ramCapacity: ramCapacityMb * 1024 * 1024, // MB → bytes
                diskUsageAbsolut: agg.total_disk_used_bytes,
                diskUsageCapacity: agg.total_disk_total_bytes,
                diskUsageReserved: 0,
                diskSpaceSchedulable: agg.total_disk_total_bytes - agg.total_disk_used_bytes,
            }];

            return new SuccessActionResult(nodes);
        } catch {
            // Non-admin users can't access node aggregate
            return new SuccessActionResult([] as NodeResourceModel[]);
        }
    }) as Promise<ServerActionResult<unknown, NodeResourceModel[]>>;
