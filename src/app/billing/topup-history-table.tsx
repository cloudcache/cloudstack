'use client'

import { useEffect, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { getTopupHistory } from "./actions";
import { Loader2 } from "lucide-react";

interface TopupRecord {
    id: string;
    session_id: string;
    amount: number;
    currency: string;
    status: string;
    created_at: string;
    completed_at?: string;
}

function statusBadge(status: string) {
    switch (status) {
        case 'COMPLETED': return <Badge variant="default" className="bg-green-600">Completed</Badge>;
        case 'PENDING':   return <Badge variant="secondary">Pending</Badge>;
        case 'EXPIRED':   return <Badge variant="outline">Expired</Badge>;
        case 'FAILED':    return <Badge variant="destructive">Failed</Badge>;
        default:          return <Badge variant="outline">{status}</Badge>;
    }
}

function formatAmount(amount: number, currency: string) {
    return new Intl.NumberFormat('en-US', { style: 'currency', currency: currency || 'CNY' }).format(amount / 100);
}

export default function TopupHistoryTable() {
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
        return <p className="text-sm text-muted-foreground py-4">No top-up records.</p>;
    }

    return (
        <Table>
            <TableHeader>
                <TableRow>
                    <TableHead>Date</TableHead>
                    <TableHead>Amount</TableHead>
                    <TableHead>Status</TableHead>
                    <TableHead>Completed</TableHead>
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
                        <TableCell>{statusBadge(r.status)}</TableCell>
                        <TableCell className="text-sm text-muted-foreground">
                            {r.completed_at ? new Date(r.completed_at).toLocaleString() : '—'}
                        </TableCell>
                    </TableRow>
                ))}
            </TableBody>
        </Table>
    );
}
