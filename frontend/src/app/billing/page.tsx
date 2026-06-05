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
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { AlertCircle } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Progress } from "@/components/ui/progress";
import Link from "next/link";
import { getT } from "@/i18n/server";

interface Props {
    searchParams?: Promise<{ tx_page?: string; inv_page?: string; payment?: string }>;
}

export default async function BillingPage({ searchParams }: Props) {
    await getAuthUserSession();
    const token = await getBackendToken();
    const { t } = await getT();

    const sp = await searchParams ?? {};
    const txPage  = Math.max(1, parseInt(sp.tx_page  ?? '1', 10) || 1);
    const invPage = Math.max(1, parseInt(sp.inv_page ?? '1', 10) || 1);

    const [walletResult, usageResult, txResult, invoicesResult, overdueResult, topupConfigResult, subResult] = await Promise.allSettled([
        backend.billing.wallet(token),
        backend.billing.currentUsage(token),
        backend.billing.listTransactions(token, { page: txPage,  per_page: TX_PER_PAGE }),
        backend.billing.listInvoices(token,     { page: invPage, per_page: INV_PER_PAGE }),
        backend.billing.overdueStatus(token),
        backend.billing.topupConfig(token),
        backend.subscription.get(token),
    ]);

    const wallet  = walletResult.status       === 'fulfilled' ? walletResult.value       : null;
    const usage   = usageResult.status        === 'fulfilled' ? usageResult.value        : null;
    const txData  = txResult.status           === 'fulfilled' ? txResult.value           : null;
    const invData = invoicesResult.status     === 'fulfilled' ? invoicesResult.value     : null;
    const overdue = overdueResult.status      === 'fulfilled' ? overdueResult.value      : null;
    const topupCfg = topupConfigResult.status === 'fulfilled' ? topupConfigResult.value  : null;
    const subscription: any = subResult.status === 'fulfilled' ? subResult.value : null;

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
                title={t("page.billing.title")}
                subtitle={t("page.billing.subtitle")}
            />
            <BreadcrumbSetter items={[{ name: t("page.billing.title") }]} />

            {/* Subscription & Quota Status */}
            <Card>
                <CardHeader className="pb-3">
                    <div className="flex items-center justify-between">
                        <div>
                            <CardTitle className="text-base">{t("page.billing.subscription")}</CardTitle>
                            <CardDescription>
                                {subscription?.status === 'ACTIVE' ? (
                                    <>
                                        {t('page.billing.plan')}: <strong>{subscription.plan_display_name ?? subscription.plan_name ?? t("common.unknown")}</strong>
                                        {' '}&middot; <Badge variant="default">{subscription.status}</Badge>
                                        {subscription.expires_at && (
                                            <> &middot; {t('page.billing.expires')}: {new Date(subscription.expires_at).toLocaleDateString()}</>
                                        )}
                                    </>
                                ) : (
                                    t('page.billing.noActiveSubscription')
                                )}
                            </CardDescription>
                        </div>
                        <Link href="/plans" className="text-sm text-primary hover:underline">
                            {subscription?.status === 'ACTIVE' ? t('page.billing.changePlan') : t('page.billing.browsePlans')}
                        </Link>
                    </div>
                </CardHeader>
                {subscription?.quota && (
                    <CardContent>
                        <div className="grid grid-cols-2 md:grid-cols-4 gap-4 text-sm">
                            <QuotaBar label="CPU" used={subscription.usage?.cpu_mcores ?? 0} total={subscription.quota.cpu_mcores} unit="m" />
                            <QuotaBar label="Memory" used={subscription.usage?.mem_mb ?? 0} total={subscription.quota.mem_mb} unit="MB" />
                            <QuotaBar label="Storage" used={subscription.usage?.storage_gb ?? 0} total={subscription.quota.storage_gb} unit="GB" />
                            <QuotaBar label="Apps" used={subscription.usage?.app_count ?? 0} total={subscription.quota.app_count} unit="" />
                        </div>
                    </CardContent>
                )}
            </Card>

            {loadError && (
                <Alert variant="destructive">
                    <AlertCircle className="h-4 w-4" />
                    <AlertTitle>{t('page.billing.loadFailed')}</AlertTitle>
                    <AlertDescription>{t('page.billing.serviceUnavailable')}</AlertDescription>
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
                            {t('page.billing.transactions')}
                            {txData && txData.total > 0 && (
                                <span className="ml-2 text-xs font-normal text-muted-foreground">
                                    ({t('common.totalCount', { count: txData.total })})
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
                            {t('page.billing.invoices')}
                            {invData && invData.total > 0 && (
                                <span className="ml-2 text-xs font-normal text-muted-foreground">
                                    ({t('common.totalCount', { count: invData.total })})
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
                        <CardTitle className="text-base">{t("page.billing.topUpHistory")}</CardTitle>
                    </CardHeader>
                    <CardContent>
                        <TopupHistoryTable />
                    </CardContent>
                </Card>
            )}
        </div>
    );
}

function QuotaBar({ label, used, total, unit }: { label: string; used: number; total: number; unit: string }) {
    const pct = total > 0 ? Math.min(100, (used / total) * 100) : 0;
    return (
        <div className="space-y-1">
            <div className="flex justify-between text-xs">
                <span className="text-muted-foreground">{label}</span>
                <span>{used}{unit} / {total}{unit}</span>
            </div>
            <Progress value={pct} className="h-2" />
        </div>
    );
}
