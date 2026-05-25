'use server'

import { Suspense } from "react";
import { getAuthUserSession, getBackendToken } from "@/server/utils/action-wrapper.utils";
import PageTitle from "@/components/custom/page-title";
import BreadcrumbSetter from "@/components/breadcrumbs-setter";
import { backend, Transaction, Invoice } from "@/server/adapter/backend-api.adapter";
import WalletCard from "./wallet-card";
import TransactionsTable, { TX_PER_PAGE } from "./transactions-table";
import InvoicesTable, { INV_PER_PAGE } from "./invoices-table";
import UsageChart from "./usage-chart";
import TopupHistoryTable from "./topup-history-table";
import PaymentStatusToast from "./payment-status-toast";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { AlertCircle } from "lucide-react";

interface Props {
    searchParams?: Promise<{ tx_page?: string; inv_page?: string; payment?: string }>;
}

export default async function BillingPage({ searchParams }: Props) {
    await getAuthUserSession();
    const token = await getBackendToken();

    const sp = await searchParams ?? {};
    const txPage  = Math.max(1, parseInt(sp.tx_page  ?? '1', 10) || 1);
    const invPage = Math.max(1, parseInt(sp.inv_page ?? '1', 10) || 1);

    const [walletResult, usageResult, txResult, invoicesResult, overdueResult, topupConfigResult] = await Promise.allSettled([
        backend.billing.wallet(token),
        backend.billing.currentUsage(token),
        backend.billing.listTransactions(token, { page: txPage,  per_page: TX_PER_PAGE }),
        backend.billing.listInvoices(token,     { page: invPage, per_page: INV_PER_PAGE }),
        backend.billing.overdueStatus(token),
        backend.billing.topupConfig(token),
    ]);

    const wallet  = walletResult.status       === 'fulfilled' ? walletResult.value       : null;
    const usage   = usageResult.status        === 'fulfilled' ? usageResult.value        : null;
    const txData  = txResult.status           === 'fulfilled' ? txResult.value           : null;
    const invData = invoicesResult.status     === 'fulfilled' ? invoicesResult.value     : null;
    const overdue = overdueResult.status      === 'fulfilled' ? overdueResult.value      : null;
    const topupCfg = topupConfigResult.status === 'fulfilled' ? topupConfigResult.value  : null;

    const transactions: Transaction[] = txData?.data  ?? [];
    const invoices:     Invoice[]     = invData?.data ?? [];
    const currency = wallet?.currency ?? topupCfg?.currency ?? 'CNY';
    const loadError = !wallet && !usage;

    return (
        <div className="flex-1 space-y-6 pt-6">
            <Suspense>
                <PaymentStatusToast />
            </Suspense>

            <PageTitle
                title="Billing"
                subtitle="View your wallet balance, usage costs, transactions and invoices."
            />
            <BreadcrumbSetter items={[{ name: "Billing" }]} />

            {loadError && (
                <Alert variant="destructive">
                    <AlertCircle className="h-4 w-4" />
                    <AlertTitle>Could not load billing data</AlertTitle>
                    <AlertDescription>The billing service may be unavailable. Please try again later.</AlertDescription>
                </Alert>
            )}

            {wallet && (
                <WalletCard
                    balance={wallet.balance}
                    currency={currency}
                    isOverdue={overdue?.is_overdue ?? false}
                    mtdCost={usage?.mtd_cost ?? 0}
                    activeApps={usage?.active_apps ?? 0}
                    activeDatabases={usage?.active_databases ?? 0}
                    lastSnapshot={usage?.last_snapshot ?? null}
                    stripeEnabled={topupCfg?.enabled ?? false}
                    stripeCurrency={topupCfg?.currency ?? 'cny'}
                    topupAmounts={topupCfg?.topup_amounts ?? [1000, 5000, 10000, 50000]}
                />
            )}

            <UsageChart currency={currency} />

            <div className="grid gap-6 lg:grid-cols-2">
                <Card>
                    <CardHeader>
                        <CardTitle className="text-base">
                            Transactions
                            {txData && txData.total > 0 && (
                                <span className="ml-2 text-xs font-normal text-muted-foreground">
                                    ({txData.total} total)
                                </span>
                            )}
                        </CardTitle>
                    </CardHeader>
                    <CardContent>
                        <TransactionsTable
                            transactions={transactions}
                            total={txData?.total ?? 0}
                            page={txPage}
                            currency={currency}
                        />
                    </CardContent>
                </Card>

                <Card>
                    <CardHeader>
                        <CardTitle className="text-base">
                            Invoices
                            {invData && invData.total > 0 && (
                                <span className="ml-2 text-xs font-normal text-muted-foreground">
                                    ({invData.total} total)
                                </span>
                            )}
                        </CardTitle>
                    </CardHeader>
                    <CardContent>
                        <InvoicesTable
                            invoices={invoices}
                            total={invData?.total ?? 0}
                            page={invPage}
                            currency={currency}
                        />
                    </CardContent>
                </Card>
            </div>

            {(topupCfg?.enabled) && (
                <Card>
                    <CardHeader>
                        <CardTitle className="text-base">Top-up History</CardTitle>
                    </CardHeader>
                    <CardContent>
                        <TopupHistoryTable />
                    </CardContent>
                </Card>
            )}
        </div>
    );
}
