import { Card, CardContent } from "@/components/ui/card";
import { AppExtendedModel } from "@/shared/model/app-extended.model";
import { useEffect, useState } from "react";
import { getRessourceDataApp } from "./actions";
import FullLoadingSpinner from "@/components/ui/full-loading-spinnter";
import { PodsResourceInfoModel } from "@/shared/model/pods-resource-info.model";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { KubeSizeConverter } from "@/shared/utils/kubernetes-size-converter.utils";

const finiteNumber = (value: unknown): number | undefined =>
    typeof value === 'number' && Number.isFinite(value) ? value : undefined;

const normalizePodMetrics = (value: any): PodsResourceInfoModel => ({
    cpuPercent: finiteNumber(value?.cpuPercent ?? value?.cpu_percent) ?? 0,
    cpuAbsolutCores: finiteNumber(value?.cpuAbsolutCores ?? value?.cpu_absolut_cores ?? value?.cpu_absolute_cores) ?? 0,
    ramPercent: finiteNumber(value?.ramPercent ?? value?.ram_percent) ?? 0,
    ramAbsolutBytes: finiteNumber(value?.ramAbsolutBytes ?? value?.ram_absolut_bytes ?? value?.ram_absolute_bytes) ?? 0,
});

const errorMessage = (value: unknown, fallback: string) => {
    if (value instanceof Error) return value.message;
    if (typeof value === 'object' && value && 'message' in value && typeof value.message === 'string') return value.message;
    return fallback;
};

export default function MonitoringTab({
    app
}: {
    app: AppExtendedModel;
}) {

    const [selectedPod, setSelectedPod] = useState<PodsResourceInfoModel | undefined>(undefined);
    const [error, setError] = useState<string | undefined>(undefined);

    const updateValues = async () => {
        setError(undefined);
        try {
            const response = await getRessourceDataApp(app.projectId, app.id);
            if (response.status === 'success' && response.data) {
                setSelectedPod(normalizePodMetrics(response.data));

            } else {
                setError(errorMessage(response, 'Could not load metrics for this app.'));
            }
        } catch (ex) {
            setError(errorMessage(ex, 'Could not load metrics for this app.'));
        }
    }

    useEffect(() => {
        updateValues();
        const intervalId = setInterval(updateValues, 10000);
        return () => clearInterval(intervalId);
    }, [app]);

    const cpuPercent = finiteNumber(selectedPod?.cpuPercent);
    const cpuAbsolutCores = finiteNumber(selectedPod?.cpuAbsolutCores);
    const ramAbsolutBytes = finiteNumber(selectedPod?.ramAbsolutBytes) ?? 0;

    return <>
        <Card>
            <CardContent className="pb-0">
                {error ? <p className="py-4 text-sm text-destructive">{error}</p> : !selectedPod ? <FullLoadingSpinner /> :
                    <Table>
                        <TableHeader>
                            <TableRow>
                                <TableHead>CPU %</TableHead>
                                <TableHead>RAM</TableHead>
                            </TableRow>
                        </TableHeader>
                        <TableBody>
                            <TableRow>
                                <TableCell className="font-medium">
                                    <TooltipProvider>
                                        <Tooltip delayDuration={200}>
                                            <TooltipTrigger asChild>
                                                <div className={'px-3 py-1.5 rounded cursor-pointer'}>{cpuPercent !== undefined ? cpuPercent.toFixed(2) : '0.00'}</div>
                                            </TooltipTrigger>
                                            <TooltipContent>
                                                <p className="max-w-[350px]">{cpuAbsolutCores !== undefined ? cpuAbsolutCores.toFixed(10) : '0.0000000000'} cores</p>
                                            </TooltipContent>
                                        </Tooltip>
                                    </TooltipProvider>
                                </TableCell>
                                <TableCell className="font-medium">{KubeSizeConverter.convertBytesToReadableSize(ramAbsolutBytes)}</TableCell>
                            </TableRow>
                        </TableBody>
                    </Table>
                }
            </CardContent>
        </Card >
    </>;
}
