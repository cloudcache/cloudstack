'use client'

import { useEffect, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { getTopupHistory } from "./actions";
import { Loader2 } from "lucide-react";
import { useT } from "@/i18n";

interface TopupRecord {
    id: string;
    session_id: string;
    amount: number;
    currency: string;
    status: string;
    created_at: string;
    completed_at?: string;
}

function statusBadge(status: string, t: ReturnType<typeof useT>) {
    switch (status) {
        case 'COMPLETED': return <Badge variant="default" className="bg-green-600">{t('page.billing.statusCompleted')}</Badge>;
        case 'PENDING':   return <Badge variant="secondary">{t('page.billing.statusPending')}</Badge>;
        case 'EXPIRED':   return <Badge variant="outline">{t('page.billing.statusExpired')}</Badge>;
        case 'FAILED':    return <Badge variant="destructive">{t('page.billing.statusFailed')}</Badge>;
        default:          return <Badge variant="outline">{status}</Badge>;
    }
}

type SupportedCurrency = 'CNY' | 'USD';

function normalizeCurrency(c: string): SupportedCurrency {
    const upper = c.toUpperCase();
    return upper === 'USD' ? 'USD' : 'CNY';
}

function formatAmount(amount: number, currency: string) {
    const cur = normalizeCurrency(currency);
    const locale = cur === 'CNY' ? 'zh-CN' : 'en-US';
    return new Intl.NumberFormat(locale, { style: 'currency', currency: cur }).format(amount / 100);
}

export default function TopupHistoryTable() {
    const t = useT();
    const [records, setRecords] = useState<TopupRecord[]>([]);
    const [loading, setLoading] = useState(true);

    useEffect(() => {
        getTopupHistory()
            .then(setRecords)
            .catch(() => setRecords([]))
            .finally(() => setLoading(false));
    }, []);

    if (loading) {
        return (
            <div className="flex items-center justify-center py-8">
                <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
            </div>
        );
    }

    if (records.length === 0) {
        return <p className="text-sm text-muted-foreground py-4">{t('page.billing.noTopupRecords')}</p>;
    }

    return (
        <Table>
            <TableHeader>
                <TableRow>
                    <TableHead>{t('common.date')}</TableHead>
                    <TableHead>{t('common.amount')}</TableHead>
                    <TableHead>{t('common.status')}</TableHead>
                    <TableHead>{t('page.billing.completed')}</TableHead>
                </TableRow>
            </TableHeader>
            <TableBody>
                {records.map(r => (
                    <TableRow key={r.id}>
                        <TableCell className="text-sm text-muted-foreground whitespace-nowrap">
                            {new Date(r.created_at).toLocaleString()}
                        </TableCell>
                        <TableCell className="font-mono text-sm font-semibold text-green-600">
                            +{formatAmount(r.amount, r.currency)}
                        </TableCell>
                        <TableCell>{statusBadge(r.status, t)}</TableCell>
                        <TableCell className="text-sm text-muted-foreground">
                            {r.completed_at ? new Date(r.completed_at).toLocaleString() : '—'}
                        </TableCell>
                    </TableRow>
                ))}
            </TableBody>
        </Table>
    );
}
