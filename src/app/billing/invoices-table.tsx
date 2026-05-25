'use client'

import { Badge } from "@/components/ui/badge";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Invoice } from "@/server/adapter/backend-api.adapter";
import PaginationControls from "./pagination-controls";

const PER_PAGE = 20;

function statusBadge(status: string) {
    switch (status) {
        case 'PAID':    return <Badge variant="default" className="bg-green-600">Paid</Badge>;
        case 'PENDING': return <Badge variant="secondary">Pending</Badge>;
        case 'OVERDUE': return <Badge variant="destructive">Overdue</Badge>;
        default:        return <Badge variant="outline">{status}</Badge>;
    }
}

function formatDate(d?: string) {
    if (!d) return '—';
    return new Date(d).toLocaleDateString();
}

export { PER_PAGE as INV_PER_PAGE };

export default function InvoicesTable({
    invoices, total, page, currency = 'CNY',
}: {
    invoices: Invoice[];
    total: number;
    page: number;
    currency?: string;
}) {
    if (invoices.length === 0) {
        return <p className="text-sm text-muted-foreground py-4">No invoices yet.</p>;
    }
    return (
        <div className="space-y-2">
            <Table>
                <TableHeader>
                    <TableRow>
                        <TableHead>Invoice #</TableHead>
                        <TableHead>Period</TableHead>
                        <TableHead>Issued</TableHead>
                        <TableHead>Paid</TableHead>
                        <TableHead>Status</TableHead>
                        <TableHead className="text-right">Total</TableHead>
                    </TableRow>
                </TableHeader>
                <TableBody>
                    {invoices.map(inv => (
                        <TableRow key={inv.id}>
                            <TableCell className="font-mono text-sm">{inv.invoice_no}</TableCell>
                            <TableCell className="text-sm text-muted-foreground">
                                {formatDate(inv.period_start)} – {formatDate(inv.period_end)}
                            </TableCell>
                            <TableCell className="text-sm text-muted-foreground">{formatDate(inv.issued_at)}</TableCell>
                            <TableCell className="text-sm text-muted-foreground">{formatDate(inv.paid_at)}</TableCell>
                            <TableCell>{statusBadge(inv.status)}</TableCell>
                            <TableCell className="text-right font-mono text-sm font-semibold">
                                {new Intl.NumberFormat('en-US', { style: 'currency', currency: currency || 'CNY' }).format(inv.total_amount)}
                            </TableCell>
                        </TableRow>
                    ))}
                </TableBody>
            </Table>
            <PaginationControls page={page} total={total} perPage={PER_PAGE} param="inv_page" />
        </div>
    );
}
