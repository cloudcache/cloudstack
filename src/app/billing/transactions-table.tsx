'use client'

import { Badge } from "@/components/ui/badge";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Transaction } from "@/server/adapter/backend-api.adapter";
import PaginationControls from "./pagination-controls";

const PER_PAGE = 20;

function formatAmount(amount: number, currency: string) {
    return new Intl.NumberFormat('en-US', { style: 'currency', currency: currency || 'CNY', signDisplay: 'always' }).format(amount);
}

function txTypeBadge(txType: string) {
    switch (txType) {
        case 'CREDIT':     return <Badge variant="default" className="bg-green-600">Credit</Badge>;
        case 'DEBIT':      return <Badge variant="destructive">Debit</Badge>;
        case 'ADJUSTMENT': return <Badge variant="secondary">Adjustment</Badge>;
        case 'REFUND':     return <Badge variant="outline">Refund</Badge>;
        default:           return <Badge variant="outline">{txType}</Badge>;
    }
}

export { PER_PAGE as TX_PER_PAGE };

export default function TransactionsTable({
    transactions, total, page, currency = 'CNY',
}: {
    transactions: Transaction[];
    total: number;
    page: number;
    currency?: string;
}) {
    if (transactions.length === 0) {
        return <p className="text-sm text-muted-foreground py-4">No transactions yet.</p>;
    }
    return (
        <div className="space-y-2">
            <Table>
                <TableHeader>
                    <TableRow>
                        <TableHead>Date</TableHead>
                        <TableHead>Type</TableHead>
                        <TableHead>Description</TableHead>
                        <TableHead className="text-right">Amount</TableHead>
                        <TableHead className="text-right">Balance After</TableHead>
                    </TableRow>
                </TableHeader>
                <TableBody>
                    {transactions.map(tx => (
                        <TableRow key={tx.id}>
                            <TableCell className="text-sm text-muted-foreground whitespace-nowrap">
                                {new Date(tx.created_at).toLocaleString()}
                            </TableCell>
                            <TableCell>{txTypeBadge(tx.tx_type)}</TableCell>
                            <TableCell className="text-sm">{tx.description ?? '—'}</TableCell>
                            <TableCell className={`text-right font-mono text-sm font-semibold ${tx.amount >= 0 ? 'text-green-600' : 'text-red-500'}`}>
                                {formatAmount(tx.amount, currency)}
                            </TableCell>
                            <TableCell className="text-right font-mono text-sm">
                                {new Intl.NumberFormat('en-US', { style: 'currency', currency: currency || 'CNY' }).format(tx.balance_after)}
                            </TableCell>
                        </TableRow>
                    ))}
                </TableBody>
            </Table>
            <PaginationControls page={page} total={total} perPage={PER_PAGE} param="tx_page" />
        </div>
    );
}
