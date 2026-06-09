'use client';

import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import AppTable from "./apps-table";
import ProjectNetworkGraph from "./project-network-graph";
import { useRouter, useSearchParams } from "next/navigation";
import { Table, Network } from "lucide-react";
import { useT } from "@/i18n";

interface ProjectOverviewProps {
    apps: any[]; // Using any to avoid complex type imports, as we know the data structure is correct
    projectId: string;
    canCreateApps: boolean;
    canDeleteApps: boolean;
}

export default function ProjectOverview({ apps, projectId, canCreateApps, canDeleteApps }: ProjectOverviewProps) {
    const router = useRouter();
    const searchParams = useSearchParams();
    const t = useT();
    const currentTab = searchParams.get('tab') || 'table';

    const handleTabChange = (value: string) => {
        router.push(`?tab=${value}`, { scroll: false });
    };

    return (
        <Tabs value={currentTab} onValueChange={handleTabChange} className="w-full">
            <TabsList>
                <TabsTrigger value="table"><Table className="mr-2 h-4 w-4" />{t("page.project.tableView")}</TabsTrigger>
                <TabsTrigger value="graph"><Network className="mr-2 h-4 w-4" />{t("page.project.networkGraph")}</TabsTrigger>
            </TabsList>
            <TabsContent value="table">
                <AppTable
                    app={apps}
                    projectId={projectId}
                    canCreateApps={canCreateApps}
                    canDeleteApps={canDeleteApps} />
            </TabsContent>
            <TabsContent value="graph">
                <ProjectNetworkGraph apps={apps} />
            </TabsContent>
        </Tabs>
    );
}
