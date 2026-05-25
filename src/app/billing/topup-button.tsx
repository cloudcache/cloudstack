'use client'

import { useState, useEffect } from "react";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Plus, Loader2, ExternalLink } from "lucide-react";
import { createTopup } from "./actions";
import { toast } from "sonner";

interface TopupButtonProps {
    enabled: boolean;
    currency: string;
    presetAmounts: number[];
}

function formatUnit(amount: number, currency: string) {
    const major = amount / 100;
    return new Intl.NumberFormat('en-US', { style: 'currency', currency: currency || 'CNY' }).format(major);
}

export default function TopupButton({ enabled, currency, presetAmounts }: TopupButtonProps) {
    const [open, setOpen] = useState(false);
    const [selected, setSelected] = useState<number | null>(null);
    const [customAmount, setCustomAmount] = useState('');
    const [loading, setLoading] = useState(false);

    if (!enabled) return null;

    const effectiveAmount = selected ?? (customAmount ? Math.round(parseFloat(customAmount) * 100) : 0);

    const handleTopup = async () => {
        if (effectiveAmount <= 0) {
            toast.error('Please select or enter an amount');
            return;
        }
        setLoading(true);
        try {
            const result = await createTopup(effectiveAmount);
            window.location.href = result.checkout_url;
        } catch (e: any) {
            toast.error(e.message ?? 'Failed to create checkout session');
            setLoading(false);
        }
    };

    return (
        <Dialog open={open} onOpenChange={setOpen}>
            <DialogTrigger asChild>
                <Button size="sm">
                    <Plus className="mr-2 h-4 w-4" />Top Up
                </Button>
            </DialogTrigger>
            <DialogContent className="sm:max-w-md">
                <DialogHeader>
                    <DialogTitle>Top Up Wallet</DialogTitle>
                    <DialogDescription>
                        Select an amount or enter a custom value. You will be redirected to Stripe to complete the payment.
                    </DialogDescription>
                </DialogHeader>

                <div className="space-y-4 py-2">
                    <div className="grid grid-cols-2 gap-2">
                        {presetAmounts.map(amt => (
                            <Button
                                key={amt}
                                variant={selected === amt ? "default" : "outline"}
                                className="h-12 text-base"
                                onClick={() => { setSelected(amt); setCustomAmount(''); }}
                            >
                                {formatUnit(amt, currency)}
                            </Button>
                        ))}
                    </div>

                    <div className="flex items-center gap-2">
                        <span className="text-sm text-muted-foreground whitespace-nowrap">Custom:</span>
                        <Input
                            type="number"
                            min="0.01"
                            step="0.01"
                            placeholder="0.00"
                            value={customAmount}
                            onChange={e => { setCustomAmount(e.target.value); setSelected(null); }}
                        />
                        <span className="text-sm text-muted-foreground uppercase">{currency}</span>
                    </div>

                    <Button
                        className="w-full"
                        disabled={loading || effectiveAmount <= 0}
                        onClick={handleTopup}
                    >
                        {loading ? (
                            <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                        ) : (
                            <ExternalLink className="mr-2 h-4 w-4" />
                        )}
                        {effectiveAmount > 0
                            ? `Pay ${formatUnit(effectiveAmount, currency)} via Stripe`
                            : 'Select amount'}
                    </Button>
                </div>
            </DialogContent>
        </Dialog>
    );
}
