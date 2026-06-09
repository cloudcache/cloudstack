'use client'

import { Badge } from "@/components/ui/badge";
import { Table, TableBody, TableCaption, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import Link from "next/link";
import { Button } from "@/components/ui/button";
import { ExternalLink } from "lucide-react";
import { useT } from "@/i18n";

interface BackupSchedule {
    id: string;
    type: 'app' | 'volume';
    app_id: string;
    app_name: string;
    project_id: string;
    project_name: string;
    s3_target_id: string;
    s3_target_name: string;
    cron_expr: string;
    retention_days?: number;
    is_active: boolean;
    backup_type?: string;
    volume_id?: string;
    volume_name?: string;
    mount_path?: string;
}

export default function BackupSchedulesTable({ schedules }: { schedules: BackupSchedule[] }) {
    const t = useT();
    if (schedules.length === 0) return null;

    return (
        <Table>
            <TableCaption>{schedules.length} backup schedule(s)</TableCaption>
            <TableHeader>
                <TableRow>
                    <TableHead>{t("common.status")}</TableHead>
                    <TableHead>{t("common.type")}</TableHead>
                    <TableHead>Project</TableHead>
                    <TableHead>App</TableHead>
                    <TableHead>{t("page.backups.targetVolume")}</TableHead>
                    <TableHead>{t("page.backups.s3Target")}</TableHead>
                    <TableHead>{t("page.backups.schedule")}</TableHead>
                    <TableHead>{t("page.backups.retention")}</TableHead>
                    <TableHead></TableHead>
                </TableRow>
            </TableHeader>
            <TableBody>
                {schedules.map((s) => (
                    <TableRow key={s.id}>
                        <TableCell>
                            <Badge variant={s.is_active ? 'default' : 'secondary'}>
                                {s.is_active ? 'Active' : 'Inactive'}
                            </Badge>
                        </TableCell>
                        <TableCell>
                            <Badge variant="outline">{s.type === 'volume' ? 'Volume' : 'App'}</Badge>
                        </TableCell>
                        <TableCell className="text-sm">{s.project_name}</TableCell>
                        <TableCell className="text-sm font-medium">{s.app_name}</TableCell>
                        <TableCell className="text-sm text-muted-foreground">
                            {s.type === 'volume'
                                ? `${s.volume_name ?? s.volume_id} (${s.mount_path})`
                                : s.backup_type ?? 'app'}
                        </TableCell>
                        <TableCell className="text-sm">{s.s3_target_name}</TableCell>
                        <TableCell>
                            <code className="text-xs bg-muted px-1 py-0.5 rounded">{s.cron_expr}</code>
                        </TableCell>
                        <TableCell className="text-sm">
                            {s.retention_days ? `${s.retention_days}d` : '—'}
                        </TableCell>
                        <TableCell>
                            <Link href={`/project/app/${s.app_id}?tab=${s.type === 'volume' ? 'storage' : 'credentials'}`}>
                                <Button variant="ghost" size="sm">
                                    <ExternalLink className="h-4 w-4" />
                                </Button>
                            </Link>
                        </TableCell>
                    </TableRow>
                ))}
            </TableBody>
        </Table>
    );
}
