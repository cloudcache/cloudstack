'use client'

import { useMemo, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Textarea } from "@/components/ui/textarea";
import { Toast } from "@/frontend/utils/toast.utils";
import { toast } from "sonner";
import { useT } from "@/i18n";
import {
    adminAdjustBalance,
    adminGenerateInvoice,
    adminMarkInvoicePaid,
    adminRecharge,
    adminVoidInvoice,
} from "./actions";
import { Ban, Check, Minus, Plus, Search } from "lucide-react";

type MoneyValue = number | string | null | undefined;

interface WalletItem {
    user_id: string;
    username: string;
    email?: string | null;
    balance: MoneyValue;
    currency?: string | null;
    subscription_plan?: string;
    subscription_status?: string;
}

interface InvoiceItem {
    id: string;
    invoice_no: string;
    username?: string;
    user_id?: string;
    period_start?: string;
    period_end?: string;
    total_amount: MoneyValue;
    status: string;
    created_at?: string;
}

interface TransactionItem {
    id: string;
    user_id?: string;
    username?: string;
    type?: string;
    tx_type?: string;
    amount: MoneyValue;
    balance_after: MoneyValue;
    description?: string | null;
    ref_id?: string | null;
    operator_username?: string | null;
    created_at?: string;
}

interface AdminBillingTabProps {
    initialWallets: WalletItem[];
    initialInvoices?: InvoiceItem[];
    initialTransactions?: TransactionItem[];
}

function toNumber(value: MoneyValue): number {
    if (typeof value === "number") return Number.isFinite(value) ? value : 0;
    if (typeof value === "string") {
        const parsed = Number(value);
        return Number.isFinite(parsed) ? parsed : 0;
    }
    return 0;
}

function formatMoney(value: MoneyValue, currency = "CNY") {
    const symbol = currency === "CNY" ? "¥" : `${currency} `;
    return `${symbol}${toNumber(value).toFixed(2)}`;
}

function formatDate(value?: string) {
    if (!value) return "-";
    const date = new Date(value);
    if (Number.isNaN(date.getTime())) return value.slice(0, 10);
    return date.toLocaleDateString();
}

function normalizeWallet(wallet: WalletItem): WalletItem {
    return {
        ...wallet,
        balance: toNumber(wallet.balance),
        currency: wallet.currency || "CNY",
    };
}

export default function AdminBillingTab({
    initialWallets,
    initialInvoices = [],
    initialTransactions = [],
}: AdminBillingTabProps) {
    const t = useT();
    const [wallets, setWallets] = useState<WalletItem[]>(() => initialWallets.map(normalizeWallet));
    const [invoices, setInvoices] = useState<InvoiceItem[]>(initialInvoices);
    const [transactions] = useState<TransactionItem[]>(initialTransactions);
    const [search, setSearch] = useState('');
    const [adjustTarget, setAdjustTarget] = useState<WalletItem | null>(null);
    const [adjustMode, setAdjustMode] = useState<'recharge' | 'adjust'>('recharge');
    const [amount, setAmount] = useState('');
    const [description, setDescription] = useState('');
    // Idempotency key for the current balance dialog: generated once when the
    // dialog opens and reused across retries/double-clicks so a single intended
    // operation can never produce two ledger entries.
    const [idempKey, setIdempKey] = useState('');
    const [invoiceOpen, setInvoiceOpen] = useState(false);
    const [invoiceUserId, setInvoiceUserId] = useState('');
    const [periodStart, setPeriodStart] = useState('');
    const [periodEnd, setPeriodEnd] = useState('');
    const [saving, setSaving] = useState(false);

    const filteredWallets = useMemo(() => {
        const term = search.toLowerCase();
        return wallets.filter(w =>
            w.username.toLowerCase().includes(term) ||
            (w.email ?? '').toLowerCase().includes(term)
        );
    }, [search, wallets]);

    const walletById = useMemo(() => new Map(wallets.map(w => [w.user_id, w])), [wallets]);

    const openRecharge = (wallet: WalletItem) => {
        setAdjustTarget(wallet);
        setAdjustMode('recharge');
        setAmount('');
        setDescription('');
        setIdempKey(crypto.randomUUID());
    };

    const openAdjust = (wallet: WalletItem) => {
        setAdjustTarget(wallet);
        setAdjustMode('adjust');
        setAmount('');
        setDescription('');
        setIdempKey(crypto.randomUUID());
    };

    const handleBalanceSubmit = async () => {
        if (!adjustTarget) return;
        const numAmount = Number(amount);
        if (!Number.isFinite(numAmount) || numAmount === 0) return;
        const reason = description.trim();
        // A recharge/gift must carry a reason so the ledger entry is auditable.
        if (adjustMode === 'recharge' && !reason) {
            toast.error('A reason is required for a recharge/gift.');
            return;
        }
        setSaving(true);
        try {
            const result = adjustMode === 'recharge'
                ? await Toast.fromAction(() => adminRecharge(adjustTarget.user_id, numAmount, reason, idempKey))
                : await Toast.fromAction(() => adminAdjustBalance(adjustTarget.user_id, numAmount, reason || t("business.billing.defaultAdjustment"), idempKey));
            const rawNewBalance = (result.data as { new_balance?: MoneyValue } | undefined)?.new_balance;
            const newBalance = rawNewBalance === undefined ? undefined : toNumber(rawNewBalance);
            setWallets(prev => prev.map(w =>
                w.user_id === adjustTarget.user_id
                    ? { ...w, balance: newBalance ?? toNumber(w.balance) + numAmount }
                    : w
            ));
            setAdjustTarget(null);
        } finally {
            setSaving(false);
        }
    };

    const handleCreateInvoice = async () => {
        if (!invoiceUserId || !periodStart || !periodEnd) return;
        setSaving(true);
        try {
            const result = await Toast.fromAction(() => adminGenerateInvoice(invoiceUserId, periodStart, periodEnd));
            const created = result.data as { id: string; invoice_no: string; total_amount: MoneyValue } | undefined;
            if (created) {
                const wallet = walletById.get(invoiceUserId);
                setInvoices(prev => [{
                    id: created.id,
                    invoice_no: created.invoice_no,
                    username: wallet?.username,
                    user_id: invoiceUserId,
                    period_start: periodStart,
                    period_end: periodEnd,
                    total_amount: created.total_amount,
                    status: "ISSUED",
                    created_at: new Date().toISOString(),
                }, ...prev]);
            }
            setInvoiceOpen(false);
            setInvoiceUserId('');
            setPeriodStart('');
            setPeriodEnd('');
        } finally {
            setSaving(false);
        }
    };

    const markPaid = async (invoice: InvoiceItem) => {
        await Toast.fromAction(() => adminMarkInvoicePaid(invoice.id));
        setInvoices(prev => prev.map(item => item.id === invoice.id ? { ...item, status: "PAID" } : item));
    };

    const voidInvoice = async (invoice: InvoiceItem) => {
        await Toast.fromAction(() => adminVoidInvoice(invoice.id));
        setInvoices(prev => prev.map(item => item.id === invoice.id ? { ...item, status: "VOID" } : item));
    };

    return (
        <div className="space-y-6">
            <Card>
                <CardHeader className="flex flex-row items-start justify-between gap-4">
                    <div>
                        <CardTitle>{t("business.billing.wallets")}</CardTitle>
                        <CardDescription>{t("business.billing.walletsDescription")}</CardDescription>
                    </div>
                    <div className="relative w-64">
                        <Search className="absolute left-2 top-2.5 h-4 w-4 text-muted-foreground" />
                        <Input
                            placeholder={t("business.billing.searchUsers")}
                            value={search}
                            onChange={e => setSearch(e.target.value)}
                            className="pl-8"
                        />
                    </div>
                </CardHeader>
                <CardContent>
                    {filteredWallets.length === 0 ? (
                        <p className="text-muted-foreground text-sm">{t("business.billing.noWallets")}</p>
                    ) : (
                        <Table>
                            <TableHeader>
                                <TableRow>
                                    <TableHead>{t("common.user")}</TableHead>
                                    <TableHead>{t("common.email")}</TableHead>
                                    <TableHead>{t("business.billing.balance")}</TableHead>
                                    <TableHead>{t("business.billing.plan")}</TableHead>
                                    <TableHead className="w-[190px]">{t("common.actions")}</TableHead>
                                </TableRow>
                            </TableHeader>
                            <TableBody>
                                {filteredWallets.map(w => (
                                    <TableRow key={w.user_id}>
                                        <TableCell className="font-medium">{w.username}</TableCell>
                                        <TableCell className="text-muted-foreground">{w.email ?? '-'}</TableCell>
                                        <TableCell>
                                            <span className={toNumber(w.balance) < 0 ? 'text-destructive font-semibold' : ''}>
                                                {formatMoney(w.balance, w.currency || "CNY")}
                                            </span>
                                        </TableCell>
                                        <TableCell>
                                            {w.subscription_plan ? (
                                                <Badge variant={w.subscription_status === 'ACTIVE' ? 'default' : 'secondary'}>
                                                    {w.subscription_plan}
                                                </Badge>
                                            ) : (
                                                <span className="text-muted-foreground text-sm">{t("business.billing.noPlan")}</span>
                                            )}
                                        </TableCell>
                                        <TableCell>
                                            <div className="flex gap-1">
                                                <Button size="sm" variant="outline" onClick={() => openRecharge(w)}>
                                                    <Plus className="mr-1 h-3 w-3" /> {t("business.billing.recharge")}
                                                </Button>
                                                <Button size="sm" variant="ghost" onClick={() => openAdjust(w)}>
                                                    <Minus className="mr-1 h-3 w-3" /> {t("business.billing.adjust")}
                                                </Button>
                                            </div>
                                        </TableCell>
                                    </TableRow>
                                ))}
                            </TableBody>
                        </Table>
                    )}
                </CardContent>
            </Card>

            <Card>
                <CardHeader className="flex flex-row items-start justify-between gap-4">
                    <div>
                        <CardTitle>{t("business.billing.orders")}</CardTitle>
                        <CardDescription>{t("business.billing.ordersDescription")}</CardDescription>
                    </div>
                    <Button onClick={() => setInvoiceOpen(true)}>
                        <Plus className="mr-2 h-4 w-4" /> {t("business.billing.createOrder")}
                    </Button>
                </CardHeader>
                <CardContent>
                    {invoices.length === 0 ? (
                        <p className="text-muted-foreground text-sm">{t("business.billing.noOrders")}</p>
                    ) : (
                        <Table>
                            <TableHeader>
                                <TableRow>
                                    <TableHead>{t("business.billing.orderNo")}</TableHead>
                                    <TableHead>{t("common.user")}</TableHead>
                                    <TableHead>{t("page.billing.period")}</TableHead>
                                    <TableHead>{t("page.billing.total")}</TableHead>
                                    <TableHead>{t("common.status")}</TableHead>
                                    <TableHead className="w-[170px]">{t("common.actions")}</TableHead>
                                </TableRow>
                            </TableHeader>
                            <TableBody>
                                {invoices.map(invoice => (
                                    <TableRow key={invoice.id}>
                                        <TableCell className="font-medium">{invoice.invoice_no}</TableCell>
                                        <TableCell>{invoice.username ?? walletById.get(invoice.user_id ?? '')?.username ?? '-'}</TableCell>
                                        <TableCell className="text-muted-foreground">
                                            {formatDate(invoice.period_start)} - {formatDate(invoice.period_end)}
                                        </TableCell>
                                        <TableCell>{formatMoney(invoice.total_amount)}</TableCell>
                                        <TableCell>
                                            <Badge variant={invoice.status === "PAID" ? "default" : invoice.status === "VOID" ? "secondary" : "outline"}>
                                                {invoice.status}
                                            </Badge>
                                        </TableCell>
                                        <TableCell>
                                            <div className="flex gap-1">
                                                <Button
                                                    size="sm"
                                                    variant="outline"
                                                    disabled={invoice.status === "PAID" || invoice.status === "VOID"}
                                                    onClick={() => markPaid(invoice)}>
                                                    <Check className="mr-1 h-3 w-3" /> {t("business.billing.markPaid")}
                                                </Button>
                                                <Button
                                                    size="sm"
                                                    variant="ghost"
                                                    disabled={invoice.status === "PAID" || invoice.status === "VOID"}
                                                    onClick={() => voidInvoice(invoice)}>
                                                    <Ban className="mr-1 h-3 w-3" /> {t("business.billing.void")}
                                                </Button>
                                            </div>
                                        </TableCell>
                                    </TableRow>
                                ))}
                            </TableBody>
                        </Table>
                    )}
                </CardContent>
            </Card>

            <Card>
                <CardHeader>
                    <CardTitle>{t("business.billing.ledger")}</CardTitle>
                    <CardDescription>{t("business.billing.ledgerDescription")}</CardDescription>
                </CardHeader>
                <CardContent>
                    {transactions.length === 0 ? (
                        <p className="text-muted-foreground text-sm">{t("business.billing.noLedger")}</p>
                    ) : (
                        <Table>
                            <TableHeader>
                                <TableRow>
                                    <TableHead>{t("business.billing.time")}</TableHead>
                                    <TableHead>{t("common.user")}</TableHead>
                                    <TableHead>{t("common.type")}</TableHead>
                                    <TableHead>{t("common.amount")}</TableHead>
                                    <TableHead>{t("page.billing.balanceAfter")}</TableHead>
                                    <TableHead>{t("common.description")}</TableHead>
                                    <TableHead>{t("business.billing.operator")}</TableHead>
                                </TableRow>
                            </TableHeader>
                            <TableBody>
                                {transactions.map(tx => {
                                    const txType = tx.type ?? tx.tx_type ?? '-';
                                    const amount = toNumber(tx.amount);
                                    return (
                                        <TableRow key={tx.id}>
                                            <TableCell className="text-muted-foreground">{formatDate(tx.created_at)}</TableCell>
                                            <TableCell>{tx.username ?? walletById.get(tx.user_id ?? '')?.username ?? '-'}</TableCell>
                                            <TableCell><Badge variant="outline">{txType}</Badge></TableCell>
                                            <TableCell className={amount < 0 ? "text-destructive" : "text-emerald-600"}>
                                                {formatMoney(tx.amount)}
                                            </TableCell>
                                            <TableCell>{formatMoney(tx.balance_after)}</TableCell>
                                            <TableCell className="max-w-[260px] truncate">{tx.description ?? tx.ref_id ?? '-'}</TableCell>
                                            <TableCell>{tx.operator_username ?? '-'}</TableCell>
                                        </TableRow>
                                    );
                                })}
                            </TableBody>
                        </Table>
                    )}
                </CardContent>
            </Card>

            <Dialog open={!!adjustTarget} onOpenChange={open => { if (!open) setAdjustTarget(null); }}>
                <DialogContent>
                    <DialogHeader>
                        <DialogTitle>
                            {adjustMode === 'recharge' ? t("business.billing.rechargeWallet") : t("business.billing.adjustBalance")}
                        </DialogTitle>
                    </DialogHeader>
                    <div className="space-y-4">
                        <p className="text-sm text-muted-foreground">
                            {t("common.user")}: <strong>{adjustTarget?.username}</strong> · {t("business.billing.currentBalance")}: {formatMoney(adjustTarget?.balance)}
                        </p>
                        <div className="space-y-2">
                            <Label>{t("business.billing.amountCny")}{adjustMode === 'adjust' && ` - ${t("business.billing.negativeDeductHint")}`}</Label>
                            <Input type="number" value={amount} onChange={e => setAmount(e.target.value)}
                                placeholder={adjustMode === 'recharge' ? '100' : '-50 or 100'} />
                        </div>
                        <div className="space-y-2">
                            <Label>{t("business.billing.descriptionOptional")}</Label>
                            <Textarea value={description} onChange={e => setDescription(e.target.value)}
                                placeholder={adjustMode === 'recharge' ? t("business.billing.manualRecharge") : t("business.billing.adjustReason")} rows={2} />
                        </div>
                    </div>
                    <DialogFooter>
                        <Button variant="outline" onClick={() => setAdjustTarget(null)}>{t("common.cancel")}</Button>
                        <Button onClick={handleBalanceSubmit} disabled={saving || !amount || Number(amount) === 0}>
                            {saving ? t("common.processing") : adjustMode === 'recharge' ? t("business.billing.recharge") : t("business.billing.adjust")}
                        </Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>

            <Dialog open={invoiceOpen} onOpenChange={setInvoiceOpen}>
                <DialogContent>
                    <DialogHeader>
                        <DialogTitle>{t("business.billing.createOrder")}</DialogTitle>
                    </DialogHeader>
                    <div className="space-y-4">
                        <div className="space-y-2">
                            <Label>{t("common.user")}</Label>
                            <select
                                className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
                                value={invoiceUserId}
                                onChange={e => setInvoiceUserId(e.target.value)}>
                                <option value="">{t("business.billing.selectUser")}</option>
                                {wallets.map(wallet => (
                                    <option key={wallet.user_id} value={wallet.user_id}>
                                        {wallet.username}{wallet.email ? ` (${wallet.email})` : ''}
                                    </option>
                                ))}
                            </select>
                        </div>
                        <div className="grid grid-cols-2 gap-3">
                            <div className="space-y-2">
                                <Label>{t("business.billing.periodStart")}</Label>
                                <Input type="date" value={periodStart} onChange={e => setPeriodStart(e.target.value)} />
                            </div>
                            <div className="space-y-2">
                                <Label>{t("business.billing.periodEnd")}</Label>
                                <Input type="date" value={periodEnd} onChange={e => setPeriodEnd(e.target.value)} />
                            </div>
                        </div>
                    </div>
                    <DialogFooter>
                        <Button variant="outline" onClick={() => setInvoiceOpen(false)}>{t("common.cancel")}</Button>
                        <Button onClick={handleCreateInvoice} disabled={saving || !invoiceUserId || !periodStart || !periodEnd}>
                            {saving ? t("common.creating") : t("common.create")}
                        </Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>
        </div>
    );
}
