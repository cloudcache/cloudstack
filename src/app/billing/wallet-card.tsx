'use client'

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { AlertTriangle, CreditCard, Cpu, Database, TrendingUp } from "lucide-react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import TopupButton from "./topup-button";

interface WalletCardProps {
    balance: number;
    currency: string;
    isOverdue: boolean;
    mtdCost: number;
    activeApps: number;
    activeDatabases: number;
    lastSnapshot?: {
        time: string;
        cpu_mcores: number;
        mem_mb: number;
        storage_gb: number;
        hourly_cost: number;
    } | null;
    stripeEnabled: boolean;
    stripeCurrency: string;
    topupAmounts: number[];
}

function formatAmount(amount: number, currency: string) {
    return new Intl.NumberFormat('en-US', { style: 'currency', currency: currency || 'CNY' }).format(amount);
}

export default function WalletCard({
    balance, currency, isOverdue, mtdCost, activeApps, activeDatabases, lastSnapshot,
    stripeEnabled, stripeCurrency, topupAmounts,
}: WalletCardProps) {
    return (
        <div className="space-y-4">
            {isOverdue && (
                <Alert variant="destructive">
                    <AlertTriangle className="h-4 w-4" />
                    <AlertDescription>
                        Your account balance is overdue. Please top up your wallet to avoid service interruption.
                    </AlertDescription>
                </Alert>
            )}
            <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
                <Card>
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                        <CardTitle className="text-sm font-medium">Wallet Balance</CardTitle>
                        <CreditCard className="h-4 w-4 text-muted-foreground" />
                    </CardHeader>
                    <CardContent>
                        <div className={`text-2xl font-bold ${balance < 0 ? 'text-red-500' : ''}`}>
                            {formatAmount(balance, currency)}
                        </div>
                        <div className="flex items-center gap-2 mt-2">
                            {isOverdue && (
                                <Badge variant="destructive">Overdue</Badge>
                            )}
                            <TopupButton
                                enabled={stripeEnabled}
                                currency={stripeCurrency}
                                presetAmounts={topupAmounts}
                            />
                        </div>
                    </CardContent>
                </Card>

                <Card>
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                        <CardTitle className="text-sm font-medium">Month-to-Date Cost</CardTitle>
                        <TrendingUp className="h-4 w-4 text-muted-foreground" />
                    </CardHeader>
                    <CardContent>
                        <div className="text-2xl font-bold">{formatAmount(mtdCost, currency)}</div>
                        {lastSnapshot && (
                            <p className="text-xs text-muted-foreground mt-1">
                                {formatAmount(lastSnapshot.hourly_cost, currency)}/hr
                            </p>
                        )}
                    </CardContent>
                </Card>

                <Card>
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                        <CardTitle className="text-sm font-medium">Active Apps</CardTitle>
                        <Cpu className="h-4 w-4 text-muted-foreground" />
                    </CardHeader>
                    <CardContent>
                        <div className="text-2xl font-bold">{activeApps}</div>
                        {lastSnapshot && (
                            <p className="text-xs text-muted-foreground mt-1">
                                {lastSnapshot.cpu_mcores}m CPU · {lastSnapshot.mem_mb} MB RAM
                            </p>
                        )}
                    </CardContent>
                </Card>

                <Card>
                    <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                        <CardTitle className="text-sm font-medium">Active Databases</CardTitle>
                        <Database className="h-4 w-4 text-muted-foreground" />
                    </CardHeader>
                    <CardContent>
                        <div className="text-2xl font-bold">{activeDatabases}</div>
                        {lastSnapshot && (
                            <p className="text-xs text-muted-foreground mt-1">
                                {lastSnapshot.storage_gb} GB storage
                            </p>
                        )}
                    </CardContent>
                </Card>
            </div>
        </div>
    );
}
