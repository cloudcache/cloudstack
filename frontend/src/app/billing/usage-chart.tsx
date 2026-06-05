'use client'

import { useState, useEffect } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { AreaChart, Area, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer } from "recharts";
import { getUsageHistory } from "./actions";
import { Loader2 } from "lucide-react";
import { useT } from "@/i18n";

interface UsageSnapshot {
    time: string;
    apps: number;
    databases: number;
    cpu_mcores: number;
    mem_mb: number;
    storage_gb: number;
    cost: number;
}

function formatTime(t: string) {
    const d = new Date(t);
    return `${(d.getMonth() + 1).toString().padStart(2, '0')}/${d.getDate().toString().padStart(2, '0')} ${d.getHours().toString().padStart(2, '0')}:00`;
}

function getFromDate(range: string): string {
    const now = new Date();
    switch (range) {
        case '24h': now.setHours(now.getHours() - 24); break;
        case '7d':  now.setDate(now.getDate() - 7); break;
        case '30d': now.setDate(now.getDate() - 30); break;
    }
    return now.toISOString().split('T')[0];
}

export default function UsageChart({ currency }: { currency: string }) {
    const t = useT();
    const [range, setRange] = useState('7d');
    const [data, setData] = useState<UsageSnapshot[]>([]);
    const [loading, setLoading] = useState(true);

    useEffect(() => {
        setLoading(true);
        const from = getFromDate(range);
        getUsageHistory(from).then(res => {
            const items = (res?.data ?? []) as UsageSnapshot[];
            // Sort ascending by time
            items.sort((a, b) => new Date(a.time).getTime() - new Date(b.time).getTime());
            setData(items);
        }).catch(() => setData([]))
          .finally(() => setLoading(false));
    }, [range]);

    const cur = (currency.toUpperCase() === 'USD' ? 'USD' : 'CNY') as 'CNY' | 'USD';
    const locale = cur === 'CNY' ? 'zh-CN' : 'en-US';
    const formatCost = (v: number) =>
        new Intl.NumberFormat(locale, { style: 'currency', currency: cur, maximumFractionDigits: 4 }).format(v);

    return (
        <Card>
            <CardHeader className="flex flex-row items-center justify-between pb-2">
                <CardTitle className="text-base">{t("page.billing.usageHistory")}</CardTitle>
                <Tabs value={range} onValueChange={setRange}>
                    <TabsList className="h-8">
                        <TabsTrigger value="24h" className="text-xs px-2">24h</TabsTrigger>
                        <TabsTrigger value="7d" className="text-xs px-2">7d</TabsTrigger>
                        <TabsTrigger value="30d" className="text-xs px-2">30d</TabsTrigger>
                    </TabsList>
                </Tabs>
            </CardHeader>
            <CardContent>
                {loading ? (
                    <div className="flex items-center justify-center h-[250px]">
                        <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
                    </div>
                ) : data.length === 0 ? (
                    <div className="flex items-center justify-center h-[250px] text-sm text-muted-foreground">
                        {t('page.billing.noUsageData')}
                    </div>
                ) : (
                    <div className="space-y-6">
                        {/* Cost chart */}
                        <div>
                            <p className="text-sm text-muted-foreground mb-2">{t('page.billing.hourlyCost')}</p>
                            <ResponsiveContainer width="100%" height={200}>
                                <AreaChart data={data}>
                                    <CartesianGrid strokeDasharray="3 3" className="stroke-muted" />
                                    <XAxis dataKey="time" tickFormatter={(v: string) => formatTime(v)} tick={{ fontSize: 11 }} />
                                    <YAxis tickFormatter={(v: number) => formatCost(v)} tick={{ fontSize: 11 }} width={80} />
                                    <Tooltip
                                        labelFormatter={(v: any) => formatTime(String(v))}
                                        formatter={(v: any) => [formatCost(Number(v)), t('page.billing.cost')]}
                                    />
                                    <Area type="monotone" dataKey="cost" stroke="hsl(var(--primary))" fill="hsl(var(--primary))" fillOpacity={0.15} />
                                </AreaChart>
                            </ResponsiveContainer>
                        </div>

                        {/* Resource chart */}
                        <div>
                            <p className="text-sm text-muted-foreground mb-2">CPU (mCores) &amp; Memory (MB)</p>
                            <ResponsiveContainer width="100%" height={200}>
                                <AreaChart data={data}>
                                    <CartesianGrid strokeDasharray="3 3" className="stroke-muted" />
                                    <XAxis dataKey="time" tickFormatter={(v: string) => formatTime(v)} tick={{ fontSize: 11 }} />
                                    <YAxis tick={{ fontSize: 11 }} width={60} />
                                    <Tooltip
                                        labelFormatter={(v: any) => formatTime(String(v))}
                                        formatter={(v: any, name: any) => [Number(v), name === 'cpu_mcores' ? 'CPU (m)' : 'Memory (MB)']}
                                    />
                                    <Area type="monotone" dataKey="cpu_mcores" stroke="#3b82f6" fill="#3b82f6" fillOpacity={0.1} name="cpu_mcores" />
                                    <Area type="monotone" dataKey="mem_mb" stroke="#10b981" fill="#10b981" fillOpacity={0.1} name="mem_mb" />
                                </AreaChart>
                            </ResponsiveContainer>
                        </div>
                    </div>
                )}
            </CardContent>
        </Card>
    );
}
