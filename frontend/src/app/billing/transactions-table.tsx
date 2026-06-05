'use client'

import { Badge } from "@/components/ui/badge";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Transaction } from "@/server/adapter/backend-api.adapter";
import PaginationControls from "./pagination-controls";
import { useT } from "@/i18n";

const PER_PAGE = 20;

function normalizeCurrency(c: string): 'CNY' | 'USD' {
    return c.toUpperCase() === 'USD' ? 'USD' : 'CNY';
}

function formatAmount(amount: number, currency: string) {
    const cur = normalizeCurrency(currency);
    const locale = cur === 'CNY' ? 'zh-CN' : 'en-US';
    return new Intl.NumberFormat(locale, { style: 'currency', currency: cur, signDisplay: 'always' }).format(amount);
}

function txTypeBadge(txType: string, t: ReturnType<typeof useT>) {
    switch (txType) {
        case 'RECHARGE':   return <Badge variant="default" className="bg-green-600">{t('page.billing.txRecharge')}</Badge>;
        case 'DEDUCTION':  return <Badge variant="destructive">{t('page.billing.txDeduction')}</Badge>;
        case 'ADJUSTMENT': return <Badge variant="secondary">{t('page.billing.txAdjustment')}</Badge>;
        case 'REFUND':     return <Badge variant="outline">{t('page.billing.txRefund')}</Badge>;
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
    const t = useT();
    if (transactions.length === 0) {
        return <p className="text-sm text-muted-foreground py-4">{t('page.billing.noTransactions')}</p>;
    }
    return (
        <div className="space-y-2">
            <Table>
                <TableHeader>
                    <TableRow>
                        <TableHead>{t("common.date")}</TableHead>
                        <TableHead>{t("common.type")}</TableHead>
                        <TableHead>{t("common.description")}</TableHead>
                        <TableHead className="text-right">{t("common.amount")}</TableHead>
                        <TableHead className="text-right">{t("page.billing.balanceAfter")}</TableHead>
                    </TableRow>
                </TableHeader>
                <TableBody>
                    {transactions.map(tx => (
                        <TableRow key={tx.id}>
                            <TableCell className="text-sm text-muted-foreground whitespace-nowrap">
                                {new Date(tx.created_at).toLocaleString()}
                            </TableCell>
                            <TableCell>{txTypeBadge(tx.tx_type, t)}</TableCell>
                            <TableCell className="text-sm">{tx.description ?? '—'}</TableCell>
                            <TableCell className={`text-right font-mono text-sm font-semibold ${tx.amount >= 0 ? 'text-green-600' : 'text-red-500'}`}>
                                {formatAmount(tx.amount, currency)}
                            </TableCell>
                            <TableCell className="text-right font-mono text-sm">
                                {new Intl.NumberFormat(normalizeCurrency(currency) === 'CNY' ? 'zh-CN' : 'en-US', { style: 'currency', currency: normalizeCurrency(currency) }).format(tx.balance_after)}
                            </TableCell>
                        </TableRow>
                    ))}
                </TableBody>
            </Table>
            <PaginationControls page={page} total={total} perPage={PER_PAGE} param="tx_page" />
        </div>
    );
}
