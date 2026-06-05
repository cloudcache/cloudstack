'use client'

import { useEffect } from "react";
import { useSearchParams } from "next/navigation";
import { toast } from "sonner";
import { useT } from "@/i18n";

export default function PaymentStatusToast() {
    const searchParams = useSearchParams();
    const t = useT();
    const payment = searchParams.get('payment');

    useEffect(() => {
        if (payment === 'success') {
            toast.success(t("page.billing.paymentSuccessful"));
        } else if (payment === 'cancelled') {
            toast.info(t("page.billing.paymentCancelled"));
        }
    }, [payment]);

    return null;
}
