'use client'

import { Badge } from "@/components/ui/badge";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Invoice } from "@/server/adapter/backend-api.adapter";
import PaginationControls from "./pagination-controls";
import { useT } from "@/i18n";

const PER_PAGE = 20;

function statusBadge(status: string, t: ReturnType<typeof useT>) {
    switch (status) {
        case 'PAID':    return <Badge variant="default" className="bg-green-600">{t('page.billing.statusPaid')}</Badge>;
        case 'DRAFT':   return <Badge variant="secondary">{t('page.billing.statusDraft')}</Badge>;
        case 'ISSUED':  return <Badge variant="outline" className="border-yellow-500 text-yellow-600">{t('page.billing.statusIssued')}</Badge>;
        case 'VOID':    return <Badge variant="destructive">{t('page.billing.statusVoid')}</Badge>;
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
    const t = useT();
    if (invoices.length === 0) {
        return <p className="text-sm text-muted-foreground py-4">{t('page.billing.noInvoices')}</p>;
    }
    return (
        <div className="space-y-2">
            <Table>
                <TableHeader>
                    <TableRow>
                        <TableHead>{t("page.billing.invoiceNumber")}</TableHead>
                        <TableHead>{t("page.billing.period")}</TableHead>
                        <TableHead>{t("page.billing.issued")}</TableHead>
                        <TableHead>{t("page.billing.paid")}</TableHead>
                        <TableHead>{t("common.status")}</TableHead>
                        <TableHead className="text-right">{t("page.billing.total")}</TableHead>
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
                            <TableCell>{statusBadge(inv.status, t)}</TableCell>
                            <TableCell className="text-right font-mono text-sm font-semibold">
                                {new Intl.NumberFormat(currency.toUpperCase() === 'USD' ? 'en-US' : 'zh-CN', { style: 'currency', currency: currency.toUpperCase() === 'USD' ? 'USD' : 'CNY' }).format(inv.total_amount)}
                            </TableCell>
                        </TableRow>
                    ))}
                </TableBody>
            </Table>
            <PaginationControls page={page} total={total} perPage={PER_PAGE} param="inv_page" />
        </div>
    );
}
