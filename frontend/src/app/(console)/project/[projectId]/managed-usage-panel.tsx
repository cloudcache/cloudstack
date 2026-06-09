'use client'

// Project-level "managed resources" usage card (P2c).
// Shows current usage vs. plan-enforced limit for each managed-service kind.
// Limit `0` means "unlimited" per the platform convention.

import { useEffect, useState } from "react";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { fetchManagedUsage } from "./actions";

type Usage = Awaited<ReturnType<typeof fetchManagedUsage>>;

const kinds: Array<{ key: keyof Usage; label: string }> = [
    { key: 'db_instances',   label: 'Database instances' },
    { key: 'mq_bindings',    label: 'MQ bindings' },
    { key: 'smtp_bindings',  label: 'SMTP bindings' },
    { key: 'redis_bindings', label: 'Redis bindings' },
    { key: 's3_bindings',    label: 'S3 bindings' },
];

export default function ManagedUsagePanel({ projectId }: { projectId: string }) {
    const [data, setData] = useState<Usage | null>(null);

    useEffect(() => {
        fetchManagedUsage(projectId).then(setData).catch(() => setData(null));
    }, [projectId]);

    if (!data) return null;

    return (
        <Card>
            <CardHeader>
                <CardTitle>Managed Resources</CardTitle>
                <CardDescription>
                    Bindings from this project to managed services (DB, cache, S3, MQ, SMTP). Subject to plan limits.
                </CardDescription>
            </CardHeader>
            <CardContent>
                <div className="grid grid-cols-2 md:grid-cols-5 gap-3">
                    {kinds.map(k => {
                        const v = (data as any)[k.key] as { used: number; limit: number };
                        const unlimited = !v.limit;
                        const pct = unlimited ? 0 : Math.min(100, (v.used / v.limit) * 100);
                        const danger = !unlimited && pct >= 90;
                        return (
                            <div key={k.key} className="space-y-1">
                                <div className="text-xs text-muted-foreground">{k.label}</div>
                                <div className="text-sm font-medium">
                                    {v.used} / {unlimited ? '∞' : v.limit}
                                </div>
                                {!unlimited && (
                                    <div className="h-1.5 bg-muted rounded overflow-hidden">
                                        <div className={`h-full ${danger ? 'bg-destructive' : 'bg-primary'}`}
                                             style={{ width: `${pct}%` }} />
                                    </div>
                                )}
                            </div>
                        );
                    })}
                </div>
            </CardContent>
        </Card>
    );
}
