'use client';

import {
  Label,
  PolarGrid,
  PolarRadiusAxis,
  RadialBar,
  RadialBarChart,
  Pie,
  PieChart,
} from 'recharts';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import { ChartConfig, ChartContainer, ChartTooltip, ChartTooltipContent } from '@/components/ui/chart';
import { NodeResourceModel } from '@/shared/model/node-resource.model';
import {
  useBreadcrumbs,
} from '@/frontend/states/zustand.states';
import { useEffect, useState, useMemo } from 'react';
import ChartDiskRessources from './disk-chart';
import { Actions } from '@/frontend/utils/nextjs-actions.utils';
import { getNodeResourceUsage } from './actions';
import { toast } from 'sonner';
import FullLoadingSpinner from '@/components/ui/full-loading-spinnter';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { KubeSizeConverter } from '@/shared/utils/kubernetes-size-converter.utils';
import { Progress } from '@/components/ui/progress';
import { Button } from '@/components/ui/button';
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle, SheetTrigger } from '@/components/ui/sheet';
import { Activity, Cpu, HardDrive, MemoryStick } from 'lucide-react';
import { useT } from '@/i18n';

export default function ResourcesNodes({
  resourcesNodes,
}: {
  resourcesNodes?: NodeResourceModel[];
}) {
  const t = useT();

  const [updatedNodeRessources, setUpdatedResourcesNodes] = useState<NodeResourceModel[] | undefined>(resourcesNodes);

  const fetchResourcesNodes = async () => {
    try {
      const data = await Actions.run(() => getNodeResourceUsage());
      setUpdatedResourcesNodes(data);
    } catch (ex) {
      toast.error(t('monitoring.nodes.fetchError'));
      console.error(t('monitoring.nodes.fetchError'), ex);
    }
  }

  useEffect(() => {
    const intervalId = setInterval(() => fetchResourcesNodes(), 5000);
    return () => {
      clearInterval(intervalId);
    }
  }, [resourcesNodes]);

  const { setBreadcrumbs } = useBreadcrumbs();
  useEffect(
    () => setBreadcrumbs([{ name: 'Monitoring', url: '/monitoring' }]
    ), []);

  const clusterStats = useMemo(() => {
    if (!updatedNodeRessources) return {
      cpuUsage: 0, cpuCapacity: 1,
      ramUsage: 0, ramCapacity: 1,
      diskUsageAbsolut: 0, diskUsageReserved: 0, diskCapacity: 1
    };

    return updatedNodeRessources.reduce((acc, node) => ({
      cpuUsage: acc.cpuUsage + node.cpuUsage,
      cpuCapacity: acc.cpuCapacity + node.cpuCapacity,
      ramUsage: acc.ramUsage + node.ramUsage,
      ramCapacity: acc.ramCapacity + node.ramCapacity,
      diskUsageAbsolut: acc.diskUsageAbsolut + node.diskUsageAbsolut,
      diskUsageReserved: acc.diskUsageReserved + node.diskUsageReserved,
      diskCapacity: acc.diskCapacity + node.diskUsageCapacity,
    }), {
      cpuUsage: 0, cpuCapacity: 0,
      ramUsage: 0, ramCapacity: 0,
      diskUsageAbsolut: 0, diskUsageReserved: 0, diskCapacity: 0
    });
  }, [updatedNodeRessources]);

  const getUsageColor = (percentage: number) => {
    if (percentage >= 90) return "hsl(var(--chart-1))";
    if (percentage >= 80) return "hsl(var(--chart-4))";
    return "hsl(var(--chart-2))";
  };

  const pieChartConfig = {
    used: {
      label: t('monitoring.nodes.used'),
      color: "hsl(var(--chart-1))",
    },
    free: {
      label: t('monitoring.nodes.free'),
      color: "hsl(var(--muted))",
    },
  } satisfies ChartConfig;

  const storagePieChartConfig = {
    used: {
      label: t('monitoring.nodes.used'),
      color: "hsl(var(--chart-1))",
    },
    reserved: {
      label: t('monitoring.nodes.reserved'),
      color: "hsl(var(--chart-2))",
    },
    free: {
      label: t('monitoring.nodes.free'),
      color: "hsl(var(--muted))",
    },
  } satisfies ChartConfig;

  const getChartData = (used: number, capacity: number) => {
    const percentage = capacity > 0 ? (used / capacity) * 100 : 0;
    return [
      { status: 'used', value: used, fill: getUsageColor(percentage) },
      { status: 'free', value: Math.max(0, capacity - used), fill: 'var(--color-free)' },
    ];
  };

  const getStorageChartData = (used: number, reserved: number, capacity: number) => {
    return [
      { status: 'used', value: used, fill: "hsl(var(--chart-1))" },
      { status: 'reserved', value: reserved, fill: "hsl(var(--chart-2))" },
      { status: 'free', value: Math.max(0, capacity - used - reserved), fill: 'var(--color-free)' },
    ];
  };

  if (!updatedNodeRessources) {
    return <FullLoadingSpinner />
  }

  return (
    <div className="space-y-6">
      <div className="grid gap-4 md:grid-cols-3">
        {/* Cluster CPU */}
        <Card className="flex flex-col">
          <CardHeader className="items-center pb-0">
            <CardTitle>{t('monitoring.nodes.clusterCpu')}</CardTitle>
            <CardDescription>{t('monitoring.nodes.totalCoresUsage')}</CardDescription>
          </CardHeader>
          <CardContent className="flex-1 pb-0">
            <ChartContainer config={pieChartConfig} className="mx-auto aspect-square max-h-[250px]">
              <PieChart>
                <ChartTooltip cursor={false} content={<ChartTooltipContent hideLabel />} />
                <Pie data={getChartData(clusterStats.cpuUsage, clusterStats.cpuCapacity)} dataKey="value" nameKey="status" innerRadius={60} strokeWidth={5}>
                  <Label content={({ viewBox }) => {
                    if (viewBox && "cx" in viewBox && "cy" in viewBox) {
                      return (
                        <text x={viewBox.cx} y={viewBox.cy} textAnchor="middle" dominantBaseline="middle">
                          <tspan x={viewBox.cx} y={viewBox.cy} className="fill-foreground text-3xl font-bold">
                            {((clusterStats.cpuUsage / clusterStats.cpuCapacity) * 100).toFixed(0)}%
                          </tspan>
                          <tspan x={viewBox.cx} y={(viewBox.cy || 0) + 24} className="fill-muted-foreground">
                            {t('monitoring.nodes.used')}
                          </tspan>
                        </text>
                      )
                    }
                  }} />
                </Pie>
              </PieChart>
            </ChartContainer>
          </CardContent>
        </Card>

        {/* Cluster RAM */}
        <Card className="flex flex-col">
          <CardHeader className="items-center pb-0">
            <CardTitle>{t('monitoring.nodes.clusterRam')}</CardTitle>
            <CardDescription>{t('monitoring.nodes.totalMemoryUsage')}</CardDescription>
          </CardHeader>
          <CardContent className="flex-1 pb-0">
            <ChartContainer config={pieChartConfig} className="mx-auto aspect-square max-h-[250px]">
              <PieChart>
                <ChartTooltip cursor={false} content={<ChartTooltipContent hideLabel formatter={(value) => KubeSizeConverter.convertBytesToReadableSize(value as number)} />} />
                <Pie data={getChartData(clusterStats.ramUsage, clusterStats.ramCapacity)} dataKey="value" nameKey="status" innerRadius={60} strokeWidth={5}>
                  <Label content={({ viewBox }) => {
                    if (viewBox && "cx" in viewBox && "cy" in viewBox) {
                      return (
                        <text x={viewBox.cx} y={viewBox.cy} textAnchor="middle" dominantBaseline="middle">
                          <tspan x={viewBox.cx} y={viewBox.cy} className="fill-foreground text-3xl font-bold">
                            {((clusterStats.ramUsage / clusterStats.ramCapacity) * 100).toFixed(0)}%
                          </tspan>
                          <tspan x={viewBox.cx} y={(viewBox.cy || 0) + 24} className="fill-muted-foreground">
                            {t('monitoring.nodes.used')}
                          </tspan>
                        </text>
                      )
                    }
                  }} />
                </Pie>
              </PieChart>
            </ChartContainer>
          </CardContent>
        </Card>

        {/* Cluster Storage */}
        <Card className="flex flex-col">
          <CardHeader className="items-center pb-0">
            <CardTitle>{t('monitoring.nodes.clusterStorage')}</CardTitle>
            <CardDescription>{t('monitoring.nodes.totalDiskUsage')}</CardDescription>
          </CardHeader>
          <CardContent className="flex-1 pb-0">
            <ChartContainer config={storagePieChartConfig} className="mx-auto aspect-square max-h-[250px]">
              <PieChart>
                <ChartTooltip cursor={false} content={<ChartTooltipContent hideLabel formatter={(value) => {
                  if (value === clusterStats.diskUsageAbsolut) {
                    return KubeSizeConverter.convertBytesToReadableSize(clusterStats.diskUsageAbsolut) + ` (${t('monitoring.nodes.used')})`;
                  }
                  if (value === clusterStats.diskUsageReserved) {
                    return KubeSizeConverter.convertBytesToReadableSize(clusterStats.diskUsageReserved) + ` (${t('monitoring.nodes.freeButUnusable')})`;
                  }
                  return KubeSizeConverter.convertBytesToReadableSize(value as number) + ` (${t('monitoring.nodes.free')})`;
                }} />} />
                <Pie data={getStorageChartData(clusterStats.diskUsageAbsolut, clusterStats.diskUsageReserved, clusterStats.diskCapacity)} dataKey="value" nameKey="status" innerRadius={60} strokeWidth={5}>
                  <Label content={({ viewBox }) => {
                    if (viewBox && "cx" in viewBox && "cy" in viewBox) {
                      return (
                        <text x={viewBox.cx} y={viewBox.cy} textAnchor="middle" dominantBaseline="middle">
                          <tspan x={viewBox.cx} y={viewBox.cy} className="fill-foreground text-3xl font-bold">
                            {(((clusterStats.diskUsageAbsolut + clusterStats.diskUsageReserved) / clusterStats.diskCapacity) * 100).toFixed(0)}%
                          </tspan>
                          <tspan x={viewBox.cx} y={(viewBox.cy || 0) + 24} className="fill-muted-foreground">
                            {t('monitoring.nodes.used')}
                          </tspan>
                        </text>
                      )
                    }
                  }} />
                </Pie>
              </PieChart>
            </ChartContainer>
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>{t('monitoring.nodes.nodeResources')}</CardTitle>
          <CardDescription>{t('monitoring.nodes.nodeResourcesDescription')}</CardDescription>
        </CardHeader>
        <CardContent>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>{t('monitoring.nodes.nodeName')}</TableHead>
                <TableHead>CPU</TableHead>
                <TableHead>RAM</TableHead>
                <TableHead>{t('plans.storage')}</TableHead>
                <TableHead className="text-right">{t('common.actions')}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {updatedNodeRessources.map((node) => (
                <TableRow key={node.name}>
                  <TableCell className="font-medium">{node.name}</TableCell>
                  <TableCell className="w-[25%]">
                    <div className="space-y-1">
                      <div className="flex justify-between text-xs text-muted-foreground">
                        <span>{((node.cpuUsage / node.cpuCapacity) * 100).toFixed(0)}%</span>
                        <span>{node.cpuUsage.toFixed(2)} / {node.cpuCapacity} Cores</span>
                      </div>
                      <Progress value={(node.cpuUsage / node.cpuCapacity) * 100} className="h-2" />
                    </div>
                  </TableCell>
                  <TableCell className="w-[25%]">
                    <div className="space-y-1">
                      <div className="flex justify-between text-xs text-muted-foreground">
                        <span>{((node.ramUsage / node.ramCapacity) * 100).toFixed(0)}%</span>
                        <span>{KubeSizeConverter.convertBytesToReadableSize(node.ramUsage)} / {KubeSizeConverter.convertBytesToReadableSize(node.ramCapacity)}</span>
                      </div>
                      <Progress value={(node.ramUsage / node.ramCapacity) * 100} className="h-2" />
                    </div>
                  </TableCell>
                  <TableCell className="w-[25%]">
                    <div className="space-y-1">
                      <div className="flex justify-between text-xs text-muted-foreground">
                        <span>{(((node.diskUsageAbsolut + node.diskUsageReserved) / node.diskUsageCapacity) * 100).toFixed(0)}%</span>
                        <span>{KubeSizeConverter.convertBytesToReadableSize(node.diskUsageAbsolut + node.diskUsageReserved)} / {KubeSizeConverter.convertBytesToReadableSize(node.diskUsageCapacity)}</span>
                      </div>
                      <Progress value={((node.diskUsageAbsolut + node.diskUsageReserved) / node.diskUsageCapacity) * 100} className="h-2" />
                    </div>
                  </TableCell>
                  <TableCell className="text-right">
                    <NodeDetailsSheet node={node} />
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </CardContent>
      </Card>
    </div>
  );
}

function NodeDetailsSheet({ node }: { node: NodeResourceModel }) {
  const t = useT();
  const chartData = [
    { browser: 'safari', usage: 1, fill: 'var(--color-safari)' },
  ];

  const chartConfig = {
    usage: {
      label: t('monitoring.nodes.usage'),
    },
    safari: {
      label: 'Safari',
      color: 'hsl(var(--chart-2))',
    },
  } satisfies ChartConfig;

  return (
    <Sheet>
      <SheetTrigger asChild>
        <Button variant="outline" size="sm">{t('monitoring.nodes.viewDetails')}</Button>
      </SheetTrigger>
      <SheetContent className="overflow-y-auto sm:max-w-xl">
        <SheetHeader>
          <SheetTitle className="flex items-center gap-2">
            <Activity className="h-5 w-5" />
            {node.name}
          </SheetTitle>
          <SheetDescription>
            {t('monitoring.nodes.detailsDescription')}
          </SheetDescription>
        </SheetHeader>

        <div className="grid gap-6 py-6">
          {/* CPU Chart */}
          <Card>
            <CardHeader className="pb-2">
              <CardTitle className="text-sm font-medium flex items-center gap-2">
                <Cpu className="h-4 w-4" /> {t('monitoring.nodes.cpuUsage')}
              </CardTitle>
            </CardHeader>
            <CardContent>
              <ChartContainer
                config={chartConfig}
                className="mx-auto aspect-square max-h-[250px]"
              >
                <RadialBarChart
                  data={chartData}
                  startAngle={0}
                  endAngle={360 * node.cpuUsage / node.cpuCapacity}
                  innerRadius={80}
                  outerRadius={110}
                >
                  <PolarGrid
                    gridType="circle"
                    radialLines={false}
                    stroke="none"
                    className="first:fill-muted last:fill-background"
                    polarRadius={[86, 74]}
                  />
                  <RadialBar
                    dataKey="usage"
                    background
                    cornerRadius={10}
                  />
                  <PolarRadiusAxis
                    tick={false}
                    tickLine={false}
                    axisLine={false}
                  >
                    <Label
                      content={({ viewBox }) => {
                        if (viewBox && 'cx' in viewBox && 'cy' in viewBox) {
                          return (
                            <text
                              x={viewBox.cx}
                              y={viewBox.cy}
                              textAnchor="middle"
                              dominantBaseline="middle"
                            >
                              <tspan
                                x={viewBox.cx}
                                y={(viewBox.cy || 0) - 10}
                                className="fill-foreground text-4xl font-bold"
                              >
                                {(node.cpuUsage / node.cpuCapacity * 100).toFixed(0)}%
                              </tspan>
                              <tspan
                                x={viewBox.cx}
                                y={(viewBox.cy || 0) + 14}
                                className="fill-muted-foreground"
                              >
                                CPU
                              </tspan>
                              <tspan
                                x={viewBox.cx}
                                y={(viewBox.cy || 0) + 30}
                                className="fill-muted-foreground"
                              >
                                {t('monitoring.nodes.load')}: {(node.cpuUsage).toFixed(2)}
                              </tspan>
                            </text>
                          );
                        }
                      }}
                    />
                  </PolarRadiusAxis>
                </RadialBarChart>
              </ChartContainer>
            </CardContent>
          </Card>

          {/* RAM Chart */}
          <Card>
            <CardHeader className="pb-2">
              <CardTitle className="text-sm font-medium flex items-center gap-2">
                <MemoryStick className="h-4 w-4" /> {t('monitoring.nodes.memoryUsage')}
              </CardTitle>
            </CardHeader>
            <CardContent>
              <ChartContainer
                config={chartConfig}
                className="mx-auto aspect-square max-h-[250px]"
              >
                <RadialBarChart
                  data={chartData}
                  startAngle={0}
                  endAngle={360 * node.ramUsage / node.ramCapacity}
                  innerRadius={80}
                  outerRadius={110}
                >
                  <PolarGrid
                    gridType="circle"
                    radialLines={false}
                    stroke="none"
                    className="first:fill-muted last:fill-background"
                    polarRadius={[86, 74]}
                  />
                  <RadialBar
                    dataKey="usage"
                    background
                    cornerRadius={10}
                  />
                  <PolarRadiusAxis
                    tick={false}
                    tickLine={false}
                    axisLine={false}
                  >
                    <Label
                      content={({ viewBox }) => {
                        if (viewBox && 'cx' in viewBox && 'cy' in viewBox) {
                          return (
                            <text
                              x={viewBox.cx}
                              y={viewBox.cy}
                              textAnchor="middle"
                              dominantBaseline="middle"
                            >
                              <tspan
                                x={viewBox.cx}
                                y={(viewBox.cy || 0) - 10}
                                className="fill-foreground text-4xl font-bold"
                              >
                                {(node.ramUsage / node.ramCapacity * 100).toFixed(0)}%
                              </tspan>
                              <tspan
                                x={viewBox.cx}
                                y={(viewBox.cy || 0) + 14}
                                className="fill-muted-foreground"
                              >
                                RAM
                              </tspan>
                              <tspan
                                x={viewBox.cx}
                                y={(viewBox.cy || 0) + 30}
                                className="fill-muted-foreground"
                              >
                                {(node.ramUsage / (1024 * 1024 * 1024)).toFixed(2)} / {KubeSizeConverter.convertBytesToReadableSize(node.ramCapacity)}
                              </tspan>
                            </text>
                          );
                        }
                      }}
                    />
                  </PolarRadiusAxis>
                </RadialBarChart>
              </ChartContainer>
            </CardContent>
          </Card>

          {/* Disk Chart */}
          <Card>
            <CardHeader className="pb-2">
              <CardTitle className="text-sm font-medium flex items-center gap-2">
                <HardDrive className="h-4 w-4" /> {t('monitoring.nodes.storageUsage')}
              </CardTitle>
            </CardHeader>
            <CardContent>
              <ChartDiskRessources nodeRessource={node} />
            </CardContent>
          </Card>
        </div>
      </SheetContent>
    </Sheet>
  );
}
